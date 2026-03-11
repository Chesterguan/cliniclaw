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
pub mod demo;
pub mod discharge;
pub mod events;
pub mod fhir_proxy;
pub mod lab_review;
pub mod notes;
pub mod nurse_assess;
pub mod orders;
pub mod pharmacy;
pub mod prior_auth;
pub mod simulate;
pub mod simulate_dynamic;
pub mod triage;
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
    // Permissive CORS in dev/mock mode; restricted in production.
    let cors = if std::env::var("CLINICLAW_MOCK")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false)
    {
        CorsLayer::permissive()
    } else {
        let origins = std::env::var("CORS_ORIGINS")
            .unwrap_or_else(|_| "http://localhost:3001".to_string());
        CorsLayer::new()
            .allow_origin(tower_http::cors::AllowOrigin::list(
                origins
                    .split(',')
                    .filter_map(|s| s.trim().parse().ok()),
            ))
            .allow_methods([
                axum::http::Method::GET,
                axum::http::Method::POST,
                axum::http::Method::PUT,
                axum::http::Method::DELETE,
                axum::http::Method::OPTIONS,
            ])
            .allow_headers([
                axum::http::header::CONTENT_TYPE,
                axum::http::header::AUTHORIZATION,
            ])
    };

    Router::new()
        // Ambient documentation
        .route("/v1/encounter/:id/note", post(notes::generate_note))
        // Intelligent order entry
        .route("/v1/encounter/:id/orders", post(orders::propose_order))
        // Prior authorization
        .route("/v1/encounter/:id/prior-auth", post(prior_auth::assemble_prior_auth))
        // Triage assessment
        .route("/v1/encounter/:id/triage", post(triage::triage_handler))
        // Lab review and interpretation
        .route("/v1/encounter/:id/lab-review", post(lab_review::lab_review_handler))
        // Discharge planning
        .route("/v1/encounter/:id/discharge-plan", post(discharge::discharge_plan_handler))
        // Nursing assessment
        .route("/v1/encounter/:id/nurse-assess", post(nurse_assess::nurse_assess_handler))
        // Pharmacy review (advisory only — no FHIR write)
        .route("/v1/encounter/:id/pharmacy-review", post(pharmacy::pharmacy_review_handler))
        // Multi-pathway simulation orchestrator
        .route("/v1/simulate", post(simulate::run_simulation))
        // Demo: scripted single-patient chest pain scenario
        .route("/v1/demo/state", get(demo::get_demo_state))
        .route("/v1/demo/start", post(demo::start_demo))
        .route("/v1/demo/approve", post(demo::approve_demo))
        .route("/v1/demo/reset", post(demo::reset_demo))
        .route("/v1/simulate/dynamic", post(simulate_dynamic::run_dynamic_simulation))
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
