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
pub struct NurseAssessInput {
    pub encounter_id: String,
    pub encounter_status: String,
    pub patient_id: String,
    pub practitioner_id: String,
    /// Type of nursing assessment: "admission" for initial assessment, "ongoing" for subsequent
    pub assessment_type: String,
    /// Free-text vitals and patient-reported status
    pub vitals_text: String,
    /// Bare capability names (backward-compatible path)
    pub capabilities: Vec<String>,
    /// Structured capability tokens for skill-aware evaluation
    pub capability_tokens: Vec<Capability>,
    /// The practitioner's clinical role (e.g. "nurse")
    pub practitioner_role: Option<String>,
    /// Whether the patient is active in the system
    pub patient_active: bool,
    /// Whether the patient is deceased
    pub patient_deceased: Option<bool>,
    /// Encounter class code (e.g. "IMP", "EMER")
    pub encounter_class: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NurseAssessOutput {
    /// Narrative nursing assessment text
    pub nursing_assessment: String,
    /// Morse Fall Scale score (0–24; ≥45 is high risk)
    pub fall_risk_score: u8,
    /// Numerical Rating Scale pain score (0–10)
    pub pain_score: u8,
    /// Braden Scale for skin integrity / pressure ulcer risk (6–23; ≤18 is at-risk)
    pub braden_score: u8,
    /// FHIR Observation encoding the assessment narrative (LOINC 75275-8)
    pub observation: Observation,
    pub confidence: Confidence,
    pub audit_event: AuditEvent,
    pub policy_decision: PolicyDecision,
    /// SHA-256 hash of the matched skill spec (if any)
    pub spec_hash: Option<String>,
}

pub struct NurseAssessAgent {
    llm: Arc<dyn LlmCapability>,
}

impl NurseAssessAgent {
    pub fn new(llm: Arc<dyn LlmCapability>) -> Self {
        Self { llm }
    }

    /// Run the nursing assessment workflow.
    /// 1. Skill-aware policy check  2. Build prompt  3. Call Claude
    /// 4. Parse response  5. Build Observation  6. Verify output  7. Build audit event
    ///
    /// Note: the agent *builds* the audit event but does not persist it.
    /// Persistence is the API layer's responsibility (VERITAS separation).
    pub async fn evaluate(
        &self,
        input: &NurseAssessInput,
        policy_engine: &PolicyEngine,
    ) -> Result<NurseAssessOutput, AgentError> {
        // Step 1: Build context and run skill-aware policy evaluation
        let context = self.build_context(input);
        let skill_eval = policy_engine.evaluate_with_skill(&context)?;

        match &skill_eval.decision {
            PolicyDecision::Deny => {
                tracing::warn!(
                    actor_id = %input.practitioner_id,
                    encounter_id = %input.encounter_id,
                    "policy denied nursing assessment"
                );
                return Err(AgentError::PolicyDenied(format!(
                    "nursing assessment denied for encounter {}",
                    input.encounter_id
                )));
            }
            PolicyDecision::RequireApproval => {
                return Err(AgentError::RequiresApproval {
                    action: "nurse_assess.evaluate".to_string(),
                });
            }
            PolicyDecision::Allow => {
                tracing::info!(
                    actor_id = %input.practitioner_id,
                    encounter_id = %input.encounter_id,
                    spec_hash = ?skill_eval.spec_hash,
                    "policy allowed nursing assessment"
                );
            }
        }

        // Step 2: Build de-identified prompt
        let prompt = self.build_prompt(input);

        // Step 3: Call LLM
        let response_text = self.llm.call(&prompt).await?;

        // Step 4: Parse response
        let parsed: serde_json::Value = serde_json::from_str(&response_text).map_err(|e| {
            AgentError::ClaudeApi(format!("failed to parse nursing assessment response: {e}"))
        })?;

        let nursing_assessment = parsed
            .get("assessment")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let fall_risk_score = parsed
            .get("fall_risk")
            .and_then(|v| v.as_u64())
            .map(|v| v.min(24) as u8) // Morse scale max is 24 (simplified)
            .unwrap_or(0);

        let pain_score = parsed
            .get("pain_score")
            .and_then(|v| v.as_u64())
            .map(|v| v.min(10) as u8)
            .unwrap_or(0);

        let braden_score = parsed
            .get("braden_score")
            .and_then(|v| v.as_u64())
            // Braden is 6-23; clamp to valid range
            .map(|v| (v.clamp(6, 23)) as u8)
            .unwrap_or(23); // Default to lowest-risk score

        // Step 5: Build FHIR Observation
        let observation = self.build_observation(input, &nursing_assessment);

        // Step 6: Verify output
        self.verify_output(fall_risk_score, pain_score, braden_score, &observation)?;

        // Step 7: Create audit event with skill metadata
        let input_descriptor = serde_json::to_vec(&serde_json::json!({
            "encounter_id": input.encounter_id,
            "practitioner_id": input.practitioner_id,
            "assessment_type": input.assessment_type,
            "skill_id": skill_eval.skill_id,
            "spec_hash": skill_eval.spec_hash,
        }))?;
        let output_descriptor = serde_json::to_vec(&observation)?;

        let audit_event = AuditEvent::new(
            &input.practitioner_id,
            Some(input.patient_id.clone()),
            "nurse_assess.evaluate",
            &skill_eval.decision.to_string(),
            sha256_hash(&input_descriptor),
            sha256_hash(&output_descriptor),
            "", // previous_hash assigned atomically by SqliteAuditStore::append
        );

        tracing::info!(
            audit_event_id = %audit_event.id,
            encounter_id = %input.encounter_id,
            fall_risk_score = fall_risk_score,
            pain_score = pain_score,
            braden_score = braden_score,
            "nursing assessment completed and verified"
        );

        let confidence =
            self.compute_confidence(fall_risk_score, pain_score, braden_score, &nursing_assessment);

        Ok(NurseAssessOutput {
            nursing_assessment,
            fall_risk_score,
            pain_score,
            braden_score,
            observation,
            confidence,
            audit_event,
            policy_decision: skill_eval.decision,
            spec_hash: skill_eval.spec_hash,
        })
    }

    fn build_context(&self, input: &NurseAssessInput) -> ActionContext {
        let mut props = HashMap::new();
        // Backward-compatible key (for existing rule conditions)
        props.insert("encounter_status".to_string(), input.encounter_status.clone());
        // New dotted convention (for skill population criteria)
        props.insert("encounter.status".to_string(), input.encounter_status.clone());
        props.insert("patient.active".to_string(), input.patient_active.to_string());
        props.insert("assessment_type".to_string(), input.assessment_type.clone());
        if let Some(deceased) = input.patient_deceased {
            props.insert("patient.deceased".to_string(), deceased.to_string());
        }
        if let Some(ref enc_class) = input.encounter_class {
            props.insert("encounter.class".to_string(), enc_class.clone());
        }

        ActionContext {
            action: "nurse_assess.evaluate".to_string(),
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

    fn build_prompt(&self, input: &NurseAssessInput) -> PromptEnvelope {
        let system = "You are a clinical nursing assessment assistant. Based on the provided \
             vitals and assessment type, generate a structured nursing assessment including \
             standardized risk scores. Output ONLY valid JSON matching this schema:\n\
             {\n\
               \"assessment\": \"narrative nursing assessment paragraph\",\n\
               \"fall_risk\": 0,\n\
               \"pain_score\": 0,\n\
               \"braden_score\": 23\n\
             }\n\n\
             Score ranges:\n\
             - fall_risk: Morse Fall Scale 0-24 (≥14 is high risk in simplified scale)\n\
             - pain_score: Numerical Rating Scale 0-10 (0=none, 10=worst imaginable)\n\
             - braden_score: Braden Scale 6-23 (≤18 at-risk for pressure injury)\n\n\
             Rules:\n\
             - Base scores solely on information provided in the vitals text\n\
             - When unable to determine a score from available data, default to lowest risk\n\
             - The assessment narrative should reflect clinical observations, not conclusions\n\
             - Do not fabricate findings not present in the provided data\n\
             - Output raw JSON only, no markdown fences"
            .to_string();

        let parts = vec![
            format!("Assessment type: {}", input.assessment_type),
            format!("Vitals and status: {}", input.vitals_text),
        ];

        PromptEnvelope::build(system, parts.join("\n\n"))
    }

    fn build_observation(&self, input: &NurseAssessInput, assessment_text: &str) -> Observation {
        let mut obs = Observation::new(
            "final",
            CodeableConcept {
                coding: Some(vec![Coding {
                    system: Some("http://loinc.org".to_string()),
                    // LOINC 75275-8: Nursing assessment note
                    code: Some("75275-8".to_string()),
                    display: Some("Nursing assessment".to_string()),
                }]),
                text: Some("Nursing assessment".to_string()),
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
        obs.value_string = Some(assessment_text.to_string());
        obs
    }

    fn compute_confidence(
        &self,
        fall_risk_score: u8,
        pain_score: u8,
        braden_score: u8,
        assessment_text: &str,
    ) -> Confidence {
        let mut score = 0.5;
        let mut factors = Vec::new();

        // Scores within valid ranges
        let fall_valid = fall_risk_score <= 24;
        let pain_valid = pain_score <= 10;
        let braden_valid = (6..=23).contains(&braden_score);

        if fall_valid {
            factors.push("fall_risk_in_range".to_string());
            score += 0.1;
        }
        if pain_valid {
            factors.push("pain_score_in_range".to_string());
            score += 0.1;
        }
        if braden_valid {
            factors.push("braden_score_in_range".to_string());
            score += 0.1;
        }

        // All three scores valid → full assessment completed
        if fall_valid && pain_valid && braden_valid {
            factors.push("all_scores_valid".to_string());
            score += 0.1;
        }

        // Assessment narrative substance
        if assessment_text.len() > 100 {
            factors.push("substantive_narrative".to_string());
            score += 0.1;
        }

        Confidence::new(score, factors)
    }

    fn verify_output(
        &self,
        fall_risk_score: u8,
        pain_score: u8,
        braden_score: u8,
        observation: &Observation,
    ) -> Result<(), AgentError> {
        // Morse Fall Scale (simplified): 0-24
        if fall_risk_score > 24 {
            return Err(AgentError::VerificationFailed(format!(
                "fall_risk_score {fall_risk_score} exceeds maximum of 24"
            )));
        }

        // Numerical Rating Scale: 0-10
        if pain_score > 10 {
            return Err(AgentError::VerificationFailed(format!(
                "pain_score {pain_score} exceeds maximum of 10"
            )));
        }

        // Braden Scale: 6-23
        if !(6..=23).contains(&braden_score) {
            return Err(AgentError::VerificationFailed(format!(
                "braden_score {braden_score} is outside valid range 6-23"
            )));
        }

        // Observation must have a subject reference
        if observation.subject.is_none() {
            return Err(AgentError::VerificationFailed(
                "Observation must include a subject reference".to_string(),
            ));
        }

        // Observation must carry the assessment text
        if observation.value_string.as_ref().map_or(true, |s| s.is_empty()) {
            return Err(AgentError::VerificationFailed(
                "Observation must include a non-empty valueString for the assessment".to_string(),
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
        let agent = NurseAssessAgent::new(Arc::new(crate::MockClaudeCapability::new()));
        let input = NurseAssessInput {
            encounter_id: "enc-001".to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: "patient-001".to_string(),
            practitioner_id: "nurse-001".to_string(),
            assessment_type: "admission".to_string(),
            vitals_text: "HR 88, BP 136/82, SpO2 97%, Temp 37.0C, Pain 4/10".to_string(),
            capabilities: vec!["nurse_assess".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("nurse".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("IMP".to_string()),
        };

        let obs = agent.build_observation(&input, "Patient alert and oriented x3. Vital signs stable.");
        assert_eq!(obs.status, "final");
        assert!(obs.subject.is_some());
        assert!(obs.encounter.is_some());
        assert!(obs.value_string.is_some());
        let code = obs.code.coding.as_ref().unwrap();
        assert_eq!(code[0].code, Some("75275-8".to_string()));
    }

    #[test]
    fn test_verify_rejects_out_of_range_scores() {
        let agent = NurseAssessAgent::new(Arc::new(crate::MockClaudeCapability::new()));
        let mut obs = Observation::new("final", CodeableConcept { coding: None, text: None });
        obs.subject = Some(Reference {
            reference: Some("Patient/p-001".to_string()),
            display: None,
            type_: None,
        });
        obs.value_string = Some("Assessment text".to_string());

        // pain_score > 10 is invalid
        assert!(agent.verify_output(0, 11, 18, &obs).is_err());
        // braden_score < 6 is invalid
        assert!(agent.verify_output(0, 5, 3, &obs).is_err());
        // All valid
        assert!(agent.verify_output(10, 5, 18, &obs).is_ok());
    }

    #[test]
    fn test_verify_requires_subject_and_value() {
        let agent = NurseAssessAgent::new(Arc::new(crate::MockClaudeCapability::new()));

        // No subject, no value
        let obs = Observation::new("final", CodeableConcept { coding: None, text: None });
        assert!(agent.verify_output(0, 0, 23, &obs).is_err());
    }
}
