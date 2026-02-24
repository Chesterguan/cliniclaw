use serde::{Deserialize, Serialize};

use crate::client::FhirResource;
use crate::resources::types::{HumanName, Identifier};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Patient {
    #[serde(rename = "resourceType")]
    pub resource_type: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Vec<HumanName>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub gender: Option<String>,

    #[serde(rename = "birthDate", skip_serializing_if = "Option::is_none")]
    pub birth_date: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub identifier: Option<Vec<Identifier>>,

    #[serde(rename = "deceasedBoolean", skip_serializing_if = "Option::is_none")]
    pub deceased_boolean: Option<bool>,

    /// FHIR allows deceasedDateTime as an alternative to deceasedBoolean.
    #[serde(rename = "deceasedDateTime", skip_serializing_if = "Option::is_none")]
    pub deceased_date_time: Option<String>,
}

impl Patient {
    /// Returns whether the patient is deceased, considering both
    /// `deceasedBoolean` and `deceasedDateTime` (presence implies deceased).
    pub fn is_deceased(&self) -> Option<bool> {
        if let Some(b) = self.deceased_boolean {
            return Some(b);
        }
        if self.deceased_date_time.is_some() {
            return Some(true);
        }
        None
    }
}

impl Default for Patient {
    fn default() -> Self {
        Self {
            resource_type: "Patient".to_string(),
            id: None,
            active: None,
            name: None,
            gender: None,
            birth_date: None,
            identifier: None,
            deceased_boolean: None,
            deceased_date_time: None,
        }
    }
}

impl FhirResource for Patient {
    fn resource_type() -> &'static str {
        "Patient"
    }

    fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }
}
