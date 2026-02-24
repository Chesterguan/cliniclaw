use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

/// Status of an in-progress step within agent execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Started,
    Completed,
    Failed,
}

/// A single real-time event emitted during agent execution.
///
/// Events are broadcast over a `tokio::sync::broadcast` channel so that
/// SSE subscribers can watch agents think, get policy-gated, call the LLM,
/// and write to FHIR — all in real time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub encounter_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    pub agent_name: String,
    pub event_type: AgentEventType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub triggered_by_turn_id: Option<String>,
}

/// Discriminated union of all event types that an agent can emit.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgentEventType {
    AgentStarted,
    ContextBuilding {
        step: u8,
        detail: String,
    },
    SkillLookup {
        skill_id: Option<String>,
        matched: bool,
    },
    RoleCheck {
        role: String,
        allowed: bool,
    },
    CapabilityCheck {
        capability: String,
        valid: bool,
    },
    PopulationGate {
        passed: bool,
        reason: Option<String>,
    },
    PolicyEvaluation {
        decision: String,
        rule_name: Option<String>,
    },
    LlmCall {
        status: StepStatus,
        elapsed_ms: Option<u64>,
    },
    ResponseParsing {
        status: StepStatus,
        detail: Option<String>,
    },
    CdsCheck {
        cards_count: usize,
        max_severity: Option<String>,
    },
    Verification {
        passed: bool,
        detail: Option<String>,
    },
    AuditCreation {
        audit_event_id: String,
    },
    FhirWrite {
        resource_type: String,
        resource_id: Option<String>,
    },
    TurnCreation {
        turn_id: String,
        confidence_score: f64,
    },
    ChainTrigger {
        trigger_pattern: String,
        target_agent: String,
    },
    AgentCompleted {
        confidence_score: f64,
        elapsed_ms: u64,
    },
    AgentFailed {
        error: String,
    },
}

/// Convenience wrapper that pre-fills encounter/agent context on every emit.
///
/// Route handlers create one `EventEmitter` per request, then call
/// `emit(event_type)` at each step. If no SSE subscribers are connected,
/// the send silently drops — zero overhead.
#[derive(Clone)]
pub struct EventEmitter {
    tx: broadcast::Sender<AgentEvent>,
    encounter_id: String,
    agent_name: String,
    workspace_id: Option<String>,
    triggered_by_turn_id: Option<String>,
}

impl EventEmitter {
    pub fn new(
        tx: broadcast::Sender<AgentEvent>,
        encounter_id: impl Into<String>,
        agent_name: impl Into<String>,
    ) -> Self {
        Self {
            tx,
            encounter_id: encounter_id.into(),
            agent_name: agent_name.into(),
            workspace_id: None,
            triggered_by_turn_id: None,
        }
    }

    pub fn with_workspace(mut self, id: impl Into<String>) -> Self {
        self.workspace_id = Some(id.into());
        self
    }

    pub fn with_trigger(mut self, turn_id: impl Into<String>) -> Self {
        self.triggered_by_turn_id = Some(turn_id.into());
        self
    }

    pub fn set_workspace(&mut self, id: impl Into<String>) {
        self.workspace_id = Some(id.into());
    }

    /// Emit an event. Silently drops if no subscribers.
    pub fn emit(&self, event_type: AgentEventType) {
        let event = AgentEvent {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            encounter_id: self.encounter_id.clone(),
            workspace_id: self.workspace_id.clone(),
            turn_id: None,
            agent_name: self.agent_name.clone(),
            event_type,
            triggered_by_turn_id: self.triggered_by_turn_id.clone(),
        };
        let _ = self.tx.send(event);
    }

    /// Emit with an explicit turn_id (used after turn creation).
    pub fn emit_with_turn(&self, turn_id: &str, event_type: AgentEventType) {
        let event = AgentEvent {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            encounter_id: self.encounter_id.clone(),
            workspace_id: self.workspace_id.clone(),
            turn_id: Some(turn_id.to_string()),
            agent_name: self.agent_name.clone(),
            event_type,
            triggered_by_turn_id: self.triggered_by_turn_id.clone(),
        };
        let _ = self.tx.send(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_event_serializes_to_json() {
        let event = AgentEvent {
            id: "evt-001".into(),
            timestamp: Utc::now(),
            encounter_id: "enc-001".into(),
            workspace_id: None,
            turn_id: None,
            agent_name: "ambient_doc".into(),
            event_type: AgentEventType::AgentStarted,
            triggered_by_turn_id: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("agent_started"));
        assert!(json.contains("ambient_doc"));
    }

    #[test]
    fn event_type_tagged_serde() {
        let et = AgentEventType::LlmCall {
            status: StepStatus::Completed,
            elapsed_ms: Some(320),
        };
        let json = serde_json::to_string(&et).unwrap();
        assert!(json.contains("\"kind\":\"llm_call\""));
        assert!(json.contains("\"elapsed_ms\":320"));
    }

    #[test]
    fn event_emitter_sends_events() {
        let (tx, mut rx) = broadcast::channel::<AgentEvent>(16);
        let emitter = EventEmitter::new(tx, "enc-001", "ambient_doc");
        emitter.emit(AgentEventType::AgentStarted);

        let received = rx.try_recv().unwrap();
        assert_eq!(received.encounter_id, "enc-001");
        assert_eq!(received.agent_name, "ambient_doc");
    }

    #[test]
    fn event_emitter_silently_drops_when_no_receivers() {
        let (tx, _) = broadcast::channel::<AgentEvent>(16);
        // Drop the receiver — emitter should not panic
        let emitter = EventEmitter::new(tx, "enc-001", "ambient_doc");
        emitter.emit(AgentEventType::AgentStarted);
    }

    #[test]
    fn event_emitter_with_workspace_and_trigger() {
        let (tx, mut rx) = broadcast::channel::<AgentEvent>(16);
        let emitter = EventEmitter::new(tx, "enc-001", "order_entry")
            .with_workspace("ws-001")
            .with_trigger("turn-prev");

        emitter.emit_with_turn(
            "turn-001",
            AgentEventType::TurnCreation {
                turn_id: "turn-001".into(),
                confidence_score: 0.85,
            },
        );

        let received = rx.try_recv().unwrap();
        assert_eq!(received.workspace_id.as_deref(), Some("ws-001"));
        assert_eq!(received.turn_id.as_deref(), Some("turn-001"));
        assert_eq!(received.triggered_by_turn_id.as_deref(), Some("turn-prev"));
    }
}
