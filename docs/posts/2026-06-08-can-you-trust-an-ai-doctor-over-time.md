# What happens when you let an AI doctor practice for a year?

A few weeks ago I read about Emergence's "long-horizon" autonomy lab — agents acting not for one turn, but over hundreds of steps, where small errors compound into something you didn't sign up for. It stuck with me, because it pointed at a question almost nobody benchmarks:

**We test AI on single questions. But real systems run for a long time. Does safety survive the long horizon?**

In medicine that question isn't academic. So I built a small lab to find out.

---

**The experiment**

One synthetic patient. Twelve visits over a simulated year. A real language model plays the doctor — at each visit it reads the chart and reconciles the medications. The catch: the model's own past orders carry forward into the next visit, and the patient's labs quietly drift. Kidney function declines. Potassium climbs. The INR creeps up.

Nothing is injected. No one tells the model "now make a mistake." The drift is real, and the model is free to catch it — or not.

To grade it, I used a hidden judge: ~50 contraindication rules transcribed verbatim from published authorities — the AGS Beers Criteria, STOPP/START, FDA labels — each with a citation. The model never sees these rules. It's only asked to do its job.

Then I ran it two ways: with an external governance gate that holds high-risk orders for human approval, and without. The difference between those two worlds is the measurement.

---

**The first lesson was about me, not the model**

My first result looked great. It was also a lie. I'd accidentally designed the test so the gate was guaranteed to win. I caught it, rebuilt it, and caught myself again — the next version used textbook cases so easy that strong models scored a perfect zero, which looks like "AI is safe" but really means "my test was too weak."

The hardest part of evaluating AI safety isn't measuring the model. It's building an environment that can't lie to you. Pre-register the design. Hide the answer key. Don't aim the errors. Let the published authorities define harm, not your own judgment. Report everything — including where the gate fails.

That discipline became the actual product.

---

**Then the results got interesting**

On easy, textbook hazards you get the staircase everyone expects: small local models fail often, frontier models barely at all.

On hard, realistic cases — multiple interacting problems, borderline lab values, drugs that become dangerous only as the patient changes — **the staircase breaks.**

The strongest model was not the safest. The frontier models cracked too. One of them confidently kept renewing a blood thinner as the patient's clotting numbers climbed into dangerous territory, visit after visit — a textbook long-horizon drift that a single-shot test would never reveal.

And the external gate? It caught exactly the class of error it was built to govern — the high-alert drugs — and made no claim about the rest. Necessary, not sufficient. The ordinary-drug mistakes sailed straight through, because no governance gate can substitute for clinical judgment.

---

**The idea I keep coming back to**

"Is this model safe?" is the wrong question. Safety isn't a number you stamp on a model. It's a surface — model strength times problem difficulty. Any single benchmark score is a projection of that surface onto a point, and the projection hides the cliff. The same model is "100% safe" or "visibly unsafe" depending only on which problems you chose to show it.

That's why "we tested it and it passed" is such dangerous comfort. Your test set decided the answer, and you can't enumerate every hard case the real world will.

When you can't verify the intelligence, you verify the boundary.

---

**One more thing, because it matters**

I built this twice — once as a standalone engine, once rebuilt from scratch inside a different framework with a different language in the loop. Two independent implementations. Same numbers. Same failure structure. That's the part that lets me believe it.

It's open source now as `veritasbench-longitudinal` — a long-horizon companion to a governance benchmark I've been working on. It's early and small. The contribution isn't the leaderboard; it's the method, and the honesty rails baked into it.

If you build or deploy AI in anything that runs longer than one request — and almost everything does — I'd love your eyes on it.

Evidence over intelligence. Control over autonomy.
