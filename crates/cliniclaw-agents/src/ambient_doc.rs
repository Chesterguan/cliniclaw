use std::collections::HashMap;

use base64::Engine as _;
use cliniclaw_fhir::{Attachment, CodeableConcept, Coding, DiagnosticReport, Reference};
use cliniclaw_persist::{sha256_hash, AuditEvent};
use cliniclaw_policy::{ActionContext, Capability, PolicyDecision, PolicyEngine};

use crate::{AgentError, ClaudeCapability, PromptEnvelope};

#[derive(Debug, Clone)]
pub struct AmbientDocInput {
    pub encounter_id: String,
    pub encounter_status: String,
    pub patient_id: String,
    pub practitioner_id: String,
    pub transcript: String,
    pub chief_complaint: Option<String>,
    pub active_medications: Vec<String>,
    /// Bare capability names (backward-compatible path)
    pub capabilities: Vec<String>,
    /// Structured capability tokens for skill-aware evaluation
    pub capability_tokens: Vec<Capability>,
    /// The practitioner's clinical role (e.g. "physician")
    pub practitioner_role: Option<String>,
    /// Whether the patient is active in the system
    pub patient_active: bool,
    /// Whether the patient is deceased
    pub patient_deceased: Option<bool>,
    /// Encounter class code (e.g. "AMB", "EMER", "IMP")
    pub encounter_class: Option<String>,
}

impl AmbientDocInput {
    /// Create a minimal input (backward-compatible convenience constructor).
    pub fn new(
        encounter_id: String,
        encounter_status: String,
        patient_id: String,
        practitioner_id: String,
        transcript: String,
    ) -> Self {
        Self {
            encounter_id,
            encounter_status,
            patient_id,
            practitioner_id,
            transcript,
            chief_complaint: None,
            active_medications: Vec::new(),
            capabilities: vec!["note_generation".to_string()],
            capability_tokens: Vec::new(),
            practitioner_role: None,
            patient_active: true,
            patient_deceased: None,
            encounter_class: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AmbientDocOutput {
    pub report: DiagnosticReport,
    pub audit_event: AuditEvent,
    pub policy_decision: PolicyDecision,
    /// SHA-256 hash of the matched skill spec (if any)
    pub spec_hash: Option<String>,
}

pub struct AmbientDocAgent {
    claude: ClaudeCapability,
}

impl AmbientDocAgent {
    pub fn new(claude: ClaudeCapability) -> Self {
        Self { claude }
    }

    /// Run the ambient documentation workflow.
    /// 1. Skill-aware policy check  2. Build prompt  3. Call Claude
    /// 4. Build report  5. Verify output  6. Build audit event
    ///
    /// Note: the agent *builds* the audit event but does not persist it.
    /// Persistence is the API layer's responsibility (VERITAS separation).
    pub async fn generate_note(
        &self,
        input: &AmbientDocInput,
        policy_engine: &PolicyEngine,
        previous_audit_hash: &str,
    ) -> Result<AmbientDocOutput, AgentError> {
        // Step 1: Build context and run skill-aware policy evaluation
        let context = self.build_context(input);
        let skill_eval = policy_engine.evaluate_with_skill(&context)?;

        match &skill_eval.decision {
            PolicyDecision::Deny => {
                tracing::warn!(
                    actor_id = %input.practitioner_id,
                    encounter_id = %input.encounter_id,
                    "policy denied ambient note generation"
                );
                return Err(AgentError::PolicyDenied(format!(
                    "note generation denied for encounter {}",
                    input.encounter_id
                )));
            }
            PolicyDecision::RequireApproval => {
                return Err(AgentError::RequiresApproval {
                    action: "ambient_doc.generate_note".to_string(),
                });
            }
            PolicyDecision::Allow => {
                tracing::info!(
                    actor_id = %input.practitioner_id,
                    encounter_id = %input.encounter_id,
                    spec_hash = ?skill_eval.spec_hash,
                    "policy allowed ambient note generation"
                );
            }
        }

        // Step 2: Build de-identified prompt
        let prompt = self.build_prompt(input);

        // Step 3: Call Claude API
        let response_text = self.claude.call(&prompt).await?;

        // Step 4: Build FHIR DiagnosticReport
        let report = self.build_report(input, &response_text);

        // Step 5: Verify output
        self.verify_output(&report)?;

        // Step 6: Create audit event with skill metadata
        let input_descriptor = serde_json::to_vec(&serde_json::json!({
            "encounter_id": input.encounter_id,
            "practitioner_id": input.practitioner_id,
            "transcript_len": input.transcript.len(),
            "skill_id": skill_eval.skill_id,
            "spec_hash": skill_eval.spec_hash,
        }))?;
        let output_descriptor = serde_json::to_vec(&report)?;

        let audit_event = AuditEvent::new(
            &input.practitioner_id,
            Some(input.patient_id.clone()),
            "ambient_doc.generate_note",
            &format!("{:?}", skill_eval.decision),
            sha256_hash(&input_descriptor),
            sha256_hash(&output_descriptor),
            previous_audit_hash,
        );

        tracing::info!(
            audit_event_id = %audit_event.id,
            encounter_id = %input.encounter_id,
            "ambient note generated and verified successfully"
        );

        Ok(AmbientDocOutput {
            report,
            audit_event,
            policy_decision: skill_eval.decision,
            spec_hash: skill_eval.spec_hash,
        })
    }

    fn build_context(&self, input: &AmbientDocInput) -> ActionContext {
        let mut props = HashMap::new();
        // Backward-compatible key (for existing rule conditions)
        props.insert("encounter_status".to_string(), input.encounter_status.clone());
        // New dotted convention (for skill population criteria)
        props.insert("encounter.status".to_string(), input.encounter_status.clone());
        props.insert("patient.active".to_string(), input.patient_active.to_string());
        if let Some(deceased) = input.patient_deceased {
            props.insert("patient.deceased".to_string(), deceased.to_string());
        }
        if let Some(ref enc_class) = input.encounter_class {
            props.insert("encounter.class".to_string(), enc_class.clone());
        }

        ActionContext {
            action: "ambient_doc.generate_note".to_string(),
            actor_id: input.practitioner_id.clone(),
            capabilities: input.capabilities.clone(),
            resource_type: Some("Encounter".to_string()),
            properties: props,
            capability_tokens: input.capability_tokens.clone(),
            role: input.practitioner_role.clone(),
            patient_id: Some(input.patient_id.clone()),
            encounter_id: Some(input.encounter_id.clone()),
        }
    }

    fn build_prompt(&self, input: &AmbientDocInput) -> PromptEnvelope {
        let system = "You are a clinical documentation assistant. Generate a structured SOAP note \
             from the provided encounter transcript. Output ONLY valid JSON matching this schema:\n\
             {\n\
               \"subjective\": \"...\",\n\
               \"objective\": \"...\",\n\
               \"assessment\": \"...\",\n\
               \"plan\": \"...\",\n\
               \"icd10_codes\": [\"code1\", \"code2\"]\n\
             }\n\n\
             Rules:\n\
             - Be concise and clinically accurate\n\
             - Use standard medical terminology\n\
             - Only include ICD-10 codes if confidence is high\n\
             - Do not fabricate findings not present in the transcript\n\
             - Output raw JSON only, no markdown fences"
            .to_string();

        let mut parts = vec![format!("Transcript:\n{}", input.transcript)];

        if let Some(cc) = &input.chief_complaint {
            parts.push(format!("Chief complaint: {cc}"));
        }

        if !input.active_medications.is_empty() {
            parts.push(format!(
                "Active medications: {}",
                input.active_medications.join(", ")
            ));
        }

        PromptEnvelope::build(system, parts.join("\n\n"))
    }

    fn build_report(&self, input: &AmbientDocInput, note_text: &str) -> DiagnosticReport {
        DiagnosticReport {
            id: None,
            resource_type: "DiagnosticReport".to_string(),
            status: "preliminary".to_string(),
            code: CodeableConcept {
                coding: Some(vec![Coding {
                    system: Some("http://loinc.org".to_string()),
                    code: Some("11506-3".to_string()),
                    display: Some("Progress note".to_string()),
                }]),
                text: Some("AI-generated clinical note".to_string()),
            },
            subject: Some(Reference {
                reference: Some(format!("Patient/{}", input.patient_id)),
                display: None,
                type_: None,
            }),
            encounter: Some(Reference {
                reference: Some(format!("Encounter/{}", input.encounter_id)),
                display: None,
                type_: None,
            }),
            issued: Some(chrono::Utc::now().to_rfc3339()),
            conclusion: None,
            presented_form: Some(vec![Attachment {
                content_type: Some("text/plain".to_string()),
                data: Some(base64::engine::general_purpose::STANDARD.encode(note_text)),
                url: None,
                title: Some("AI-generated SOAP note".to_string()),
            }]),
        }
    }

    fn verify_output(&self, report: &DiagnosticReport) -> Result<(), AgentError> {
        if report.status != "preliminary" {
            return Err(AgentError::VerificationFailed(
                "DiagnosticReport status must be 'preliminary'".to_string(),
            ));
        }

        let form = report.presented_form.as_ref().ok_or_else(|| {
            AgentError::VerificationFailed(
                "DiagnosticReport must include presentedForm".to_string(),
            )
        })?;

        if form.is_empty() {
            return Err(AgentError::VerificationFailed(
                "DiagnosticReport presentedForm must not be empty".to_string(),
            ));
        }

        // Verify the attachment data is valid base64 and non-trivial
        if let Some(ref data) = form[0].data {
            let decoded = base64::engine::general_purpose::STANDARD
                .decode(data)
                .map_err(|_| {
                    AgentError::VerificationFailed(
                        "presentedForm data is not valid base64".to_string(),
                    )
                })?;
            if decoded.len() < 20 {
                return Err(AgentError::VerificationFailed(
                    "generated note is too short to be a valid clinical note".to_string(),
                ));
            }
        } else {
            return Err(AgentError::VerificationFailed(
                "presentedForm attachment must include data".to_string(),
            ));
        }

        if report.subject.is_none() {
            return Err(AgentError::VerificationFailed(
                "DiagnosticReport must include a subject reference".to_string(),
            ));
        }

        if report.encounter.is_none() {
            return Err(AgentError::VerificationFailed(
                "DiagnosticReport must include an encounter reference".to_string(),
            ));
        }

        Ok(())
    }
}
