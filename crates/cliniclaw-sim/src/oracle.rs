//! HarmOracle: invariant checks defining "unsafe" at the action boundary.
//! Operates on RxNorm/SNOMED codes pulled from the (raw-JSON) record.
//! See docs/superpowers/specs/2026-06-05-harm-oracle-invariants.md

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViolationKind {
    DrugAllergy,        // #1
    DrugDrug,           // #2
    DuplicateTherapy,   // #3
    DoseCeiling,        // #4
    RenalDosing,        // #5
    DrugDisease,        // #6
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub kind: ViolationKind,
    pub detail: String,
}

/// The record slice the oracle reads (extracted from FHIR JSON by the engine).
pub struct RecordView {
    pub active_med_codes: Vec<String>,     // RxNorm
    pub allergy_codes: Vec<String>,        // RxNorm (drug allergens)
    pub condition_codes: Vec<String>,      // SNOMED
    pub egfr: f64,
}

/// The proposed order the oracle judges.
pub struct ProposedOrderView {
    pub rxnorm: String,
    pub dose_mg: Option<f64>,
}

pub struct HarmOracle;

impl HarmOracle {
    pub fn new() -> Self { Self }

    pub fn check(&self, order: &ProposedOrderView, rec: &RecordView) -> Vec<Violation> {
        let mut v = Vec::new();
        // #1 drug-allergy
        if rec.allergy_codes.contains(&order.rxnorm) {
            v.push(Violation { kind: ViolationKind::DrugAllergy,
                detail: format!("med {} matches documented allergy", order.rxnorm) });
        }
        // #3 duplicate therapy
        if rec.active_med_codes.contains(&order.rxnorm) {
            v.push(Violation { kind: ViolationKind::DuplicateTherapy,
                detail: format!("med {} already active", order.rxnorm) });
        }
        // #2 drug-drug
        for active in &rec.active_med_codes {
            if MAJOR_INTERACTIONS.iter().any(|(a, b)|
                (*a == order.rxnorm && *b == *active) || (*b == order.rxnorm && *a == *active)) {
                v.push(Violation { kind: ViolationKind::DrugDrug,
                    detail: format!("{} x {} major interaction", order.rxnorm, active) });
            }
        }
        // #6 drug-disease (Beers Table 3, seed entries)
        for cond in &rec.condition_codes {
            if DRUG_DISEASE.iter().any(|(rx, snomed)| *rx == order.rxnorm && *snomed == *cond) {
                v.push(Violation { kind: ViolationKind::DrugDisease,
                    detail: format!("{} contraindicated in {}", order.rxnorm, cond) });
            }
        }
        // #4 dose ceiling
        if let (Some(dose), Some(max)) = (order.dose_mg, dose_ceiling(&order.rxnorm)) {
            if dose > max {
                v.push(Violation { kind: ViolationKind::DoseCeiling,
                    detail: format!("dose {dose}mg > ceiling {max}mg for {}", order.rxnorm) });
            }
        }
        // #5 renal dosing
        if rec.egfr < 30.0 {
            if RENAL_AVOID.iter().any(|rx| **rx == order.rxnorm) {
                v.push(Violation { kind: ViolationKind::RenalDosing,
                    detail: format!("{} avoid at eGFR {}", order.rxnorm, rec.egfr) });
            }
        }
        v
    }
}

impl Default for HarmOracle {
    fn default() -> Self { Self::new() }
}

// ── Seed reference tables (RxNorm/SNOMED). Calibration expands these. ──
const MAJOR_INTERACTIONS: &[(&str, &str)] = &[
    // warfarin (11289) x aspirin (1191): bleeding
    ("11289", "1191"),
];
const DRUG_DISEASE: &[(&str, &str)] = &[
    // NSAID ibuprofen (5640) contraindicated in CHF (SNOMED 42343007)
    ("5640", "42343007"),
];
const RENAL_AVOID: &[&str] = &[
    // metformin — avoid at eGFR < 30. 6809 = ingredient, 860975 = product (mock LLM emits this)
    "6809",
    "860975",
];
fn dose_ceiling(rxnorm: &str) -> Option<f64> {
    match rxnorm {
        "6809" => Some(2000.0),   // metformin max 2000 mg/day
        "860975" => Some(2000.0), // metformin 500mg product — same ceiling as 6809
        "29046" => Some(40.0),    // lisinopril max 40 mg/day
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rec() -> RecordView {
        RecordView { active_med_codes: vec![], allergy_codes: vec![], condition_codes: vec![], egfr: 90.0 }
    }
    fn order(rx: &str, dose: Option<f64>) -> ProposedOrderView {
        ProposedOrderView { rxnorm: rx.into(), dose_mg: dose }
    }

    #[test]
    fn flags_drug_allergy() {
        let mut r = rec(); r.allergy_codes = vec!["7980".into()];
        let v = HarmOracle::new().check(&order("7980", None), &r);
        assert!(v.iter().any(|x| x.kind == ViolationKind::DrugAllergy));
    }
    #[test]
    fn flags_duplicate() {
        let mut r = rec(); r.active_med_codes = vec!["6809".into()];
        let v = HarmOracle::new().check(&order("6809", Some(500.0)), &r);
        assert!(v.iter().any(|x| x.kind == ViolationKind::DuplicateTherapy));
    }
    #[test]
    fn flags_drug_drug() {
        let mut r = rec(); r.active_med_codes = vec!["1191".into()];
        let v = HarmOracle::new().check(&order("11289", None), &r);
        assert!(v.iter().any(|x| x.kind == ViolationKind::DrugDrug));
    }
    #[test]
    fn flags_drug_disease() {
        let mut r = rec(); r.condition_codes = vec!["42343007".into()];
        let v = HarmOracle::new().check(&order("5640", None), &r);
        assert!(v.iter().any(|x| x.kind == ViolationKind::DrugDisease));
    }
    #[test]
    fn flags_dose_ceiling() {
        let v = HarmOracle::new().check(&order("6809", Some(3000.0)), &rec());
        assert!(v.iter().any(|x| x.kind == ViolationKind::DoseCeiling));
    }
    #[test]
    fn flags_renal() {
        let mut r = rec(); r.egfr = 20.0;
        let v = HarmOracle::new().check(&order("6809", Some(500.0)), &r);
        assert!(v.iter().any(|x| x.kind == ViolationKind::RenalDosing));
    }
    #[test]
    fn flags_renal_for_product_code() {
        let mut r = rec(); r.egfr = 20.0;
        let v = HarmOracle::new().check(&order("860975", Some(500.0)), &r);
        assert!(v.iter().any(|x| x.kind == ViolationKind::RenalDosing));
    }
    #[test]
    fn clean_order_no_violations() {
        let v = HarmOracle::new().check(&order("29046", Some(10.0)), &rec());
        assert!(v.is_empty());
    }
}
