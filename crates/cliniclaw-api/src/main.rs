use anyhow::Result;
use std::sync::Arc;

mod error;
mod routes;
mod state;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cliniclaw=info,tower_http=info".into()),
        )
        .init();

    let mock_mode = std::env::var("CLINICLAW_MOCK")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    if mock_mode {
        tracing::info!("starting ClinicClaw API server in MOCK mode");
    } else {
        tracing::info!("starting ClinicClaw API server");
    }

    // Configuration from environment
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:cliniclaw.sqlite".to_string());
    let listen_addr = std::env::var("LISTEN_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:3000".to_string());

    // FHIR backend — mock or live
    let fhir: Arc<dyn cliniclaw_fhir::FhirBackend> = if mock_mode {
        let mock = cliniclaw_fhir::MockFhirServer::new();
        mock.seed_all(cliniclaw_fhir::mock_data::seed_resources()).await;
        tracing::info!(count = mock.count().await, "mock FHIR server seeded");
        Arc::new(mock)
    } else {
        let fhir_base_url = std::env::var("FHIR_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:8103/fhir/R4".to_string());
        let fhir_token = std::env::var("FHIR_TOKEN").ok();
        let mut client = cliniclaw_fhir::FhirClient::new(&fhir_base_url);
        if let Some(token) = fhir_token {
            client = client.with_token(token);
        }
        Arc::new(client)
    };

    // LLM capability — mock or live
    let llm: Arc<dyn cliniclaw_agents::LlmCapability> = if mock_mode {
        Arc::new(cliniclaw_agents::MockClaudeCapability::new())
    } else {
        let claude_api_key =
            std::env::var("CLAUDE_API_KEY").expect("CLAUDE_API_KEY must be set");
        let claude_model = std::env::var("CLAUDE_MODEL")
            .unwrap_or_else(|_| "claude-sonnet-4-20250514".to_string());
        Arc::new(cliniclaw_agents::ClaudeCapability::new(
            secrecy::SecretString::from(claude_api_key),
            claude_model,
            4096,
        ))
    };

    // Policy engine — deny-by-default if no rules loaded
    let mut policy_engine = cliniclaw_policy::PolicyEngine::new();
    let policy_dir = std::path::Path::new("crates/cliniclaw-policy/policies");
    if policy_dir.exists() {
        for entry in std::fs::read_dir(policy_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "toml") {
                tracing::info!(path = %path.display(), "loading policy file");
                policy_engine.load_skills_from_file(&path)?;
            }
        }
    } else {
        tracing::warn!(
            dir = %policy_dir.display(),
            "policy directory not found — running deny-all"
        );
    }

    // Audit store
    let audit_store = cliniclaw_persist::SqliteAuditStore::new(&database_url).await?;

    // Kernel workspace store — shares the same SQLite pool as audit_store so
    // workspaces, turns, and audit events all live in the same database file.
    let workspace_store: Arc<dyn cliniclaw_kernel::WorkspaceStore> = Arc::new(
        cliniclaw_kernel::SqliteWorkspaceStore::new(audit_store.pool().clone()).await?,
    );

    // Agent — AmbientDocAgent holds its own Arc<dyn LlmCapability>.
    // Other agents (OrderEntryAgent, PriorAuthAgent) are constructed per-request
    // in the route handlers using state.llm, so they do not live in AppState.
    let ambient_agent = cliniclaw_agents::AmbientDocAgent::new(llm.clone());

    // SSE event bus — 256-slot broadcast channel for real-time agent events.
    // Silently drops events when no SSE subscribers are connected.
    let (event_tx, _) = tokio::sync::broadcast::channel::<cliniclaw_kernel::AgentEvent>(256);

    // Shared state
    let app_state = Arc::new(state::AppState {
        fhir,
        llm,
        policy_engine,
        audit_store,
        ambient_agent,
        workspace_store,
        event_tx,
    });

    // Router
    let app = routes::router(app_state);

    // Listen
    let listener = tokio::net::TcpListener::bind(&listen_addr).await?;
    tracing::info!(addr = %listen_addr, "ClinicClaw API server listening");
    axum::serve(listener, app).await?;

    Ok(())
}
