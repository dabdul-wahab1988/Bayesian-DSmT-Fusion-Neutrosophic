//! Latent hotspot model (plan §8.1).
//!
//! psi_i = gamma_0 + gamma_E * E_i + gamma_S * S_i + gamma_X * X_i - gamma_U * U_i
//! p_i^hotspot = sigmoid(psi_i)
//!
//! This is the unsupervised decision-support mode in plan §8.1.  The MCMC
//! samples psi directly (already on the logit scale) around the evidence-based
//! score from E_i, S_i, X_i, U_i.  It is not a supervised predictive posterior
//! unless an explicit labelled-hotspot likelihood is added; validation labels
//! stay out of this model to avoid leakage.

use crate::bayes::mcmc::{adaptive_metropolis, seeded_rng, Chain};
use crate::error::Result;

/// Log density for the unsupervised decision-support psi distribution.
pub fn log_posterior(psi: f64, prior_mean: f64, prior_sd: f64) -> f64 {
    -0.5 * ((psi - prior_mean) / prior_sd).powi(2) - prior_sd.ln()
}

pub fn sample(
    prior_mean: f64,
    prior_sd: f64,
    seed: u64,
    n_iter: usize,
    burn_in: usize,
) -> Result<Chain> {
    let mut rng = seeded_rng(seed);
    let chain = adaptive_metropolis(
        &mut rng,
        prior_mean,
        |psi| log_posterior(psi, prior_mean, prior_sd),
        n_iter,
        burn_in,
    );
    Ok(chain)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::stats::inv_logit;

    #[test]
    fn hotspot_probability_in_range() {
        let c = sample(0.0, 1.0, 42, 1000, 200).unwrap();
        for psi in &c.draws {
            let p = inv_logit(*psi);
            assert!((0.0..=1.0).contains(&p));
        }
    }
}
