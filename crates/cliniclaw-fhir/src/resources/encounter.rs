use serde::{Deserialize, Serialize};

use crate::client::FhirResource;
use crate::resources::types::{Coding, Period, Reference};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Encounter {
    #[serde(rename = "resourceType")]
    pub resource_type: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    pub status: String,

    /// FHIR R4 defines class as 1..1, but we keep it optional for client resilience —
    /// real-world FHIR servers sometimes return incomplete resources.
    #[serde(rename = "class", skip_serializing_if = "Option::is_none")]
    pub class_: Option<Coding>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<Reference>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub participant: Option<Vec<EncounterParticipant>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub period: Option<Period>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncounterParticipant {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub individual: Option<Reference>,
}

impl Encounter {
    pub fn new(status: impl Into<String>) -> Self {
        Self {
            resource_type: "Encounter".to_string(),
            id: None,
            status: status.into(),
            class_: None,
            subject: None,
            participant: None,
            period: None,
        }
    }
}

impl FhirResource for Encounter {
    fn resource_type() -> &'static str {
        "Encounter"
    }

    fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }
}
