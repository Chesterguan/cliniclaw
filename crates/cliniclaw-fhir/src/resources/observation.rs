use serde::{Deserialize, Serialize};

use crate::client::FhirResource;
use crate::resources::types::{CodeableConcept, Quantity, Reference};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    #[serde(rename = "resourceType")]
    pub resource_type: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    pub status: String,

    /// Required by US Core; e.g. "vital-signs", "laboratory", "survey".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<Vec<CodeableConcept>>,

    pub code: CodeableConcept,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<Reference>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub encounter: Option<Reference>,

    #[serde(
        rename = "effectiveDateTime",
        skip_serializing_if = "Option::is_none"
    )]
    pub effective_date_time: Option<String>,

    #[serde(rename = "valueString", skip_serializing_if = "Option::is_none")]
    pub value_string: Option<String>,

    #[serde(rename = "valueQuantity", skip_serializing_if = "Option::is_none")]
    pub value_quantity: Option<Quantity>,
}

impl Observation {
    pub fn new(status: impl Into<String>, code: CodeableConcept) -> Self {
        Self {
            resource_type: "Observation".to_string(),
            id: None,
            status: status.into(),
            category: None,
            code,
            subject: None,
            encounter: None,
            effective_date_time: None,
            value_string: None,
            value_quantity: None,
        }
    }
}

impl FhirResource for Observation {
    fn resource_type() -> &'static str {
        "Observation"
    }

    fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }
}
