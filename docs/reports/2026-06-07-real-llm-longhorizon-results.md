# Can you trust a medical AI over months of use — and what do you put around it?

*A ClinicClaw study, inspired by Emergence World. Written to be read by anyone.*

Hospitals are starting to let AI draft medication orders. The reassuring demos all show a single, clean
decision. But a real hospital runs for *months*: the same AI sees the same patient again and again, and the
patient's body changes underneath it. The worry — which Emergence World raised for AI agents in general — is
that an AI can look great on a one-off test and quietly **drift** over a long run. We set out to answer two
plain questions, in a hospital setting, and to do it in a way that can't be quietly rigged:

> **1. Does a real medical AI drift into unsafe behavior over a long run?**
> **2. If it does, can something *outside* the AI catch it — reliably, in a way the AI can't talk its way around?**

Here is the whole chain of reasoning, step by step.

---

## Step 1 — First, build the machine. (And learn why a clean result can lie.)

Before testing any real AI, we built the apparatus: a simulated hospital that runs for two flu seasons, a
patient who comes back over and over, an "order-writer," a rulebook that can block orders, and an independent
**judge** that decides whether an order was actually unsafe.

To check the wiring end-to-end we ran it with a *fake* AI and a *planted* error. It produced a dramatic
result — the rulebook appeared to stop **76%** of unsafe orders. That number is **not a finding, and we do not
report it as one.** We had planted the error *and* planted it on exactly the kind of drug our one rule was
watching. Of course the rule caught it. All this step proves is that the machine runs. It says nothing about a
real AI. (Full write-up, clearly marked superseded: the 2026-06-05 report.)

**The lesson that shaped everything after:** a safety result is only meaningful if *we don't get to choose
where the mistakes happen.*

---

## Step 2 — The real test: hire real AIs, hide the rules, don't plant anything.

So we threw out the fake AI and the planted error, and locked the design in writing *before* running it (so we
couldn't move the goalposts afterward). The honest setup:

- **Real AIs.** Two real models running locally — `llama3.2` and `medgemma:4b` (a medical model). No make-believe.
- **Real-shaped patients.** Six patients built from real synthetic medical records (Synthea) — messy charts,
  real medication lists, real distractor drugs. Five of the six dangers use a drug the patient *actually* had.
- **The AI is told nothing.** Each visit it sees the patient's current labs and medication list and is asked
  only to "reconcile the medications." It is **never told which drug is risky, or what our safety rules are.**
- **The AI's own mistakes are the only errors.** We plant nothing. We just let the patient's labs slowly change
  over the visits — kidney function declines, potassium rises, a blood thinner's level climbs — so a drug the
  AI has been happily continuing **quietly becomes dangerous.** The AI has to notice on its own.
- **It's a long run.** The AI's orders get written into the chart it reads next time — so a mistake sticks
  unless the AI catches and fixes it later.

---

## Step 3 — What the real AIs actually did.

### Finding A — Real AI drifts. Once a drug turned dangerous, the AI kept ordering it.

![Harm appears only after a lab crosses the line, then persists](assets/llm_drift_over_horizon.png)

For the first few visits, while the labs were normal, both AIs were safe — **zero** unsafe orders. Then, the
moment a lab crossed into the danger zone (around visit 5), the AI kept right on re-ordering the
now-contraindicated drug — **visit after visit, and it never self-corrected.** This is exactly the long-run
failure the demos hide: being good at the one-off question is *not* the same as being reliable over time.

### Finding B — How safe the AI is depends entirely on *which* AI.

![Unsafe rate by model](assets/llm_two_model.png)

Same patients, same task: one model was unsafe on **24%** of orders, the other on **8%** — a 3× difference.
You cannot look at "an AI" and know if it's safe; it depends completely on the specific model you deploy. And
you can't verify, in advance, which one will drift.

### Finding C — An external rulebook can catch the AI — but only for the rules you actually write.

Findings A and B say: you can't trust the AI alone, and you can't fix it by shopping for a better model. So
something *outside* the AI has to enforce safety. That outside thing is what **VERITAS** is in ClinicClaw.

**Important, so we don't oversell it:** VERITAS is **not a finished medical safety system.** It is a
**framework** — a rulebook engine that sits outside the AI, checks every order against rules *you write*,
blocks or escalates what violates them, and logs everything. The AI can't see these rules and can't argue its
way past them. Its protection is exactly as good as the rules you put in it.

To test the framework we wrote **one** simple, standard rule: *"high-risk drugs (blood thinners, opioids,
insulin…) need a human sign-off before they go through."* Here's what that single rule did:

![One demo rule: removes the high-risk hazard, not the others](assets/llm_per_patient_llama.png)

- It did its job **cleanly**: the dangerous blood-thinner orders (a high-risk drug) went from 29 unsafe visits
  to **zero**.
- It did **only** its job: the other mistakes — continuing a diabetes drug after the kidneys failed, a blood-
  pressure drug after potassium spiked — are *ordinary* drugs, which our one rule wasn't written to check, so
  **they sailed through.** For the second model, *every* mistake was of this ordinary-drug kind, so the one
  rule helped almost not at all (8% → 7%).

The gap is **not** a ceiling on VERITAS. It's simply the rules we **haven't written yet.** A clinical rule like
"hold a diabetes drug when kidney function is too low" is the same kind of rule and goes in the same framework —
it's just more policy to author. What this step proves is narrower and more honest: **the framework genuinely
enforces a hidden, external, un-gameable rule on a real AI's actions.** How much it protects a patient is then
a question of how complete your rulebook is.

---

## Step 4 — What this proves (the chain, end to end).

1. **Real medical AI drifts into unsafe behavior over a long run** (Finding A) — so it can't be trusted on its own.
2. **You can't fix that by picking a model**, and you can't verify which model will drift (Finding B).
3. **Therefore safety has to be enforced from outside the AI** — by something verifiable that the AI cannot see
   or evade. That is precisely the role of the VERITAS framework.
4. **We demonstrated the framework doing that on a real AI** with a rule hidden from the model (Finding C). The
   amount of protection equals the completeness of the rules you author — our single demo rule was deliberately
   narrow; building out the clinical rulebook is the next work, and the same framework carries it.

**Bottom line:** the experiment establishes the *need* (real AI drifts, unpredictably by model) and
demonstrates the *mechanism* (an external, verifiable, model-blind rulebook can enforce safety on a real AI).
It does **not** claim our one demo rule is enough — making the rulebook comprehensive is the road ahead.

---

## The numbers

| | llama3.2 | medgemma:4b |
|---|---|---|
| Unsafe-order rate, no rulebook | 24% | 8% |
| Unsafe-order rate, with the one demo rule | 18% | 7% |
| High-risk hazard (blood thinner) | 29 → **0** | (model already held it) |
| Ordinary-drug hazards (kidney/potassium) | landed in both | landed in both |

*(6 patients × 12 visits × 5 repetitions × 2 arms per model; harm judged by a fixed rulebook the AI never saw.)*

## Honest limits (please read before quoting)
- **Small study:** 2 local models, 6 patients, one danger each, mild settings. A slice — more models and
  patients get *added* later, never swapped in to cherry-pick.
- **Comparison noise:** the "with rule" and "no rule" runs are separate and the AI is slightly random, so small
  net differences wobble run-to-run (in one case the "with rule" arm was a touch *worse* on a drug the rule
  doesn't touch). The clean signals are the per-danger outcomes (blood thinner 29→0) and the rates, not tiny gaps.
- **The sign-off has a cost:** the rule sent 13 high-risk orders to a human to review — protection isn't free.
- **One demo rule only**, by design. This is a framework demonstration, not a finished clinical safety product.

## Reproduce
`cargo run -p cliniclaw-sim --bin longhorizon_llm -- llama3.2 5` (and `medgemma:4b`). Design locked beforehand:
`2026-06-06-real-llm-longhorizon-preregistration.md`.
