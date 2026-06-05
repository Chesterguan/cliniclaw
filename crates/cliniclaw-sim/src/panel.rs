//! PatientPanel: a longitudinal cohort whose record persists across the run.

use crate::SimError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelClass { Chronic, Acute }

#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct CodeRef {
    pub system: String,
    pub code: String,
    pub display: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct PanelPatient {
    pub patient_id: String,
    pub age: u32,
    pub egfr: f64,
    pub conditions: Vec<CodeRef>,
    pub medications: Vec<CodeRef>,
    #[serde(default)]
    pub allergies: Vec<CodeRef>,
    pub visit_weeks: Vec<usize>,
    #[serde(skip, default = "default_class")]
    pub class: PanelClass,
}
fn default_class() -> PanelClass { PanelClass::Chronic }

pub struct PatientPanel {
    chronic: Vec<PanelPatient>,
}

impl PatientPanel {
    pub fn from_json(json: &str) -> Result<Self, SimError> {
        let mut chronic: Vec<PanelPatient> = serde_json::from_str(json)
            .map_err(|e| SimError::Panel(format!("parse chronic panel: {e}")))?;
        if chronic.is_empty() {
            return Err(SimError::Panel("empty panel".into()));
        }
        for p in &mut chronic { p.class = PanelClass::Chronic; }
        Ok(Self { chronic })
    }
    pub fn chronic(&self) -> &[PanelPatient] { &self.chronic }
    /// Chronic patients scheduled to return in `week`.
    pub fn returns(&self, week: usize) -> Vec<&PanelPatient> {
        self.chronic.iter().filter(|p| p.visit_weeks.contains(&week)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const J: &str = r#"[
      {"patient_id":"c1","age":70,"egfr":40,"conditions":[],"medications":[],"allergies":[],"visit_weeks":[0,8]},
      {"patient_id":"c2","age":60,"egfr":80,"conditions":[],"medications":[],"visit_weeks":[8]}
    ]"#;

    #[test]
    fn loads_panel() {
        let p = PatientPanel::from_json(J).unwrap();
        assert_eq!(p.chronic().len(), 2);
        assert_eq!(p.chronic()[0].class, PanelClass::Chronic);
    }
    #[test]
    fn returns_by_week() {
        let p = PatientPanel::from_json(J).unwrap();
        assert_eq!(p.returns(8).len(), 2);
        assert_eq!(p.returns(0).len(), 1);
        assert_eq!(p.returns(99).len(), 0);
    }
    #[test]
    fn allergies_default_empty() {
        let p = PatientPanel::from_json(J).unwrap();
        assert!(p.chronic()[1].allergies.is_empty());
    }
    #[test]
    fn real_seed_file_loads_50() {
        let json = include_str!("../data/panel/chronic_50.json");
        let p = PatientPanel::from_json(json).unwrap();
        assert_eq!(p.chronic().len(), 50);
    }
}
