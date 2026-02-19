use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bundle<R> {
    #[serde(rename = "resourceType")]
    pub resource_type: String,

    /// FHIR R4 Bundle.type (1..1): searchset, document, transaction, etc.
    #[serde(rename = "type")]
    pub type_: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry: Option<Vec<BundleEntry<R>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleEntry<R> {
    #[serde(rename = "fullUrl", skip_serializing_if = "Option::is_none")]
    pub full_url: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<R>,
}
