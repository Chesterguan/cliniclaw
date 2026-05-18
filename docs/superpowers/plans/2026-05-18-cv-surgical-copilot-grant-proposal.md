# CV Surgical Copilot Grant Proposal — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Produce a complete, fundable written grant/study proposal for a cardiovascular surgical AI copilot, candidate-customized for Dr. Eric I. Jeng, drafted section-by-section strictly from the approved grounding spec.

**Architecture:** The deliverable is one assembled proposal document plus a references file, written into `docs/proposals/2026-05-18-cv-surgical-copilot-jeng/`. Each task drafts one section by reading the corresponding grounding section(s), writing grant-grade prose, then passing a fixed editorial Quality Gate. NIH-style structure (Specific Aims → Significance → Innovation → Approach → Regulatory → Environment/Team → Budget) so the institutional-pilot ask is a clean subset of the federal continuum.

**Tech Stack:** Markdown. Source of truth = `docs/superpowers/specs/2026-05-18-cv-surgical-copilot-grant-design.md` (the grounding spec; §14 = candidate overrides, §15 = non-interruption overrides; these supersede §§2/4/6/7 where they conflict).

---

## Standing rules (apply to ALL tasks)

- **Commits are user-initiated only.** This project's rule is "never commit unless explicitly asked." Do NOT run `git commit` or `git add` in any task. Each task ends at a **Checkpoint** (save file, summarize what changed, surface anything needing the PI's input). The user commits manually when they choose.
- **Source of truth is the grounding spec.** Every claim in the proposal must trace to a grounding section or a cited public source. Do not introduce new clinical or technical claims not in the grounding without flagging them in the Checkpoint.
- **Zero fabrication.** Never invent: Dr. Jeng's case volumes, exact budget dollar figures, a specific NIH Institute/PAR number, IRB numbers, citation details you have not verified, or outcome statistics. Where a real grant needs a number the team must supply, write a **bracketed confirm-field** in this exact form: `[CONFIRM: <what> — owner: Dr. Jeng / biostatistician / grants office]`. These bracketed fields are expected in a grant draft; they are NOT plan placeholders.
- **Dates:** today is 2026-05-18. Use 2026 dates. Never use stale years.
- **Candidate facts** must match grounding §14.1 verbatim in substance (titles, board certs, program directorships). Do not upgrade or embellish titles.
- **Non-interruption is non-negotiable** (grounding §15): v1 is capture-only/shadow-mode, no OR output, async post-op review only, smart glass staged Phase 2/3. Any section implying real-time intraop output in v1 fails the Quality Gate.

## Quality Gate (the "test" — run at the end of every task)

A section PASSES only if all six hold. If any fails, fix inline before Checkpoint:

1. **Spec coverage:** every requirement from the task's named grounding section(s) is reflected in the prose. List the grounding subsections covered in the Checkpoint.
2. **No fabrication / no plan-placeholders:** no invented numbers; every team-supplied unknown uses the `[CONFIRM: …]` form; no "TBD/TODO/write later" prose.
3. **Grant-credible register:** specific, evidence-anchored, hedged appropriately; reads as a study a review panel would fund, not a vision pitch. No marketing adjectives without evidence.
4. **Override compliance:** §14 (Jeng/LVAD) and §15 (non-interruption) govern; no §2 generic-CABG framing as the primary wedge; no v1 real-time OR output.
5. **Dates + candidate accuracy:** 2026 dates; Dr. Jeng's titles/credentials match grounding §14.1.
6. **Visuals + precise citations (user directive 2026-05-18):** the proposal is not pure text — figures are added where they aid comprehension (Mermaid, consistent with this repo's existing Mermaid usage), each numbered `Figure N` with a one-line caption and referenced from the prose. Every citation is precise: `REFERENCES.md` entries carry a real, locatable title + venue/publisher + year + URL (not a generic phrase); only genuinely missing bibliographic granularity (full author list / volume / DOI) may be a `[CONFIRM: … — owner: team]`. No inline `[n]` without a resolving precise entry.

## File structure (locked before tasks)

- Create: `docs/proposals/2026-05-18-cv-surgical-copilot-jeng/PROPOSAL.md` — the assembled proposal, built section by section in order.
- Create: `docs/proposals/2026-05-18-cv-surgical-copilot-jeng/REFERENCES.md` — numbered bibliography; sections cite `[n]` into this file.
- Create: `docs/proposals/2026-05-18-cv-surgical-copilot-jeng/CONFIRM-LIST.md` — running list auto-collected from every `[CONFIRM: …]` field, so the team has one punch-list before submission.

One responsibility per file. `PROPOSAL.md` is prose only; `REFERENCES.md` is the citation registry; `CONFIRM-LIST.md` is the open-items tracker.

---

## Task 1: Scaffold the three files + proposal skeleton

**Files:**
- Create: `docs/proposals/2026-05-18-cv-surgical-copilot-jeng/PROPOSAL.md`
- Create: `docs/proposals/2026-05-18-cv-surgical-copilot-jeng/REFERENCES.md`
- Create: `docs/proposals/2026-05-18-cv-surgical-copilot-jeng/CONFIRM-LIST.md`

- [ ] **Step 1: Re-read the full grounding spec**

Read `docs/superpowers/specs/2026-05-18-cv-surgical-copilot-grant-design.md` end to end. Note §14 and §15 override §§2/4/6/7.

- [ ] **Step 2: Write `PROPOSAL.md` skeleton**

Title block + empty section headers in this exact order, each with an HTML comment naming its source grounding section:

```markdown
# An AI Surgical Copilot for Mechanical Circulatory Support: A Governed, Non-Interruptive Pilot Toward Personalized Cardiac Surgical Decision Support

> Draft proposal · 2026-05-18 · PI: Eric I. Jeng, MD, MBA (candidate champion surgeon)
> Status: working draft — contains [CONFIRM: …] fields for team verification

## 1. Project Summary / Abstract
<!-- source: grounding §1, §2, §7, §13 -->

## 2. Specific Aims
<!-- source: grounding §7, §9, §14.2, §14.4 -->

## 3. Significance
<!-- source: grounding §1, §2, §12 -->

## 4. Innovation
<!-- source: grounding §3, §5, §15.1 -->

## 5. Approach
### 5.1 Conceptual framework & wedge
<!-- source: grounding §2, §14.2 -->
### 5.2 Non-interruptive capture & interaction model
<!-- source: grounding §15 -->
### 5.3 Aim 1 — Build & retrospective validation
<!-- source: grounding §3, §4, §7 Aim 1, §15.2 -->
### 5.4 Aim 2 — Prospective single-surgeon clinical/OR pilot
<!-- source: grounding §6, §7 Aim 2, §14.3, §15.5 -->
### 5.5 Aim 3 — Governed expertise twin + native residency education
<!-- source: grounding §5, §7 Aim 3, §14.4 -->
### 5.6 Rigor, statistics & data
<!-- source: grounding §6, §11, §14.3, §15.5 -->
### 5.7 Pitfalls & alternative strategies
<!-- source: grounding §10, §15.4, §15.5 -->
### 5.8 Timeline, milestones & pilot→federal continuum
<!-- source: grounding §9 -->

## 6. Regulatory & Governance Plan
<!-- source: grounding §8, §15.3 -->

## 7. Champion Surgeon, Environment & Team
<!-- source: grounding §14 -->

## 8. Smart-Glass Staging Roadmap (Phase 2 / Phase 3)
<!-- source: grounding §15.4 -->

## 9. Budget & Budget Justification
<!-- source: grounding §7, §9, §15.2 -->

## 10. References
<!-- see REFERENCES.md -->
```

- [ ] **Step 3: Write `REFERENCES.md` seed**

Create the file with a numbered list seeded from grounding §12 and §14.6:

```markdown
# References

1. UF Health — Eric I. Jeng, MD, MBA physician profile. https://ufhealth.org/doctors/eric-i-jeng
2. University of Florida College of Medicine, Department of Surgery — Eric Jeng profile. https://surgery.med.ufl.edu/profile/jeng-eric/
3. UF Experts — Eric Jeng. https://experts.ufl.edu/experts/eric.jeng/
4. Digital twins for the era of personalized surgery. npj Digital Medicine, 2025.
5. Digital twins: pioneering personalized precision in modern surgery. Annals of Medicine and Surgery, 2025.
6. Systematic review: AI models for surgical phase / instrument / anatomy identification, 2025.
7. AI-assisted phase recognition and skill assessment in laparoscopic surgery: systematic review. Frontiers in Surgery, 2025.
8. RCTs evaluating AI in clinical practice: scoping review. Lancet Digital Health, 2024.
9. FDA — Artificial Intelligence-Enabled Medical Devices guidance and Predetermined Change Control Plan framework.
10. INTERMACS / STS Intermacs registry (mechanical circulatory support outcomes). [CONFIRM: exact current registry citation — owner: team]
11. STS Adult Cardiac Surgery Database. [CONFIRM: exact current citation — owner: team]
```

- [ ] **Step 4: Write `CONFIRM-LIST.md` seed**

```markdown
# Confirm-before-submission punch list

Auto-collected `[CONFIRM: …]` fields from PROPOSAL.md. Update as sections are drafted.

- [ ] (populated during drafting)
```

- [ ] **Step 5: Quality Gate**

Apply the Quality Gate (header). Skeleton-only: confirm section order matches grounding override hierarchy and every header has a source comment.

- [ ] **Step 6: Checkpoint**

Summarize files created; confirm skeleton order; no commit.

---

## Task 2: Specific Aims (§2 of proposal)

This is the single most important page. Draft it first after the skeleton because every later section must align to it.

**Files:**
- Modify: `docs/proposals/2026-05-18-cv-surgical-copilot-jeng/PROPOSAL.md` (section 2)
- Modify: `CONFIRM-LIST.md`

- [ ] **Step 1: Re-read grounding §7 (three aims), §9 (continuum), §14.2 (LVAD wedge), §14.4 (native education), §15 (non-interruption)**

- [ ] **Step 2: Draft the Specific Aims page**

One page. Required structure and content (write full prose, not bullets-only):

- **Opening framing (1 short para):** the clinical problem in MCS/LVAD cardiac surgery — documentation and registry (INTERMACS) burden, decision complexity, surgeon burnout — and the gap legacy HIS/EHR cannot close. State the thesis: a *governed, non-interruptive* copilot, validated as a single-surgeon wedge in a program Dr. Jeng directs, designed to seed a federal multi-site study.
- **Central hypothesis (1 sentence):** a capture-only/shadow-mode governed copilot can (a) auto-produce accurate operative documentation + INTERMACS fields, (b) generate personalized, guideline-concordant pre-op plans from the surgeon's own governed case library, and (c) measurably improve his risk-adjusted personal OR-process trajectory — with zero OR interruption.
- **Aim 1** (from grounding §7 Aim 1): build the governed pipeline (passive capture channels §15.2; mature backbones + PEFT, not from-scratch §3; shadow-mode flags §4-override) and validate retrospectively on his historical LVAD case library. Deliverable: locked system + PCCP.
- **Aim 2** (grounding §7 Aim 2, §6, §15.5): prospective single-surgeon LVAD pilot; primary = proximal intraoperative process-and-safety composite vs his INTERMACS-risk-adjusted personal baseline (NOT mortality); co-primary go/no-go = adoption/compliance. Note v1 clinical leverage is pre-op + post-op (§15.5), intraop is shadow-only.
- **Aim 3** (grounding §7 Aim 3, §14.4): governed expertise twin from policy-approved annotated cases; education value evaluated *natively inside his Integrated CT Surgery Residency* (he is Program Director); define go/no-go feasibility criteria and the federal scale-out protocol.
- **Closing (1 para):** the pilot→federal continuum (§9) and the expected impact: a reusable governance pattern for trustworthy surgical AI, not a one-off model.

Insert `[CONFIRM: Dr. Jeng annual isolated durable-LVAD implant volume — owner: Dr. Jeng]` where the single-surgeon power feasibility is asserted.

- [ ] **Step 3: Update CONFIRM-LIST.md** with every `[CONFIRM: …]` added.

- [ ] **Step 4: Quality Gate** (header). Specifically verify: LVAD (not CABG) is the wedge; no v1 intraop output; aims are numbered and testable.

- [ ] **Step 5: Checkpoint** — paste the drafted Aims page into the summary for user eyes (this page warrants direct review); no commit.

---

## Task 3: Significance (§3)

**Files:**
- Modify: `PROPOSAL.md` (section 3); `REFERENCES.md`; `CONFIRM-LIST.md`

- [ ] **Step 1: Re-read grounding §1 (clinician feedback), §2 (wedge rationale), §12 (literature).**

- [ ] **Step 2: Draft Significance.** Required content:

- The clinical and operational problem, MCS/LVAD-specific: long, high-acuity cases; heavy structured documentation + INTERMACS registry burden; decision complexity (cannulation, RV management, de-airing, CPB weaning); contribution to surgeon cognitive load and burnout. Anchor to clinician feedback in grounding §1 (compliance + adoption is the #1 determinant of value).
- Why legacy HIS/EHR cannot solve it (intelligence-gap argument from project framing) — AI as first-class, governed citizen, not bolt-on.
- State of the art with honest framing (cite REFERENCES 4–9): surgical video/phase AI is mature (81–93% phase accuracy) → the bedrock is not speculative; patient digital-twin funding momentum exists (npj/Annals/HSS) → appetite is real; but diagnostic-accuracy ≠ patient benefit and self-updating models break locked trials (Lancet/FDA) → why governance + adoption endpoints + PCCP are the right design.
- The specific gap this fills: a *governed, auditable, non-interruptive* surgical copilot validated where the champion has program control. Significance of the wedge logic (win one program, scale federally).

Add citations to REFERENCES.md as used; renumber if needed.

- [ ] **Step 3: Update CONFIRM-LIST.md.**
- [ ] **Step 4: Quality Gate** (header).
- [ ] **Step 5: Checkpoint** — no commit.

---

## Task 4: Innovation (§4)

**Files:** Modify `PROPOSAL.md` (section 4); `CONFIRM-LIST.md`

- [ ] **Step 1: Re-read grounding §3 (technical approach + non-goals), §5 (twin definition), §15.1 (capture/feedback decoupling).**

- [ ] **Step 2: Draft Innovation.** Required, sharply differentiated content:

- **Primary innovation = the governance layer (VERITAS), not the AI model.** Every AI suggestion policy-gated, provenance-tracked, human-in-the-loop, audit-chained. State plainly that commodity video models at ~90% accuracy are not the contribution.
- **Capture/feedback decoupling (§15.1)** as a design innovation that resolves the adoption-killer; v1 capture-only/shadow.
- **Mature backbones + parameter-efficient fine-tuning + retrieval on the surgeon's governed case library (§3)** — explicit non-goal: no from-scratch model training; data-regime self-justification (single-surgeon volume is right for PEFT/retrieval, wrong for from-scratch).
- **The governed "expertise twin" (§5)** as a novel construct: a retrieval-grounded, PEFT-adapted, policy-bound representation of documented practice — not a cloned model of the hands; with explicit feasibility gates.
- One paragraph distinguishing this from patient digital twins and from ungoverned scribe/CDS products.

- [ ] **Step 3: Update CONFIRM-LIST.md.**
- [ ] **Step 4: Quality Gate** (header) — especially override compliance (no from-scratch training claim; no v1 OR output).
- [ ] **Step 5: Checkpoint** — no commit.

---

## Task 5: Approach 5.1 — Conceptual framework & wedge

**Files:** Modify `PROPOSAL.md` (5.1); `CONFIRM-LIST.md`

- [ ] **Step 1: Re-read grounding §2 + §14.2.**
- [ ] **Step 2: Draft 5.1.** Content: define the wedge = isolated durable LVAD implantation under the MCS Program Dr. Jeng directs (§14.2); justify by volume, INTERMACS risk-adjusted outcomes, protocolized critical steps (cannulation, pump pocket, outflow graft, de-airing, RV management, CPB weaning). State CABG demoted to federal-phase generalization target; bicuspid-aortic named as the fallback wedge if LVAD volume is insufficient (`[CONFIRM: annual LVAD vs bicuspid-aortic volumes — owner: Dr. Jeng]`). Describe the ClinicClaw/VERITAS execution spine (State→Policy→Capability→Agent→Verify→Audit) adapted async, at a reviewer-appropriate level.
- [ ] **Step 3: Update CONFIRM-LIST.md.**
- [ ] **Step 4: Quality Gate** (header).
- [ ] **Step 5: Checkpoint** — no commit.

---

## Task 6: Approach 5.2 — Non-interruptive capture & interaction model

This is a key differentiator section; draft it carefully and completely.

**Files:** Modify `PROPOSAL.md` (5.2); `CONFIRM-LIST.md`

- [ ] **Step 1: Re-read grounding §15 in full.**
- [ ] **Step 2: Draft 5.2.** Content, faithfully expanded from §15:
- Core principle: decouple capture from feedback; v1 capture-only/shadow, zero OR output, all reasoning pre-op + post-op (§15.1).
- The three passive capture channels with the LVAD-specific justification (§15.2): structured streams (AIMS, CPB/perfusion logs, monitors, TEE DICOM); ambient standardized callout audio; existing boom/field camera (consent/IRB, not behavior change).
- The single human interaction: ~2-min async post-op tablet review = the VERITAS human-in-the-loop gate relocated out of the OR (§15.3).
- Explicitly state what v1 does NOT do: no HUD, no real-time prompts, no new sterile-field device.
- [ ] **Step 3: Update CONFIRM-LIST.md** — e.g., `[CONFIRM: OR ambient-audio capture + recording governance approval path at UF — owner: Dr. Jeng / IRB]`.
- [ ] **Step 4: Quality Gate** (header) — non-interruption compliance is the critical check here.
- [ ] **Step 5: Checkpoint** — no commit.

---

## Task 7: Approach 5.3 — Aim 1 (build & retrospective validation)

**Files:** Modify `PROPOSAL.md` (5.3); `CONFIRM-LIST.md`

- [ ] **Step 1: Re-read grounding §3, §4 (+override), §7 Aim 1, §15.2.**
- [ ] **Step 2: Draft 5.3.** Content:
- Pipeline build: ingestion of the three passive channels → structured critical-step timeline via off-the-shelf models → PEFT/retrieval personalization on his historical governed case library → auto-draft operative note + INTERMACS fields → shadow-flag computation (logged, not surfaced).
- Retrospective validation design: on his historical LVAD cases, measure (a) documentation/registry field accuracy vs the official record, (b) shadow-flag predictive validity vs recorded intraoperative events, (c) timeline step-detection accuracy. Define concrete acceptance thresholds as `[CONFIRM: target accuracy thresholds with biostatistician — owner: biostatistician]` (do not invent numbers).
- Deliverable: locked system + Predetermined Change Control Plan.
- [ ] **Step 3: Update CONFIRM-LIST.md.**
- [ ] **Step 4: Quality Gate** (header).
- [ ] **Step 5: Checkpoint** — no commit.

---

## Task 8: Approach 5.4 — Aim 2 (prospective single-surgeon clinical/OR pilot)

**Files:** Modify `PROPOSAL.md` (5.4); `CONFIRM-LIST.md`

- [ ] **Step 1: Re-read grounding §6 (+override), §7 Aim 2, §14.3, §15.5.**
- [ ] **Step 2: Draft 5.4.** Content:
- Design: prospective, single-surgeon (Dr. Jeng), single-center pilot; comparator = his INTERMACS-risk-adjusted historical LVAD cohort and/or sequential baseline→intervention (state both, mark final choice `[CONFIRM: comparator design — owner: biostatistician]` per grounding §11).
- Primary endpoint: proximal, high-frequency, surgeon-level intraoperative process-and-safety composite vs his risk-adjusted personal baseline; **explicitly not 30-day mortality**, with the single-surgeon power rationale stated honestly.
- The §15.5 reconciliation made explicit: because v1 is non-interruptive, the causal clinical lever is pre-op personalized planning + the post-op closed learning loop across his series; the intraop layer is shadow-only and validates flags for Phase 2. State the reviewer fallback (route minimal output to perfusionist/circulating-nurse screen, never the surgeon's field).
- Secondary endpoints (INTERMACS-adjusted morbidity composite; note accuracy/completeness; time-to-documentation).
- Co-primary go/no-go feasibility gate: acceptance rate, override-with-justification rate, validated trust scale, workflow-disruption time, 100% policy-gated/audited (automatic via VERITAS). Adoption failure stops the study regardless of clinical signal.
- Power/sample: framework only; numbers as `[CONFIRM: power calculation — owner: biostatistician]`.
- [ ] **Step 3: Update CONFIRM-LIST.md.**
- [ ] **Step 4: Quality Gate** (header) — verify clinical endpoint + non-interruption coexist exactly as §15.5.
- [ ] **Step 5: Checkpoint** — paste drafted 5.4 into summary for user review (endpoint design warrants direct eyes); no commit.

---

## Task 9: Approach 5.5 — Aim 3 (governed expertise twin + native residency education)

**Files:** Modify `PROPOSAL.md` (5.5); `CONFIRM-LIST.md`

- [ ] **Step 1: Re-read grounding §5, §7 Aim 3, §14.4.**
- [ ] **Step 2: Draft 5.5.** Content:
- The governed expertise twin: retrieval + PEFT over his accumulating policy-approved annotated case library; explicit non-goals (no from-scratch model, no autonomy, not a judgment substitute); explicit go/no-go feasibility criteria before "works" is claimed.
- Native education evaluation inside his Integrated CT Surgery Residency (he is Program Director; 2025 teaching award) — design a trainee-facing evaluation (e.g., concordance of trainee decisions with the governed twin vs faculty gold standard; the loved video-annotation/learning feature as the mechanism). Mark instrument choices `[CONFIRM: validated education assessment instrument — owner: Dr. Jeng / education research]`.
- The federal multi-surgeon/multi-site scale-out protocol as the bridge to the continuum.
- [ ] **Step 3: Update CONFIRM-LIST.md.**
- [ ] **Step 4: Quality Gate** (header).
- [ ] **Step 5: Checkpoint** — no commit.

---

## Task 10: Approach 5.6 & 5.7 — Rigor/statistics/data + Pitfalls & alternatives

**Files:** Modify `PROPOSAL.md` (5.6, 5.7); `CONFIRM-LIST.md`

- [ ] **Step 1: Re-read grounding §6, §10, §11, §14.3, §15.4, §15.5.**
- [ ] **Step 2: Draft 5.6 (Rigor/stats/data):** INTERMACS-based risk adjustment; data provenance and the SHA-256 audit chain doubling as research-data-integrity evidence; HIPAA/PHI minimization (identifiers not names); biological-variable/generalizability framing for a single-surgeon design; all numeric targets as `[CONFIRM: … — owner: biostatistician]`.
- [ ] **Step 3: Draft 5.7 (Pitfalls & alternatives):** expand grounding §10 table into prose with mitigations — single-surgeon generalizability (wedge-by-design; federal phase scales), open-cardiac capture limits (multimodal/ambient), endpoint powering (proximal metrics), adoption risk (hard go/no-go gate), model-update vs locked trial (PCCP + audit), plus the §15.4/§15.5 staged-feedback and reviewer-fallback alternatives.
- [ ] **Step 4: Update CONFIRM-LIST.md.**
- [ ] **Step 5: Quality Gate** (header).
- [ ] **Step 6: Checkpoint** — no commit.

---

## Task 11: Approach 5.8 — Timeline, milestones & pilot→federal continuum

**Files:** Modify `PROPOSAL.md` (5.8); `CONFIRM-LIST.md`

- [ ] **Step 1: Re-read grounding §9.**
- [ ] **Step 2: Draft 5.8.** Content: phased timeline with milestones and explicit go/no-go gates; institutional innovation pilot funds Aims 1–2 + Aim 3 setup; preliminary data + locked endpoints deliberately structured to seed an NIH/AHRQ multi-site application (Aim 3 at scale). Present as one continuum, not two disconnected asks. Calendar in 2026+ relative terms; mark the specific federal mechanism `[CONFIRM: target NIH IC / AHRQ mechanism or PAR — owner: grants office / Dr. Jeng]`.
- [ ] **Step 3: Update CONFIRM-LIST.md.**
- [ ] **Step 4: Quality Gate** (header).
- [ ] **Step 5: Checkpoint** — no commit.

---

## Task 12: Regulatory & Governance Plan (§6) + Smart-Glass Roadmap (§8)

**Files:** Modify `PROPOSAL.md` (sections 6 and 8); `CONFIRM-LIST.md`

- [ ] **Step 1: Re-read grounding §8, §15.3, §15.4.**
- [ ] **Step 2: Draft section 6 (Regulatory & Governance):** likely non-significant-risk device rationale (decision support, human-in-the-loop, no autonomous action, capture-only v1) — framed as a determination to confirm, not asserted (`[CONFIRM: NSR determination — owner: IRB / regulatory affairs]`); IRB + consent approach (incl. OR audio/video consent); PCCP for model updates; audit chain as data-integrity evidence; HIPAA.
- [ ] **Step 3: Draft section 8 (Smart-Glass Roadmap):** strictly from §15.4 — Phase 2 = passive egocentric recorder on existing head-worn gear, no display; Phase 3/business = HUD feedback with its own human-factors + device-classification study. Explicitly out of v1 scope. Frame as grounded vision, not a v1 promise.
- [ ] **Step 4: Update CONFIRM-LIST.md.**
- [ ] **Step 5: Quality Gate** (header) — verify smart glass is unambiguously Phase 2/3, not v1.
- [ ] **Step 6: Checkpoint** — no commit.

---

## Task 13: Champion Surgeon, Environment & Team (§7)

**Files:** Modify `PROPOSAL.md` (section 7); `CONFIRM-LIST.md`

- [ ] **Step 1: Re-read grounding §14 in full.**
- [ ] **Step 2: Draft section 7.** Content, strictly matching §14.1 (no embellishment): Dr. Eric I. Jeng, MD, MBA — Associate Professor with tenure, Division of Cardiovascular Surgery, UF College of Medicine / UF Health; double board-certified (American Board of Surgery; American Board of Thoracic Surgery); FACS/FACC/FCCP; Surgical Director MCS Program; Surgical Director Bicuspid AV Program; Associate Director Aortic Disease Center; Program Director Integrated CT Surgery Residency; stated research interests (AI/advanced imaging; MCS/transplant; economics in medicine); patent-pending devices; 2025 UF COM teaching award. Then the fundability argument from §14.5 (MBA/economics → cost arm; patents → innovation/tech-transfer; program directorships → real fast institutional path; single-surgeon design justified because the wedge is his program). UF environment: MCS program, residency, INTERMACS participation. Add `[CONFIRM: current titles, appointments, case volumes, INTERMACS site participation — owner: Dr. Jeng]`. Note a biosketch + letters-of-support list is required at submission (list who: Dept Chair, perfusion lead, IRB, biostatistician, residency).
- [ ] **Step 3: Update CONFIRM-LIST.md.**
- [ ] **Step 4: Quality Gate** (header) — candidate-accuracy check is critical here; titles must match §14.1 exactly.
- [ ] **Step 5: Checkpoint** — no commit.

---

## Task 14: Budget & Justification (§9) + Abstract (§1) + final assembly review

Abstract is written last so it accurately summarizes the finished document.

**Files:** Modify `PROPOSAL.md` (sections 9 and 1); `REFERENCES.md`; `CONFIRM-LIST.md`

- [ ] **Step 1: Re-read grounding §7, §9, §15.2 and skim the now-drafted PROPOSAL.md end to end.**
- [ ] **Step 2: Draft section 9 (Budget & Justification):** category structure only, with rationale per category, every dollar figure as `[CONFIRM: amount — owner: grants office]`. Categories: personnel (PI effort, research coordinator, ML/data engineer, biostatistician %); ambient-capture hardware (OR microphones, secure capture appliance); data integration (AIMS/CPB/monitor/TEE feeds); compute (PEFT + inference, no training-from-scratch cluster — keep modest, justify via data regime); IRB/regulatory; education-arm costs; dissemination. Justify scale as an institutional pilot sized to seed the federal continuum (§9).
- [ ] **Step 3: Draft section 1 (Project Summary/Abstract):** ≤30 lines, accurately summarizing the final document — problem, governed non-interruptive approach, LVAD wedge under Dr. Jeng's program, three aims, pilot→federal continuum, expected impact.
- [ ] **Step 4: Finalize REFERENCES.md** — ensure every `[n]` used in PROPOSAL.md resolves; remove unused; renumber contiguously. (Precision pass already done in Task 15; here just verify integrity after all sections + figures exist.)
- [ ] **Step 5: Consolidate CONFIRM-LIST.md** — ensure every `[CONFIRM: …]` in PROPOSAL.md appears once in the punch list, grouped by owner.
- [ ] **Step 6: Quality Gate** (header) applied to the whole document, plus the Self-Review below.
- [ ] **Step 7: Checkpoint** — present the assembled proposal + the CONFIRM punch-list; no commit.

---

## Task 15: Precise citations pass (user directive 2026-05-18)

Run AFTER Tasks 11–13, BEFORE Task 16 (figures cite into precise refs) and Task 14.

**Files:** Modify `REFERENCES.md`; `PROPOSAL.md` (inline `[n]` only if renumbered); `CONFIRM-LIST.md`

- [ ] **Step 1:** Inventory every `[n]` actually cited across PROPOSAL.md §§2–10.
- [ ] **Step 2:** Rewrite each `REFERENCES.md` entry as a precise, locatable citation: exact article/page title + venue or publisher + year + URL. Use the verified sources gathered during grounding research where available:
  - npj Digital Medicine, "Digital twins for the era of personalized surgery" (2025) — https://www.nature.com/articles/s41746-025-01575-5
  - Annals of Medicine and Surgery, "Digital twins: pioneering personalized precision in modern surgery" (2025) — https://pmc.ncbi.nlm.nih.gov/articles/PMC12578008/
  - Acta Obstetricia et Gynecologica Scandinavica / Wiley, "AI in the operating room: a systematic review of AI models for surgical phase, instrument and anatomical structure identification" (2025) — https://obgyn.onlinelibrary.wiley.com/doi/full/10.1111/aogs.70045
  - Frontiers in Surgery, "AI-assisted phase recognition and skill assessment in laparoscopic surgery: a systematic review" (2025) — https://www.frontiersin.org/journals/surgery/articles/10.3389/fsurg.2025.1551838/full
  - Lancet Digital Health, "RCTs evaluating AI in clinical practice: a scoping review" (2024) — https://www.thelancet.com/journals/landig/article/PIIS2589-7500(24)00047-5/fulltext
  - U.S. FDA, "Artificial Intelligence-Enabled Medical Devices" + Predetermined Change Control Plan guidance — https://www.fda.gov/medical-devices/software-medical-device-samd/artificial-intelligence-enabled-medical-devices
  - UF Health / UF College of Medicine / UF Experts — Eric I. Jeng profiles (URLs already in grounding §14.6)
  - INTERMACS / STS National Database — cite the official STS registry pages.
- [ ] **Step 3:** Only the residual missing granularity (full author list / volume / DOI) may remain as `[CONFIRM: full bibliographic detail for ref n — owner: team]`. No generic phrase-only entries may remain.
- [ ] **Step 4:** Verify every inline `[n]` resolves; renumber contiguously; update CONFIRM-LIST.md.
- [ ] **Step 5: Quality Gate** (header — criterion 6 especially). **Step 6: Checkpoint** — no commit.

---

## Task 16: Figures pass (user directive 2026-05-18)

Run AFTER Task 15, BEFORE Task 14 final assembly. Figures are Mermaid (consistent with this repo's existing Mermaid usage). Each figure: a `**Figure N.** <caption>` line, the ```mermaid block, and at least one in-prose reference ("(Figure N)"). Keep diagrams legible — no PHI, no invented numbers.

**Files:** Modify `PROPOSAL.md`; `CONFIRM-LIST.md` (only if a figure needs a confirm)

- [ ] **Step 1:** Insert **Figure 1 — Governed execution spine** in §5.1.3: Mermaid flowchart `State → Policy (deny-by-default) → Capability → Agent → Verify → Audit (SHA-256 chain) → next State`, async I/O annotated.
- [ ] **Step 2:** Insert **Figure 2 — Non-interruptive capture & interaction model** in §5.2 (the key differentiator): three passive capture channels (structured streams; ambient callout audio; existing field/boom camera) → governed reasoning that runs ONLY pre-op + post-op → ~2-min async post-op review (HITL gate); the OR box explicitly labeled "v1 output = 0 (shadow only)".
- [ ] **Step 3:** Insert **Figure 3 — Aims & pilot→federal continuum** in §5.8: Mermaid timeline/flow showing Aim 1 → Aim 2 (with co-primary go/no-go gate) → Aim 3, and the institutional pilot → NIH/AHRQ federal phase bridge. No fabricated dates/durations — use relative phases, not invented calendar months.
- [ ] **Step 4:** Insert **Figure 4 — Aim 2 study design & endpoint logic** in §5.4: comparator (INTERMACS-risk-adjusted personal baseline) vs prospective copilot-supported series; primary proximal composite; secondary endpoints; co-primary adoption go/no-go gate that can stop the study.
- [ ] **Step 5:** Add a one-line "Figures" note under §1 or a List of Figures after the title block so reviewers can find them.
- [ ] **Step 6: Quality Gate** (header — criteria 4 and 6: no figure may imply v1 OR output; LVAD/INTERMACS correct; no invented numbers in any diagram). **Step 7: Checkpoint** — no commit.

---

## Plan Self-Review (run by the plan author now, before handoff)

**1. Spec coverage:** mapped each grounding section to a task —
§1→T2/T3; §2→T5(+T2); §3→T4/T7; §4(+override)→T6/T7; §5→T4/T9; §6(+override)→T8/T10; §7 Aims→T2/T7/T8/T9; §8→T12; §9→T11/T14; §10→T10; §11→T8/T10; §12→T3 (+REFERENCES T1); §13→T2/T14 (deliverable quality bar = Quality Gate, all tasks); §14→T2/T5/T9/T13; §15→T6 (+T8/T9/T12, overrides enforced in Quality Gate). No grounding section is unmapped.

**2. Placeholder scan:** the plan contains no "TBD/TODO/write later." `[CONFIRM: …]` fields are a deliberately specified deliverable mechanism (real grant drafts require team-supplied numbers) with named owners — not plan placeholders. Each task gives concrete content to write, not "write the X section."

**3. Type consistency:** file paths identical across tasks (`docs/proposals/2026-05-18-cv-surgical-copilot-jeng/{PROPOSAL,REFERENCES,CONFIRM-LIST}.md`); section numbering in Task 1 skeleton matches every later task's target; override hierarchy (§14/§15 govern §§2/4/6/7) stated in standing rules and re-asserted in every affected task's Quality Gate.

No issues found requiring a structural change.

---

## Execution Handoff

(Presented to the user after this plan is saved.)
