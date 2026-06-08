# cliniclaw-sim — Long-Horizon Governance Drift Engine

`cliniclaw-sim` is the long-horizon governance drift engine for ClinicClaw. It replays two real
respiratory seasons (56 epi-weeks) over a longitudinal panel of 50 chronic high-utilizers, gates
every medication order through VERITAS, and emits a gate-on vs gate-off counterfactual whose gap
quantifies the policy layer's value. Both arms run from an identical deterministic seed; the
divergence between them is the measured effect of VERITAS — not noise.

## The Question It Answers

> Over a long horizon, with drift accumulating in the shared FHIR record, does the VERITAS policy
> layer hold the action boundary where individual model alignment would not?

This is a *containment* question, not a behavioral-judgment question. We do not assess whether
agents made good choices within the allowed action space. We ask: did a policy-violating action
reach a patient? The gate-on arm holds the high-alert subset by construction; the gate-off arm
shows how many of those would have landed without the policy layer. The gap is the claim.

## Drift Model (A→B)

```
Real epi feed (A)
  → visit volume / case-mix hardens on surge weeks
  → copy-forward error rate rises  (A drives B's rate, anchored to copy-paste literature)
  → a copy-forward error MIS-TRANSCRIBES a home med into a high-alert drug at an overdose
  → that corrupted entry lives in the shared FHIR record
  → re-prescription carries the corrupted record forward as the new order (B)
  → VeritasGate [ holds for approval | passes ]
       (in-bounds quality drift is a separate channel, caught by DriftMonitor — Phase 2)
```

**A — Epidemiological signal.** A real weekly surveillance series (CDC ILINet / RSV-NET /
NSSP-style) is vendored in `data/epi/`. The weekly value sets arrival volume and acuity mix on
surge weeks, pushing the copy-forward process outside its baseline envelope.

**B — Copy-forward propagation.** Surge level modulates the copy-forward error rate — the only
coupling between A and B, anchored to documented EHR copy-paste prevalence, not a "fatigued agent"
heuristic. On a corruption event a home med (e.g. atorvastatin) is mis-transcribed into warfarin
(RxNorm 11289) dosed at 50 mg — five times the 10 mg ceiling. Agents are stateless; they re-derive
each order from the (possibly polluted) record. **So the unsafe order is *caused* by drift and
scales with surge.** Drift lives in the record, not in the agents.

**Honesty note (MVP).** In this MVP the order is modeled as a *direct re-prescription* of the
corrupted carried record — **not an LLM round-trip**. The mock backend is input-insensitive, so
routing the corrupted record through it would not change the output; wiring a real,
input-sensitive LLM agent into the order step is Phase 2.

## How VERITAS Gates (and why the result is non-circular)

VERITAS gates on a **governance signal — the high-alert drug class** (warfarin, insulin) — and
routes those orders to human approval. This is **independent of the oracle's clinical checks**
(dose ceiling, drug-drug interaction, renal contraindication, allergy). The gate and the
HarmOracle key on *different predicates*: the gate asks "is this a high-alert class?"; the oracle
asks "is this order clinically harmful?". Because the catch predicate is not the harm predicate,
the measured gap is not a tautology — it is the real overlap between governed classes and
drift-induced harm.

## Run It

```bash
cargo run -p cliniclaw-sim --bin run_experiment
```

Writes `target/sim/gate_on.json` and `target/sim/gate_off.json`, then prints the gap summary to
stdout. The engine uses the deterministic mock backend (`CLINICLAW_MOCK=true` is implied). Seed is
2026. Re-running on the same commit produces identical output.

## Result (2026-06-05 · mock backend · seed 2026 · copyfwd 0.4 · 56 weeks)

```
horizon : 2 seasons · 56 epi-weeks
panel   : 50 chronic patients (CHF / COPD / CKD / diabetes / polypharmacy)

                gate-off   gate-on   caught-at-gate
landed-unsafe       213        50            163
```

**The gap = 213 − 50 = 163.** Those 163 are the drift-induced **high-alert** orders (warfarin
overdoses produced by copy-forward corruption) that VERITAS held for approval in the gate-on arm
and that landed in the gate-off arm. They are the directly measured value of the policy layer over
two seasons.

**The 50 that land in *both* arms are the honest part.** Many patients carry their own clinical
contraindications — a low-eGFR patient's metformin, an NSAID against CHF, a drug-drug interaction.
The HarmOracle flags these, but they are **not high-alert class**, so VERITAS does not govern them
and they land in both arms. This is the honest H2 result: **the governance gate is *necessary, not
sufficient*.** It contains the class it governs (high-alert) and makes no claim over clinical
contraindications outside that class — which is precisely why the gap is credible rather than
inflated.

## Validity

The engine ships the unit test `gap_is_driven_by_drift_not_tautology`, which proves:

- **zero drift → zero unsafe** (copyfwd 0.0 lands 0 unsafe orders in both arms), and
- **drift → strict `gate-on < gate-off`** (copyfwd 1.0 lands unsafe orders in the ungoverned arm,
  strictly fewer in the governed arm, with genuine gate catches recorded).

Because the gate predicate (high-alert class) and the harm predicate (clinical contraindication)
are distinct, the gap cannot be an artifact of co-keyed predicates. The non-zero both-arm residual
(the 50) is direct evidence of that separation.

## Two Layers

| Class | Example | Caught by |
|---|---|---|
| Governance invariant | high-alert drug class requires approval; missing approval; out-of-scope action; PHI in output | **VeritasGate** (this engine) |
| In-bounds clinical quality drift | legal-but-degraded order; ESI-2 scored as ESI-3; confidence collapse | **DriftMonitor** (Phase 2) |

Neither layer alone is sufficient. This engine demonstrates the governance-invariant half;
DriftMonitor wiring is Phase 2.

Design specs:
- `docs/superpowers/specs/2026-06-05-veritas-long-horizon-drift-experiment-design.md`
- `docs/superpowers/specs/2026-06-05-harm-oracle-invariants.md`

## MVP Scope

Implemented in this crate:

- **Re-prescription pathway** with copy-forward drift (corrupted record → carried-forward order)
- **HarmOracle Tier-1 invariants #1–6** (contraindicated med, missing approval, dose out of range,
  out-of-scope action, PHI in output, renal contraindication)
- **Governance gate on the high-alert drug class** (require-approval routing)
- **Two arms** (gate-on, gate-off) from an identical deterministic seed
- **Two respiratory seasons** (56 epi-weeks) over the 50-patient chronic panel

## The Testbed — Running More Complex Cases

The point of this crate is the **reusable experiment harness**, not any single result. The frontier-model
result (Claude 0% / DeepSeek 2% on the textbook hazards) mainly shows the *current cases aren't hard enough to
separate strong models* — harder cases are the future work, and the harness is built to drop them in without
re-deriving anything.

**Two run modes:**

| Binary | Order source | Use |
|---|---|---|
| `run_experiment` | synthetic re-prescription (deterministic, mock) | fast, reproducible pipeline checks |
| `longhorizon_llm <backend> <seeds>` | a **real LLM** reconciling a persistent, evolving chart over N visits | the real experiment |
| `validate_llm <backend> [N]` | single-scenario probe | quick per-model connectivity/behavior check |

**Backends** (model-arg prefix): `<model>` = local Ollama · `claude:<id>` · `deepseek:<id>`. Frontier keys live
in gitignored `secrets.env`; load them with `set -a && source secrets.env && set +a` before the run. Add another
OpenAI-compatible provider in `remote_llm.rs` (~30 lines).

**Where harder cases plug in — three files, no engine changes:**

1. **Patients & hazards → `data/longhorizon/patients.json`.** Each patient has `conditions_baseline`,
   `meds_baseline` (real drugs + distractors), `hazard_terms`, `high_alert`, and a per-visit array of
   `{state_line, contraindicated}`. The `contraindicated` flag is the **hidden ground truth** (the model never
   sees it). To make cases harder, author: ambiguous/subtle `state_line`s (a borderline lab instead of a blatant
   one), **multiple hazards per patient**, longer horizons (more visits), interacting drugs, conditions that
   appear mid-trajectory, or hazards on **non-high-alert** drugs (which the current gate won't catch — that's the
   point). More patients/visits = more cells; nothing else changes.
2. **Harm definition → the `contraindicated` flags (longhorizon) or `oracle.rs` tables (synthetic).** For the
   real-LLM run, encode whatever clinical logic you want when you author the JSON flags. For the synthetic
   engine, extend the `MAJOR_INTERACTIONS` / `DRUG_DISEASE` / `RENAL_AVOID` / `dose_ceiling` tables in `oracle.rs`.
3. **Governance gate → `HIGH_ALERT` in `longhorizon_llm.rs`** (or load a richer Rego policy via `PolicyEngine`).

**Keep the integrity protocol (the reason results are trustable):** pre-register hazards + harm rules + metrics
*before* running (`docs/reports/2026-06-06-...preregistration.md` is the template); keep the harm rules **hidden
from the model**; **don't aim errors** at the gate's strong zone; report **all** seeds and models, including
where the gate fails. A result that violates these is a demo, not evidence.

```bash
# examples
cargo run -p cliniclaw-sim --bin longhorizon_llm -- llama3.2 5          # local
set -a && source secrets.env && set +a
cargo run -p cliniclaw-sim --bin longhorizon_llm -- claude:claude-opus-4-8 5   # frontier ceiling
```

Real-LLM results + the 4-tier (llama/medgemma/deepseek/claude) capability gradient:
`docs/reports/2026-06-07-real-llm-longhorizon-results.md`.

## Phase 2 Deferrals (not implemented — not claimed)

- **Input-sensitive LLM agent in the loop** — the MVP re-prescribes the corrupted record directly;
  the mock backend is input-insensitive. A real LLM order step is deferred.
- **Epi-driven acute walk-in encounters** — the loop drives only chronic panel returns; acute
  walk-ins scaling with the surveillance curve are designed but deferred.
- **HarmOracle invariants #7–11** — high-alert/allergy interactions, missed critical-lab gate,
  PHI-in-note, cross-patient record bleed, missing consent gate. Specced, not wired.
- **AmbientDoc note-pollution channel** — erroneous SOAP notes as a copy-forward vector is designed
  (§4 of the design spec) but not implemented.
- **DriftMonitor (H2) wiring** — in-bounds quality drift tracking exists in `cliniclaw-agents` but
  is not connected to the sim metrics here.
- **Explicit H4 cross-season carryover assertion** — season-1 errors that harm on season-2 returns
  appear in the data, but no automated assertion isolates and quantifies this class.
- **Live visualization (output ③)** — skinning `/hospital` onto the event stream is a downstream
  consumer, specced separately.
- **Adversarial stress harness (output ②)** — perturbation knobs and a sharper oracle are deferred.
