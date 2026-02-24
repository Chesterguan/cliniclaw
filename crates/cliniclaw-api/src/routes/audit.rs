use std::sync::Arc;

use axum::{
    extract::{Json, Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use serde::Deserialize;
use uuid::Uuid;

use crate::error::ApiError;
use crate::state::AppState;
use super::extract_bearer_token;

// ── Query params ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ListEventsQuery {
    /// Filter by patient ID (optional)
    pub patient_id: Option<String>,
    /// Filter by action name (optional)
    pub action: Option<String>,
}

// ── Response types ────────────────────────────────────────────────────────────

#[derive(Debug, serde::Serialize)]
pub struct VerifyChainResponse {
    pub valid: bool,
    pub message: String,
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// GET /v1/audit/events
///
/// List audit events, optionally filtered by patient_id or action.
/// When both filters are provided, patient_id takes precedence.
pub async fn list_events(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<ListEventsQuery>,
) -> Result<Json<Vec<cliniclaw_persist::AuditEvent>>, ApiError> {
    let _bearer = extract_bearer_token(&headers)?;
    // TODO: validate token; restrict patient_id results to authorized practitioners

    let events = if let Some(ref patient_id) = query.patient_id {
        state
            .audit_store
            .get_by_patient(patient_id)
            .await
            .map_err(ApiError::from)?
    } else if let Some(ref action) = query.action {
        state
            .audit_store
            .get_by_action(action)
            .await
            .map_err(ApiError::from)?
    } else {
        // No filter — return all events by fetching with an empty action match.
        // We use get_by_action with a wildcard-style pattern that won't match anything
        // to avoid accidentally leaking all events. An explicit filter is required.
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "must provide at least one filter: patient_id or action",
        ));
    };

    Ok(Json(events))
}

/// GET /v1/audit/events/{id}
///
/// Retrieve a single audit event by its UUID.
pub async fn get_event(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<cliniclaw_persist::AuditEvent>, ApiError> {
    let _bearer = extract_bearer_token(&headers)?;

    let event_id = Uuid::parse_str(&id)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid audit event ID format — expected UUID"))?;

    let event = state
        .audit_store
        .get(event_id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "audit event not found"))?;

    Ok(Json(event))
}

/// GET /v1/audit/chain/verify
///
/// Verify the integrity of the entire audit chain.
/// Returns 200 with `valid: true` if every event hash and chain link is intact.
pub async fn verify_chain(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<VerifyChainResponse>, ApiError> {
    let _bearer = extract_bearer_token(&headers)?;

    let valid = state
        .audit_store
        .verify_chain()
        .await
        .map_err(ApiError::from)?;

    let message = if valid {
        "audit chain integrity verified — all hashes valid".to_string()
    } else {
        "audit chain integrity FAILED — possible tampering detected".to_string()
    };

    tracing::info!(valid = valid, "audit chain verification complete");

    Ok(Json(VerifyChainResponse { valid, message }))
}
