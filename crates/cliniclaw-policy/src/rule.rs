use std::collections::HashMap;

use crate::decision::PolicyDecision;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PolicyRule {
    pub name: String,
    pub action: String,
    pub decision: PolicyDecision,
    #[serde(default)]
    pub required_capabilities: Vec<String>,
    #[serde(default)]
    pub conditions: HashMap<String, String>,
    #[serde(default)]
    pub priority: i32,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct PolicyFile {
    #[serde(rename = "rule")]
    pub rules: Vec<PolicyRule>,
}
