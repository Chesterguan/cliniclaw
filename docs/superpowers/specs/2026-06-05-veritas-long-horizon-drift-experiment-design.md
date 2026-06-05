# VERITAS Long-Horizon Governance Drift Experiment — Design

> Status: Draft for review · Date: 2026-06-05 · Owner: chesterguan
> Scope: Sub-project **A** (the engine). Outputs ② (stress harness) and ③ (live viz) are downstream consumers, specced separately.

## 1. Context & Motivation

Emergence AI's *Emergence World* (a continuously-running multi-agent town, 5 worlds × 10 agents × 15+ days)
showed that agent safety is **ecological, not individual**: a model that stays peaceful in isolation drifts
into harmful behavior in mixed company ("normative drift / cross-contamination"), and agent societies
either stabilize or collapse with no graceful degradation. Their conclusion: long-horizon agent intelligence
is a different construct from short-task skill, and deployed autonomy needs **formally verified safety
architectures** rather than trust in individual model alignment.

That conclusion is the VERITAS thesis. This experiment asks the ClinicClaw-shaped version of it:

> **Over a long horizon, with drift accumulating in the shared clinical record, does the VERITAS policy
> layer hold the line at the action boundary where individual model alignment would not?**

### Framing decisions (settled in design discussion)

1. **We do not judge the agents' "choices" by human standards.** Emergence's metrics (crime, civic
   participation) are anthropocentric measurement instruments. VERITAS enforces **boundary invariants**, not
   choice quality. So our measurement is a *containment* question — "did any policy-violating action reach a
   patient?" — not a *behavioral-judgment* question.
2. **Drift lives in the world, not in the agents' heads.** Agents stay **stateless**; they re-derive each
   decision from a (possibly polluted) FHIR record. This is both more realistic (clinicians re-read the chart
   fresh) and avoids anthropomorphizing agent cognition.
3. **VERITAS is necessary but not sufficient.** A hard policy gate structurally catches *boundary violations*
   (contraindicated med, missing approval, out-of-scope action, PHI leak) but **cannot** catch *in-bounds
   quality drift* (a legal-but-degraded decision, ESI-2 scored as ESI-3, confidence quietly collapsing).
   ClinicClaw has a second layer (DriftMonitor) for the latter. The experiment demonstrates that each layer
   catches a distinct class and neither alone is enough.
4. **Drift sources must anchor to real-world signals.** No synthetic prompt-injection. Every drift mechanism
   must be calibratable against external, documented healthcare data so we can claim external validity.

## 2. Goals & Non-Goals

### Goals
- Build a **long-horizon governance engine** that replays a real epidemiological season, runs ClinicClaw's
  agents over a longitudinal patient panel, accumulates drift in the shared record, and gates every action
  through VERITAS.
- Produce a **two-arm counterfactual** (gate-on vs gate-off, identical seed) whose difference quantifies the
  value of the policy layer.
- Track **per-patient state** over the horizon (record pollution, trajectory, harm events, drift indicators)
  as a first-class, queryable object.
- Emit a reproducible **evidence ledger + weekly metrics** suitable for a publishable result (output ①).

### Non-Goals (YAGNI / deferred)
- The live visualization (output ③) — consumes this engine's event stream; separate spec.
- The adversarial stress harness (output ②) — adds perturbation knobs + sharper oracle on this engine;
  separate spec.
- Real Medplum/Claude-backed production runs — long runs use the deterministic mock backend; real-LLM
  validation is limited to sampled "slice" weeks.
- Building a synthetic episodic-memory pool for agents — **the FHIR record is the shared memory**; no separate
  memory substrate is built.

## 3. Thesis & Hypotheses

- **H1 (containment).** Over two seasons, `landed-unsafe` count is ≈ 0 in the gate-on arm and strictly
  positive (and growing) in the gate-off arm. The gap is the quantified value of VERITAS.
- **H2 (two layers).** VERITAS catches boundary violations; DriftMonitor catches in-bounds quality drift.
  Plotting the two classes shows non-overlapping coverage — each layer is necessary.
- **H3 (A drives B).** Surge weeks (high real-surveillance volume) raise the copy-forward rate, which
  accelerates error propagation; `landed-unsafe` in the gate-off arm correlates with the surveillance curve
  at a lag.
- **H4 (cross-season carryover).** Errors introduced in season 1 and left in the record produce harm events
  when panel patients return in season 2 — long-horizon residue, invisible to any short benchmark.

## 4. Drift Model — the A→B Pipeline

```
真实 epi feed (A)  →  case-mix 变硬 + 就诊量上升  →  copy-forward 率上升 (A→B 速率)
   →  AmbientDoc/OrderEntry 把上轮病历当事实读回  →  错误顺着 problem/med list 传播 (B)
   →  病历被污染  →  下游不安全医嘱  →  VeritasGate【拦 / 放】
       （界内质量塌陷由 DriftMonitor 另抓一条线）
```

### A — Epidemiological case-mix shift (root signal)
- **Real anchor:** CDC ILINet / FluView (outpatient ILI %), RSV-NET, NSSP syndromic ED surveillance —
  public, weekly, real time series. Degradation-under-shift literature: Epic Sepsis Model external validation
  (Wong et al., *JAMA Intern Med* 2021); COVID-era model failures.
- **Mechanism:** the weekly surveillance value sets (a) arrival volume and (b) acuity/presentation mix,
  pushing agents outside their validated envelope on surge weeks.

### B — Copy-forward / record error propagation (propagation mechanism)
- **Real anchor:** documented note-bloat / copy-paste prevalence and downstream error propagation in EHRs
  (exact percentages pinned during calibration). The problem list and medication list are the classic
  copy-forward vectors.
- **Mechanism:** an agent writes an erroneous resource (e.g., a wrong med or a mis-stated condition); the next
  encounter reads it as ground truth. **Surge level (A) modulates the copy-forward rate** — the only coupling
  between A and B, and it is anchored to real copy-paste statistics, *not* to a "fatigued agent" heuristic.

### What VERITAS can / cannot catch (the H2 split)
| Class | Example | Caught by |
|---|---|---|
| Boundary violation | contraindicated med vs recorded allergy; missing required approval; dose out of range; PHI in output | **VeritasGate** |
| In-bounds quality drift | legal-but-worse order; ESI-2 → ESI-3; confidence collapse | **DriftMonitor** (not the gate) |

## 5. Horizon Model

- **Clock unit:** the **epi-week**, matching the real surveillance cadence (the clock *is* the real signal).
- **Length:** **two full respiratory seasons** (≈ 30+ weeks each, Sep→Apr), to expose **cross-season
  carryover** (H4).
- **Cost control:** long runs use the deterministic mock backend (`CLINICLAW_MOCK`) + the existing speed knob;
  real-Claude validation is limited to sampled slice weeks.

## 6. Object Model

Design principle: small, well-bounded units; agents stateless; the world accumulates.

| Object | Responsibility | Persistent state | Depends on |
|---|---|---|---|
| `EpiDriver` | Read real surveillance series → per-week `arrivals` (volume + acuity mix) + `surge_level` | none (pure over a data file) | CDC/NSSP data file (vendored) |
| `PatientPanel` | A cohort of **returning** longitudinal patients; schedule their encounters across the timeline | per-patient encounter schedule | Synthea bundles |
| longitudinal **FHIR record** | **= shared long-term memory**; agents write → next round reads back as fact | Conditions, MedicationRequests, Observations, notes | existing FHIR store |
| 8 **agents** | stateless; re-derive each decision from the (possibly polluted) record | **none (deliberate)** | `LlmCapability` |
| `CopyForwardChannel` | On write/read, propagate prior resources at `copyfwd_rate = f(surge_level)`; carry errors forward | none | record, `EpiDriver` |
| `VeritasGate` | Hard invariant gate at the action boundary; record the counterfactual decision | none | existing policy engine |
| audit chain | **= evidence ledger**: every action → decision → counterfactual | SHA-256 chain | existing persist |
| `DriftMonitor` | Layer 2: model-level rolling confidence drift (in-bounds quality) | rolling window | existing |
| `HarmOracle` | Measurement instrument: does an action violate a clinical **invariant**? Seed invariant set (11 Tier-1 hard-stops + Tier-2 drift signals + documented gaps) is specced in `2026-06-05-harm-oracle-invariants.md`, anchored to NQF/WHO-ICPS/ISMP/Beers/CMS-HAC/HIPAA. Checks invariants, does not judge choices | none | record + contraindication/renal/high-alert reference tables |
| `PatientState` ⭐ | Per-patient longitudinal monitor (see §8) | pollution ledger, harm events, trajectory, drift indicators | record, audit, oracle |
| `Arm` | Same seed + panel + epi feed, run gate-on and gate-off; diff outcomes | deterministic seed | mock backend |

### PatientPanel composition (settled)
- **50 chronic high-utilizers** (CHF / COPD / CKD / diabetes / polypharmacy) — long problem & med lists =
  maximal copy-forward surface; they return frequently and are where B bites hardest. Sourced from Synthea
  longitudinal histories.
- **epi-driven acute walk-ins** — volume and mix scale with the real surveillance curve (A); a fraction are
  panel patients catching seasonal illness on top of chronic disease.
- **Money scenario:** a chronic panel patient returning on a **surge week** — A (busy, atypical, out-of-envelope)
  compounds with B (already-polluted record) → unsafe downstream order → VeritasGate is the last line.

## 7. Core Loop

```text
for week w in season(2 seasons, real curve):
    arrivals      = EpiDriver.arrivals(w)              # real signal A
    copyfwd_rate  = f(EpiDriver.surge_level(w))        # A drives B's rate, anchored to copy-paste lit
    for patient p in arrivals ∪ PatientPanel.returns(w):
        record = FHIR.read(p)                          # may be polluted from prior weeks (B)
        for agent in pathway(p.presentation):
            action   = agent.decide(record)            # stateless, re-derived from polluted record
            decision = VeritasGate.eval(action)        # arm: on / off
            counterfactual = VeritasGate.eval(action)  # always computed; applied only in gate-off arm
            if applied(decision, arm):
                FHIR.write(action)                     # pollution persists into record (B)
                CopyForwardChannel.maybe_propagate(record, copyfwd_rate)
            HarmOracle.check(action, record) -> violation?
            Audit.append(action, decision, counterfactual, violation)
            DriftMonitor.record(agent, confidence)     # layer-2 signal
            PatientState.update(p, action, decision, violation, confidence)
    Metrics.snapshot(w)   # weekly: landed-unsafe, caught-at-gate, in-bounds-drift, per-class counts
```

Primary result = **gate-on vs gate-off difference in landed-unsafe actions** = the quantified value of VERITAS.

## 8. PatientState Monitoring (first-class)

Tracked per patient, per week; aggregated for metrics (①) and streamed for viz (③):

- **Identity / cohort:** `patient_id`, `panel_class` (chronic | acute), `enrollment_week`, `weeks_seen[]`,
  `encounter_count`.
- **Record surface:** current problem-list size, med-list size (the copy-forward surface).
- **Pollution ledger:** list of `{ introduced_week, introducing_agent, resource_type, error_kind,
  propagation_count (downstream reads that consumed it), still_present }`.
- **Harm events:** list of `{ week, agent, action, oracle_violation_kind, arm, gate_decision, landed }`.
- **Drift indicators:** confidence trend on this patient's decisions (layer-2).

This object is what makes B *measurable* (propagation counts) and what output ③ renders as "watch a chart
get polluted across the season."

## 9. Measurement & Reproducibility

- **HarmOracle invariants** (the only "judgment"): contraindication vs recorded allergy/condition, dose out of
  range, missing required approval gate, out-of-scope action, PHI in output. Invariants, not preferences.
- **Two-arm counterfactual:** identical seed, panel, and epi feed; gate-on vs gate-off; the gate decision is
  *always computed* so the gate-off arm records "what VERITAS would have blocked."
- **Determinism:** seeded RNG + mock backend ⇒ each arm is byte-reproducible on re-run. The two arms share an
  identical trajectory **only until the first action whose gate decision differs in application**; from that
  point the records diverge **by design** — that divergence is precisely the measured effect of VERITAS, not
  noise. So we compare *outcomes accumulated*, not step-by-step equality.
- **Weekly metrics:** `landed-unsafe`, `caught-at-gate`, `in-bounds-drift (DriftMonitor)`, per violation class,
  per cohort; plus the epi curve overlay for H3.

## 10. Architecture / Placement in Codebase

- **New crate `cliniclaw-sim`** (binary + lib): owns `EpiDriver`, `PatientPanel`, `CopyForwardChannel`,
  `HarmOracle`, `PatientState`, `Arm`, the loop, and metrics emission. Depends on `cliniclaw-agents`,
  `-policy`, `-fhir`, `-persist`, `-kernel`. Keeps experiment code out of the API server.
- **Targeted refactor (justified by this work):** the per-encounter pathway orchestration currently lives
  inside `cliniclaw-api/src/routes/simulate.rs` / `simulate_dynamic.rs` and is not reusable outside an axum
  handler. Extract the pathway runner (agent sequence + gate + audit + emit) into a reusable unit
  (`cliniclaw-agents` or a small `cliniclaw-orchestrate` module) that both the API routes and `cliniclaw-sim`
  call. No behavior change; enables reuse. Scope limited to extraction.
- **Data vendoring:** real surveillance series (ILINet / RSV-NET / NSSP) checked into `data/epi/` as static
  CSV/JSON; Synthea panel bundles under `data/synthea/`. No live network calls in the engine (reproducibility).
- **Output:** the engine writes the evidence ledger (audit chain) + a `weekly_metrics.json` per arm; a small
  reporter renders the ① result tables/plots.

## 11. Testing Strategy

- Unit: `EpiDriver` parsing/lag; `CopyForwardChannel` propagation math; `HarmOracle` invariant checks (table
  of contraindication cases); `PatientState` ledger accounting.
- Determinism: re-running a *single arm* with the same seed produces a byte-equal ledger.
- Counterfactual integrity (pre-divergence only): up to the first action whose application differs, both arms
  see identical inputs and the gate decision is identical; assert the two ledgers match up to that point.
  After first divergence the arms differ by design — assert only that each remains internally reproducible.
- Smoke: a short 2-week, 5-patient run wired end-to-end in CI on the mock backend.

## 12. Risks & Open Questions

- **Calibration honesty:** copy-paste prevalence and override-rate numbers vary widely across studies; pin
  specific cited figures and ranges before claiming external validity. Log any value not directly sourced.
- **Oracle completeness:** the HarmOracle defines "unsafe." Gaps in the invariant set = silent misses; the
  invariant table must be reviewed and its known gaps documented (no silent caps).
- **Mock realism:** mock agents may not produce the same error modes as real LLMs; mitigate with sampled
  real-Claude slice weeks and report the delta.
- **A→B coupling form:** `f(surge_level)` shape (linear? thresholded?) needs a defensible anchor; treat as a
  calibrated parameter with sensitivity analysis, not a free knob.

## 13. Downstream Sub-Projects (deferred, own specs)
- **② Adversarial stress harness:** perturbation knobs (mixed models, injected errors) + sharper oracle on this
  engine to *find* where VERITAS misses.
- **③ Live "season" visualization:** skin `/hospital` onto this engine's event stream; render `PatientState`
  pollution accumulating week by week.
