# ClinicClaw — Architecture

> Last updated: 2026-02-18

## Overview

ClinicClaw is an AI-native, FHIR R4-native Hospital Information System (HIS). It is not a
traditional HIS that adds AI as a feature. It is a system where AI agents are the primary
actors, every agent action is governed by a policy engine, and the FHIR R4 data model is
the only source of truth.

The architecture has three distinct layers, each with a clear contract and boundary.

---

## Three-Layer Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│  LAYER 3 — ClinicClaw Agents (cliniclaw-agents)                  │
│                                                                  │
│   AmbientDocAgent    OrderEntryAgent    PriorAuthAgent           │
│                                                                  │
│   Each agent:                                                    │
│     - Receives a typed input (FHIR resource or derived struct)   │
│     - Calls Claude API via a capability wrapper                  │
│     - Returns a typed, schema-validated output                   │
│     - Never writes to FHIR directly — passes output upward       │
└────────────────────────┬─────────────────────────────────────────┘
                         │ agent output (proposed action)
                         ▼
┌──────────────────────────────────────────────────────────────────┐
│  LAYER 2 — VERITAS Trust Layer (cliniclaw-policy + contracts)    │
│                                                                  │
│   Policy Engine        Audit Trail        Verifier               │
│   ─────────────        ───────────        ────────               │
│   TOML rules           Append-only        JSON Schema +          │
│   deny-by-default      hash-chained       semantic rules         │
│   capability check     event log          output validation       │
│                                                                  │
│   Execution order (per agent action):                            │
│     1. Policy check  → Allow / Deny / RequireApproval            │
│     2. Capability gate → verified capability token               │
│     3. Agent runs (only if step 1+2 pass)                        │
│     4. Output verification → schema + rule check                 │
│     5. Audit event written → immutable record                    │
│     6. FHIR write dispatched (only if step 4 passes)             │
└────────────────────────┬─────────────────────────────────────────┘
                         │ validated, audited FHIR resource
                         ▼
┌──────────────────────────────────────────────────────────────────┐
│  LAYER 1 — FHIR Data Layer (cliniclaw-fhir → Medplum REST)       │
│                                                                  │
│   Patient    Encounter    MedicationRequest    Observation        │
│   DiagnosticReport    ServiceRequest    ClaimResponse            │
│                                                                  │
│   Medplum provides:                                              │
│     - FHIR R4 REST API (create, read, update, search)           │
│     - $everything operation for patient bundles                  │
│     - Subscriptions for event-driven triggers                    │
│     - Auth via SMART-on-FHIR (OAuth2 + PKCE)                    │
└──────────────────────────────────────────────────────────────────┘
```

---

## Async Execution Model

Unlike VERITAS core (which is fully synchronous), ClinicClaw is async-first because its
critical path includes network I/O: FHIR API reads, Claude API calls, and FHIR API writes.

The execution model per agent action:

```
async fn run_agent_action(input: AgentInput) -> Result<AuditedOutput, ClinicError> {

    // Step 1: Policy check (fast, synchronous inside async context)
    let policy_result = policy_engine.check(&input.action, &input.context).await?;
    ensure!(policy_result == PolicyDecision::Allow, ClinicError::PolicyDenied);

    // Step 2: Capability gate
    let capability = capability_store.acquire(&input.action.required_capability)?;

    // Step 3: Agent runs (calls Claude API via capability)
    let proposed_output = agent.propose(input, capability).await?;

    // Step 4: Output verification
    verifier.verify(&proposed_output)?;

    // Step 5: Audit record written
    audit_writer.append(AuditEvent::from(&input, &proposed_output)).await?;

    // Step 6: FHIR write
    fhir_client.create_or_update(proposed_output.fhir_resource).await?;

    Ok(AuditedOutput { ... })
}
```

Key properties:
- Policy check is always first — the agent never runs if policy denies
- Audit is written before FHIR write — if FHIR write fails, the audit still exists
- Agent.propose() is never called without a valid capability token
- Output verification happens before any external write

---

## FHIR R4 Resource Types

ClinicClaw works with a focused subset of FHIR R4. These are the resources that map to
the three initial clinical workflows:

| Resource | Used In | Description |
|----------|---------|-------------|
| `Patient` | All workflows | Demographic context, consent flags |
| `Encounter` | Ambient doc, Order entry | Active clinical encounter |
| `Observation` | Ambient doc | Vital signs, clinical findings |
| `DiagnosticReport` | Ambient doc | Generated clinical note as FHIR resource |
| `MedicationRequest` | Order entry | Medication orders generated by AI |
| `ServiceRequest` | Order entry | Lab/imaging orders generated by AI |
| `ClaimResponse` | Prior auth | Payer response to PA request |
| `Task` | Prior auth | PA workflow state tracking |
| `Practitioner` | All workflows | Ordering provider identity |
| `Organization` | Prior auth | Payer organization |

### FHIR Resource Lifecycle

```
Encounter opens
     │
     ├─► AmbientDocAgent reads Encounter + Observation
     │         └─► writes DiagnosticReport (note)
     │
     ├─► OrderEntryAgent reads DiagnosticReport + Practitioner intent
     │         └─► writes MedicationRequest / ServiceRequest
     │
     └─► PriorAuthAgent reads MedicationRequest + ClaimResponse
               └─► writes Task (PA status) + updates MedicationRequest
```

---

## Clinical Workflows (Phase 1)

### 1. Ambient Documentation

The most impactful workflow. During a patient encounter, the system listens to the
conversation (or receives a transcript) and generates a structured clinical note.

```
Input:
  Encounter (FHIR) + audio transcript (text) + prior Observations

Policy checks:
  - Practitioner has "note_generation" capability
  - Encounter is in "in-progress" status
  - Patient consent for AI note generation is present

Agent action:
  AmbientDocAgent calls Claude API with:
    - De-identified or minimized PHI (use FHIR IDs, not names in prompt)
    - Transcript text
    - Relevant context (chief complaint, active medications)

Output:
  DiagnosticReport (FHIR) with:
    - presentedForm: structured note text
    - conclusion: ICD-10 codes (if confidence is high)
    - status: "preliminary" (requires physician review before "final")

Verification rules:
  - Output must be valid DiagnosticReport JSON
  - status must be "preliminary" (agents cannot create "final" notes)
  - Must include at least one presentedForm entry

Audit event:
  action: "ambient_note_generated"
  inputs: [encounter_id, practitioner_id]
  output_hash: SHA-256 of DiagnosticReport JSON
  timestamp + chain hash
```

Data flow:

```
Transcript
    │
    ▼
[AmbientDocAgent]──Claude API call──►[Claude response: structured note]
    │                                           │
    │◄──────── output verification ─────────────┘
    │
    ▼
[Audit record written]
    │
    ▼
[DiagnosticReport → Medplum FHIR API]
    │
    ▼
Physician review queue (status: preliminary)
```

---

### 2. Intelligent Order Entry

A practitioner describes an order in natural language. The agent parses it into a
structured FHIR MedicationRequest or ServiceRequest, checks for conflicts, and
submits for approval.

```
Input:
  Practitioner intent (free text) + Patient context (active meds, allergies)
  + Encounter reference

Policy checks:
  - Practitioner has "order_entry" capability
  - Patient has no active consent hold on automated orders
  - Drug is on the hospital formulary (capability check)

Agent action:
  OrderEntryAgent calls Claude API with:
    - Practitioner's natural language intent
    - Patient's active medication list (from FHIR MedicationStatement)
    - Known allergies (from FHIR AllergyIntolerance)

Output:
  MedicationRequest (FHIR) or ServiceRequest (FHIR) with:
    - status: "draft" (requires practitioner co-signature)
    - requester: Practitioner reference
    - reasonReference: Encounter reference

Verification rules:
  - Output must be valid MedicationRequest or ServiceRequest JSON
  - status must be "draft"
  - requester must match the requesting practitioner
  - Medication must be in formulary (semantic rule)

Audit event:
  action: "order_proposed"
  inputs: [practitioner_id, patient_id, encounter_id]
  output_hash: SHA-256 of order JSON
```

Data flow:

```
Practitioner natural language intent
    │
    ▼
[OrderEntryAgent]──Claude API call──►[Claude: structured FHIR order]
    │                                           │
    │        Drug interaction check ────────────┤
    │        Formulary check        ────────────┤
    │                                           │
    │◄──────── output verification ─────────────┘
    │
    ▼
[Audit record]
    │
    ▼
[MedicationRequest (status: draft) → Medplum]
    │
    ▼
Practitioner co-signature UI
```

---

### 3. Prior Authorization

The most regulation-heavy workflow. When a MedicationRequest or ServiceRequest requires
payer authorization, the agent assembles the PA request, submits it, and tracks status.

```
Input:
  MedicationRequest or ServiceRequest (FHIR) + Patient insurance info
  + Clinical justification (from DiagnosticReport)

Policy checks:
  - Request has "prior_auth" capability
  - Payer connection is available (capability check)
  - RequireApproval: human must review the PA package before submission

Agent action (two phases):
  Phase A — PA package assembly:
    PriorAuthAgent calls Claude API with:
      - Clinical justification narrative
      - Relevant diagnoses (ICD-10)
      - Drug/service details
    Output: structured PA request document + CoverageEligibilityRequest (FHIR)

  Phase B — Submission and tracking:
    After RequireApproval gate (human reviews PA package):
    Agent submits to payer FHIR endpoint
    Polls for ClaimResponse
    Updates Task resource with status

Verification rules:
  - PA package must include required fields (diagnosis, justification, NPI)
  - CoverageEligibilityRequest must be valid FHIR
  - ClaimResponse must be parsed for outcome (approved/denied/pending)

Audit events:
  action: "prior_auth_package_assembled"
  action: "prior_auth_submitted"
  action: "prior_auth_response_received"
```

Data flow:

```
MedicationRequest (status: active, requires PA)
    │
    ▼
[PriorAuthAgent Phase A]──Claude API──►[PA narrative + CoverageEligibilityRequest]
    │
    ▼
[HUMAN REVIEW GATE — RequireApproval]
    │
    ├─► Approved by physician
    │       │
    │       ▼
    │   [PriorAuthAgent Phase B]──►[Payer FHIR endpoint]
    │       │
    │       ▼
    │   [ClaimResponse received]
    │       │
    │       ├─► outcome: complete/approved → MedicationRequest status: "active"
    │       └─► outcome: denied → Task status: "rejected", notify care team
    │
    └─► Rejected by physician → Task cancelled, audit record
```

---

## HIPAA Considerations

### PHI Minimization in LLM Calls

Claude API calls are the highest-risk surface for PHI exposure. The policy engine enforces:

1. **No direct PHI in prompts** — agent capabilities must de-identify before calling Claude.
   Use FHIR resource IDs, not patient names. Use age ranges, not exact DOB.
   Use condition categories, not exact diagnoses where possible.

2. **Prompt templates are audited** — the capability wrapper logs a hash of the prompt
   (not the prompt itself) to the audit trail.

3. **Response caching is disabled** — Claude API responses containing clinical context
   must not be cached by any middleware layer.

4. **TLS everywhere** — reqwest is configured with `rustls-tls`, no `native-tls`.
   No unencrypted connections to FHIR backend or Claude API.

### Audit Trail Requirements

Every agent action produces an audit event containing:
- Timestamp (UTC, RFC 3339)
- Actor (practitioner FHIR ID)
- Patient (FHIR ID only — no name/DOB in audit log)
- Action type
- Policy decision (Allow/Deny/RequireApproval)
- Input hash (SHA-256)
- Output hash (SHA-256)
- Chain hash (links to previous event — tamper-evident)

The audit trail is append-only. Events are never deleted. This satisfies HIPAA
audit log retention requirements (6 years minimum).

### Access Control

- SMART-on-FHIR (OAuth2 + PKCE) for practitioner authentication
- Capability-based access control via VERITAS policy engine
- No ambient authority — every agent must hold a named capability token
- Consent flags on Patient resource are checked before any AI action

---

## Claude API Call Safety

```rust
// All Claude API calls go through this capability wrapper.
// The wrapper enforces:
//   1. PHI minimization (caller must provide de-identified prompt)
//   2. Audit logging (prompt hash + response hash)
//   3. Token budget enforcement (max_tokens policy)
//   4. Retry with exponential backoff (network errors only)
//   5. Response schema validation before returning

pub struct ClaudeCapability {
    client: reqwest::Client,
    api_key: SecretString,      // never logged
    model: String,
    max_tokens: u32,
    audit_writer: Arc<dyn AuditWriter>,
}

impl ClaudeCapability {
    pub async fn call(
        &self,
        prompt: DeidentifiedPrompt,  // typed wrapper — prevents raw PHI strings
        output_schema: &serde_json::Value,
    ) -> Result<VerifiedResponse, ClaudeError> {
        // ... policy check, call, validate, audit
    }
}
```

The `DeidentifiedPrompt` type is a newtype wrapper. It can only be constructed via
`DeidentifiedPrompt::build()` which requires the caller to attest (at the type level)
that PHI has been removed. This is a compile-time guarantee, not a runtime check.

---

## Phase 1 Scope

Phase 1 builds the foundation that all three workflows depend on.

### What gets built in Phase 1

| Component | Deliverable |
|-----------|-------------|
| `cliniclaw-fhir` | FHIR R4 client (Patient, Encounter, DiagnosticReport, MedicationRequest) + Medplum auth |
| `cliniclaw-policy` | TOML policy rules for all three workflows, deny-by-default |
| `cliniclaw-agents` | AmbientDocAgent (workflow 1 only in Phase 1) |
| `cliniclaw-persist` | SQLite-backed audit store |
| `cliniclaw-api` | Single axum endpoint: `POST /v1/encounter/:id/note` |

### What is explicitly out of Phase 1 scope

- OrderEntryAgent (Phase 2)
- PriorAuthAgent (Phase 2)
- Payer FHIR integration (Phase 2)
- Real SMART-on-FHIR auth (Phase 1 uses static API keys for dev)
- PostgreSQL migration (Phase 1 is SQLite only)
- Multi-tenant support
- UI / frontend of any kind

### Phase 1 success criteria

1. `POST /v1/encounter/:id/note` accepts an Encounter ID and transcript text
2. Returns a FHIR DiagnosticReport (status: "preliminary") written to Medplum
3. Every call produces an audit event in the SQLite store
4. A policy deny (missing capability, wrong encounter status) returns HTTP 403
   with a typed error body — never a 500
5. No PHI appears in any log output
6. All tests use Synthea-generated synthetic FHIR data

---

## Directory Structure (target end state)

```
cliniclaw/
├── Cargo.toml                  # workspace
├── CLAUDE.md
├── LICENSE
├── .gitignore
├── docs/
│   └── ARCHITECTURE.md
├── crates/
│   ├── cliniclaw-fhir/         # FHIR R4 client
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── client.rs       # reqwest FHIR client
│   │       ├── resources/      # Patient, Encounter, etc.
│   │       └── error.rs
│   ├── cliniclaw-agents/       # AI agents
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── ambient_doc.rs
│   │       ├── order_entry.rs
│   │       ├── prior_auth.rs
│   │       └── claude.rs       # ClaudeCapability wrapper
│   ├── cliniclaw-policy/       # TOML policy rules
│   │   ├── Cargo.toml
│   │   ├── policies/
│   │   │   ├── ambient_doc.toml
│   │   │   ├── order_entry.toml
│   │   │   └── prior_auth.toml
│   │   └── src/
│   │       └── lib.rs
│   ├── cliniclaw-persist/      # Audit + app storage
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs
│   └── cliniclaw-api/          # axum HTTP server
│       ├── Cargo.toml
│       └── src/
│           └── main.rs
└── tests/
    └── fixtures/
        └── synthetic/          # Synthea FHIR bundles (no real PHI)
```
