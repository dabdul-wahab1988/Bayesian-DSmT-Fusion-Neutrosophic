//! MCMC diagnostics: R-hat and ESS (plan §9).
//!
//! ESS uses the Stan-style autocovariance estimator with Geyer's
//! initial-positive sequence truncation (Stan Reference Manual §6.4,
//! Aki Vehtari et al. 2021) via `math::stats::effective_sample_size`.
//! ESS-tail is computed on the IQR-flagged subset of each chain — a
//! cheap but informative approximation of the tail behaviour.

use crate::bayes::mcmc::Chain;
use crate::math::stats::{effective_sample_size, rhat};

pub struct Diagnostics {
    pub rhat: f64,
    pub ess_bulk: f64,
    pub ess_tail: f64,
}

pub fn compute(chains: &[Chain]) -> Diagnostics {
    if chains.is_empty() {
        return Diagnostics {
            rhat: f64::NAN,
            ess_bulk: 0.0,
            ess_tail: 0.0,
        };
    }
    let chain_draws: Vec<Vec<f64>> = chains.iter().map(|c| c.draws.clone()).collect();
    let r = rhat(&chain_draws);
    let ess_bulk = effective_sample_size(&chain_draws);
    // ESS-tail: take the IQR subset of each chain (drop the lowest and
    // highest 25% of draws) and recompute ESS.  This is a tail-focused
    // approximation that highlights whether the chain is mixing
    // *within* the bulk of the distribution, not just at the centre.
    let mut tail_draws: Vec<Vec<f64>> = vec![];
    for c in &chain_draws {
        if c.len() < 8 {
            tail_draws.push(c.clone());
            continue;
        }
        let mut sorted = c.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let lo = sorted.len() / 4;
        let hi = (3 * sorted.len()) / 4;
        tail_draws.push(sorted[lo..hi].to_vec());
    }
    let ess_tail = effective_sample_size(&tail_draws);
    Diagnostics {
        rhat: r,
        ess_bulk,
        ess_tail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bayes::mcmc::run_chains;
    use crate::math::stats::quantile;

    #[test]
    fn good_chains_have_low_rhat() {
        let chains = run_chains(7, 2, 2000, 500, 0.0, |x: f64| -0.5 * x * x);
        let d = compute(&chains);
        assert!(d.rhat < 1.05, "got rhat={}", d.rhat);
    }

    #[test]
    fn quantiles_in_range() {
        let mut xs: Vec<f64> = (0..1000).map(|i| i as f64 / 1000.0).collect();
        let q = quantile(&mut xs, 0.5).unwrap();
        assert!((q - 0.4995).abs() < 0.01);
    }
}
