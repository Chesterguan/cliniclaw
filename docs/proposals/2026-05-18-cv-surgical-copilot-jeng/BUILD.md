# Building & sharing this proposal

## Source of truth

`PROPOSAL.md` is the **only** source of truth. `REFERENCES.md` and
`CONFIRM-LIST.md` support it. Everything in `dist/` is generated and
disposable — it is git-ignored and never committed.

## Regenerate the shareable files

**Locally:**

```bash
python3 docs/proposals/2026-05-18-cv-surgical-copilot-jeng/build_share.py
# -> dist/ClinicClaw-CV-Copilot-Proposal.docx   (editing copy, Word styles + TOC)
# -> dist/ClinicClaw-CV-Copilot-Proposal.html   (self-contained, for explaining)
```

Needs: `pandoc` ≥ 3.1, `node`, `python3`, and a Chrome/Chromium for
`mermaid-cli` to render the four figures.

**On GitHub (manual trigger):** Actions → *Build CV Copilot Proposal
(manual)* → **Run workflow**. It rebuilds from `PROPOSAL.md` and publishes
the `.docx` + `.html` as a **GitHub Release** (and as run artifacts). Nothing
binary is committed to the repo.

## The one rule that keeps history clean

The pipeline is **one-directional**: `PROPOSAL.md` → `.docx` / `.html`.

- Do **not** edit the generated `.docx` and expect it to stick — the next
  build overwrites it, and git cannot meaningfully diff a binary `.docx`.
- When a collaborator (grants office, Dr. Jeng) returns Word
  track-changes/comments, an owner reads them and **transcribes the intent
  back into `PROPOSAL.md` by hand**, then regenerates. Word→Markdown
  auto-conversion is lossy here (it destroys the Mermaid diagram source and
  the `[CONFIRM:…]` callout structure) and is not used.
- Change history lives in git as reviewable **Markdown diffs**; distributable
  snapshots live as tagged **Releases**.

## CONFIRM items

The build keeps `⚑ CONFIRM` markers visible on purpose — they are the team's
pre-submission action list. Resolve them in `PROPOSAL.md`; keep
`CONFIRM-LIST.md` in sync; rebuild.
