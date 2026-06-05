//! CopyForwardChannel: propagates prior record entries, with surge-driven
//! error injection. This is the A->B coupling: surge_level raises copyfwd_rate.

use rand::Rng;
use rand::rngs::StdRng;
use crate::panel::CodeRef;

#[derive(Debug, Clone, PartialEq)]
pub struct CarriedItem {
    pub code: CodeRef,
    pub is_error: bool,   // true = a propagated documentation error
}

pub struct CopyForwardChannel {
    /// error probability at surge_level 1.0 (anchored to copy-paste lit; calibrate)
    max_error_prob: f64,
}

impl CopyForwardChannel {
    pub fn new(max_error_prob: f64) -> Self { Self { max_error_prob } }

    /// copyfwd error probability for this week.
    pub fn error_prob(&self, surge_level: f64) -> f64 {
        self.max_error_prob * surge_level
    }

    /// Carry prior meds forward; each may be flipped to an erroneous code.
    /// `corrupt` supplies a wrong code when an error fires.
    pub fn carry_forward(
        &self,
        prior: &[CodeRef],
        surge_level: f64,
        rng: &mut StdRng,
        corrupt: impl Fn(&CodeRef) -> CodeRef,
    ) -> Vec<CarriedItem> {
        let p = self.error_prob(surge_level);
        prior.iter().map(|c| {
            if rng.gen::<f64>() < p {
                CarriedItem { code: corrupt(c), is_error: true }
            } else {
                CarriedItem { code: c.clone(), is_error: false }
            }
        }).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    fn code(c: &str) -> CodeRef {
        CodeRef { system: "rx".into(), code: c.into(), display: c.into() }
    }

    #[test]
    fn error_prob_scales_with_surge() {
        let ch = CopyForwardChannel::new(0.5);
        assert!((ch.error_prob(0.0) - 0.0).abs() < 1e-9);
        assert!((ch.error_prob(1.0) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn no_errors_at_zero_surge() {
        let ch = CopyForwardChannel::new(0.5);
        let mut rng = StdRng::seed_from_u64(42);
        let out = ch.carry_forward(&[code("A"), code("B")], 0.0, &mut rng, |_| code("WRONG"));
        assert!(out.iter().all(|i| !i.is_error));
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn deterministic_under_seed() {
        let ch = CopyForwardChannel::new(1.0); // force errors
        let mut r1 = StdRng::seed_from_u64(7);
        let mut r2 = StdRng::seed_from_u64(7);
        let a = ch.carry_forward(&[code("A")], 1.0, &mut r1, |_| code("WRONG"));
        let b = ch.carry_forward(&[code("A")], 1.0, &mut r2, |_| code("WRONG"));
        assert_eq!(a, b);
        assert!(a[0].is_error);
    }
}
