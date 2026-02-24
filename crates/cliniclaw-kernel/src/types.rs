use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Per-encounter collaboration session.
///
/// Groups all agent turns for a single clinical encounter, giving context
/// to each AI action and enabling workspace-scoped review.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub encounter_id: String,
    pub practitioner_id: String,
    pub created_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
}

/// The collaboration atom: agent proposes, human reviews.
///
/// Every agent output flows through a Turn. The clinician can accept,
/// modify (capturing a diff as Feedback), reject, or escalate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turn {
    pub id: String,
    pub workspace_id: String,
    pub agent_name: String,
    pub action: String,
    pub input_snapshot: serde_json::Value,
    pub output_snapshot: serde_json::Value,
    pub confidence: Confidence,
    pub status: TurnStatus,
    pub feedback: Option<Feedback>,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolved_by: Option<String>,
    /// If this turn was triggered by another turn (agent chain), the source turn ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub triggered_by_turn_id: Option<String>,
}

/// Quantified certainty on an agent output.
///
/// Score is 0.0–1.0. Factors are human-readable explanations
/// of what contributed to the score (e.g. "known_medication",
/// "complete_soap_sections").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Confidence {
    pub score: f64,
    pub factors: Vec<String>,
}

impl Confidence {
    pub fn new(score: f64, factors: Vec<String>) -> Self {
        Self {
            score: score.clamp(0.0, 1.0),
            factors,
        }
    }

    pub fn high(factors: Vec<String>) -> Self {
        Self::new(0.9, factors)
    }

    pub fn medium(factors: Vec<String>) -> Self {
        Self::new(0.6, factors)
    }

    pub fn low(factors: Vec<String>) -> Self {
        Self::new(0.3, factors)
    }
}

impl Default for Confidence {
    fn default() -> Self {
        Self {
            score: 0.5,
            factors: vec!["default".to_string()],
        }
    }
}

/// The lifecycle of a Turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnStatus {
    Pending,
    Accepted,
    Modified,
    Rejected,
    Escalated,
}

impl TurnStatus {
    pub fn is_resolved(&self) -> bool {
        matches!(
            self,
            TurnStatus::Accepted | TurnStatus::Modified | TurnStatus::Rejected | TurnStatus::Escalated
        )
    }
}

impl std::fmt::Display for TurnStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Accepted => write!(f, "accepted"),
            Self::Modified => write!(f, "modified"),
            Self::Rejected => write!(f, "rejected"),
            Self::Escalated => write!(f, "escalated"),
        }
    }
}

impl std::str::FromStr for TurnStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "accepted" => Ok(Self::Accepted),
            "modified" => Ok(Self::Modified),
            "rejected" => Ok(Self::Rejected),
            "escalated" => Ok(Self::Escalated),
            other => Err(format!("unknown turn status: {other}")),
        }
    }
}

/// Structured correction captured when a human modifies AI output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feedback {
    pub action: FeedbackAction,
    pub original_output: serde_json::Value,
    pub corrected_output: Option<serde_json::Value>,
    pub reason: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// What the human did with the agent's output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackAction {
    Accept,
    Modify,
    Reject,
    Escalate,
}

impl std::fmt::Display for FeedbackAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Accept => write!(f, "accept"),
            Self::Modify => write!(f, "modify"),
            Self::Reject => write!(f, "reject"),
            Self::Escalate => write!(f, "escalate"),
        }
    }
}

/// Serialized input snapshot for deterministic replay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayInput {
    pub turn_id: String,
    pub agent_name: String,
    pub input_snapshot: serde_json::Value,
    pub original_output: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confidence_clamps_to_range() {
        let c = Confidence::new(1.5, vec![]);
        assert!((c.score - 1.0).abs() < f64::EPSILON);

        let c = Confidence::new(-0.5, vec![]);
        assert!(c.score.abs() < f64::EPSILON);
    }

    #[test]
    fn turn_status_roundtrip() {
        for status in [
            TurnStatus::Pending,
            TurnStatus::Accepted,
            TurnStatus::Modified,
            TurnStatus::Rejected,
            TurnStatus::Escalated,
        ] {
            let s = status.to_string();
            let parsed: TurnStatus = s.parse().unwrap();
            assert_eq!(parsed, status);
        }
    }

    #[test]
    fn turn_status_is_resolved() {
        assert!(!TurnStatus::Pending.is_resolved());
        assert!(TurnStatus::Accepted.is_resolved());
        assert!(TurnStatus::Modified.is_resolved());
        assert!(TurnStatus::Rejected.is_resolved());
        assert!(TurnStatus::Escalated.is_resolved());
    }

    #[test]
    fn confidence_serde_roundtrip() {
        let c = Confidence::new(0.85, vec!["known_medication".into(), "complete_soap".into()]);
        let json = serde_json::to_string(&c).unwrap();
        let deserialized: Confidence = serde_json::from_str(&json).unwrap();
        assert!((deserialized.score - 0.85).abs() < f64::EPSILON);
        assert_eq!(deserialized.factors.len(), 2);
    }

    #[test]
    fn feedback_action_display() {
        assert_eq!(FeedbackAction::Accept.to_string(), "accept");
        assert_eq!(FeedbackAction::Modify.to_string(), "modify");
        assert_eq!(FeedbackAction::Reject.to_string(), "reject");
        assert_eq!(FeedbackAction::Escalate.to_string(), "escalate");
    }
}
