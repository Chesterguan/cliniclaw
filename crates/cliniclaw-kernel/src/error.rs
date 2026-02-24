/// Errors originating from the kernel collaboration layer.
#[derive(Debug, thiserror::Error)]
pub enum KernelError {
    #[error("workspace not found: {0}")]
    WorkspaceNotFound(String),

    #[error("turn not found: {0}")]
    TurnNotFound(String),

    #[error("invalid transition: cannot move turn from {from} to {to}")]
    InvalidTransition { from: String, to: String },

    #[error("workspace already closed: {0}")]
    WorkspaceClosed(String),

    #[error("store error: {0}")]
    Store(#[from] sqlx::Error),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("corrupt data: {0}")]
    Corrupt(String),
}
