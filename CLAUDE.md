# ClinicClaw — Project Instructions

> Last updated: 2026-03-11

## What ClinicClaw Is

An AI-native, FHIR R4-native Hospital Information System (HIS) that uses VERITAS as its trust and governance layer. Every AI agent action is policy-bound, audited, and verifiable. Pluggable LLM backend (Claude API, Ollama, mock). Real FHIR data model. Real clinical workflows.

Think: what a modern HIS looks like when AI is a first-class citizen, not an afterthought bolted onto a legacy system.

## What ClinicClaw Is NOT

- NOT a replacement for VERITAS (ClinicClaw depends on VERITAS principles)
- NOT a FHIR server (Medplum handles FHIR storage; ClinicClaw is the intelligence layer)
- NOT a billing system or EHR clone
- NOT trying to replicate Epic — solving problems Epic can't solve
- NOT a research prototype — designed for production deployment

## Architecture

```
┌──────────────────────────────────────────────────────────┐
│                  ClinicClaw Agents (8)                     │
│  Triage │ Nurse │ AmbientDoc │ OrderEntry │ LabReview    │
│  PharmacyReview │ PriorAuth │ DischargePlan              │
├──────────────────────────────────────────────────────────┤
│                  VERITAS Trust Layer                       │
│  OPA Rego Policy Engine │ Audit Chain │ Capabilities      │
├──────────────────────────────────────────────────────────┤
│                  FHIR Data Layer                          │
│  Patient │ Encounter │ MedicationRequest │ Observation    │
├──────────────────────────────────────────────────────────┤
│                  Pluggable AI Layer                        │
│  Claude API │ Ollama (local) │ Mock (deterministic)       │
└──────────────────────────────────────────────────────────┘
```

## Design Principles

1. VERITAS governs all AI actions — no agent logic runs without a policy gate
2. FHIR R4 as the native data model — no proprietary schemas
3. Pluggable LLM — Claude API, Ollama, or mock for deterministic testing
4. Async-first — tokio runtime (unlike VERITAS core which is sync)
5. Pluggable FHIR backend — Medplum by default, any FHIR R4 server works
6. HIPAA-aware by design — audit everything, minimize PHI surface area
7. Agent-first — every clinical workflow is expressed as a policy-bound agent
8. Human-in-the-loop — approval gates enforced by policy, not UI convention

## Crates

| Crate | Purpose |
|-------|---------|
| `cliniclaw-fhir` | FHIR R4 client, resource types, Synthea bundle importer |
| `cliniclaw-agents` | 8 AI agents + LlmCapability trait (Claude/Ollama/Mock) |
| `cliniclaw-policy` | OPA Rego policy engine (regorus) + clinical skill metadata (TOML) |
| `cliniclaw-api` | axum HTTP API server (20+ routes, SSE events, demo orchestrator) |
| `cliniclaw-persist` | SQLite/Postgres audit store with SHA-256 hash chain |
| `cliniclaw-kernel` | Workspace/turn store, AgentEvent system, EventEmitter |
| `cliniclaw-tui` | Terminal UI with hospital view, live metrics, event detail |

## Key Stack

- Language: Rust (async, tokio)
- FHIR backend: Medplum (self-hosted or cloud) via REST API
- AI: Claude API / Ollama / Mock via `LlmCapability` trait
- Policy: OPA Rego via regorus 0.2
- Storage: SQLite (dev) / PostgreSQL (prod) via sqlx
- HTTP server: axum 0.7
- Web: Next.js 15, Tailwind CSS, lucide-react, react-three-fiber (3D)
- Trust layer: VERITAS execution model re-implemented for async I/O

## Running

```bash
# Backend (port 3001)
CLINICLAW_MOCK=true LISTEN_ADDR=0.0.0.0:3001 cargo run -p cliniclaw-api

# Frontend (port 3000)
cd web && npm run dev

# With Ollama
LLM_BACKEND=ollama OLLAMA_MODEL=mistral-small LISTEN_ADDR=0.0.0.0:3001 cargo run -p cliniclaw-api

# With Synthea data
SYNTHEA_DIR=data/synthea/fhir CLINICLAW_MOCK=true LISTEN_ADDR=0.0.0.0:3001 cargo run -p cliniclaw-api
```

## Web Pages

| Route | Purpose |
|-------|---------|
| `/` | Clinician worklist |
| `/demo` | Scripted chest pain demo (dual-pane, 2 approval gates) |
| `/hospital` | Dynamic multi-patient simulation (swim lanes) |
| `/hospital/3d` | 3D hospital floor + network graph |
| `/audit` | Audit trail viewer |
| `/admin` | Admin dashboard |
| `/chart/[patientId]/*` | Patient chart (notes, orders, prior-auth, review) |

## VERITAS Relationship

ClinicClaw implements the same execution model as VERITAS:

```
State → Policy → Capability → Agent → Verify → Audit → Next State
```

But adapted for async Rust and real external I/O (FHIR API calls, LLM calls).
VERITAS itself (at /Volumes/extraSupply/veritas) remains the synchronous reference implementation.

When in doubt: follow VERITAS design decisions. Deny by default. Audit everything.
Evidence over intelligence. Control over autonomy.

## Code Guidelines

- All agent actions must be gated by a policy check — no exceptions
- All LLM calls must be wrapped in a capability — never call Claude API directly from agent logic
- PHI must never appear in log output — use identifiers, not patient names/DOB/MRN
- FHIR resources are the source of truth — do not duplicate clinical data in app schemas
- Async throughout — but keep the policy check itself as a fast, synchronous operation
- Errors must be typed (thiserror 2.0) — no `anyhow` in library crates, only in binaries/tests
- Each crate has its own error type; cross-crate errors use `From` impl
- Tests use synthetic FHIR resources (Synthea-style) — never real patient data
- Follow VERITAS principle: explicit over implicit, small over large

## Key Crate Versions

- `thiserror = "2.0"` (NOT 1.0)
- `axum = "0.7"` (NOT 0.6)
- `sqlx = "0.8"` (NOT 0.7)
- `reqwest = "0.12"` with `rustls-tls`, no `native-tls`
- `tokio = "1"` with `features = ["full"]`
- `regorus = "0.2"` with `set_rego_v1(true)`

## Reference Projects

- Medplum: https://github.com/medplum/medplum (FHIR backend)
- HAPI FHIR: https://github.com/hapifhir/hapi-fhir (alternative FHIR server)
- Helios FHIR: https://github.com/HeliosSoftware/hfs (Rust FHIR server, watch)
- WSO2 FHIR MCP: https://github.com/wso2/fhir-mcp-server (FHIR-LLM bridge)
- Synthea: https://github.com/synthetichealth/synthea (synthetic patient data generator)
- SMART-on-FHIR: https://github.com/smart-on-fhir (app auth standard)
- VERITAS: /Volumes/extraSupply/veritas (trust layer reference implementation)
