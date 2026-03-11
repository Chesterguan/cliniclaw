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
pub struct DischargePlanInput {
    pub encounter_id: String,
    pub encounter_status: String,
    pub patient_id: String,
    pub practitioner_id: String,
    /// Active diagnoses at time of discharge
    pub active_conditions: Vec<String>,
    /// Current medications the patient will continue post-discharge
    pub current_medications: Vec<String>,
    /// Brief clinical summary of the encounter (e.g. from ambient note or provider)
    pub assessment_summary: String,
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
    /// Encounter class code (e.g. "IMP", "EMER")
    pub encounter_class: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DischargePlanOutput {
    /// Patient-facing discharge instructions narrative
    pub discharge_instructions: String,
    /// Follow-up schedule (e.g. "PCP in 7 days", "Cardiology in 2 weeks")
    pub follow_up_schedule: Vec<String>,
    /// Medication reconciliation notes (additions, changes, or discontinuations)
    pub medication_reconciliation: Vec<String>,
    /// FHIR DiagnosticReport carrying the discharge summary (LOINC 18842-5)
    pub report: DiagnosticReport,
    pub confidence: Confidence,
    pub audit_event: AuditEvent,
    pub policy_decision: PolicyDecision,
    /// SHA-256 hash of the matched skill spec (if any)
    pub spec_hash: Option<String>,
}

pub struct DischargePlanAgent {
    llm: Arc<dyn LlmCapability>,
}

impl DischargePlanAgent {
    pub fn new(llm: Arc<dyn LlmCapability>) -> Self {
        Self { llm }
    }

    /// Run the discharge planning workflow.
    /// 1. Skill-aware policy check  2. Build prompt  3. Call Claude
    /// 4. Parse response  5. Build DiagnosticReport  6. Verify output  7. Build audit event
    ///
    /// Note: the agent *builds* the audit event but does not persist it.
    /// Persistence is the API layer's responsibility (VERITAS separation).
    pub async fn generate(
        &self,
        input: &DischargePlanInput,
        policy_engine: &PolicyEngine,
    ) -> Result<DischargePlanOutput, AgentError> {
        // Step 1: Build context and run skill-aware policy evaluation
        let context = self.build_context(input);
        let skill_eval = policy_engine.evaluate_with_skill(&context)?;

        match &skill_eval.decision {
            PolicyDecision::Deny => {
                tracing::warn!(
                    actor_id = %input.practitioner_id,
                    encounter_id = %input.encounter_id,
                    "policy denied discharge plan generation"
                );
                return Err(AgentError::PolicyDenied(format!(
                    "discharge plan generation denied for encounter {}",
                    input.encounter_id
                )));
            }
            PolicyDecision::RequireApproval => {
                return Err(AgentError::RequiresApproval {
                    action: "discharge_plan.generate".to_string(),
                });
            }
            PolicyDecision::Allow => {
                tracing::info!(
                    actor_id = %input.practitioner_id,
                    encounter_id = %input.encounter_id,
                    spec_hash = ?skill_eval.spec_hash,
                    "policy allowed discharge plan generation"
                );
            }
        }

        // Step 2: Build de-identified prompt
        let prompt = self.build_prompt(input);

        // Step 3: Call LLM
        let response_text = self.llm.call(&prompt).await?;

        // Step 4: Parse response
        let parsed: serde_json::Value = serde_json::from_str(&response_text).map_err(|e| {
            AgentError::ClaudeApi(format!("failed to parse discharge plan response: {e}"))
        })?;

        let discharge_instructions = parsed
            .get("instructions")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let follow_up_schedule: Vec<String> = parsed
            .get("follow_up")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();

        let medication_reconciliation: Vec<String> = parsed
            .get("med_reconciliation")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();

        // Step 5: Build FHIR DiagnosticReport (discharge summary document)
        let report = self.build_report(input, &discharge_instructions);

        // Step 6: Verify output
        self.verify_output(&report)?;

        // Step 7: Create audit event with skill metadata
        let input_descriptor = serde_json::to_vec(&serde_json::json!({
            "encounter_id": input.encounter_id,
            "practitioner_id": input.practitioner_id,
            "condition_count": input.active_conditions.len(),
            "medication_count": input.current_medications.len(),
            "assessment_summary_len": input.assessment_summary.len(),
            "skill_id": skill_eval.skill_id,
            "spec_hash": skill_eval.spec_hash,
        }))?;
        let output_descriptor = serde_json::to_vec(&report)?;

        let audit_event = AuditEvent::new(
            &input.practitioner_id,
            Some(input.patient_id.clone()),
            "discharge_plan.generate",
            &skill_eval.decision.to_string(),
            sha256_hash(&input_descriptor),
            sha256_hash(&output_descriptor),
            "", // previous_hash assigned atomically by SqliteAuditStore::append
        );

        tracing::info!(
            audit_event_id = %audit_event.id,
            encounter_id = %input.encounter_id,
            follow_up_count = follow_up_schedule.len(),
            med_reconciliation_count = medication_reconciliation.len(),
            "discharge plan generated and verified"
        );

        let confidence = self.compute_confidence(input, &discharge_instructions, &follow_up_schedule);

        Ok(DischargePlanOutput {
            discharge_instructions,
            follow_up_schedule,
            medication_reconciliation,
            report,
            confidence,
            audit_event,
            policy_decision: skill_eval.decision,
            spec_hash: skill_eval.spec_hash,
        })
    }

    fn build_context(&self, input: &DischargePlanInput) -> ActionContext {
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
            action: "discharge_plan.generate".to_string(),
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

    fn build_prompt(&self, input: &DischargePlanInput) -> PromptEnvelope {
        let system = "You are a clinical discharge planning assistant. Generate a structured \
             discharge plan based on the patient's encounter summary, active conditions, and \
             medications. Output ONLY valid JSON matching this schema:\n\
             {\n\
               \"instructions\": \"patient-facing narrative discharge instructions\",\n\
               \"follow_up\": [\"PCP in 7 days\", \"Cardiology in 2 weeks\"],\n\
               \"med_reconciliation\": [\"Continue metformin 500mg BID\", \"Start lisinopril 5mg daily\"]\n\
             }\n\n\
             Rules:\n\
             - Write instructions in clear patient-appropriate language\n\
             - Base follow-up schedule on the diagnoses and severity indicated\n\
             - Reconcile medications based on the active medication list provided\n\
             - Do not fabricate diagnoses, medications, or referral specialties not mentioned\n\
             - Output raw JSON only, no markdown fences"
            .to_string();

        let mut parts = vec![format!(
            "Assessment summary: {}",
            input.assessment_summary
        )];

        if !input.active_conditions.is_empty() {
            parts.push(format!(
                "Active conditions: {}",
                input.active_conditions.join(", ")
            ));
        }

        if !input.current_medications.is_empty() {
            parts.push(format!(
                "Current medications: {}",
                input.current_medications.join(", ")
            ));
        }

        PromptEnvelope::build(system, parts.join("\n\n"))
    }

    fn build_report(&self, input: &DischargePlanInput, instructions: &str) -> DiagnosticReport {
        DiagnosticReport {
            id: None,
            resource_type: "DiagnosticReport".to_string(),
            status: "preliminary".to_string(),
            code: CodeableConcept {
                coding: Some(vec![Coding {
                    system: Some("http://loinc.org".to_string()),
                    // LOINC 18842-5: Discharge summary
                    code: Some("18842-5".to_string()),
                    display: Some("Discharge summary".to_string()),
                }]),
                text: Some("AI-generated discharge summary".to_string()),
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
                    base64::engine::general_purpose::STANDARD.encode(instructions),
                ),
                url: None,
                title: Some("AI-generated discharge instructions".to_string()),
            }]),
        }
    }

    fn compute_confidence(
        &self,
        input: &DischargePlanInput,
        instructions: &str,
        follow_up_schedule: &[String],
    ) -> Confidence {
        let mut score = 0.5;
        let mut factors = Vec::new();

        // Assessment summary provided
        if !input.assessment_summary.is_empty() {
            factors.push("assessment_summary_present".to_string());
            score += 0.1;
        }

        // Active conditions context provided
        if !input.active_conditions.is_empty() {
            factors.push("conditions_context_present".to_string());
            score += 0.05;
        }

        // Medications provided for reconciliation
        if !input.current_medications.is_empty() {
            factors.push("medications_provided".to_string());
            score += 0.05;
        }

        // Substantive instructions generated
        if instructions.len() > 200 {
            factors.push("detailed_instructions".to_string());
            score += 0.15;
        } else if instructions.len() > 50 {
            factors.push("adequate_instructions".to_string());
            score += 0.1;
        }

        // Follow-up plan is essential for safe discharge
        if !follow_up_schedule.is_empty() {
            factors.push("follow_up_scheduled".to_string());
            score += 0.1;
        } else {
            // Missing follow-up plan reduces confidence in completeness
            factors.push("no_follow_up_plan".to_string());
            score -= 0.1;
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

        // Verify the discharge instructions data is valid base64 and non-trivial
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
                    "discharge instructions are too short to be a valid clinical document"
                        .to_string(),
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
        let agent = DischargePlanAgent::new(Arc::new(crate::MockClaudeCapability::new()));
        let input = DischargePlanInput {
            encounter_id: "enc-001".to_string(),
            encounter_status: "finished".to_string(),
            patient_id: "patient-001".to_string(),
            practitioner_id: "physician-001".to_string(),
            active_conditions: vec!["Type 2 diabetes mellitus".to_string(), "Hypertension".to_string()],
            current_medications: vec!["Metformin 500mg BID".to_string(), "Lisinopril 5mg daily".to_string()],
            assessment_summary: "Patient admitted for hyperglycemia, now stable.".to_string(),
            capabilities: vec!["discharge_plan".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("physician".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("IMP".to_string()),
        };

        let instructions = "Please take your medications as directed. Monitor blood sugar twice daily.";
        let report = agent.build_report(&input, instructions);
        assert_eq!(report.resource_type, "DiagnosticReport");
        assert_eq!(report.status, "preliminary");
        assert!(report.subject.is_some());
        assert!(report.encounter.is_some());
        assert!(report.presented_form.is_some());
        let code = report.code.coding.as_ref().unwrap();
        assert_eq!(code[0].code, Some("18842-5".to_string()));
    }

    #[test]
    fn test_verify_rejects_trivially_short_instructions() {
        let agent = DischargePlanAgent::new(Arc::new(crate::MockClaudeCapability::new()));
        let input = DischargePlanInput {
            encounter_id: "enc-001".to_string(),
            encounter_status: "finished".to_string(),
            patient_id: "patient-001".to_string(),
            practitioner_id: "physician-001".to_string(),
            active_conditions: vec![],
            current_medications: vec![],
            assessment_summary: "Stable.".to_string(),
            capabilities: vec!["discharge_plan".to_string()],
            capability_tokens: vec![],
            practitioner_role: None,
            patient_active: true,
            patient_deceased: None,
            encounter_class: None,
        };

        // Instructions under 20 bytes when decoded should fail
        let report = agent.build_report(&input, "Too short");
        // "Too short" is 9 bytes — < 20 → verification must fail
        assert!(agent.verify_output(&report).is_err());
    }
}
