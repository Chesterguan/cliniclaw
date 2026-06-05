//! 2-week, small-panel end-to-end smoke on mock backends.

use cliniclaw_policy::PolicyEngine;
use cliniclaw_sim::engine::Engine;
use cliniclaw_sim::epi::EpiDriver;
use cliniclaw_sim::panel::PatientPanel;

#[tokio::test]
async fn two_week_smoke_runs_both_arms() {
    let epi = EpiDriver::from_csv(
        "iso_week,ili_pct\n2023-W40,1.0\n2023-W41,6.0\n", 3, 10).unwrap();
    let panel = PatientPanel::from_json(r#"[
        {"patient_id":"c1","age":74,"egfr":20,"conditions":[],
         "medications":[{"system":"rx","code":"6809","display":"metformin"}],
         "allergies":[],"visit_weeks":[0,1]},
        {"patient_id":"c2","age":66,"egfr":85,"conditions":[],
         "medications":[],"allergies":[],"visit_weeks":[1]}
    ]"#).unwrap();
    let mut policy = PolicyEngine::new();
    policy.load_rego_str("order_entry.rego", r#"
package cliniclaw.order_entry
default decision := "deny"
decision := "allow" if {
    startswith(input.action, "order_entry.")
    "order_entry" in input.capabilities
}
"#).unwrap();

    let engine = Engine { epi, panel, policy };
    let (on, off) = engine.run_experiment(7, 2, 0.5).await.unwrap();
    assert_eq!(on.metrics.weeks.len(), 2);
    assert!(on.metrics.total_landed_unsafe() <= off.metrics.total_landed_unsafe());
}
