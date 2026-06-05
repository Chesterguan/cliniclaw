# cliniclaw-sim — Long-Horizon Governance Drift Engine

`cliniclaw-sim` is the long-horizon governance drift engine for ClinicClaw. It replays two real
respiratory seasons (56 epi-weeks) over a longitudinal patient panel of 50 chronic high-utilizers,
gates every medication order through VERITAS, and emits a gate-on vs gate-off counterfactual whose
gap quantifies the policy layer's value. Both arms run from an identical deterministic seed; the
divergence between them is the measured effect of VERITAS — not noise.

## The Question It Answers

> Over a long horizon, with drift accumulating in the shared FHIR record, does the VERITAS policy
> layer hold the action boundary where individual model alignment would not?

This is a *containment* question, not a behavioral-judgment question. We do not assess whether
agents made good choices within the allowed action space. We ask: did any policy-violating action
reach a patient? The gate-on arm answers "no" by construction; the gate-off arm shows how many
would have landed without the policy layer. The gap is the claim.

## Drift Model (A→B)

```
Real epi feed (A)
  → case-mix hardens + visit volume rises
  → copy-forward error rate rises  (A drives B's rate, anchored to copy-paste literature)
  → AmbientDoc / OrderEntry read the prior chart as fact
  → errors propagate through the problem list and medication list (B)
  → FHIR record is polluted
  → downstream unsafe orders are generated
  → VeritasGate [ blocks | passes ]
       (in-bounds quality drift is caught by DriftMonitor on a separate channel)
```

**A — Epidemiological signal:** a real weekly surveillance series (CDC ILINet / RSV-NET /
NSSP-style) is vendored in `data/epi/`. The weekly value sets arrival volume and acuity mix,
pushing agents outside their validated envelope on surge weeks.

**B — Copy-forward propagation:** surge level modulates the copy-forward error rate — the only
coupling between A and B, anchored to documented EHR copy-paste prevalence statistics, not to a
"fatigued agent" heuristic. Agents are stateless; they re-derive each decision from a (possibly
polluted) FHIR record. Drift lives in the record, not in the agents.

**VERITAS gates at the action boundary.** In-bounds quality drift (a legal-but-degraded order,
confidence quietly collapsing) is a distinct class caught by `DriftMonitor` (Phase 2), not by the
policy gate. The two layers cover non-overlapping failure modes.

## Run It

```bash
cargo run -p cliniclaw-sim --bin run_experiment
```

Writes `target/sim/gate_on.json` and `target/sim/gate_off.json`, then prints the gap summary to
stdout.

The engine uses the deterministic mock backend (`CLINICLAW_MOCK=true` is implied). Seed is 2026.
Re-running on the same commit produces identical output.

## Result (2026-06-05, mock backend, seed 2026)

```
horizon       : 2 seasons · 56 epi-weeks
panel         : 50 chronic patients (CHF / COPD / CKD / diabetes / polypharmacy)
renal-at-risk : 16 patients with low-eGFR (metformin contraindicated)

arm           gate-off   gate-on   caught-at-gate
landed-unsafe      90         0            90

VERITAS prevented 90 unsafe actions from reaching patients.
```

**Drift mechanism:** the real epi surveillance series drives the surge level; surge raises the
copy-forward error rate; errors propagate into the problem and medication lists; the HarmOracle
flags renally-contraindicated metformin orders for the 16 low-eGFR patients; VERITAS denies those
orders in the gate-on arm and lets them through in the gate-off arm. The 90-unit gap is the
directly measured value of the policy layer over two seasons.

## Two Layers

| Class | Example | Caught by |
|---|---|---|
| Boundary violation | contraindicated med vs renal function; missing approval; dose out of range; PHI in output | **VeritasGate** (this engine) |
| In-bounds quality drift | legal-but-degraded order; ESI-2 scored as ESI-3; confidence collapse | **DriftMonitor** (Phase 2) |

Neither layer alone is sufficient. This engine demonstrates the boundary-violation half. DriftMonitor
wiring is Phase 2 work.

Design specs:
- `docs/superpowers/specs/2026-06-05-veritas-long-horizon-drift-experiment-design.md`
- `docs/superpowers/specs/2026-06-05-harm-oracle-invariants.md`

## MVP Scope

What is implemented in this crate:

- Medication order pathway (OrderEntry agent through the gate)
- HarmOracle Tier-1 invariants #1–6 (contraindicated med, missing approval, dose out of range,
  out-of-scope action, PHI in output, renal contraindication)
- Two arms (gate-on, gate-off) from identical deterministic seed
- Two respiratory seasons (56 epi-weeks) over the 50-patient chronic panel

## Phase 2 Deferrals (not implemented)

The following items are explicitly out of scope for this MVP:

- **Epi-driven acute walk-in encounters** — the loop currently drives only chronic panel returns.
  Acute walk-ins (volume and mix scaling with the surveillance curve) are designed but deferred.
- **HarmOracle invariants #7–11** — Tier-1 high-alert drug/allergy interactions, missed critical
  lab gate, PHI-in-note, cross-patient record bleed, missing consent gate. Specced in
  `docs/superpowers/specs/2026-06-05-harm-oracle-invariants.md`; not yet wired.
- **AmbientDoc note-pollution channel** — erroneous SOAP notes as a copy-forward vector is
  designed (see §4 of the design spec) but not implemented in the current loop.
- **DriftMonitor (H2) wiring** — in-bounds quality drift tracking is present in `cliniclaw-agents`
  but is not connected to the sim metrics in this MVP.
- **Explicit H4 cross-season carryover assertion** — errors introduced in season 1 that produce
  harm events on season-2 returns are expected to appear in the output data, but no automated
  assertion isolates and quantifies this class separately.
- **Live visualization (output ③)** — skinning `/hospital` onto the engine's event stream is a
  downstream consumer, specced separately.
- **Adversarial stress harness (output ②)** — perturbation knobs and sharper oracle on this
  engine are deferred to a separate spec.
