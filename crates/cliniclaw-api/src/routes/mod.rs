use std::sync::Arc;

use axum::{
    http::StatusCode,
    routing::{get, post},
    Router,
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::state::AppState;

pub mod audit;
pub mod chain;
pub mod events;
pub mod fhir_proxy;
pub mod notes;
pub mod orders;
pub mod prior_auth;
pub mod turns;
pub mod workspace;
pub mod worklist;

// ── Shared constants ────────────────────────────────────────────────────────

/// Maximum allowed transcript length in bytes (50 KB).
pub(crate) const MAX_TRANSCRIPT_BYTES: usize = 50 * 1024;
/// Maximum chief complaint length in bytes (1 KB).
pub(crate) const MAX_CHIEF_COMPLAINT_BYTES: usize = 1024;
/// Maximum number of active medications.
pub(crate) const MAX_ACTIVE_MEDICATIONS: usize = 100;
/// Maximum length of a single medication entry in bytes.
pub(crate) const MAX_MEDICATION_ENTRY_BYTES: usize = 256;

// ── Shared helpers ───────────────────────────────────────────────────────────

/// Encounter statuses that allow agent actions. Agents must not operate on
/// finished, cancelled, or entered-in-error encounters.
const ACTIONABLE_ENCOUNTER_STATUSES: &[&str] = &["in-progress", "planned", "arrived", "triaged", "onleave"];

/// Validates that an encounter status allows agent actions. Returns an error
/// for finished, cancelled, entered-in-error, or unknown statuses.
pub(crate) fn validate_encounter_status(status: &str) -> Result<(), crate::error::ApiError> {
    if ACTIONABLE_ENCOUNTER_STATUSES.contains(&status) {
        Ok(())
    } else {
        Err(crate::error::ApiError::new(
            StatusCode::CONFLICT,
            format!("encounter status '{}' does not allow agent actions", status),
        ))
    }
}

/// Allowed characters in FHIR resource IDs: alphanumeric, hyphen, dot, up to 64 chars.
pub(crate) fn is_valid_fhir_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
}

/// Extract the Bearer token from the Authorization header.
///
/// **WARNING**: This currently only checks that a Bearer token is present —
/// it does NOT validate the token against any store. Before production
/// deployment, integrate with SMART-on-FHIR or a capability token store.
pub(crate) fn extract_bearer_token(
    headers: &axum::http::HeaderMap,
) -> Result<&str, crate::error::ApiError> {
    let header = headers
        .get("authorization")
        .ok_or_else(|| crate::error::ApiError::new(StatusCode::UNAUTHORIZED, "missing Authorization header"))?;
    let value = header
        .to_str()
        .map_err(|_| crate::error::ApiError::new(StatusCode::UNAUTHORIZED, "invalid Authorization header"))?;
    let token = value
        .strip_prefix("Bearer ")
        .ok_or_else(|| crate::error::ApiError::new(StatusCode::UNAUTHORIZED, "expected Bearer token"))?;

    // TODO(production): Validate token against SMART-on-FHIR / capability store.
    // This is a dev-mode passthrough — any non-empty Bearer token is accepted.

    Ok(token)
}

// ── Router ───────────────────────────────────────────────────────────────────

pub fn router(state: Arc<AppState>) -> Router {
    // Permissive CORS for development; tighten for production.
    let cors = CorsLayer::permissive();

    Router::new()
        // Ambient documentation
        .route("/v1/encounter/:id/note", post(notes::generate_note))
        // Intelligent order entry
        .route("/v1/encounter/:id/orders", post(orders::propose_order))
        // Prior authorization
        .route("/v1/encounter/:id/prior-auth", post(prior_auth::assemble_prior_auth))
        // Clinician worklist
        .route("/v1/worklist", get(worklist::get_worklist))
        // Audit trail
        .route("/v1/audit/events", get(audit::list_events))
        .route("/v1/audit/events/:id", get(audit::get_event))
        .route("/v1/audit/chain/verify", get(audit::verify_chain))
        // FHIR resource proxies
        .route("/v1/patients/:id", get(fhir_proxy::get_patient))
        .route("/v1/encounters/:id", get(fhir_proxy::get_encounter))
        // Kernel: workspaces
        .route("/v1/workspaces", post(workspace::create_workspace))
        .route("/v1/workspaces/:id", get(workspace::get_workspace))
        .route("/v1/workspaces/:id/close", post(workspace::close_workspace))
        // Kernel: turns
        .route("/v1/workspaces/:id/turns", get(turns::list_turns))
        .route("/v1/turns/:id", get(turns::get_turn))
        .route("/v1/turns/:id/resolve", post(turns::resolve_turn))
        .route("/v1/turns/:id/replay", post(turns::replay_turn))
        .route("/v1/turns/:id/chain", get(turns::get_turn_chain))
        // Kernel: feedback stats
        .route("/v1/feedback/stats", get(turns::feedback_stats))
        // SSE: real-time agent events
        .route("/v1/events", get(events::event_stream))
        // Health check
        .route("/health", get(health_check))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn health_check() -> &'static str {
    "ok"
}
