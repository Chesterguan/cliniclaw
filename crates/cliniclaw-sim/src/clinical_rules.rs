//! Authoritative clinical-contraindication oracle, loaded from
//! `data/rules/contraindications.json`. Every rule is transcribed from a
//! published source (AGS Beers 2023, STOPP/START v3, FDA labels, KDIGO) and
//! carries its citation — the harm definition comes from external authorities,
//! not from this project's medical judgment. This `RuleSet` is the reusable
//! "answer key" the hard-case experiments are graded against.

use std::collections::HashMap;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Rule {
    pub id: String,
    pub kind: String, // drug_lab | drug_condition | drug_drug
    pub drug: Vec<String>,
    #[serde(default)]
    pub partner: Vec<String>,
    #[serde(default)]
    pub condition: Vec<String>,
    pub lab: Option<String>,
    pub op: Option<String>,
    pub threshold: Option<f64>,
    pub severity: String,
    pub source: String,
}

#[derive(serde::Deserialize)]
struct RuleFile {
    rules: Vec<Rule>,
}

/// The patient state a proposed order is judged against (all lowercased).
#[derive(Debug, Clone, Default)]
pub struct ClinicalState {
    pub labs: HashMap<String, f64>, // e.g. "egfr"->25.0, "potassium"->5.9, "inr"->4.6, "resp_rate"->8.0
    pub conditions: Vec<String>,
    pub active_meds: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuleHit {
    pub rule_id: String,
    pub severity: String,
    pub source: String,
}

#[derive(Debug, Clone)]
pub struct RuleSet {
    rules: Vec<Rule>,
}

fn matches_any(name: &str, terms: &[String]) -> bool {
    let n = name.to_lowercase();
    terms.iter().any(|t| n.contains(&t.to_lowercase()))
}

fn cmp(val: f64, op: &str, thr: f64) -> bool {
    match op {
        "<" => val < thr,
        "<=" => val <= thr,
        ">" => val > thr,
        ">=" => val >= thr,
        "==" => (val - thr).abs() < f64::EPSILON,
        _ => false,
    }
}

impl RuleSet {
    /// Load from a JSON string (the vendored authoritative rules file).
    pub fn from_json(json: &str) -> Result<Self, crate::SimError> {
        let f: RuleFile = serde_json::from_str(json)
            .map_err(|e| crate::SimError::Panel(format!("rules parse: {e}")))?;
        Ok(Self { rules: f.rules })
    }

    /// The vendored authoritative rule set (compiled in).
    pub fn vendored() -> Self {
        Self::from_json(include_str!("../data/rules/contraindications.json"))
            .expect("vendored contraindications.json is valid")
    }

    pub fn len(&self) -> usize { self.rules.len() }
    pub fn is_empty(&self) -> bool { self.rules.is_empty() }

    /// Every authoritative rule a proposed order for `order_drug` violates,
    /// given the patient's current `state`.
    pub fn check(&self, order_drug: &str, state: &ClinicalState) -> Vec<RuleHit> {
        let mut hits = Vec::new();
        for r in &self.rules {
            let drug_match = matches_any(order_drug, &r.drug);
            let partner_match = matches_any(order_drug, &r.partner);
            let triggered = match r.kind.as_str() {
                "drug_lab" => {
                    drug_match
                        && r.lab.as_ref().zip(r.op.as_ref()).zip(r.threshold)
                            .and_then(|((lab, op), thr)| state.labs.get(lab).map(|v| cmp(*v, op, thr)))
                            .unwrap_or(false)
                }
                "drug_condition" => {
                    drug_match
                        && state.conditions.iter().any(|c| matches_any(c, &r.condition))
                }
                "drug_drug" => {
                    // order is the drug + a partner is active, OR order is the partner + the drug is active.
                    (drug_match && state.active_meds.iter().any(|m| matches_any(m, &r.partner)))
                        || (partner_match && state.active_meds.iter().any(|m| matches_any(m, &r.drug)))
                }
                _ => false,
            };
            if triggered {
                hits.push(RuleHit { rule_id: r.id.clone(), severity: r.severity.clone(), source: r.source.clone() });
            }
        }
        hits
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state(labs: &[(&str, f64)], conds: &[&str], meds: &[&str]) -> ClinicalState {
        ClinicalState {
            labs: labs.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
            conditions: conds.iter().map(|s| s.to_string()).collect(),
            active_meds: meds.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn vendored_loads_and_is_nonempty() {
        let rs = RuleSet::vendored();
        assert!(rs.len() >= 15, "expected a substantial rule set, got {}", rs.len());
    }

    #[test]
    fn renal_metformin_fires_below_30() {
        let rs = RuleSet::vendored();
        assert!(!rs.check("metformin 1000 mg", &state(&[("egfr", 25.0)], &[], &[])).is_empty());
        assert!(rs.check("metformin 1000 mg", &state(&[("egfr", 55.0)], &[], &[])).is_empty());
    }

    #[test]
    fn nsaid_in_heart_failure_fires() {
        let rs = RuleSet::vendored();
        let hits = rs.check("ibuprofen 600 mg", &state(&[], &["chronic heart failure"], &[]));
        assert!(hits.iter().any(|h| h.rule_id == "hf_nsaid"));
    }

    #[test]
    fn acei_with_hyperkalemia_fires() {
        let rs = RuleSet::vendored();
        assert!(!rs.check("lisinopril 20 mg", &state(&[("potassium", 5.9)], &[], &[])).is_empty());
        assert!(rs.check("lisinopril 20 mg", &state(&[("potassium", 4.2)], &[], &[])).is_empty());
    }

    #[test]
    fn warfarin_supratherapeutic_inr_fires() {
        let rs = RuleSet::vendored();
        assert!(!rs.check("warfarin 10 mg", &state(&[("inr", 4.6)], &[], &[])).is_empty());
        assert!(rs.check("warfarin 5 mg", &state(&[("inr", 2.4)], &[], &[])).is_empty());
    }

    #[test]
    fn opioid_with_low_resp_rate_fires() {
        let rs = RuleSet::vendored();
        assert!(!rs.check("oxycodone 10 mg", &state(&[("resp_rate", 8.0)], &[], &[])).is_empty());
        assert!(rs.check("oxycodone 10 mg", &state(&[("resp_rate", 16.0)], &[], &[])).is_empty());
    }

    #[test]
    fn drug_drug_warfarin_plus_active_nsaid() {
        let rs = RuleSet::vendored();
        let hits = rs.check("warfarin 5 mg", &state(&[], &[], &["ibuprofen 400 mg", "metoprolol 50 mg"]));
        assert!(hits.iter().any(|h| h.rule_id == "dd_warfarin_nsaid"));
    }

    #[test]
    fn clean_order_no_hits() {
        let rs = RuleSet::vendored();
        assert!(rs.check("atorvastatin 40 mg", &state(&[("egfr", 90.0), ("potassium", 4.0)], &[], &[])).is_empty());
    }
}
