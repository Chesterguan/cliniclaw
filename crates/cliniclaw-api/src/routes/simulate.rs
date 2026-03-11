use std::sync::Arc;

use axum::extract::{Json, State};

use cliniclaw_kernel::{AgentEventType, EventEmitter, StepStatus};

use crate::error::ApiError;
use crate::state::AppState;

#[derive(Debug, serde::Deserialize)]
pub struct SimulateRequest {
    #[serde(default = "default_speed")]
    pub speed: String,
}

fn default_speed() -> String {
    "fast".to_string()
}

#[derive(Debug, serde::Serialize)]
pub struct SimulateResponse {
    pub status: String,
    pub pathways: usize,
    pub total_agent_executions: usize,
}

pub async fn run_simulation(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SimulateRequest>,
) -> Result<Json<SimulateResponse>, ApiError> {
    let base_delay_ms = match body.speed.as_str() {
        "fast" => 300,
        "demo" => 2000,
        _ => 1500,
    };
    // Micro-delay between event phases within an agent step (ms).
    // Makes connection beams visible in 3D visualization.
    let phase_delay_ms = match body.speed.as_str() {
        "fast" => 0,
        "demo" => 400,
        _ => 100,
    };

    // 6 patient pathways running concurrently — each creates its own workspace
    // and runs agents directly against state (no HTTP round-trip).
    tokio::spawn(pathway_mitchell(state.clone(), base_delay_ms, phase_delay_ms));
    tokio::spawn(pathway_thompson(state.clone(), base_delay_ms, phase_delay_ms));
    tokio::spawn(pathway_garcia(state.clone(), base_delay_ms, phase_delay_ms));
    tokio::spawn(pathway_chen(state.clone(), base_delay_ms, phase_delay_ms));
    tokio::spawn(pathway_johnson(state.clone(), base_delay_ms, phase_delay_ms));
    tokio::spawn(pathway_williams(state.clone(), base_delay_ms, phase_delay_ms));

    // Step counts per pathway:
    //   Mitchell:  triage + nurse + ambient_doc + order_entry + pharmacy + discharge = 6
    //   Thompson:  triage + nurse + lab_review + ambient_doc + order_entry + pharmacy = 6
    //   Garcia:    triage + nurse + ambient_doc + lab_review                          = 4
    //   Chen:      triage + nurse + ambient_doc + order × 2 + lab_review + pharmacy  = 7
    //   Johnson:   triage + nurse + ambient_doc + prior_auth                          = 4
    //   Williams:  triage + nurse + ambient_doc + order_entry + pharmacy + lab_review + discharge = 7
    let total_agent_executions = 6 + 6 + 4 + 7 + 4 + 7; // = 34

    Ok(Json(SimulateResponse {
        status: "running".to_string(),
        pathways: 6,
        total_agent_executions,
    }))
}

// ── Deterministic jitter helper ───────────────────────────────────────────────

/// Sleeps for `base_ms / 2 + jitter` milliseconds where jitter is derived
/// deterministically from the step number. Avoids a rand dependency while
/// still spreading steps across time so SSE events arrive in a realistic order.
pub(crate) async fn sim_delay(base_ms: u64, step: u64) {
    let jitter = (step.wrapping_mul(137)) % base_ms.max(1);
    tokio::time::sleep(std::time::Duration::from_millis(base_ms / 2 + jitter)).await;
}

// ── Shared workflow helpers ────────────────────────────────────────────────────

/// Ensure an open workspace exists for an encounter. If one already exists and
/// is open, returns it. Otherwise creates a new one.
pub(crate) async fn ensure_workspace(state: &Arc<AppState>, encounter_id: &str, practitioner_id: &str) -> String {
    if let Ok(Some(ws)) = state.workspace_store.find_workspace_by_encounter(encounter_id).await {
        if ws.closed_at.is_none() {
            return ws.id;
        }
    }
    match state.workspace_store.create_workspace(encounter_id, practitioner_id).await {
        Ok(ws) => ws.id,
        Err(e) => {
            tracing::warn!(encounter_id = %encounter_id, error = %e, "sim: failed to create workspace, using placeholder");
            uuid::Uuid::new_v4().to_string()
        }
    }
}

/// Persist a turn for a completed agent step. Logs a warning on failure rather
/// than propagating — simulation pathways are fire-and-forget.
pub(crate) async fn persist_turn(
    state: &Arc<AppState>,
    workspace_id: &str,
    agent_name: &str,
    action: &str,
    input_snap: serde_json::Value,
    output_snap: serde_json::Value,
    confidence: cliniclaw_kernel::Confidence,
    emitter: &EventEmitter,
) {
    let tid = uuid::Uuid::new_v4().to_string();
    let turn = cliniclaw_kernel::Turn {
        id: tid.clone(),
        workspace_id: workspace_id.to_string(),
        agent_name: agent_name.to_string(),
        action: action.to_string(),
        input_snapshot: input_snap,
        output_snapshot: output_snap,
        confidence: confidence.clone(),
        status: cliniclaw_kernel::TurnStatus::Pending,
        feedback: None,
        created_at: chrono::Utc::now(),
        resolved_at: None,
        resolved_by: None,
        triggered_by_turn_id: None,
    };
    if let Err(e) = state.workspace_store.create_turn(&turn).await {
        tracing::warn!(turn_id = %tid, error = %e, "sim: failed to persist turn");
    }
    emitter.emit_with_turn(&tid, AgentEventType::TurnCreation {
        turn_id: tid.clone(),
        confidence_score: confidence.score,
    });
}

// ── Shared standard event sequence emitters ───────────────────────────────────

/// Micro-delay between event phases so 3D beams are visible.
pub(crate) async fn phase_sleep(ms: u64) {
    if ms > 0 {
        tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
    }
}

pub(crate) async fn emit_pre_llm(emitter: &EventEmitter, capability: &str, skill_id: &str, phase_ms: u64) {
    emitter.emit(AgentEventType::ContextBuilding {
        step: 1,
        detail: "Fetching encounter and patient from FHIR".into(),
    });
    emitter.emit(AgentEventType::ContextBuilding {
        step: 2,
        detail: "Population gate verified".into(),
    });
    emitter.emit(AgentEventType::PopulationGate { passed: true, reason: None });
    phase_sleep(phase_ms).await; // pause after context building
    emitter.emit(AgentEventType::RoleCheck { role: "physician".into(), allowed: true });
    emitter.emit(AgentEventType::CapabilityCheck {
        capability: capability.to_string(),
        valid: true,
    });
    emitter.emit(AgentEventType::SkillLookup {
        skill_id: Some(skill_id.to_string()),
        matched: true,
    });
    phase_sleep(phase_ms).await; // pause after policy gates
    emitter.emit(AgentEventType::PolicyEvaluation {
        decision: "evaluating".into(),
        rule_name: None,
    });
    phase_sleep(phase_ms).await; // pause before LLM call
    emitter.emit(AgentEventType::LlmCall {
        status: StepStatus::Started,
        elapsed_ms: None,
    });
    phase_sleep(phase_ms).await; // simulate LLM "thinking" time
}

pub(crate) async fn emit_post_llm_allow(emitter: &EventEmitter, elapsed_ms: u64, rule_name: &str, parse_detail: &str, phase_ms: u64) {
    emitter.emit(AgentEventType::LlmCall {
        status: StepStatus::Completed,
        elapsed_ms: Some(elapsed_ms),
    });
    phase_sleep(phase_ms).await; // pause after LLM response
    emitter.emit(AgentEventType::PolicyEvaluation {
        decision: "allow".into(),
        rule_name: Some(rule_name.to_string()),
    });
    emitter.emit(AgentEventType::ResponseParsing {
        status: StepStatus::Completed,
        detail: Some(parse_detail.to_string()),
    });
    phase_sleep(phase_ms).await; // pause before verification
    emitter.emit(AgentEventType::Verification {
        passed: true,
        detail: Some("Output structure verified".into()),
    });
}

// ── Pathway 1: Sarah Mitchell (enc-001, patient-001, HTN) ─────────────────────
// triage → nurse → ambient_doc → order_entry → pharmacy → discharge

async fn pathway_mitchell(state: Arc<AppState>, base_ms: u64, phase_ms: u64) {
    let encounter_id = "enc-001";
    let patient_id = "patient-001";
    let practitioner_id = "prac-001";
    let vitals = "BP 142/88, HR 76, Temp 98.6, RR 16, SpO2 99%";

    let ws_id = ensure_workspace(&state, encounter_id, practitioner_id).await;

    // Step 1: Triage
    sim_delay(base_ms, 1).await;
    {
        let emitter = EventEmitter::new(state.event_tx.clone(), encounter_id, "triage_assess");
        emitter.emit(AgentEventType::AgentStarted);
        emit_pre_llm(&emitter, "triage_assess", "triage_assess.evaluate", phase_ms).await;
        let llm_start = std::time::Instant::now();
        let agent = cliniclaw_agents::TriageAssessAgent::new(state.llm.clone());
        let input = cliniclaw_agents::TriageAssessInput {
            encounter_id: encounter_id.to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: patient_id.to_string(),
            practitioner_id: practitioner_id.to_string(),
            chief_complaint: "Headaches and elevated blood pressure".to_string(),
            vitals_text: vitals.to_string(),
            capabilities: vec!["triage_assess".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("nurse".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("AMB".to_string()),
        };
        match agent.evaluate(&input, &state.policy_engine).await {
            Ok(mut output) => {
                let elapsed = llm_start.elapsed().as_millis() as u64;
                emit_post_llm_allow(&emitter, elapsed, "allow_triage_assess", "Triage assessment parsed", phase_ms).await;
                if let Err(e) = state.audit_store.append(&mut output.audit_event).await {
                    tracing::warn!(error = %e, "sim mitchell: triage audit persist failed");
                }
                emitter.emit(AgentEventType::AuditCreation { audit_event_id: output.audit_event.id.to_string() });
                let obs_json = serde_json::to_value(&output.observation).unwrap_or_default();
                emitter.emit(AgentEventType::FhirWrite { resource_type: "Observation".into(), resource_id: None });
                persist_turn(&state, &ws_id, "triage_assess", "evaluate",
                    serde_json::json!({"encounter_id": encounter_id, "patient_id": patient_id}),
                    obs_json, output.confidence.clone(), &emitter).await;
                emitter.emit(AgentEventType::AgentCompleted { confidence_score: output.confidence.score, elapsed_ms: elapsed });
            }
            Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
        }
    }

    // Step 2: Nursing assessment
    sim_delay(base_ms, 2).await;
    {
        let emitter = EventEmitter::new(state.event_tx.clone(), encounter_id, "nurse_assess");
        emitter.emit(AgentEventType::AgentStarted);
        emit_pre_llm(&emitter, "nurse_assess", "nurse_assess.evaluate", phase_ms).await;
        let llm_start = std::time::Instant::now();
        let agent = cliniclaw_agents::NurseAssessAgent::new(state.llm.clone());
        let input = cliniclaw_agents::NurseAssessInput {
            encounter_id: encounter_id.to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: patient_id.to_string(),
            practitioner_id: practitioner_id.to_string(),
            assessment_type: "admission".to_string(),
            vitals_text: vitals.to_string(),
            capabilities: vec!["nurse_assess".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("nurse".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("AMB".to_string()),
        };
        match agent.evaluate(&input, &state.policy_engine).await {
            Ok(mut output) => {
                let elapsed = llm_start.elapsed().as_millis() as u64;
                emit_post_llm_allow(&emitter, elapsed, "allow_nurse_assess", "Nursing assessment parsed", phase_ms).await;
                if let Err(e) = state.audit_store.append(&mut output.audit_event).await {
                    tracing::warn!(error = %e, "sim mitchell: nurse audit persist failed");
                }
                emitter.emit(AgentEventType::AuditCreation { audit_event_id: output.audit_event.id.to_string() });
                let obs_json = serde_json::to_value(&output.observation).unwrap_or_default();
                emitter.emit(AgentEventType::FhirWrite { resource_type: "Observation".into(), resource_id: None });
                persist_turn(&state, &ws_id, "nurse_assess", "evaluate",
                    serde_json::json!({"encounter_id": encounter_id, "assessment_type": "admission"}),
                    obs_json, output.confidence.clone(), &emitter).await;
                emitter.emit(AgentEventType::AgentCompleted { confidence_score: output.confidence.score, elapsed_ms: elapsed });
            }
            Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
        }
    }

    // Step 3: Ambient documentation
    sim_delay(base_ms, 3).await;
    {
        let emitter = EventEmitter::new(state.event_tx.clone(), encounter_id, "ambient_doc");
        emitter.emit(AgentEventType::AgentStarted);
        emit_pre_llm(&emitter, "note_generation", "ambient_doc.generate_note", phase_ms).await;
        let llm_start = std::time::Instant::now();
        let input = cliniclaw_agents::AmbientDocInput {
            encounter_id: encounter_id.to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: patient_id.to_string(),
            practitioner_id: practitioner_id.to_string(),
            transcript: "Patient presents with headaches for the past two weeks and reports elevated blood pressure readings at home. Current lisinopril dose does not appear to be adequately controlling BP. Will consider increasing lisinopril dosage. Patient denies chest pain, shortness of breath, or visual changes.".to_string(),
            chief_complaint: Some("Headaches and elevated blood pressure".to_string()),
            active_medications: vec!["Lisinopril 10mg PO daily".to_string()],
            capabilities: vec!["note_generation".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("physician".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("AMB".to_string()),
        };
        match state.ambient_agent.generate_note(&input, &state.policy_engine).await {
            Ok(mut output) => {
                let elapsed = llm_start.elapsed().as_millis() as u64;
                emit_post_llm_allow(&emitter, elapsed, "allow_note_generation", "SOAP note parsed", phase_ms).await;
                if let Err(e) = state.audit_store.append(&mut output.audit_event).await {
                    tracing::warn!(error = %e, "sim mitchell: ambient_doc audit persist failed");
                }
                emitter.emit(AgentEventType::AuditCreation { audit_event_id: output.audit_event.id.to_string() });
                let report_json = serde_json::to_value(&output.report).unwrap_or_default();
                emitter.emit(AgentEventType::FhirWrite { resource_type: "DiagnosticReport".into(), resource_id: None });
                persist_turn(&state, &ws_id, "ambient_doc", "generate_note",
                    serde_json::json!({"encounter_id": encounter_id, "patient_id": patient_id}),
                    report_json, output.confidence.clone(), &emitter).await;
                emitter.emit(AgentEventType::AgentCompleted { confidence_score: output.confidence.score, elapsed_ms: elapsed });
            }
            Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
        }
    }

    // Step 4: Order entry
    sim_delay(base_ms, 4).await;
    {
        let emitter = EventEmitter::new(state.event_tx.clone(), encounter_id, "order_entry");
        emitter.emit(AgentEventType::AgentStarted);
        emit_pre_llm(&emitter, "order_entry", "order_entry.propose_order", phase_ms).await;
        let llm_start = std::time::Instant::now();
        let agent = cliniclaw_agents::OrderEntryAgent::new(state.llm.clone());
        let input = cliniclaw_agents::OrderEntryInput {
            encounter_id: encounter_id.to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: patient_id.to_string(),
            practitioner_id: practitioner_id.to_string(),
            order_text: "increase lisinopril to 20mg PO daily".to_string(),
            active_medications: vec!["Lisinopril 10mg PO daily".to_string()],
            capabilities: vec!["order_entry".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("physician".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("AMB".to_string()),
        };
        match agent.propose_order(&input, &state.policy_engine).await {
            Ok(mut output) => {
                let elapsed = llm_start.elapsed().as_millis() as u64;
                emit_post_llm_allow(&emitter, elapsed, "allow_order_entry", "MedicationRequest parsed", phase_ms).await;
                use cliniclaw_agents::CdsIndicator;
                let max_sev = output.cds_cards.iter().map(|c| match c.indicator {
                    CdsIndicator::HardStop => 4, CdsIndicator::Critical => 3,
                    CdsIndicator::Warning => 2, CdsIndicator::Info => 1,
                }).max().map(|s| match s { 4 => "hard_stop", 3 => "critical", 2 => "warning", _ => "info" }).map(String::from);
                emitter.emit(AgentEventType::CdsCheck { cards_count: output.cds_cards.len(), max_severity: max_sev });
                if let Err(e) = state.audit_store.append(&mut output.audit_event).await {
                    tracing::warn!(error = %e, "sim mitchell: order audit persist failed");
                }
                emitter.emit(AgentEventType::AuditCreation { audit_event_id: output.audit_event.id.to_string() });
                let med_json = serde_json::to_value(&output.medication_request).unwrap_or_default();
                emitter.emit(AgentEventType::FhirWrite { resource_type: "MedicationRequest".into(), resource_id: None });
                persist_turn(&state, &ws_id, "order_entry", "propose_order",
                    serde_json::json!({"encounter_id": encounter_id, "order_text": "increase lisinopril to 20mg PO daily"}),
                    med_json, output.confidence.clone(), &emitter).await;
                emitter.emit(AgentEventType::AgentCompleted { confidence_score: output.confidence.score, elapsed_ms: elapsed });
            }
            Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
        }
    }

    // Step 5: Pharmacy review
    sim_delay(base_ms, 5).await;
    {
        let emitter = EventEmitter::new(state.event_tx.clone(), encounter_id, "pharmacy_review");
        emitter.emit(AgentEventType::AgentStarted);
        emit_pre_llm(&emitter, "pharmacy_review", "pharmacy_review.evaluate", phase_ms).await;
        let llm_start = std::time::Instant::now();
        let agent = cliniclaw_agents::PharmacyReviewAgent::new(state.llm.clone());
        let input = cliniclaw_agents::PharmacyReviewInput {
            encounter_id: encounter_id.to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: patient_id.to_string(),
            practitioner_id: practitioner_id.to_string(),
            pending_orders: vec!["Lisinopril 20mg PO daily".to_string()],
            active_medications: vec!["Lisinopril 10mg PO daily".to_string()],
            allergies: vec!["Penicillin".to_string()],
            capabilities: vec!["pharmacy_review".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("pharmacist".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("AMB".to_string()),
        };
        match agent.evaluate(&input, &state.policy_engine).await {
            Ok(mut output) => {
                let elapsed = llm_start.elapsed().as_millis() as u64;
                emit_post_llm_allow(&emitter, elapsed, "allow_pharmacy_review", "Pharmacy review parsed", phase_ms).await;
                use cliniclaw_agents::CdsIndicator;
                let max_sev = output.cds_cards.iter().map(|c| match c.indicator {
                    CdsIndicator::HardStop => 4, CdsIndicator::Critical => 3,
                    CdsIndicator::Warning => 2, CdsIndicator::Info => 1,
                }).max().map(|s| match s { 4 => "hard_stop", 3 => "critical", 2 => "warning", _ => "info" }).map(String::from);
                emitter.emit(AgentEventType::CdsCheck { cards_count: output.cds_cards.len(), max_severity: max_sev });
                if let Err(e) = state.audit_store.append(&mut output.audit_event).await {
                    tracing::warn!(error = %e, "sim mitchell: pharmacy audit persist failed");
                }
                emitter.emit(AgentEventType::AuditCreation { audit_event_id: output.audit_event.id.to_string() });
                persist_turn(&state, &ws_id, "pharmacy_review", "evaluate",
                    serde_json::json!({"encounter_id": encounter_id, "pending_order_count": 1}),
                    serde_json::json!({"review_status": output.review_status}),
                    output.confidence.clone(), &emitter).await;
                emitter.emit(AgentEventType::AgentCompleted { confidence_score: output.confidence.score, elapsed_ms: elapsed });
            }
            Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
        }
    }

    // Step 6: Discharge plan
    sim_delay(base_ms, 6).await;
    {
        let emitter = EventEmitter::new(state.event_tx.clone(), encounter_id, "discharge_plan");
        emitter.emit(AgentEventType::AgentStarted);
        emit_pre_llm(&emitter, "discharge_plan", "discharge_plan.generate", phase_ms).await;
        let llm_start = std::time::Instant::now();
        let agent = cliniclaw_agents::DischargePlanAgent::new(state.llm.clone());
        let input = cliniclaw_agents::DischargePlanInput {
            encounter_id: encounter_id.to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: patient_id.to_string(),
            practitioner_id: practitioner_id.to_string(),
            active_conditions: vec!["Essential hypertension".to_string()],
            current_medications: vec!["Lisinopril 20mg PO daily".to_string()],
            assessment_summary: "Patient with essential hypertension, blood pressure now better controlled following lisinopril dose increase to 20mg daily. Headaches improving. Stable for discharge.".to_string(),
            capabilities: vec!["discharge_plan".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("physician".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("AMB".to_string()),
        };
        match agent.generate(&input, &state.policy_engine).await {
            Ok(mut output) => {
                let elapsed = llm_start.elapsed().as_millis() as u64;
                emit_post_llm_allow(&emitter, elapsed, "allow_discharge_plan", "Discharge plan parsed", phase_ms).await;
                if let Err(e) = state.audit_store.append(&mut output.audit_event).await {
                    tracing::warn!(error = %e, "sim mitchell: discharge audit persist failed");
                }
                emitter.emit(AgentEventType::AuditCreation { audit_event_id: output.audit_event.id.to_string() });
                let report_json = serde_json::to_value(&output.report).unwrap_or_default();
                emitter.emit(AgentEventType::FhirWrite { resource_type: "DiagnosticReport".into(), resource_id: None });
                persist_turn(&state, &ws_id, "discharge_plan", "generate",
                    serde_json::json!({"encounter_id": encounter_id, "condition_count": 1}),
                    report_json, output.confidence.clone(), &emitter).await;
                emitter.emit(AgentEventType::AgentCompleted { confidence_score: output.confidence.score, elapsed_ms: elapsed });
            }
            Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
        }
    }
}

// ── Pathway 2: James Thompson (enc-002, patient-002, T2DM) ───────────────────
// triage → nurse → lab_review → ambient_doc → order_entry → pharmacy

async fn pathway_thompson(state: Arc<AppState>, base_ms: u64, phase_ms: u64) {
    let encounter_id = "enc-002";
    let patient_id = "patient-002";
    let practitioner_id = "prac-002";
    let vitals = "BP 128/82, HR 80, Temp 98.4, RR 14, SpO2 99%";

    let ws_id = ensure_workspace(&state, encounter_id, practitioner_id).await;

    // Step 1: Triage
    sim_delay(base_ms, 7).await;
    {
        let emitter = EventEmitter::new(state.event_tx.clone(), encounter_id, "triage_assess");
        emitter.emit(AgentEventType::AgentStarted);
        emit_pre_llm(&emitter, "triage_assess", "triage_assess.evaluate", phase_ms).await;
        let llm_start = std::time::Instant::now();
        let agent = cliniclaw_agents::TriageAssessAgent::new(state.llm.clone());
        let input = cliniclaw_agents::TriageAssessInput {
            encounter_id: encounter_id.to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: patient_id.to_string(),
            practitioner_id: practitioner_id.to_string(),
            chief_complaint: "Diabetes follow-up, reports increased thirst".to_string(),
            vitals_text: vitals.to_string(),
            capabilities: vec!["triage_assess".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("nurse".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("AMB".to_string()),
        };
        match agent.evaluate(&input, &state.policy_engine).await {
            Ok(mut output) => {
                let elapsed = llm_start.elapsed().as_millis() as u64;
                emit_post_llm_allow(&emitter, elapsed, "allow_triage_assess", "Triage assessment parsed", phase_ms).await;
                let _ = state.audit_store.append(&mut output.audit_event).await;
                emitter.emit(AgentEventType::AuditCreation { audit_event_id: output.audit_event.id.to_string() });
                emitter.emit(AgentEventType::FhirWrite { resource_type: "Observation".into(), resource_id: None });
                persist_turn(&state, &ws_id, "triage_assess", "evaluate",
                    serde_json::json!({"encounter_id": encounter_id}),
                    serde_json::to_value(&output.observation).unwrap_or_default(),
                    output.confidence.clone(), &emitter).await;
                emitter.emit(AgentEventType::AgentCompleted { confidence_score: output.confidence.score, elapsed_ms: elapsed });
            }
            Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
        }
    }

    // Step 2: Nursing assessment
    sim_delay(base_ms, 8).await;
    {
        let emitter = EventEmitter::new(state.event_tx.clone(), encounter_id, "nurse_assess");
        emitter.emit(AgentEventType::AgentStarted);
        emit_pre_llm(&emitter, "nurse_assess", "nurse_assess.evaluate", phase_ms).await;
        let llm_start = std::time::Instant::now();
        let agent = cliniclaw_agents::NurseAssessAgent::new(state.llm.clone());
        let input = cliniclaw_agents::NurseAssessInput {
            encounter_id: encounter_id.to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: patient_id.to_string(),
            practitioner_id: practitioner_id.to_string(),
            assessment_type: "admission".to_string(),
            vitals_text: vitals.to_string(),
            capabilities: vec!["nurse_assess".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("nurse".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("AMB".to_string()),
        };
        match agent.evaluate(&input, &state.policy_engine).await {
            Ok(mut output) => {
                let elapsed = llm_start.elapsed().as_millis() as u64;
                emit_post_llm_allow(&emitter, elapsed, "allow_nurse_assess", "Nursing assessment parsed", phase_ms).await;
                let _ = state.audit_store.append(&mut output.audit_event).await;
                emitter.emit(AgentEventType::AuditCreation { audit_event_id: output.audit_event.id.to_string() });
                emitter.emit(AgentEventType::FhirWrite { resource_type: "Observation".into(), resource_id: None });
                persist_turn(&state, &ws_id, "nurse_assess", "evaluate",
                    serde_json::json!({"encounter_id": encounter_id}),
                    serde_json::to_value(&output.observation).unwrap_or_default(),
                    output.confidence.clone(), &emitter).await;
                emitter.emit(AgentEventType::AgentCompleted { confidence_score: output.confidence.score, elapsed_ms: elapsed });
            }
            Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
        }
    }

    // Step 3: Lab review
    sim_delay(base_ms, 9).await;
    {
        let emitter = EventEmitter::new(state.event_tx.clone(), encounter_id, "lab_review");
        emitter.emit(AgentEventType::AgentStarted);
        emit_pre_llm(&emitter, "lab_review", "lab_review.interpret", phase_ms).await;
        let llm_start = std::time::Instant::now();
        let agent = cliniclaw_agents::LabReviewAgent::new(state.llm.clone());
        let input = cliniclaw_agents::LabReviewInput {
            encounter_id: encounter_id.to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: patient_id.to_string(),
            practitioner_id: practitioner_id.to_string(),
            lab_results_text: "HbA1c 8.2%, fasting glucose 186 mg/dL, CMP normal".to_string(),
            active_conditions: vec!["E11.9".to_string()],
            capabilities: vec!["lab_review".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("physician".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("AMB".to_string()),
        };
        match agent.interpret(&input, &state.policy_engine).await {
            Ok(mut output) => {
                let elapsed = llm_start.elapsed().as_millis() as u64;
                emit_post_llm_allow(&emitter, elapsed, "allow_lab_review", "Lab interpretation parsed", phase_ms).await;
                let _ = state.audit_store.append(&mut output.audit_event).await;
                emitter.emit(AgentEventType::AuditCreation { audit_event_id: output.audit_event.id.to_string() });
                emitter.emit(AgentEventType::FhirWrite { resource_type: "DiagnosticReport".into(), resource_id: None });
                persist_turn(&state, &ws_id, "lab_review", "interpret",
                    serde_json::json!({"encounter_id": encounter_id}),
                    serde_json::to_value(&output.report).unwrap_or_default(),
                    output.confidence.clone(), &emitter).await;
                emitter.emit(AgentEventType::AgentCompleted { confidence_score: output.confidence.score, elapsed_ms: elapsed });
            }
            Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
        }
    }

    // Step 4: Ambient documentation
    sim_delay(base_ms, 10).await;
    {
        let emitter = EventEmitter::new(state.event_tx.clone(), encounter_id, "ambient_doc");
        emitter.emit(AgentEventType::AgentStarted);
        emit_pre_llm(&emitter, "note_generation", "ambient_doc.generate_note", phase_ms).await;
        let llm_start = std::time::Instant::now();
        let input = cliniclaw_agents::AmbientDocInput {
            encounter_id: encounter_id.to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: patient_id.to_string(),
            practitioner_id: practitioner_id.to_string(),
            transcript: "Patient with type 2 diabetes presenting for follow-up. HbA1c of 8.2% indicates suboptimal glycemic control. Reports polydipsia. Currently on glipizide. Will initiate metformin 500mg BID to improve glycemic management. Counseled on diet and exercise.".to_string(),
            chief_complaint: Some("Diabetes follow-up, increased thirst".to_string()),
            active_medications: vec!["Glipizide 5mg daily".to_string()],
            capabilities: vec!["note_generation".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("physician".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("AMB".to_string()),
        };
        match state.ambient_agent.generate_note(&input, &state.policy_engine).await {
            Ok(mut output) => {
                let elapsed = llm_start.elapsed().as_millis() as u64;
                emit_post_llm_allow(&emitter, elapsed, "allow_note_generation", "SOAP note parsed", phase_ms).await;
                let _ = state.audit_store.append(&mut output.audit_event).await;
                emitter.emit(AgentEventType::AuditCreation { audit_event_id: output.audit_event.id.to_string() });
                emitter.emit(AgentEventType::FhirWrite { resource_type: "DiagnosticReport".into(), resource_id: None });
                persist_turn(&state, &ws_id, "ambient_doc", "generate_note",
                    serde_json::json!({"encounter_id": encounter_id}),
                    serde_json::to_value(&output.report).unwrap_or_default(),
                    output.confidence.clone(), &emitter).await;
                emitter.emit(AgentEventType::AgentCompleted { confidence_score: output.confidence.score, elapsed_ms: elapsed });
            }
            Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
        }
    }

    // Step 5: Order entry
    sim_delay(base_ms, 11).await;
    {
        let emitter = EventEmitter::new(state.event_tx.clone(), encounter_id, "order_entry");
        emitter.emit(AgentEventType::AgentStarted);
        emit_pre_llm(&emitter, "order_entry", "order_entry.propose_order", phase_ms).await;
        let llm_start = std::time::Instant::now();
        let agent = cliniclaw_agents::OrderEntryAgent::new(state.llm.clone());
        let input = cliniclaw_agents::OrderEntryInput {
            encounter_id: encounter_id.to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: patient_id.to_string(),
            practitioner_id: practitioner_id.to_string(),
            order_text: "start metformin 500mg BID".to_string(),
            active_medications: vec!["Glipizide 5mg daily".to_string()],
            capabilities: vec!["order_entry".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("physician".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("AMB".to_string()),
        };
        match agent.propose_order(&input, &state.policy_engine).await {
            Ok(mut output) => {
                let elapsed = llm_start.elapsed().as_millis() as u64;
                emit_post_llm_allow(&emitter, elapsed, "allow_order_entry", "MedicationRequest parsed", phase_ms).await;
                use cliniclaw_agents::CdsIndicator;
                let max_sev = output.cds_cards.iter().map(|c| match c.indicator {
                    CdsIndicator::HardStop => 4, CdsIndicator::Critical => 3,
                    CdsIndicator::Warning => 2, CdsIndicator::Info => 1,
                }).max().map(|s| match s { 4 => "hard_stop", 3 => "critical", 2 => "warning", _ => "info" }).map(String::from);
                emitter.emit(AgentEventType::CdsCheck { cards_count: output.cds_cards.len(), max_severity: max_sev });
                let _ = state.audit_store.append(&mut output.audit_event).await;
                emitter.emit(AgentEventType::AuditCreation { audit_event_id: output.audit_event.id.to_string() });
                emitter.emit(AgentEventType::FhirWrite { resource_type: "MedicationRequest".into(), resource_id: None });
                persist_turn(&state, &ws_id, "order_entry", "propose_order",
                    serde_json::json!({"encounter_id": encounter_id, "order_text": "start metformin 500mg BID"}),
                    serde_json::to_value(&output.medication_request).unwrap_or_default(),
                    output.confidence.clone(), &emitter).await;
                emitter.emit(AgentEventType::AgentCompleted { confidence_score: output.confidence.score, elapsed_ms: elapsed });
            }
            Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
        }
    }

    // Step 6: Pharmacy review
    sim_delay(base_ms, 12).await;
    {
        let emitter = EventEmitter::new(state.event_tx.clone(), encounter_id, "pharmacy_review");
        emitter.emit(AgentEventType::AgentStarted);
        emit_pre_llm(&emitter, "pharmacy_review", "pharmacy_review.evaluate", phase_ms).await;
        let llm_start = std::time::Instant::now();
        let agent = cliniclaw_agents::PharmacyReviewAgent::new(state.llm.clone());
        let input = cliniclaw_agents::PharmacyReviewInput {
            encounter_id: encounter_id.to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: patient_id.to_string(),
            practitioner_id: practitioner_id.to_string(),
            pending_orders: vec!["Metformin 500mg BID".to_string()],
            active_medications: vec!["Glipizide 5mg daily".to_string()],
            allergies: vec![],
            capabilities: vec!["pharmacy_review".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("pharmacist".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("AMB".to_string()),
        };
        match agent.evaluate(&input, &state.policy_engine).await {
            Ok(mut output) => {
                let elapsed = llm_start.elapsed().as_millis() as u64;
                emit_post_llm_allow(&emitter, elapsed, "allow_pharmacy_review", "Pharmacy review parsed", phase_ms).await;
                use cliniclaw_agents::CdsIndicator;
                let max_sev = output.cds_cards.iter().map(|c| match c.indicator {
                    CdsIndicator::HardStop => 4, CdsIndicator::Critical => 3,
                    CdsIndicator::Warning => 2, CdsIndicator::Info => 1,
                }).max().map(|s| match s { 4 => "hard_stop", 3 => "critical", 2 => "warning", _ => "info" }).map(String::from);
                emitter.emit(AgentEventType::CdsCheck { cards_count: output.cds_cards.len(), max_severity: max_sev });
                let _ = state.audit_store.append(&mut output.audit_event).await;
                emitter.emit(AgentEventType::AuditCreation { audit_event_id: output.audit_event.id.to_string() });
                persist_turn(&state, &ws_id, "pharmacy_review", "evaluate",
                    serde_json::json!({"encounter_id": encounter_id}),
                    serde_json::json!({"review_status": output.review_status}),
                    output.confidence.clone(), &emitter).await;
                emitter.emit(AgentEventType::AgentCompleted { confidence_score: output.confidence.score, elapsed_ms: elapsed });
            }
            Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
        }
    }
}

// ── Pathway 3: Maria Garcia (enc-003, patient-003, prenatal) ─────────────────
// triage → nurse → ambient_doc → lab_review

async fn pathway_garcia(state: Arc<AppState>, base_ms: u64, phase_ms: u64) {
    let encounter_id = "enc-003";
    let patient_id = "patient-003";
    let practitioner_id = "prac-003";
    let vitals = "BP 118/72, HR 82, Temp 98.6, RR 16, SpO2 100%";

    let ws_id = ensure_workspace(&state, encounter_id, practitioner_id).await;

    // Step 1: Triage
    sim_delay(base_ms, 13).await;
    {
        let emitter = EventEmitter::new(state.event_tx.clone(), encounter_id, "triage_assess");
        emitter.emit(AgentEventType::AgentStarted);
        emit_pre_llm(&emitter, "triage_assess", "triage_assess.evaluate", phase_ms).await;
        let llm_start = std::time::Instant::now();
        let agent = cliniclaw_agents::TriageAssessAgent::new(state.llm.clone());
        let input = cliniclaw_agents::TriageAssessInput {
            encounter_id: encounter_id.to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: patient_id.to_string(),
            practitioner_id: practitioner_id.to_string(),
            chief_complaint: "Routine prenatal visit, 28 weeks".to_string(),
            vitals_text: vitals.to_string(),
            capabilities: vec!["triage_assess".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("nurse".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("AMB".to_string()),
        };
        match agent.evaluate(&input, &state.policy_engine).await {
            Ok(mut output) => {
                let elapsed = llm_start.elapsed().as_millis() as u64;
                emit_post_llm_allow(&emitter, elapsed, "allow_triage_assess", "Triage assessment parsed", phase_ms).await;
                let _ = state.audit_store.append(&mut output.audit_event).await;
                emitter.emit(AgentEventType::AuditCreation { audit_event_id: output.audit_event.id.to_string() });
                emitter.emit(AgentEventType::FhirWrite { resource_type: "Observation".into(), resource_id: None });
                persist_turn(&state, &ws_id, "triage_assess", "evaluate",
                    serde_json::json!({"encounter_id": encounter_id}),
                    serde_json::to_value(&output.observation).unwrap_or_default(),
                    output.confidence.clone(), &emitter).await;
                emitter.emit(AgentEventType::AgentCompleted { confidence_score: output.confidence.score, elapsed_ms: elapsed });
            }
            Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
        }
    }

    // Step 2: Nursing assessment
    sim_delay(base_ms, 14).await;
    {
        let emitter = EventEmitter::new(state.event_tx.clone(), encounter_id, "nurse_assess");
        emitter.emit(AgentEventType::AgentStarted);
        emit_pre_llm(&emitter, "nurse_assess", "nurse_assess.evaluate", phase_ms).await;
        let llm_start = std::time::Instant::now();
        let agent = cliniclaw_agents::NurseAssessAgent::new(state.llm.clone());
        let input = cliniclaw_agents::NurseAssessInput {
            encounter_id: encounter_id.to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: patient_id.to_string(),
            practitioner_id: practitioner_id.to_string(),
            assessment_type: "admission".to_string(),
            vitals_text: vitals.to_string(),
            capabilities: vec!["nurse_assess".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("nurse".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("AMB".to_string()),
        };
        match agent.evaluate(&input, &state.policy_engine).await {
            Ok(mut output) => {
                let elapsed = llm_start.elapsed().as_millis() as u64;
                emit_post_llm_allow(&emitter, elapsed, "allow_nurse_assess", "Nursing assessment parsed", phase_ms).await;
                let _ = state.audit_store.append(&mut output.audit_event).await;
                emitter.emit(AgentEventType::AuditCreation { audit_event_id: output.audit_event.id.to_string() });
                emitter.emit(AgentEventType::FhirWrite { resource_type: "Observation".into(), resource_id: None });
                persist_turn(&state, &ws_id, "nurse_assess", "evaluate",
                    serde_json::json!({"encounter_id": encounter_id}),
                    serde_json::to_value(&output.observation).unwrap_or_default(),
                    output.confidence.clone(), &emitter).await;
                emitter.emit(AgentEventType::AgentCompleted { confidence_score: output.confidence.score, elapsed_ms: elapsed });
            }
            Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
        }
    }

    // Step 3: Ambient documentation
    sim_delay(base_ms, 15).await;
    {
        let emitter = EventEmitter::new(state.event_tx.clone(), encounter_id, "ambient_doc");
        emitter.emit(AgentEventType::AgentStarted);
        emit_pre_llm(&emitter, "note_generation", "ambient_doc.generate_note", phase_ms).await;
        let llm_start = std::time::Instant::now();
        let input = cliniclaw_agents::AmbientDocInput {
            encounter_id: encounter_id.to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: patient_id.to_string(),
            practitioner_id: practitioner_id.to_string(),
            transcript: "28-week prenatal visit. Patient reports no complaints. Fetal movement present. Fundal height appropriate for gestational age. GBS culture pending. Patient is Rh positive, no Rhogam needed. Reviewed birth plan and warning signs. Follow-up in 2 weeks.".to_string(),
            chief_complaint: Some("Routine prenatal visit, 28 weeks gestation".to_string()),
            active_medications: vec![],
            capabilities: vec!["note_generation".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("physician".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("AMB".to_string()),
        };
        match state.ambient_agent.generate_note(&input, &state.policy_engine).await {
            Ok(mut output) => {
                let elapsed = llm_start.elapsed().as_millis() as u64;
                emit_post_llm_allow(&emitter, elapsed, "allow_note_generation", "SOAP note parsed", phase_ms).await;
                let _ = state.audit_store.append(&mut output.audit_event).await;
                emitter.emit(AgentEventType::AuditCreation { audit_event_id: output.audit_event.id.to_string() });
                emitter.emit(AgentEventType::FhirWrite { resource_type: "DiagnosticReport".into(), resource_id: None });
                persist_turn(&state, &ws_id, "ambient_doc", "generate_note",
                    serde_json::json!({"encounter_id": encounter_id}),
                    serde_json::to_value(&output.report).unwrap_or_default(),
                    output.confidence.clone(), &emitter).await;
                emitter.emit(AgentEventType::AgentCompleted { confidence_score: output.confidence.score, elapsed_ms: elapsed });
            }
            Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
        }
    }

    // Step 4: Lab review
    sim_delay(base_ms, 16).await;
    {
        let emitter = EventEmitter::new(state.event_tx.clone(), encounter_id, "lab_review");
        emitter.emit(AgentEventType::AgentStarted);
        emit_pre_llm(&emitter, "lab_review", "lab_review.interpret", phase_ms).await;
        let llm_start = std::time::Instant::now();
        let agent = cliniclaw_agents::LabReviewAgent::new(state.llm.clone());
        let input = cliniclaw_agents::LabReviewInput {
            encounter_id: encounter_id.to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: patient_id.to_string(),
            practitioner_id: practitioner_id.to_string(),
            lab_results_text: "CBC normal, blood type O+, Rh positive, GBS pending".to_string(),
            active_conditions: vec!["Z33.1".to_string()],
            capabilities: vec!["lab_review".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("physician".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("AMB".to_string()),
        };
        match agent.interpret(&input, &state.policy_engine).await {
            Ok(mut output) => {
                let elapsed = llm_start.elapsed().as_millis() as u64;
                emit_post_llm_allow(&emitter, elapsed, "allow_lab_review", "Lab interpretation parsed", phase_ms).await;
                let _ = state.audit_store.append(&mut output.audit_event).await;
                emitter.emit(AgentEventType::AuditCreation { audit_event_id: output.audit_event.id.to_string() });
                emitter.emit(AgentEventType::FhirWrite { resource_type: "DiagnosticReport".into(), resource_id: None });
                persist_turn(&state, &ws_id, "lab_review", "interpret",
                    serde_json::json!({"encounter_id": encounter_id}),
                    serde_json::to_value(&output.report).unwrap_or_default(),
                    output.confidence.clone(), &emitter).await;
                emitter.emit(AgentEventType::AgentCompleted { confidence_score: output.confidence.score, elapsed_ms: elapsed });
            }
            Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
        }
    }
}

// ── Pathway 4: Robert Chen (enc-004, patient-004, COPD inpatient) ────────────
// triage → nurse → ambient_doc → order × 2 → lab_review → pharmacy

async fn pathway_chen(state: Arc<AppState>, base_ms: u64, phase_ms: u64) {
    let encounter_id = "enc-004";
    let patient_id = "patient-004";
    let practitioner_id = "prac-004";
    let vitals = "BP 148/92, HR 102, Temp 100.2, RR 24, SpO2 88% on RA";

    let ws_id = ensure_workspace(&state, encounter_id, practitioner_id).await;

    // Step 1: Triage
    sim_delay(base_ms, 17).await;
    {
        let emitter = EventEmitter::new(state.event_tx.clone(), encounter_id, "triage_assess");
        emitter.emit(AgentEventType::AgentStarted);
        emit_pre_llm(&emitter, "triage_assess", "triage_assess.evaluate", phase_ms).await;
        let llm_start = std::time::Instant::now();
        let agent = cliniclaw_agents::TriageAssessAgent::new(state.llm.clone());
        let input = cliniclaw_agents::TriageAssessInput {
            encounter_id: encounter_id.to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: patient_id.to_string(),
            practitioner_id: practitioner_id.to_string(),
            chief_complaint: "COPD exacerbation, increasing dyspnea".to_string(),
            vitals_text: vitals.to_string(),
            capabilities: vec!["triage_assess".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("nurse".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("IMP".to_string()),
        };
        match agent.evaluate(&input, &state.policy_engine).await {
            Ok(mut output) => {
                let elapsed = llm_start.elapsed().as_millis() as u64;
                emit_post_llm_allow(&emitter, elapsed, "allow_triage_assess", "Triage assessment parsed", phase_ms).await;
                let _ = state.audit_store.append(&mut output.audit_event).await;
                emitter.emit(AgentEventType::AuditCreation { audit_event_id: output.audit_event.id.to_string() });
                emitter.emit(AgentEventType::FhirWrite { resource_type: "Observation".into(), resource_id: None });
                persist_turn(&state, &ws_id, "triage_assess", "evaluate",
                    serde_json::json!({"encounter_id": encounter_id}),
                    serde_json::to_value(&output.observation).unwrap_or_default(),
                    output.confidence.clone(), &emitter).await;
                emitter.emit(AgentEventType::AgentCompleted { confidence_score: output.confidence.score, elapsed_ms: elapsed });
            }
            Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
        }
    }

    // Step 2: Nursing assessment
    sim_delay(base_ms, 18).await;
    {
        let emitter = EventEmitter::new(state.event_tx.clone(), encounter_id, "nurse_assess");
        emitter.emit(AgentEventType::AgentStarted);
        emit_pre_llm(&emitter, "nurse_assess", "nurse_assess.evaluate", phase_ms).await;
        let llm_start = std::time::Instant::now();
        let agent = cliniclaw_agents::NurseAssessAgent::new(state.llm.clone());
        let input = cliniclaw_agents::NurseAssessInput {
            encounter_id: encounter_id.to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: patient_id.to_string(),
            practitioner_id: practitioner_id.to_string(),
            assessment_type: "admission".to_string(),
            vitals_text: vitals.to_string(),
            capabilities: vec!["nurse_assess".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("nurse".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("IMP".to_string()),
        };
        match agent.evaluate(&input, &state.policy_engine).await {
            Ok(mut output) => {
                let elapsed = llm_start.elapsed().as_millis() as u64;
                emit_post_llm_allow(&emitter, elapsed, "allow_nurse_assess", "Nursing assessment parsed", phase_ms).await;
                let _ = state.audit_store.append(&mut output.audit_event).await;
                emitter.emit(AgentEventType::AuditCreation { audit_event_id: output.audit_event.id.to_string() });
                emitter.emit(AgentEventType::FhirWrite { resource_type: "Observation".into(), resource_id: None });
                persist_turn(&state, &ws_id, "nurse_assess", "evaluate",
                    serde_json::json!({"encounter_id": encounter_id}),
                    serde_json::to_value(&output.observation).unwrap_or_default(),
                    output.confidence.clone(), &emitter).await;
                emitter.emit(AgentEventType::AgentCompleted { confidence_score: output.confidence.score, elapsed_ms: elapsed });
            }
            Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
        }
    }

    // Step 3: Ambient documentation
    sim_delay(base_ms, 19).await;
    {
        let emitter = EventEmitter::new(state.event_tx.clone(), encounter_id, "ambient_doc");
        emitter.emit(AgentEventType::AgentStarted);
        emit_pre_llm(&emitter, "note_generation", "ambient_doc.generate_note", phase_ms).await;
        let llm_start = std::time::Instant::now();
        let input = cliniclaw_agents::AmbientDocInput {
            encounter_id: encounter_id.to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: patient_id.to_string(),
            practitioner_id: practitioner_id.to_string(),
            transcript: "Patient with COPD exacerbation presenting with worsening dyspnea over 3 days. SpO2 88% on room air. ABG shows respiratory acidosis. Initiated oxygen therapy. Will start prednisone for exacerbation and albuterol nebulizer treatments. Patient has history of COPD and CAD.".to_string(),
            chief_complaint: Some("COPD exacerbation, increasing dyspnea".to_string()),
            active_medications: vec!["Tiotropium 18mcg daily".to_string(), "Fluticasone 250mcg BID".to_string()],
            capabilities: vec!["note_generation".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("physician".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("IMP".to_string()),
        };
        match state.ambient_agent.generate_note(&input, &state.policy_engine).await {
            Ok(mut output) => {
                let elapsed = llm_start.elapsed().as_millis() as u64;
                emit_post_llm_allow(&emitter, elapsed, "allow_note_generation", "SOAP note parsed", phase_ms).await;
                let _ = state.audit_store.append(&mut output.audit_event).await;
                emitter.emit(AgentEventType::AuditCreation { audit_event_id: output.audit_event.id.to_string() });
                emitter.emit(AgentEventType::FhirWrite { resource_type: "DiagnosticReport".into(), resource_id: None });
                persist_turn(&state, &ws_id, "ambient_doc", "generate_note",
                    serde_json::json!({"encounter_id": encounter_id}),
                    serde_json::to_value(&output.report).unwrap_or_default(),
                    output.confidence.clone(), &emitter).await;
                emitter.emit(AgentEventType::AgentCompleted { confidence_score: output.confidence.score, elapsed_ms: elapsed });
            }
            Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
        }
    }

    // Step 4a: Order entry — prednisone
    sim_delay(base_ms, 20).await;
    {
        let emitter = EventEmitter::new(state.event_tx.clone(), encounter_id, "order_entry");
        emitter.emit(AgentEventType::AgentStarted);
        emit_pre_llm(&emitter, "order_entry", "order_entry.propose_order", phase_ms).await;
        let llm_start = std::time::Instant::now();
        let agent = cliniclaw_agents::OrderEntryAgent::new(state.llm.clone());
        let input = cliniclaw_agents::OrderEntryInput {
            encounter_id: encounter_id.to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: patient_id.to_string(),
            practitioner_id: practitioner_id.to_string(),
            order_text: "prednisone 40mg PO daily for COPD exacerbation".to_string(),
            active_medications: vec!["Tiotropium 18mcg daily".to_string(), "Fluticasone 250mcg BID".to_string()],
            capabilities: vec!["order_entry".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("physician".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("IMP".to_string()),
        };
        match agent.propose_order(&input, &state.policy_engine).await {
            Ok(mut output) => {
                let elapsed = llm_start.elapsed().as_millis() as u64;
                emit_post_llm_allow(&emitter, elapsed, "allow_order_entry", "MedicationRequest parsed", phase_ms).await;
                use cliniclaw_agents::CdsIndicator;
                let max_sev = output.cds_cards.iter().map(|c| match c.indicator {
                    CdsIndicator::HardStop => 4, CdsIndicator::Critical => 3,
                    CdsIndicator::Warning => 2, CdsIndicator::Info => 1,
                }).max().map(|s| match s { 4 => "hard_stop", 3 => "critical", 2 => "warning", _ => "info" }).map(String::from);
                emitter.emit(AgentEventType::CdsCheck { cards_count: output.cds_cards.len(), max_severity: max_sev });
                let _ = state.audit_store.append(&mut output.audit_event).await;
                emitter.emit(AgentEventType::AuditCreation { audit_event_id: output.audit_event.id.to_string() });
                emitter.emit(AgentEventType::FhirWrite { resource_type: "MedicationRequest".into(), resource_id: None });
                persist_turn(&state, &ws_id, "order_entry", "propose_order",
                    serde_json::json!({"encounter_id": encounter_id, "order_text": "prednisone 40mg PO daily"}),
                    serde_json::to_value(&output.medication_request).unwrap_or_default(),
                    output.confidence.clone(), &emitter).await;
                emitter.emit(AgentEventType::AgentCompleted { confidence_score: output.confidence.score, elapsed_ms: elapsed });
            }
            Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
        }
    }

    // Step 4b: Order entry — albuterol
    sim_delay(base_ms, 21).await;
    {
        let emitter = EventEmitter::new(state.event_tx.clone(), encounter_id, "order_entry");
        emitter.emit(AgentEventType::AgentStarted);
        emit_pre_llm(&emitter, "order_entry", "order_entry.propose_order", phase_ms).await;
        let llm_start = std::time::Instant::now();
        let agent = cliniclaw_agents::OrderEntryAgent::new(state.llm.clone());
        let input = cliniclaw_agents::OrderEntryInput {
            encounter_id: encounter_id.to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: patient_id.to_string(),
            practitioner_id: practitioner_id.to_string(),
            order_text: "albuterol nebulizer 2.5mg Q4H".to_string(),
            active_medications: vec!["Tiotropium 18mcg daily".to_string(), "Fluticasone 250mcg BID".to_string(), "Prednisone 40mg daily".to_string()],
            capabilities: vec!["order_entry".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("physician".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("IMP".to_string()),
        };
        match agent.propose_order(&input, &state.policy_engine).await {
            Ok(mut output) => {
                let elapsed = llm_start.elapsed().as_millis() as u64;
                emit_post_llm_allow(&emitter, elapsed, "allow_order_entry", "MedicationRequest parsed", phase_ms).await;
                use cliniclaw_agents::CdsIndicator;
                let max_sev = output.cds_cards.iter().map(|c| match c.indicator {
                    CdsIndicator::HardStop => 4, CdsIndicator::Critical => 3,
                    CdsIndicator::Warning => 2, CdsIndicator::Info => 1,
                }).max().map(|s| match s { 4 => "hard_stop", 3 => "critical", 2 => "warning", _ => "info" }).map(String::from);
                emitter.emit(AgentEventType::CdsCheck { cards_count: output.cds_cards.len(), max_severity: max_sev });
                let _ = state.audit_store.append(&mut output.audit_event).await;
                emitter.emit(AgentEventType::AuditCreation { audit_event_id: output.audit_event.id.to_string() });
                emitter.emit(AgentEventType::FhirWrite { resource_type: "MedicationRequest".into(), resource_id: None });
                persist_turn(&state, &ws_id, "order_entry", "propose_order",
                    serde_json::json!({"encounter_id": encounter_id, "order_text": "albuterol nebulizer 2.5mg Q4H"}),
                    serde_json::to_value(&output.medication_request).unwrap_or_default(),
                    output.confidence.clone(), &emitter).await;
                emitter.emit(AgentEventType::AgentCompleted { confidence_score: output.confidence.score, elapsed_ms: elapsed });
            }
            Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
        }
    }

    // Step 5: Lab review
    sim_delay(base_ms, 22).await;
    {
        let emitter = EventEmitter::new(state.event_tx.clone(), encounter_id, "lab_review");
        emitter.emit(AgentEventType::AgentStarted);
        emit_pre_llm(&emitter, "lab_review", "lab_review.interpret", phase_ms).await;
        let llm_start = std::time::Instant::now();
        let agent = cliniclaw_agents::LabReviewAgent::new(state.llm.clone());
        let input = cliniclaw_agents::LabReviewInput {
            encounter_id: encounter_id.to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: patient_id.to_string(),
            practitioner_id: practitioner_id.to_string(),
            lab_results_text: "WBC 14.2, ABG: pH 7.32 pCO2 52 pO2 58".to_string(),
            active_conditions: vec!["J44.1".to_string(), "I25.10".to_string()],
            capabilities: vec!["lab_review".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("physician".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("IMP".to_string()),
        };
        match agent.interpret(&input, &state.policy_engine).await {
            Ok(mut output) => {
                let elapsed = llm_start.elapsed().as_millis() as u64;
                emit_post_llm_allow(&emitter, elapsed, "allow_lab_review", "Lab interpretation parsed", phase_ms).await;
                let _ = state.audit_store.append(&mut output.audit_event).await;
                emitter.emit(AgentEventType::AuditCreation { audit_event_id: output.audit_event.id.to_string() });
                emitter.emit(AgentEventType::FhirWrite { resource_type: "DiagnosticReport".into(), resource_id: None });
                persist_turn(&state, &ws_id, "lab_review", "interpret",
                    serde_json::json!({"encounter_id": encounter_id}),
                    serde_json::to_value(&output.report).unwrap_or_default(),
                    output.confidence.clone(), &emitter).await;
                emitter.emit(AgentEventType::AgentCompleted { confidence_score: output.confidence.score, elapsed_ms: elapsed });
            }
            Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
        }
    }

    // Step 6: Pharmacy review
    sim_delay(base_ms, 23).await;
    {
        let emitter = EventEmitter::new(state.event_tx.clone(), encounter_id, "pharmacy_review");
        emitter.emit(AgentEventType::AgentStarted);
        emit_pre_llm(&emitter, "pharmacy_review", "pharmacy_review.evaluate", phase_ms).await;
        let llm_start = std::time::Instant::now();
        let agent = cliniclaw_agents::PharmacyReviewAgent::new(state.llm.clone());
        let input = cliniclaw_agents::PharmacyReviewInput {
            encounter_id: encounter_id.to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: patient_id.to_string(),
            practitioner_id: practitioner_id.to_string(),
            pending_orders: vec!["Prednisone 40mg daily".to_string(), "Albuterol neb 2.5mg Q4H".to_string()],
            active_medications: vec!["Tiotropium 18mcg daily".to_string(), "Fluticasone 250mcg BID".to_string()],
            allergies: vec![],
            capabilities: vec!["pharmacy_review".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("pharmacist".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("IMP".to_string()),
        };
        match agent.evaluate(&input, &state.policy_engine).await {
            Ok(mut output) => {
                let elapsed = llm_start.elapsed().as_millis() as u64;
                emit_post_llm_allow(&emitter, elapsed, "allow_pharmacy_review", "Pharmacy review parsed", phase_ms).await;
                use cliniclaw_agents::CdsIndicator;
                let max_sev = output.cds_cards.iter().map(|c| match c.indicator {
                    CdsIndicator::HardStop => 4, CdsIndicator::Critical => 3,
                    CdsIndicator::Warning => 2, CdsIndicator::Info => 1,
                }).max().map(|s| match s { 4 => "hard_stop", 3 => "critical", 2 => "warning", _ => "info" }).map(String::from);
                emitter.emit(AgentEventType::CdsCheck { cards_count: output.cds_cards.len(), max_severity: max_sev });
                let _ = state.audit_store.append(&mut output.audit_event).await;
                emitter.emit(AgentEventType::AuditCreation { audit_event_id: output.audit_event.id.to_string() });
                persist_turn(&state, &ws_id, "pharmacy_review", "evaluate",
                    serde_json::json!({"encounter_id": encounter_id}),
                    serde_json::json!({"review_status": output.review_status}),
                    output.confidence.clone(), &emitter).await;
                emitter.emit(AgentEventType::AgentCompleted { confidence_score: output.confidence.score, elapsed_ms: elapsed });
            }
            Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
        }
    }
}

// ── Pathway 5: Emily Johnson (enc-005, patient-005, knee OA) ─────────────────
// triage → nurse → ambient_doc → prior_auth

async fn pathway_johnson(state: Arc<AppState>, base_ms: u64, phase_ms: u64) {
    let encounter_id = "enc-005";
    let patient_id = "patient-005";
    let practitioner_id = "prac-005";
    let vitals = "BP 132/78, HR 72, Temp 98.6, RR 14, SpO2 99%";

    let ws_id = ensure_workspace(&state, encounter_id, practitioner_id).await;

    // Step 1: Triage
    sim_delay(base_ms, 24).await;
    {
        let emitter = EventEmitter::new(state.event_tx.clone(), encounter_id, "triage_assess");
        emitter.emit(AgentEventType::AgentStarted);
        emit_pre_llm(&emitter, "triage_assess", "triage_assess.evaluate", phase_ms).await;
        let llm_start = std::time::Instant::now();
        let agent = cliniclaw_agents::TriageAssessAgent::new(state.llm.clone());
        let input = cliniclaw_agents::TriageAssessInput {
            encounter_id: encounter_id.to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: patient_id.to_string(),
            practitioner_id: practitioner_id.to_string(),
            chief_complaint: "Bilateral knee pain, pre-surgical evaluation".to_string(),
            vitals_text: vitals.to_string(),
            capabilities: vec!["triage_assess".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("nurse".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("AMB".to_string()),
        };
        match agent.evaluate(&input, &state.policy_engine).await {
            Ok(mut output) => {
                let elapsed = llm_start.elapsed().as_millis() as u64;
                emit_post_llm_allow(&emitter, elapsed, "allow_triage_assess", "Triage assessment parsed", phase_ms).await;
                let _ = state.audit_store.append(&mut output.audit_event).await;
                emitter.emit(AgentEventType::AuditCreation { audit_event_id: output.audit_event.id.to_string() });
                emitter.emit(AgentEventType::FhirWrite { resource_type: "Observation".into(), resource_id: None });
                persist_turn(&state, &ws_id, "triage_assess", "evaluate",
                    serde_json::json!({"encounter_id": encounter_id}),
                    serde_json::to_value(&output.observation).unwrap_or_default(),
                    output.confidence.clone(), &emitter).await;
                emitter.emit(AgentEventType::AgentCompleted { confidence_score: output.confidence.score, elapsed_ms: elapsed });
            }
            Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
        }
    }

    // Step 2: Nursing assessment
    sim_delay(base_ms, 25).await;
    {
        let emitter = EventEmitter::new(state.event_tx.clone(), encounter_id, "nurse_assess");
        emitter.emit(AgentEventType::AgentStarted);
        emit_pre_llm(&emitter, "nurse_assess", "nurse_assess.evaluate", phase_ms).await;
        let llm_start = std::time::Instant::now();
        let agent = cliniclaw_agents::NurseAssessAgent::new(state.llm.clone());
        let input = cliniclaw_agents::NurseAssessInput {
            encounter_id: encounter_id.to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: patient_id.to_string(),
            practitioner_id: practitioner_id.to_string(),
            assessment_type: "admission".to_string(),
            vitals_text: vitals.to_string(),
            capabilities: vec!["nurse_assess".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("nurse".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("AMB".to_string()),
        };
        match agent.evaluate(&input, &state.policy_engine).await {
            Ok(mut output) => {
                let elapsed = llm_start.elapsed().as_millis() as u64;
                emit_post_llm_allow(&emitter, elapsed, "allow_nurse_assess", "Nursing assessment parsed", phase_ms).await;
                let _ = state.audit_store.append(&mut output.audit_event).await;
                emitter.emit(AgentEventType::AuditCreation { audit_event_id: output.audit_event.id.to_string() });
                emitter.emit(AgentEventType::FhirWrite { resource_type: "Observation".into(), resource_id: None });
                persist_turn(&state, &ws_id, "nurse_assess", "evaluate",
                    serde_json::json!({"encounter_id": encounter_id}),
                    serde_json::to_value(&output.observation).unwrap_or_default(),
                    output.confidence.clone(), &emitter).await;
                emitter.emit(AgentEventType::AgentCompleted { confidence_score: output.confidence.score, elapsed_ms: elapsed });
            }
            Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
        }
    }

    // Step 3: Ambient documentation
    sim_delay(base_ms, 26).await;
    {
        let emitter = EventEmitter::new(state.event_tx.clone(), encounter_id, "ambient_doc");
        emitter.emit(AgentEventType::AgentStarted);
        emit_pre_llm(&emitter, "note_generation", "ambient_doc.generate_note", phase_ms).await;
        let llm_start = std::time::Instant::now();
        let input = cliniclaw_agents::AmbientDocInput {
            encounter_id: encounter_id.to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: patient_id.to_string(),
            practitioner_id: practitioner_id.to_string(),
            transcript: "Pre-surgical evaluation for bilateral knee replacement. Patient reports bilateral knee pain for 5 years, progressive. X-rays show severe joint space narrowing bilaterally. Failed 6 months of conservative management including PT, NSAIDs, and cortisone injections. Cleared for bilateral total knee arthroplasty. Will submit prior authorization to insurance.".to_string(),
            chief_complaint: Some("Bilateral knee pain, pre-surgical evaluation for total knee arthroplasty".to_string()),
            active_medications: vec![],
            capabilities: vec!["note_generation".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("physician".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("AMB".to_string()),
        };
        match state.ambient_agent.generate_note(&input, &state.policy_engine).await {
            Ok(mut output) => {
                let elapsed = llm_start.elapsed().as_millis() as u64;
                emit_post_llm_allow(&emitter, elapsed, "allow_note_generation", "SOAP note parsed", phase_ms).await;
                let _ = state.audit_store.append(&mut output.audit_event).await;
                emitter.emit(AgentEventType::AuditCreation { audit_event_id: output.audit_event.id.to_string() });
                emitter.emit(AgentEventType::FhirWrite { resource_type: "DiagnosticReport".into(), resource_id: None });
                persist_turn(&state, &ws_id, "ambient_doc", "generate_note",
                    serde_json::json!({"encounter_id": encounter_id}),
                    serde_json::to_value(&output.report).unwrap_or_default(),
                    output.confidence.clone(), &emitter).await;
                emitter.emit(AgentEventType::AgentCompleted { confidence_score: output.confidence.score, elapsed_ms: elapsed });
            }
            Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
        }
    }

    // Step 4: Prior authorization
    sim_delay(base_ms, 27).await;
    {
        let emitter = EventEmitter::new(state.event_tx.clone(), encounter_id, "prior_auth");
        emitter.emit(AgentEventType::AgentStarted);
        emit_pre_llm(&emitter, "prior_auth", "prior_auth.assemble_package", phase_ms).await;
        let llm_start = std::time::Instant::now();
        let agent = cliniclaw_agents::PriorAuthAgent::new(state.llm.clone());
        let input = cliniclaw_agents::PriorAuthInput {
            encounter_id: encounter_id.to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: patient_id.to_string(),
            practitioner_id: practitioner_id.to_string(),
            service_request_id: "sr-005".to_string(),
            service_description: "Bilateral total knee arthroplasty".to_string(),
            diagnosis_codes: vec!["M17.0".to_string()],
            cpt_codes: vec!["27447".to_string()],
            clinical_notes: Some("Failed 6 months conservative management including PT, NSAIDs, and cortisone injections. Severe bilateral OA on imaging.".to_string()),
            capabilities: vec!["prior_auth".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("physician".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("AMB".to_string()),
        };
        match agent.assemble_package(&input, &state.policy_engine).await {
            Ok(mut output) => {
                let elapsed = llm_start.elapsed().as_millis() as u64;
                emit_post_llm_allow(&emitter, elapsed, "allow_prior_auth", "Prior auth package parsed", phase_ms).await;
                let _ = state.audit_store.append(&mut output.audit_event).await;
                emitter.emit(AgentEventType::AuditCreation { audit_event_id: output.audit_event.id.to_string() });
                persist_turn(&state, &ws_id, "prior_auth", "assemble_package",
                    serde_json::json!({"encounter_id": encounter_id, "cpt_codes": ["27447"]}),
                    serde_json::json!({
                        "diagnosis_summary": output.diagnosis_summary,
                        "clinical_justification": output.clinical_justification,
                        "urgency": output.urgency,
                    }),
                    output.confidence.clone(), &emitter).await;
                emitter.emit(AgentEventType::AgentCompleted { confidence_score: output.confidence.score, elapsed_ms: elapsed });
            }
            Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
        }
    }
}

// ── Pathway 6: David Williams (enc-006, patient-006, CHF) ────────────────────
// triage → nurse → ambient_doc → order_entry → pharmacy → lab_review → discharge

async fn pathway_williams(state: Arc<AppState>, base_ms: u64, phase_ms: u64) {
    let encounter_id = "enc-006";
    let patient_id = "patient-006";
    let practitioner_id = "prac-006";
    let vitals = "BP 158/94, HR 88, Temp 98.2, RR 22, SpO2 93%";

    let ws_id = ensure_workspace(&state, encounter_id, practitioner_id).await;

    // Step 1: Triage
    sim_delay(base_ms, 28).await;
    {
        let emitter = EventEmitter::new(state.event_tx.clone(), encounter_id, "triage_assess");
        emitter.emit(AgentEventType::AgentStarted);
        emit_pre_llm(&emitter, "triage_assess", "triage_assess.evaluate", phase_ms).await;
        let llm_start = std::time::Instant::now();
        let agent = cliniclaw_agents::TriageAssessAgent::new(state.llm.clone());
        let input = cliniclaw_agents::TriageAssessInput {
            encounter_id: encounter_id.to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: patient_id.to_string(),
            practitioner_id: practitioner_id.to_string(),
            chief_complaint: "CHF exacerbation, weight gain and edema".to_string(),
            vitals_text: vitals.to_string(),
            capabilities: vec!["triage_assess".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("nurse".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("IMP".to_string()),
        };
        match agent.evaluate(&input, &state.policy_engine).await {
            Ok(mut output) => {
                let elapsed = llm_start.elapsed().as_millis() as u64;
                emit_post_llm_allow(&emitter, elapsed, "allow_triage_assess", "Triage assessment parsed", phase_ms).await;
                let _ = state.audit_store.append(&mut output.audit_event).await;
                emitter.emit(AgentEventType::AuditCreation { audit_event_id: output.audit_event.id.to_string() });
                emitter.emit(AgentEventType::FhirWrite { resource_type: "Observation".into(), resource_id: None });
                persist_turn(&state, &ws_id, "triage_assess", "evaluate",
                    serde_json::json!({"encounter_id": encounter_id}),
                    serde_json::to_value(&output.observation).unwrap_or_default(),
                    output.confidence.clone(), &emitter).await;
                emitter.emit(AgentEventType::AgentCompleted { confidence_score: output.confidence.score, elapsed_ms: elapsed });
            }
            Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
        }
    }

    // Step 2: Nursing assessment
    sim_delay(base_ms, 29).await;
    {
        let emitter = EventEmitter::new(state.event_tx.clone(), encounter_id, "nurse_assess");
        emitter.emit(AgentEventType::AgentStarted);
        emit_pre_llm(&emitter, "nurse_assess", "nurse_assess.evaluate", phase_ms).await;
        let llm_start = std::time::Instant::now();
        let agent = cliniclaw_agents::NurseAssessAgent::new(state.llm.clone());
        let input = cliniclaw_agents::NurseAssessInput {
            encounter_id: encounter_id.to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: patient_id.to_string(),
            practitioner_id: practitioner_id.to_string(),
            assessment_type: "admission".to_string(),
            vitals_text: vitals.to_string(),
            capabilities: vec!["nurse_assess".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("nurse".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("IMP".to_string()),
        };
        match agent.evaluate(&input, &state.policy_engine).await {
            Ok(mut output) => {
                let elapsed = llm_start.elapsed().as_millis() as u64;
                emit_post_llm_allow(&emitter, elapsed, "allow_nurse_assess", "Nursing assessment parsed", phase_ms).await;
                let _ = state.audit_store.append(&mut output.audit_event).await;
                emitter.emit(AgentEventType::AuditCreation { audit_event_id: output.audit_event.id.to_string() });
                emitter.emit(AgentEventType::FhirWrite { resource_type: "Observation".into(), resource_id: None });
                persist_turn(&state, &ws_id, "nurse_assess", "evaluate",
                    serde_json::json!({"encounter_id": encounter_id}),
                    serde_json::to_value(&output.observation).unwrap_or_default(),
                    output.confidence.clone(), &emitter).await;
                emitter.emit(AgentEventType::AgentCompleted { confidence_score: output.confidence.score, elapsed_ms: elapsed });
            }
            Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
        }
    }

    // Step 3: Ambient documentation
    sim_delay(base_ms, 30).await;
    {
        let emitter = EventEmitter::new(state.event_tx.clone(), encounter_id, "ambient_doc");
        emitter.emit(AgentEventType::AgentStarted);
        emit_pre_llm(&emitter, "note_generation", "ambient_doc.generate_note", phase_ms).await;
        let llm_start = std::time::Instant::now();
        let input = cliniclaw_agents::AmbientDocInput {
            encounter_id: encounter_id.to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: patient_id.to_string(),
            practitioner_id: practitioner_id.to_string(),
            transcript: "Patient with known CHF and atrial fibrillation presenting with 5 lb weight gain over 3 days, worsening bilateral ankle edema, and dyspnea on exertion. BNP markedly elevated at 890. INR supratherapeutic at 2.8 on current warfarin 5mg. Will adjust warfarin dose and augment diuresis. Echocardiogram ordered.".to_string(),
            chief_complaint: Some("CHF exacerbation, weight gain and bilateral edema".to_string()),
            active_medications: vec!["Carvedilol 25mg BID".to_string(), "Furosemide 40mg daily".to_string(), "Warfarin 5mg daily".to_string()],
            capabilities: vec!["note_generation".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("physician".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("IMP".to_string()),
        };
        match state.ambient_agent.generate_note(&input, &state.policy_engine).await {
            Ok(mut output) => {
                let elapsed = llm_start.elapsed().as_millis() as u64;
                emit_post_llm_allow(&emitter, elapsed, "allow_note_generation", "SOAP note parsed", phase_ms).await;
                let _ = state.audit_store.append(&mut output.audit_event).await;
                emitter.emit(AgentEventType::AuditCreation { audit_event_id: output.audit_event.id.to_string() });
                emitter.emit(AgentEventType::FhirWrite { resource_type: "DiagnosticReport".into(), resource_id: None });
                persist_turn(&state, &ws_id, "ambient_doc", "generate_note",
                    serde_json::json!({"encounter_id": encounter_id}),
                    serde_json::to_value(&output.report).unwrap_or_default(),
                    output.confidence.clone(), &emitter).await;
                emitter.emit(AgentEventType::AgentCompleted { confidence_score: output.confidence.score, elapsed_ms: elapsed });
            }
            Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
        }
    }

    // Step 4: Order entry — warfarin adjustment
    sim_delay(base_ms, 31).await;
    {
        let emitter = EventEmitter::new(state.event_tx.clone(), encounter_id, "order_entry");
        emitter.emit(AgentEventType::AgentStarted);
        emit_pre_llm(&emitter, "order_entry", "order_entry.propose_order", phase_ms).await;
        let llm_start = std::time::Instant::now();
        let agent = cliniclaw_agents::OrderEntryAgent::new(state.llm.clone());
        let input = cliniclaw_agents::OrderEntryInput {
            encounter_id: encounter_id.to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: patient_id.to_string(),
            practitioner_id: practitioner_id.to_string(),
            order_text: "adjust warfarin to 7.5mg PO daily based on INR 2.8".to_string(),
            active_medications: vec!["Carvedilol 25mg BID".to_string(), "Furosemide 40mg daily".to_string(), "Warfarin 5mg daily".to_string()],
            capabilities: vec!["order_entry".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("physician".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("IMP".to_string()),
        };
        match agent.propose_order(&input, &state.policy_engine).await {
            Ok(mut output) => {
                let elapsed = llm_start.elapsed().as_millis() as u64;
                emit_post_llm_allow(&emitter, elapsed, "allow_order_entry", "MedicationRequest parsed", phase_ms).await;
                use cliniclaw_agents::CdsIndicator;
                let max_sev = output.cds_cards.iter().map(|c| match c.indicator {
                    CdsIndicator::HardStop => 4, CdsIndicator::Critical => 3,
                    CdsIndicator::Warning => 2, CdsIndicator::Info => 1,
                }).max().map(|s| match s { 4 => "hard_stop", 3 => "critical", 2 => "warning", _ => "info" }).map(String::from);
                emitter.emit(AgentEventType::CdsCheck { cards_count: output.cds_cards.len(), max_severity: max_sev });
                let _ = state.audit_store.append(&mut output.audit_event).await;
                emitter.emit(AgentEventType::AuditCreation { audit_event_id: output.audit_event.id.to_string() });
                emitter.emit(AgentEventType::FhirWrite { resource_type: "MedicationRequest".into(), resource_id: None });
                persist_turn(&state, &ws_id, "order_entry", "propose_order",
                    serde_json::json!({"encounter_id": encounter_id, "order_text": "adjust warfarin to 7.5mg PO daily"}),
                    serde_json::to_value(&output.medication_request).unwrap_or_default(),
                    output.confidence.clone(), &emitter).await;
                emitter.emit(AgentEventType::AgentCompleted { confidence_score: output.confidence.score, elapsed_ms: elapsed });
            }
            Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
        }
    }

    // Step 5: Pharmacy review
    sim_delay(base_ms, 32).await;
    {
        let emitter = EventEmitter::new(state.event_tx.clone(), encounter_id, "pharmacy_review");
        emitter.emit(AgentEventType::AgentStarted);
        emit_pre_llm(&emitter, "pharmacy_review", "pharmacy_review.evaluate", phase_ms).await;
        let llm_start = std::time::Instant::now();
        let agent = cliniclaw_agents::PharmacyReviewAgent::new(state.llm.clone());
        let input = cliniclaw_agents::PharmacyReviewInput {
            encounter_id: encounter_id.to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: patient_id.to_string(),
            practitioner_id: practitioner_id.to_string(),
            pending_orders: vec!["Warfarin 7.5mg daily".to_string()],
            active_medications: vec!["Carvedilol 25mg BID".to_string(), "Furosemide 40mg daily".to_string(), "Warfarin 5mg daily".to_string()],
            allergies: vec![],
            capabilities: vec!["pharmacy_review".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("pharmacist".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("IMP".to_string()),
        };
        match agent.evaluate(&input, &state.policy_engine).await {
            Ok(mut output) => {
                let elapsed = llm_start.elapsed().as_millis() as u64;
                emit_post_llm_allow(&emitter, elapsed, "allow_pharmacy_review", "Pharmacy review parsed", phase_ms).await;
                use cliniclaw_agents::CdsIndicator;
                let max_sev = output.cds_cards.iter().map(|c| match c.indicator {
                    CdsIndicator::HardStop => 4, CdsIndicator::Critical => 3,
                    CdsIndicator::Warning => 2, CdsIndicator::Info => 1,
                }).max().map(|s| match s { 4 => "hard_stop", 3 => "critical", 2 => "warning", _ => "info" }).map(String::from);
                emitter.emit(AgentEventType::CdsCheck { cards_count: output.cds_cards.len(), max_severity: max_sev });
                let _ = state.audit_store.append(&mut output.audit_event).await;
                emitter.emit(AgentEventType::AuditCreation { audit_event_id: output.audit_event.id.to_string() });
                persist_turn(&state, &ws_id, "pharmacy_review", "evaluate",
                    serde_json::json!({"encounter_id": encounter_id}),
                    serde_json::json!({"review_status": output.review_status}),
                    output.confidence.clone(), &emitter).await;
                emitter.emit(AgentEventType::AgentCompleted { confidence_score: output.confidence.score, elapsed_ms: elapsed });
            }
            Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
        }
    }

    // Step 6: Lab review
    sim_delay(base_ms, 33).await;
    {
        let emitter = EventEmitter::new(state.event_tx.clone(), encounter_id, "lab_review");
        emitter.emit(AgentEventType::AgentStarted);
        emit_pre_llm(&emitter, "lab_review", "lab_review.interpret", phase_ms).await;
        let llm_start = std::time::Instant::now();
        let agent = cliniclaw_agents::LabReviewAgent::new(state.llm.clone());
        let input = cliniclaw_agents::LabReviewInput {
            encounter_id: encounter_id.to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: patient_id.to_string(),
            practitioner_id: practitioner_id.to_string(),
            lab_results_text: "BNP 890, Creatinine 1.8, INR 2.8".to_string(),
            active_conditions: vec!["I50.9".to_string(), "I48.91".to_string()],
            capabilities: vec!["lab_review".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("physician".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("IMP".to_string()),
        };
        match agent.interpret(&input, &state.policy_engine).await {
            Ok(mut output) => {
                let elapsed = llm_start.elapsed().as_millis() as u64;
                emit_post_llm_allow(&emitter, elapsed, "allow_lab_review", "Lab interpretation parsed", phase_ms).await;
                let _ = state.audit_store.append(&mut output.audit_event).await;
                emitter.emit(AgentEventType::AuditCreation { audit_event_id: output.audit_event.id.to_string() });
                emitter.emit(AgentEventType::FhirWrite { resource_type: "DiagnosticReport".into(), resource_id: None });
                persist_turn(&state, &ws_id, "lab_review", "interpret",
                    serde_json::json!({"encounter_id": encounter_id}),
                    serde_json::to_value(&output.report).unwrap_or_default(),
                    output.confidence.clone(), &emitter).await;
                emitter.emit(AgentEventType::AgentCompleted { confidence_score: output.confidence.score, elapsed_ms: elapsed });
            }
            Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
        }
    }

    // Step 7: Discharge plan
    sim_delay(base_ms, 34).await;
    {
        let emitter = EventEmitter::new(state.event_tx.clone(), encounter_id, "discharge_plan");
        emitter.emit(AgentEventType::AgentStarted);
        emit_pre_llm(&emitter, "discharge_plan", "discharge_plan.generate", phase_ms).await;
        let llm_start = std::time::Instant::now();
        let agent = cliniclaw_agents::DischargePlanAgent::new(state.llm.clone());
        let input_williams_discharge = cliniclaw_agents::DischargePlanInput {
            encounter_id: encounter_id.to_string(),
            encounter_status: "in-progress".to_string(),
            patient_id: patient_id.to_string(),
            practitioner_id: practitioner_id.to_string(),
            active_conditions: vec!["Heart failure".to_string(), "Atrial fibrillation".to_string()],
            current_medications: vec!["Carvedilol 25mg BID".to_string(), "Furosemide 40mg daily".to_string(), "Warfarin 7.5mg daily".to_string()],
            assessment_summary: "CHF exacerbation with volume overload now resolved with augmented diuresis. Warfarin adjusted for atrial fibrillation anticoagulation. BNP improving. Patient stable for discharge with close outpatient follow-up.".to_string(),
            capabilities: vec!["discharge_plan".to_string()],
            capability_tokens: vec![],
            practitioner_role: Some("physician".to_string()),
            patient_active: true,
            patient_deceased: None,
            encounter_class: Some("IMP".to_string()),
        };
        match agent.generate(&input_williams_discharge, &state.policy_engine).await {
            Ok(mut output) => {
                let elapsed = llm_start.elapsed().as_millis() as u64;
                emit_post_llm_allow(&emitter, elapsed, "allow_discharge_plan", "Discharge plan parsed", phase_ms).await;
                let _ = state.audit_store.append(&mut output.audit_event).await;
                emitter.emit(AgentEventType::AuditCreation { audit_event_id: output.audit_event.id.to_string() });
                emitter.emit(AgentEventType::FhirWrite { resource_type: "DiagnosticReport".into(), resource_id: None });
                persist_turn(&state, &ws_id, "discharge_plan", "generate",
                    serde_json::json!({"encounter_id": encounter_id, "condition_count": 2}),
                    serde_json::to_value(&output.report).unwrap_or_default(),
                    output.confidence.clone(), &emitter).await;
                emitter.emit(AgentEventType::AgentCompleted { confidence_score: output.confidence.score, elapsed_ms: elapsed });
            }
            Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
        }
    }
}
