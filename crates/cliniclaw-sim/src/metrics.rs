//! Weekly metrics per arm; the gate-on vs gate-off gap is the headline result.

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct WeeklySnapshot {
    pub week_index: usize,
    pub iso_week: String,
    pub surge_level: f64,
    pub encounters: usize,
    pub proposed_actions: usize,
    pub caught_at_gate: usize,    // gate-on: blocked
    pub landed_unsafe: usize,     // applied actions that violate an invariant
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct MetricsLog {
    pub arm: String,              // "gate_on" | "gate_off"
    pub weeks: Vec<WeeklySnapshot>,
}

impl MetricsLog {
    pub fn new(arm: impl Into<String>) -> Self {
        Self { arm: arm.into(), weeks: Vec::new() }
    }
    pub fn push(&mut self, s: WeeklySnapshot) { self.weeks.push(s); }
    pub fn total_landed_unsafe(&self) -> usize {
        self.weeks.iter().map(|w| w.landed_unsafe).sum()
    }
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("serialize metrics")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn totals_landed_unsafe() {
        let mut m = MetricsLog::new("gate_off");
        m.push(WeeklySnapshot { landed_unsafe: 2, ..Default::default() });
        m.push(WeeklySnapshot { landed_unsafe: 3, ..Default::default() });
        assert_eq!(m.total_landed_unsafe(), 5);
    }
    #[test]
    fn serializes() {
        let m = MetricsLog::new("gate_on");
        assert!(m.to_json().contains("gate_on"));
    }
}
