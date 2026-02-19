use std::sync::Arc;

use axum::{
    extract::{Json, Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Router,
};
use tower_http::trace::TraceLayer;

use crate::error::ApiError;
use crate::state::AppState;

/// Maximum allowed transcript length in bytes (50 KB).
const MAX_TRANSCRIPT_BYTES: usize = 50 * 1024;

/// Allowed characters in FHIR resource IDs: alphanumeric, hyphen, dot, up to 64 chars.
fn is_valid_fhir_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
}

#[derive(Debug, serde::Deserialize)]
pub struct GenerateNoteRequest {
    pub practitioner_id: String,
    pub transcript: String,
    pub chief_complaint: Option<String>,
    #[serde(default)]
    pub active_medications: Vec<String>,
    /// Optional practitioner role for skill-aware policy evaluation
    pub practitioner_role: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct GenerateNoteResponse {
    pub status: String,
    pub report: serde_json::Value,
    pub audit_event_id: String,
    /// SHA-256 hash of the matched skill spec (if skill-aware evaluation was used)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spec_hash: Option<String>,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/v1/encounter/{id}/note", post(generate_note))
        .route("/health", get(health_check))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn health_check() -> &'static str {
    "ok"
}

/// Extract and validate the Bearer token from the Authorization header.
fn extract_bearer_token(headers: &HeaderMap) -> Result<&str, ApiError> {
    let header = headers
        .get("authorization")
        .ok_or_else(|| ApiError::new(StatusCode::UNAUTHORIZED, "missing Authorization header"))?;
    let value = header
        .to_str()
        .map_err(|_| ApiError::new(StatusCode::UNAUTHORIZED, "invalid Authorization header"))?;
    value
        .strip_prefix("Bearer ")
        .ok_or_else(|| ApiError::new(StatusCode::UNAUTHORIZED, "expected Bearer token"))
}

async fn generate_note(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(encounter_id): Path<String>,
    Json(body): Json<GenerateNoteRequest>,
) -> Result<Json<GenerateNoteResponse>, ApiError> {
    // 0a. Authenticate — require a Bearer token
    let _bearer = extract_bearer_token(&headers)?;
    // TODO: validate token against SMART-on-FHIR / API key store,
    //       extract verified practitioner_id from the token claims
    //       instead of trusting body.practitioner_id

    // 0b. Validate encounter_id format (defense against path traversal)
    if !is_valid_fhir_id(&encounter_id) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid encounter ID format",
        ));
    }

    // 0c. Validate transcript — reject empty or oversized
    if body.transcript.trim().is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "transcript must not be empty",
        ));
    }
    if body.transcript.len() > MAX_TRANSCRIPT_BYTES {
        return Err(ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("transcript exceeds maximum size of {} bytes", MAX_TRANSCRIPT_BYTES),
        ));
    }

    tracing::info!(
        encounter_id = %encounter_id,
        practitioner_id = %body.practitioner_id,
        "generating note for encounter"
    );

    // 1. Fetch encounter from FHIR
    let encounter: cliniclaw_fhir::Encounter = state
        .fhir_client
        .read::<cliniclaw_fhir::Encounter>(&encounter_id)
        .await
        .map_err(ApiError::from)?;

    // H5: Fail if encounter has no valid Patient/ subject reference
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
    let patient = state
        .fhir_client
        .read::<cliniclaw_fhir::Patient>(&patient_id)
        .await
        .map_err(|e| {
            tracing::warn!(patient_id = %patient_id, error = %e, "failed to fetch patient for population gating");
            ApiError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "unable to verify patient status — cannot proceed",
            )
        })?;
    let patient_active = patient.active.unwrap_or(true);
    let patient_deceased = patient.deceased;

    let encounter_class = encounter
        .class_
        .as_ref()
        .and_then(|c| c.code.clone());

    // 3. Build agent input with skill-aware context
    let input = cliniclaw_agents::AmbientDocInput {
        encounter_id: encounter_id.clone(),
        encounter_status: encounter.status.clone(),
        patient_id,
        practitioner_id: body.practitioner_id.clone(),
        transcript: body.transcript,
        chief_complaint: body.chief_complaint,
        active_medications: body.active_medications,
        capabilities: vec!["note_generation".to_string()],
        capability_tokens: vec![], // TODO: populate from SMART-on-FHIR token
        practitioner_role: body.practitioner_role,
        patient_active,
        patient_deceased,
        encounter_class,
    };

    // 4. Get latest audit hash for chain linking
    let previous_hash = state
        .audit_store
        .latest_hash()
        .await
        .map_err(ApiError::from)?;

    // 5. Run the agent (policy → Claude → verify → audit event)
    let output = state
        .ambient_agent
        .generate_note(&input, &state.policy_engine, &previous_hash)
        .await
        .map_err(ApiError::from)?;

    // 6. Persist audit event (atomic check-and-append in SqliteAuditStore)
    state
        .audit_store
        .append(&output.audit_event)
        .await
        .map_err(ApiError::from)?;

    // 7. Write DiagnosticReport to FHIR
    let created_report = state
        .fhir_client
        .create(&output.report)
        .await
        .map_err(ApiError::from)?;

    let report_json = serde_json::to_value(&created_report)?;

    tracing::info!(
        encounter_id = %encounter_id,
        audit_event_id = %output.audit_event.id,
        spec_hash = ?output.spec_hash,
        "note generation complete"
    );

    Ok(Json(GenerateNoteResponse {
        status: "created".to_string(),
        report: report_json,
        audit_event_id: output.audit_event.id.to_string(),
        spec_hash: output.spec_hash,
    }))
}
