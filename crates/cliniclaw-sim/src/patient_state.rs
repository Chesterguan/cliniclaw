//! PatientState: per-patient pollution ledger, harm events, trajectory.

use crate::oracle::Violation;

#[derive(Debug, Clone, serde::Serialize)]
pub struct PollutionEntry {
    pub introduced_week: usize,
    pub rxnorm: String,
    pub propagation_count: usize,  // downstream reads that consumed it
    pub still_present: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct HarmEvent {
    pub week: usize,
    pub kind: String,
    pub detail: String,
    pub arm_gate_on: bool,
    pub landed: bool,   // applied to the record (gate-off, or allow)
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct PatientState {
    pub patient_id: String,
    pub encounter_count: usize,
    pub weeks_seen: Vec<usize>,
    pub med_list_size: usize,
    pub pollution: Vec<PollutionEntry>,
    pub harm_events: Vec<HarmEvent>,
}

impl PatientState {
    pub fn new(patient_id: impl Into<String>) -> Self {
        Self { patient_id: patient_id.into(), ..Default::default() }
    }
    pub fn record_visit(&mut self, week: usize, med_list_size: usize) {
        self.encounter_count += 1;
        self.weeks_seen.push(week);
        self.med_list_size = med_list_size;
    }
    pub fn add_pollution(&mut self, week: usize, rxnorm: impl Into<String>) {
        self.pollution.push(PollutionEntry {
            introduced_week: week, rxnorm: rxnorm.into(),
            propagation_count: 0, still_present: true,
        });
    }
    pub fn mark_propagated(&mut self, rxnorm: &str) {
        for e in self.pollution.iter_mut().filter(|e| e.rxnorm == rxnorm && e.still_present) {
            e.propagation_count += 1;
        }
    }
    pub fn record_harm(&mut self, week: usize, v: &Violation, gate_on: bool, landed: bool) {
        self.harm_events.push(HarmEvent {
            week, kind: format!("{:?}", v.kind), detail: v.detail.clone(),
            arm_gate_on: gate_on, landed,
        });
    }
    pub fn landed_unsafe_count(&self) -> usize {
        self.harm_events.iter().filter(|h| h.landed).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oracle::ViolationKind;
    fn viol() -> Violation { Violation { kind: ViolationKind::DrugAllergy, detail: "x".into() } }

    #[test]
    fn tracks_visits() {
        let mut s = PatientState::new("p1");
        s.record_visit(0, 3);
        s.record_visit(8, 4);
        assert_eq!(s.encounter_count, 2);
        assert_eq!(s.weeks_seen, vec![0, 8]);
        assert_eq!(s.med_list_size, 4);
    }
    #[test]
    fn pollution_propagation_counts() {
        let mut s = PatientState::new("p1");
        s.add_pollution(2, "6809");
        s.mark_propagated("6809");
        s.mark_propagated("6809");
        assert_eq!(s.pollution[0].propagation_count, 2);
    }
    #[test]
    fn landed_unsafe_only_counts_landed() {
        let mut s = PatientState::new("p1");
        s.record_harm(3, &viol(), true, false);
        s.record_harm(3, &viol(), false, true);
        assert_eq!(s.landed_unsafe_count(), 1);
    }
}
