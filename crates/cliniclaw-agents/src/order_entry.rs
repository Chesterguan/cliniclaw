use std::collections::HashMap;
use std::sync::Arc;

use cliniclaw_fhir::{CodeableConcept, Coding, DosageInstruction, MedicationRequest, Reference};
use cliniclaw_persist::{sha256_hash, AuditEvent};
use cliniclaw_policy::{ActionContext, Capability, PolicyDecision, PolicyEngine};

use cliniclaw_kernel::Confidence;

use crate::cds::{self, CdsCard};
use crate::error::AgentError;
use crate::llm::LlmCapability;
use crate::PromptEnvelope;

#[derive(Debug, Clone)]
pub struct OrderEntryInput {
    pub encounter_id: String,
    pub encounter_status: String,
    pub patient_id: String,
    pub practitioner_id: String,
    /// Natural language order text (e.g. "start metformin 500mg BID")
    pub order_text: String,
    /// Current active medications for interaction checking
    pub active_medications: Vec<String>,
    pub capabilities: Vec<String>,
    pub capability_tokens: Vec<Capability>,
    pub practitioner_role: Option<String>,
    pub patient_active: bool,
    pub patient_deceased: Option<bool>,
    pub encounter_class: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OrderEntryOutput {
    pub medication_request: MedicationRequest,
    pub cds_cards: Vec<CdsCard>,
    pub confidence: Confidence,
    pub audit_event: AuditEvent,
    pub policy_decision: PolicyDecision,
    pub spec_hash: Option<String>,
}

pub struct OrderEntryAgent {
    llm: Arc<dyn LlmCapability>,
}

impl OrderEntryAgent {
    pub fn new(llm: Arc<dyn LlmCapability>) -> Self {
        Self { llm }
    }

    /// Parse a natural language order and produce a FHIR MedicationRequest + CDS cards.
    pub async fn propose_order(
        &self,
        input: &OrderEntryInput,
        policy_engine: &PolicyEngine,
    ) -> Result<OrderEntryOutput, AgentError> {
        // Step 1: Policy check
        let context = self.build_context(input);
        let skill_eval = policy_engine.evaluate_with_skill(&context)?;

        match &skill_eval.decision {
            PolicyDecision::Deny => {
                tracing::warn!(
                    actor_id = %input.practitioner_id,
                    encounter_id = %input.encounter_id,
                    "policy denied order entry"
                );
                return Err(AgentError::PolicyDenied(format!(
                    "order entry denied for encounter {}",
                    input.encounter_id
                )));
            }
            PolicyDecision::RequireApproval => {
                return Err(AgentError::RequiresApproval {
                    action: "order_entry.propose".to_string(),
                });
            }
            PolicyDecision::Allow => {
                tracing::info!(
                    actor_id = %input.practitioner_id,
                    encounter_id = %input.encounter_id,
                    "policy allowed order entry"
                );
            }
        }

        // Step 2: Parse order via LLM
        let prompt = self.build_prompt(input);
        let response_text = self.llm.call(&prompt).await?;

        // Step 3: Build MedicationRequest from LLM response
        let parsed: serde_json::Value = serde_json::from_str(&response_text)
            .map_err(|e| AgentError::ClaudeApi(format!("failed to parse order response: {e}")))?;

        let medication_request = self.build_medication_request(input, &parsed);

        // Step 4: Run CDS checks
        let medication_name = parsed
            .get("medication")
            .and_then(|v| v.as_str())
            .unwrap_or(&input.order_text);
        let mut cds_cards = Vec::new();

        // Drug interaction checks
        cds_cards.extend(cds::check_drug_interactions(
            medication_name,
            &input.active_medications,
        ));

        // High-risk medication check
        if let Some(card) = cds::check_high_risk(medication_name) {
            cds_cards.push(card);
        }

        // Duplicate order check
        if let Some(card) = cds::check_duplicate(medication_name, &input.active_medications) {
            cds_cards.push(card);
        }

        // Step 5: Verify output
        self.verify_output(&medication_request)?;

        // Step 6: Audit event
        let input_descriptor = serde_json::to_vec(&serde_json::json!({
            "encounter_id": input.encounter_id,
            "practitioner_id": input.practitioner_id,
            "order_text": input.order_text,
            "skill_id": skill_eval.skill_id,
            "spec_hash": skill_eval.spec_hash,
            "cds_card_count": cds_cards.len(),
        }))?;
        let output_descriptor = serde_json::to_vec(&medication_request)?;

        let audit_event = AuditEvent::new(
            &input.practitioner_id,
            Some(input.patient_id.clone()),
            "order_entry.propose",
            &skill_eval.decision.to_string(),
            sha256_hash(&input_descriptor),
            sha256_hash(&output_descriptor),
            "",
        );

        tracing::info!(
            audit_event_id = %audit_event.id,
            encounter_id = %input.encounter_id,
            cds_cards = cds_cards.len(),
            "order entry completed"
        );

        // Compute confidence based on parsing quality and CDS severity
        let confidence = self.compute_confidence(&parsed, &cds_cards);

        Ok(OrderEntryOutput {
            medication_request,
            cds_cards,
            confidence,
            audit_event,
            policy_decision: skill_eval.decision,
            spec_hash: skill_eval.spec_hash,
        })
    }

    fn build_context(&self, input: &OrderEntryInput) -> ActionContext {
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
            action: "order_entry.propose".to_string(),
            actor_id: input.practitioner_id.clone(),
            capabilities: input.capabilities.clone(),
            resource_type: Some("MedicationRequest".to_string()),
            properties: props,
            capability_tokens: input.capability_tokens.clone(),
            role: input.practitioner_role.clone(),
            patient_id: Some(input.patient_id.clone()),
            encounter_id: Some(input.encounter_id.clone()),
        }
    }

    fn build_prompt(&self, input: &OrderEntryInput) -> PromptEnvelope {
        let system = "You are a clinical order entry assistant. Parse the natural language medication \
             order into structured data. Output ONLY valid JSON matching this schema:\n\
             {\n\
               \"medication\": \"drug name\",\n\
               \"dose\": \"dose with unit\",\n\
               \"route\": \"oral|iv|inhalation|topical|subcutaneous\",\n\
               \"frequency\": \"daily|BID|TID|QID|Q4H|Q6H|PRN|once\",\n\
               \"indication\": \"clinical indication\",\n\
               \"icd10\": \"ICD-10 code if clear\",\n\
               \"rxnorm\": \"RxNorm code if known\",\n\
               \"instructions\": \"additional instructions\"\n\
             }\n\n\
             Rules:\n\
             - Parse the medication name, dose, route, and frequency from the text\n\
             - Infer route as oral if not specified\n\
             - Only include ICD-10 and RxNorm if confident\n\
             - Output raw JSON only, no markdown fences"
            .to_string();

        let mut parts = vec![format!("Order: {}", input.order_text)];
        if !input.active_medications.is_empty() {
            parts.push(format!(
                "Active medications: {}",
                input.active_medications.join(", ")
            ));
        }

        PromptEnvelope::build(system, parts.join("\n\n"))
    }

    fn build_medication_request(
        &self,
        input: &OrderEntryInput,
        parsed: &serde_json::Value,
    ) -> MedicationRequest {
        let medication_name = parsed
            .get("medication")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");
        let dose = parsed.get("dose").and_then(|v| v.as_str());
        let route = parsed.get("route").and_then(|v| v.as_str());
        let frequency = parsed.get("frequency").and_then(|v| v.as_str());
        let instructions = parsed.get("instructions").and_then(|v| v.as_str());
        let rxnorm = parsed.get("rxnorm").and_then(|v| v.as_str());

        let mut coding = vec![];
        if let Some(code) = rxnorm {
            coding.push(Coding {
                system: Some("http://www.nlm.nih.gov/research/umls/rxnorm".to_string()),
                code: Some(code.to_string()),
                display: Some(medication_name.to_string()),
            });
        }

        let dosage_text = format!(
            "{}{}{}",
            dose.unwrap_or(""),
            route.map(|r| format!(" {}", r)).unwrap_or_default(),
            frequency.map(|f| format!(" {}", f)).unwrap_or_default(),
        );

        let mut med_req = MedicationRequest::new("draft", "order");
        med_req.medication_codeable_concept = Some(CodeableConcept {
            coding: if coding.is_empty() { None } else { Some(coding) },
            text: Some(format!(
                "{} {}",
                medication_name,
                dose.unwrap_or("")
            )),
        });
        med_req.subject = Some(Reference {
            reference: Some(format!("Patient/{}", input.patient_id)),
            display: None,
            type_: None,
        });
        med_req.encounter = Some(Reference {
            reference: Some(format!("Encounter/{}", input.encounter_id)),
            display: None,
            type_: None,
        });
        med_req.requester = Some(Reference {
            reference: Some(format!("Practitioner/{}", input.practitioner_id)),
            display: None,
            type_: None,
        });
        med_req.dosage_instruction = Some(vec![DosageInstruction {
            text: Some(dosage_text),
            route: route.map(|r| CodeableConcept {
                coding: None,
                text: Some(r.to_string()),
            }),
            dose_and_rate: None,
            as_needed_boolean: None,
        }]);

        if let Some(instr) = instructions {
            if let Some(ref mut dosages) = med_req.dosage_instruction {
                if let Some(first) = dosages.first_mut() {
                    if let Some(ref mut text) = first.text {
                        text.push_str(&format!(" — {}", instr));
                    }
                }
            }
        }

        med_req
    }

    fn compute_confidence(&self, parsed: &serde_json::Value, cds_cards: &[CdsCard]) -> Confidence {
        let mut score = 0.6;
        let mut factors = Vec::new();

        // Medication name parsed successfully
        if parsed.get("medication").and_then(|v| v.as_str()).is_some() {
            factors.push("medication_parsed".to_string());
            score += 0.1;
        }

        // Dose parsed
        if parsed.get("dose").and_then(|v| v.as_str()).is_some() {
            factors.push("dose_parsed".to_string());
            score += 0.05;
        }

        // RxNorm code present (indicates high certainty match)
        if parsed.get("rxnorm").and_then(|v| v.as_str()).is_some() {
            factors.push("rxnorm_matched".to_string());
            score += 0.1;
        }

        // CDS card severity reduces confidence
        let has_hard_stop = cds_cards.iter().any(|c| matches!(c.indicator, cds::CdsIndicator::HardStop));
        let has_critical = cds_cards.iter().any(|c| matches!(c.indicator, cds::CdsIndicator::Critical));
        if has_hard_stop {
            factors.push("cds_hard_stop".to_string());
            score -= 0.3;
        } else if has_critical {
            factors.push("cds_critical_alert".to_string());
            score -= 0.2;
        }

        if cds_cards.is_empty() {
            factors.push("no_cds_alerts".to_string());
            score += 0.1;
        }

        Confidence::new(score, factors)
    }

    fn verify_output(&self, med_req: &MedicationRequest) -> Result<(), AgentError> {
        if med_req.status != "draft" {
            return Err(AgentError::VerificationFailed(
                "proposed MedicationRequest status must be 'draft'".to_string(),
            ));
        }
        if med_req.subject.is_none() {
            return Err(AgentError::VerificationFailed(
                "MedicationRequest must include subject reference".to_string(),
            ));
        }
        if med_req.medication_codeable_concept.is_none() {
            return Err(AgentError::VerificationFailed(
                "MedicationRequest must include medication".to_string(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_medication_request() {
        let agent = OrderEntryAgent::new(Arc::new(crate::MockClaudeCapability::new()));
        let input = OrderEntryInput {
            encounter_id: "enc-001".to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: "patient-001".to_string(),
            practitioner_id: "practitioner-001".to_string(),
            order_text: "metformin 500mg BID".to_string(),
            active_medications: vec![],
            capabilities: vec!["order_entry".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("physician".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("AMB".to_string()),
        };

        let parsed = serde_json::json!({
            "medication": "metformin",
            "dose": "500mg",
            "route": "oral",
            "frequency": "BID",
            "rxnorm": "860975"
        });

        let med_req = agent.build_medication_request(&input, &parsed);
        assert_eq!(med_req.status, "draft");
        assert_eq!(med_req.intent, "order");
        assert!(med_req.subject.is_some());
        assert!(med_req.medication_codeable_concept.is_some());
    }
}
