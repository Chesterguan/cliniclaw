use std::collections::HashMap;
use std::sync::Arc;

use base64::Engine as _;
use cliniclaw_fhir::{Attachment, CodeableConcept, Coding, DiagnosticReport, Reference};
use cliniclaw_persist::{sha256_hash, AuditEvent};
use cliniclaw_policy::{ActionContext, Capability, PolicyDecision, PolicyEngine};

use cliniclaw_kernel::Confidence;

use crate::error::AgentError;
use crate::llm::LlmCapability;
use crate::PromptEnvelope;

#[derive(Debug, Clone)]
pub struct LabReviewInput {
    pub encounter_id: String,
    pub encounter_status: String,
    pub patient_id: String,
    pub practitioner_id: String,
    /// Free-text representation of lab results (e.g. "Na 138, K 3.2 L, Cr 1.4 H, …")
    pub lab_results_text: String,
    /// Active diagnoses/conditions providing clinical context for interpretation
    pub active_conditions: Vec<String>,
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
    /// Encounter class code (e.g. "AMB", "IMP")
    pub encounter_class: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LabReviewOutput {
    /// Narrative clinical interpretation of the lab panel
    pub interpretation: String,
    /// Abnormal or clinically significant flags (e.g. "Potassium low — hypokalemia risk")
    pub flags: Vec<String>,
    /// Ordered list of follow-up recommendations
    pub follow_up_recommendations: Vec<String>,
    /// FHIR DiagnosticReport carrying the interpretation (LOINC 11502-2)
    pub report: DiagnosticReport,
    pub confidence: Confidence,
    pub audit_event: AuditEvent,
    pub policy_decision: PolicyDecision,
    /// SHA-256 hash of the matched skill spec (if any)
    pub spec_hash: Option<String>,
}

pub struct LabReviewAgent {
    llm: Arc<dyn LlmCapability>,
}

impl LabReviewAgent {
    pub fn new(llm: Arc<dyn LlmCapability>) -> Self {
        Self { llm }
    }

    /// Run the lab review workflow.
    /// 1. Skill-aware policy check  2. Build prompt  3. Call Claude
    /// 4. Parse response  5. Build DiagnosticReport  6. Verify output  7. Build audit event
    ///
    /// Note: the agent *builds* the audit event but does not persist it.
    /// Persistence is the API layer's responsibility (VERITAS separation).
    pub async fn interpret(
        &self,
        input: &LabReviewInput,
        policy_engine: &PolicyEngine,
    ) -> Result<LabReviewOutput, AgentError> {
        // Step 1: Build context and run skill-aware policy evaluation
        let context = self.build_context(input);
        let skill_eval = policy_engine.evaluate_with_skill(&context)?;

        match &skill_eval.decision {
            PolicyDecision::Deny => {
                tracing::warn!(
                    actor_id = %input.practitioner_id,
                    encounter_id = %input.encounter_id,
                    "policy denied lab review"
                );
                return Err(AgentError::PolicyDenied(format!(
                    "lab review denied for encounter {}",
                    input.encounter_id
                )));
            }
            PolicyDecision::RequireApproval => {
                return Err(AgentError::RequiresApproval {
                    action: "lab_review.interpret".to_string(),
                });
            }
            PolicyDecision::Allow => {
                tracing::info!(
                    actor_id = %input.practitioner_id,
                    encounter_id = %input.encounter_id,
                    spec_hash = ?skill_eval.spec_hash,
                    "policy allowed lab review"
                );
            }
        }

        // Step 2: Build de-identified prompt
        let prompt = self.build_prompt(input);

        // Step 3: Call LLM
        let response_text = self.llm.call(&prompt).await?;

        // Step 4: Parse response
        let parsed: serde_json::Value = serde_json::from_str(&response_text).map_err(|e| {
            AgentError::ClaudeApi(format!("failed to parse lab review response: {e}"))
        })?;

        let interpretation = parsed
            .get("interpretation")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let flags: Vec<String> = parsed
            .get("flags")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();

        let follow_up_recommendations: Vec<String> = parsed
            .get("follow_up")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();

        // Step 5: Build FHIR DiagnosticReport
        let report = self.build_report(input, &interpretation);

        // Step 6: Verify output
        self.verify_output(&report)?;

        // Step 7: Create audit event with skill metadata
        let input_descriptor = serde_json::to_vec(&serde_json::json!({
            "encounter_id": input.encounter_id,
            "practitioner_id": input.practitioner_id,
            "lab_results_len": input.lab_results_text.len(),
            "condition_count": input.active_conditions.len(),
            "skill_id": skill_eval.skill_id,
            "spec_hash": skill_eval.spec_hash,
        }))?;
        let output_descriptor = serde_json::to_vec(&report)?;

        let audit_event = AuditEvent::new(
            &input.practitioner_id,
            Some(input.patient_id.clone()),
            "lab_review.interpret",
            &skill_eval.decision.to_string(),
            sha256_hash(&input_descriptor),
            sha256_hash(&output_descriptor),
            "", // previous_hash assigned atomically by SqliteAuditStore::append
        );

        tracing::info!(
            audit_event_id = %audit_event.id,
            encounter_id = %input.encounter_id,
            flag_count = flags.len(),
            "lab review interpretation completed and verified"
        );

        let confidence = self.compute_confidence(&interpretation, &flags);

        Ok(LabReviewOutput {
            interpretation,
            flags,
            follow_up_recommendations,
            report,
            confidence,
            audit_event,
            policy_decision: skill_eval.decision,
            spec_hash: skill_eval.spec_hash,
        })
    }

    fn build_context(&self, input: &LabReviewInput) -> ActionContext {
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
            action: "lab_review.interpret".to_string(),
            actor_id: input.practitioner_id.clone(),
            capabilities: input.capabilities.clone(),
            resource_type: Some("DiagnosticReport".to_string()),
            properties: props,
            capability_tokens: input.capability_tokens.clone(),
            role: input.practitioner_role.clone(),
            patient_id: Some(input.patient_id.clone()),
            encounter_id: Some(input.encounter_id.clone()),
        }
    }

    fn build_prompt(&self, input: &LabReviewInput) -> PromptEnvelope {
        let system = "You are a clinical laboratory interpretation assistant. Review the provided \
             lab results in context of the patient's active conditions and generate a structured \
             interpretation. Output ONLY valid JSON matching this schema:\n\
             {\n\
               \"interpretation\": \"narrative paragraph\",\n\
               \"flags\": [\"flag 1\", \"flag 2\"],\n\
               \"follow_up\": [\"recommendation 1\", \"recommendation 2\"]\n\
             }\n\n\
             Rules:\n\
             - Flag any result outside reference range with clinical significance\n\
             - Interpret results in the context of the active conditions provided\n\
             - Recommend follow-up based on the findings, not assumptions\n\
             - Do not fabricate results or diagnoses not supported by the data\n\
             - Output raw JSON only, no markdown fences"
            .to_string();

        let mut parts = vec![format!("Lab results:\n{}", input.lab_results_text)];

        if !input.active_conditions.is_empty() {
            parts.push(format!(
                "Active conditions: {}",
                input.active_conditions.join(", ")
            ));
        }

        PromptEnvelope::build(system, parts.join("\n\n"))
    }

    fn build_report(&self, input: &LabReviewInput, interpretation: &str) -> DiagnosticReport {
        DiagnosticReport {
            id: None,
            resource_type: "DiagnosticReport".to_string(),
            status: "preliminary".to_string(),
            code: CodeableConcept {
                coding: Some(vec![Coding {
                    system: Some("http://loinc.org".to_string()),
                    // LOINC 11502-2: Laboratory report
                    code: Some("11502-2".to_string()),
                    display: Some("Laboratory report".to_string()),
                }]),
                text: Some("AI-assisted laboratory interpretation".to_string()),
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
                data: Some(
                    base64::engine::general_purpose::STANDARD.encode(interpretation),
                ),
                url: None,
                title: Some("AI-assisted lab interpretation".to_string()),
            }]),
        }
    }

    fn compute_confidence(&self, interpretation: &str, flags: &[String]) -> Confidence {
        let mut score = 0.5;
        let mut factors = Vec::new();

        // Substantive interpretation length
        if interpretation.len() > 200 {
            factors.push("detailed_interpretation".to_string());
            score += 0.15;
        } else if interpretation.len() > 50 {
            factors.push("adequate_interpretation".to_string());
            score += 0.1;
        }

        // Presence of clinical flags indicates the model engaged with abnormals
        if !flags.is_empty() {
            factors.push("flags_identified".to_string());
            score += 0.1;
        }

        // Many flags may indicate complex panel — lower confidence in completeness
        if flags.len() > 5 {
            factors.push("high_flag_count".to_string());
            score -= 0.05;
        }

        // Clean panel (no flags) is unambiguous
        if flags.is_empty() && !interpretation.is_empty() {
            factors.push("clean_panel".to_string());
            score += 0.1;
        }

        Confidence::new(score, factors)
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

        // Verify the interpretation data is valid base64 and non-trivial
        if let Some(ref data) = form[0].data {
            let decoded = base64::engine::general_purpose::STANDARD
                .decode(data)
                .map_err(|_| {
                    AgentError::VerificationFailed(
                        "presentedForm data is not valid base64".to_string(),
                    )
                })?;
            if decoded.len() < 10 {
                return Err(AgentError::VerificationFailed(
                    "lab interpretation is too short to be clinically meaningful".to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_report_structure() {
        let agent = LabReviewAgent::new(Arc::new(crate::MockClaudeCapability::new()));
        let input = LabReviewInput {
            encounter_id: "enc-001".to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: "patient-001".to_string(),
            practitioner_id: "physician-001".to_string(),
            lab_results_text: "Na 140, K 3.1 L, Cr 1.6 H, BUN 28 H, Hgb 11.2 L".to_string(),
            active_conditions: vec!["Chronic kidney disease stage 3".to_string()],
            capabilities: vec!["lab_review".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("physician".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("IMP".to_string()),
        };

        let report = agent.build_report(
            &input,
            "Mild hypokalemia and elevated creatinine consistent with CKD stage 3.",
        );
        assert_eq!(report.resource_type, "DiagnosticReport");
        assert_eq!(report.status, "preliminary");
        assert!(report.subject.is_some());
        assert!(report.encounter.is_some());
        assert!(report.presented_form.is_some());
        let code = report.code.coding.as_ref().unwrap();
        assert_eq!(code[0].code, Some("11502-2".to_string()));
    }

    #[test]
    fn test_verify_rejects_empty_interpretation() {
        let agent = LabReviewAgent::new(Arc::new(crate::MockClaudeCapability::new()));
        let input = LabReviewInput {
            encounter_id: "enc-001".to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: "patient-001".to_string(),
            practitioner_id: "physician-001".to_string(),
            lab_results_text: "Na 140".to_string(),
            active_conditions: vec![],
            capabilities: vec!["lab_review".to_string()],
            capability_tokens: vec![],
            practitioner_role: None,
            patient_active: true,
            patient_deceased: None,
            encounter_class: None,
        };

        // Empty interpretation → base64 of "" is too short
        let report = agent.build_report(&input, "");
        assert!(agent.verify_output(&report).is_err());
    }
}
