use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// The outcome of an audited action — aligns with VERITAS's "audit all outcomes" principle.
/// Every agent action produces an audit record regardless of success or failure.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditOutcome {
    /// Action completed successfully
    Success,
    /// Policy denied the action
    PolicyDenied { reason: String },
    /// Action requires human approval before proceeding
    AwaitingApproval { reason: String },
    /// Agent encountered an error during execution
    AgentError { reason: String },
    /// Output verification failed
    VerificationFailed { reason: String },
}

impl AuditOutcome {
    /// Returns the string representation used in the policy_decision field for backward compat.
    pub fn as_decision_str(&self) -> &str {
        match self {
            Self::Success => "allow",
            Self::PolicyDenied { .. } => "deny",
            Self::AwaitingApproval { .. } => "require_approval",
            Self::AgentError { .. } => "agent_error",
            Self::VerificationFailed { .. } => "verification_failed",
        }
    }
}

impl std::fmt::Display for AuditOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Success => write!(f, "success"),
            Self::PolicyDenied { reason } => write!(f, "policy_denied: {reason}"),
            Self::AwaitingApproval { reason } => write!(f, "awaiting_approval: {reason}"),
            Self::AgentError { reason } => write!(f, "agent_error: {reason}"),
            Self::VerificationFailed { reason } => write!(f, "verification_failed: {reason}"),
        }
    }
}

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
            &id, &timestamp, &actor_id, &patient_id,
            &action, &policy_decision,
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

    /// Create an audit event for a policy denial.
    /// Records the denial in the audit chain even though no agent work was performed.
    pub fn denied(
        actor_id: impl Into<String>,
        patient_id: Option<String>,
        action: impl Into<String>,
        reason: impl Into<String>,
        input_hash: impl Into<String>,
    ) -> Self {
        let reason = reason.into();
        Self::new(
            actor_id, patient_id, action,
            "policy_denied",
            input_hash, "none", "",
        ).with_metadata(serde_json::json!({
            "outcome": "policy_denied",
            "reason": reason,
        }))
    }

    /// Create an audit event for an action awaiting human approval.
    pub fn awaiting_approval(
        actor_id: impl Into<String>,
        patient_id: Option<String>,
        action: impl Into<String>,
        input_hash: impl Into<String>,
    ) -> Self {
        Self::new(
            actor_id, patient_id, action,
            "awaiting_approval",
            input_hash, "none", "",
        ).with_metadata(serde_json::json!({
            "outcome": "awaiting_approval",
        }))
    }

    /// Create an audit event for an agent execution error.
    /// The error message must NOT contain PHI.
    pub fn agent_error(
        actor_id: impl Into<String>,
        patient_id: Option<String>,
        action: impl Into<String>,
        error_message: impl Into<String>,
        input_hash: impl Into<String>,
    ) -> Self {
        let error_message = error_message.into();
        Self::new(
            actor_id, patient_id, action,
            "agent_error",
            input_hash, "none", "",
        ).with_metadata(serde_json::json!({
            "outcome": "agent_error",
            "error": error_message,
        }))
    }

    /// Create an audit event for a verification failure.
    /// Records the rejected output hash for forensic review.
    pub fn verification_failed(
        actor_id: impl Into<String>,
        patient_id: Option<String>,
        action: impl Into<String>,
        reason: impl Into<String>,
        input_hash: impl Into<String>,
        output_hash: impl Into<String>,
    ) -> Self {
        let reason = reason.into();
        Self::new(
            actor_id, patient_id, action,
            "verification_failed",
            input_hash, output_hash, "",
        ).with_metadata(serde_json::json!({
            "outcome": "verification_failed",
            "reason": reason,
        }))
    }

    /// Create an audit event for a failure outcome (policy denial, agent error, etc.).
    /// The output_hash is set to empty since no output was produced.
    /// Aligns with VERITAS: every outcome — success or failure — gets an audit record.
    pub fn for_outcome(
        actor_id: impl Into<String>,
        patient_id: Option<String>,
        action: impl Into<String>,
        outcome: &AuditOutcome,
        input_hash: impl Into<String>,
        previous_hash: impl Into<String>,
    ) -> Self {
        let output_hash = match outcome {
            AuditOutcome::Success => String::new(), // caller should set real hash
            _ => String::new(), // no output for failures
        };
        let mut event = Self::new(
            actor_id,
            patient_id,
            action,
            outcome.as_decision_str(),
            input_hash,
            output_hash,
            previous_hash,
        );
        event.metadata = Some(serde_json::json!({
            "outcome": outcome.to_string(),
        }));
        event
    }

    /// Compute SHA-256 hash of all security-relevant audit event fields.
    /// Includes patient_id and policy_decision for tamper detection.
    pub fn compute_hash(
        id: &Uuid,
        timestamp: &DateTime<Utc>,
        actor_id: &str,
        patient_id: &Option<String>,
        action: &str,
        policy_decision: &str,
        input_hash: &str,
        output_hash: &str,
        previous_hash: &str,
    ) -> String {
        let mut h = Sha256::new();
        h.update(id.to_string().as_bytes());
        h.update(timestamp.to_rfc3339().as_bytes());
        h.update(actor_id.as_bytes());
        h.update(patient_id.as_deref().unwrap_or("").as_bytes());
        h.update(action.as_bytes());
        h.update(policy_decision.as_bytes());
        h.update(input_hash.as_bytes());
        h.update(output_hash.as_bytes());
        h.update(previous_hash.as_bytes());
        format!("{:x}", h.finalize())
    }

    /// Recompute and verify the event hash from stored fields.
    pub fn verify_hash(&self) -> bool {
        let expected = Self::compute_hash(
            &self.id, &self.timestamp, &self.actor_id, &self.patient_id,
            &self.action, &self.policy_decision,
            &self.input_hash, &self.output_hash, &self.previous_hash,
        );
        self.event_hash == expected
    }
}

/// Compute SHA-256 hex digest of arbitrary bytes.
pub fn sha256_hash(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    format!("{:x}", h.finalize())
}
