# ClinicClaw

AI-native, FHIR R4-native Hospital Information System

ClinicClaw is the intelligence layer that sits on top of a FHIR R4 backend (Medplum or any FHIR R4 server) — not an EHR clone, not a FHIR server. Every AI agent action is governed by the VERITAS trust model: policy-gated, audited, and verifiable. The LLM backend is fully pluggable (Claude API, Ollama, or deterministic Mock).

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

## Quick Start

```bash
# Backend (port 3001)
CLINICLAW_MOCK=true LISTEN_ADDR=0.0.0.0:3001 cargo run -p cliniclaw-api

# Frontend (port 3000)
cd web && npm install && npm run dev

# With Ollama
LLM_BACKEND=ollama OLLAMA_MODEL=mistral-small LISTEN_ADDR=0.0.0.0:3001 cargo run -p cliniclaw-api

# With Synthea data
SYNTHEA_DIR=data/synthea/fhir CLINICLAW_MOCK=true LISTEN_ADDR=0.0.0.0:3001 cargo run -p cliniclaw-api
```

## Web Pages

| Route | Purpose |
|---|---|
| `/` | Clinician worklist |
| `/demo` | Scripted chest pain demo (dual-pane, 2 approval gates) |
| `/hospital` | Dynamic multi-patient simulation (swim lanes) |
| `/hospital/3d` | 3D hospital floor + network graph |
| `/audit` | Audit trail viewer |
| `/admin` | Admin dashboard |
| `/chart/[patientId]/*` | Patient chart (notes, orders, prior-auth, review) |

## Crates

| Crate | Purpose |
|---|---|
| `cliniclaw-fhir` | FHIR R4 client, resource types, Synthea bundle importer |
| `cliniclaw-agents` | 8 AI agents + LlmCapability trait (Claude/Ollama/Mock) + model registry |
| `cliniclaw-policy` | OPA Rego policy engine + clinical skill metadata (TOML) |
| `cliniclaw-persist` | SQLite/Postgres audit store with SHA-256 hash chain |
| `cliniclaw-kernel` | Workspace/turn store, AgentEvent system, EventEmitter |
| `cliniclaw-api` | axum HTTP API server (20+ routes, SSE events, demo orchestrator) |
| `cliniclaw-tui` | Terminal UI with hospital view, live metrics |

## Stack

- Rust (async, tokio), axum 0.7, sqlx 0.8, reqwest 0.12 (rustls-tls)
- OPA Rego (regorus 0.2) for policy evaluation
- Next.js 15 + Tailwind CSS + Three.js (react-three-fiber) for web
- Pluggable LLM: Claude API / Ollama / Mock
- FHIR backend: Medplum (or any FHIR R4 server)

## Tests

```bash
cargo test --workspace    # 176 tests
```

## Docker

```bash
docker build -t cliniclaw .
docker run -p 3001:3001 cliniclaw

# Frontend
cd web && docker build -t cliniclaw-web .
docker run -p 3000:3001 cliniclaw-web
```

## License

Apache-2.0
