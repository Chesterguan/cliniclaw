use std::sync::Arc;

use axum::{
    extract::{Json, Path, State},
    http::{HeaderMap, StatusCode},
};

use crate::error::ApiError;
use crate::state::AppState;
use super::{extract_bearer_token, is_valid_fhir_id};

// ── Handlers ──────────────────────────────────────────────────────────────────

/// GET /v1/patients/{id}
///
/// Proxy a Patient resource from the FHIR backend.
/// Returns the raw FHIR Patient JSON — no PHI is added by this layer.
pub async fn get_patient(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _bearer = extract_bearer_token(&headers)?;
    // TODO: validate token; restrict to patients the practitioner is authorized to view

    if !is_valid_fhir_id(&id) {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "invalid patient ID format"));
    }

    tracing::info!(patient_id = %id, "fetching patient via FHIR proxy");

    let resource = state
        .fhir
        .read_resource("Patient", &id)
        .await
        .map_err(ApiError::from)?;

    Ok(Json(resource))
}

/// GET /v1/encounters/{id}
///
/// Proxy an Encounter resource from the FHIR backend, enriched with the
/// referenced patient ID for convenience.
pub async fn get_encounter(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _bearer = extract_bearer_token(&headers)?;
    // TODO: validate token; restrict to encounters the practitioner participates in

    if !is_valid_fhir_id(&id) {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "invalid encounter ID format"));
    }

    tracing::info!(encounter_id = %id, "fetching encounter via FHIR proxy");

    let encounter: cliniclaw_fhir::Encounter =
        cliniclaw_fhir::backend::read_typed(state.fhir.as_ref(), &id)
            .await
            .map_err(ApiError::from)?;

    // Enrich response: include the resolved patient ID for client convenience.
    // We do NOT fetch the full patient here — the client should call GET /v1/patients/{id}
    // to retrieve patient details, keeping PHI exposure minimal.
    let patient_id = encounter
        .subject
        .as_ref()
        .and_then(|s| s.reference.as_deref())
        .and_then(|r| r.strip_prefix("Patient/"))
        .map(String::from);

    let mut encounter_json = serde_json::to_value(&encounter)?;

    // Inject a convenience field `_patientId` (underscore prefix = non-FHIR extension).
    if let Some(ref pid) = patient_id {
        encounter_json
            .as_object_mut()
            .map(|obj| obj.insert("_patientId".to_string(), serde_json::json!(pid)));
    }

    Ok(Json(encounter_json))
}
