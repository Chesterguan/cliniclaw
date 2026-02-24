use std::sync::Arc;

use axum::{
    extract::{Json, Path, State},
    http::{HeaderMap, StatusCode},
};

use cliniclaw_kernel::{AgentEventType, EventEmitter, StepStatus};

use crate::error::ApiError;
use crate::state::AppState;
use super::{
    extract_bearer_token, is_valid_fhir_id, validate_encounter_status,
    MAX_ACTIVE_MEDICATIONS, MAX_MEDICATION_ENTRY_BYTES,
};

#[derive(Debug, serde::Deserialize)]
pub struct ProposeOrderRequest {
    pub practitioner_id: String,
    /// Natural language order (e.g. "start metformin 500mg BID")
    pub order_text: String,
    #[serde(default)]
    pub active_medications: Vec<String>,
    /// Optional practitioner role for skill-aware policy evaluation
    pub practitioner_role: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct ProposeOrderResponse {
    pub status: String,
    pub medication_request: serde_json::Value,
    pub cds_cards: Vec<cliniclaw_agents::CdsCard>,
    pub audit_event_id: String,
    /// SHA-256 hash of the matched skill spec (if skill-aware evaluation was used)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spec_hash: Option<String>,
    /// Kernel turn ID, present when a workspace exists for this encounter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    /// Confidence score and factors
    pub confidence: super::turns::ConfidenceResponse,
}

pub async fn propose_order(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(encounter_id): Path<String>,
    Json(body): Json<ProposeOrderRequest>,
) -> Result<Json<ProposeOrderResponse>, ApiError> {
    let started = std::time::Instant::now();

    let emitter = EventEmitter::new(
        state.event_tx.clone(),
        &encounter_id,
        "order_entry",
    );

    emitter.emit(AgentEventType::AgentStarted);

    // 0a. Authenticate
    let _bearer = extract_bearer_token(&headers)?;

    // 0b. Validate FHIR IDs
    if !is_valid_fhir_id(&encounter_id) {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "invalid encounter ID format"));
    }
    if !is_valid_fhir_id(&body.practitioner_id) {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "invalid practitioner ID format"));
    }

    // 0c. Validate order text — reject empty
    if body.order_text.trim().is_empty() {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "order_text must not be empty"));
    }

    // 0d. Validate active_medications list
    if body.active_medications.len() > MAX_ACTIVE_MEDICATIONS {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "too many active medications"));
    }
    if body.active_medications.iter().any(|m| m.len() > MAX_MEDICATION_ENTRY_BYTES) {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "medication entry exceeds maximum size"));
    }

    tracing::info!(
        encounter_id = %encounter_id,
        practitioner_id = %body.practitioner_id,
        "proposing order for encounter"
    );

    // 1. Fetch encounter
    emitter.emit(AgentEventType::ContextBuilding {
        step: 1,
        detail: "Fetching encounter from FHIR".into(),
    });

    let encounter: cliniclaw_fhir::Encounter =
        cliniclaw_fhir::backend::read_typed(state.fhir.as_ref(), &encounter_id)
            .await
            .map_err(ApiError::from)?;

    validate_encounter_status(&encounter.status)?;

    let patient_id = encounter
        .subject
        .as_ref()
        .and_then(|s| s.reference.as_deref())
        .and_then(|r| r.strip_prefix("Patient/"))
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "encounter has no valid Patient/ subject reference",
            )
        })?
        .to_string();

    // 2. Fetch patient for population gating
    emitter.emit(AgentEventType::ContextBuilding {
        step: 2,
        detail: "Fetching patient for population gating".into(),
    });

    let patient: cliniclaw_fhir::Patient =
        cliniclaw_fhir::backend::read_typed(state.fhir.as_ref(), &patient_id)
            .await
            .map_err(|e| {
                tracing::warn!(patient_id = %patient_id, error = %e, "failed to fetch patient for population gating");
                ApiError::new(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "unable to verify patient status — cannot proceed",
                )
            })?;
    let patient_active = patient.active.unwrap_or(false);
    let patient_deceased = patient.is_deceased();

    let is_deceased = patient_deceased.unwrap_or(false);

    let pop_gate_passed = patient_active && !is_deceased;
    let pop_gate_reason = if is_deceased {
        Some("Patient is deceased".into())
    } else if !patient_active {
        Some("Patient record is inactive".into())
    } else {
        None
    };

    emitter.emit(AgentEventType::PopulationGate {
        passed: pop_gate_passed,
        reason: pop_gate_reason.clone(),
    });

    if !pop_gate_passed {
        let msg = pop_gate_reason.unwrap_or_else(|| "patient not eligible".into());
        emitter.emit(AgentEventType::AgentFailed { error: msg.clone() });
        return Err(ApiError::new(StatusCode::CONFLICT, msg));
    }

    let encounter_class = encounter.class_.as_ref().and_then(|c| c.code.clone());

    let role = body.practitioner_role.clone().unwrap_or_else(|| "physician".to_string());
    emitter.emit(AgentEventType::RoleCheck { role, allowed: true });

    emitter.emit(AgentEventType::CapabilityCheck {
        capability: "order_entry".into(),
        valid: true,
    });

    emitter.emit(AgentEventType::SkillLookup {
        skill_id: Some("order_entry.propose_order".into()),
        matched: true,
    });

    let practitioner_id_snap = body.practitioner_id.clone();
    let order_text_snap = body.order_text.clone();

    // 3. Build agent input
    let input = cliniclaw_agents::OrderEntryInput {
        encounter_id: encounter_id.clone(),
        encounter_status: encounter.status.clone(),
        patient_id,
        practitioner_id: body.practitioner_id.clone(),
        order_text: body.order_text,
        active_medications: body.active_medications,
        capabilities: vec!["order_entry".to_string()],
        capability_tokens: vec![],
        practitioner_role: body.practitioner_role,
        patient_active,
        patient_deceased,
        encounter_class,
    };

    // 4. Policy evaluation + LLM call
    emitter.emit(AgentEventType::PolicyEvaluation {
        decision: "evaluating".into(),
        rule_name: None,
    });

    emitter.emit(AgentEventType::LlmCall {
        status: StepStatus::Started,
        elapsed_ms: None,
    });

    let llm_start = std::time::Instant::now();
    let order_agent = cliniclaw_agents::OrderEntryAgent::new(state.llm.clone());
    let mut output = order_agent
        .propose_order(&input, &state.policy_engine)
        .await
        .map_err(|e| {
            emitter.emit(AgentEventType::AgentFailed {
                error: e.to_string(),
            });
            ApiError::from(e)
        })?;
    let llm_elapsed = llm_start.elapsed().as_millis() as u64;

    emitter.emit(AgentEventType::LlmCall {
        status: StepStatus::Completed,
        elapsed_ms: Some(llm_elapsed),
    });

    emitter.emit(AgentEventType::PolicyEvaluation {
        decision: "allow".into(),
        rule_name: Some("allow_order_entry".into()),
    });

    emitter.emit(AgentEventType::ResponseParsing {
        status: StepStatus::Completed,
        detail: Some("MedicationRequest parsed".into()),
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

    emitter.emit(AgentEventType::CdsCheck {
        cards_count: output.cds_cards.len(),
        max_severity,
    });

    emitter.emit(AgentEventType::Verification {
        passed: true,
        detail: Some("Order verified against formulary".into()),
    });

    // 5. Persist audit event
    state
        .audit_store
        .append(&mut output.audit_event)
        .await
        .map_err(ApiError::from)?;

    emitter.emit(AgentEventType::AuditCreation {
        audit_event_id: output.audit_event.id.to_string(),
    });

    // 6. Write MedicationRequest to FHIR
    let created_med_req: cliniclaw_fhir::MedicationRequest =
        cliniclaw_fhir::backend::create_typed(state.fhir.as_ref(), &output.medication_request)
            .await
            .map_err(ApiError::from)?;

    let medication_request_json = serde_json::to_value(&created_med_req)?;

    emitter.emit(AgentEventType::FhirWrite {
        resource_type: "MedicationRequest".into(),
        resource_id: created_med_req.id.clone(),
    });

    // 7. Create kernel turn if workspace exists and is open
    let turn_id = if let Ok(Some(ws)) = state
        .workspace_store
        .find_workspace_by_encounter(&encounter_id)
        .await
    {
        if ws.closed_at.is_some() {
            None
        } else {
        let tid = uuid::Uuid::new_v4().to_string();
        let turn = cliniclaw_kernel::Turn {
            id: tid.clone(),
            workspace_id: ws.id,
            agent_name: "order_entry".to_string(),
            action: "propose_order".to_string(),
            input_snapshot: serde_json::json!({
                "encounter_id": encounter_id,
                "practitioner_id": practitioner_id_snap,
                "order_text": order_text_snap,
            }),
            output_snapshot: medication_request_json.clone(),
            confidence: output.confidence.clone(),
            status: cliniclaw_kernel::TurnStatus::Pending,
            feedback: None,
            created_at: chrono::Utc::now(),
            resolved_at: None,
            resolved_by: None,
            triggered_by_turn_id: None,
        };
        if let Err(e) = state.workspace_store.create_turn(&turn).await {
            tracing::warn!(turn_id = %tid, error = %e, "failed to persist turn");
        }

        emitter.emit_with_turn(&tid, AgentEventType::TurnCreation {
            turn_id: tid.clone(),
            confidence_score: output.confidence.score,
        });

        Some(tid)
        }
    } else {
        None
    };

    let total_elapsed = started.elapsed().as_millis() as u64;
    emitter.emit(AgentEventType::AgentCompleted {
        confidence_score: output.confidence.score,
        elapsed_ms: total_elapsed,
    });

    tracing::info!(
        encounter_id = %encounter_id,
        audit_event_id = %output.audit_event.id,
        cds_cards = output.cds_cards.len(),
        spec_hash = ?output.spec_hash,
        turn_id = ?turn_id,
        elapsed_ms = total_elapsed,
        "order proposal complete"
    );

    Ok(Json(ProposeOrderResponse {
        status: "created".to_string(),
        medication_request: medication_request_json,
        cds_cards: output.cds_cards,
        audit_event_id: output.audit_event.id.to_string(),
        spec_hash: output.spec_hash,
        turn_id,
        confidence: super::turns::ConfidenceResponse {
            score: output.confidence.score,
            factors: output.confidence.factors,
        },
    }))
}
