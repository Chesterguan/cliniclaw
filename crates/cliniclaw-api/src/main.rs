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

    tracing::info!("starting ClinicClaw API server");

    // Configuration from environment
    let fhir_base_url = std::env::var("FHIR_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:8103/fhir/R4".to_string());
    let fhir_token = std::env::var("FHIR_TOKEN").ok();
    let claude_api_key =
        std::env::var("CLAUDE_API_KEY").expect("CLAUDE_API_KEY must be set");
    let claude_model = std::env::var("CLAUDE_MODEL")
        .unwrap_or_else(|_| "claude-sonnet-4-20250514".to_string());
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:cliniclaw.sqlite".to_string());
    let listen_addr = std::env::var("LISTEN_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:3000".to_string());

    // FHIR client
    let mut fhir_client = cliniclaw_fhir::FhirClient::new(&fhir_base_url);
    if let Some(token) = fhir_token {
        fhir_client = fhir_client.with_token(token);
    }

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

    // Claude capability
    let claude_capability = cliniclaw_agents::ClaudeCapability::new(
        secrecy::SecretString::from(claude_api_key),
        claude_model,
        4096,
    );

    // Agent
    let ambient_agent = cliniclaw_agents::AmbientDocAgent::new(claude_capability);

    // Shared state
    let app_state = Arc::new(state::AppState {
        fhir_client,
        policy_engine,
        audit_store,
        ambient_agent,
    });

    // Router
    let app = routes::router(app_state);

    // Listen
    let listener = tokio::net::TcpListener::bind(&listen_addr).await?;
    tracing::info!(addr = %listen_addr, "ClinicClaw API server listening");
    axum::serve(listener, app).await?;

    Ok(())
}
