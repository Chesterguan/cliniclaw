# Can You Trust an AI Doctor Over Time? I Built a Lab to Find Out

*We benchmark AI on single questions. But real systems run for a long time — and that's where trust actually gets tested. Here's what happened when I let language models practice medicine for a simulated year, and graded them with the same rulebooks a hospital uses.*

---

A few weeks ago I read about [Emergence's "long-horizon" autonomy lab](https://www.emergence.ai/blog/emergence-world-a-laboratory-for-evaluating-long-horizon-agent-autonomy) — agents acting not for one turn, but over hundreds of steps, where tiny errors compound into something nobody signed up for. It stuck with me, because it pointed at a question almost no benchmark asks.

We evaluate AI the way we give a pop quiz: one question, one answer, grade it, move on. But the systems we're actually deploying don't live in single questions. They run. They accumulate state. They read their own past work and build on it. **Does safety survive the long horizon — or does it quietly erode in a way a one-shot test would never catch?**

In medicine, that's not a philosophical question. So I built a small lab to find out.

## The experiment

One synthetic patient. Twelve visits over a simulated year. A real language model plays the doctor: at each visit it reads the chart and reconciles the patient's medications — continue, adjust, hold, start.

Three things make it a *long-horizon* test rather than a quiz:

1. **The model's own past orders carry forward.** What it prescribed last visit becomes the chart it reads next visit. It is, in effect, trusting its earlier self.
2. **The patient drifts.** Kidney function declines. Potassium climbs. Clotting times creep up. A drug that was perfectly safe at visit one becomes dangerous by visit eight — and nobody flags it.
3. **Nothing is injected.** I never tell the model "now make a mistake." The drift is real and clinical; the model is entirely free to catch it.

To grade it, I used a hidden judge: about fifty contraindication rules transcribed verbatim from published authorities — the AGS Beers Criteria, STOPP/START, FDA drug labels — each carrying its citation. The harm definition comes from medicine, not from me. And critically, **the model never sees these rules.** It's only asked to do its job.

Then I ran every case two ways: once with an external governance gate that holds high-risk orders for human approval, and once without. The gap between those two worlds is the measurement.

## The first lesson was about me, not the model

My first result looked fantastic. It was also a lie.

I had accidentally designed the test so the gate was *guaranteed* to win — the thing I was measuring and the thing the gate caught were secretly the same thing. I caught it, rebuilt it, and then caught myself again: the new version used textbook cases so easy that the strong models scored a perfect zero. A perfect zero looks like "AI is safe." It actually means "my test was too weak to tell me anything."

That is the real trap in AI safety evaluation. **The hard part isn't measuring the model. It's building an environment that can't lie to you.** Pre-register the design. Hide the answer key. Don't aim the errors at the gate's strengths. Let published authorities define harm, not your own judgment. Report everything — including a clean "no-hazard" control to catch your judge over-flagging, and including every place the gate fails.

That discipline, not any single number, turned out to be the actual product.

## What we found: the staircase breaks

On easy, textbook hazards, you get exactly the picture everyone expects. Small local models fail often. Bigger, better models fail less. The frontier models barely fail at all. A clean descending staircase — more capability, more safety.

Then I ran the hard cases: multiple interacting problems in one patient, borderline lab values, drugs that only turn dangerous as the patient changes over time.

**The staircase breaks.**

![The staircase breaks](./assets/01_staircase_breaks.png)
*Left: textbook hazards — the clean capability staircase. Right: hard cases — the order scrambles. The strongest model is not the safest. Models left→right by capability.*

The ranking scrambles. The most capable model was *not* the safest. A small medical model and a frontier model land in the same neighborhood. Errors cluster by *case type*, not by model strength — which means you cannot read a model's safety off its general capability. They are different axes.

## The drift a single-shot test never sees

Here is one of the cracks, in detail. A patient on a blood thinner. Over the year, their clotting time (INR) climbs steadily past the safe ceiling into dangerous territory. The right move is obvious to any clinician: hold or cut the dose.

The model kept renewing it. Visit after visit. Not because it's stupid — because at each visit it re-derived the plan from its own prior orders, and the danger built up too gradually to trip an alarm.

![Long-horizon drift](./assets/03_longhorizon_drift.png)
*One patient, one drug, twelve visits. The harm accumulates slowly and persists — exactly the failure mode a one-shot benchmark is blind to.*

This is the whole point of testing over a horizon. No single visit looks like a catastrophe. The catastrophe is the *trajectory*.

## The gate: necessary, not sufficient

So does an external governance gate help? Yes — but only in a specific, honest way.

![The gate is necessary, not sufficient](./assets/02_necessary_not_sufficient.png)
*The gate cleanly contains the high-alert class it governs (the frontier model's warfarin drift, 66→30). Where a model's errors fall on ordinary drugs, the gate can't see them.*

The gate is built to hold a defined class of high-alert drugs for human sign-off. When the frontier model drifted on exactly that class — the blood thinner — the gate caught it cleanly. But when a model's errors fell on ordinary drugs the gate doesn't govern, they sailed straight through.

That's the honest conclusion: **a governance gate is necessary, but not sufficient.** It contains the class it's responsible for — verifiably, and independently of which model you plugged in — and it makes no claim about the rest. The ordinary-drug mistakes still need a separate clinical-rules layer. Anyone selling you a single gate as "AI safety solved" is selling the wrong story.

## The idea I keep coming back to

"Is this model safe?" is the wrong question.

Safety isn't a number you stamp on a model. It's a **surface** — model capability along one axis, problem difficulty along the other. Any single benchmark score is a projection of that surface onto a single point, and the projection hides the cliff. The same model, honestly measured, is "100% safe" or "visibly unsafe" depending only on which problems you chose to show it.

Which is why "we tested it and it passed" is such dangerous comfort. Your test set quietly decided the answer — and you can't enumerate every hard case the real world will hand you.

When you can't verify the intelligence, you verify the boundary.

## Why I believe it

One more thing, because it's the part that earns the rest.

I built this twice. Once as a standalone engine. Then I rebuilt it from scratch inside a completely different framework, with a different language driving the model in the loop. Two independent implementations, no shared code in the critical path.

Same numbers. Same failure structure — down to *which* patients broke and *how*. Two roads, one destination. That convergence is what lets me trust the finding rather than my own cleverness.

## Where this goes

It's open source now as `veritasbench-longitudinal` — a long-horizon companion to a governance benchmark I've been building. It is deliberately early and small. The contribution isn't a leaderboard; it's a method, and the honesty rails baked into it: pre-registration, a hidden and authoritative answer key, a clean control, and full reporting of where the gate fails.

If you build or deploy AI in anything that runs longer than a single request — and almost everything does — this failure mode is waiting for you, and your one-shot evals won't show it. I'd genuinely value your eyes on the method.

Evidence over intelligence. Control over autonomy.

---

*Built on synthetic patients and published clinical criteria — no real patient data. This is a measurement harness, not a clinical product or medical advice.*
