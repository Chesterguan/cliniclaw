pub struct AppState {
    pub fhir_client: cliniclaw_fhir::FhirClient,
    pub policy_engine: cliniclaw_policy::PolicyEngine,
    pub audit_store: cliniclaw_persist::SqliteAuditStore,
    pub ambient_agent: cliniclaw_agents::AmbientDocAgent,
}
