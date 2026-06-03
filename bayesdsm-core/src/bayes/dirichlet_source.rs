//! Evidence-weighted Dirichlet source-support model (plan §7.1).

use rand::Rng;
use rand_distr::{Dirichlet, Distribution};

use crate::bayes::mcmc::seeded_rng;
use crate::error::Result;

/// Per-site, per-source Dirichlet concentration A_{ih} = alpha_0h + lambda * sum_k rho_kh * x_ikh.
pub fn build_concentrations(
    site_ids: &[String],
    source_indicators: &[(String, String, String, f64, f64, String)], // (indicator_id, site_id, hyp_id, value, reliability, direction)
    hypothesis_ids: &[String],
    alpha_0: f64,
    lambda: f64,
) -> Result<Vec<Vec<f64>>> {
    let mut out = Vec::with_capacity(site_ids.len());
    for s in site_ids {
        let mut a = vec![alpha_0; hypothesis_ids.len()];
        for (_id, ss, hyp, val, rel, dir) in source_indicators {
            if ss != s {
                continue;
            }
            if let Some(j) = hypothesis_ids.iter().position(|h| h == hyp) {
                let sgn = if dir == "higher_risk" { 1.0 } else { -1.0 };
                a[j] += lambda * rel * sgn * val;
                if a[j] < 1e-3 {
                    a[j] = 1e-3;
                }
            }
        }
        out.push(a);
    }
    Ok(out)
}

/// Draw one Dirichlet sample per site.
pub fn sample(concentrations: &[Vec<f64>], seed: u64) -> Result<Vec<Vec<f64>>> {
    let mut rng = seeded_rng(seed);
    let mut out = Vec::with_capacity(concentrations.len());
    for a in concentrations {
        let d = Dirichlet::new(a.as_slice())
            .map_err(|e| crate::error::BayesDsmError::Invalid(e.to_string()))?;
        let sample = d.sample(&mut rng);
        out.push(sample);
    }
    Ok(out)
}

#[allow(dead_code)]
fn _force_rng() {
    let mut rng = seeded_rng(0);
    let _x: f64 = rng.gen();
}
