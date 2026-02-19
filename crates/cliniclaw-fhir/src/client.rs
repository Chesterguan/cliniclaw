use secrecy::{ExposeSecret, SecretString};
use serde::{de::DeserializeOwned, Serialize};
use tracing::{instrument, trace};

use crate::bundle::Bundle;
use crate::error::FhirError;

pub trait FhirResource: Serialize + DeserializeOwned + Send + Sync {
    fn resource_type() -> &'static str;
    fn id(&self) -> Option<&str>;
}

/// FHIR R4 REST client. Auth token is stored as SecretString
/// and never appears in Debug output.
pub struct FhirClient {
    http: reqwest::Client,
    base_url: String,
    auth_token: Option<SecretString>,
}

impl std::fmt::Debug for FhirClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FhirClient")
            .field("base_url", &self.base_url)
            .field("auth_token", &self.auth_token.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}

impl Clone for FhirClient {
    fn clone(&self) -> Self {
        Self {
            http: self.http.clone(),
            base_url: self.base_url.clone(),
            auth_token: self
                .auth_token
                .as_ref()
                .map(|t| SecretString::from(t.expose_secret().to_owned())),
        }
    }
}

impl FhirClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
            auth_token: None,
        }
    }

    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(SecretString::from(token.into()));
        self
    }

    fn request(&self, method: reqwest::Method, url: &str) -> reqwest::RequestBuilder {
        let builder = self.http.request(method, url);
        if let Some(token) = &self.auth_token {
            builder.bearer_auth(token.expose_secret())
        } else {
            builder
        }
    }

    async fn handle_error_response(
        resource_type: &str,
        id: &str,
        response: reqwest::Response,
    ) -> FhirError {
        let status = response.status();
        match status.as_u16() {
            401 | 403 => FhirError::Unauthorized,
            404 => FhirError::NotFound {
                resource_type: resource_type.to_string(),
                id: id.to_string(),
            },
            _ => {
                // Do NOT include the response body — it may contain PHI
                // from FHIR OperationOutcome resources. Log status only.
                let _body = response.text().await; // consume body, discard
                FhirError::ServerError {
                    status: status.as_u16(),
                    body: format!("FHIR server returned HTTP {}", status.as_u16()),
                }
            }
        }
    }

    #[instrument(skip(self), fields(resource_type = R::resource_type(), id))]
    pub async fn read<R: FhirResource>(&self, id: &str) -> Result<R, FhirError> {
        let url = format!("{}/{}/{}", self.base_url, R::resource_type(), id);
        trace!(url = %url, "FHIR read");

        let response = self
            .request(reqwest::Method::GET, &url)
            .send()
            .await
            .map_err(FhirError::Http)?;

        if response.status().is_success() {
            let bytes = response.bytes().await.map_err(FhirError::Http)?;
            serde_json::from_slice::<R>(&bytes).map_err(|source| FhirError::Deserialize {
                resource_type: R::resource_type().to_string(),
                source,
            })
        } else {
            Err(Self::handle_error_response(R::resource_type(), id, response).await)
        }
    }

    #[instrument(skip(self, resource), fields(resource_type = R::resource_type()))]
    pub async fn create<R: FhirResource>(&self, resource: &R) -> Result<R, FhirError> {
        let url = format!("{}/{}", self.base_url, R::resource_type());
        trace!(url = %url, "FHIR create");

        let response = self
            .request(reqwest::Method::POST, &url)
            .json(resource)
            .send()
            .await
            .map_err(FhirError::Http)?;

        if response.status().is_success() {
            let bytes = response.bytes().await.map_err(FhirError::Http)?;
            serde_json::from_slice::<R>(&bytes).map_err(|source| FhirError::Deserialize {
                resource_type: R::resource_type().to_string(),
                source,
            })
        } else {
            Err(Self::handle_error_response(R::resource_type(), "<new>", response).await)
        }
    }

    #[instrument(skip(self, resource), fields(resource_type = R::resource_type()))]
    pub async fn update<R: FhirResource>(&self, resource: &R) -> Result<R, FhirError> {
        let id = resource.id().ok_or_else(|| FhirError::InvalidResource {
            message: format!(
                "cannot update a {} with no id — create it first",
                R::resource_type()
            ),
        })?;

        let url = format!("{}/{}/{}", self.base_url, R::resource_type(), id);
        trace!(url = %url, "FHIR update");

        let response = self
            .request(reqwest::Method::PUT, &url)
            .json(resource)
            .send()
            .await
            .map_err(FhirError::Http)?;

        if response.status().is_success() {
            let bytes = response.bytes().await.map_err(FhirError::Http)?;
            serde_json::from_slice::<R>(&bytes).map_err(|source| FhirError::Deserialize {
                resource_type: R::resource_type().to_string(),
                source,
            })
        } else {
            Err(Self::handle_error_response(R::resource_type(), id, response).await)
        }
    }

    #[instrument(skip(self), fields(resource_type = R::resource_type()))]
    pub async fn search<R: FhirResource>(
        &self,
        params: &[(&str, &str)],
    ) -> Result<Bundle<R>, FhirError> {
        let url = format!("{}/{}", self.base_url, R::resource_type());
        trace!(url = %url, ?params, "FHIR search");

        let response = self
            .request(reqwest::Method::GET, &url)
            .query(params)
            .send()
            .await
            .map_err(FhirError::Http)?;

        if response.status().is_success() {
            let bytes = response.bytes().await.map_err(FhirError::Http)?;
            serde_json::from_slice::<Bundle<R>>(&bytes).map_err(|source| {
                FhirError::Deserialize {
                    resource_type: format!("Bundle<{}>", R::resource_type()),
                    source,
                }
            })
        } else {
            Err(Self::handle_error_response(R::resource_type(), "<search>", response).await)
        }
    }
}
