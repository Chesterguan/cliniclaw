use serde::{Deserialize, Serialize};

use crate::client::FhirResource;
use crate::resources::types::{CodeableConcept, Reference};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceRequest {
    #[serde(rename = "resourceType")]
    pub resource_type: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    pub status: String,

    pub intent: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<CodeableConcept>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<Reference>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub encounter: Option<Reference>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub requester: Option<Reference>,
}

impl ServiceRequest {
    pub fn new(status: impl Into<String>, intent: impl Into<String>) -> Self {
        Self {
            resource_type: "ServiceRequest".to_string(),
            id: None,
            status: status.into(),
            intent: intent.into(),
            code: None,
            subject: None,
            encounter: None,
            requester: None,
        }
    }
}

impl FhirResource for ServiceRequest {
    fn resource_type() -> &'static str {
        "ServiceRequest"
    }

    fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }
}
