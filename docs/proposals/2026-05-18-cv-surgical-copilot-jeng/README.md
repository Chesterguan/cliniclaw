# CV Surgical Copilot — Grant/Study Proposal (Dr. Eric I. Jeng)

A feasible, clinically- and technically-credible grant proposal for an
AI **surgical copilot for mechanical circulatory support (durable LVAD)**,
championed by Dr. Eric I. Jeng (UF). Governed by the VERITAS trust layer,
**non-interruptive** (capture-only / shadow-mode v1, zero OR output),
structured as a single-surgeon institutional pilot deliberately built to seed
a federal multi-site study.

## Files

| File | Role |
|---|---|
| `PROPOSAL.md` | **The proposal. Single source of truth.** §1–§10, 4 figures. |
| `REFERENCES.md` | Numbered bibliography (precise; author/DOI gaps flagged). |
| `CONFIRM-LIST.md` | Pre-submission punch list — every `⚑ CONFIRM` open item, by owner. |
| `build_share.py` | Regenerates the shareable `.docx` + `.html` into `dist/`. |
| `BUILD.md` | How to build, the manual GitHub Action, and the source-of-truth rule. |
| `dist/` | Generated exports — git-ignored, never committed (published as Releases). |

## Provenance

- Design / grounding: `../../superpowers/specs/2026-05-18-cv-surgical-copilot-grant-design.md`
  (§14 = Dr. Jeng candidate customization; §15 = non-interruption model — these
  override the generic framing where they conflict).
- Implementation plan: `../../superpowers/plans/2026-05-18-cv-surgical-copilot-grant-proposal.md`

## Status

Draft, independently reviewed and approved section-by-section. Fundability is
conditional on resolving the `CONFIRM` items — start with Dr. Jeng's annual
durable-LVAD volume (it decides whether the wedge stays LVAD or pivots to the
bicuspid-aortic fallback). See `BUILD.md` for the editing/regeneration policy.
