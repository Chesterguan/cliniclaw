//! Dynamic simulation — discovers active encounters from FHIR store and runs
//! agent pathways based on patient conditions. Works with Synthea data.

use std::sync::Arc;

use axum::extract::{Json, State};
use axum::http::StatusCode;

use cliniclaw_kernel::{AgentEventType, EventEmitter};

use crate::error::ApiError;
use crate::state::AppState;

use super::simulate::{
    ensure_workspace, persist_turn, emit_pre_llm, emit_post_llm_allow, sim_delay,
};

fn default_speed() -> String {
    "fast".to_string()
}

#[derive(Debug, serde::Deserialize)]
pub struct DynamicSimulateRequest {
    #[serde(default = "default_speed")]
    pub speed: String,
    /// Maximum number of concurrent patient pathways (default: 10)
    #[serde(default = "default_max_pathways")]
    pub max_pathways: usize,
}

fn default_max_pathways() -> usize {
    10
}

#[derive(Debug, serde::Serialize)]
pub struct DynamicSimulateResponse {
    pub status: String,
    pub pathways: usize,
    pub patients: Vec<DynamicPathwaySummary>,
}

#[derive(Debug, serde::Serialize)]
pub struct DynamicPathwaySummary {
    pub encounter_id: String,
    pub patient_display: String,
    pub encounter_class: String,
    pub conditions: Vec<String>,
    pub agents: Vec<String>,
}

pub async fn run_dynamic_simulation(
    State(state): State<Arc<AppState>>,
    Json(body): Json<DynamicSimulateRequest>,
) -> Result<Json<DynamicSimulateResponse>, ApiError> {
    let base_delay_ms: u64 = match body.speed.as_str() {
        "fast" => 300,
        "demo" => 2000,
        _ => 1500,
    };
    let phase_delay_ms: u64 = match body.speed.as_str() {
        "fast" => 0,
        "demo" => 400,
        _ => 100,
    };

    // Discover in-progress encounters
    let enc_bundle = state
        .fhir
        .search_resources("Encounter", &[("status", "in-progress")])
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, format!("FHIR search failed: {e}")))?;

    let enc_entries = enc_bundle
        .get("entry")
        .and_then(|e| e.as_array())
        .cloned()
        .unwrap_or_default();

    if enc_entries.is_empty() {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "no in-progress encounters found — load Synthea data with SYNTHEA_DIR or use built-in mock data",
        ));
    }

    let mut summaries = Vec::new();
    let limit = body.max_pathways.min(20);

    for entry in enc_entries.iter().take(limit) {
        let Some(enc) = entry.get("resource") else { continue };
        let enc_id = enc.get("id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let patient_ref = enc
            .pointer("/subject/reference")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let patient_display = enc
            .pointer("/subject/display")
            .and_then(|v| v.as_str())
            .unwrap_or(&patient_ref)
            .to_string();
        let enc_class = enc
            .pointer("/class/code")
            .and_then(|v| v.as_str())
            .unwrap_or("AMB")
            .to_string();

        if enc_id.is_empty() || patient_ref.is_empty() {
            continue;
        }

        let patient_id = patient_ref
            .strip_prefix("Patient/")
            .unwrap_or(&patient_ref)
            .to_string();

        // Look up conditions for this patient
        let cond_bundle = state
            .fhir
            .search_resources("Condition", &[("patient", &patient_id)])
            .await
            .unwrap_or_else(|_| serde_json::json!({"entry": []}));

        let condition_displays: Vec<String> = cond_bundle
            .get("entry")
            .and_then(|e| e.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|e| {
                        let display = e.pointer("/resource/code/coding/0/display")
                            .and_then(|v| v.as_str());
                        let text = e.pointer("/resource/code/text")
                            .and_then(|v| v.as_str());
                        display.or(text).map(|s| s.to_string())
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Look up active medications
        let med_bundle = state
            .fhir
            .search_resources("MedicationRequest", &[("patient", &patient_id)])
            .await
            .unwrap_or_else(|_| serde_json::json!({"entry": []}));

        let medications: Vec<String> = med_bundle
            .get("entry")
            .and_then(|e| e.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|e| {
                        e.pointer("/resource/medicationCodeableConcept/text")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                    })
                    .collect()
            })
            .unwrap_or_default();

        let agents = determine_pathway(&condition_displays, &medications, &enc_class);

        summaries.push(DynamicPathwaySummary {
            encounter_id: enc_id.clone(),
            patient_display: patient_display.clone(),
            encounter_class: enc_class.clone(),
            conditions: condition_displays.clone(),
            agents: agents.clone(),
        });

        let s = state.clone();
        let conds = condition_displays;
        let meds = medications;
        tokio::spawn(async move {
            dynamic_pathway(s, enc_id, patient_id, patient_display, enc_class, conds, meds, agents, base_delay_ms, phase_delay_ms).await;
        });
    }

    let pathway_count = summaries.len();
    Ok(Json(DynamicSimulateResponse {
        status: "running".to_string(),
        pathways: pathway_count,
        patients: summaries,
    }))
}

/// Determine which agents should process this patient based on conditions and encounter class.
fn determine_pathway(conditions: &[String], medications: &[String], enc_class: &str) -> Vec<String> {
    let mut agents = vec!["triage_assess".to_string(), "nurse_assess".to_string()];
    agents.push("ambient_doc".to_string());

    let cond_lower: Vec<String> = conditions.iter().map(|c| c.to_lowercase()).collect();
    let med_lower: Vec<String> = medications.iter().map(|m| m.to_lowercase()).collect();

    let has_diabetes = cond_lower.iter().any(|c| c.contains("diabetes") || c.contains("glucose"));
    let has_hypertension = cond_lower.iter().any(|c| c.contains("hypertension") || c.contains("blood pressure"));
    let has_copd = cond_lower.iter().any(|c| c.contains("copd") || c.contains("chronic obstructive") || c.contains("pulmonary disease"));
    let has_chf = cond_lower.iter().any(|c| c.contains("heart failure") || c.contains("cardiac"));
    let has_oa = cond_lower.iter().any(|c| c.contains("osteoarthritis") || c.contains("knee"));
    let has_high_risk_meds = med_lower.iter().any(|m| {
        m.contains("warfarin") || m.contains("insulin") || m.contains("heparin")
            || m.contains("opioid") || m.contains("morphine") || m.contains("fentanyl")
    });

    if has_diabetes || has_chf || has_copd {
        agents.push("lab_review".to_string());
    }
    if has_diabetes || has_hypertension || has_copd || has_chf {
        agents.push("order_entry".to_string());
    }
    if has_high_risk_meds || conditions.len() >= 3 {
        agents.push("pharmacy_review".to_string());
    }
    if has_oa {
        agents.push("prior_auth".to_string());
    }
    if enc_class == "IMP" || (conditions.len() >= 2 && (has_chf || has_copd)) {
        agents.push("discharge_plan".to_string());
    }

    // If no special conditions, still add pharmacy review for polypharmacy
    if agents.len() <= 3 && medications.len() >= 3 {
        agents.push("pharmacy_review".to_string());
    }

    agents
}

async fn dynamic_pathway(
    state: Arc<AppState>,
    encounter_id: String,
    patient_id: String,
    patient_display: String,
    enc_class: String,
    conditions: Vec<String>,
    medications: Vec<String>,
    agents: Vec<String>,
    base_ms: u64,
    phase_ms: u64,
) {
    let practitioner_id = "practitioner-001";
    let ws_id = ensure_workspace(&state, &encounter_id, practitioner_id).await;
    let conditions_text = if conditions.is_empty() { "routine visit".to_string() } else { conditions.join(", ") };
    let meds_text = if medications.is_empty() { "none".to_string() } else { medications.join(", ") };
    let vitals = "BP 130/82, HR 78, Temp 98.6, RR 16, SpO2 97%";

    tracing::info!(
        encounter_id = %encounter_id,
        patient = %patient_display,
        agents = ?agents,
        conditions = %conditions_text,
        "starting dynamic pathway"
    );

    for (step, agent_name) in agents.iter().enumerate() {
        sim_delay(base_ms, (step + 1) as u64).await;

        let emitter = EventEmitter::new(state.event_tx.clone(), &encounter_id, agent_name);
        emitter.emit(AgentEventType::AgentStarted);

        match agent_name.as_str() {
            "triage_assess" => {
                emit_pre_llm(&emitter, "triage_assess", "triage_assess.evaluate", phase_ms).await;
                let t = std::time::Instant::now();
                let agent = cliniclaw_agents::TriageAssessAgent::new(state.llm.clone());
                let input = cliniclaw_agents::TriageAssessInput {
                    encounter_id: encounter_id.clone(),
                    encounter_status: "in-progress".into(),
                    patient_id: patient_id.clone(),
                    practitioner_id: practitioner_id.into(),
                    chief_complaint: conditions.first().cloned().unwrap_or_else(|| "Routine visit".into()),
                    vitals_text: vitals.into(),
                    capabilities: vec!["triage_assess".into()],
                    capability_tokens: vec![],
                    practitioner_role: Some("nurse".into()),
                    patient_active: true,
                    patient_deceased: None,
                    encounter_class: Some(enc_class.clone()),
                };
                match agent.evaluate(&input, &state.policy_engine).await {
                    Ok(mut o) => {
                        let ms = t.elapsed().as_millis() as u64;
                        emit_post_llm_allow(&emitter, ms, "allow_triage_assess", "Triage parsed", phase_ms).await;
                        let _ = state.audit_store.append(&mut o.audit_event).await;
                        emitter.emit(AgentEventType::AuditCreation { audit_event_id: o.audit_event.id.to_string() });
                        emitter.emit(AgentEventType::FhirWrite { resource_type: "Observation".into(), resource_id: o.observation.id.clone() });
                        persist_turn(&state, &ws_id, "triage_assess", "evaluate",
                            serde_json::json!({"encounter_id": &encounter_id}),
                            serde_json::json!({"triage_level": o.triage_level, "acuity": o.acuity_label}),
                            o.confidence.clone(), &emitter).await;
                        emitter.emit(AgentEventType::AgentCompleted { confidence_score: o.confidence.score, elapsed_ms: ms });
                    }
                    Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
                }
            }
            "nurse_assess" => {
                emit_pre_llm(&emitter, "nurse_assess", "nurse_assess.evaluate", phase_ms).await;
                let t = std::time::Instant::now();
                let agent = cliniclaw_agents::NurseAssessAgent::new(state.llm.clone());
                let input = cliniclaw_agents::NurseAssessInput {
                    encounter_id: encounter_id.clone(),
                    encounter_status: "in-progress".into(),
                    patient_id: patient_id.clone(),
                    practitioner_id: practitioner_id.into(),
                    assessment_type: "admission".into(),
                    vitals_text: vitals.into(),
                    capabilities: vec!["nurse_assess".into()],
                    capability_tokens: vec![],
                    practitioner_role: Some("nurse".into()),
                    patient_active: true,
                    patient_deceased: None,
                    encounter_class: Some(enc_class.clone()),
                };
                match agent.evaluate(&input, &state.policy_engine).await {
                    Ok(mut o) => {
                        let ms = t.elapsed().as_millis() as u64;
                        emit_post_llm_allow(&emitter, ms, "allow_nurse_assess", "Nurse assessment parsed", phase_ms).await;
                        let _ = state.audit_store.append(&mut o.audit_event).await;
                        emitter.emit(AgentEventType::AuditCreation { audit_event_id: o.audit_event.id.to_string() });
                        emitter.emit(AgentEventType::FhirWrite { resource_type: "Observation".into(), resource_id: o.observation.id.clone() });
                        persist_turn(&state, &ws_id, "nurse_assess", "evaluate",
                            serde_json::json!({"encounter_id": &encounter_id}),
                            serde_json::json!({"fall_risk": o.fall_risk_score, "pain": o.pain_score}),
                            o.confidence.clone(), &emitter).await;
                        emitter.emit(AgentEventType::AgentCompleted { confidence_score: o.confidence.score, elapsed_ms: ms });
                    }
                    Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
                }
            }
            "ambient_doc" => {
                emit_pre_llm(&emitter, "ambient_doc", "ambient_doc.generate_note", phase_ms).await;
                let t = std::time::Instant::now();
                let transcript = format!(
                    "Patient {} presents with {}. Current medications: {}. Vitals: {}.",
                    patient_display, conditions_text, meds_text, vitals,
                );
                let agent = cliniclaw_agents::AmbientDocAgent::new(state.llm.clone());
                let input = cliniclaw_agents::AmbientDocInput {
                    encounter_id: encounter_id.clone(),
                    encounter_status: "in-progress".into(),
                    patient_id: patient_id.clone(),
                    practitioner_id: practitioner_id.into(),
                    transcript,
                    chief_complaint: conditions.first().cloned(),
                    active_medications: medications.clone(),
                    capabilities: vec!["ambient_doc".into()],
                    capability_tokens: vec![],
                    practitioner_role: Some("physician".into()),
                    patient_active: true,
                    patient_deceased: None,
                    encounter_class: Some(enc_class.clone()),
                };
                match agent.generate_note(&input, &state.policy_engine).await {
                    Ok(mut o) => {
                        let ms = t.elapsed().as_millis() as u64;
                        emit_post_llm_allow(&emitter, ms, "allow_ambient_doc", "SOAP note parsed", phase_ms).await;
                        let _ = state.audit_store.append(&mut o.audit_event).await;
                        emitter.emit(AgentEventType::AuditCreation { audit_event_id: o.audit_event.id.to_string() });
                        emitter.emit(AgentEventType::FhirWrite { resource_type: "DiagnosticReport".into(), resource_id: o.report.id.clone() });
                        persist_turn(&state, &ws_id, "ambient_doc", "generate_note",
                            serde_json::json!({"encounter_id": &encounter_id}),
                            serde_json::to_value(&o.report).unwrap_or_default(),
                            o.confidence.clone(), &emitter).await;
                        emitter.emit(AgentEventType::AgentCompleted { confidence_score: o.confidence.score, elapsed_ms: ms });
                    }
                    Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
                }
            }
            "order_entry" => {
                emit_pre_llm(&emitter, "order_entry", "order_entry.parse_order", phase_ms).await;
                let t = std::time::Instant::now();
                let order_text = if conditions.iter().any(|c| c.to_lowercase().contains("diabetes")) {
                    "Start metformin 500mg PO BID"
                } else if conditions.iter().any(|c| c.to_lowercase().contains("hypertension")) {
                    "Start lisinopril 10mg PO daily"
                } else if conditions.iter().any(|c| c.to_lowercase().contains("copd") || c.to_lowercase().contains("pulmonary")) {
                    "Start prednisone 40mg PO daily x 5 days"
                } else if conditions.iter().any(|c| c.to_lowercase().contains("heart failure")) {
                    "Adjust furosemide to 40mg IV now"
                } else {
                    "Continue current medications"
                };
                let agent = cliniclaw_agents::OrderEntryAgent::new(state.llm.clone());
                let input = cliniclaw_agents::OrderEntryInput {
                    encounter_id: encounter_id.clone(),
                    encounter_status: "in-progress".into(),
                    patient_id: patient_id.clone(),
                    practitioner_id: practitioner_id.into(),
                    order_text: order_text.into(),
                    active_medications: medications.clone(),
                    capabilities: vec!["order_entry".into()],
                    capability_tokens: vec![],
                    practitioner_role: Some("physician".into()),
                    patient_active: true,
                    patient_deceased: None,
                    encounter_class: Some(enc_class.clone()),
                };
                match agent.propose_order(&input, &state.policy_engine).await {
                    Ok(mut o) => {
                        let ms = t.elapsed().as_millis() as u64;
                        emit_post_llm_allow(&emitter, ms, "allow_order_entry", "Order parsed", phase_ms).await;
                        let _ = state.audit_store.append(&mut o.audit_event).await;
                        emitter.emit(AgentEventType::AuditCreation { audit_event_id: o.audit_event.id.to_string() });
                        emitter.emit(AgentEventType::FhirWrite { resource_type: "MedicationRequest".into(), resource_id: o.medication_request.id.clone() });
                        persist_turn(&state, &ws_id, "order_entry", "parse_order",
                            serde_json::json!({"order_text": order_text}),
                            serde_json::to_value(&o.medication_request).unwrap_or_default(),
                            o.confidence.clone(), &emitter).await;
                        emitter.emit(AgentEventType::AgentCompleted { confidence_score: o.confidence.score, elapsed_ms: ms });
                    }
                    Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
                }
            }
            "lab_review" => {
                emit_pre_llm(&emitter, "lab_review", "lab_review.interpret", phase_ms).await;
                let t = std::time::Instant::now();
                let agent = cliniclaw_agents::LabReviewAgent::new(state.llm.clone());
                let input = cliniclaw_agents::LabReviewInput {
                    encounter_id: encounter_id.clone(),
                    encounter_status: "in-progress".into(),
                    patient_id: patient_id.clone(),
                    practitioner_id: practitioner_id.into(),
                    lab_results_text: "CBC, CMP, and relevant disease-specific labs reviewed.".into(),
                    active_conditions: conditions.clone(),
                    capabilities: vec!["lab_review".into()],
                    capability_tokens: vec![],
                    practitioner_role: Some("physician".into()),
                    patient_active: true,
                    patient_deceased: None,
                    encounter_class: Some(enc_class.clone()),
                };
                match agent.interpret(&input, &state.policy_engine).await {
                    Ok(mut o) => {
                        let ms = t.elapsed().as_millis() as u64;
                        emit_post_llm_allow(&emitter, ms, "allow_lab_review", "Lab review parsed", phase_ms).await;
                        let _ = state.audit_store.append(&mut o.audit_event).await;
                        emitter.emit(AgentEventType::AuditCreation { audit_event_id: o.audit_event.id.to_string() });
                        emitter.emit(AgentEventType::FhirWrite { resource_type: "DiagnosticReport".into(), resource_id: o.report.id.clone() });
                        persist_turn(&state, &ws_id, "lab_review", "interpret",
                            serde_json::json!({"encounter_id": &encounter_id}),
                            serde_json::json!({"flags": o.flags}),
                            o.confidence.clone(), &emitter).await;
                        emitter.emit(AgentEventType::AgentCompleted { confidence_score: o.confidence.score, elapsed_ms: ms });
                    }
                    Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
                }
            }
            "pharmacy_review" => {
                emit_pre_llm(&emitter, "pharmacy_review", "pharmacy_review.review", phase_ms).await;
                let t = std::time::Instant::now();
                let agent = cliniclaw_agents::PharmacyReviewAgent::new(state.llm.clone());
                let input = cliniclaw_agents::PharmacyReviewInput {
                    encounter_id: encounter_id.clone(),
                    encounter_status: "in-progress".into(),
                    patient_id: patient_id.clone(),
                    practitioner_id: practitioner_id.into(),
                    pending_orders: vec!["See current orders".into()],
                    active_medications: medications.clone(),
                    allergies: vec![],
                    capabilities: vec!["pharmacy_review".into()],
                    capability_tokens: vec![],
                    practitioner_role: Some("pharmacist".into()),
                    patient_active: true,
                    patient_deceased: None,
                    encounter_class: Some(enc_class.clone()),
                };
                match agent.evaluate(&input, &state.policy_engine).await {
                    Ok(mut o) => {
                        let ms = t.elapsed().as_millis() as u64;
                        let decision = if o.interactions_found.is_empty() { "allow" } else { "flagged" };
                        emit_post_llm_allow(&emitter, ms, decision, "Pharmacy review parsed", phase_ms).await;
                        let _ = state.audit_store.append(&mut o.audit_event).await;
                        emitter.emit(AgentEventType::AuditCreation { audit_event_id: o.audit_event.id.to_string() });
                        persist_turn(&state, &ws_id, "pharmacy_review", "review",
                            serde_json::json!({"encounter_id": &encounter_id}),
                            serde_json::json!({"status": o.review_status, "interactions": o.interactions_found}),
                            o.confidence.clone(), &emitter).await;
                        emitter.emit(AgentEventType::AgentCompleted { confidence_score: o.confidence.score, elapsed_ms: ms });
                    }
                    Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
                }
            }
            "prior_auth" => {
                emit_pre_llm(&emitter, "prior_auth", "prior_auth.generate_justification", phase_ms).await;
                let t = std::time::Instant::now();
                let agent = cliniclaw_agents::PriorAuthAgent::new(state.llm.clone());
                let input = cliniclaw_agents::PriorAuthInput {
                    encounter_id: encounter_id.clone(),
                    encounter_status: "in-progress".into(),
                    patient_id: patient_id.clone(),
                    practitioner_id: practitioner_id.into(),
                    service_request_id: "dynamic-sr".into(),
                    service_description: "Surgical procedure".into(),
                    diagnosis_codes: vec!["M17.0".into()],
                    cpt_codes: vec!["27447".into()],
                    clinical_notes: Some(conditions_text.clone()),
                    capabilities: vec!["prior_auth".into()],
                    capability_tokens: vec![],
                    practitioner_role: Some("physician".into()),
                    patient_active: true,
                    patient_deceased: None,
                    encounter_class: Some(enc_class.clone()),
                };
                match agent.assemble_package(&input, &state.policy_engine).await {
                    Ok(mut o) => {
                        let ms = t.elapsed().as_millis() as u64;
                        emit_post_llm_allow(&emitter, ms, "require_physician_signoff", "Prior auth parsed", phase_ms).await;
                        let _ = state.audit_store.append(&mut o.audit_event).await;
                        emitter.emit(AgentEventType::AuditCreation { audit_event_id: o.audit_event.id.to_string() });
                        persist_turn(&state, &ws_id, "prior_auth", "generate_justification",
                            serde_json::json!({"encounter_id": &encounter_id}),
                            serde_json::json!({"status": format!("{}", o.status)}),
                            o.confidence.clone(), &emitter).await;
                        emitter.emit(AgentEventType::AgentCompleted { confidence_score: o.confidence.score, elapsed_ms: ms });
                    }
                    Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
                }
            }
            "discharge_plan" => {
                emit_pre_llm(&emitter, "discharge_plan", "discharge_plan.generate", phase_ms).await;
                let t = std::time::Instant::now();
                let agent = cliniclaw_agents::DischargePlanAgent::new(state.llm.clone());
                let input = cliniclaw_agents::DischargePlanInput {
                    encounter_id: encounter_id.clone(),
                    encounter_status: "in-progress".into(),
                    patient_id: patient_id.clone(),
                    practitioner_id: practitioner_id.into(),
                    active_conditions: conditions.clone(),
                    current_medications: medications.clone(),
                    assessment_summary: format!("Patient managed for {}. Stable for discharge.", conditions_text),
                    capabilities: vec!["discharge_plan".into()],
                    capability_tokens: vec![],
                    practitioner_role: Some("physician".into()),
                    patient_active: true,
                    patient_deceased: None,
                    encounter_class: Some(enc_class.clone()),
                };
                match agent.generate(&input, &state.policy_engine).await {
                    Ok(mut o) => {
                        let ms = t.elapsed().as_millis() as u64;
                        emit_post_llm_allow(&emitter, ms, "allow_discharge_plan", "Discharge plan parsed", phase_ms).await;
                        let _ = state.audit_store.append(&mut o.audit_event).await;
                        emitter.emit(AgentEventType::AuditCreation { audit_event_id: o.audit_event.id.to_string() });
                        emitter.emit(AgentEventType::FhirWrite { resource_type: "DiagnosticReport".into(), resource_id: None });
                        persist_turn(&state, &ws_id, "discharge_plan", "generate",
                            serde_json::json!({"encounter_id": &encounter_id}),
                            serde_json::to_value(&o.report).unwrap_or_default(),
                            o.confidence.clone(), &emitter).await;
                        emitter.emit(AgentEventType::AgentCompleted { confidence_score: o.confidence.score, elapsed_ms: ms });
                    }
                    Err(e) => { emitter.emit(AgentEventType::AgentFailed { error: e.to_string() }); }
                }
            }
            other => {
                tracing::warn!(agent = %other, "unknown agent in dynamic pathway");
                emitter.emit(AgentEventType::AgentFailed { error: format!("unknown agent: {other}") });
            }
        }
    }

    tracing::info!(encounter_id = %encounter_id, patient = %patient_display, "dynamic pathway completed");
}
