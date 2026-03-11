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

    // LLM backend selection: mock (default), ollama, claude
    let llm_backend = std::env::var("LLM_BACKEND").unwrap_or_else(|_| {
        // Legacy compat: CLINICLAW_MOCK=true maps to "mock"
        if std::env::var("CLINICLAW_MOCK")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false)
        {
            "mock".to_string()
        } else if std::env::var("CLAUDE_API_KEY").is_ok() {
            "claude".to_string()
        } else {
            "mock".to_string()
        }
    });

    let mock_mode = llm_backend == "mock";

    tracing::info!(llm_backend = %llm_backend, "starting ClinicClaw API server");

    // Configuration from environment
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:cliniclaw.sqlite".to_string());
    let listen_addr = std::env::var("LISTEN_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:3000".to_string());

    // FHIR backend — mock (with optional Synthea data) or live
    let fhir: Arc<dyn cliniclaw_fhir::FhirBackend> = if mock_mode || llm_backend == "ollama" {
        let mock = cliniclaw_fhir::MockFhirServer::new();

        // Check for Synthea data directory first, fall back to built-in seed data
        let synthea_dir = std::env::var("SYNTHEA_DIR").ok().map(std::path::PathBuf::from);
        if let Some(ref dir) = synthea_dir {
            if dir.exists() {
                let result = cliniclaw_fhir::synthea::load_synthea_dir(&mock, dir).await?;
                tracing::info!(%result, dir = %dir.display(), "Synthea data loaded");
                // Activate the most recent encounter per patient so simulation can run
                let activated = cliniclaw_fhir::synthea::activate_recent_encounters(&mock).await?;
                tracing::info!(activated, "encounters set to in-progress for simulation");
            } else {
                tracing::warn!(dir = %dir.display(), "SYNTHEA_DIR not found, using built-in seed data");
                mock.seed_all(cliniclaw_fhir::mock_data::seed_resources()).await;
            }
        } else {
            mock.seed_all(cliniclaw_fhir::mock_data::seed_resources()).await;
        }

        tracing::info!(count = mock.count().await, "mock FHIR server seeded");
        Arc::new(mock)
    } else {
        let fhir_base_url = std::env::var("FHIR_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:8103/fhir/R4".to_string());
        let fhir_token = std::env::var("FHIR_TOKEN").ok();
        let mut client = cliniclaw_fhir::FhirClient::new(&fhir_base_url)?;
        if let Some(token) = fhir_token {
            client = client.with_token(token);
        }
        Arc::new(client)
    };

    // LLM capability — mock, ollama, or claude
    let llm: Arc<dyn cliniclaw_agents::LlmCapability> = match llm_backend.as_str() {
        "ollama" => {
            let model = std::env::var("OLLAMA_MODEL")
                .unwrap_or_else(|_| "mistral-small".to_string());
            let base_url = std::env::var("OLLAMA_URL")
                .unwrap_or_else(|_| "http://localhost:11434".to_string());
            tracing::info!(model = %model, url = %base_url, "using Ollama LLM backend");
            Arc::new(cliniclaw_agents::OllamaCapability::with_base_url(base_url, model))
        }
        "claude" => {
            let claude_api_key = std::env::var("CLAUDE_API_KEY")
                .map_err(|_| anyhow::anyhow!("CLAUDE_API_KEY environment variable must be set"))?;
            let claude_model = std::env::var("CLAUDE_MODEL")
                .unwrap_or_else(|_| "claude-sonnet-4-20250514".to_string());
            tracing::info!(model = %claude_model, "using Claude API LLM backend");
            Arc::new(cliniclaw_agents::ClaudeCapability::new(
                secrecy::SecretString::from(claude_api_key),
                claude_model,
                4096,
            )?)
        }
        _ => {
            tracing::info!("using Mock LLM backend");
            Arc::new(cliniclaw_agents::MockClaudeCapability::new())
        }
    };

    // Policy engine — loads .rego rules + .toml skill metadata from policies dir
    let mut policy_engine = cliniclaw_policy::PolicyEngine::new();
    let policy_dir = std::path::Path::new("crates/cliniclaw-policy/policies");
    if policy_dir.exists() {
        policy_engine.load_policies_dir(policy_dir)?;
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

    // SSE event bus — 1024-slot broadcast channel for real-time agent events.
    // Sized for hospital simulation (6+ concurrent patient pathways).
    let (event_tx, _) = tokio::sync::broadcast::channel::<cliniclaw_kernel::AgentEvent>(1024);

    // Shared state
    let app_state = Arc::new(state::AppState {
        fhir,
        llm,
        policy_engine,
        audit_store,
        ambient_agent,
        workspace_store,
        event_tx,
        demo: routes::demo::DemoController::new(),
    });

    // Router
    let app = routes::router(app_state);

    // Listen
    let listener = tokio::net::TcpListener::bind(&listen_addr).await?;
    tracing::info!(addr = %listen_addr, "ClinicClaw API server listening");
    axum::serve(listener, app).await?;

    Ok(())
}
