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

        if system.contains("SOAP note") || system.contains("clinical documentation") {
            Ok(if is_replay { mock_ambient_doc_replay() } else { mock_ambient_doc_response() })
        } else if system.contains("medication") || system.contains("order") {
            Ok(if is_replay { mock_order_entry_replay() } else { mock_order_entry_response() })
        } else if system.contains("prior authorization") || system.contains("authorization") {
            Ok(if is_replay { mock_prior_auth_replay() } else { mock_prior_auth_response() })
        } else {
            Ok(mock_ambient_doc_response())
        }
    }
}

/// Deterministic SOAP note response for ambient documentation.
fn mock_ambient_doc_response() -> String {
    serde_json::json!({
        "subjective": "Patient presents with persistent headaches for 2 weeks, worse in the morning, rated 6/10 on pain scale. Reports associated mild nausea but no visual disturbances, fever, or neck stiffness. Has been taking OTC acetaminophen with minimal relief. Denies recent head trauma. Notes increased work-related stress over the past month.",
        "objective": "BP 142/88 mmHg, HR 76 bpm, Temp 98.6°F, RR 16, SpO2 99% on RA. Alert and oriented x3. HEENT: normocephalic, atraumatic. Pupils equal, round, reactive to light. No papilledema on fundoscopic exam. Neck supple, no meningismus. Neurologic exam grossly intact — cranial nerves II-XII intact, strength 5/5 bilateral upper and lower extremities, sensation intact. DTRs 2+ throughout.",
        "assessment": "1. Essential hypertension, uncontrolled (I10)\n2. Tension-type headache, likely secondary to hypertension and stress (G44.209)",
        "plan": "1. Increase lisinopril from 10mg to 20mg PO daily\n2. Order CBC, CMP, UA to evaluate end-organ effects\n3. Headache diary for 2 weeks\n4. Stress management counseling discussed\n5. Return in 2 weeks for BP recheck and headache reassessment\n6. If headaches worsen or new neurologic symptoms, present to ED immediately",
        "icd10_codes": ["I10", "G44.209"]
    })
    .to_string()
}

/// Deterministic order parsing response.
fn mock_order_entry_response() -> String {
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
fn mock_ambient_doc_replay() -> String {
    serde_json::json!({
        "subjective": "Patient presents with persistent headaches for 2 weeks, worse in the morning, rated 6/10 on pain scale. Reports associated mild nausea but no visual disturbances, fever, or neck stiffness. Has been taking OTC acetaminophen with minimal relief. Denies recent head trauma. Notes increased work-related stress over the past month. Sleep quality has also deteriorated.",
        "objective": "BP 142/88 mmHg, HR 76 bpm, Temp 98.6°F, RR 16, SpO2 99% on RA. Alert and oriented x3. HEENT: normocephalic, atraumatic. Pupils equal, round, reactive to light. No papilledema on fundoscopic exam. Neck supple, no meningismus. Neurologic exam grossly intact — cranial nerves II-XII intact, strength 5/5 bilateral upper and lower extremities, sensation intact. DTRs 2+ throughout.",
        "assessment": "1. Essential hypertension, uncontrolled (I10)\n2. Tension-type headache, likely secondary to hypertension and stress (G44.209)\n3. Insomnia, unspecified (G47.00)",
        "plan": "1. Increase lisinopril from 10mg to 20mg PO daily\n2. Order CBC, CMP, UA to evaluate end-organ effects\n3. Headache diary for 2 weeks\n4. Stress management counseling discussed\n5. Sleep hygiene education provided\n6. Return in 2 weeks for BP recheck and headache reassessment\n7. If headaches worsen or new neurologic symptoms, present to ED immediately",
        "icd10_codes": ["I10", "G44.209", "G47.00"]
    })
    .to_string()
}

/// Replay variant for order entry.
fn mock_order_entry_replay() -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_ambient_doc() {
        let mock = MockClaudeCapability::new();
        let prompt = PromptEnvelope::build(
            "Generate a SOAP note from clinical documentation",
            "Patient presents with headaches...",
        );
        let response = mock.call(&prompt).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert!(parsed.get("subjective").is_some());
        assert!(parsed.get("objective").is_some());
        assert!(parsed.get("assessment").is_some());
        assert!(parsed.get("plan").is_some());
        assert!(parsed.get("icd10_codes").is_some());
    }

    #[tokio::test]
    async fn test_mock_order_entry() {
        let mock = MockClaudeCapability::new();
        let prompt = PromptEnvelope::build(
            "Parse this medication order",
            "metformin 500mg twice daily",
        );
        let response = mock.call(&prompt).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["medication"], "metformin");
        assert_eq!(parsed["dose"], "500mg");
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
