use serde::{Deserialize, Serialize};

use crate::client::FhirResource;
use crate::resources::types::{CodeableConcept, DosageInstruction, Reference};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MedicationRequest {
    #[serde(rename = "resourceType")]
    pub resource_type: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    pub status: String,

    pub intent: String,

    #[serde(rename = "medicationCodeableConcept", skip_serializing_if = "Option::is_none")]
    pub medication_codeable_concept: Option<CodeableConcept>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<Reference>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub encounter: Option<Reference>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub requester: Option<Reference>,

    #[serde(rename = "dosageInstruction", skip_serializing_if = "Option::is_none")]
    pub dosage_instruction: Option<Vec<DosageInstruction>>,
}

impl MedicationRequest {
    pub fn new(status: impl Into<String>, intent: impl Into<String>) -> Self {
        Self {
            resource_type: "MedicationRequest".to_string(),
            id: None,
            status: status.into(),
            intent: intent.into(),
            medication_codeable_concept: None,
            subject: None,
            encounter: None,
            requester: None,
            dosage_instruction: None,
        }
    }
}

impl FhirResource for MedicationRequest {
    fn resource_type() -> &'static str {
        "MedicationRequest"
    }

    fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }
}
