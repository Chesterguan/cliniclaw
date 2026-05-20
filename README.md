# ClinicClaw

**What if your hospital's AI was policy-gated, auditable, and FHIR-native from day one?**

ClinicClaw is an AI-native Hospital Information System (HIS) where every agent action is governed by OPA Rego policies, cryptographically audited, and built on real FHIR R4 data — not bolted onto a legacy system as an afterthought. The LLM backend is fully pluggable: Claude API, Ollama, or a deterministic mock for offline development.

> This is a research/demo project (v0.1.0). Auth is a dev-mode passthrough. Do not deploy with real patient data.

## Architecture

```
ClinicClaw Agents (8 clinical workflows)
        │
VERITAS Trust Layer (OPA Rego policy engine, audit chain, capabilities)
        │
FHIR Data Layer (Medplum or any FHIR R4 server)
        │
Pluggable AI (Claude API │ Ollama │ Mock)
```

Execution model: `State → Policy → Capability → Agent → Verify → Audit → FHIR Write`

## 8 Clinical Agents

| Agent | Action | Description |
|---|---|---|
| Triage Assessment | `triage_assess` | ESI 1-5 acuity scoring, ACS protocol |
| Nurse Assessment | `nurse_assess` | Nursing evaluation (Joint Commission NPSG) |
| Ambient Documentation | `ambient_doc` | Transcript → SOAP note with ICD-10 coding |
| Order Entry | `order_entry` | Natural language → FHIR ServiceRequest/MedicationRequest |
| Lab Review | `lab_review` | Lab interpretation, critical value flagging |
| Pharmacy Review | `pharmacy_review` | Drug interaction check, dosing advisory |
| Prior Authorization | `prior_auth` | Payer submission workflow |
| Discharge Planning | `discharge_plan` | Discharge instructions + follow-up coordination |

## Prerequisites

- Rust 1.75+ (tested on 1.88)
- Node.js 20+ (tested on 25.x)
- npm 10+

## Quick Start

```bash
# Clone
git clone https://github.com/Chesterguan/cliniclaw.git
cd cliniclaw

# Backend (port 3001) — mock LLM, no external dependencies
CLINICLAW_MOCK=true LISTEN_ADDR=0.0.0.0:3001 cargo run -p cliniclaw-api

# Frontend (port 3000) — in a second terminal
cd web && npm install && npm run dev
```

Open [http://localhost:3000](http://localhost:3000) and explore.

### Other modes

```bash
# With Ollama (local LLM)
LLM_BACKEND=ollama OLLAMA_MODEL=mistral-small LISTEN_ADDR=0.0.0.0:3001 cargo run -p cliniclaw-api

# With Claude API
LLM_BACKEND=claude CLAUDE_API_KEY=sk-... LISTEN_ADDR=0.0.0.0:3001 cargo run -p cliniclaw-api

# With Synthea synthetic patient data
SYNTHEA_DIR=data/synthea/fhir CLINICLAW_MOCK=true LISTEN_ADDR=0.0.0.0:3001 cargo run -p cliniclaw-api
```

## Web Pages

| Route | What you'll see |
|---|---|
| `/` | Clinician worklist — patient list with status |
| `/demo` | Scripted chest pain scenario (dual-pane, 2 human-in-the-loop approval gates) |
| `/hospital` | Dynamic multi-patient simulation with swim-lane visualization |
| `/hospital/3d` | 3D hospital floor + network graph |
| `/audit` | Audit trail viewer (SHA-256 hash chain) |
| `/admin` | Admin dashboard |
| `/chart/[patientId]/*` | Patient chart — notes, orders, prior-auth, review, audit trail |

## Crates

| Crate | Purpose |
|---|---|
| `cliniclaw-fhir` | FHIR R4 client, resource types, Synthea bundle importer |
| `cliniclaw-agents` | 8 AI agents + LlmCapability trait (Claude/Ollama/Mock) + model registry + drift monitor |
| `cliniclaw-policy` | OPA Rego policy engine + clinical skill metadata (TOML) |
| `cliniclaw-persist` | SQLite/Postgres audit store with SHA-256 hash chain |
| `cliniclaw-kernel` | Workspace/turn store, AgentEvent system, EventEmitter |
| `cliniclaw-api` | axum HTTP API server (31 routes, SSE events, demo orchestrator) |
| `cliniclaw-tui` | Terminal UI with hospital view and live metrics |

## Stack

- **Backend:** Rust (async/tokio), axum 0.7, sqlx 0.8, reqwest 0.12 (rustls-tls)
- **Policy:** OPA Rego via regorus 0.2
- **Frontend:** Next.js 15, Tailwind CSS, Three.js (react-three-fiber)
- **LLM:** Claude API / Ollama / Mock (pluggable via `LlmCapability` trait)
- **FHIR:** Medplum or any FHIR R4 server

## Tests

```bash
cargo test --workspace    # 220 tests, all passing
```

## Docker

```bash
docker compose up         # backend + frontend

# Or individually:
docker build -t cliniclaw .
docker run -p 3001:3001 cliniclaw
```

## How it works

Every agent follows the VERITAS execution model:

1. **State** — FHIR resources describe the current clinical state
2. **Policy** — OPA Rego policy decides: allow, deny, or require approval
3. **Capability** — LLM call is wrapped in a capability with confidence tracking
4. **Agent** — clinical logic produces a structured output
5. **Verify** — output is validated against clinical rules
6. **Audit** — SHA-256 hash-chained event is written to the audit store
7. **FHIR Write** — result is written back as FHIR resources

Human-in-the-loop approval gates are enforced by policy, not by UI convention.

## Governance Benchmark

ClinicClaw's governance properties are validated by [VeritasBench](https://github.com/Chesterguan/veritasbench) — the first benchmark for AI agent governance (500 scenarios, 7 types).

```
                          Policy     Safety    Trace-      Control-
                          Compliance            ability     lability
ClinicClaw (VERITAS)        98%       96%       100%        100%
LangGraph + HITL            58%       59%        33%        100%
OpenAI Guardrails           51%       48%        29%          0%
NeMo Guardrails             50%       46%         0%          0%
Bare LLM                    49%       46%         0%          0%
```

## Related Projects

- **[AesculTwin](https://github.com/Chesterguan/AesculTwin)** — The Surgical Second Brain. A SMART on FHIR application for cardiovascular surgeons, featuring RAG-powered knowledge base, surgical video analysis, and performance analytics. Grant proposals (including the CV Surgical Copilot proposal for Dr. Eric I. Jeng) live in that repo.
- **[VERITAS](https://github.com/Chesterguan/veritas)** — The trust-layer reference implementation that ClinicClaw's governance is built on.
- **[VeritasBench](https://github.com/Chesterguan/veritasbench)** — Governance benchmark (500 scenarios, 7 types) used to validate ClinicClaw's policy compliance.

## License

Apache-2.0
