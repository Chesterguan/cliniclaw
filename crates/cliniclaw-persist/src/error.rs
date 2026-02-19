#[derive(Debug, thiserror::Error)]
pub enum PersistError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("chain integrity violation: expected previous hash '{expected}', got '{actual}'")]
    ChainIntegrity { expected: String, actual: String },

    #[error("corrupt data in audit store: {0}")]
    Corrupt(String),

    #[error("audit store not initialized")]
    NotInitialized,
}
