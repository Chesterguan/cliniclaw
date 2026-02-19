use serde::{Deserialize, Serialize};

use crate::client::FhirResource;
use crate::resources::types::{Attachment, CodeableConcept, Reference};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticReport {
    #[serde(rename = "resourceType")]
    pub resource_type: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    pub status: String,

    /// FHIR R4 DiagnosticReport.code (1..1 required).
    pub code: CodeableConcept,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<Reference>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub encounter: Option<Reference>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub issued: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub conclusion: Option<String>,

    #[serde(rename = "presentedForm", skip_serializing_if = "Option::is_none")]
    pub presented_form: Option<Vec<Attachment>>,
}

impl DiagnosticReport {
    pub fn new(status: impl Into<String>, code: CodeableConcept) -> Self {
        Self {
            resource_type: "DiagnosticReport".to_string(),
            id: None,
            status: status.into(),
            code,
            subject: None,
            encounter: None,
            issued: None,
            conclusion: None,
            presented_form: None,
        }
    }
}

impl FhirResource for DiagnosticReport {
    fn resource_type() -> &'static str {
        "DiagnosticReport"
    }

    fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }
}
