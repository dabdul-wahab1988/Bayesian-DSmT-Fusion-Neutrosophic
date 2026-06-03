//! Neutrosophic membership (plan §14).
//!
//! T = q * r
//! F = q * (1 - r)
//! I = 1 - q
//!
//! q = (1 - missingness) * (1 - uncertainty) * (1 - K)  (combined confidence)

use crate::error::{BayesDsmError, Result};

#[derive(Debug, Clone, Copy)]
pub struct Triplet {
    pub t: f64,
    pub f: f64,
    pub i: f64,
}

impl Triplet {
    pub fn check(&self) -> Result<()> {
        for (name, v) in [("T", self.t), ("F", self.f), ("I", self.i)] {
            if !(0.0..=1.0).contains(&v) {
                return Err(BayesDsmError::Stop {
                    module: "neutrosophic".into(),
                    code: "E1407".into(),
                    message: format!("{name} out of [0,1]: {v}"),
                });
            }
        }
        if ((self.t + self.f + self.i) - 1.0).abs() > 1e-6 {
            // Soft warn: do not STOP since the general definition allows >1.
        }
        Ok(())
    }
}

pub fn triplet(r: f64, q: f64) -> Triplet {
    // Clamp q and r to [0, 1] to defend against tiny floating-point overshoots
    // (e.g. r * q = 1.0000000000000002 from summing normalised Dirichlet means).
    let q = q.clamp(0.0, 1.0);
    let r = r.clamp(0.0, 1.0);
    let t = q * r;
    let f = q * (1.0 - r);
    let i = 1.0 - q;
    Triplet { t, f, i }
}

pub fn criterion_score(t: f64, f: f64, i: f64, eta: f64) -> f64 {
    let s = (1.0 + t - f - eta * i) / 2.0;
    s.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sums_to_q() {
        let tp = triplet(0.8, 0.9);
        assert!((tp.t + tp.f - 0.9).abs() < 1e-9);
        assert!((tp.i - 0.1).abs() < 1e-9);
    }

    #[test]
    fn criterion_in_range() {
        let s = criterion_score(0.9, 0.1, 0.0, 0.5);
        assert!((0.0..=1.0).contains(&s));
    }
}
