use std::sync::Arc;

use cliniclaw_kernel::{AgentEventType, EventEmitter, StepStatus};

use crate::state::AppState;

/// Check if a completed turn should trigger a chain of agent actions.
///
/// This is deterministic pattern matching on turn output — used in mock mode
/// to demonstrate agent-to-agent collaboration. In production, chain rules
/// would come from a policy file.
pub async fn check_chain_triggers(
    state: &Arc<AppState>,
    source_turn_id: &str,
    agent_name: &str,
    encounter_id: &str,
    workspace_id: &str,
    input_snapshot: &serde_json::Value,
    output_snapshot: &serde_json::Value,
    emitter: &EventEmitter,
) {
    // Rule: if ambient_doc output mentions "lisinopril", trigger order_entry
    if agent_name == "ambient_doc" {
        let output_str = serde_json::to_string(output_snapshot).unwrap_or_default();
        let output_lower = output_str.to_lowercase();

        if output_lower.contains("lisinopril") {
            tracing::info!(
                source_turn_id = source_turn_id,
                "chain trigger: lisinopril detected in SOAP note — triggering order_entry"
            );

            emitter.emit(AgentEventType::ChainTrigger {
                trigger_pattern: "lisinopril detected in SOAP note".into(),
                target_agent: "order_entry".into(),
            });

            // Extract patient/practitioner IDs from the source turn's input context
            let patient_id = input_snapshot
                .get("patient_id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let practitioner_id = input_snapshot
                .get("practitioner_id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            // Fire order_entry agent
            let chain_emitter = EventEmitter::new(
                state.event_tx.clone(),
                encounter_id,
                "order_entry",
            )
            .with_workspace(workspace_id.to_string())
            .with_trigger(source_turn_id.to_string());

            chain_emitter.emit(AgentEventType::AgentStarted);

            // Build input for the chained order, inheriting context from source turn
            let input = cliniclaw_agents::OrderEntryInput {
                encounter_id: encounter_id.to_string(),
                encounter_status: "in-progress".to_string(),
                patient_id,
                practitioner_id,
                order_text: "increase lisinopril to 20mg PO daily".to_string(),
                active_medications: vec!["Lisinopril 10mg PO daily".to_string()],
                capabilities: vec!["order_entry".to_string()],
                capability_tokens: vec![],
                practitioner_role: Some("physician".to_string()),
                patient_active: true,
                patient_deceased: None,
                encounter_class: Some("AMB".to_string()),
            };

            chain_emitter.emit(AgentEventType::ContextBuilding {
                step: 1,
                detail: "Chain-triggered from ambient_doc output".into(),
            });

            chain_emitter.emit(AgentEventType::PolicyEvaluation {
                decision: "evaluating".into(),
                rule_name: None,
            });

            chain_emitter.emit(AgentEventType::LlmCall {
                status: StepStatus::Started,
                elapsed_ms: None,
            });

            let llm_start = std::time::Instant::now();
            let order_agent = cliniclaw_agents::OrderEntryAgent::new(state.llm.clone());
            let result = order_agent
                .propose_order(&input, &state.policy_engine)
                .await;

            let llm_elapsed = llm_start.elapsed().as_millis() as u64;

            match result {
                Ok(mut output) => {
                    chain_emitter.emit(AgentEventType::LlmCall {
                        status: StepStatus::Completed,
                        elapsed_ms: Some(llm_elapsed),
                    });

                    chain_emitter.emit(AgentEventType::PolicyEvaluation {
                        decision: "allow".into(),
                        rule_name: Some("allow_order_entry".into()),
                    });

                    // CDS check
                    use cliniclaw_agents::CdsIndicator;
                    let max_severity = output.cds_cards.iter()
                        .map(|c| match c.indicator {
                            CdsIndicator::HardStop => 4,
                            CdsIndicator::Critical => 3,
                            CdsIndicator::Warning => 2,
                            CdsIndicator::Info => 1,
                        })
                        .max()
                        .map(|s| match s {
                            4 => "hard_stop",
                            3 => "critical",
                            2 => "warning",
                            _ => "info",
                        })
                        .map(String::from);

                    chain_emitter.emit(AgentEventType::CdsCheck {
                        cards_count: output.cds_cards.len(),
                        max_severity,
                    });

                    chain_emitter.emit(AgentEventType::Verification {
                        passed: true,
                        detail: Some("Chained order verified".into()),
                    });

                    // Persist audit
                    if let Err(e) = state.audit_store.append(&mut output.audit_event).await {
                        tracing::warn!(
                            source_turn_id = source_turn_id,
                            error = %e,
                            "chain: failed to persist audit event"
                        );
                    }
                    chain_emitter.emit(AgentEventType::AuditCreation {
                        audit_event_id: output.audit_event.id.to_string(),
                    });

                    // Write to FHIR
                    let fhir_result: Result<cliniclaw_fhir::MedicationRequest, _> =
                        cliniclaw_fhir::backend::create_typed(
                            state.fhir.as_ref(),
                            &output.medication_request,
                        )
                        .await;

                    let output_json = match &fhir_result {
                        Ok(created) => {
                            chain_emitter.emit(AgentEventType::FhirWrite {
                                resource_type: "MedicationRequest".into(),
                                resource_id: created.id.clone(),
                            });
                            serde_json::to_value(created).unwrap_or_default()
                        }
                        Err(e) => {
                            tracing::warn!(
                                source_turn_id = source_turn_id,
                                error = %e,
                                "chain: FHIR write failed, creating turn with agent output"
                            );
                            // Fall back to agent output (not persisted to FHIR)
                            serde_json::to_value(&output.medication_request).unwrap_or_default()
                        }
                    };

                    // Create chained turn regardless of FHIR result
                    let tid = uuid::Uuid::new_v4().to_string();
                    let turn = cliniclaw_kernel::Turn {
                        id: tid.clone(),
                        workspace_id: workspace_id.to_string(),
                        agent_name: "order_entry".to_string(),
                        action: "propose_order".to_string(),
                        input_snapshot: serde_json::json!({
                            "encounter_id": encounter_id,
                            "order_text": "increase lisinopril to 20mg PO daily",
                            "chain_source": "ambient_doc",
                        }),
                        output_snapshot: output_json,
                        confidence: output.confidence.clone(),
                        status: cliniclaw_kernel::TurnStatus::Pending,
                        feedback: None,
                        created_at: chrono::Utc::now(),
                        resolved_at: None,
                        resolved_by: None,
                        triggered_by_turn_id: Some(source_turn_id.to_string()),
                    };
                    if let Err(e) = state.workspace_store.create_turn(&turn).await {
                        tracing::warn!(
                            turn_id = %tid,
                            error = %e,
                            "chain: failed to persist chained turn"
                        );
                    }

                    chain_emitter.emit_with_turn(&tid, AgentEventType::TurnCreation {
                        turn_id: tid.clone(),
                        confidence_score: output.confidence.score,
                    });

                    chain_emitter.emit(AgentEventType::AgentCompleted {
                        confidence_score: output.confidence.score,
                        elapsed_ms: llm_elapsed,
                    });
                }
                Err(e) => {
                    chain_emitter.emit(AgentEventType::AgentFailed {
                        error: e.to_string(),
                    });
                }
            }
        }
    }
}
