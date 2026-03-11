#[derive(Debug, Clone, thiserror::Error)]
pub enum PolicyError {
    #[error("policy denied action '{action}' for actor '{actor_id}'")]
    Denied { action: String, actor_id: String },

    #[error("no matching policy rule for action '{action}'")]
    NoMatchingRule { action: String },

    #[error("capability '{capability}' not held by actor '{actor_id}'")]
    MissingCapability { capability: String, actor_id: String },

    #[error("failed to load policy: {0}")]
    LoadError(String),

    #[error("invalid policy rule: {0}")]
    InvalidRule(String),

    #[error("population check excluded: {reason}")]
    PopulationExcluded { reason: String },

    #[error("role '{role}' is not allowed for skill '{skill_id}'")]
    RoleNotAllowed { role: String, skill_id: String },

    #[error("capability '{capability}' expired at {expired_at}")]
    CapabilityExpired {
        capability: String,
        expired_at: chrono::DateTime<chrono::Utc>,
    },

    #[error("capability '{capability}' actor mismatch: expected '{expected}', got '{actual}'")]
    CapabilityActorMismatch {
        capability: String,
        expected: String,
        actual: String,
    },

    #[error("capability '{capability}' scope mismatch: scoped to '{scope}', got '{actual}'")]
    CapabilityScopeMismatch {
        capability: String,
        scope: String,
        actual: String,
    },

    #[error("policy evaluation error: {0}")]
    EvaluationError(String),
}
