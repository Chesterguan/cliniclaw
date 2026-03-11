# Hospital Simulation — "ClinicClaw Live" Demo

> Date: 2026-02-24

## Goal

Transform the current 3-agent demo into a full hospital simulation where **8 agent types** process **6 patients concurrently** across realistic clinical pathways — all visible in real time through both TUI and web dashboards. The demo should feel like watching a hospital breathe.

## What Changes

### 1. New Simulation Orchestrator — `POST /v1/simulate`

A single API endpoint that kicks off the entire hospital simulation. Lives in `cliniclaw-api/src/routes/simulate.rs`.

**Behavior:**
- Accepts `{ "speed": "normal" | "fast" }` (fast = shorter delays between steps)
- Spawns one `tokio::spawn` per patient pathway (6 concurrent tasks)
- Each pathway calls existing + new agent API endpoints in sequence with staggered delays (200ms-800ms between steps in fast mode, 1-3s in normal)
- Returns immediately with `{ "status": "running", "pathways": 6 }`
- All events flow through the existing SSE broadcast channel — zero changes to the event system

**Patient Pathways (using existing 6 encounters):**

| Patient | Enc | Class | Pathway |
|---------|-----|-------|---------|
| Sarah Mitchell (HTN) | enc-001 | AMB | triage → nurse_assess → ambient_doc → order_entry (lisinopril adjust) → pharmacy_review → discharge_plan |
| James Thompson (T2DM) | enc-002 | AMB | triage → nurse_assess → lab_review → ambient_doc → order_entry (metformin) → pharmacy_review |
| Maria Garcia (prenatal) | enc-003 | AMB | triage → nurse_assess → ambient_doc → lab_review |
| Robert Chen (COPD) | enc-004 | IMP | triage → nurse_assess → ambient_doc → order_entry (prednisone) → order_entry (albuterol) → lab_review → pharmacy_review |
| Emily Johnson (knee OA) | enc-005 | AMB | triage → nurse_assess → ambient_doc → prior_auth (TKR) |
| David Williams (CHF) | enc-006 | AMB | triage → nurse_assess → ambient_doc → order_entry (warfarin) → pharmacy_review → lab_review → discharge_plan |

This means at peak, up to 6 agents run concurrently across different patients, with chain triggers adding more. Total agent executions: ~32 across all pathways.

### 2. Five New Agents in `cliniclaw-agents`

Each follows the exact same pattern as existing agents: struct wrapping `Arc<dyn LlmCapability>`, async method taking input + PolicyEngine, returns typed output with confidence + audit event.

#### a) `TriageAssessAgent` (~150 LOC)
- **Input:** encounter_id, chief_complaint, vitals_text
- **Output:** triage_level (ESI 1-5), acuity_label, recommended_actions
- **FHIR write:** Observation (triage assessment)
- **Mock response:** ESI-2 for IMP encounters, ESI-3 for AMB

#### b) `LabReviewAgent` (~150 LOC)
- **Input:** encounter_id, lab_results_text, active_conditions
- **Output:** interpretation, flags, follow_up_recommendations
- **FHIR write:** DiagnosticReport (lab interpretation)
- **Mock response:** Flags abnormal HbA1c for diabetes patients, elevated BNP for CHF

#### c) `DischargePlanAgent` (~150 LOC)
- **Input:** encounter_id, active_conditions, current_medications, assessment_summary
- **Output:** discharge_instructions, follow_up_schedule, medication_reconciliation
- **FHIR write:** DocumentReference (discharge summary)
- **Mock response:** Structured discharge plan with follow-up in 2 weeks

#### d) `NurseAssessAgent` (~120 LOC)
- **Input:** encounter_id, assessment_type (admission/ongoing), vitals_text
- **Output:** nursing_assessment, fall_risk_score, pain_score, braden_score
- **FHIR write:** Observation (nursing assessment)
- **Mock response:** Risk scores based on patient age/conditions

#### e) `PharmacyReviewAgent` (~120 LOC)
- **Input:** encounter_id, pending_orders, active_medications, allergies
- **Output:** review_status, interactions_found, substitution_suggestions
- **FHIR write:** none (advisory — emits CDS cards only)
- **Mock response:** Flags warfarin+NSAID interaction for Williams, penicillin allergy for Mitchell

### 3. New API Routes

Add to `routes/mod.rs`:
```
POST /v1/encounter/:id/triage          → triage handler
POST /v1/encounter/:id/lab-review      → lab review handler
POST /v1/encounter/:id/discharge-plan  → discharge plan handler
POST /v1/encounter/:id/nurse-assess    → nurse assessment handler
POST /v1/encounter/:id/pharmacy-review → pharmacy review handler
POST /v1/simulate                      → simulation orchestrator
```

Each route follows the exact same pattern as `notes.rs`: create EventEmitter → emit AgentStarted → fetch encounter/patient → population gate → policy check → run agent → persist audit → FHIR write → create turn → check chains → emit AgentCompleted.

### 4. Mock LLM Extensions

Extend `MockClaudeCapability` in `mock_claude.rs` to pattern-match on new agent prompt keywords:
- "triage" → ESI-level JSON
- "lab review" / "interpret" → lab interpretation JSON
- "discharge" → discharge plan JSON
- "nursing assessment" → nursing assessment JSON
- "pharmacy" / "medication review" → drug interaction JSON

### 5. Policy Rules (5 new TOML files)

Each in `crates/cliniclaw-policy/policies/`:
- `triage_assess.toml` — allow for in-progress encounters
- `lab_review.toml` — allow for in-progress encounters
- `discharge_plan.toml` — allow for in-progress, require approval for IMP class
- `nurse_assess.toml` — allow for all actionable encounters
- `pharmacy_review.toml` — allow, always emit CDS cards

### 6. TUI Hospital Dashboard Mode

Add a hospital view to `cliniclaw-tui` (toggled with `h` key):

```
┌──────────────────────────────────────────────────────────────┐
│ ClinicClaw Hospital Simulation              ● Connected      │
├──────────────┬───────────────────────────────────────────────┤
│  PATIENTS    │  LIVE ACTIVITY                                │
│              │                                               │
│ ✓ Mitchell   │  14:32:01 [TR] Mitchell  ✓ Triage ESI-3      │
│ ✓ Thompson   │  14:32:01 [TR] Thompson  ✓ Triage ESI-3      │
│ ● Garcia     │  14:32:02 [TR] Chen      ✓ Triage ESI-2      │
│ ● Chen       │  14:32:03 [NA] Mitchell  ✓ Nurse assess      │
│ ○ Johnson    │  14:32:03 [AD] Thompson  ● LLM call...       │
│ ○ Williams   │  14:32:04 [LR] Garcia    ✓ Lab review done   │
│              │  14:32:05 [OE] Mitchell  ✓ Lisinopril 20mg   │
│  6 patients  │  14:32:06 [PA] Johnson   ● Prior auth...     │
│  24 turns    │  14:32:07 [PR] Williams  ✗ Warfarin flag     │
│  8 agents    │  14:32:08 [DC] Mitchell  ✓ Discharge ready   │
├──────────────┴───────────────────────────────────────────────┤
│ [s]imulate  [h]ospital/detail  [c]lear  [q]uit              │
└──────────────────────────────────────────────────────────────┘
```

- Left panel: Patient status sidebar with `✓` done / `●` active / `○` waiting
- Right panel: Unified activity feed across all patients, prefixed with [agent_abbrev] and patient name
- Agent abbreviations: TR=triage, NA=nurse, AD=ambient_doc, OE=order_entry, PA=prior_auth, LR=lab_review, DC=discharge, PR=pharmacy
- Press `s` to trigger `POST /v1/simulate` and watch it all unfold

### 7. Web Hospital Dashboard — `/hospital` Page

A new Next.js page with real-time visualization:

- **Patient swim lanes:** 6 horizontal lanes (one per patient), agent steps appear as colored blocks flowing left-to-right as events arrive
- **Agent activity counter:** total running, completed, failed — updates live
- **Start simulation button** — calls `POST /v1/simulate`
- Uses existing `useEventStream` hook with no encounter filter (receives ALL events)

### 8. Broadcast Channel Size

One-line change in `main.rs`:
```rust
let (event_tx, _) = tokio::sync::broadcast::channel::<cliniclaw_kernel::AgentEvent>(1024);
```

## Implementation Order

1. **New agents** — 5 agent structs in `cliniclaw-agents` + mock LLM extensions + policy TOML files
2. **New API routes** — 5 route handlers + simulate orchestrator in `cliniclaw-api`
3. **Broadcast channel resize** — one line in `main.rs`
4. **TUI hospital mode** — add hospital view + `s` trigger to `cliniclaw-tui`
5. **Web hospital page** — new `/hospital` page in `web/`
6. **Integration test** — run simulation, verify all 6 pathways complete
7. **Update demo.tape** — record the hospital simulation

## What We DON'T Change

- No changes to kernel (workspace, turn, event types are sufficient)
- No changes to FHIR resource types (existing resources cover all needs)
- No changes to persist layer (SQLite + audit store unchanged)
- No new crates — everything fits in existing crate structure
- No changes to SSE/event streaming infrastructure

## Estimated Scope

| Component | Files | ~LOC |
|-----------|-------|------|
| 5 new agents | 5 new + 2 modified | ~750 |
| 5 policy files | 5 new | ~100 |
| Mock LLM extensions | 1 modified | ~150 |
| 6 new route handlers | 6 new + 1 modified | ~900 |
| Simulate orchestrator | 1 new | ~200 |
| TUI hospital mode | 3 modified | ~250 |
| Web hospital page | 2 new + 1 modified | ~300 |
| Tests | across all | ~200 |
| **Total** | **~25 files** | **~2,850** |
