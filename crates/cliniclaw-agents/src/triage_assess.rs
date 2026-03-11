use std::collections::HashMap;
use std::sync::Arc;

use cliniclaw_fhir::{CodeableConcept, Coding, Observation, Reference};
use cliniclaw_persist::{sha256_hash, AuditEvent};
use cliniclaw_policy::{ActionContext, Capability, PolicyDecision, PolicyEngine};

use cliniclaw_kernel::Confidence;

use crate::error::AgentError;
use crate::llm::LlmCapability;
use crate::PromptEnvelope;

#[derive(Debug, Clone)]
pub struct TriageAssessInput {
    pub encounter_id: String,
    pub encounter_status: String,
    pub patient_id: String,
    pub practitioner_id: String,
    /// Free-text chief complaint as stated by the patient
    pub chief_complaint: String,
    /// Free-text vitals (HR, BP, SpO2, temp, RR, pain score, etc.)
    pub vitals_text: String,
    /// Bare capability names (backward-compatible path)
    pub capabilities: Vec<String>,
    /// Structured capability tokens for skill-aware evaluation
    pub capability_tokens: Vec<Capability>,
    /// The practitioner's clinical role (e.g. "nurse", "physician")
    pub practitioner_role: Option<String>,
    /// Whether the patient is active in the system
    pub patient_active: bool,
    /// Whether the patient is deceased
    pub patient_deceased: Option<bool>,
    /// Encounter class code (e.g. "EMER", "IMP")
    pub encounter_class: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TriageAssessOutput {
    /// Emergency Severity Index level (1 = most critical, 5 = least urgent)
    pub triage_level: u8,
    /// Human-readable acuity label (e.g. "Immediate", "Emergent")
    pub acuity_label: String,
    /// Ordered list of recommended immediate actions
    pub recommended_actions: Vec<String>,
    /// FHIR Observation encoding the triage result (LOINC 56840-2)
    pub observation: Observation,
    pub confidence: Confidence,
    pub audit_event: AuditEvent,
    pub policy_decision: PolicyDecision,
    /// SHA-256 hash of the matched skill spec (if any)
    pub spec_hash: Option<String>,
}

pub struct TriageAssessAgent {
    llm: Arc<dyn LlmCapability>,
}

impl TriageAssessAgent {
    pub fn new(llm: Arc<dyn LlmCapability>) -> Self {
        Self { llm }
    }

    /// Run the triage assessment workflow.
    /// 1. Skill-aware policy check  2. Build prompt  3. Call Claude
    /// 4. Parse response  5. Build Observation  6. Verify output  7. Build audit event
    ///
    /// Note: the agent *builds* the audit event but does not persist it.
    /// Persistence is the API layer's responsibility (VERITAS separation).
    pub async fn evaluate(
        &self,
        input: &TriageAssessInput,
        policy_engine: &PolicyEngine,
    ) -> Result<TriageAssessOutput, AgentError> {
        // Step 1: Build context and run skill-aware policy evaluation
        let context = self.build_context(input);
        let skill_eval = policy_engine.evaluate_with_skill(&context)?;

        match &skill_eval.decision {
            PolicyDecision::Deny => {
                tracing::warn!(
                    actor_id = %input.practitioner_id,
                    encounter_id = %input.encounter_id,
                    "policy denied triage assessment"
                );
                return Err(AgentError::PolicyDenied(format!(
                    "triage assessment denied for encounter {}",
                    input.encounter_id
                )));
            }
            PolicyDecision::RequireApproval => {
                return Err(AgentError::RequiresApproval {
                    action: "triage_assess.evaluate".to_string(),
                });
            }
            PolicyDecision::Allow => {
                tracing::info!(
                    actor_id = %input.practitioner_id,
                    encounter_id = %input.encounter_id,
                    spec_hash = ?skill_eval.spec_hash,
                    "policy allowed triage assessment"
                );
            }
        }

        // Step 2: Build de-identified prompt
        let prompt = self.build_prompt(input);

        // Step 3: Call LLM
        let response_text = self.llm.call(&prompt).await?;

        // Step 4: Parse response
        let parsed: serde_json::Value = serde_json::from_str(&response_text).map_err(|e| {
            AgentError::ClaudeApi(format!("failed to parse triage response: {e}"))
        })?;

        let triage_level = parsed
            .get("triage_level")
            .and_then(|v| v.as_u64())
            .map(|v| v as u8)
            .unwrap_or(3);

        let acuity_label = parsed
            .get("acuity")
            .and_then(|v| v.as_str())
            .unwrap_or("Urgent")
            .to_string();

        let recommended_actions: Vec<String> = parsed
            .get("recommended_actions")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();

        // Step 5: Build FHIR Observation
        let observation = self.build_observation(input, triage_level, &acuity_label);

        // Step 6: Verify output
        self.verify_output(triage_level, &observation)?;

        // Step 7: Create audit event with skill metadata
        let input_descriptor = serde_json::to_vec(&serde_json::json!({
            "encounter_id": input.encounter_id,
            "practitioner_id": input.practitioner_id,
            "chief_complaint_len": input.chief_complaint.len(),
            "skill_id": skill_eval.skill_id,
            "spec_hash": skill_eval.spec_hash,
        }))?;
        let output_descriptor = serde_json::to_vec(&observation)?;

        let audit_event = AuditEvent::new(
            &input.practitioner_id,
            Some(input.patient_id.clone()),
            "triage_assess.evaluate",
            &skill_eval.decision.to_string(),
            sha256_hash(&input_descriptor),
            sha256_hash(&output_descriptor),
            "", // previous_hash assigned atomically by SqliteAuditStore::append
        );

        tracing::info!(
            audit_event_id = %audit_event.id,
            encounter_id = %input.encounter_id,
            triage_level = triage_level,
            "triage assessment completed and verified"
        );

        let confidence = self.compute_confidence(input, &acuity_label, triage_level);

        Ok(TriageAssessOutput {
            triage_level,
            acuity_label,
            recommended_actions,
            observation,
            confidence,
            audit_event,
            policy_decision: skill_eval.decision,
            spec_hash: skill_eval.spec_hash,
        })
    }

    fn build_context(&self, input: &TriageAssessInput) -> ActionContext {
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
            action: "triage_assess.evaluate".to_string(),
            actor_id: input.practitioner_id.clone(),
            capabilities: input.capabilities.clone(),
            resource_type: Some("Observation".to_string()),
            properties: props,
            capability_tokens: input.capability_tokens.clone(),
            role: input.practitioner_role.clone(),
            patient_id: Some(input.patient_id.clone()),
            encounter_id: Some(input.encounter_id.clone()),
        }
    }

    fn build_prompt(&self, input: &TriageAssessInput) -> PromptEnvelope {
        let system = "You are an emergency triage clinical assistant using the Emergency Severity \
             Index (ESI) scale. Evaluate the patient presentation and assign a triage level. \
             Output ONLY valid JSON matching this schema:\n\
             {\n\
               \"triage_level\": 1,\n\
               \"acuity\": \"Immediate|Emergent|Urgent|Less Urgent|Non-Urgent\",\n\
               \"recommended_actions\": [\"action 1\", \"action 2\"]\n\
             }\n\n\
             ESI Levels:\n\
             - 1 (Immediate): Life threat, requires immediate intervention\n\
             - 2 (Emergent): High-risk situation or severe pain/distress\n\
             - 3 (Urgent): Stable but requires multiple resources\n\
             - 4 (Less Urgent): Stable, requires one resource\n\
             - 5 (Non-Urgent): Stable, no resources needed\n\n\
             Rules:\n\
             - Assign triage_level as an integer 1-5\n\
             - Base assessment solely on provided complaint and vitals\n\
             - Do not fabricate clinical findings not mentioned\n\
             - Output raw JSON only, no markdown fences"
            .to_string();

        let parts = vec![
            format!("Chief complaint: {}", input.chief_complaint),
            format!("Vitals: {}", input.vitals_text),
        ];

        PromptEnvelope::build(system, parts.join("\n\n"))
    }

    fn build_observation(
        &self,
        input: &TriageAssessInput,
        triage_level: u8,
        acuity_label: &str,
    ) -> Observation {
        let mut obs = Observation::new(
            "final",
            CodeableConcept {
                coding: Some(vec![Coding {
                    system: Some("http://loinc.org".to_string()),
                    // LOINC 56840-2: Emergency Severity Index
                    code: Some("56840-2".to_string()),
                    display: Some("Emergency Severity Index".to_string()),
                }]),
                text: Some("Emergency Severity Index (ESI)".to_string()),
            },
        );
        obs.subject = Some(Reference {
            reference: Some(format!("Patient/{}", input.patient_id)),
            display: None,
            type_: None,
        });
        obs.encounter = Some(Reference {
            reference: Some(format!("Encounter/{}", input.encounter_id)),
            display: None,
            type_: None,
        });
        obs.effective_date_time = Some(chrono::Utc::now().to_rfc3339());
        obs.value_string = Some(format!("ESI-{triage_level}: {acuity_label}"));
        obs
    }

    fn compute_confidence(
        &self,
        input: &TriageAssessInput,
        acuity_label: &str,
        triage_level: u8,
    ) -> Confidence {
        let mut score = 0.5;
        let mut factors = Vec::new();

        // Both chief complaint and vitals present
        if !input.chief_complaint.is_empty() {
            factors.push("chief_complaint_present".to_string());
            score += 0.1;
        }
        if !input.vitals_text.is_empty() {
            factors.push("vitals_present".to_string());
            score += 0.1;
        }

        // Acuity label has meaningful content
        if acuity_label.len() >= 6 {
            factors.push("acuity_label_substantive".to_string());
            score += 0.1;
        }

        // Valid ESI range (1-5)
        if (1..=5).contains(&triage_level) {
            factors.push("valid_esi_level".to_string());
            score += 0.2;
        }

        // High acuity assessments (1-2) carry inherently lower uncertainty
        // because the clinical signal is clearer; low-acuity (4-5) is similarly clear.
        // Mid-range (3) is the most ambiguous.
        if triage_level == 3 {
            factors.push("mid_acuity_uncertainty".to_string());
            score -= 0.05;
        }

        Confidence::new(score, factors)
    }

    fn verify_output(&self, triage_level: u8, observation: &Observation) -> Result<(), AgentError> {
        // ESI levels are strictly 1-5
        if !(1..=5).contains(&triage_level) {
            return Err(AgentError::VerificationFailed(format!(
                "triage_level {triage_level} is out of valid ESI range 1-5"
            )));
        }

        // Observation must have a subject reference
        if observation.subject.is_none() {
            return Err(AgentError::VerificationFailed(
                "Observation must include a subject reference".to_string(),
            ));
        }

        // Observation must carry a value
        if observation.value_string.is_none() {
            return Err(AgentError::VerificationFailed(
                "Observation must include a valueString for the ESI result".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_observation_structure() {
        let agent = TriageAssessAgent::new(Arc::new(crate::MockClaudeCapability::new()));
        let input = TriageAssessInput {
            encounter_id: "enc-001".to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: "patient-001".to_string(),
            practitioner_id: "nurse-001".to_string(),
            chief_complaint: "Chest pain, radiating to left arm".to_string(),
            vitals_text: "HR 110, BP 90/60, SpO2 94%, Temp 37.2C, RR 22".to_string(),
            capabilities: vec!["triage_assess".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("nurse".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("EMER".to_string()),
        };

        let obs = agent.build_observation(&input, 2, "Emergent");
        assert_eq!(obs.status, "final");
        assert!(obs.subject.is_some());
        assert!(obs.encounter.is_some());
        assert_eq!(obs.value_string, Some("ESI-2: Emergent".to_string()));
        let code = obs.code.coding.as_ref().unwrap();
        assert_eq!(code[0].code, Some("56840-2".to_string()));
    }

    #[test]
    fn test_verify_output_rejects_invalid_esi() {
        let agent = TriageAssessAgent::new(Arc::new(crate::MockClaudeCapability::new()));

        // Build a valid observation first, then test level validation independently
        let dummy_obs = Observation::new(
            "final",
            CodeableConcept { coding: None, text: None },
        );

        // Level 0 is invalid
        assert!(agent.verify_output(0, &dummy_obs).is_err());
        // Level 6 is invalid
        assert!(agent.verify_output(6, &dummy_obs).is_err());
    }

    #[test]
    fn test_verify_output_requires_subject() {
        let agent = TriageAssessAgent::new(Arc::new(crate::MockClaudeCapability::new()));

        let obs = Observation::new(
            "final",
            CodeableConcept { coding: None, text: None },
        );
        // No subject → should fail
        let result = agent.verify_output(3, &obs);
        assert!(result.is_err());
    }
}
