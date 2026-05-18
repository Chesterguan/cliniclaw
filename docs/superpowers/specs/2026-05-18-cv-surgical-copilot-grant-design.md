# Cardiovascular Surgical Copilot — Grant/Study Proposal Design

> Status: DRAFT design (pre-proposal blueprint) · Date: 2026-05-18
> Rev: 2026-05-18 — candidate customization for Dr. Eric I. Jeng (see §14;
> §14 overrides the generic wedge/endpoint/education framing where they conflict)
> Owner: ClinicClaw · Champion: cardiovascular surgeon (single-surgeon pilot)
> This document is the agreed *design* for the grant proposal. The full proposal
> is written from this blueprint after sign-off.

## 1. Why this proposal exists (clinician feedback driving it)

Feedback collected from clinicians over the dormant period:

1. **Compliance and workflow adoption are the #1 concern** — clinicians do not
   care about the tech internals. A system that is not adopted, or that is not
   demonstrably policy-compliant, has zero clinical value regardless of model
   accuracy.
2. **The proposed feature set is valued but unfinished** — clinicians
   specifically like the **surgical video annotation + learning** capability.
3. **A champion cardiovascular surgeon wants his own "digital twin."**
4. **Today's goal:** a feasible, clinically and technically credible
   grant/proposal the surgeon can submit for a copilot study. Not a fantasy —
   something a study section would actually fund.

These constraints, not technology ambition, drive every decision below.

## 2. Strategic framing: a wedge, not a platform

Scope is deliberately narrow: **isolated, elective, on-pump CABG (coronary
artery bypass grafting), single champion surgeon, single center.**

> **Candidate override — see §14.2:** for Dr. Eric I. Jeng the wedge is
> re-anchored to **durable LVAD implantation** under the Mechanical Circulatory
> Support Program he directs. The CABG framing in this section is retained only
> as the procedure-agnostic fallback.

Rationale:

- **Volume** — CABG is the highest-volume adult cardiac procedure, giving a
  single surgeon enough cases for statistical feasibility in a pilot.
- **Free risk-adjusted outcomes** — the STS Adult Cardiac Surgery Database
  already captures risk-adjusted outcomes for these cases, which makes a
  *clinical* primary endpoint tractable without building a registry.
- **Well-defined critical steps and decision gates** — cannulation, aortic
  cross-clamp on/off, cardiopulmonary bypass (CPB) on/off, conduit strategy,
  completeness of revascularization, weaning from bypass.
- **Wedge logic** — win one procedure with one surgeon, generate the
  preliminary data, then expand to multi-surgeon / multi-site in the federal
  phase. Generalizability is *intentionally deferred*, not ignored.

Honest scope note (this is the "not fantasy" rigor reviewers reward): open
cardiac surgery is **not** laparoscopic. "Video annotation" here means
**multimodal case capture** — procedural timeline + intraoperative TEE echo
loops + endoscopic conduit-harvest video + ambient OR narrative — not
scope-only phase recognition. The proposal states this limitation explicitly.

## 3. Technical approach (and explicit non-goals)

**Non-goals (stated up front in the proposal):**

- No training of foundation or perception models *from scratch*. This is not
  the contribution, not the expertise, and — decisively — the *wrong*
  engineering choice for a single-surgeon data regime.
- No autonomous action. The copilot never acts; it surfaces, the surgeon
  decides.
- No replacement of surgical judgment.

**In scope and central:**

- **Mature pretrained backbones** — surgical video foundation models and LLMs,
  consumed through ClinicClaw's existing pluggable LLM layer
  (Claude / Ollama / mock).
- **Parameter-efficient fine-tuning / adaptation** (LoRA, adapters,
  instruction-tuning) of those mature backbones on the surgeon's *governed,
  policy-approved* annotated case library. This is how personalization happens.
- **Retrieval-grounding** over the same governed case library for case-specific
  reasoning.
- **The governance layer is the novel, fundable contribution** — VERITAS:
  every AI suggestion is policy-gated, provenance-tracked, human-in-the-loop,
  and audit-chained. Anyone can run a 90%-accurate video model; almost no one
  has a *governed, auditable, adoption-instrumented* surgical copilot. That is
  the moat and it maps directly to clinician concern #1.

Data-regime justification: a single surgeon's CABG history is the correct
scale for PEFT + retrieval and the wrong scale for from-scratch training. The
technical approach is therefore self-justifying, not just a preference.

## 4. What the copilot does

**Pre-operative**
- Ingest FHIR record + imaging/cath reports + STS risk score.
- Surface *the surgeon's own historical approach* for similar patients
  (retrieval over his governed case library) alongside guideline concordance.
- Generate a structured operative plan; surgeon edits and approves it
  (VERITAS human-in-the-loop gate, audited).

**Intra-operative**
- Structured critical-step timeline (cannulation, clamp on/off, CPB on/off,
  weaning) from off-the-shelf models on the ambient/passive capture.
- v1 is **shadow-mode and capture-only**: the system computes what it *would*
  flag at decision gates and logs it with timestamps, but **surfaces nothing
  in the OR**. No real-time prompts, no display, no behavior change.
- Real-time surfacing is deferred to a later phase with its own human-factors
  study — see §15. Every computed flag is still gated and audited.

> **Non-interruption override — see §15:** v1 makes NO intraoperative output.
> Copilot clinical leverage in v1 is pre-op + post-op only. This supersedes
> any "real-time prompt" reading of this section.

**Post-operative**
- Auto-draft operative note + STS data fields from the structured timeline.
- Flag deviations from the surgeon's own pattern and from guidelines, for his
  review.
- Approved case feeds the governed case library — closing the learning loop.

## 5. The "expertise twin" — honest definition

The twin is **a governed, retrieval-grounded representation of the surgeon's
documented practice**: his annotated case library, technique preferences,
personalized thresholds, and decision rules — surfaced through a mature,
PEFT-adapted reasoning engine under VERITAS policy and audit.

It "learns" by accumulating his *policy-approved* annotated cases (the
annotation/learning feature clinicians liked). It is explicitly **not** a
custom-trained model of his hands, not autonomous, and not a substitute for
judgment. The proposal defines concrete go/no-go feasibility criteria before
the twin is claimed to "work."

## 6. Study design & endpoints

> **Candidate override — see §14.3:** for the LVAD wedge, risk adjustment and
> comparator use the **INTERMACS** registry, not STS. The endpoint *principle*
> below is unchanged.

- **Design:** prospective, single-surgeon, single-center pilot.
- **Comparator:** risk-adjusted historical STS CABG cohort from the same
  surgeon, and/or a sequential baseline → intervention period. (Final choice
  deferred to the implementation plan — see §10.)
- **Primary endpoint (clinical / OR — proximal, high-frequency, powerable):**
  a composite intraoperative process-and-safety endpoint — myocardial
  ischemic (cross-clamp) time and CPB time vs the surgeon's risk-adjusted
  personal baseline, plus the rate of detected and resolved critical-step
  deviations. **Explicitly not 30-day mortality** — a single-surgeon pilot
  cannot power a hard outcome, and the proposal says so directly.
- **Secondary:** STS risk-adjusted morbidity composite; operative-note
  accuracy/completeness; time-to-documentation.
- **Co-primary feasibility gate (clinician concern #1):** copilot acceptance
  rate, override-with-justification rate, validated trust scale,
  workflow-disruption time, and 100% of AI suggestions policy-gated and
  audited (automatic via VERITAS). **Adoption failure stops the study
  regardless of clinical signal.** Clinical benefit that is not adopted is not
  benefit.

## 7. Three aims

- **Aim 1 — Build & retrospectively validate.** Construct the governed CABG
  copilot pipeline (perioperative ingestion, off-the-shelf step recognition,
  PEFT adaptation on his historical cases, policy-gated decision prompts,
  auto-documentation). Validate retrospectively on his historical case
  library. Deliverable: locked system + Predetermined Change Control Plan.
- **Aim 2 — Prospective single-surgeon clinical/OR pilot.** Primary
  intraoperative process-safety endpoint vs risk-adjusted personal baseline,
  with the adoption/compliance feasibility gate as go/no-go.
- **Aim 3 — Governed expertise twin + education + scale-out protocol.**
  Accumulate the annotated policy-approved case library; demonstrate
  personalized decision-support concordance and trainee-education value (the
  loved annotation/learning feature); define explicit feasibility criteria and
  the federal multi-surgeon / multi-site expansion protocol.

> **Candidate override — see §14.4:** the education evaluation runs *natively*
> inside Dr. Jeng's Integrated CT Surgery Residency (he is Program Director),
> not as a separate bolt-on study.

## 8. Regulatory & governance

- Likely **non-significant-risk device** (clinical decision support,
  human-in-the-loop, no autonomous action) — to be confirmed with the IRB and
  regulatory affairs.
- IRB approval; informed consent as applicable.
- **Predetermined Change Control Plan (PCCP)** for any model updates — directly
  answers the FDA concern that self-updating models break locked trial
  endpoints.
- The VERITAS SHA-256 audit chain doubles as ready-made trial data-integrity
  evidence — a governance feature reused as a research-integrity feature.

## 9. Funding continuum ("1 or 4")

- **Now:** institutional / hospital innovation pilot funds Aims 1–2 and the
  setup of Aim 3. Small budget, fast IRB path, lowest reviewer risk.
- **Next:** the pilot's preliminary data and *locked* endpoints are
  deliberately structured to seed an NIH R01 / AHRQ multi-surgeon, multi-site
  application that executes Aim 3 at scale.
- The proposal explicitly presents this as a planned continuum, not two
  disconnected asks.

## 10. Risks & honest caveats (stated in the proposal, with mitigations)

| Risk | Mitigation |
|---|---|
| Single-surgeon generalizability | It is a *wedge by design*; the federal phase scales it. |
| Open-cardiac video limits | Multimodal capture (timeline + TEE + harvest video + ambient), not scope-only. |
| Endpoint powering | Proximal, high-frequency surgeon-level metrics — not mortality. |
| Adoption risk | A hard go/no-go gate, not an afterthought. |
| Model update vs locked trial | PCCP + audit chain. |

## 11. Decisions deferred to the implementation plan

- Comparator: historical STS cohort vs sequential baseline→intervention
  (or both) — to be fixed with the biostatistician.
- Exact composite-endpoint weighting and power calculation.
- Specific mature backbones and PEFT method selection.
- Target funder shortlist within routes 1 and 4.

## 12. Reference grounding (literature reviewed 2026-05)

- Digital twins for personalized surgery — npj Digital Medicine (2025);
  Annals of Medicine and Surgery (2025); HSS $10M orthopedic digital-twin
  platform (Nov 2025) — establishes funding appetite and the patient-twin vs
  surgeon-twin distinction.
- Surgical video phase recognition — systematic reviews (2025) report
  81–93% phase-recognition accuracy; AI surgical-competency-from-video is an
  actively funded area — establishes the bedrock is mature, not speculative.
- AI clinical-trial design — Lancet Digital Health scoping review; FDA
  AI-enabled device guidance and PCCP framework — establishes that
  diagnostic accuracy ≠ patient benefit and that adoption/process endpoints
  and PCCP are legitimate and expected.

## 13. Deliverable

A grant/study proposal document, written from this blueprint, that is:
clinically credible (STS-backed endpoints, IRB/PCCP path), technically
feasible (mature models + PEFT + retrieval + existing VERITAS spine, no
from-scratch training), and honest about scope and power. It must read as a
fundable study, not a vision pitch.

**Presentation directive (added 2026-05-18, user):** the proposal is not pure
text. It must include figures/charts where they aid comprehension — Mermaid
diagrams (consistent with this repo's existing Mermaid usage), each numbered
and captioned and referenced from the prose (minimum: governed execution
spine; the non-interruptive capture/interaction model; the aims + pilot→
federal continuum; the Aim 2 study/endpoint logic). All citations must be
precise and locatable (real title + venue/publisher + year + URL); generic
phrase-only references are not acceptable — only residual bibliographic
granularity (author list / volume / DOI) may remain a `[CONFIRM: …]`.

## 14. Candidate customization — Dr. Eric I. Jeng (added 2026-05-18)

Tailors the procedure-agnostic blueprint above to the named champion surgeon.
Where this section conflicts with §2 (wedge), §6 (endpoint registry), or §7
(education arm), **this section governs**; §§2/6/7 are retained as the
procedure-agnostic fallback.

### 14.1 Verified profile (public sources; confirm current titles with him)

- Eric I. Jeng, MD, MBA — Associate Professor with tenure, Division of
  Cardiovascular Surgery, University of Florida College of Medicine / UF
  Health, Gainesville, FL.
- Double board-certified: American Board of Surgery; American Board of
  Thoracic Surgery. FACS, FACC, FCCP.
- Leadership: Surgical Director, Mechanical Circulatory Support (MCS) Program;
  Surgical Director, Bicuspid Aortic Valve Program; Associate Director, Aortic
  Disease Center; Program Director, Integrated Thoracic & Cardiovascular
  Surgery Residency.
- Operative expertise: durable VAD implantation, ECMO/MCS, heart & lung
  transplantation, open aortic surgery, bicuspid/complex valve, TAVR, TEVAR,
  minimally invasive cardiac surgery.
- Stated research interests: AI and advanced imaging technology;
  cardiopulmonary mechanical support & transplantation; dysphagia in
  thoracic/CV surgery; economics in medicine. Multiple patent-pending devices.
  2025 UF COM teaching/mentorship award.
- Titles and case-volume figures must be confirmed directly with him before
  submission — appointments change.

### 14.2 Wedge re-anchoring (overrides §2)

Generic on-pump CABG was the procedure-agnostic default and is **not** where
his program leadership or case concentration sits. Re-anchor to a program he
personally directs:

- **Primary recommended wedge: durable LVAD implantation, under the MCS
  Program he is Surgical Director of.**
  - "Champion surgeon" becomes airtight — he controls case flow, registry
    participation, trainees, and the institutional-pilot path.
  - **INTERMACS** supplies risk-adjusted outcomes for the clinical primary
    endpoint — the role STS played for CABG, arguably tighter for a
    device-implant procedure with protocolized critical steps (cannulation,
    pump pocket, outflow graft, de-airing, RV management, CPB weaning).
  - Directly engages his stated AI/advanced-imaging and economics interests.
- **Named alternative wedge: bicuspid aortic valve / aortic root surgery**
  (Bicuspid AV Program / Aortic Disease Center he directs; STS-tracked). Use
  if LVAD annual volume is too low to power the pilot.
- Generic CABG demoted to a federal-phase *generalization target*, not the
  pilot wedge.

### 14.3 Endpoint adjustment (overrides §6 registry references)

- LVAD wedge → comparator and risk adjustment via **INTERMACS** (not STS).
- Valve/aortic wedge → **STS Adult Cardiac** as written in §6.
- Endpoint principle unchanged: a proximal, high-frequency, surgeon-level
  intraoperative process-and-safety composite vs his risk-adjusted personal
  baseline — never a hard mortality outcome in a single-surgeon pilot.

### 14.4 Education arm is native, not bolt-on (strengthens §7 Aim 3)

He is **Program Director of the Integrated CT Surgery Residency** and a 2025
teaching-award recipient.

- The governed expertise twin encodes *his* MCS/valve technique and decision
  rules from his policy-approved annotated case library.
- Trainee-education value is evaluated inside his own residency program with
  him as PI/PD — making the education endpoint credible and the consent/IRB
  path straightforward.
- This converts "I want my own digital twin" from a vanity ask into a
  defensible program-director education-and-legacy rationale.

### 14.5 Why the candidate strengthens fundability

- MBA + "economics in medicine" → the adoption/workflow + cost-of-care arm is
  credible coming from him; supports an AHRQ/federal economics angle.
- Patent-pending-device track record → Innovation-section credibility and a
  tech-transfer narrative.
- Multiple program directorships → the institutional-pilot ("route 1") path is
  real and fast; he is the decision-maker for his own programs.
- Single-surgeon pilot fully justified: high-volume program director, the
  wedge is *his* program, the federal phase scales beyond him.

### 14.6 Sources

- UF Health physician profile / bio / research:
  https://ufhealth.org/doctors/eric-i-jeng
- UF College of Medicine, Department of Surgery:
  https://surgery.med.ufl.edu/profile/jeng-eric/
- UF Experts: https://experts.ufl.edu/experts/eric.jeng/

## 15. Non-interruptive capture & interaction model (added 2026-05-18)

Hard constraint from the surgeon: the system must **never interrupt the
clinical workflow or the OR**. Capture and interaction must be pervasive
(ambient, embedded, no behavior change). This section governs §4 (intra-op)
and the human-in-the-loop placement in §5/§7 where they conflict.

### 15.1 Core principle: decouple capture from feedback

Surgical-AI projects die in adoption by pushing real-time intraoperative
*output* at the surgeon. The output channel is the hard, device-regulated,
distraction-inducing part; the *capture* channel is not. v1 is **capture-only,
ambient, shadow-mode — zero output in the OR.** All copilot reasoning runs
pre-op and post-op, outside the sterile field and outside the critical path.

### 15.2 Three passive capture channels (all already exist, zero new behavior)

For the LVAD/MCS wedge the OR is already densely instrumented:

1. **Structured streams (backbone):** AIMS/anesthesia record, CPB/perfusion
   machine logs (flows, ACT, clamp/bypass on-off), monitor data, TEE DICOM.
   Already timestamped, FHIR/HL7-mappable. No new action by anyone.
2. **Ambient OR audio:** standardized cardiac callouts ("going on bypass",
   "cross-clamp on", "coming off pump", "de-airing") — structured-by-
   convention, captured by a microphone with zero workflow change.
3. **Existing field/boom camera:** already present in most cardiac ORs;
   recording is a consent/IRB matter, not a behavior change.

### 15.3 The only human interaction: async post-op review

A ~2-minute asynchronous post-op review on a tablet — surgeon confirms /
annotates the auto-drafted note and the shadow-flagged deviations. This is the
VERITAS human-in-the-loop gate **moved out of the OR entirely**, while still
producing policy-approved, human-validated labels for the governed twin.
(This relocates the §5/§7 human-in-the-loop gate from intra-op to post-op.)

### 15.4 Smart glass — staged, not v1

Egocentric capture is the best data source for an individual-surgeon twin, but
it is **Phase 2+**, for clinical (not technical) reasons: it must integrate
with existing loupes + coaxial headlight (consumer AR does not); sterility/
IPAC review, OR-committee approval, multi-hour battery, and first-person
consent are additive scope that would sink a v1; and the heads-up *display* is
itself the in-OR interruption being ruled out.

- **Phase 2:** smart glass as a *passive egocentric recorder* mounted on
  existing head-worn gear — no display.
- **Phase 3 / business:** real-time heads-up feedback, with its own
  human-factors + device-classification study.

### 15.5 Endpoint reconciliation (resolves a tension with §6)

A zero-interruption v1 performs no intraoperative action, so it cannot
causally move an *intraoperative* metric — pure shadow mode proves only
documentation/registry accuracy + shadow-flag predictive validity. Resolution:

- v1 clinical leverage = **pre-op** (the twin generates a personalized,
  guideline-concordant plan) **+ post-op** (closed learning loop across his
  own case series). Both are causally plausible for moving his *risk-adjusted
  personal OR trajectory* over the series, with zero intraop interruption.
- The intraop layer stays capture/shadow-only and *validates* flags for a
  Phase-2 where minimal vetted feedback is introduced.
- Keeps the §6 clinical/OR primary endpoint **and** zero OR disruption.
- Fallback if reviewers demand an in-OR intervention: route minimal output to
  the perfusionist / circulating-nurse screen — never the surgeon's field.
