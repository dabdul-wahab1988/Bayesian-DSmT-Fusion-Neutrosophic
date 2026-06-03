//! Posterior summary helpers (plan §10).

use crate::math::stats::{mean, quantile, sd};

#[derive(Debug, Clone)]
pub struct PosteriorSummary {
    pub mean: f64,
    pub sd: f64,
    pub ci_lo: f64,
    pub ci_hi: f64,
}

pub fn summarise(draws: &mut [f64]) -> Option<PosteriorSummary> {
    if draws.is_empty() {
        return None;
    }
    let m = mean(draws).unwrap_or(0.0);
    let s = sd(draws).unwrap_or(0.0);
    let lo = quantile(draws, 0.025).unwrap_or(m);
    let hi = quantile(draws, 0.975).unwrap_or(m);
    Some(PosteriorSummary {
        mean: m,
        sd: s,
        ci_lo: lo,
        ci_hi: hi,
    })
}
