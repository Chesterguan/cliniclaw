use std::collections::HashMap;
use std::sync::Arc;

use cliniclaw_persist::{sha256_hash, AuditEvent};
use cliniclaw_policy::{ActionContext, Capability, PolicyDecision, PolicyEngine};

use cliniclaw_kernel::Confidence;

use crate::error::AgentError;
use crate::llm::LlmCapability;
use crate::PromptEnvelope;

#[derive(Debug, Clone)]
pub struct PriorAuthInput {
    pub encounter_id: String,
    pub encounter_status: String,
    pub patient_id: String,
    pub practitioner_id: String,
    /// The ServiceRequest ID that needs prior authorization
    pub service_request_id: String,
    /// Description of the procedure/service
    pub service_description: String,
    /// ICD-10 diagnosis codes supporting the request
    pub diagnosis_codes: Vec<String>,
    /// CPT codes for the requested service
    pub cpt_codes: Vec<String>,
    /// Clinical notes/justification from the provider
    pub clinical_notes: Option<String>,
    pub capabilities: Vec<String>,
    pub capability_tokens: Vec<Capability>,
    pub practitioner_role: Option<String>,
    pub patient_active: bool,
    pub patient_deceased: Option<bool>,
    pub encounter_class: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PriorAuthOutput {
    pub diagnosis_summary: String,
    pub clinical_justification: String,
    pub supporting_evidence: Vec<String>,
    pub urgency: String,
    pub cpt_codes: Vec<String>,
    pub icd10_codes: Vec<String>,
    pub status: PriorAuthStatus,
    pub confidence: Confidence,
    pub audit_event: AuditEvent,
    pub policy_decision: PolicyDecision,
    pub spec_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PriorAuthStatus {
    /// Package assembled, pending physician sign-off
    PendingApproval,
    /// Physician approved, ready to submit to payer
    Approved,
    /// Submitted to payer, awaiting response
    Submitted,
    /// Payer approved
    Authorized,
    /// Payer denied
    Denied,
}

impl std::fmt::Display for PriorAuthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PendingApproval => write!(f, "pending_approval"),
            Self::Approved => write!(f, "approved"),
            Self::Submitted => write!(f, "submitted"),
            Self::Authorized => write!(f, "authorized"),
            Self::Denied => write!(f, "denied"),
        }
    }
}

pub struct PriorAuthAgent {
    llm: Arc<dyn LlmCapability>,
}

impl PriorAuthAgent {
    pub fn new(llm: Arc<dyn LlmCapability>) -> Self {
        Self { llm }
    }

    /// Assemble a prior authorization package using AI to generate
    /// clinical justification from chart data.
    pub async fn assemble_package(
        &self,
        input: &PriorAuthInput,
        policy_engine: &PolicyEngine,
    ) -> Result<PriorAuthOutput, AgentError> {
        // Step 1: Policy check — prior auth always requires approval
        let context = self.build_context(input);
        let skill_eval = policy_engine.evaluate_with_skill(&context)?;

        match &skill_eval.decision {
            PolicyDecision::Deny => {
                tracing::warn!(
                    actor_id = %input.practitioner_id,
                    service_request_id = %input.service_request_id,
                    "policy denied prior auth assembly"
                );
                return Err(AgentError::PolicyDenied(format!(
                    "prior auth denied for service request {}",
                    input.service_request_id
                )));
            }
            PolicyDecision::Allow | PolicyDecision::RequireApproval => {
                tracing::info!(
                    actor_id = %input.practitioner_id,
                    service_request_id = %input.service_request_id,
                    decision = %skill_eval.decision,
                    "policy evaluated prior auth"
                );
            }
        }

        // Step 2: Build prompt and call LLM for clinical justification
        let prompt = self.build_prompt(input);
        let response_text = self.llm.call(&prompt).await?;

        // Step 3: Parse response
        let parsed: serde_json::Value = serde_json::from_str(&response_text).map_err(|e| {
            AgentError::ClaudeApi(format!("failed to parse prior auth response: {e}"))
        })?;

        let diagnosis_summary = parsed
            .get("diagnosis_summary")
            .and_then(|v| v.as_str())
            .unwrap_or("See clinical justification")
            .to_string();

        let clinical_justification = parsed
            .get("clinical_justification")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let supporting_evidence: Vec<String> = parsed
            .get("supporting_evidence")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();

        let urgency = parsed
            .get("urgency")
            .and_then(|v| v.as_str())
            .unwrap_or("routine")
            .to_string();

        // Step 4: Verify we have substantive content
        if clinical_justification.len() < 50 {
            return Err(AgentError::VerificationFailed(
                "clinical justification is too short for a valid prior auth submission"
                    .to_string(),
            ));
        }

        // Step 5: Audit event
        let input_descriptor = serde_json::to_vec(&serde_json::json!({
            "encounter_id": input.encounter_id,
            "practitioner_id": input.practitioner_id,
            "service_request_id": input.service_request_id,
            "cpt_codes": input.cpt_codes,
            "diagnosis_codes": input.diagnosis_codes,
            "skill_id": skill_eval.skill_id,
            "spec_hash": skill_eval.spec_hash,
        }))?;
        let output_descriptor = serde_json::to_vec(&serde_json::json!({
            "diagnosis_summary": diagnosis_summary,
            "urgency": urgency,
            "cpt_codes": input.cpt_codes,
            "icd10_codes": input.diagnosis_codes,
        }))?;

        let audit_event = AuditEvent::new(
            &input.practitioner_id,
            Some(input.patient_id.clone()),
            "prior_auth.assemble",
            &skill_eval.decision.to_string(),
            sha256_hash(&input_descriptor),
            sha256_hash(&output_descriptor),
            "",
        );

        tracing::info!(
            audit_event_id = %audit_event.id,
            service_request_id = %input.service_request_id,
            "prior auth package assembled"
        );

        // Prior auth always starts as pending approval (physician must sign off)
        let status = if skill_eval.decision == PolicyDecision::RequireApproval {
            PriorAuthStatus::PendingApproval
        } else {
            PriorAuthStatus::PendingApproval // Even if policy allows, PA needs human sign-off
        };

        // Compute confidence based on justification quality
        let confidence = self.compute_confidence(&clinical_justification, &supporting_evidence, &input.diagnosis_codes);

        Ok(PriorAuthOutput {
            diagnosis_summary,
            clinical_justification,
            supporting_evidence,
            urgency,
            cpt_codes: input.cpt_codes.clone(),
            icd10_codes: input.diagnosis_codes.clone(),
            status,
            confidence,
            audit_event,
            policy_decision: skill_eval.decision,
            spec_hash: skill_eval.spec_hash,
        })
    }

    fn compute_confidence(
        &self,
        justification: &str,
        evidence: &[String],
        diagnosis_codes: &[String],
    ) -> Confidence {
        let mut score = 0.5;
        let mut factors = Vec::new();

        // Justification length and quality
        if justification.len() > 200 {
            factors.push("detailed_justification".to_string());
            score += 0.15;
        } else if justification.len() > 100 {
            factors.push("adequate_justification".to_string());
            score += 0.1;
        }

        // Supporting evidence
        if evidence.len() >= 3 {
            factors.push("strong_evidence".to_string());
            score += 0.15;
        } else if !evidence.is_empty() {
            factors.push("has_evidence".to_string());
            score += 0.1;
        }

        // Diagnosis code specificity
        if !diagnosis_codes.is_empty() {
            factors.push("has_diagnosis_codes".to_string());
            score += 0.1;
        }

        Confidence::new(score, factors)
    }

    fn build_context(&self, input: &PriorAuthInput) -> ActionContext {
        let mut props = HashMap::new();
        props.insert("encounter_status".to_string(), input.encounter_status.clone());
        props.insert("encounter.status".to_string(), input.encounter_status.clone());
        props.insert("patient.active".to_string(), input.patient_active.to_string());
        if let Some(deceased) = input.patient_deceased {
            props.insert("patient.deceased".to_string(), deceased.to_string());
        }
        if let Some(ref enc_class) = input.encounter_class {
            props.insert("encounter.class".to_string(), enc_class.clone());
        }

        ActionContext {
            action: "prior_auth.assemble".to_string(),
            actor_id: input.practitioner_id.clone(),
            capabilities: input.capabilities.clone(),
            resource_type: Some("ServiceRequest".to_string()),
            properties: props,
            capability_tokens: input.capability_tokens.clone(),
            role: input.practitioner_role.clone(),
            patient_id: Some(input.patient_id.clone()),
            encounter_id: Some(input.encounter_id.clone()),
        }
    }

    fn build_prompt(&self, input: &PriorAuthInput) -> PromptEnvelope {
        let system = "You are a prior authorization assistant. Generate a clinical justification \
             package for the requested service. Output ONLY valid JSON matching this schema:\n\
             {\n\
               \"diagnosis_summary\": \"brief summary of diagnosis\",\n\
               \"clinical_justification\": \"detailed clinical justification paragraph\",\n\
               \"supporting_evidence\": [\"evidence item 1\", \"evidence item 2\"],\n\
               \"urgency\": \"routine|urgent|emergent\",\n\
               \"cpt_codes\": [\"code1\"],\n\
               \"icd10_codes\": [\"code1\", \"code2\"]\n\
             }\n\n\
             Rules:\n\
             - Justify the medical necessity based on the clinical data provided\n\
             - Reference specific failed conservative treatments if applicable\n\
             - Include relevant imaging/test results in supporting evidence\n\
             - Be thorough but factual — do not fabricate clinical data\n\
             - Output raw JSON only, no markdown fences"
            .to_string();

        let mut parts = vec![
            format!("Service: {}", input.service_description),
            format!("CPT codes: {}", input.cpt_codes.join(", ")),
            format!("Diagnosis codes: {}", input.diagnosis_codes.join(", ")),
        ];

        if let Some(ref notes) = input.clinical_notes {
            parts.push(format!("Clinical notes: {}", notes));
        }

        PromptEnvelope::build(system, parts.join("\n\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prior_auth_status_display() {
        assert_eq!(PriorAuthStatus::PendingApproval.to_string(), "pending_approval");
        assert_eq!(PriorAuthStatus::Authorized.to_string(), "authorized");
        assert_eq!(PriorAuthStatus::Denied.to_string(), "denied");
    }

    #[test]
    fn test_prior_auth_status_serde() {
        let json = serde_json::to_string(&PriorAuthStatus::PendingApproval).unwrap();
        assert_eq!(json, "\"pending_approval\"");
        let deserialized: PriorAuthStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, PriorAuthStatus::PendingApproval);
    }
}
