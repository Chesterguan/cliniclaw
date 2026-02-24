use std::sync::Arc;

use axum::{
    extract::{Json, Path, State},
    http::{HeaderMap, StatusCode},
};

use crate::error::ApiError;
use crate::state::AppState;
use super::{extract_bearer_token, is_valid_fhir_id};

#[derive(Debug, serde::Deserialize)]
pub struct CreateWorkspaceRequest {
    pub encounter_id: String,
    pub practitioner_id: String,
}

#[derive(Debug, serde::Serialize)]
pub struct WorkspaceResponse {
    pub id: String,
    pub encounter_id: String,
    pub practitioner_id: String,
    pub created_at: String,
    pub closed_at: Option<String>,
    pub pending_turns: usize,
}

pub async fn create_workspace(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<CreateWorkspaceRequest>,
) -> Result<Json<WorkspaceResponse>, ApiError> {
    let _bearer = extract_bearer_token(&headers)?;
    if !is_valid_fhir_id(&body.encounter_id) {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "invalid encounter ID"));
    }
    if !is_valid_fhir_id(&body.practitioner_id) {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "invalid practitioner ID"));
    }

    // Return the existing open workspace if one already exists for this encounter,
    // rather than creating a duplicate. This makes the endpoint idempotent for callers
    // that may retry on transient errors.
    if let Some(existing) = state
        .workspace_store
        .find_workspace_by_encounter(&body.encounter_id)
        .await
        .map_err(ApiError::from)?
    {
        let pending = state
            .workspace_store
            .list_turns(&existing.id, Some(cliniclaw_kernel::TurnStatus::Pending))
            .await
            .map_err(ApiError::from)?;
        return Ok(Json(WorkspaceResponse {
            id: existing.id,
            encounter_id: existing.encounter_id,
            practitioner_id: existing.practitioner_id,
            created_at: existing.created_at.to_rfc3339(),
            closed_at: existing.closed_at.map(|t| t.to_rfc3339()),
            pending_turns: pending.len(),
        }));
    }

    let ws = state
        .workspace_store
        .create_workspace(&body.encounter_id, &body.practitioner_id)
        .await
        .map_err(ApiError::from)?;

    Ok(Json(WorkspaceResponse {
        id: ws.id,
        encounter_id: ws.encounter_id,
        practitioner_id: ws.practitioner_id,
        created_at: ws.created_at.to_rfc3339(),
        closed_at: None,
        pending_turns: 0,
    }))
}

pub async fn get_workspace(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<WorkspaceResponse>, ApiError> {
    let _bearer = extract_bearer_token(&headers)?;
    let ws = state
        .workspace_store
        .get_workspace(&id)
        .await
        .map_err(ApiError::from)?;
    let pending = state
        .workspace_store
        .list_turns(&ws.id, Some(cliniclaw_kernel::TurnStatus::Pending))
        .await
        .map_err(ApiError::from)?;

    Ok(Json(WorkspaceResponse {
        id: ws.id,
        encounter_id: ws.encounter_id,
        practitioner_id: ws.practitioner_id,
        created_at: ws.created_at.to_rfc3339(),
        closed_at: ws.closed_at.map(|t| t.to_rfc3339()),
        pending_turns: pending.len(),
    }))
}

pub async fn close_workspace(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<WorkspaceResponse>, ApiError> {
    let _bearer = extract_bearer_token(&headers)?;
    let ws = state
        .workspace_store
        .close_workspace(&id)
        .await
        .map_err(ApiError::from)?;

    Ok(Json(WorkspaceResponse {
        id: ws.id,
        encounter_id: ws.encounter_id,
        practitioner_id: ws.practitioner_id,
        created_at: ws.created_at.to_rfc3339(),
        closed_at: ws.closed_at.map(|t| t.to_rfc3339()),
        pending_turns: 0,
    }))
}
