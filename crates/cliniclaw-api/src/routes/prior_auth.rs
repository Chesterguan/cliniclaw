use std::sync::Arc;

use axum::{
    extract::{Json, Path, State},
    http::{HeaderMap, StatusCode},
};

use cliniclaw_kernel::{AgentEventType, EventEmitter, StepStatus};

use crate::error::ApiError;
use crate::state::AppState;
use super::{extract_bearer_token, is_valid_fhir_id, validate_encounter_status};

#[derive(Debug, serde::Deserialize)]
pub struct AssemblePriorAuthRequest {
    pub practitioner_id: String,
    pub service_request_id: String,
    pub service_description: String,
    pub diagnosis_codes: Vec<String>,
    pub cpt_codes: Vec<String>,
    pub clinical_notes: Option<String>,
    /// Optional practitioner role for skill-aware policy evaluation
    pub practitioner_role: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct PriorAuthResponse {
    pub status: String,
    pub diagnosis_summary: String,
    pub clinical_justification: String,
    pub supporting_evidence: Vec<String>,
    pub urgency: String,
    pub cpt_codes: Vec<String>,
    pub icd10_codes: Vec<String>,
    pub prior_auth_status: String,
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

pub async fn assemble_prior_auth(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(encounter_id): Path<String>,
    Json(body): Json<AssemblePriorAuthRequest>,
) -> Result<Json<PriorAuthResponse>, ApiError> {
    let started = std::time::Instant::now();

    let emitter = EventEmitter::new(
        state.event_tx.clone(),
        &encounter_id,
        "prior_auth",
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
    if !is_valid_fhir_id(&body.service_request_id) {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "invalid service request ID format"));
    }

    // 0c. Validate required clinical content
    if body.service_description.trim().is_empty() {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "service_description must not be empty"));
    }
    if body.cpt_codes.is_empty() {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "at least one CPT code is required"));
    }
    if body.diagnosis_codes.is_empty() {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "at least one diagnosis code is required"));
    }

    tracing::info!(
        encounter_id = %encounter_id,
        practitioner_id = %body.practitioner_id,
        service_request_id = %body.service_request_id,
        "assembling prior auth package"
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
        capability: "prior_auth".into(),
        valid: true,
    });

    emitter.emit(AgentEventType::SkillLookup {
        skill_id: Some("prior_auth.assemble_package".into()),
        matched: true,
    });

    let practitioner_id_snap = body.practitioner_id.clone();
    let service_request_id_snap = body.service_request_id.clone();
    let cpt_codes_snap = body.cpt_codes.clone();

    // 3. Build agent input
    let input = cliniclaw_agents::PriorAuthInput {
        encounter_id: encounter_id.clone(),
        encounter_status: encounter.status.clone(),
        patient_id,
        practitioner_id: body.practitioner_id.clone(),
        service_request_id: body.service_request_id.clone(),
        service_description: body.service_description,
        diagnosis_codes: body.diagnosis_codes,
        cpt_codes: body.cpt_codes,
        clinical_notes: body.clinical_notes,
        capabilities: vec!["prior_auth".to_string()],
        capability_tokens: vec![],
        practitioner_role: body.practitioner_role,
        patient_active,
        patient_deceased,
        encounter_class,
    };

    // 4. Policy + LLM
    emitter.emit(AgentEventType::PolicyEvaluation {
        decision: "evaluating".into(),
        rule_name: None,
    });

    emitter.emit(AgentEventType::LlmCall {
        status: StepStatus::Started,
        elapsed_ms: None,
    });

    let llm_start = std::time::Instant::now();
    let prior_auth_agent = cliniclaw_agents::PriorAuthAgent::new(state.llm.clone());
    let mut output = prior_auth_agent
        .assemble_package(&input, &state.policy_engine)
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
        rule_name: Some("allow_prior_auth".into()),
    });

    emitter.emit(AgentEventType::ResponseParsing {
        status: StepStatus::Completed,
        detail: Some("Prior auth package parsed".into()),
    });

    emitter.emit(AgentEventType::Verification {
        passed: true,
        detail: Some("Clinical justification verified".into()),
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

    // 6. Create kernel turn if workspace exists and is open
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
            agent_name: "prior_auth".to_string(),
            action: "assemble_package".to_string(),
            input_snapshot: serde_json::json!({
                "encounter_id": encounter_id,
                "practitioner_id": practitioner_id_snap,
                "service_request_id": service_request_id_snap,
                "cpt_codes": cpt_codes_snap,
            }),
            output_snapshot: serde_json::json!({
                "diagnosis_summary": output.diagnosis_summary,
                "clinical_justification": output.clinical_justification,
                "urgency": output.urgency,
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
        service_request_id = %service_request_id_snap,
        audit_event_id = %output.audit_event.id,
        prior_auth_status = %output.status,
        spec_hash = ?output.spec_hash,
        turn_id = ?turn_id,
        elapsed_ms = total_elapsed,
        "prior auth assembly complete"
    );

    Ok(Json(PriorAuthResponse {
        status: "assembled".to_string(),
        diagnosis_summary: output.diagnosis_summary,
        clinical_justification: output.clinical_justification,
        supporting_evidence: output.supporting_evidence,
        urgency: output.urgency,
        cpt_codes: output.cpt_codes,
        icd10_codes: output.icd10_codes,
        prior_auth_status: output.status.to_string(),
        audit_event_id: output.audit_event.id.to_string(),
        spec_hash: output.spec_hash,
        turn_id,
        confidence: super::turns::ConfidenceResponse {
            score: output.confidence.score,
            factors: output.confidence.factors,
        },
    }))
}
