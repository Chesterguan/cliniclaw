//! Arm configuration: gate-on (control: VERITAS enforced) vs gate-off (counterfactual).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmMode { GateOn, GateOff }

impl ArmMode {
    pub fn label(&self) -> &'static str {
        match self { ArmMode::GateOn => "gate_on", ArmMode::GateOff => "gate_off" }
    }
    pub fn gate_on(&self) -> bool { matches!(self, ArmMode::GateOn) }
}

#[derive(Debug, Clone)]
pub struct ArmConfig {
    pub mode: ArmMode,
    pub seed: u64,
    pub weeks: usize,             // cap (e.g. 56 for two seasons; small for tests)
    pub max_copyfwd_error_prob: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn labels() {
        assert_eq!(ArmMode::GateOn.label(), "gate_on");
        assert!(!ArmMode::GateOff.gate_on());
    }
}
