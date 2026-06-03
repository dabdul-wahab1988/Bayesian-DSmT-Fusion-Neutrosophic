//! Generalized pignistic transformation (plan §13).

use std::collections::HashMap;

use crate::dsmt::expression::{cardinality, intersect};

/// Compute BetP_i(theta_h) from a fused mass assignment.
///   BetP(theta_h) = sum_X m(X) * |X ∩ theta_h| / |X|
pub fn pignistic(fused: &HashMap<Vec<usize>, f64>, h: usize) -> f64 {
    let mut acc = 0.0;
    for (set, m) in fused {
        let cm_x = cardinality(set).max(1) as f64;
        let isect = intersect(set, &vec![h]);
        let cm_isect = cardinality(&isect) as f64;
        acc += m * cm_isect / cm_x;
    }
    acc
}

/// Compute BetP for all singletons and find the dominant hypothesis.
pub fn dominant(fused: &HashMap<Vec<usize>, f64>, n: usize) -> (usize, Vec<f64>) {
    let bp: Vec<f64> = (0..n).map(|h| pignistic(fused, h)).collect();
    let dom = (0..n)
        .max_by(|&a, &b| {
            bp[a]
                .partial_cmp(&bp[b])
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(0);
    (dom, bp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pignistic_preserves_total_mass() {
        let mut f = HashMap::new();
        f.insert(vec![0usize], 0.6);
        f.insert(vec![1usize], 0.3);
        f.insert(vec![0, 1], 0.1);
        // |{0}|=1; BetP(theta_0) gets 0.6 (full) + 0.1 * 1/2 = 0.65
        // |{1}|=1; BetP(theta_1) gets 0.3 + 0.05 = 0.35
        let b0 = pignistic(&f, 0);
        let b1 = pignistic(&f, 1);
        assert!((b0 - 0.65).abs() < 1e-9);
        assert!((b1 - 0.35).abs() < 1e-9);
    }
}
