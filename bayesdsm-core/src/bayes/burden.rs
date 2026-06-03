//! Bayesian metal-burden score (plan §6.2).
//!
//! E_i^(s) = sum_m a_m * 1(EF_im^(s) > T_E)
//!
//! Requires non-negative weights summing to 1.

use crate::error::{BayesDsmError, Result};

pub fn burden_one_draw(ef_im: &[f64], a_m: &[f64], t_e: f64) -> f64 {
    debug_assert_eq!(ef_im.len(), a_m.len());
    let mut acc = 0.0;
    for (v, a) in ef_im.iter().zip(a_m.iter()) {
        if *v > t_e {
            acc += a;
        }
    }
    acc
}

pub fn check_weights(a_m: &[(String, f64)]) -> Result<()> {
    let s: f64 = a_m.iter().map(|(_, w)| w).sum();
    if (s - 1.0).abs() > 1e-8 {
        return Err(BayesDsmError::Stop {
            module: "bayes".into(),
            code: "E1601".into(),
            message: format!("metal weights must sum to 1 (got {s})"),
        });
    }
    for (m, w) in a_m {
        if *w < 0.0 {
            return Err(BayesDsmError::Stop {
                module: "bayes".into(),
                code: "E1601".into(),
                message: format!("metal weight for {m} is negative ({w})"),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn burden_in_unit_range() {
        let ef = vec![2.0, 0.5, 1.6];
        let a = vec![0.5, 0.3, 0.2];
        let t = 1.5;
        let b = burden_one_draw(&ef, &a, t);
        // Only metals 0 and 2 exceed T_E, so b = 0.5 + 0.2 = 0.7.
        assert!((b - 0.7).abs() < 1e-9);
    }
}
