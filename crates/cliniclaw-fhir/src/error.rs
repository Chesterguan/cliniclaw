use thiserror::Error;

#[derive(Debug, Error)]
pub enum FhirError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("failed to deserialize {resource_type}: {source}")]
    Deserialize {
        resource_type: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("{resource_type}/{id} not found")]
    NotFound { resource_type: String, id: String },

    #[error("unauthorized: check auth token and SMART scopes")]
    Unauthorized,

    #[error("server error {status}: {body}")]
    ServerError { status: u16, body: String },

    #[error("invalid resource: {message}")]
    InvalidResource { message: String },
}
