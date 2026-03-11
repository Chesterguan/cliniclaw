use std::sync::Arc;
use tokio::sync::broadcast;

pub struct AppState {
    pub fhir: Arc<dyn cliniclaw_fhir::FhirBackend>,
    /// Shared LLM capability — used directly by route handlers to construct
    /// per-request agents (OrderEntryAgent, PriorAuthAgent) without storing
    /// a separate agent instance in state for each workflow.
    pub llm: Arc<dyn cliniclaw_agents::LlmCapability>,
    pub policy_engine: cliniclaw_policy::PolicyEngine,
    pub audit_store: cliniclaw_persist::SqliteAuditStore,
    pub ambient_agent: cliniclaw_agents::AmbientDocAgent,
    /// Kernel workspace + turn store — shares the same SQLite pool as audit_store.
    pub workspace_store: Arc<dyn cliniclaw_kernel::WorkspaceStore>,
    /// Broadcast channel for real-time agent events (SSE).
    pub event_tx: broadcast::Sender<cliniclaw_kernel::AgentEvent>,
    /// Demo orchestrator controller — single-patient scripted scenario.
    pub demo: crate::routes::demo::DemoController,
}
