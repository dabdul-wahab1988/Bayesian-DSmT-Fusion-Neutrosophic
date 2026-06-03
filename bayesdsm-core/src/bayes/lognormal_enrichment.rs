//! Lognormal enrichment model (plan §6.1).
//!
//! For detected observations:
//!   z_ijm = log(y_ijm) ~ N(mu_im, sigma_m^2)
//!
//! Site-metal latent mean:
//!   mu_im ~ N(alpha_m, tau_m^2)
//!
//! Metal-level prior centred on background:
//!   alpha_m ~ N(log(B_m), sigma_alpha_m^2)
//!
//! Posterior draws of mu_im yield per-draw EF_im = exp(mu_im) / B_m*, from
//! which we compute the posterior probability that EF > T_E.

use rand_distr::{Distribution, Normal};
use std::collections::HashMap;

use crate::bayes::mcmc::{adaptive_metropolis, seeded_rng, Chain};
use crate::math::stats::norm_cdf;

#[derive(Debug, Clone)]
pub struct SiteMetal {
    pub site_id: String,
    pub metal: String,
    pub log_y: Vec<f64>,   // log of detected cleaned values
    pub dl_log: Vec<f64>,  // log detection limits for non-detects (same length when censored)
    pub detect: Vec<bool>, // true for detected
}

pub fn build_site_metal(
    cleaned: &[(String, String, String, f64, f64, i64, Option<f64>)], // site, sample, metal, value_std, dl, detect_flag, dl
) -> Vec<SiteMetal> {
    let mut map: HashMap<(String, String), SiteMetal> = HashMap::new();
    for (s, _p, m, v, _dl, flag, dl_opt) in cleaned {
        let key = (s.clone(), m.clone());
        let e = map.entry(key).or_insert_with(|| SiteMetal {
            site_id: s.clone(),
            metal: m.clone(),
            log_y: vec![],
            dl_log: vec![],
            detect: vec![],
        });
        if *flag == 1 && *v > 0.0 {
            e.log_y.push(v.ln());
            e.dl_log.push(0.0);
            e.detect.push(true);
        } else if let Some(d) = dl_opt {
            if *d > 0.0 {
                e.dl_log.push(d.ln());
                e.log_y.push(0.0);
                e.detect.push(false);
            }
        }
    }
    map.into_values().collect()
}

/// Log posterior for a single (site, metal) latent mean `mu`.
/// Includes the metal-level prior alpha_m centred on log(B_m) when supplied.
///
/// Likelihood is the product over all observations of either the
/// log-normal density (detected) or the censored log-normal CDF
/// (non-detect).  The `−ln σ` Jacobian of the log-normal density is
/// added **once per detected observation** (i.e. per likelihood term,
/// not per data point — this is the standard normalising constant of
/// `N(z; μ, σ²)`).
pub fn log_posterior(mu: f64, sigma: f64, sm: &SiteMetal, alpha_m: f64, tau_m: f64) -> f64 {
    // Likelihood for detected values.
    let mut lp = 0.0;
    let mut n_det = 0usize;
    for (i, &z) in sm.log_y.iter().enumerate() {
        if !sm.detect[i] {
            continue;
        }
        if !z.is_finite() {
            // Defensive: a non-finite log-concentration is structurally
            // impossible (we only push finite `v.ln()` above) but a
            // poisoned input would otherwise NaN-poison the chain.
            continue;
        }
        lp += -0.5 * ((z - mu) / sigma).powi(2);
        n_det += 1;
    }
    if n_det > 0 {
        lp -= n_det as f64 * sigma.ln();
    }

    // Censored likelihood for non-detects:  P(z < log DL) = Φ((log DL − μ)/σ).
    // Floor Φ at 1e-300 before taking the log to avoid log(0) = −∞ for
    // extreme z (a posterior mean far above the DL drives Φ → 0).
    for (i, _) in sm.log_y.iter().enumerate() {
        if sm.detect[i] {
            continue;
        }
        let log_dl = sm.dl_log[i];
        if !log_dl.is_finite() {
            continue;
        }
        let z = (log_dl - mu) / sigma;
        let cdf = norm_cdf(z).max(1e-300);
        lp += cdf.ln();
    }

    // Prior on mu.
    lp += -0.5 * ((mu - alpha_m) / tau_m).powi(2) - tau_m.ln();
    lp
}

/// Run the enrichment model for one (site, metal) and return posterior
/// draws of EF_im.  sigma is the shared metal-level scale.
pub fn sample_ef(
    sm: &SiteMetal,
    b_m: f64,
    sigma: f64,
    alpha_m: f64,
    tau_m: f64,
    seed: u64,
    n_iter: usize,
    burn_in: usize,
) -> Chain {
    // initial value: mean of detected log_y, or log(B_m) if no detected
    let init = if !sm.log_y.is_empty() && sm.detect.iter().any(|&d| d) {
        let n = sm
            .log_y
            .iter()
            .zip(sm.detect.iter())
            .filter(|(_, d)| **d)
            .count();
        let s: f64 = sm
            .log_y
            .iter()
            .zip(sm.detect.iter())
            .filter(|(_, d)| **d)
            .map(|(v, _)| v)
            .sum();
        s / n as f64
    } else {
        b_m.ln()
    };
    let mut rng = seeded_rng(seed);
    let chain = adaptive_metropolis(
        &mut rng,
        init,
        |mu| log_posterior(mu, sigma, sm, alpha_m, tau_m),
        n_iter,
        burn_in,
    );
    // Convert mu draws to EF draws on the fly via a thin wrapper.
    Chain {
        draws: chain.draws.iter().map(|mu| mu.exp() / b_m).collect(),
        accepts: chain.accepts,
        proposes: chain.proposes,
    }
}

#[allow(dead_code)]
fn _unused_normal() {
    let mut rng = seeded_rng(0);
    let n = Normal::new(0.0, 1.0).unwrap();
    let _x: f64 = n.sample(&mut rng);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn site_metal_build() {
        let rows = vec![
            (
                "S1".into(),
                "p1".into(),
                "As".into(),
                5.0,
                0.1,
                1,
                Some(0.1),
            ),
            (
                "S1".into(),
                "p2".into(),
                "As".into(),
                7.0,
                0.1,
                1,
                Some(0.1),
            ),
            (
                "S1".into(),
                "p3".into(),
                "As".into(),
                0.0,
                0.05,
                0,
                Some(0.05),
            ),
        ];
        let sms = build_site_metal(&rows);
        assert_eq!(sms.len(), 1);
        let sm = &sms[0];
        assert_eq!(sm.detect.iter().filter(|d| **d).count(), 2);
    }

    #[test]
    fn sample_ef_returns_positive() {
        let sm = SiteMetal {
            site_id: "S1".into(),
            metal: "As".into(),
            log_y: vec![2.0, 2.1, 2.2],
            dl_log: vec![0.0, 0.0, 0.0],
            detect: vec![true, true, true],
        };
        let chain = sample_ef(&sm, 5.0, 0.2, 1.5, 0.5, 42, 2000, 500);
        for v in &chain.draws {
            assert!(*v > 0.0 && v.is_finite());
        }
    }
}
