use std::collections::HashMap;

use crate::capability::Capability;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ActionContext {
    pub action: String,
    pub actor_id: String,
    /// Bare capability names (backward-compatible path)
    pub capabilities: Vec<String>,
    pub resource_type: Option<String>,
    pub properties: HashMap<String, String>,
    /// Structured capability tokens (used by skill-aware evaluation path)
    pub capability_tokens: Vec<Capability>,
    /// The actor's clinical role (e.g. "physician", "nurse")
    pub role: Option<String>,
    /// Patient ID for scope checking
    pub patient_id: Option<String>,
    /// Encounter ID for scope checking
    pub encounter_id: Option<String>,
}

impl ActionContext {
    pub fn new(action: impl Into<String>, actor_id: impl Into<String>) -> Self {
        Self {
            action: action.into(),
            actor_id: actor_id.into(),
            capabilities: Vec::new(),
            resource_type: None,
            properties: HashMap::new(),
            capability_tokens: Vec::new(),
            role: None,
            patient_id: None,
            encounter_id: None,
        }
    }
}
