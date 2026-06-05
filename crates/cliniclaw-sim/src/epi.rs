//! EpiDriver: turns a vendored weekly surveillance series into a per-week plan.

use crate::SimError;

#[derive(Debug, Clone, PartialEq)]
pub struct WeekPlan {
    pub week_index: usize,   // 0-based across the whole run
    pub iso_week: String,    // e.g. "2023-W46"
    pub ili_pct: f64,
    pub surge_level: f64,    // normalized 0.0..=1.0 across the run
    pub arrivals: usize,     // acute walk-ins this week
}

#[derive(Debug, Clone)]
pub struct EpiDriver {
    weeks: Vec<WeekPlan>,
}

impl EpiDriver {
    /// `base_arrivals` = acute walk-ins at surge_level 0; `surge_arrivals` =
    /// additional walk-ins at surge_level 1. arrivals scales linearly between.
    pub fn from_csv(csv: &str, base_arrivals: usize, surge_arrivals: usize) -> Result<Self, SimError> {
        let mut rows: Vec<(String, f64)> = Vec::new();
        for (i, line) in csv.lines().enumerate() {
            let line = line.trim();
            if i == 0 || line.is_empty() { continue; } // header / blank
            let mut parts = line.split(',');
            let iso = parts.next().ok_or_else(|| SimError::Epi(format!("row {i}: missing iso_week")))?;
            let pct: f64 = parts.next()
                .ok_or_else(|| SimError::Epi(format!("row {i}: missing ili_pct")))?
                .trim().parse()
                .map_err(|e| SimError::Epi(format!("row {i}: bad ili_pct: {e}")))?;
            rows.push((iso.to_string(), pct));
        }
        if rows.is_empty() {
            return Err(SimError::Epi("no data rows".into()));
        }
        let min = rows.iter().map(|r| r.1).fold(f64::INFINITY, f64::min);
        let max = rows.iter().map(|r| r.1).fold(f64::NEG_INFINITY, f64::max);
        let span = (max - min).max(f64::EPSILON);
        let weeks = rows.into_iter().enumerate().map(|(idx, (iso, pct))| {
            let surge_level = (pct - min) / span;
            let arrivals = base_arrivals + (surge_level * surge_arrivals as f64).round() as usize;
            WeekPlan { week_index: idx, iso_week: iso, ili_pct: pct, surge_level, arrivals }
        }).collect();
        Ok(Self { weeks })
    }
    pub fn weeks(&self) -> &[WeekPlan] { &self.weeks }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "iso_week,ili_pct\n2023-W40,1.0\n2023-W46,6.0\n2023-W50,3.5\n";

    #[test]
    fn parses_rows_in_order() {
        let d = EpiDriver::from_csv(SAMPLE, 10, 40).unwrap();
        assert_eq!(d.weeks().len(), 3);
        assert_eq!(d.weeks()[0].iso_week, "2023-W40");
        assert_eq!(d.weeks()[0].week_index, 0);
        assert_eq!(d.weeks()[2].week_index, 2);
    }

    #[test]
    fn surge_level_normalized_min_to_max() {
        let d = EpiDriver::from_csv(SAMPLE, 10, 40).unwrap();
        assert!((d.weeks()[0].surge_level - 0.0).abs() < 1e-9);
        assert!((d.weeks()[1].surge_level - 1.0).abs() < 1e-9);
    }

    #[test]
    fn arrivals_scale_with_surge() {
        let d = EpiDriver::from_csv(SAMPLE, 10, 40).unwrap();
        assert_eq!(d.weeks()[0].arrivals, 10);
        assert_eq!(d.weeks()[1].arrivals, 50);
    }

    #[test]
    fn rejects_empty() {
        assert!(EpiDriver::from_csv("iso_week,ili_pct\n", 10, 40).is_err());
    }
}
