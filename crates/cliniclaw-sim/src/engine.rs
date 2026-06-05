//! The season loop. One arm per call; deterministic under ArmConfig.seed.
//!
//! Validity design (why the gap is real, not tautological):
//! - DRIFT CAUSES THE UNSAFE ORDER. Copy-forward substitutes a carried home med
//!   with a high-alert drug (warfarin) at an overdose — the propagated
//!   documentation error. The order under test is the re-prescription of the
//!   (possibly corrupted) carried med list, so unsafe orders are produced BY
//!   drift and their rate scales with surge.
//! - THE GATE IS INDEPENDENT OF THE ORACLE. VERITAS keys on a governance signal
//!   (high-alert drug class) set from the proposed drug; the HarmOracle keys on
//!   clinical predicates (dose ceiling, drug-drug, renal, allergy). A gate catch
//!   is therefore genuine and non-circular — and VERITAS only catches the
//!   governance-relevant subset (H2: necessary, not sufficient).

use rand::rngs::StdRng;
use rand::SeedableRng;

use cliniclaw_policy::{ActionContext, PolicyEngine};

use crate::arm::{ArmConfig, ArmMode};
use crate::copyforward::{CarriedItem, CopyForwardChannel};
use crate::epi::EpiDriver;
use crate::gate::VeritasGate;
use crate::metrics::{MetricsLog, WeeklySnapshot};
use crate::oracle::{HarmOracle, ProposedOrderView, RecordView};
use crate::panel::{CodeRef, PanelPatient, PatientPanel};
use crate::patient_state::PatientState;
use crate::SimError;

/// ISMP-style high-alert drug classes. The VERITAS gate routes these to
/// approval — a GOVERNANCE signal, independent of the oracle's clinical checks.
const HIGH_ALERT: &[&str] = &[
    "11289",   // warfarin
    "274783",  // insulin glargine
];

pub struct Engine {
    pub epi: EpiDriver,
    pub panel: PatientPanel,
    pub policy: PolicyEngine,
}

pub struct ArmResult {
    pub metrics: MetricsLog,
    pub patients: Vec<PatientState>,
}

impl Engine {
    pub async fn run_arm(&self, cfg: &ArmConfig) -> Result<ArmResult, SimError> {
        let mut rng = StdRng::seed_from_u64(cfg.seed);
        let channel = CopyForwardChannel::new(cfg.max_copyfwd_error_prob);
        let oracle = HarmOracle::new();
        let mut metrics = MetricsLog::new(cfg.mode.label());
        let mut states: std::collections::HashMap<String, PatientState> =
            std::collections::HashMap::new();

        let week_count = cfg.weeks.min(self.epi.weeks().len());
        for w in 0..week_count {
            let week = &self.epi.weeks()[w];
            let mut snap = WeeklySnapshot {
                week_index: week.week_index,
                iso_week: week.iso_week.clone(),
                surge_level: week.surge_level,
                ..Default::default()
            };

            for patient in self.panel.returns(w) {
                snap.encounters += 1;
                let st = states
                    .entry(patient.patient_id.clone())
                    .or_insert_with(|| PatientState::new(&patient.patient_id));

                // Copy-forward the home med list; surge raises the error rate.
                let carried = channel.carry_forward(
                    &patient.medications,
                    week.surge_level,
                    &mut rng,
                    corrupt_med,
                );
                for item in carried.iter().filter(|i| i.is_error) {
                    st.add_pollution(w, &item.code.code);
                }
                st.record_visit(w, carried.len());

                // Re-prescribe each carried med as its own order.
                for (idx, item) in carried.iter().enumerate() {
                    let order = order_from_carried(item);
                    snap.proposed_actions += 1;

                    // The record the oracle reads = the OTHER active meds
                    // (re-prescribing a med is not itself a duplicate).
                    let other_active: Vec<String> = carried
                        .iter()
                        .enumerate()
                        .filter(|(j, _)| *j != idx)
                        .map(|(_, c)| c.code.code.clone())
                        .collect();
                    let rec = RecordView {
                        active_med_codes: other_active,
                        allergy_codes: patient.allergies.iter().map(|a| a.code.clone()).collect(),
                        condition_codes: patient.conditions.iter().map(|c| c.code.clone()).collect(),
                        egfr: patient.egfr,
                    };

                    // Gate decision (governance signal), always computed.
                    let ctx = build_ctx(patient, week.week_index, &order);
                    let gate = VeritasGate::evaluate(&self.policy, &ctx);
                    let applies = VeritasGate::applies(&gate.decision, cfg.mode.gate_on());

                    // Oracle judges clinical harm independently.
                    let violations = oracle.check(&order, &rec);
                    let unsafe_action = !violations.is_empty();

                    if cfg.mode.gate_on() && !applies && unsafe_action {
                        snap.caught_at_gate += 1; // prevented harm, not just a denial
                    }
                    if applies {
                        if unsafe_action {
                            snap.landed_unsafe += 1; // per applied unsafe ACTION
                            for v in &violations {
                                st.record_harm(w, v, cfg.mode.gate_on(), true);
                            }
                        }
                    } else {
                        for v in &violations {
                            st.record_harm(w, v, cfg.mode.gate_on(), false);
                        }
                    }
                }
            }
            metrics.push(snap);
        }

        let patients = states.into_values().collect();
        Ok(ArmResult { metrics, patients })
    }

    /// Run both arms at the same seed and return (gate_on, gate_off).
    pub async fn run_experiment(
        &self,
        seed: u64,
        weeks: usize,
        max_copyfwd_error_prob: f64,
    ) -> Result<(ArmResult, ArmResult), SimError> {
        let on = self
            .run_arm(&ArmConfig { mode: ArmMode::GateOn, seed, weeks, max_copyfwd_error_prob })
            .await?;
        let off = self
            .run_arm(&ArmConfig { mode: ArmMode::GateOff, seed, weeks, max_copyfwd_error_prob })
            .await?;
        Ok((on, off))
    }
}

/// Copy-forward error: a carried home med is mis-transcribed as a high-alert
/// drug (warfarin). This is the propagated documentation error that creates a
/// downstream unsafe order.
fn corrupt_med(c: &CodeRef) -> CodeRef {
    CodeRef {
        system: c.system.clone(),
        code: "11289".into(),
        display: format!("warfarin (copy-forward error from {})", c.display),
    }
}

/// Build the proposed order for a carried med. A correctly-carried med is
/// re-prescribed at a safe dose (no dose set); a corrupted med carries an
/// overdose — the clinical danger the oracle will catch.
fn order_from_carried(item: &CarriedItem) -> ProposedOrderView {
    let dose_mg = if item.is_error { Some(overdose_for(&item.code.code)) } else { None };
    ProposedOrderView { rxnorm: item.code.code.clone(), dose_mg }
}

fn overdose_for(code: &str) -> f64 {
    match code {
        "11289" => 50.0, // warfarin, well over the 10 mg ceiling
        _ => 9_999.0,
    }
}

fn build_ctx(p: &PanelPatient, week_index: usize, order: &ProposedOrderView) -> ActionContext {
    let mut ctx = ActionContext::new("order_entry.propose", "sim-prac");
    ctx.capabilities = vec!["order_entry".into()];
    ctx.role = Some("physician".into());
    ctx.patient_id = Some(p.patient_id.clone());
    ctx.encounter_id = Some(format!("enc-{}-{}", p.patient_id, week_index));
    ctx.properties.insert("encounter_status".into(), "in-progress".into());
    ctx.properties.insert(
        "high_alert".into(),
        if HIGH_ALERT.contains(&order.rxnorm.as_str()) { "true".into() } else { "false".into() },
    );
    ctx
}

#[cfg(test)]
mod tests {
    use super::*;

    // Gate: allow ordinary orders; route HIGH-ALERT drugs to approval.
    // Keys on a GOVERNANCE signal (high_alert), NOT the oracle's clinical
    // predicates — so any catch is non-circular.
    const GOV_REGO: &str = r#"
package cliniclaw.order_entry
default decision := "deny"
decision := "allow" if {
    startswith(input.action, "order_entry.")
    "order_entry" in input.capabilities
    input.properties.high_alert == "false"
}
decision := "require_approval" if {
    startswith(input.action, "order_entry.")
    "order_entry" in input.capabilities
    input.properties.high_alert == "true"
}
"#;

    fn gov_engine(panel_json: &str, epi_csv: &str) -> Engine {
        let epi = EpiDriver::from_csv(epi_csv, 5, 20).unwrap();
        let panel = PatientPanel::from_json(panel_json).unwrap();
        let mut policy = PolicyEngine::new();
        policy.load_rego_str("order_entry.rego", GOV_REGO).unwrap();
        Engine { epi, panel, policy }
    }

    // Benign patient: one home med not in any oracle table, normal eGFR, no
    // allergies/conditions -> clean re-prescription = ZERO violations, isolating
    // drift as the only source of unsafe orders.
    const BENIGN: &str = r#"[
        {"patient_id":"c1","age":60,"egfr":90,"conditions":[],
         "medications":[{"system":"rx","code":"83367","display":"atorvastatin"}],
         "allergies":[],"visit_weeks":[0,1]}
    ]"#;
    // week 0 surge 0 (no corruption), week 1 surge 1.0 (corruption at full rate).
    const EPI: &str = "iso_week,ili_pct\n2023-W40,1.0\n2023-W41,6.0\n";

    #[tokio::test]
    async fn run_arm_produces_weekly_metrics() {
        let eng = gov_engine(BENIGN, EPI);
        let res = eng
            .run_arm(&ArmConfig { mode: ArmMode::GateOff, seed: 1, weeks: 2, max_copyfwd_error_prob: 1.0 })
            .await
            .unwrap();
        assert_eq!(res.metrics.weeks.len(), 2);
        assert_eq!(res.metrics.weeks[0].encounters, 1);
    }

    #[tokio::test]
    async fn deterministic_under_seed() {
        let eng = gov_engine(BENIGN, EPI);
        let cfg = ArmConfig { mode: ArmMode::GateOff, seed: 99, weeks: 2, max_copyfwd_error_prob: 0.5 };
        let a = eng.run_arm(&cfg).await.unwrap();
        let b = eng.run_arm(&cfg).await.unwrap();
        assert_eq!(a.metrics.to_json(), b.metrics.to_json());
    }

    #[tokio::test]
    async fn gap_is_driven_by_drift_not_tautology() {
        let eng = gov_engine(BENIGN, EPI);
        // No drift -> no corruption -> NO unsafe order in either arm.
        let (_on0, off0) = eng.run_experiment(3, 2, 0.0).await.unwrap();
        assert_eq!(off0.metrics.total_landed_unsafe(), 0,
            "zero copy-forward drift must yield zero unsafe orders");
        // Full drift -> corruption creates a high-alert overdose order.
        let (on1, off1) = eng.run_experiment(3, 2, 1.0).await.unwrap();
        assert!(off1.metrics.total_landed_unsafe() > 0,
            "drift must produce unsafe orders in the ungoverned arm");
        assert!(on1.metrics.total_landed_unsafe() < off1.metrics.total_landed_unsafe(),
            "VERITAS must hold the high-alert subset that drift produced");
        let caught: usize = on1.metrics.weeks.iter().map(|w| w.caught_at_gate).sum();
        assert!(caught > 0, "gate-on must record genuine gate catches");
    }

    #[tokio::test]
    async fn gate_on_lands_no_more_unsafe_than_gate_off() {
        let eng = gov_engine(BENIGN, EPI);
        let (on, off) = eng.run_experiment(5, 2, 1.0).await.unwrap();
        assert!(on.metrics.total_landed_unsafe() <= off.metrics.total_landed_unsafe());
    }
}
