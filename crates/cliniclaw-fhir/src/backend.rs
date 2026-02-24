use async_trait::async_trait;
use serde::de::DeserializeOwned;

use crate::bundle::Bundle;
use crate::client::FhirResource;
use crate::error::FhirError;

/// Object-safe async interface for any FHIR R4 backend.
///
/// Methods operate on `serde_json::Value` to preserve object-safety.
/// Typed access is provided by the free-standing helpers below
/// (`read_typed`, `create_typed`, `search_typed`).
#[async_trait]
pub trait FhirBackend: Send + Sync {
    async fn read_resource(
        &self,
        resource_type: &str,
        id: &str,
    ) -> Result<serde_json::Value, FhirError>;

    async fn create_resource(
        &self,
        resource_type: &str,
        resource: &serde_json::Value,
    ) -> Result<serde_json::Value, FhirError>;

    async fn update_resource(
        &self,
        resource_type: &str,
        id: &str,
        resource: &serde_json::Value,
    ) -> Result<serde_json::Value, FhirError>;

    async fn search_resources(
        &self,
        resource_type: &str,
        params: &[(&str, &str)],
    ) -> Result<serde_json::Value, FhirError>;
}

/// Read a typed FHIR resource from any backend.
pub async fn read_typed<R: FhirResource>(
    backend: &dyn FhirBackend,
    id: &str,
) -> Result<R, FhirError> {
    let val = backend.read_resource(R::resource_type(), id).await?;
    serde_json::from_value(val).map_err(|source| FhirError::Deserialize {
        resource_type: R::resource_type().to_string(),
        source,
    })
}

/// Create a typed FHIR resource on any backend.
pub async fn create_typed<R: FhirResource>(
    backend: &dyn FhirBackend,
    resource: &R,
) -> Result<R, FhirError> {
    let val = serde_json::to_value(resource).map_err(|e| FhirError::InvalidResource {
        message: format!("serialization failed: {e}"),
    })?;
    let result = backend.create_resource(R::resource_type(), &val).await?;
    serde_json::from_value(result).map_err(|source| FhirError::Deserialize {
        resource_type: R::resource_type().to_string(),
        source,
    })
}

/// Search typed FHIR resources on any backend.
pub async fn search_typed<R: FhirResource + DeserializeOwned>(
    backend: &dyn FhirBackend,
    params: &[(&str, &str)],
) -> Result<Bundle<R>, FhirError> {
    let val = backend
        .search_resources(R::resource_type(), params)
        .await?;
    serde_json::from_value(val).map_err(|source| FhirError::Deserialize {
        resource_type: format!("Bundle<{}>", R::resource_type()),
        source,
    })
}
