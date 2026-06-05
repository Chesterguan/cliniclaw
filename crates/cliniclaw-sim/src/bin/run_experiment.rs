//! Run the two-season, two-arm experiment on mock backends and print the gap.
//! Usage: cargo run -p cliniclaw-sim --bin run_experiment

use cliniclaw_policy::PolicyEngine;
use cliniclaw_sim::engine::Engine;
use cliniclaw_sim::epi::EpiDriver;
use cliniclaw_sim::panel::PatientPanel;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let epi_csv = include_str!("../../data/epi/respiratory_2023_2025.csv");
    let epi = EpiDriver::from_csv(epi_csv, 20, 60)?;
    let panel = PatientPanel::from_json(include_str!("../../data/panel/chronic_50.json"))?;

    // Sim renal-safety policy (deny-by-default; blocks low-eGFR contraindicated orders).
    let mut policy = PolicyEngine::new();
    policy.load_rego_str("order_entry.rego", include_str!("../../data/policy/order_entry.rego"))?;

    let weeks = epi.weeks().len();
    let engine = Engine { epi, panel, policy };
    let (on, off) = engine.run_experiment(/*seed*/ 2026, weeks, /*max_copyfwd_error_prob*/ 0.4).await?;

    std::fs::create_dir_all("target/sim")?;
    std::fs::write("target/sim/gate_on.json", on.metrics.to_json())?;
    std::fs::write("target/sim/gate_off.json", off.metrics.to_json())?;

    let on_landed = on.metrics.total_landed_unsafe();
    let off_landed = off.metrics.total_landed_unsafe();
    let caught: usize = on.metrics.weeks.iter().map(|w| w.caught_at_gate).sum();
    println!("=== VERITAS long-horizon result ({weeks} weeks, two seasons) ===");
    println!("gate-on  landed-unsafe: {on_landed}  (caught at gate: {caught})");
    println!("gate-off landed-unsafe: {off_landed}");
    println!("VERITAS prevented {} unsafe actions from reaching patients", off_landed as i64 - on_landed as i64);
    println!("metrics written to target/sim/gate_on.json and target/sim/gate_off.json");
    Ok(())
}
