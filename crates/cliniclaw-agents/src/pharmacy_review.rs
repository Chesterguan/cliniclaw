use std::collections::HashMap;
use std::sync::Arc;

use cliniclaw_persist::{sha256_hash, AuditEvent};
use cliniclaw_policy::{ActionContext, Capability, PolicyDecision, PolicyEngine};

use cliniclaw_kernel::Confidence;

use crate::cds::{CdsCard, CdsIndicator, CdsSuggestion};
use crate::error::AgentError;
use crate::llm::LlmCapability;
use crate::PromptEnvelope;

#[derive(Debug, Clone)]
pub struct PharmacyReviewInput {
    pub encounter_id: String,
    pub encounter_status: String,
    pub patient_id: String,
    pub practitioner_id: String,
    /// Ordered list of pending medication orders to review
    pub pending_orders: Vec<String>,
    /// Currently active medications for interaction and duplicate checking
    pub active_medications: Vec<String>,
    /// Patient-reported and documented allergies
    pub allergies: Vec<String>,
    /// Bare capability names (backward-compatible path)
    pub capabilities: Vec<String>,
    /// Structured capability tokens for skill-aware evaluation
    pub capability_tokens: Vec<Capability>,
    /// The practitioner's clinical role (e.g. "pharmacist")
    pub practitioner_role: Option<String>,
    /// Whether the patient is active in the system
    pub patient_active: bool,
    /// Whether the patient is deceased
    pub patient_deceased: Option<bool>,
    /// Encounter class code (e.g. "IMP", "EMER")
    pub encounter_class: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PharmacyReviewOutput {
    /// Overall disposition: "approved", "flagged", or "hold"
    pub review_status: String,
    /// Drug-drug, drug-allergy, or other interactions found
    pub interactions_found: Vec<String>,
    /// Recommended therapeutic substitutions
    pub substitution_suggestions: Vec<String>,
    /// CDS Hooks-style alert cards for each flagged interaction or concern
    pub cds_cards: Vec<CdsCard>,
    pub confidence: Confidence,
    pub audit_event: AuditEvent,
    pub policy_decision: PolicyDecision,
    /// SHA-256 hash of the matched skill spec (if any)
    pub spec_hash: Option<String>,
}

pub struct PharmacyReviewAgent {
    llm: Arc<dyn LlmCapability>,
}

impl PharmacyReviewAgent {
    pub fn new(llm: Arc<dyn LlmCapability>) -> Self {
        Self { llm }
    }

    /// Run the pharmacy review workflow (advisory only — no FHIR write).
    /// 1. Skill-aware policy check  2. Build prompt  3. Call Claude
    /// 4. Parse response  5. Build CDS cards  6. Verify output  7. Build audit event
    ///
    /// This agent does NOT write a FHIR resource — its output is advisory.
    /// CDS cards surface to the prescriber for review and override.
    ///
    /// Note: the agent *builds* the audit event but does not persist it.
    /// Persistence is the API layer's responsibility (VERITAS separation).
    pub async fn evaluate(
        &self,
        input: &PharmacyReviewInput,
        policy_engine: &PolicyEngine,
    ) -> Result<PharmacyReviewOutput, AgentError> {
        // Step 1: Build context and run skill-aware policy evaluation
        let context = self.build_context(input);
        let skill_eval = policy_engine.evaluate_with_skill(&context)?;

        match &skill_eval.decision {
            PolicyDecision::Deny => {
                tracing::warn!(
                    actor_id = %input.practitioner_id,
                    encounter_id = %input.encounter_id,
                    "policy denied pharmacy review"
                );
                return Err(AgentError::PolicyDenied(format!(
                    "pharmacy review denied for encounter {}",
                    input.encounter_id
                )));
            }
            PolicyDecision::RequireApproval => {
                return Err(AgentError::RequiresApproval {
                    action: "pharmacy_review.evaluate".to_string(),
                });
            }
            PolicyDecision::Allow => {
                tracing::info!(
                    actor_id = %input.practitioner_id,
                    encounter_id = %input.encounter_id,
                    spec_hash = ?skill_eval.spec_hash,
                    "policy allowed pharmacy review"
                );
            }
        }

        // Step 2: Build de-identified prompt
        let prompt = self.build_prompt(input);

        // Step 3: Call LLM
        let response_text = self.llm.call(&prompt).await?;

        // Step 4: Parse response
        let parsed: serde_json::Value = serde_json::from_str(&response_text).map_err(|e| {
            AgentError::ClaudeApi(format!("failed to parse pharmacy review response: {e}"))
        })?;

        let review_status = parsed
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("flagged")
            .to_string();

        let interactions_found: Vec<String> = parsed
            .get("interactions")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();

        let substitution_suggestions: Vec<String> = parsed
            .get("substitutions")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();

        // Step 5: Build CDS cards for each interaction found
        let cds_cards = self.build_cds_cards(&interactions_found, &review_status);

        // Step 6: Verify output
        self.verify_output(&review_status, &cds_cards)?;

        // Step 7: Create audit event with skill metadata
        //
        // PHI note: pending_orders and allergies may contain medication names but
        // NOT patient identifiers. We log counts only, not the content.
        let input_descriptor = serde_json::to_vec(&serde_json::json!({
            "encounter_id": input.encounter_id,
            "practitioner_id": input.practitioner_id,
            "pending_order_count": input.pending_orders.len(),
            "active_medication_count": input.active_medications.len(),
            "allergy_count": input.allergies.len(),
            "skill_id": skill_eval.skill_id,
            "spec_hash": skill_eval.spec_hash,
        }))?;
        let output_descriptor = serde_json::to_vec(&serde_json::json!({
            "review_status": review_status,
            "interaction_count": interactions_found.len(),
            "substitution_count": substitution_suggestions.len(),
            "cds_card_count": cds_cards.len(),
        }))?;

        let audit_event = AuditEvent::new(
            &input.practitioner_id,
            Some(input.patient_id.clone()),
            "pharmacy_review.evaluate",
            &skill_eval.decision.to_string(),
            sha256_hash(&input_descriptor),
            sha256_hash(&output_descriptor),
            "", // previous_hash assigned atomically by SqliteAuditStore::append
        );

        tracing::info!(
            audit_event_id = %audit_event.id,
            encounter_id = %input.encounter_id,
            review_status = %review_status,
            interaction_count = interactions_found.len(),
            cds_card_count = cds_cards.len(),
            "pharmacy review completed"
        );

        let confidence = self.compute_confidence(&interactions_found, &review_status, &cds_cards);

        Ok(PharmacyReviewOutput {
            review_status,
            interactions_found,
            substitution_suggestions,
            cds_cards,
            confidence,
            audit_event,
            policy_decision: skill_eval.decision,
            spec_hash: skill_eval.spec_hash,
        })
    }

    fn build_context(&self, input: &PharmacyReviewInput) -> ActionContext {
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
            action: "pharmacy_review.evaluate".to_string(),
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

    fn build_prompt(&self, input: &PharmacyReviewInput) -> PromptEnvelope {
        let system = "You are a clinical pharmacy review assistant. Review the pending medication \
             orders against active medications and allergies, then provide a structured safety \
             assessment. Output ONLY valid JSON matching this schema:\n\
             {\n\
               \"status\": \"approved|flagged|hold\",\n\
               \"interactions\": [\"interaction description 1\", \"interaction description 2\"],\n\
               \"substitutions\": [\"substitution suggestion 1\"]\n\
             }\n\n\
             Status definitions:\n\
             - approved: No interactions or concerns found\n\
             - flagged: Interactions found but can proceed with prescriber awareness\n\
             - hold: Serious interaction or allergy conflict requiring intervention before dispensing\n\n\
             Rules:\n\
             - Check all pending orders against active medications for drug-drug interactions\n\
             - Check all pending orders against documented allergies for contraindications\n\
             - Flag duplicates (same drug or same therapeutic class already active)\n\
             - Suggest therapeutic alternatives only when clinically supported\n\
             - Do not fabricate interactions not supported by the provided data\n\
             - Output raw JSON only, no markdown fences"
            .to_string();

        let mut parts = Vec::new();

        if !input.pending_orders.is_empty() {
            parts.push(format!(
                "Pending orders:\n{}",
                input.pending_orders.join("\n")
            ));
        }

        if !input.active_medications.is_empty() {
            parts.push(format!(
                "Active medications: {}",
                input.active_medications.join(", ")
            ));
        }

        if !input.allergies.is_empty() {
            parts.push(format!("Allergies: {}", input.allergies.join(", ")));
        }

        PromptEnvelope::build(system, parts.join("\n\n"))
    }

    /// Convert the list of interaction descriptions into CDS Hooks cards.
    ///
    /// Severity mapping:
    /// - Status "hold" → Critical card
    /// - Individual interactions → Warning cards
    /// - Status "approved" and no interactions → Info card
    fn build_cds_cards(&self, interactions: &[String], review_status: &str) -> Vec<CdsCard> {
        let mut cards = Vec::new();

        if interactions.is_empty() && review_status == "approved" {
            // Clean review — surface an informational card
            cards.push(CdsCard {
                summary: "Pharmacy review: no interactions detected".into(),
                detail: Some("All pending orders cleared by automated pharmacy review.".into()),
                indicator: CdsIndicator::Info,
                source: "ClinicClaw Pharmacy Review".into(),
                suggestions: vec![],
            });
            return cards;
        }

        // Generate one card per interaction, escalated to Critical if status is "hold"
        for interaction in interactions {
            let indicator = if review_status == "hold" {
                CdsIndicator::Critical
            } else {
                CdsIndicator::Warning
            };

            let suggestions = if review_status == "hold" {
                vec![CdsSuggestion {
                    label: "Place order on hold pending pharmacist review".into(),
                    action_type: "hold".to_string(),
                }]
            } else {
                vec![
                    CdsSuggestion {
                        label: "Override with reason".into(),
                        action_type: "override".to_string(),
                    },
                    CdsSuggestion {
                        label: "Remove order".into(),
                        action_type: "cancel".to_string(),
                    },
                ]
            };

            cards.push(CdsCard {
                summary: "Drug interaction detected".into(),
                detail: Some(interaction.clone()),
                indicator,
                source: "ClinicClaw Pharmacy Review".into(),
                suggestions,
            });
        }

        cards
    }

    fn compute_confidence(
        &self,
        interactions_found: &[String],
        review_status: &str,
        cds_cards: &[CdsCard],
    ) -> Confidence {
        let mut score = 0.6;
        let mut factors = Vec::new();

        // Clean review is the most deterministic outcome
        if interactions_found.is_empty() && review_status == "approved" {
            factors.push("clean_review".to_string());
            score += 0.2;
        }

        // Any interactions identified signals the model engaged with the drug list
        if !interactions_found.is_empty() {
            factors.push("interactions_identified".to_string());
            // Many interactions may indicate complex polypharmacy — lower certainty per item
            if interactions_found.len() > 3 {
                factors.push("complex_polypharmacy".to_string());
                score -= 0.1;
            }
        }

        // Hold status is a strong, deterministic clinical signal
        if review_status == "hold" {
            factors.push("hold_status".to_string());
            score += 0.1;
        }

        // CDS cards generated successfully
        if !cds_cards.is_empty() {
            factors.push("cds_cards_generated".to_string());
            score += 0.05;
        }

        // Presence of critical cards reduces confidence in automatic approval
        let has_critical = cds_cards.iter().any(|c| matches!(c.indicator, CdsIndicator::Critical));
        if has_critical {
            factors.push("critical_alert_present".to_string());
            score -= 0.1;
        }

        Confidence::new(score, factors)
    }

    fn verify_output(&self, review_status: &str, cds_cards: &[CdsCard]) -> Result<(), AgentError> {
        // Status must be one of the three valid dispositions
        if !matches!(review_status, "approved" | "flagged" | "hold") {
            return Err(AgentError::VerificationFailed(format!(
                "review_status '{review_status}' is not a valid disposition (approved/flagged/hold)"
            )));
        }

        // If status is "flagged" or "hold", there must be at least one CDS card
        // to give the prescriber actionable context.
        if review_status != "approved" && cds_cards.is_empty() {
            return Err(AgentError::VerificationFailed(format!(
                "pharmacy review status '{review_status}' requires at least one CDS card"
            )));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_cds_cards_clean() {
        let agent = PharmacyReviewAgent::new(Arc::new(crate::MockClaudeCapability::new()));
        let cards = agent.build_cds_cards(&[], "approved");
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].indicator, CdsIndicator::Info);
    }

    #[test]
    fn test_build_cds_cards_flagged() {
        let agent = PharmacyReviewAgent::new(Arc::new(crate::MockClaudeCapability::new()));
        let interactions = vec![
            "Warfarin + Naproxen: increased bleeding risk".to_string(),
        ];
        let cards = agent.build_cds_cards(&interactions, "flagged");
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].indicator, CdsIndicator::Warning);
        assert_eq!(cards[0].suggestions.len(), 2);
    }

    #[test]
    fn test_build_cds_cards_hold() {
        let agent = PharmacyReviewAgent::new(Arc::new(crate::MockClaudeCapability::new()));
        let interactions = vec![
            "Penicillin allergy — patient has documented penicillin allergy".to_string(),
        ];
        let cards = agent.build_cds_cards(&interactions, "hold");
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].indicator, CdsIndicator::Critical);
        assert_eq!(cards[0].suggestions.len(), 1);
        assert_eq!(cards[0].suggestions[0].action_type, "hold");
    }

    #[test]
    fn test_verify_rejects_invalid_status() {
        let agent = PharmacyReviewAgent::new(Arc::new(crate::MockClaudeCapability::new()));
        assert!(agent.verify_output("unknown", &[]).is_err());
    }

    #[test]
    fn test_verify_requires_cds_card_when_flagged() {
        let agent = PharmacyReviewAgent::new(Arc::new(crate::MockClaudeCapability::new()));
        // flagged with no CDS cards → invalid
        assert!(agent.verify_output("flagged", &[]).is_err());
    }

    #[test]
    fn test_verify_approved_with_no_cards_ok() {
        let agent = PharmacyReviewAgent::new(Arc::new(crate::MockClaudeCapability::new()));
        assert!(agent.verify_output("approved", &[]).is_ok());
    }
}
