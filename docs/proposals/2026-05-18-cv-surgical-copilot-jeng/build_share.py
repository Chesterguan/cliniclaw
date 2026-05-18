#!/usr/bin/env python3
"""Build shareable Word + HTML from PROPOSAL.md (working version, CONFIRM callouts visible)."""
import os, re, subprocess, sys, html, json, shutil

BASE = os.path.dirname(os.path.abspath(__file__))
SRC = os.path.join(BASE, "PROPOSAL.md")
REFS = os.path.join(BASE, "REFERENCES.md")
DIST = os.path.join(BASE, "dist")
FIGS = os.path.join(DIST, "figs")
os.makedirs(FIGS, exist_ok=True)

raw = open(SRC, encoding="utf-8").read()
refs = open(REFS, encoding="utf-8").read()

# 1. strip internal provenance HTML comments (clutter for external readers)
raw = re.sub(r"<!--.*?-->", "", raw, flags=re.S)

# 2. inline the references under "## 10. References" (drop the file's own H1)
refs_body = re.sub(r"^#\s*References\s*\n", "", refs, count=1).strip()
raw = re.sub(r"(##\s*10\.\s*References\s*\n)", r"\1\n" + refs_body.replace("\\", "\\\\") + "\n", raw, count=1)

# 3. extract mermaid blocks -> render PNG (docx) + SVG (html)
mmd_re = re.compile(r"```mermaid\n(.*?)\n```", re.S)
blocks = mmd_re.findall(raw)
print(f"mermaid blocks found: {len(blocks)}")

pup = os.path.join(DIST, "puppeteer.json")
open(pup, "w").write(json.dumps({"args": ["--no-sandbox"]}))
mmconf = os.path.join(DIST, "mermaid.json")
open(mmconf, "w").write(json.dumps({"theme": "neutral", "flowchart": {"htmlLabels": True, "curve": "basis"}}))

for i, code in enumerate(blocks, 1):
    mmd = os.path.join(FIGS, f"figure-{i}.mmd")
    open(mmd, "w", encoding="utf-8").write(code)
    for ext, extra in (("png", ["-s", "3"]), ("svg", [])):
        out = os.path.join(FIGS, f"figure-{i}.{ext}")
        cmd = ["npx", "-y", "@mermaid-js/mermaid-cli", "-i", mmd, "-o", out,
               "-b", "white", "-c", mmconf, "-p", pup] + extra
        r = subprocess.run(cmd, capture_output=True, text=True)
        if r.returncode != 0 or not os.path.exists(out):
            print(f"  FIG {i} {ext} FAILED:\n{r.stderr[-800:]}"); sys.exit(1)
        print(f"  figure-{i}.{ext} ok")

# helper: replace nth mermaid block with a per-format image reference
def sub_blocks(text, ext, attrs):
    idx = [0]
    def repl(m):
        idx[0] += 1
        return f'![](figs/figure-{idx[0]}.{ext}){attrs}'
    return mmd_re.sub(repl, text)

# 4. CONFIRM -> visible callout. Only match the real "[CONFIRM: ... — owner: ...]" form,
#    NOT the descriptive "[CONFIRM: …]" notation note in the status banner.
conf_re = re.compile(r"\[CONFIRM:\s*(?P<body>[^\]]*?\bowner:[^\]]*?)\]")

def confirm_html(m):
    return ('<span class="confirm"><strong>⚑ CONFIRM</strong> &mdash; '
            + html.escape(m.group("body").strip()) + "</span>")

def confirm_docx(m):
    # bold + flag marker: unambiguously visible & editable in Word without a reference doc
    body = m.group("body").strip().replace("|", "\\|")
    return f" **⚑ CONFIRM — {body}** "

html_md = sub_blocks(raw, "svg", "")
html_md = conf_re.sub(confirm_html, html_md)

docx_md = sub_blocks(raw, "png", "{width=92%}")
docx_md = conf_re.sub(confirm_docx, docx_md)

open(os.path.join(DIST, "_html.md"), "w", encoding="utf-8").write(html_md)
open(os.path.join(DIST, "_docx.md"), "w", encoding="utf-8").write(docx_md)

# 5. CSS for the HTML (human-friendly, document-like, CONFIRM callouts)
CSS = r"""
:root{--ink:#1a2230;--mut:#5b6675;--line:#e3e7ee;--brand:#0b5fa5;--warn:#9a6a00}
*{box-sizing:border-box}
html{-webkit-text-size-adjust:100%}
body{font:16px/1.65 -apple-system,BlinkMacSystemFont,"Segoe UI",Inter,Roboto,Helvetica,Arial,sans-serif;
 color:var(--ink);max-width:860px;margin:0 auto;padding:56px 28px 120px;background:#fff}
h1{font-size:2rem;line-height:1.25;margin:.2em 0 .1em;letter-spacing:-.01em}
h2{font-size:1.4rem;margin:2.4em 0 .5em;padding-bottom:.25em;border-bottom:2px solid var(--line)}
h3{font-size:1.13rem;margin:1.8em 0 .4em;color:#27313f}
h4{font-size:1rem;margin:1.4em 0 .3em;color:var(--mut);text-transform:uppercase;letter-spacing:.04em}
p,li{margin:.55em 0}
a{color:var(--brand);text-decoration:none}a:hover{text-decoration:underline}
blockquote{margin:1.2em 0;padding:.6em 1.1em;background:#f4f7fb;border-left:4px solid var(--brand);
 color:var(--mut);font-size:.93rem;border-radius:0 6px 6px 0}
blockquote p{margin:.3em 0}
img{max-width:100%;height:auto;display:block;margin:1.1em auto;
 border:1px solid var(--line);border-radius:8px;padding:10px;background:#fff}
table{border-collapse:collapse;width:100%;margin:1.2em 0;font-size:.92rem}
th,td{border:1px solid var(--line);padding:7px 10px;text-align:left;vertical-align:top}
th{background:#f4f7fb}
hr{border:0;border-top:1px solid var(--line);margin:2.4em 0}
code{background:#f3f5f8;padding:.1em .35em;border-radius:4px;font-size:.88em}
.confirm{display:inline;background:#fff3cd;color:var(--warn);border:1px solid #f0d27a;
 border-radius:5px;padding:1px 7px;font-size:.86em;line-height:1.9}
.confirm strong{color:#7a5400;letter-spacing:.02em}
#TOC{background:#fafbfc;border:1px solid var(--line);border-radius:8px;padding:14px 22px;margin:1.6em 0 2.4em}
#TOC::before{content:"Contents";display:block;font-weight:700;font-size:.8rem;
 text-transform:uppercase;letter-spacing:.06em;color:var(--mut);margin-bottom:.4em}
#TOC ul{margin:.2em 0;padding-left:1.1em}#TOC a{color:var(--ink)}
@media print{body{max-width:none;padding:0;font-size:11pt}.confirm{background:#fff3cd!important;-webkit-print-color-adjust:exact}}
@media(max-width:600px){body{padding:32px 16px}h1{font-size:1.55rem}}
"""
cssf = os.path.join(DIST, "style.css"); open(cssf, "w").write(CSS)

OUT_HTML = os.path.join(DIST, "ClinicClaw-CV-Copilot-Proposal.html")
OUT_DOCX = os.path.join(DIST, "ClinicClaw-CV-Copilot-Proposal.docx")

def run(cmd):
    r = subprocess.run(cmd, capture_output=True, text=True, cwd=DIST)
    if r.returncode != 0:
        print("PANDOC FAIL:", " ".join(cmd), "\n", r.stderr[-1200:]); sys.exit(1)

# 6a. self-contained HTML (SVG figures + CSS embedded)
run(["pandoc", "_html.md", "-o", OUT_HTML, "--standalone", "--embed-resources",
     "--toc", "--toc-depth=3", "-c", "style.css", "--metadata", "lang=en",
     "--metadata", "title=ClinicClaw — Cardiovascular Surgical Copilot Proposal"])

# 6b. editable Word with heading styles + TOC field (navigation pane works)
run(["pandoc", "_docx.md", "-o", OUT_DOCX, "--toc", "--toc-depth=3",
     "--metadata", "title=ClinicClaw — Cardiovascular Surgical Copilot Proposal"])

for f in (OUT_HTML, OUT_DOCX):
    print(f"  {os.path.basename(f)}  {os.path.getsize(f):,} bytes")
print("BUILD OK")
