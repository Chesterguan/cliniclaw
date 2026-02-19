use serde::{Deserialize, Serialize};

use crate::client::FhirResource;
use crate::resources::types::{HumanName, Identifier};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Practitioner {
    #[serde(rename = "resourceType")]
    pub resource_type: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Vec<HumanName>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub identifier: Option<Vec<Identifier>>,
}

impl Default for Practitioner {
    fn default() -> Self {
        Self {
            resource_type: "Practitioner".to_string(),
            id: None,
            active: None,
            name: None,
            identifier: None,
        }
    }
}

impl FhirResource for Practitioner {
    fn resource_type() -> &'static str {
        "Practitioner"
    }

    fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }
}
