use axum::{
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};

pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = serde_json::json!({
            "error": self.message,
            "status": self.status.as_u16(),
        });
        (self.status, Json(body)).into_response()
    }
}

impl From<cliniclaw_agents::AgentError> for ApiError {
    fn from(err: cliniclaw_agents::AgentError) -> Self {
        match &err {
            cliniclaw_agents::AgentError::PolicyDenied(_) => {
                ApiError::new(StatusCode::FORBIDDEN, err.to_string())
            }
            cliniclaw_agents::AgentError::RequiresApproval { .. } => {
                ApiError::new(StatusCode::FORBIDDEN, err.to_string())
            }
            cliniclaw_agents::AgentError::VerificationFailed(_) => {
                ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, err.to_string())
            }
            cliniclaw_agents::AgentError::Policy(policy_err) => {
                match policy_err {
                    cliniclaw_policy::PolicyError::RoleNotAllowed { .. }
                    | cliniclaw_policy::PolicyError::PopulationExcluded { .. }
                    | cliniclaw_policy::PolicyError::MissingCapability { .. } => {
                        ApiError::new(StatusCode::FORBIDDEN, policy_err.to_string())
                    }
                    cliniclaw_policy::PolicyError::CapabilityExpired { .. }
                    | cliniclaw_policy::PolicyError::CapabilityActorMismatch { .. }
                    | cliniclaw_policy::PolicyError::CapabilityScopeMismatch { .. } => {
                        ApiError::new(StatusCode::UNAUTHORIZED, policy_err.to_string())
                    }
                    _ => {
                        tracing::error!(error = %policy_err, "policy error");
                        ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
                    }
                }
            }
            _ => {
                tracing::error!(error = %err, "agent internal error");
                ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
            }
        }
    }
}

impl From<cliniclaw_fhir::FhirError> for ApiError {
    fn from(err: cliniclaw_fhir::FhirError) -> Self {
        match &err {
            cliniclaw_fhir::FhirError::NotFound { .. } => {
                ApiError::new(StatusCode::NOT_FOUND, err.to_string())
            }
            cliniclaw_fhir::FhirError::Unauthorized => {
                ApiError::new(StatusCode::BAD_GATEWAY, "FHIR server authentication failed")
            }
            _ => {
                tracing::error!(error = %err, "FHIR client error");
                ApiError::new(StatusCode::BAD_GATEWAY, "FHIR server error")
            }
        }
    }
}

impl From<cliniclaw_persist::PersistError> for ApiError {
    fn from(err: cliniclaw_persist::PersistError) -> Self {
        tracing::error!(error = %err, "audit store error");
        ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(err: serde_json::Error) -> Self {
        tracing::error!(error = %err, "serialization error");
        ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
    }
}
