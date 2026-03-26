/// Integration tests for ClinicClaw API routes.
///
/// Uses axum's tower::ServiceExt::oneshot to send requests directly to the
/// router without binding a TCP port. All dependencies use in-memory/mock
/// implementations so tests are fast, deterministic, and self-contained.

use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use tower::ServiceExt; // for .oneshot()

use cliniclaw_api::{routes, state::AppState};

// ── Test app builder ─────────────────────────────────────────────────────────

/// Build a fully-wired Router for testing.
///
/// Uses:
/// - MockFhirServer with built-in seed data (8 patients, 6 encounters)
/// - MockClaudeCapability (deterministic LLM responses)
/// - in-memory SQLite for audit + workspace stores
/// - PolicyEngine loaded from the actual policy directory (if present)
async fn test_app() -> axum::Router {
    // FHIR backend with seed data
    let fhir = cliniclaw_fhir::MockFhirServer::new();
    fhir.seed_all(cliniclaw_fhir::mock_data::seed_resources()).await;
    let fhir: Arc<dyn cliniclaw_fhir::FhirBackend> = Arc::new(fhir);

    // Mock LLM — returns deterministic, policy-passing responses
    let llm: Arc<dyn cliniclaw_agents::LlmCapability> =
        Arc::new(cliniclaw_agents::MockClaudeCapability::new());

    // Policy engine — load from the sibling cliniclaw-policy crate.
    // Integration tests run with cwd = crates/cliniclaw-api, so the policies
    // directory is at ../cliniclaw-policy/policies relative to cwd.
    // Falls back to a deny-all engine if not found (CI environments).
    let mut policy_engine = cliniclaw_policy::PolicyEngine::new();
    for candidate in &[
        "../cliniclaw-policy/policies",         // from crates/cliniclaw-api (normal)
        "crates/cliniclaw-policy/policies",     // from workspace root (alternate)
    ] {
        let p = std::path::Path::new(candidate);
        if p.exists() {
            policy_engine
                .load_policies_dir(p)
                .expect("failed to load policy files");
            break;
        }
    }

    // In-memory SQLite audit store
    let audit_store = cliniclaw_persist::SqliteAuditStore::new("sqlite::memory:")
        .await
        .expect("failed to create in-memory audit store");

    // Kernel workspace store — shares the audit store's SQLite pool
    let workspace_store: Arc<dyn cliniclaw_kernel::WorkspaceStore> = Arc::new(
        cliniclaw_kernel::SqliteWorkspaceStore::new(audit_store.pool().clone())
            .await
            .expect("failed to create in-memory workspace store"),
    );

    // AmbientDocAgent holds its own LLM reference
    let ambient_agent = cliniclaw_agents::AmbientDocAgent::new(llm.clone());

    // SSE event bus
    let (event_tx, _) = tokio::sync::broadcast::channel::<cliniclaw_kernel::AgentEvent>(256);

    let app_state = Arc::new(AppState {
        fhir,
        llm,
        policy_engine,
        audit_store,
        ambient_agent,
        workspace_store,
        event_tx,
        demo: routes::demo::DemoController::new(),
    });

    routes::router(app_state)
}

// ── Helper ───────────────────────────────────────────────────────────────────

/// Collect the response body as a String.
async fn body_string(body: Body) -> String {
    let bytes = body
        .collect()
        .await
        .expect("failed to collect body")
        .to_bytes();
    String::from_utf8_lossy(&bytes).to_string()
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn health_check() {
    let app = test_app().await;

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp.into_body()).await;
    assert_eq!(body, "ok");
}

#[tokio::test]
async fn worklist_requires_auth() {
    let app = test_app().await;

    // No Authorization header — expect 401
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/worklist?practitioner_id=practitioner-001")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn worklist_returns_patients() {
    let app = test_app().await;

    // practitioner-001 is a participant on all 6 seed encounters
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/worklist?practitioner_id=practitioner-001")
                .header("Authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let body = body_string(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_str(&body).expect("response is not JSON");

    // The response must have an `entries` array and a positive `total`
    assert!(json["entries"].is_array(), "entries field must be an array");
    assert!(
        json["total"].as_u64().unwrap_or(0) > 0,
        "total should be > 0 with seed data; body: {}",
        body_string_ref(&json)
    );
}

/// Render a serde_json::Value back to a string for assertion messages.
fn body_string_ref(v: &serde_json::Value) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

#[tokio::test]
async fn audit_events_requires_filter() {
    let app = test_app().await;

    // No filter params — the route requires at least patient_id or action
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/audit/events")
                .header("Authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn audit_events_with_filter_returns_array() {
    let app = test_app().await;

    // Filter by action — in-memory store has no events yet, so returns []
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/audit/events?action=triage_assess")
                .header("Authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let body = body_string(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_str(&body).expect("response is not JSON");
    assert!(json.is_array(), "audit events response must be a JSON array");
}

#[tokio::test]
async fn audit_chain_verify_empty() {
    let app = test_app().await;

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/audit/chain/verify")
                .header("Authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let body = body_string(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_str(&body).expect("response is not JSON");
    assert_eq!(json["valid"], true, "empty chain must be valid");
}

#[tokio::test]
async fn create_workspace() {
    let app = test_app().await;

    let payload = serde_json::json!({
        "encounter_id": "enc-001",
        "practitioner_id": "practitioner-001"
    });

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workspaces")
                .header("Authorization", "Bearer test-token")
                .header("Content-Type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // The workspace endpoint returns 200 (idempotent) with the workspace object
    assert_eq!(resp.status(), StatusCode::OK);

    let body = body_string(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_str(&body).expect("response is not JSON");
    assert_eq!(json["encounter_id"], "enc-001");
    assert_eq!(json["practitioner_id"], "practitioner-001");
    assert!(json["id"].is_string(), "workspace must have an id");
}

#[tokio::test]
async fn get_workspace_after_create() {
    let app = test_app().await;

    // First create a workspace for enc-003 (use a unique encounter so this test
    // is independent of the create_workspace test above)
    let payload = serde_json::json!({
        "encounter_id": "enc-003",
        "practitioner_id": "practitioner-001"
    });

    let create_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workspaces")
                .header("Authorization", "Bearer test-token")
                .header("Content-Type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(create_resp.status(), StatusCode::OK);
    let create_body = body_string(create_resp.into_body()).await;
    let created: serde_json::Value =
        serde_json::from_str(&create_body).expect("create response is not JSON");
    let ws_id = created["id"].as_str().expect("workspace id must be a string").to_string();

    // Fetch by ID
    let get_resp = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/workspaces/{}", ws_id))
                .header("Authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(get_resp.status(), StatusCode::OK);
    let get_body = body_string(get_resp.into_body()).await;
    let fetched: serde_json::Value =
        serde_json::from_str(&get_body).expect("get response is not JSON");
    assert_eq!(fetched["id"], ws_id);
    assert_eq!(fetched["encounter_id"], "enc-003");
}

#[tokio::test]
async fn triage_with_mock_llm() {
    let app = test_app().await;

    // enc-001 → patient-001 (Sarah Mitchell, active, non-deceased) — safe for triage
    let payload = serde_json::json!({
        "practitioner_id": "practitioner-001",
        "chief_complaint": "Chest pain radiating to left arm, onset 30 minutes ago",
        "vitals_text": "BP 160/95 HR 98 RR 18 SpO2 96% Temp 37.1C",
        "practitioner_role": "physician"
    });

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/encounter/enc-001/triage")
                .header("Authorization", "Bearer test-token")
                .header("Content-Type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    let status = resp.status();
    let body = body_string(resp.into_body()).await;

    assert_eq!(
        status,
        StatusCode::OK,
        "triage should succeed with mock LLM; body: {}",
        body
    );

    let json: serde_json::Value = serde_json::from_str(&body).expect("response is not JSON");
    assert_eq!(json["status"], "created");
    assert!(
        json["triage_level"].as_u64().is_some(),
        "triage_level must be a number"
    );
    assert!(
        json["audit_event_id"].is_string(),
        "audit_event_id must be a string"
    );
}

#[tokio::test]
async fn triage_requires_auth() {
    let app = test_app().await;

    let payload = serde_json::json!({
        "practitioner_id": "practitioner-001",
        "chief_complaint": "Chest pain",
        "vitals_text": "BP 120/80 HR 72"
    });

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/encounter/enc-001/triage")
                .header("Content-Type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn unknown_route_404() {
    let app = test_app().await;

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/nonexistent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
