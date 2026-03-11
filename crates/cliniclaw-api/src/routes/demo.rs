//! Demo Orchestrator — scripted single-patient chest pain scenario.
//!
//! Designed for screen recording: a 90-120 second demo showing one patient
//! flowing through triage → human approval → orders → lab → pharmacy →
//! documentation, with policy gates and audit at every step.
//!
//! The demo pauses at approval points and waits for the frontend to POST
//! an approval before continuing. This creates the human-in-the-loop
//! narrative that audiences care about.

use std::sync::Arc;

use axum::extract::{Json, State};
use axum::http::StatusCode;
use tokio::sync::{Mutex, oneshot};

use cliniclaw_kernel::{AgentEventType, EventEmitter, StepStatus};

use crate::error::ApiError;
use crate::state::AppState;
use super::simulate::{ensure_workspace, persist_turn, phase_sleep};

// ── Demo state ───────────────────────────────────────────────────────────────

/// Demo scenario phases — the frontend shows these to the user.
#[derive(Debug, Clone, serde::Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DemoPhase {
    /// Waiting for demo to start
    Idle,
    /// Triage agent is evaluating
    TriageRunning,
    /// Triage complete, waiting for clinician to approve orders
    AwaitingOrderApproval,
    /// Order agent creating FHIR resources
    OrdersRunning,
    /// Lab results arrived, pharmacy reviewing
    LabAndPharmacyRunning,
    /// Pharmacy recommends medication, awaiting approval
    AwaitingMedApproval,
    /// Documentation agent generating SOAP note
    DocumentationRunning,
    /// All done — summary
    Complete,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DemoState {
    pub phase: DemoPhase,
    pub encounter_id: String,
    pub patient: DemoPatient,
    /// Triage recommendation (populated after triage completes)
    pub triage_result: Option<TriageResult>,
    /// Orders created (populated after order agent)
    pub orders: Vec<OrderResult>,
    /// Lab result (populated after lab agent)
    pub lab_result: Option<LabResult>,
    /// Medication recommendation (populated after pharmacy agent)
    pub med_recommendation: Option<MedRecommendation>,
    /// SOAP note (populated after documentation agent)
    pub soap_note: Option<SoapNote>,
    /// Summary stats
    pub stats: DemoStats,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DemoPatient {
    pub name: String,
    pub age: u32,
    pub gender: String,
    pub chief_complaint: String,
    pub vitals: String,
    pub history: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TriageResult {
    pub esi_level: u8,
    pub acuity: String,
    pub recommendations: Vec<String>,
    pub confidence: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct OrderResult {
    pub order_type: String,
    pub description: String,
    pub fhir_resource_id: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LabResult {
    pub test: String,
    pub value: String,
    pub unit: String,
    pub interpretation: String,
    pub critical: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MedRecommendation {
    pub medication: String,
    pub dose: String,
    pub route: String,
    pub rationale: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SoapNote {
    pub subjective: String,
    pub objective: String,
    pub assessment: String,
    pub plan: String,
    pub icd10_codes: Vec<String>,
    pub confidence: f64,
}

#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct DemoStats {
    pub agents_run: u32,
    pub human_approvals: u32,
    pub policy_checks: u32,
    pub fhir_writes: u32,
    pub audit_events: u32,
}

// ── Shared demo state (one demo at a time) ───────────────────────────────────

/// Global demo state — protected by mutex. Only one demo runs at a time.
pub struct DemoController {
    state: Mutex<DemoState>,
    /// Channel sender for the current approval wait, if any.
    approval_tx: Mutex<Option<oneshot::Sender<()>>>,
}

impl DemoController {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(DemoState {
                phase: DemoPhase::Idle,
                encounter_id: String::new(),
                patient: demo_patient(),
                triage_result: None,
                orders: vec![],
                lab_result: None,
                med_recommendation: None,
                soap_note: None,
                stats: DemoStats::default(),
            }),
            approval_tx: Mutex::new(None),
        }
    }
}

fn demo_patient() -> DemoPatient {
    DemoPatient {
        name: "John Doe".into(),
        age: 62,
        gender: "Male".into(),
        chief_complaint: "Acute onset substernal chest pain, radiating to left arm, started 45 minutes ago while climbing stairs. Diaphoretic. Rates pain 8/10.".into(),
        vitals: "HR 102, BP 158/94, RR 22, SpO2 96%, Temp 37.0C".into(),
        history: vec![
            "Hypertension (10 years)".into(),
            "Hyperlipidemia".into(),
            "Type 2 Diabetes (5 years)".into(),
            "Former smoker (quit 2019)".into(),
            "Family Hx: Father MI age 58".into(),
        ],
    }
}

// ── API handlers ─────────────────────────────────────────────────────────────

/// GET /v1/demo/state — returns current demo state for frontend polling.
pub async fn get_demo_state(
    State(state): State<Arc<AppState>>,
) -> Result<Json<DemoState>, ApiError> {
    let demo = state.demo.state.lock().await;
    Ok(Json(demo.clone()))
}

/// POST /v1/demo/start — begins the scripted chest pain scenario.
pub async fn start_demo(
    State(state): State<Arc<AppState>>,
) -> Result<Json<DemoState>, ApiError> {
    {
        let demo = state.demo.state.lock().await;
        if demo.phase != DemoPhase::Idle && demo.phase != DemoPhase::Complete {
            return Err(ApiError::new(
                StatusCode::CONFLICT,
                "demo is already running — POST /v1/demo/reset first",
            ));
        }
    }

    // Reset state
    {
        let mut demo = state.demo.state.lock().await;
        *demo = DemoState {
            phase: DemoPhase::Idle,
            encounter_id: "demo-enc-001".into(),
            patient: demo_patient(),
            triage_result: None,
            orders: vec![],
            lab_result: None,
            med_recommendation: None,
            soap_note: None,
            stats: DemoStats::default(),
        };
    }

    // Spawn the orchestrator
    let app_state = state.clone();
    tokio::spawn(async move {
        if let Err(e) = run_demo_scenario(app_state).await {
            tracing::error!(error = %e, "demo scenario failed");
        }
    });

    let demo = state.demo.state.lock().await;
    Ok(Json(demo.clone()))
}

/// POST /v1/demo/approve — clinician approves the current pending action.
pub async fn approve_demo(
    State(state): State<Arc<AppState>>,
) -> Result<Json<DemoState>, ApiError> {
    let tx = state.demo.approval_tx.lock().await.take();
    if let Some(tx) = tx {
        let _ = tx.send(());
    }
    let demo = state.demo.state.lock().await;
    Ok(Json(demo.clone()))
}

/// POST /v1/demo/reset — reset demo to idle state.
pub async fn reset_demo(
    State(state): State<Arc<AppState>>,
) -> Result<Json<DemoState>, ApiError> {
    let mut demo = state.demo.state.lock().await;
    *demo = DemoState {
        phase: DemoPhase::Idle,
        encounter_id: String::new(),
        patient: demo_patient(),
        triage_result: None,
        orders: vec![],
        lab_result: None,
        med_recommendation: None,
        soap_note: None,
        stats: DemoStats::default(),
    };
    // Cancel any pending approval
    let _ = state.demo.approval_tx.lock().await.take();
    Ok(Json(demo.clone()))
}

// ── Demo scenario orchestrator ───────────────────────────────────────────────

async fn run_demo_scenario(state: Arc<AppState>) -> anyhow::Result<()> {
    let encounter_id = "demo-enc-001";
    let patient_id = "demo-patient-001";
    let practitioner_id = "demo-physician-001";
    let phase_ms: u64 = 400; // visible pause between event phases

    // Ensure FHIR resources exist for the demo
    seed_demo_patient(&state).await;

    let workspace_id = ensure_workspace(&state, encounter_id, practitioner_id).await;

    // ── Phase 1: Triage ──────────────────────────────────────────────────────
    {
        let mut demo = state.demo.state.lock().await;
        demo.phase = DemoPhase::TriageRunning;
    }

    let emitter = EventEmitter::new(
        state.event_tx.clone(),
        encounter_id,
        "triage_assess",
    );

    emitter.emit(AgentEventType::AgentStarted);
    emitter.emit(AgentEventType::ContextBuilding {
        step: 1,
        detail: "Reading patient demographics and history".into(),
    });
    phase_sleep(phase_ms).await;
    emitter.emit(AgentEventType::ContextBuilding {
        step: 2,
        detail: "Evaluating chief complaint: chest pain".into(),
    });
    phase_sleep(phase_ms).await;
    emitter.emit(AgentEventType::PopulationGate { passed: true, reason: None });
    emitter.emit(AgentEventType::RoleCheck { role: "physician".into(), allowed: true });
    emitter.emit(AgentEventType::CapabilityCheck {
        capability: "triage_assess".into(),
        valid: true,
    });
    phase_sleep(phase_ms).await;
    emitter.emit(AgentEventType::PolicyEvaluation {
        decision: "Allow".into(),
        rule_name: Some("triage_assess.evaluate".into()),
    });
    phase_sleep(phase_ms).await;

    // LLM call
    emitter.emit(AgentEventType::LlmCall {
        status: StepStatus::Started,
        elapsed_ms: None,
    });

    // Simulate LLM thinking time (use real LLM if available, else mock)
    // Simulate LLM thinking time — use real LLM if available, with
    // deterministic fallback so the demo always works.
    let triage_result = {
        let prompt = cliniclaw_agents::PromptEnvelope::build(
            "You are a triage nurse AI. Return JSON only: {\"esi_level\": N, \"acuity\": \"...\", \"recommendations\": [...]}",
            format!(
                "Patient: 62yo male. Chief complaint: {}. Vitals: {}. History: {}",
                "Acute substernal chest pain radiating to left arm, diaphoretic, 8/10 pain",
                "HR 102, BP 158/94, RR 22, SpO2 96%",
                "HTN, hyperlipidemia, T2DM, former smoker, family Hx MI"
            ),
        );
        let _ = state.llm.call(&prompt).await; // best-effort real LLM call
        // Always use deterministic result for demo consistency
        TriageResult {
            esi_level: 2,
            acuity: "Emergent".into(),
            recommendations: vec![
                "12-lead ECG immediately".into(),
                "Troponin I (stat)".into(),
                "Chest X-ray PA/Lateral".into(),
                "IV access, cardiac monitoring".into(),
                "Aspirin 325mg PO if not contraindicated".into(),
            ],
            confidence: 0.94,
        }
    };

    emitter.emit(AgentEventType::LlmCall {
        status: StepStatus::Completed,
        elapsed_ms: Some(3200),
    });
    phase_sleep(phase_ms).await;
    emitter.emit(AgentEventType::Verification {
        passed: true,
        detail: Some("ESI-2 validated, recommendations substantive".into()),
    });
    emitter.emit(AgentEventType::FhirWrite {
        resource_type: "Observation".into(),
        resource_id: Some("demo-triage-obs-001".into()),
    });

    // Record audit
    let audit_id = record_audit(&state, practitioner_id, patient_id, "triage_assess.evaluate").await;
    emitter.emit(AgentEventType::AuditCreation { audit_event_id: audit_id });

    persist_turn(
        &state, &workspace_id, "triage_assess", "evaluate",
        serde_json::json!({"chief_complaint": "chest pain"}),
        serde_json::json!({"esi_level": triage_result.esi_level, "recommendations": &triage_result.recommendations}),
        cliniclaw_kernel::Confidence::new(triage_result.confidence, vec!["esi_validated".into(), "high_acuity".into()]),
        &emitter,
    ).await;

    emitter.emit(AgentEventType::AgentCompleted {
        confidence_score: triage_result.confidence,
        elapsed_ms: 3800,
    });

    {
        let mut demo = state.demo.state.lock().await;
        demo.triage_result = Some(triage_result);
        demo.stats.agents_run += 1;
        demo.stats.policy_checks += 1;
        demo.stats.fhir_writes += 1;
        demo.stats.audit_events += 1;
    }

    // ── Phase 2: Await order approval ────────────────────────────────────────
    {
        let mut demo = state.demo.state.lock().await;
        demo.phase = DemoPhase::AwaitingOrderApproval;
    }

    // Wait for clinician to approve
    let (tx, rx) = oneshot::channel();
    {
        let mut approval = state.demo.approval_tx.lock().await;
        *approval = Some(tx);
    }
    let _ = rx.await; // blocks until POST /v1/demo/approve

    {
        let mut demo = state.demo.state.lock().await;
        demo.stats.human_approvals += 1;
    }

    // ── Phase 3: Order agent ─────────────────────────────────────────────────
    {
        let mut demo = state.demo.state.lock().await;
        demo.phase = DemoPhase::OrdersRunning;
    }

    let order_emitter = EventEmitter::new(
        state.event_tx.clone(),
        encounter_id,
        "order_entry",
    );

    order_emitter.emit(AgentEventType::AgentStarted);
    order_emitter.emit(AgentEventType::ContextBuilding {
        step: 1,
        detail: "Reading triage recommendations".into(),
    });
    phase_sleep(phase_ms).await;
    order_emitter.emit(AgentEventType::PolicyEvaluation {
        decision: "Allow".into(),
        rule_name: Some("order_entry.propose_standard".into()),
    });
    order_emitter.emit(AgentEventType::CapabilityCheck {
        capability: "order_entry".into(),
        valid: true,
    });
    phase_sleep(phase_ms).await;

    order_emitter.emit(AgentEventType::LlmCall {
        status: StepStatus::Started,
        elapsed_ms: None,
    });
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    order_emitter.emit(AgentEventType::LlmCall {
        status: StepStatus::Completed,
        elapsed_ms: Some(2100),
    });
    phase_sleep(phase_ms).await;

    let orders = vec![
        OrderResult {
            order_type: "ServiceRequest".into(),
            description: "12-lead ECG".into(),
            fhir_resource_id: "demo-order-ecg-001".into(),
        },
        OrderResult {
            order_type: "ServiceRequest".into(),
            description: "Troponin I (stat)".into(),
            fhir_resource_id: "demo-order-troponin-001".into(),
        },
        OrderResult {
            order_type: "ServiceRequest".into(),
            description: "Chest X-ray PA/Lateral".into(),
            fhir_resource_id: "demo-order-cxr-001".into(),
        },
    ];

    for order in &orders {
        order_emitter.emit(AgentEventType::FhirWrite {
            resource_type: order.order_type.clone(),
            resource_id: Some(order.fhir_resource_id.clone()),
        });
        phase_sleep(200).await;
    }

    order_emitter.emit(AgentEventType::Verification {
        passed: true,
        detail: Some("3 orders validated against formulary".into()),
    });

    let audit_id = record_audit(&state, practitioner_id, patient_id, "order_entry.propose_standard").await;
    order_emitter.emit(AgentEventType::AuditCreation { audit_event_id: audit_id });

    persist_turn(
        &state, &workspace_id, "order_entry", "propose_standard",
        serde_json::json!({"recommendations": ["ECG", "Troponin", "CXR"]}),
        serde_json::json!({"orders": orders.iter().map(|o| &o.description).collect::<Vec<_>>()}),
        cliniclaw_kernel::Confidence::new(0.92, vec!["standard_workup".into()]),
        &order_emitter,
    ).await;

    order_emitter.emit(AgentEventType::AgentCompleted {
        confidence_score: 0.92,
        elapsed_ms: 2800,
    });

    {
        let mut demo = state.demo.state.lock().await;
        demo.orders = orders;
        demo.stats.agents_run += 1;
        demo.stats.policy_checks += 1;
        demo.stats.fhir_writes += 3;
        demo.stats.audit_events += 1;
    }

    // ── Phase 4: Lab result + pharmacy ───────────────────────────────────────
    {
        let mut demo = state.demo.state.lock().await;
        demo.phase = DemoPhase::LabAndPharmacyRunning;
    }

    // Lab review agent
    let lab_emitter = EventEmitter::new(
        state.event_tx.clone(),
        encounter_id,
        "lab_review",
    );

    lab_emitter.emit(AgentEventType::AgentStarted);
    lab_emitter.emit(AgentEventType::ContextBuilding {
        step: 1,
        detail: "Troponin I result received: 2.4 ng/mL (critical high)".into(),
    });
    phase_sleep(phase_ms).await;
    lab_emitter.emit(AgentEventType::PolicyEvaluation {
        decision: "Allow".into(),
        rule_name: Some("lab_review.interpret".into()),
    });
    phase_sleep(phase_ms).await;

    lab_emitter.emit(AgentEventType::LlmCall {
        status: StepStatus::Started,
        elapsed_ms: None,
    });
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    lab_emitter.emit(AgentEventType::LlmCall {
        status: StepStatus::Completed,
        elapsed_ms: Some(2300),
    });

    let lab_result = LabResult {
        test: "Troponin I".into(),
        value: "2.4".into(),
        unit: "ng/mL".into(),
        interpretation: "Critical high — consistent with acute myocardial infarction. Normal range <0.04 ng/mL.".into(),
        critical: true,
    };

    lab_emitter.emit(AgentEventType::Verification {
        passed: true,
        detail: Some("Critical value flagged, interpretation substantive".into()),
    });
    lab_emitter.emit(AgentEventType::FhirWrite {
        resource_type: "DiagnosticReport".into(),
        resource_id: Some("demo-lab-report-001".into()),
    });

    let audit_id = record_audit(&state, practitioner_id, patient_id, "lab_review.interpret").await;
    lab_emitter.emit(AgentEventType::AuditCreation { audit_event_id: audit_id });

    persist_turn(
        &state, &workspace_id, "lab_review", "interpret",
        serde_json::json!({"test": "Troponin I", "value": "2.4 ng/mL"}),
        serde_json::json!({"interpretation": &lab_result.interpretation, "critical": true}),
        cliniclaw_kernel::Confidence::new(0.97, vec!["critical_value".into(), "clear_interpretation".into()]),
        &lab_emitter,
    ).await;

    lab_emitter.emit(AgentEventType::AgentCompleted {
        confidence_score: 0.97,
        elapsed_ms: 3100,
    });

    // Pharmacy review agent (chain triggered from lab)
    phase_sleep(phase_ms).await;

    let pharm_emitter = EventEmitter::new(
        state.event_tx.clone(),
        encounter_id,
        "pharmacy_review",
    );

    lab_emitter.emit(AgentEventType::ChainTrigger {
        trigger_pattern: "critical_lab_value".into(),
        target_agent: "pharmacy_review".into(),
    });

    pharm_emitter.emit(AgentEventType::AgentStarted);
    pharm_emitter.emit(AgentEventType::ContextBuilding {
        step: 1,
        detail: "Reviewing active medications and lab results".into(),
    });
    phase_sleep(phase_ms).await;
    pharm_emitter.emit(AgentEventType::PolicyEvaluation {
        decision: "Allow".into(),
        rule_name: Some("pharmacy_review.evaluate".into()),
    });
    phase_sleep(phase_ms).await;

    pharm_emitter.emit(AgentEventType::LlmCall {
        status: StepStatus::Started,
        elapsed_ms: None,
    });
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    pharm_emitter.emit(AgentEventType::LlmCall {
        status: StepStatus::Completed,
        elapsed_ms: Some(1800),
    });

    let med_rec = MedRecommendation {
        medication: "Aspirin".into(),
        dose: "325mg".into(),
        route: "PO".into(),
        rationale: "Acute coronary syndrome protocol — antiplatelet therapy. No documented aspirin allergy. Compatible with current medications.".into(),
        confidence: 0.91,
    };

    pharm_emitter.emit(AgentEventType::CdsCheck {
        cards_count: 1,
        max_severity: Some("info".into()),
    });
    pharm_emitter.emit(AgentEventType::Verification {
        passed: true,
        detail: Some("No contraindications found".into()),
    });

    let audit_id = record_audit(&state, practitioner_id, patient_id, "pharmacy_review.evaluate").await;
    pharm_emitter.emit(AgentEventType::AuditCreation { audit_event_id: audit_id });

    persist_turn(
        &state, &workspace_id, "pharmacy_review", "evaluate",
        serde_json::json!({"lab_result": "Troponin I 2.4 ng/mL"}),
        serde_json::json!({"medication": &med_rec.medication, "dose": &med_rec.dose}),
        cliniclaw_kernel::Confidence::new(med_rec.confidence, vec!["acs_protocol".into(), "no_contraindications".into()]),
        &pharm_emitter,
    ).await;

    pharm_emitter.emit(AgentEventType::AgentCompleted {
        confidence_score: med_rec.confidence,
        elapsed_ms: 2400,
    });

    {
        let mut demo = state.demo.state.lock().await;
        demo.lab_result = Some(lab_result);
        demo.med_recommendation = Some(med_rec);
        demo.stats.agents_run += 2;
        demo.stats.policy_checks += 2;
        demo.stats.fhir_writes += 1;
        demo.stats.audit_events += 2;
    }

    // ── Phase 5: Await medication approval ───────────────────────────────────
    {
        let mut demo = state.demo.state.lock().await;
        demo.phase = DemoPhase::AwaitingMedApproval;
    }

    let (tx, rx) = oneshot::channel();
    {
        let mut approval = state.demo.approval_tx.lock().await;
        *approval = Some(tx);
    }
    let _ = rx.await;

    {
        let mut demo = state.demo.state.lock().await;
        demo.stats.human_approvals += 1;
    }

    // Write the medication order after approval
    let med_emitter = EventEmitter::new(
        state.event_tx.clone(),
        encounter_id,
        "order_entry",
    );

    med_emitter.emit(AgentEventType::AgentStarted);
    med_emitter.emit(AgentEventType::PolicyEvaluation {
        decision: "Allow".into(),
        rule_name: Some("order_entry.propose_standard".into()),
    });
    phase_sleep(phase_ms).await;
    med_emitter.emit(AgentEventType::FhirWrite {
        resource_type: "MedicationRequest".into(),
        resource_id: Some("demo-med-aspirin-001".into()),
    });

    let audit_id = record_audit(&state, practitioner_id, patient_id, "order_entry.propose_standard").await;
    med_emitter.emit(AgentEventType::AuditCreation { audit_event_id: audit_id });
    med_emitter.emit(AgentEventType::AgentCompleted {
        confidence_score: 0.95,
        elapsed_ms: 800,
    });

    {
        let mut demo = state.demo.state.lock().await;
        demo.stats.agents_run += 1;
        demo.stats.policy_checks += 1;
        demo.stats.fhir_writes += 1;
        demo.stats.audit_events += 1;
    }

    // ── Phase 6: Documentation ───────────────────────────────────────────────
    {
        let mut demo = state.demo.state.lock().await;
        demo.phase = DemoPhase::DocumentationRunning;
    }

    let doc_emitter = EventEmitter::new(
        state.event_tx.clone(),
        encounter_id,
        "ambient_doc",
    );

    doc_emitter.emit(AgentEventType::AgentStarted);
    doc_emitter.emit(AgentEventType::ContextBuilding {
        step: 1,
        detail: "Gathering encounter transcript and results".into(),
    });
    phase_sleep(phase_ms).await;
    doc_emitter.emit(AgentEventType::PolicyEvaluation {
        decision: "Allow".into(),
        rule_name: Some("ambient_doc.generate_note".into()),
    });
    doc_emitter.emit(AgentEventType::CapabilityCheck {
        capability: "ambient_doc".into(),
        valid: true,
    });
    phase_sleep(phase_ms).await;

    doc_emitter.emit(AgentEventType::LlmCall {
        status: StepStatus::Started,
        elapsed_ms: None,
    });
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    doc_emitter.emit(AgentEventType::LlmCall {
        status: StepStatus::Completed,
        elapsed_ms: Some(3400),
    });

    let soap = SoapNote {
        subjective: "62-year-old male presents with acute onset substernal chest pain radiating to left arm, started 45 minutes ago while climbing stairs. Reports diaphoresis. Pain rated 8/10. PMH significant for HTN (10 years), hyperlipidemia, T2DM (5 years), former smoker. Family history of MI (father, age 58).".into(),
        objective: "Vitals: HR 102, BP 158/94, RR 22, SpO2 96%, Temp 37.0C. Diaphoretic, anxious. Troponin I elevated at 2.4 ng/mL (critical high, normal <0.04).".into(),
        assessment: "Acute ST-elevation myocardial infarction (STEMI). Elevated troponin confirms myocardial injury. Multiple cardiac risk factors present.".into(),
        plan: "1. Cardiology consult for emergent cardiac catheterization\n2. Aspirin 325mg PO administered\n3. Heparin drip per ACS protocol\n4. Serial troponins q6h\n5. Continuous cardiac monitoring\n6. Admit to CCU".into(),
        icd10_codes: vec!["I21.0".into(), "I10".into(), "E11.9".into(), "Z87.891".into()],
        confidence: 0.96,
    };

    phase_sleep(phase_ms).await;
    doc_emitter.emit(AgentEventType::Verification {
        passed: true,
        detail: Some("SOAP structure complete, ICD-10 codes validated".into()),
    });
    doc_emitter.emit(AgentEventType::FhirWrite {
        resource_type: "DocumentReference".into(),
        resource_id: Some("demo-note-001".into()),
    });

    let audit_id = record_audit(&state, practitioner_id, patient_id, "ambient_doc.generate_note").await;
    doc_emitter.emit(AgentEventType::AuditCreation { audit_event_id: audit_id });

    persist_turn(
        &state, &workspace_id, "ambient_doc", "generate_note",
        serde_json::json!({"encounter_id": encounter_id}),
        serde_json::json!({"subjective": &soap.subjective, "assessment": &soap.assessment}),
        cliniclaw_kernel::Confidence::new(soap.confidence, vec!["complete_soap".into(), "icd10_validated".into()]),
        &doc_emitter,
    ).await;

    doc_emitter.emit(AgentEventType::AgentCompleted {
        confidence_score: soap.confidence,
        elapsed_ms: 4200,
    });

    {
        let mut demo = state.demo.state.lock().await;
        demo.soap_note = Some(soap);
        demo.stats.agents_run += 1;
        demo.stats.policy_checks += 1;
        demo.stats.fhir_writes += 1;
        demo.stats.audit_events += 1;
    }

    // ── Complete ─────────────────────────────────────────────────────────────
    {
        let mut demo = state.demo.state.lock().await;
        demo.phase = DemoPhase::Complete;
    }

    tracing::info!("demo scenario completed");
    Ok(())
}

// ── Helpers ──────────────────────────────────────────────────────────────────

async fn seed_demo_patient(state: &Arc<AppState>) {
    let patient = serde_json::json!({
        "resourceType": "Patient",
        "id": "demo-patient-001",
        "active": true,
        "name": [{"family": "Doe", "given": ["John"], "use": "official"}],
        "gender": "male",
        "birthDate": "1964-03-15"
    });

    let encounter = serde_json::json!({
        "resourceType": "Encounter",
        "id": "demo-enc-001",
        "status": "in-progress",
        "class": {"code": "EMER", "display": "Emergency"},
        "subject": {"reference": "Patient/demo-patient-001", "display": "John Doe"},
        "reasonCode": [{"coding": [{"code": "R07.9", "display": "Chest pain, unspecified"}]}],
        "period": {"start": chrono::Utc::now().to_rfc3339()}
    });

    let _ = state.fhir.create_resource("Patient", &patient).await;
    let _ = state.fhir.create_resource("Encounter", &encounter).await;
}

async fn record_audit(
    state: &Arc<AppState>,
    actor: &str,
    patient: &str,
    action: &str,
) -> String {
    let input_hash = cliniclaw_persist::sha256_hash(
        serde_json::json!({"action": action}).to_string().as_bytes()
    );
    let output_hash = cliniclaw_persist::sha256_hash(
        serde_json::json!({"result": "ok"}).to_string().as_bytes()
    );
    let mut event = cliniclaw_persist::AuditEvent::new(
        actor,
        Some(patient.to_string()),
        action,
        "allow",
        input_hash,
        output_hash,
        "", // previous_hash — append() fills this atomically
    );
    let id = event.id.to_string();
    if let Err(e) = state.audit_store.append(&mut event).await {
        tracing::warn!(error = %e, "demo: failed to record audit event");
    }
    id
}
