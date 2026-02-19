use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AuditEvent {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub actor_id: String,
    pub patient_id: Option<String>,
    pub action: String,
    pub policy_decision: String,
    pub input_hash: String,
    pub output_hash: String,
    pub previous_hash: String,
    pub event_hash: String,
    pub metadata: Option<serde_json::Value>,
}

impl AuditEvent {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        actor_id: impl Into<String>,
        patient_id: Option<String>,
        action: impl Into<String>,
        policy_decision: impl Into<String>,
        input_hash: impl Into<String>,
        output_hash: impl Into<String>,
        previous_hash: impl Into<String>,
    ) -> Self {
        let id = Uuid::new_v4();
        let timestamp = Utc::now();
        let actor_id = actor_id.into();
        let action = action.into();
        let policy_decision = policy_decision.into();
        let input_hash = input_hash.into();
        let output_hash = output_hash.into();
        let previous_hash = previous_hash.into();

        let event_hash = Self::compute_hash(
            &id, &timestamp, &actor_id, &action,
            &input_hash, &output_hash, &previous_hash,
        );

        Self {
            id,
            timestamp,
            actor_id,
            patient_id,
            action,
            policy_decision,
            input_hash,
            output_hash,
            previous_hash,
            event_hash,
            metadata: None,
        }
    }

    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    fn compute_hash(
        id: &Uuid,
        timestamp: &DateTime<Utc>,
        actor_id: &str,
        action: &str,
        input_hash: &str,
        output_hash: &str,
        previous_hash: &str,
    ) -> String {
        let mut h = Sha256::new();
        h.update(id.to_string().as_bytes());
        h.update(timestamp.to_rfc3339().as_bytes());
        h.update(actor_id.as_bytes());
        h.update(action.as_bytes());
        h.update(input_hash.as_bytes());
        h.update(output_hash.as_bytes());
        h.update(previous_hash.as_bytes());
        format!("{:x}", h.finalize())
    }
}

/// Compute SHA-256 hex digest of arbitrary bytes.
pub fn sha256_hash(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    format!("{:x}", h.finalize())
}
