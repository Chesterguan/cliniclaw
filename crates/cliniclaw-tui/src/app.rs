use chrono::{DateTime, Utc};
use cliniclaw_kernel::{AgentEvent, AgentEventType, StepStatus};
use ratatui::widgets::ListState;

// Governance pipeline stages in order
pub const STAGES: [&str; 6] = ["State", "Policy", "Capab", "Exec", "Verify", "Audit"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageStatus {
    Waiting,
    Active,
    Completed,
    Failed,
}

#[derive(Debug, Clone)]
pub struct ChainEntry {
    pub source_agent: String,
    pub target_agent: String,
    pub trigger_pattern: String,
    pub source_confidence: Option<f64>,
    pub target_confidence: Option<f64>,
}

pub struct App {
    pub events: Vec<AgentEvent>,
    pub current_run: Vec<AgentEvent>,
    pub chains: Vec<ChainEntry>,
    pub connected: bool,
    pub should_quit: bool,
    pub list_state: ListState,
    pub auto_scroll: bool,
    pub encounter_id: String,
    pub triggering: Option<String>,
    pub error_message: Option<String>,
}

impl App {
    pub fn new(encounter_id: String) -> Self {
        Self {
            events: Vec::new(),
            current_run: Vec::new(),
            chains: Vec::new(),
            connected: false,
            should_quit: false,
            list_state: ListState::default(),
            auto_scroll: true,
            encounter_id,
            triggering: None,
            error_message: None,
        }
    }

    pub fn on_agent_event(&mut self, event: AgentEvent) {
        // Detect new agent run
        if matches!(event.event_type, AgentEventType::AgentStarted) {
            self.current_run.clear();
        }

        // Detect chain triggers
        if let AgentEventType::ChainTrigger {
            ref trigger_pattern,
            ref target_agent,
        } = event.event_type
        {
            let source_confidence = self.current_run.iter().rev().find_map(|e| {
                if let AgentEventType::TurnCreation {
                    confidence_score, ..
                } = &e.event_type
                {
                    Some(*confidence_score)
                } else {
                    None
                }
            });
            self.chains.push(ChainEntry {
                source_agent: event.agent_name.clone(),
                target_agent: target_agent.clone(),
                trigger_pattern: trigger_pattern.clone(),
                source_confidence,
                target_confidence: None,
            });
        }

        // Update chain target confidence on completion
        if let AgentEventType::AgentCompleted {
            confidence_score, ..
        } = &event.event_type
        {
            if let Some(chain) = self
                .chains
                .iter_mut()
                .rev()
                .find(|c| c.target_agent == event.agent_name)
            {
                chain.target_confidence = Some(*confidence_score);
            }
        }

        self.current_run.push(event.clone());
        self.events.push(event);

        // Cap at 500 events
        if self.events.len() > 500 {
            let drain_count = self.events.len() - 500;
            self.events.drain(0..drain_count);
            // Adjust selected index to account for removed items
            if let Some(sel) = self.list_state.selected() {
                if sel < drain_count {
                    self.list_state.select(Some(0));
                } else {
                    self.list_state.select(Some(sel - drain_count));
                }
            }
        }

        // Clear triggering status on agent start
        if matches!(
            self.events.last().map(|e| &e.event_type),
            Some(AgentEventType::AgentStarted)
        ) {
            self.triggering = None;
        }

        // Auto-scroll
        if self.auto_scroll && !self.events.is_empty() {
            self.list_state.select(Some(self.events.len() - 1));
        }
    }

    pub fn clear(&mut self) {
        self.events.clear();
        self.current_run.clear();
        self.chains.clear();
        self.list_state.select(None);
        self.error_message = None;
        self.triggering = None;
    }

    pub fn scroll_up(&mut self) {
        self.auto_scroll = false;
        let i = self.list_state.selected().unwrap_or(0);
        if i > 0 {
            self.list_state.select(Some(i - 1));
        }
    }

    pub fn scroll_down(&mut self) {
        if self.events.is_empty() {
            return;
        }
        let i = self.list_state.selected().unwrap_or(0);
        let max = self.events.len() - 1;
        if i < max {
            self.list_state.select(Some(i + 1));
        }
        if i + 1 >= max {
            self.auto_scroll = true;
        }
    }

    pub fn jump_to_bottom(&mut self) {
        if !self.events.is_empty() {
            self.list_state.select(Some(self.events.len() - 1));
        }
        self.auto_scroll = true;
    }

    pub fn compute_stage_statuses(&self) -> [StageStatus; 6] {
        let mut statuses = [StageStatus::Waiting; 6];

        if self.current_run.is_empty() {
            return statuses;
        }

        let mut max_stage: Option<usize> = None;
        let mut has_failed = false;
        let mut has_completed = false;

        for event in &self.current_run {
            if let Some(idx) = event_to_stage_index(&event.event_type) {
                max_stage = Some(max_stage.map_or(idx, |m: usize| m.max(idx)));
            }
            if matches!(event.event_type, AgentEventType::AgentFailed { .. }) {
                has_failed = true;
            }
            if matches!(event.event_type, AgentEventType::AgentCompleted { .. }) {
                has_completed = true;
            }
        }

        if let Some(max) = max_stage {
            for (i, status) in statuses.iter_mut().enumerate() {
                if i < max {
                    *status = StageStatus::Completed;
                } else if i == max {
                    if has_failed {
                        *status = StageStatus::Failed;
                    } else if has_completed {
                        *status = StageStatus::Completed;
                    } else {
                        *status = StageStatus::Active;
                    }
                }
                // i > max stays Waiting
            }
        }

        statuses
    }
}

fn event_to_stage_index(et: &AgentEventType) -> Option<usize> {
    match et {
        AgentEventType::AgentStarted | AgentEventType::ContextBuilding { .. } => Some(0),
        AgentEventType::SkillLookup { .. }
        | AgentEventType::RoleCheck { .. }
        | AgentEventType::PopulationGate { .. }
        | AgentEventType::PolicyEvaluation { .. } => Some(1),
        AgentEventType::CapabilityCheck { .. } => Some(2),
        AgentEventType::LlmCall { .. }
        | AgentEventType::ResponseParsing { .. }
        | AgentEventType::CdsCheck { .. } => Some(3),
        AgentEventType::Verification { .. } => Some(4),
        AgentEventType::AuditCreation { .. }
        | AgentEventType::FhirWrite { .. }
        | AgentEventType::TurnCreation { .. }
        | AgentEventType::ChainTrigger { .. }
        | AgentEventType::AgentCompleted { .. }
        | AgentEventType::AgentFailed { .. } => Some(5),
    }
}

pub fn event_label(et: &AgentEventType) -> &'static str {
    match et {
        AgentEventType::AgentStarted => "Agent Started",
        AgentEventType::ContextBuilding { .. } => "Building Context",
        AgentEventType::SkillLookup { .. } => "Skill Lookup",
        AgentEventType::RoleCheck { .. } => "Role Verification",
        AgentEventType::CapabilityCheck { .. } => "Capability Check",
        AgentEventType::PopulationGate { .. } => "Population Gate",
        AgentEventType::PolicyEvaluation { .. } => "Policy Evaluation",
        AgentEventType::LlmCall { .. } => "Calling Claude",
        AgentEventType::ResponseParsing { .. } => "Parsing Response",
        AgentEventType::CdsCheck { .. } => "CDS Check",
        AgentEventType::Verification { .. } => "Output Verification",
        AgentEventType::AuditCreation { .. } => "Creating Audit Record",
        AgentEventType::FhirWrite { .. } => "Writing to FHIR",
        AgentEventType::TurnCreation { .. } => "Creating Turn",
        AgentEventType::ChainTrigger { .. } => "Chain Triggered",
        AgentEventType::AgentCompleted { .. } => "Agent Completed",
        AgentEventType::AgentFailed { .. } => "Agent Failed",
    }
}

pub fn event_detail(et: &AgentEventType) -> String {
    match et {
        AgentEventType::AgentStarted => String::new(),
        AgentEventType::ContextBuilding { detail, .. } => detail.clone(),
        AgentEventType::SkillLookup { skill_id, matched } => {
            let id = skill_id.as_deref().unwrap_or("none");
            format!("{id} — {}", if *matched { "matched" } else { "no match" })
        }
        AgentEventType::RoleCheck { role, allowed } => {
            format!(
                "{role} — {}",
                if *allowed { "allowed" } else { "denied" }
            )
        }
        AgentEventType::CapabilityCheck { capability, valid } => {
            format!(
                "{capability} — {}",
                if *valid { "valid" } else { "invalid" }
            )
        }
        AgentEventType::PopulationGate { passed, reason } => {
            if *passed {
                "Passed".to_string()
            } else {
                format!("Failed: {}", reason.as_deref().unwrap_or("unknown"))
            }
        }
        AgentEventType::PolicyEvaluation {
            decision,
            rule_name,
        } => {
            let rule = rule_name
                .as_deref()
                .map(|r| format!(" ({r})"))
                .unwrap_or_default();
            format!("{decision}{rule}")
        }
        AgentEventType::LlmCall { status, elapsed_ms } => match status {
            StepStatus::Started => "...".to_string(),
            StepStatus::Completed => {
                format!("{}ms", elapsed_ms.unwrap_or(0))
            }
            StepStatus::Failed => "failed".to_string(),
        },
        AgentEventType::ResponseParsing { status, detail } => match status {
            StepStatus::Completed => detail.as_deref().unwrap_or("ok").to_string(),
            StepStatus::Failed => detail.as_deref().unwrap_or("failed").to_string(),
            StepStatus::Started => "...".to_string(),
        },
        AgentEventType::CdsCheck {
            cards_count,
            max_severity,
        } => {
            let sev = max_severity.as_deref().unwrap_or("none");
            format!("{cards_count} cards, severity: {sev}")
        }
        AgentEventType::Verification { passed, detail } => {
            let d = detail.as_deref().unwrap_or("");
            if *passed {
                format!("Passed{}", if d.is_empty() { String::new() } else { format!(" — {d}") })
            } else {
                format!("Failed{}", if d.is_empty() { String::new() } else { format!(" — {d}") })
            }
        }
        AgentEventType::AuditCreation { audit_event_id } => audit_event_id.clone(),
        AgentEventType::FhirWrite {
            resource_type,
            resource_id,
        } => {
            let id = resource_id.as_deref().unwrap_or("?");
            format!("{resource_type}/{id}")
        }
        AgentEventType::TurnCreation {
            turn_id,
            confidence_score,
        } => format!("{turn_id} ({:.0}%)", confidence_score * 100.0),
        AgentEventType::ChainTrigger {
            trigger_pattern,
            target_agent,
        } => format!("{trigger_pattern} → {target_agent}"),
        AgentEventType::AgentCompleted {
            confidence_score,
            elapsed_ms,
        } => format!("{:.0}% confidence, {elapsed_ms}ms", confidence_score * 100.0),
        AgentEventType::AgentFailed { error } => error.clone(),
    }
}

pub fn event_icon(et: &AgentEventType) -> &'static str {
    match et {
        AgentEventType::AgentFailed { .. } => "✗",
        AgentEventType::LlmCall { status, .. } | AgentEventType::ResponseParsing { status, .. } => {
            match status {
                StepStatus::Started => "●",
                StepStatus::Completed => "✓",
                StepStatus::Failed => "✗",
            }
        }
        AgentEventType::PopulationGate { passed, .. }
        | AgentEventType::Verification { passed, .. } => {
            if *passed { "✓" } else { "✗" }
        }
        AgentEventType::RoleCheck { allowed, .. } => {
            if *allowed { "✓" } else { "✗" }
        }
        AgentEventType::CapabilityCheck { valid, .. } => {
            if *valid { "✓" } else { "✗" }
        }
        AgentEventType::ChainTrigger { .. } => "→",
        _ => "✓",
    }
}

pub fn time_delta_ms(base: &DateTime<Utc>, ts: &DateTime<Utc>) -> i64 {
    (*ts - *base).num_milliseconds()
}

pub fn agent_short(name: &str) -> &str {
    match name {
        "ambient_doc" => "AD",
        "order_entry" => "OE",
        "prior_auth" => "PA",
        _ => name,
    }
}
