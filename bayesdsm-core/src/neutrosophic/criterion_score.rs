//! Criterion-level scoring helpers.

use crate::error::Result;
use crate::neutrosophic::membership::{criterion_score, triplet, Triplet};

pub fn score_one(r: f64, q: f64, eta: f64) -> (Triplet, f64) {
    let t = triplet(r, q);
    let s = criterion_score(t.t, t.f, t.i, eta);
    (t, s)
}

pub fn weighted_priority(scores: &[(String, f64)], weights: &[(String, f64)]) -> Result<f64> {
    // Validate weights sum to 1.
    let wsum: f64 = weights.iter().map(|(_, w)| w).sum();
    if (wsum - 1.0).abs() > 1e-8 {
        return Err(crate::error::BayesDsmError::Stop {
            module: "neutrosophic".into(),
            code: "E1403".into(),
            message: format!("weights must sum to 1 (got {wsum})"),
        });
    }
    let mut acc = 0.0;
    for (c, w) in weights {
        let s = scores
            .iter()
            .find(|(k, _)| k == c)
            .map(|x| x.1)
            .ok_or_else(|| crate::error::BayesDsmError::Stop {
                module: "neutrosophic".into(),
                code: "E1404".into(),
                message: format!("weight criterion '{c}' does not match any computed criterion"),
            })?;
        acc += w * s;
    }
    Ok(acc.clamp(0.0, 1.0))
}
