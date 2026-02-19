#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("policy denied: {0}")]
    PolicyDenied(String),

    #[error("policy requires approval for action '{action}'")]
    RequiresApproval { action: String },

    #[error("missing capability: {0}")]
    MissingCapability(String),

    #[error("Claude API error: {0}")]
    ClaudeApi(String),

    #[error("Claude API HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("verification failed: {0}")]
    VerificationFailed(String),

    #[error("FHIR error: {0}")]
    Fhir(#[from] cliniclaw_fhir::FhirError),

    #[error("policy error: {0}")]
    Policy(#[from] cliniclaw_policy::PolicyError),

    #[error("persistence error: {0}")]
    Persist(#[from] cliniclaw_persist::PersistError),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
