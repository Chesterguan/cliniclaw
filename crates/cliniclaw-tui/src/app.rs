use chrono::{DateTime, Utc};
use cliniclaw_kernel::{AgentEvent, AgentEventType, StepStatus};
use ratatui::widgets::ListState;
use std::collections::HashMap;

// Governance pipeline stages in order
pub const STAGES: [&str; 6] = ["State", "Policy", "Capab", "Exec", "Verify", "Audit"];

/// Which top-level view the TUI is rendering.
#[derive(Debug, Clone, PartialEq)]
pub enum ViewMode {
    /// Single-encounter detail view (original).
    Detail,
    /// Multi-patient hospital simulation dashboard.
    Hospital,
}

/// Which panel is shown in the right half of the detail view.
#[derive(Debug, Clone, PartialEq)]
pub enum RightPanel {
    /// Show the chain trigger visualization (default).
    Chain,
    /// Show full details for the currently selected event.
    EventDetail,
}

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

/// Per-patient progress state derived from accumulated events.
#[derive(Debug, Clone, PartialEq)]
pub enum PatientProgress {
    /// No events received yet for this encounter.
    Waiting,
    /// At least one LLM call is in-flight.
    Active,
    /// Agents have run but none are currently in-flight.
    InProgress,
}

/// Snapshot of a single patient's activity for the hospital dashboard.
///
/// All fields are public for future use (detail panels, tooltips, export).
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PatientStatus {
    pub encounter_id: String,
    pub name: String,
    pub diagnosis: String,
    pub status: PatientProgress,
    pub agent_count: usize,
    pub turn_count: usize,
    pub last_agent: Option<String>,
}

/// Live metrics derived from the accumulated event stream.
#[derive(Debug, Clone, Default)]
pub struct LiveMetrics {
    /// Total events received since last clear.
    pub total_events: usize,
    /// Smoothed events-per-second (computed over a 5-second rolling window).
    pub events_per_second: f64,
    /// Number of agents that have started but not yet completed or failed.
    pub active_agents: usize,
    /// How many PolicyEvaluation events resolved to "allow".
    pub policy_allow: usize,
    /// How many PolicyEvaluation events resolved to "deny".
    pub policy_deny: usize,
    /// How many PolicyEvaluation events resolved to "require_approval".
    pub policy_require_approval: usize,
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
    /// Wall-clock instant when `triggering` was last set — used to auto-clear it.
    pub triggering_set_at: Option<std::time::Instant>,
    pub error_message: Option<String>,
    pub view_mode: ViewMode,
    /// Which panel is shown on the right of the detail view.
    pub right_panel: RightPanel,
    /// Live metrics bar state.
    pub metrics: LiveMetrics,
    /// Dynamic patient roster: encounter_id → (display_name, diagnosis_hint)
    /// Populated by events and pre-seeded with the static simulation roster.
    pub patient_roster: HashMap<String, (String, String)>,
    /// Timestamps of the last N events used for EPS calculation.
    event_timestamps: std::collections::VecDeque<std::time::Instant>,
    /// Track in-flight agent runs for active_agents count: key = (enc_id, agent_name).
    agent_in_flight: std::collections::HashSet<(String, String)>,
}

impl App {
    pub fn new(encounter_id: String) -> Self {
        // Pre-seed roster from the simulation's static encounter-to-patient mapping
        let patient_roster = [
            ("enc-001", "Mitchell", "HTN"),
            ("enc-002", "Thompson", "T2DM"),
            ("enc-003", "Garcia", "Prenatal"),
            ("enc-004", "Chen", "COPD"),
            ("enc-005", "Johnson", "Knee OA"),
            ("enc-006", "Williams", "CHF"),
        ]
        .iter()
        .map(|(enc, name, dx)| (enc.to_string(), (name.to_string(), dx.to_string())))
        .collect();

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
            triggering_set_at: None,
            error_message: None,
            view_mode: ViewMode::Detail,
            right_panel: RightPanel::Chain,
            metrics: LiveMetrics::default(),
            patient_roster,
            event_timestamps: std::collections::VecDeque::new(),
            agent_in_flight: std::collections::HashSet::new(),
        }
    }

    pub fn on_agent_event(&mut self, event: AgentEvent) {
        // ── Dynamic patient roster ──────────────────────────────────────────
        // If we see an encounter_id we don't know yet, add a placeholder entry.
        // Real patient names currently only come from the static seed above,
        // but the slot is here for future FHIR Patient demographic events.
        if !self.patient_roster.contains_key(&event.encounter_id) {
            self.patient_roster.insert(
                event.encounter_id.clone(),
                (event.encounter_id.clone(), "Unknown".to_string()),
            );
        }

        // ── Agent in-flight tracking ────────────────────────────────────────
        let key = (event.encounter_id.clone(), event.agent_name.clone());
        match &event.event_type {
            AgentEventType::AgentStarted => {
                self.agent_in_flight.insert(key.clone());
            }
            AgentEventType::AgentCompleted { .. } | AgentEventType::AgentFailed { .. } => {
                self.agent_in_flight.remove(&key);
            }
            _ => {}
        }

        // ── Policy decision counters ────────────────────────────────────────
        if let AgentEventType::PolicyEvaluation { ref decision, .. } = event.event_type {
            match decision.as_str() {
                "allow" => self.metrics.policy_allow += 1,
                "deny" => self.metrics.policy_deny += 1,
                "require_approval" => self.metrics.policy_require_approval += 1,
                _ => {}
            }
        }

        // ── Detect new agent run (for detail-view pipeline) ─────────────────
        if matches!(event.event_type, AgentEventType::AgentStarted) {
            self.current_run.clear();
        }

        // ── Chain trigger detection ─────────────────────────────────────────
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

        // ── EPS rolling window ──────────────────────────────────────────────
        let now = std::time::Instant::now();
        self.event_timestamps.push_back(now);
        // Keep only events in the last 5 seconds
        let cutoff = now - std::time::Duration::from_secs(5);
        while self
            .event_timestamps
            .front()
            .map_or(false, |&t| t < cutoff)
        {
            self.event_timestamps.pop_front();
        }
        let window_count = self.event_timestamps.len();
        self.metrics.events_per_second = if window_count > 1 {
            window_count as f64 / 5.0
        } else {
            0.0
        };

        // ── Cap at 500 events ───────────────────────────────────────────────
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

        // ── Update aggregate metrics ────────────────────────────────────────
        self.metrics.total_events = self.events.len();
        self.metrics.active_agents = self.agent_in_flight.len();

        // Clear triggering status on agent start
        if matches!(
            self.events.last().map(|e| &e.event_type),
            Some(AgentEventType::AgentStarted)
        ) {
            self.triggering = None;
            self.triggering_set_at = None;
        }

        // Auto-scroll
        if self.auto_scroll && !self.events.is_empty() {
            self.list_state.select(Some(self.events.len() - 1));
        }
    }

    /// Called each tick; handles time-based state transitions (flash timer, EPS decay).
    pub fn on_tick(&mut self) {
        // Auto-expire the "Triggering…" flash after 2 seconds
        if let Some(set_at) = self.triggering_set_at {
            if set_at.elapsed() >= std::time::Duration::from_secs(2) {
                self.triggering = None;
                self.triggering_set_at = None;
            }
        }

        // Decay EPS toward zero when no events are arriving
        let now = std::time::Instant::now();
        let cutoff = now - std::time::Duration::from_secs(5);
        while self
            .event_timestamps
            .front()
            .map_or(false, |&t| t < cutoff)
        {
            self.event_timestamps.pop_front();
        }
        let window_count = self.event_timestamps.len();
        self.metrics.events_per_second = if window_count > 1 {
            window_count as f64 / 5.0
        } else {
            0.0
        };
    }

    pub fn clear(&mut self) {
        self.events.clear();
        self.current_run.clear();
        self.chains.clear();
        self.list_state.select(None);
        self.error_message = None;
        self.triggering = None;
        self.triggering_set_at = None;
        self.metrics = LiveMetrics::default();
        self.agent_in_flight.clear();
        self.event_timestamps.clear();
        self.right_panel = RightPanel::Chain;
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

    /// Open event detail for the currently selected event.
    pub fn open_event_detail(&mut self) {
        if self.list_state.selected().is_some() {
            self.right_panel = RightPanel::EventDetail;
        }
    }

    /// Return to the default chain panel.
    pub fn close_event_detail(&mut self) {
        self.right_panel = RightPanel::Chain;
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

    /// Derive per-patient status from accumulated events.
    ///
    /// Returns entries for every encounter seen so far (dynamic roster),
    /// ordered with known simulation encounters first, then any unseen ones.
    pub fn patient_statuses(&self) -> Vec<PatientStatus> {
        // Ordered list: static simulation encounters first so hospital view is stable
        let static_order = [
            "enc-001", "enc-002", "enc-003", "enc-004", "enc-005", "enc-006",
        ];

        // Collect all encounter IDs seen in events (may include extra encounters)
        let mut all_ids: Vec<String> = static_order
            .iter()
            .map(|s| s.to_string())
            .collect();
        for ev in &self.events {
            if !all_ids.contains(&ev.encounter_id) {
                all_ids.push(ev.encounter_id.clone());
            }
        }

        all_ids
            .iter()
            .map(|enc_id| {
                let (name, dx) = self
                    .patient_roster
                    .get(enc_id)
                    .cloned()
                    .unwrap_or_else(|| (enc_id.clone(), "Unknown".to_string()));

                let patient_events: Vec<_> = self
                    .events
                    .iter()
                    .filter(|e| &e.encounter_id == enc_id)
                    .collect();

                let last_event = patient_events.last();

                let has_active = patient_events.iter().any(|e| {
                    matches!(
                        e.event_type,
                        AgentEventType::LlmCall {
                            status: StepStatus::Started,
                            ..
                        }
                    ) && !patient_events.iter().any(|e2| {
                        e2.timestamp > e.timestamp
                            && matches!(
                                e2.event_type,
                                AgentEventType::AgentCompleted { .. }
                                    | AgentEventType::AgentFailed { .. }
                            )
                    })
                });

                let agent_count = patient_events
                    .iter()
                    .filter(|e| matches!(e.event_type, AgentEventType::AgentStarted))
                    .count();

                let turn_count = patient_events
                    .iter()
                    .filter(|e| matches!(e.event_type, AgentEventType::TurnCreation { .. }))
                    .count();

                let status = if has_active {
                    PatientProgress::Active
                } else if agent_count > 0 {
                    PatientProgress::InProgress
                } else {
                    PatientProgress::Waiting
                };

                PatientStatus {
                    encounter_id: enc_id.clone(),
                    name,
                    diagnosis: dx,
                    status,
                    agent_count,
                    turn_count,
                    last_agent: last_event.map(|e| e.agent_name.clone()),
                }
            })
            .collect()
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

/// Return a short two-letter abbreviation for an agent name.
pub fn agent_short(name: &str) -> &'static str {
    match name {
        "ambient_doc" => "AD",
        "order_entry" => "OE",
        "prior_auth" => "PA",
        "triage_assess" => "TR",
        "lab_review" => "LR",
        "discharge_plan" => "DC",
        "nurse_assess" => "NA",
        "pharmacy_review" => "PR",
        _ => "??",
    }
}
