use async_trait::async_trait;

use crate::claude::PromptEnvelope;
use crate::error::AgentError;
use crate::llm::LlmCapability;

/// Deterministic mock LLM for demo/test mode.
///
/// Returns clinically realistic fixture responses based on
/// the prompt system instruction content, without making any API calls.
#[derive(Debug, Clone)]
pub struct MockClaudeCapability;

impl MockClaudeCapability {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MockClaudeCapability {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LlmCapability for MockClaudeCapability {
    async fn call(&self, prompt: &PromptEnvelope) -> Result<String, AgentError> {
        tracing::info!(
            system_len = prompt.system().len(),
            user_len = prompt.user().len(),
            "MockClaudeCapability: returning deterministic response"
        );

        // Route to appropriate fixture based on prompt content
        let system = prompt.system();
        let user = prompt.user();
        let is_replay = user.contains("__replay__");

        if system.contains("triage") || system.contains("Emergency Severity Index") {
            Ok(mock_triage_response(user))
        } else if system.contains("nursing assessment") || system.contains("Nurse") {
            Ok(mock_nurse_assess_response())
        } else if system.contains("lab review") || system.contains("Interpret") || system.contains("interpret") {
            Ok(mock_lab_review_response(user))
        } else if system.contains("discharge") || system.contains("Discharge") {
            Ok(mock_discharge_plan_response())
        } else if system.contains("pharmacy") || system.contains("Pharmacy") || system.contains("medication review") {
            Ok(mock_pharmacy_review_response(user))
        } else if system.contains("SOAP note") || system.contains("clinical documentation") {
            Ok(if is_replay { mock_ambient_doc_replay(user) } else { mock_ambient_doc_response(user) })
        } else if system.contains("medication") || system.contains("order") {
            Ok(if is_replay { mock_order_entry_replay(user) } else { mock_order_entry_response(user) })
        } else if system.contains("prior authorization") || system.contains("authorization") {
            Ok(if is_replay { mock_prior_auth_replay() } else { mock_prior_auth_response() })
        } else {
            Ok(mock_ambient_doc_response(user))
        }
    }
}

/// Deterministic SOAP note response for ambient documentation.
/// Routes based on transcript content to produce clinically appropriate notes.
fn mock_ambient_doc_response(user: &str) -> String {
    let u = user.to_lowercase();

    if u.contains("diabetes") || u.contains("t2dm") || u.contains("hba1c") || u.contains("glycemic")
        || u.contains("metformin") || u.contains("glipizide") || u.contains("polydipsia")
    {
        mock_ambient_doc_diabetes()
    } else if u.contains("prenatal") || u.contains("pregnancy") || u.contains("gestation")
        || u.contains("fundal") || u.contains("fetal")
    {
        mock_ambient_doc_prenatal()
    } else if u.contains("copd") || u.contains("dyspnea") || u.contains("bronch")
        || u.contains("spo2 88") || u.contains("respiratory acidosis")
    {
        mock_ambient_doc_copd()
    } else if u.contains("knee") || u.contains("arthroplasty") || u.contains("osteoarthritis")
        || u.contains("joint space") || u.contains("kellgren")
    {
        mock_ambient_doc_knee_oa()
    } else if u.contains("chf") || u.contains("heart failure") || u.contains("edema")
        || u.contains("bnp") || u.contains("warfarin") || u.contains("atrial fibrillation")
    {
        mock_ambient_doc_chf()
    } else {
        // Default: hypertension/headache
        mock_ambient_doc_htn()
    }
}

fn mock_ambient_doc_htn() -> String {
    serde_json::json!({
        "subjective": "Patient presents with persistent headaches for 2 weeks, worse in the morning, rated 6/10 on pain scale. Reports associated mild nausea but no visual disturbances, fever, or neck stiffness. Has been taking OTC acetaminophen with minimal relief. Denies recent head trauma. Notes increased work-related stress over the past month.",
        "objective": "BP 142/88 mmHg, HR 76 bpm, Temp 98.6°F, RR 16, SpO2 99% on RA. Alert and oriented x3. HEENT: normocephalic, atraumatic. Pupils equal, round, reactive to light. No papilledema on fundoscopic exam. Neck supple, no meningismus. Neurologic exam grossly intact — cranial nerves II-XII intact, strength 5/5 bilateral upper and lower extremities, sensation intact. DTRs 2+ throughout.",
        "assessment": "1. Essential hypertension, uncontrolled (I10)\n2. Tension-type headache, likely secondary to hypertension and stress (G44.209)",
        "plan": "1. Increase lisinopril from 10mg to 20mg PO daily\n2. Order CBC, CMP, UA to evaluate end-organ effects\n3. Headache diary for 2 weeks\n4. Stress management counseling discussed\n5. Return in 2 weeks for BP recheck and headache reassessment\n6. If headaches worsen or new neurologic symptoms, present to ED immediately",
        "icd10_codes": ["I10", "G44.209"]
    })
    .to_string()
}

fn mock_ambient_doc_diabetes() -> String {
    serde_json::json!({
        "subjective": "Patient with type 2 diabetes presents for routine follow-up. Reports increased thirst and polyuria over the past 2 weeks. Has been compliant with current oral hypoglycemic regimen. Denies hypoglycemic episodes, blurred vision, or extremity numbness/tingling. Diet adherence has been suboptimal — reports difficulty with carbohydrate counting.",
        "objective": "BP 128/82 mmHg, HR 80 bpm, Temp 98.4°F, RR 14, SpO2 99% on RA. BMI 31.2. Alert and oriented x3. Skin warm, dry, intact. No acanthosis nigricans. Peripheral pulses 2+ bilateral dorsalis pedis and posterior tibial. Monofilament testing intact bilateral. HbA1c 8.2%, fasting glucose 186 mg/dL. CMP within normal limits. Creatinine 0.9 mg/dL, eGFR >60.",
        "assessment": "1. Type 2 diabetes mellitus, inadequately controlled (E11.65)\n2. Polydipsia, likely secondary to hyperglycemia (R63.1)",
        "plan": "1. Start metformin 500mg PO BID with meals, increase to 1000mg BID after 2 weeks if tolerated\n2. Continue glipizide 5mg daily\n3. Diabetic diet counseling and referral to nutritionist\n4. Recheck HbA1c in 3 months\n5. Diabetic foot exam completed — no neuropathy detected\n6. Annual ophthalmology referral for diabetic retinopathy screening\n7. Return in 6 weeks for fasting glucose and medication tolerance assessment",
        "icd10_codes": ["E11.65", "R63.1"]
    })
    .to_string()
}

fn mock_ambient_doc_prenatal() -> String {
    serde_json::json!({
        "subjective": "Patient presents for routine prenatal visit at 28 weeks gestation. Reports no complaints — no vaginal bleeding, leaking of fluid, contractions, or decreased fetal movement. Fetal movements felt regularly, perceived as normal. Sleep mildly disrupted due to positional discomfort. No headache, visual changes, or epigastric pain. Diet and prenatal vitamin compliance confirmed.",
        "objective": "BP 118/72 mmHg, HR 82 bpm, Temp 98.6°F, RR 16, SpO2 100% on RA. Weight 158 lbs (pre-pregnancy weight 142 lbs, appropriate gain). Fundal height 28 cm — appropriate for gestational age. Fetal heart tones 148 bpm via Doppler. Abdomen soft, non-tender. No edema in extremities. GBS culture collected, result pending. Blood type A+, Rh positive — no Rhogam indicated.",
        "assessment": "1. Normal intrauterine pregnancy, 28 weeks gestation (Z34.08)\n2. Routine prenatal supervision (Z34.83)",
        "plan": "1. Continue prenatal vitamins and iron supplementation\n2. Review GBS culture results when available\n3. Reviewed warning signs: bleeding, contractions <37 weeks, decreased fetal movement, severe headache, visual changes\n4. Discussed birth plan preferences — patient prefers vaginal delivery\n5. Glucose tolerance test completed at 24 weeks — normal\n6. Return in 2 weeks for 30-week prenatal visit",
        "icd10_codes": ["Z34.08", "Z34.83"]
    })
    .to_string()
}

fn mock_ambient_doc_copd() -> String {
    serde_json::json!({
        "subjective": "Patient with known COPD (GOLD stage III) presents with worsening dyspnea over the past 3 days. Reports increased sputum production — yellow-green in color. Unable to complete sentences without pausing for breath. Increased rescue inhaler use from 2-3 times weekly to 6-8 times daily. Denies fever, chest pain, or hemoptysis. History of CAD, on aspirin and statin.",
        "objective": "BP 138/85 mmHg, HR 98 bpm, Temp 99.1°F, RR 24, SpO2 88% on room air (improved to 93% on 2L NC). Alert, in moderate respiratory distress. Using accessory muscles of respiration. Diffuse expiratory wheezing bilateral. Diminished breath sounds at bases. No peripheral edema. ABG: pH 7.32, pCO2 52 mmHg, pO2 58 mmHg, HCO3 26 — acute on chronic respiratory acidosis. WBC 14.2 x10^9/L. CXR: hyperinflation, no infiltrate.",
        "assessment": "1. Acute exacerbation of COPD (J44.1)\n2. Respiratory acidosis, acute on chronic (J96.01)\n3. Leukocytosis — possible infectious trigger (D72.829)",
        "plan": "1. Oxygen therapy via nasal cannula — titrate to SpO2 88-92%\n2. Prednisone 40mg PO daily x 5 days\n3. Albuterol nebulizer 2.5mg Q4H\n4. Continue tiotropium and fluticasone\n5. Sputum culture obtained\n6. If no improvement in 24 hours, consider azithromycin for infectious exacerbation\n7. Repeat ABG in 4 hours to assess response\n8. Pulmonology consult if respiratory status does not improve",
        "icd10_codes": ["J44.1", "J96.01", "D72.829"]
    })
    .to_string()
}

fn mock_ambient_doc_knee_oa() -> String {
    serde_json::json!({
        "subjective": "Patient presents for pre-surgical evaluation for bilateral total knee arthroplasty. Reports progressive bilateral knee pain for 5 years, rated 7-8/10 with weight-bearing. Unable to walk more than one block. Difficulty with stairs and rising from seated position. Significant impact on ADLs and quality of life. Has completed 12 sessions of physical therapy without lasting improvement. NSAIDs (naproxen 500mg BID) for 4 months provided only partial relief. Two cortisone injections per knee — temporary relief only.",
        "objective": "BP 130/78 mmHg, HR 72 bpm, Temp 98.6°F, RR 16, SpO2 99% on RA. BMI 28.2. Bilateral knee exam: moderate effusion bilateral, crepitus with range of motion, flexion limited to 100° bilateral. Varus alignment bilateral. No ligamentous instability. Distal pulses intact. X-ray bilateral knees: Kellgren-Lawrence grade 4 bilateral with bone-on-bone contact in medial compartments, severe joint space narrowing, marginal osteophytes, subchondral sclerosis.",
        "assessment": "1. Severe bilateral knee osteoarthritis, Kellgren-Lawrence grade 4 (M17.0)\n2. Limited mobility secondary to bilateral knee OA (M25.561)",
        "plan": "1. Patient cleared for bilateral total knee arthroplasty — staged approach recommended\n2. Submit prior authorization to insurance carrier\n3. Pre-operative labs: CBC, CMP, PT/INR, type and screen\n4. Pre-operative cardiology clearance obtained\n5. Discontinue naproxen 7 days prior to surgery\n6. Physical therapy to begin post-operatively — home health PT ordered\n7. Patient educated on surgical risks, recovery timeline, and rehabilitation expectations",
        "icd10_codes": ["M17.0", "M25.561"]
    })
    .to_string()
}

fn mock_ambient_doc_chf() -> String {
    serde_json::json!({
        "subjective": "Patient with known congestive heart failure (EF 35%) and atrial fibrillation on warfarin presents with 5 lb weight gain over 3 days, worsening bilateral ankle edema, and dyspnea on exertion (unable to walk to mailbox without stopping). Increased pillow use — now sleeping on 3 pillows. Reports dietary indiscretion — high sodium meals over the holiday weekend. Denies chest pain, palpitations, or syncope. Medication compliant.",
        "objective": "BP 148/92 mmHg, HR 88 bpm irregularly irregular, Temp 98.4°F, RR 20, SpO2 94% on RA. Weight 215 lbs (baseline 210 lbs). JVP elevated at 10 cm. Bilateral crackles at lung bases. Heart: irregularly irregular, S3 gallop present, no murmur. Abdomen: soft, hepatomegaly by percussion. 2+ bilateral pitting edema to mid-shin. BNP 890 pg/mL (baseline 200). INR 2.8 (therapeutic range 2.0-3.0). Creatinine 1.8 mg/dL (baseline 1.4).",
        "assessment": "1. Acute decompensated congestive heart failure (I50.21)\n2. Atrial fibrillation, chronic (I48.2)\n3. INR supratherapeutic — warfarin dose adjustment needed (T45.515A)\n4. Acute kidney injury — likely cardiorenal (N17.9)",
        "plan": "1. IV furosemide 40mg now, then reassess — target 1-2L net negative per day\n2. Strict I&O monitoring, daily weights\n3. Fluid restriction 1.5L/day, sodium restriction <2g/day\n4. Adjust warfarin from 5mg to 7.5mg PO daily — recheck INR in 3 days\n5. Continue carvedilol 25mg BID\n6. Echocardiogram ordered to assess current EF and wall motion\n7. Renal panel in AM — hold ACE inhibitor if creatinine continues to rise\n8. Cardiology consult for heart failure optimization",
        "icd10_codes": ["I50.21", "I48.2", "T45.515A", "N17.9"]
    })
    .to_string()
}

/// Deterministic order parsing response — adapts based on order text content.
fn mock_order_entry_response(user: &str) -> String {
    let u = user.to_lowercase();

    if u.contains("lisinopril") {
        let dose = if u.contains("20mg") || u.contains("20 mg") { "20mg" } else if u.contains("10mg") { "10mg" } else { "20mg" };
        serde_json::json!({
            "medication": "lisinopril",
            "dose": dose,
            "route": "oral",
            "frequency": "daily",
            "indication": "Essential hypertension",
            "icd10": "I10",
            "rxnorm": "314076",
            "instructions": "Take once daily in the morning"
        })
    } else if u.contains("prednisone") {
        let dose = if u.contains("40mg") || u.contains("40 mg") { "40mg" } else { "40mg" };
        serde_json::json!({
            "medication": "prednisone",
            "dose": dose,
            "route": "oral",
            "frequency": "daily",
            "indication": "Acute COPD exacerbation",
            "icd10": "J44.1",
            "rxnorm": "763179",
            "instructions": "Take with food for 5 days, then taper as directed"
        })
    } else if u.contains("albuterol") {
        let dose = if u.contains("2.5mg") || u.contains("2.5 mg") { "2.5mg" } else { "2.5mg" };
        let freq = if u.contains("q4h") || u.contains("q4") { "Q4H" } else { "Q4H" };
        serde_json::json!({
            "medication": "albuterol",
            "dose": dose,
            "route": "inhalation",
            "frequency": freq,
            "indication": "Acute bronchospasm, COPD exacerbation",
            "icd10": "J44.1",
            "rxnorm": "245314",
            "instructions": "Administer via nebulizer every 4 hours as needed for dyspnea"
        })
    } else if u.contains("warfarin") {
        let dose = if u.contains("7.5mg") || u.contains("7.5 mg") { "7.5mg" } else if u.contains("5mg") { "5mg" } else { "7.5mg" };
        serde_json::json!({
            "medication": "warfarin",
            "dose": dose,
            "route": "oral",
            "frequency": "daily",
            "indication": "Atrial fibrillation, anticoagulation",
            "icd10": "I48.2",
            "rxnorm": "855288",
            "instructions": "Take at the same time each evening. Monitor INR in 3 days."
        })
    } else if u.contains("furosemide") || u.contains("lasix") {
        let dose = if u.contains("40mg") || u.contains("40 mg") { "40mg" } else { "40mg" };
        serde_json::json!({
            "medication": "furosemide",
            "dose": dose,
            "route": "iv",
            "frequency": "once",
            "indication": "Acute decompensated heart failure",
            "icd10": "I50.21",
            "rxnorm": "310429",
            "instructions": "Administer IV push now. Reassess urine output and volume status in 2 hours."
        })
    } else if u.contains("metformin") {
        let dose = if u.contains("1000mg") || u.contains("1000 mg") || u.contains("1g") { "1000mg" } else { "500mg" };
        let freq = if u.contains("daily") || u.contains("qd") { "daily" } else { "BID" };
        serde_json::json!({
            "medication": "metformin",
            "dose": dose,
            "route": "oral",
            "frequency": freq,
            "indication": "Type 2 diabetes mellitus",
            "icd10": "E11.9",
            "rxnorm": "860975",
            "instructions": "Take with meals to reduce GI side effects"
        })
    } else {
        // Default: try to extract medication name from the order text
        serde_json::json!({
            "medication": "metformin",
            "dose": "500mg",
            "route": "oral",
            "frequency": "BID",
            "indication": "Type 2 diabetes mellitus",
            "icd10": "E11.9",
            "rxnorm": "860975",
            "instructions": "Take with meals"
        })
    }
    .to_string()
}

/// Deterministic prior auth clinical justification response.
fn mock_prior_auth_response() -> String {
    serde_json::json!({
        "diagnosis_summary": "Severe bilateral knee osteoarthritis (M17.0) with Kellgren-Lawrence grade 4 changes bilaterally. Patient has failed 6 months of conservative management.",
        "clinical_justification": "Patient has exhausted conservative treatment options including 12 weeks of physical therapy, NSAIDs (naproxen 500mg BID for 4 months), and two intra-articular corticosteroid injections per knee. Functional status has deteriorated significantly — unable to walk more than one block, difficulty with stairs, impaired ADLs. BMI 28.2 (within surgical range). No contraindications to surgery.",
        "supporting_evidence": [
            "X-ray bilateral knees (2026-01-15): Kellgren-Lawrence grade 4 bilateral, bone-on-bone medial compartment",
            "Physical therapy discharge summary (2025-12-01): Goals not met after 12 sessions, continued functional decline",
            "Orthopedic consultation (2026-01-20): Recommends bilateral TKR, staged approach"
        ],
        "urgency": "routine",
        "cpt_codes": ["27447"],
        "icd10_codes": ["M17.0", "M25.561"]
    })
    .to_string()
}

/// Replay variant: slightly different SOAP note to produce diff in demo mode.
/// Adds a minor additional finding or recommendation to show clinician review value.
fn mock_ambient_doc_replay(user: &str) -> String {
    let u = user.to_lowercase();

    // For replay, use the same routing but add slight clinical refinements
    if u.contains("diabetes") || u.contains("t2dm") || u.contains("hba1c") || u.contains("glycemic") {
        serde_json::json!({
            "subjective": "Patient with type 2 diabetes presents for routine follow-up. Reports increased thirst and polyuria over the past 2 weeks. Has been compliant with current oral hypoglycemic regimen. Denies hypoglycemic episodes, blurred vision, or extremity numbness/tingling. Diet adherence has been suboptimal — reports difficulty with carbohydrate counting. Also notes mild fatigue.",
            "objective": "BP 128/82 mmHg, HR 80 bpm, Temp 98.4°F, RR 14, SpO2 99% on RA. BMI 31.2. Alert and oriented x3. Skin warm, dry, intact. No acanthosis nigricans. Peripheral pulses 2+ bilateral dorsalis pedis and posterior tibial. Monofilament testing intact bilateral. HbA1c 8.2%, fasting glucose 186 mg/dL. CMP within normal limits. Creatinine 0.9 mg/dL, eGFR >60.",
            "assessment": "1. Type 2 diabetes mellitus, inadequately controlled (E11.65)\n2. Polydipsia, likely secondary to hyperglycemia (R63.1)\n3. Fatigue, likely related to hyperglycemia (R53.83)",
            "plan": "1. Start metformin 500mg PO BID with meals, increase to 1000mg BID after 2 weeks if tolerated\n2. Continue glipizide 5mg daily\n3. Diabetic diet counseling and referral to nutritionist\n4. Recheck HbA1c in 3 months\n5. Diabetic foot exam completed — no neuropathy detected\n6. Annual ophthalmology referral for diabetic retinopathy screening\n7. Check vitamin B12 level at next visit (metformin can cause deficiency)\n8. Return in 6 weeks for fasting glucose and medication tolerance assessment",
            "icd10_codes": ["E11.65", "R63.1", "R53.83"]
        })
    } else {
        serde_json::json!({
            "subjective": "Patient presents with persistent headaches for 2 weeks, worse in the morning, rated 6/10 on pain scale. Reports associated mild nausea but no visual disturbances, fever, or neck stiffness. Has been taking OTC acetaminophen with minimal relief. Denies recent head trauma. Notes increased work-related stress over the past month. Sleep quality has also deteriorated.",
            "objective": "BP 142/88 mmHg, HR 76 bpm, Temp 98.6°F, RR 16, SpO2 99% on RA. Alert and oriented x3. HEENT: normocephalic, atraumatic. Pupils equal, round, reactive to light. No papilledema on fundoscopic exam. Neck supple, no meningismus. Neurologic exam grossly intact — cranial nerves II-XII intact, strength 5/5 bilateral upper and lower extremities, sensation intact. DTRs 2+ throughout.",
            "assessment": "1. Essential hypertension, uncontrolled (I10)\n2. Tension-type headache, likely secondary to hypertension and stress (G44.209)\n3. Insomnia, unspecified (G47.00)",
            "plan": "1. Increase lisinopril from 10mg to 20mg PO daily\n2. Order CBC, CMP, UA to evaluate end-organ effects\n3. Headache diary for 2 weeks\n4. Stress management counseling discussed\n5. Sleep hygiene education provided\n6. Return in 2 weeks for BP recheck and headache reassessment\n7. If headaches worsen or new neurologic symptoms, present to ED immediately",
            "icd10_codes": ["I10", "G44.209", "G47.00"]
        })
    }
    .to_string()
}

/// Replay variant for order entry — same routing with minor refinement.
fn mock_order_entry_replay(user: &str) -> String {
    let u = user.to_lowercase();

    if u.contains("lisinopril") {
        serde_json::json!({
            "medication": "lisinopril",
            "dose": "20mg",
            "route": "oral",
            "frequency": "daily",
            "indication": "Essential hypertension, uncontrolled",
            "icd10": "I10",
            "rxnorm": "314076",
            "instructions": "Take once daily in the morning. Recheck BMP in 1 week for potassium/creatinine."
        })
    } else if u.contains("prednisone") {
        serde_json::json!({
            "medication": "prednisone",
            "dose": "40mg",
            "route": "oral",
            "frequency": "daily",
            "indication": "Acute COPD exacerbation",
            "icd10": "J44.1",
            "rxnorm": "763179",
            "instructions": "Take with food for 5 days. Monitor blood glucose — may cause hyperglycemia."
        })
    } else if u.contains("warfarin") {
        serde_json::json!({
            "medication": "warfarin",
            "dose": "7.5mg",
            "route": "oral",
            "frequency": "daily",
            "indication": "Atrial fibrillation anticoagulation, dose adjustment",
            "icd10": "I48.2",
            "rxnorm": "855288",
            "instructions": "Take at the same time each evening. Recheck INR in 3 days. Maintain INR 2.0-3.0."
        })
    } else {
        serde_json::json!({
            "medication": "metformin",
            "dose": "500mg",
            "route": "oral",
            "frequency": "BID",
            "indication": "Type 2 diabetes mellitus",
            "icd10": "E11.65",
            "rxnorm": "860975",
            "instructions": "Take with meals, increase to 1000mg BID after 2 weeks if tolerated"
        })
    }
    .to_string()
}

/// Replay variant for prior auth.
fn mock_prior_auth_replay() -> String {
    serde_json::json!({
        "diagnosis_summary": "Severe bilateral knee osteoarthritis (M17.0) with Kellgren-Lawrence grade 4 changes bilaterally. Patient has failed 6 months of conservative management including physical therapy and pharmacotherapy.",
        "clinical_justification": "Patient has exhausted conservative treatment options including 12 weeks of physical therapy, NSAIDs (naproxen 500mg BID for 4 months), and two intra-articular corticosteroid injections per knee. Functional status has deteriorated significantly — unable to walk more than one block, difficulty with stairs, impaired ADLs. BMI 28.2 (within surgical range). No contraindications to surgery. Patient has also failed viscosupplementation injections.",
        "supporting_evidence": [
            "X-ray bilateral knees (2026-01-15): Kellgren-Lawrence grade 4 bilateral, bone-on-bone medial compartment",
            "Physical therapy discharge summary (2025-12-01): Goals not met after 12 sessions, continued functional decline",
            "Orthopedic consultation (2026-01-20): Recommends bilateral TKR, staged approach",
            "MRI bilateral knees (2026-01-18): Confirms complete cartilage loss medial compartments"
        ],
        "urgency": "routine",
        "cpt_codes": ["27447"],
        "icd10_codes": ["M17.0", "M25.561", "M79.3"]
    })
    .to_string()
}

// ── New agent mock responses for hospital simulation ────────────────────────

/// Deterministic triage response — ESI level based on encounter context.
fn mock_triage_response(user: &str) -> String {
    let user_lower = user.to_lowercase();
    // Inpatient/emergency → ESI 2; otherwise ESI 3
    let (esi, acuity) = if user_lower.contains("copd") || user_lower.contains("chest pain")
        || user_lower.contains("shortness of breath") || user_lower.contains("imp")
    {
        (2, "emergent")
    } else {
        (3, "urgent")
    };

    serde_json::json!({
        "triage_level": esi,
        "acuity": acuity,
        "recommended_actions": [
            "Continuous pulse oximetry",
            "Establish IV access",
            "Obtain baseline labs (CBC, CMP, BNP)"
        ]
    })
    .to_string()
}

/// Deterministic nurse assessment response.
/// Keys match NurseAssessAgent parser: "assessment", "fall_risk", "pain_score", "braden_score"
fn mock_nurse_assess_response() -> String {
    serde_json::json!({
        "assessment": "Patient alert and oriented x3. Skin warm, dry, intact. Lungs clear to auscultation bilaterally. Heart regular rate and rhythm. Abdomen soft, non-tender. Peripheral pulses 2+ bilaterally. No edema noted.",
        "fall_risk": 3,
        "pain_score": 4,
        "braden_score": 18
    })
    .to_string()
}

/// Deterministic lab review response — flags based on conditions in context.
fn mock_lab_review_response(user: &str) -> String {
    let user_lower = user.to_lowercase();
    let mut flags = Vec::<&str>::new();
    let mut interpretation_parts = vec![
        "CBC within normal limits. CMP reviewed.",
    ];

    if user_lower.contains("diabetes") || user_lower.contains("t2dm") || user_lower.contains("e11") {
        flags.push("HbA1c 8.2% — above target (goal <7%)");
        flags.push("Fasting glucose 186 mg/dL — elevated");
        interpretation_parts.push("HbA1c elevated at 8.2%, indicating suboptimal glycemic control over past 3 months. Fasting glucose also elevated. Recommend medication adjustment and dietary counseling.");
    }

    if user_lower.contains("chf") || user_lower.contains("heart failure") || user_lower.contains("i50") {
        flags.push("BNP 890 pg/mL — elevated (normal <100)");
        flags.push("Creatinine 1.8 mg/dL — mildly elevated");
        interpretation_parts.push("BNP significantly elevated consistent with decompensated heart failure. Creatinine mildly elevated — monitor renal function with diuretic therapy.");
    }

    if user_lower.contains("copd") || user_lower.contains("j44") {
        flags.push("WBC 14.2 x10^9/L — elevated");
        flags.push("ABG: pH 7.32, pCO2 52 — respiratory acidosis");
        interpretation_parts.push("Leukocytosis suggests infectious exacerbation. ABG shows compensated respiratory acidosis consistent with COPD exacerbation.");
    }

    if flags.is_empty() {
        flags.push("All values within normal limits");
        interpretation_parts.push("No significant abnormalities detected.");
    }

    serde_json::json!({
        "interpretation": interpretation_parts.join(" "),
        "flags": flags,
        "follow_up": ["Repeat labs in 48 hours if clinically indicated", "Monitor trends"]
    })
    .to_string()
}

/// Deterministic discharge plan response.
/// Keys match DischargePlanAgent parser: "instructions", "follow_up", "med_reconciliation"
fn mock_discharge_plan_response() -> String {
    serde_json::json!({
        "instructions": "Patient clinically stable for discharge. All active medical issues addressed during this encounter. Medications reconciled — see updated medication list. Patient educated on warning signs requiring immediate medical attention. Follow-up appointments scheduled with primary care and relevant specialists. Take all medications as prescribed. Return to ED if symptoms worsen or new symptoms develop. Keep all follow-up appointments.",
        "follow_up": [
            "Primary care follow-up in 2 weeks",
            "Specialist follow-up in 4 weeks if indicated",
            "Lab work 1 week prior to follow-up visit"
        ],
        "med_reconciliation": [
            "Continue current medications as prescribed",
            "New medications added during encounter — see updated list",
            "Patient counseled on medication adherence"
        ]
    })
    .to_string()
}

/// Deterministic pharmacy review response — flags based on medication context.
/// Keys match PharmacyReviewAgent parser: "status" (string), "interactions" (string array), "substitutions" (string array)
fn mock_pharmacy_review_response(user: &str) -> String {
    let user_lower = user.to_lowercase();
    let mut interactions = Vec::<String>::new();
    let mut substitutions = Vec::<String>::new();
    let mut status = "approved";

    if user_lower.contains("warfarin") && (user_lower.contains("naproxen") || user_lower.contains("nsaid")) {
        interactions.push("Warfarin + Naproxen: concurrent use significantly increases bleeding risk".to_string());
        substitutions.push("Consider acetaminophen as alternative analgesic or add PPI for gastroprotection".to_string());
        status = "hold";
    }

    if user_lower.contains("penicillin") && user_lower.contains("allerg") {
        interactions.push("Penicillin allergy — amoxicillin is contraindicated (penicillin-class)".to_string());
        substitutions.push("Consider azithromycin or fluoroquinolone as alternative".to_string());
        status = "hold";
    }

    // Polypharmacy check for elderly patients
    if user_lower.contains("carvedilol") && user_lower.contains("furosemide") && user_lower.contains("warfarin") {
        interactions.push("Complex polypharmacy: carvedilol + furosemide + warfarin — risk of hypotension, electrolyte imbalance, and bleeding".to_string());
        if status == "approved" { status = "flagged"; }
    }

    serde_json::json!({
        "status": status,
        "interactions": interactions,
        "substitutions": substitutions
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_ambient_doc_htn() {
        let mock = MockClaudeCapability::new();
        let prompt = PromptEnvelope::build(
            "Generate a SOAP note from clinical documentation",
            "Patient presents with headaches and elevated blood pressure...",
        );
        let response = mock.call(&prompt).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert!(parsed.get("subjective").is_some());
        assert!(parsed.get("objective").is_some());
        assert!(parsed.get("assessment").is_some());
        assert!(parsed.get("plan").is_some());
        assert!(parsed.get("icd10_codes").is_some());
        assert!(parsed["assessment"].as_str().unwrap().contains("hypertension"));
    }

    #[tokio::test]
    async fn test_mock_ambient_doc_diabetes() {
        let mock = MockClaudeCapability::new();
        let prompt = PromptEnvelope::build(
            "Generate a SOAP note from clinical documentation",
            "Patient with type 2 diabetes, HbA1c 8.2%, increased thirst",
        );
        let response = mock.call(&prompt).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert!(parsed["assessment"].as_str().unwrap().contains("diabetes"));
        assert!(parsed["icd10_codes"].as_array().unwrap().iter().any(|c| c.as_str().unwrap().starts_with("E11")));
    }

    #[tokio::test]
    async fn test_mock_ambient_doc_copd() {
        let mock = MockClaudeCapability::new();
        let prompt = PromptEnvelope::build(
            "Generate a SOAP note from clinical documentation",
            "Patient with COPD exacerbation, worsening dyspnea, SpO2 88%",
        );
        let response = mock.call(&prompt).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert!(parsed["assessment"].as_str().unwrap().contains("COPD"));
    }

    #[tokio::test]
    async fn test_mock_order_entry_metformin() {
        let mock = MockClaudeCapability::new();
        let prompt = PromptEnvelope::build(
            "Parse this medication order",
            "Order: start metformin 500mg BID",
        );
        let response = mock.call(&prompt).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["medication"], "metformin");
        assert_eq!(parsed["dose"], "500mg");
    }

    #[tokio::test]
    async fn test_mock_order_entry_lisinopril() {
        let mock = MockClaudeCapability::new();
        let prompt = PromptEnvelope::build(
            "Parse this medication order",
            "Order: increase lisinopril to 20mg PO daily",
        );
        let response = mock.call(&prompt).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["medication"], "lisinopril");
        assert_eq!(parsed["dose"], "20mg");
        assert_eq!(parsed["icd10"], "I10");
    }

    #[tokio::test]
    async fn test_mock_order_entry_albuterol() {
        let mock = MockClaudeCapability::new();
        let prompt = PromptEnvelope::build(
            "Parse this medication order",
            "Order: albuterol nebulizer 2.5mg Q4H",
        );
        let response = mock.call(&prompt).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["medication"], "albuterol");
        assert_eq!(parsed["route"], "inhalation");
    }

    #[tokio::test]
    async fn test_mock_prior_auth() {
        let mock = MockClaudeCapability::new();
        let prompt = PromptEnvelope::build(
            "Generate prior authorization justification",
            "TKR for bilateral knee OA",
        );
        let response = mock.call(&prompt).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert!(parsed.get("diagnosis_summary").is_some());
        assert!(parsed.get("clinical_justification").is_some());
    }
}
