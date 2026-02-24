# Simulation Environment & Human-in-the-Loop Plan

> Date: 2026-02-23

## Problem Statement

The current ClinicClaw demo is agent-driven with hardcoded defaults. There's no genuine human
interaction flow, and no way to stress-test the system by simulating realistic hospital scenarios.
The user wants:

1. **Well-designed human-in-the-loop** — real decision points, not just "click accept"
2. **Simulation environment** — role-based agents that simulate hospital staff + patients
3. **Self-evolve** — simulation surfaces limitations and suggests improvements

## Design: Two New Crates + Frontend Changes

### Crate 1: `cliniclaw-sim` — Simulation Engine

A new crate that provides role-based simulation agents and a scenario runner.

#### Simulation Agents (4 roles)

Each simulation agent implements a common trait and generates realistic inputs for the system:

```rust
#[async_trait]
pub trait SimAgent: Send + Sync {
    fn role(&self) -> SimRole;
    async fn act(&self, scenario: &ScenarioState, llm: &dyn LlmCapability) -> Result<SimAction, SimError>;
    async fn react(&self, event: &SimEvent, scenario: &ScenarioState, llm: &dyn LlmCapability) -> Result<Option<SimAction>, SimError>;
}
```

**1. PatientAgent** — Generates realistic patient presentations
- Produces chief complaints, symptom descriptions, medication histories
- Can simulate different acuity levels (routine, urgent, emergent)
- Uses Claude to generate realistic but synthetic clinical narratives
- Outputs: transcript fragments, symptom lists, medication history

**2. PhysicianAgent** — Simulates physician decision-making
- Reviews agent-generated notes, decides accept/modify/reject
- Proposes orders based on assessment
- Signs off on prior auth packages
- Simulates different physician styles (cautious, aggressive, protocol-driven)

**3. NurseAgent** — Simulates nursing workflow
- Initiates encounter documentation
- Reports vitals, observations
- Flags concerns for physician review
- Escalates based on acuity

**4. AdminAgent** — Simulates administrative review
- Reviews prior auth packages
- Checks compliance with payer requirements
- Generates denial/approval scenarios

#### Scenario System

Scenarios are JSON files that define a clinical encounter from start to finish:

```json
{
  "id": "scenario-chest-pain-001",
  "name": "Acute Chest Pain Workup",
  "description": "55M presents with substernal chest pain, PMH of HTN and DM2",
  "patient_profile": {
    "age": 55, "gender": "male",
    "conditions": ["I10", "E11.9"],
    "medications": ["lisinopril 10mg daily", "metformin 500mg BID"],
    "allergies": ["penicillin"]
  },
  "encounter": {
    "class": "emergency",
    "chief_complaint": "Chest pain for 2 hours"
  },
  "phases": [
    {
      "phase": "triage",
      "actor": "nurse",
      "action": "document_vitals",
      "expect_human_decision": false
    },
    {
      "phase": "documentation",
      "actor": "physician",
      "action": "ambient_doc",
      "expect_human_decision": true,
      "decision_point": "review_note"
    },
    {
      "phase": "orders",
      "actor": "physician",
      "action": "order_entry",
      "expect_human_decision": true,
      "decision_point": "approve_orders"
    },
    {
      "phase": "prior_auth",
      "actor": "admin",
      "action": "prior_auth",
      "expect_human_decision": true,
      "decision_point": "sign_auth_package"
    }
  ],
  "expected_outcomes": {
    "min_turns": 3,
    "expected_cds_alerts": true,
    "expected_chain_triggers": true
  }
}
```

#### Simulation Runner

```rust
pub struct SimRunner {
    agents: HashMap<SimRole, Box<dyn SimAgent>>,
    scenario: Scenario,
    state: ScenarioState,
    api_client: SimApiClient,  // Calls ClinicClaw API endpoints
    event_log: Vec<SimEvent>,
}
```

The runner:
1. Seeds FHIR with patient/encounter from scenario profile
2. Steps through phases in order
3. For each phase: the sim agent generates input → calls the real API → collects output
4. At `expect_human_decision: true` points, PAUSES and waits for real human or auto-resolves
5. Records everything: inputs, outputs, confidence scores, CDS alerts, human decisions
6. After completion: generates a SimReport with metrics and findings

#### Simulation Report

```rust
pub struct SimReport {
    scenario_id: String,
    total_phases: usize,
    completed_phases: usize,
    turns_created: Vec<TurnSummary>,
    human_decisions: Vec<HumanDecision>,
    cds_alerts_fired: Vec<CdsCardSummary>,
    chain_triggers: Vec<ChainTriggerSummary>,
    confidence_distribution: ConfidenceStats,
    policy_decisions: Vec<PolicyDecisionSummary>,
    issues_found: Vec<SimIssue>,  // Limitations surfaced
    suggestions: Vec<String>,     // Auto-generated improvement ideas
}
```

### Crate 2: No new crate needed — extend `cliniclaw-api`

Add simulation routes to the existing API:

- `POST /v1/sim/scenarios` — Load a scenario
- `POST /v1/sim/scenarios/:id/run` — Start a simulation run
- `GET /v1/sim/scenarios/:id/status` — Get current phase + state
- `POST /v1/sim/scenarios/:id/decide` — Human makes decision at a pause point
- `POST /v1/sim/scenarios/:id/auto` — Let sim agent auto-decide (for stress testing)
- `GET /v1/sim/scenarios/:id/report` — Get final report
- `GET /v1/sim/scenarios` — List available scenarios

### Frontend Changes

#### 1. Simulation Dashboard (`/sim`)
- Scenario picker — browse and select scenarios
- Run controls — start, pause, step-through, auto-run
- Live activity stream — reuses existing `ActivityStream` component
- Phase progress — visual pipeline of scenario phases
- Decision points — modal that presents the human with the agent output and asks for a real decision

#### 2. Enhanced Human-in-the-Loop on Existing Clinical Pages
- **Notes page**: Instead of just a transcript textarea, show a structured input form that the PatientAgent can pre-fill, but the human can edit before submitting
- **Orders page**: Show the proposed order with CDS alerts inline, require explicit acknowledge of each alert
- **Prior Auth page**: Show the assembled package with a checklist of required fields, allow modification before signing
- **Turn review**: Enhanced modify flow — inline editor for JSON output, side-by-side diff preview

#### 3. Simulation Report View (`/sim/:id/report`)
- Summary stats (confidence distribution, decision breakdown)
- Timeline of events
- Issues found with severity
- Improvement suggestions

## Implementation Order

### Phase 1: Simulation Engine Core (Rust)
1. Create `cliniclaw-sim` crate with `SimAgent` trait and `SimRole` enum
2. Implement `PatientAgent` — uses Claude to generate realistic patient narratives
3. Implement `PhysicianAgent` — reviews outputs, makes accept/modify/reject decisions
4. Implement `NurseAgent` and `AdminAgent`
5. Build `Scenario` and `ScenarioState` types (serde JSON)
6. Build `SimRunner` that steps through phases and calls real API
7. Build `SimReport` generation

### Phase 2: Simulation API Routes
8. Add sim routes to `cliniclaw-api` (load, run, status, decide, report)
9. Create 3 starter scenarios: chest-pain, routine-visit, prior-auth-denial

### Phase 3: Frontend — Simulation Dashboard
10. Simulation dashboard page with scenario picker
11. Phase progress component
12. Decision point modal (human-in-the-loop)
13. Report view

### Phase 4: Enhanced Human-in-the-Loop on Clinical Pages
14. Structured input forms (notes, orders, prior-auth) with pre-fill from sim agents
15. CDS alert acknowledgment flow on orders
16. Inline turn modification editor

## Key Design Decisions

- **Sim agents use the same LlmCapability trait** — can use real Claude or mock
- **Sim agents call the real API** — tests the full pipeline end-to-end, not a toy
- **Scenarios are JSON files** — easy to add new ones, no code changes needed
- **Human-in-the-loop is opt-in per phase** — some phases auto-run, decision points pause
- **SimReport surfaces issues automatically** — low confidence scores, policy denials, CDS alerts all become "issues found"
- **No new persistence layer** — simulation state lives in memory during the run; reports are JSON files saved to disk or returned via API
