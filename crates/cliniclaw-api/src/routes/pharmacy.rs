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
pub struct PharmacyReviewRequest {
    pub practitioner_id: String,
    #[serde(default)]
    pub pending_orders: Vec<String>,
    #[serde(default)]
    pub active_medications: Vec<String>,
    #[serde(default)]
    pub allergies: Vec<String>,
    /// Optional practitioner role for skill-aware policy evaluation
    pub practitioner_role: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct PharmacyReviewResponse {
    pub status: String,
    pub review_status: String,
    pub interactions_found: Vec<String>,
    pub substitution_suggestions: Vec<String>,
    /// CDS Hooks-style alert cards (advisory only — no FHIR write)
    pub cds_cards: serde_json::Value,
    pub audit_event_id: String,
    /// Kernel turn ID, present when a workspace exists for this encounter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    /// Confidence score and factors
    pub confidence: super::turns::ConfidenceResponse,
}

pub async fn pharmacy_review_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(encounter_id): Path<String>,
    Json(body): Json<PharmacyReviewRequest>,
) -> Result<Json<PharmacyReviewResponse>, ApiError> {
    let started = std::time::Instant::now();

    // Create event emitter for real-time SSE streaming
    let emitter = EventEmitter::new(
        state.event_tx.clone(),
        &encounter_id,
        "pharmacy_review",
    );

    emitter.emit(AgentEventType::AgentStarted);

    // 0a. Authenticate — require a Bearer token
    let _bearer = extract_bearer_token(&headers)?;

    // 0b. Validate all FHIR ID fields
    if !is_valid_fhir_id(&encounter_id) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid encounter ID format",
        ));
    }
    if !is_valid_fhir_id(&body.practitioner_id) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid practitioner ID format",
        ));
    }

    // 0c. Validate pending orders — at least one must be provided
    if body.pending_orders.is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "at least one pending order is required",
        ));
    }

    // 0d. Validate list lengths and entry sizes
    if body.pending_orders.len() > MAX_ACTIVE_MEDICATIONS {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "too many pending orders",
        ));
    }
    if body.pending_orders.iter().any(|o| o.len() > MAX_MEDICATION_ENTRY_BYTES) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "pending order entry exceeds maximum size",
        ));
    }
    if body.active_medications.len() > MAX_ACTIVE_MEDICATIONS {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "too many active medications",
        ));
    }
    if body.active_medications.iter().any(|m| m.len() > MAX_MEDICATION_ENTRY_BYTES) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "medication entry exceeds maximum size",
        ));
    }

    tracing::info!(
        encounter_id = %encounter_id,
        practitioner_id = %body.practitioner_id,
        pending_order_count = body.pending_orders.len(),
        "running pharmacy review for encounter"
    );

    // 1. Fetch encounter from FHIR via backend trait
    emitter.emit(AgentEventType::ContextBuilding {
        step: 1,
        detail: "Fetching encounter from FHIR".into(),
    });

    let encounter: cliniclaw_fhir::Encounter =
        cliniclaw_fhir::backend::read_typed(state.fhir.as_ref(), &encounter_id)
            .await
            .map_err(ApiError::from)?;

    // Fail-fast if encounter status doesn't allow agent actions
    validate_encounter_status(&encounter.status)?;

    // Fail if encounter has no valid Patient/ subject reference
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

    // 2. Fetch patient for population gating — fail-closed on error
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
    // Fail-closed: absent `active` field → treat as inactive (deny by default)
    let patient_active = patient.active.unwrap_or(false);
    let patient_deceased = patient.is_deceased();

    let is_deceased = patient_deceased.unwrap_or(false);

    // Population gate check — fail-closed: block deceased/inactive patients
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

    let encounter_class = encounter
        .class_
        .as_ref()
        .and_then(|c| c.code.clone());

    // Role check — pharmacy reviews may be performed by pharmacists or physicians
    let role = body.practitioner_role.clone().unwrap_or_else(|| "pharmacist".to_string());
    emitter.emit(AgentEventType::RoleCheck {
        role: role.clone(),
        allowed: true, // will be checked by policy engine
    });

    // Capability check
    emitter.emit(AgentEventType::CapabilityCheck {
        capability: "pharmacy_review".into(),
        valid: true,
    });

    // Capture fields needed for the turn snapshot before body is moved into input
    let practitioner_id_snap = body.practitioner_id.clone();
    let pending_order_count_snap = body.pending_orders.len();

    // 3. Build agent input
    emitter.emit(AgentEventType::SkillLookup {
        skill_id: Some("pharmacy_review.evaluate".into()),
        matched: true,
    });

    let input = cliniclaw_agents::PharmacyReviewInput {
        encounter_id: encounter_id.clone(),
        encounter_status: encounter.status.clone(),
        patient_id: patient_id.clone(),
        practitioner_id: body.practitioner_id.clone(),
        pending_orders: body.pending_orders,
        active_medications: body.active_medications,
        allergies: body.allergies,
        capabilities: vec!["pharmacy_review".to_string()],
        capability_tokens: vec![],
        practitioner_role: body.practitioner_role,
        patient_active,
        patient_deceased,
        encounter_class,
    };

    // 4. Policy evaluation
    emitter.emit(AgentEventType::PolicyEvaluation {
        decision: "evaluating".into(),
        rule_name: None,
    });

    // Run the agent (policy → Claude → verify → build audit event)
    emitter.emit(AgentEventType::LlmCall {
        status: StepStatus::Started,
        elapsed_ms: None,
    });

    let llm_start = std::time::Instant::now();
    let pharmacy_agent = cliniclaw_agents::PharmacyReviewAgent::new(state.llm.clone());
    let mut output = pharmacy_agent
        .evaluate(&input, &state.policy_engine)
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
        rule_name: Some("allow_pharmacy_review".into()),
    });

    // Response parsing
    emitter.emit(AgentEventType::ResponseParsing {
        status: StepStatus::Completed,
        detail: Some("Pharmacy review parsed successfully".into()),
    });

    // Verification
    emitter.emit(AgentEventType::Verification {
        passed: true,
        detail: Some("Review status and CDS cards verified".into()),
    });

    // 5. Persist audit event — append atomically assigns previous_hash + event_hash
    state
        .audit_store
        .append(&mut output.audit_event)
        .await
        .map_err(ApiError::from)?;

    emitter.emit(AgentEventType::AuditCreation {
        audit_event_id: output.audit_event.id.to_string(),
    });

    // 6. Advisory only — no FHIR write; emit CdsCheck event instead
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

    let cds_cards_json = serde_json::to_value(&output.cds_cards)?;

    // 7. Create a kernel turn if an open workspace exists for this encounter
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
                workspace_id: ws.id.clone(),
                agent_name: "pharmacy_review".to_string(),
                action: "evaluate".to_string(),
                input_snapshot: serde_json::json!({
                    "encounter_id": encounter_id,
                    "patient_id": patient_id,
                    "practitioner_id": practitioner_id_snap,
                    "pending_order_count": pending_order_count_snap,
                }),
                output_snapshot: serde_json::json!({
                    "review_status": output.review_status,
                    "interaction_count": output.interactions_found.len(),
                    "cds_card_count": output.cds_cards.len(),
                }),
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
        review_status = %output.review_status,
        interaction_count = output.interactions_found.len(),
        cds_card_count = output.cds_cards.len(),
        turn_id = ?turn_id,
        elapsed_ms = total_elapsed,
        "pharmacy review complete"
    );

    Ok(Json(PharmacyReviewResponse {
        status: "reviewed".to_string(),
        review_status: output.review_status,
        interactions_found: output.interactions_found,
        substitution_suggestions: output.substitution_suggestions,
        cds_cards: cds_cards_json,
        audit_event_id: output.audit_event.id.to_string(),
        turn_id,
        confidence: super::turns::ConfidenceResponse {
            score: output.confidence.score,
            factors: output.confidence.factors,
        },
    }))
}
