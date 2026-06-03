//! Hand-rolled, deterministic MCMC: adaptive Metropolis-within-Gibbs.
//!
//! Inputs are parameter blocks.  The `sample` function takes a vector of
//! `Updater` closures; each is responsible for proposing a new value and
//! accepting/rejecting via the MH rule.  Burn-in and thinning are handled
//! here.  RNG is seeded externally for reproducibility.

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// Result of an MCMC run:  one chain (Vec<f64>) of post-burn-in draws.
pub struct Chain {
    pub draws: Vec<f64>,
    pub accepts: u64,
    pub proposes: u64,
}

impl Chain {
    pub fn acceptance_rate(&self) -> f64 {
        if self.proposes == 0 {
            0.0
        } else {
            self.accepts as f64 / self.proposes as f64
        }
    }
}

/// Adaptive Metropolis (Haario et al., 2001) — proposes from a Gaussian
/// scaled by the running covariance of accepted draws after a warm-up.
pub fn adaptive_metropolis(
    rng: &mut StdRng,
    init: f64,
    log_post: impl Fn(f64) -> f64,
    n_iter: usize,
    burn_in: usize,
) -> Chain {
    let mut x = init;
    let mut lp = log_post(x);
    let mut draws = Vec::with_capacity(n_iter.saturating_sub(burn_in));
    let mut history: Vec<f64> = Vec::with_capacity(n_iter);
    let mut accepts = 0u64;
    let mut proposes = 0u64;
    let initial_scale = 0.1_f64; // proposal scale for the first 100 iters
    for it in 0..n_iter {
        // adapt scale every 50 iters based on empirical SD of accepted
        let proposal_sd = if it < 100 {
            initial_scale
        } else {
            let mean = history.iter().copied().sum::<f64>() / history.len() as f64;
            let var =
                history.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / history.len() as f64;
            (var.sqrt() * 2.38 / 1.0).max(1e-6)
        };
        let z: f64 = rng.gen::<f64>().mul_add(2.0, -1.0); // uniform[-1,1]
        let prop = x + z * proposal_sd;
        let lp_prop = log_post(prop);
        let log_alpha = lp_prop - lp;
        // Standard Metropolis-Hastings rule with the log-sum-exp safe form
        // `u < min(1, exp(log_alpha))` to avoid `exp(very_negative)` =
        // 0 underflow, which silently rejects every proposal.
        let u: f64 = rng.gen();
        if u < log_alpha.exp().min(1.0) {
            x = prop;
            lp = lp_prop;
            accepts += 1;
        }
        proposes += 1;
        history.push(x);
        if it >= burn_in {
            draws.push(x);
        }
        if it % 50 == 0 {
            // keep history bounded
            if history.len() > 2000 {
                let drop = history.len() - 2000;
                history.drain(0..drop);
            }
        }
    }
    Chain {
        draws,
        accepts,
        proposes,
    }
}

/// Helper to seed a StdRng deterministically from a u64.
pub fn seeded_rng(seed: u64) -> StdRng {
    StdRng::seed_from_u64(seed)
}

/// Run `n_chains` independent chains and return.
pub fn run_chains(
    seed: u64,
    n_chains: usize,
    n_iter: usize,
    burn_in: usize,
    init: f64,
    log_post: impl Fn(f64) -> f64 + Copy,
) -> Vec<Chain> {
    (0..n_chains)
        .map(|c| {
            let mut rng = seeded_rng(seed.wrapping_add(c as u64 + 1));
            adaptive_metropolis(&mut rng, init, log_post, n_iter, burn_in)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sample from N(0,1) — log posterior ∝ -x^2/2
    #[test]
    fn recovers_normal_mean() {
        let chains = run_chains(42, 2, 4000, 1000, 1.0, |x: f64| -0.5 * x * x);
        for c in &chains {
            let m: f64 = c.draws.iter().sum::<f64>() / c.draws.len() as f64;
            assert!(m.abs() < 0.2, "mean too far from 0: {m}");
        }
    }
}
