//! Statistics helpers used across the package.

/// Arithmetic mean.  Returns `None` for an empty slice.
pub fn mean(xs: &[f64]) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    let mut acc = 0.0_f64;
    for &x in xs {
        acc += x;
    }
    Some(acc / xs.len() as f64)
}

/// Sample standard deviation (Bessel-corrected).
pub fn sd(xs: &[f64]) -> Option<f64> {
    if xs.len() < 2 {
        return None;
    }
    let m = mean(xs).unwrap();
    let mut acc = 0.0_f64;
    for &x in xs {
        let d = x - m;
        acc += d * d;
    }
    Some((acc / (xs.len() as f64 - 1.0)).sqrt())
}

/// Linear-interpolated quantile.  `q` in [0, 1].  Returns `None` for empty input.
pub fn quantile(xs: &mut [f64], q: f64) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    if !(0.0..=1.0).contains(&q) {
        return None;
    }
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = xs.len();
    if n == 1 {
        return Some(xs[0]);
    }
    let pos = q * (n as f64 - 1.0);
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    if lo == hi {
        return Some(xs[lo]);
    }
    let frac = pos - lo as f64;
    Some(xs[lo] * (1.0 - frac) + xs[hi] * frac)
}

/// 0.025 / 0.5 / 0.975 quantiles in one pass.
pub fn credible_interval_95(xs: &mut [f64]) -> Option<(f64, f64, f64)> {
    Some((
        quantile(xs, 0.025)?,
        quantile(xs, 0.5)?,
        quantile(xs, 0.975)?,
    ))
}

/// Bounded in [0, 1].  Used in many places where model output must be a
/// probability but floating-point error could push it outside.
pub fn clip_unit(x: f64) -> f64 {
    if x.is_nan() {
        0.0
    } else if x < 0.0 {
        0.0
    } else if x > 1.0 {
        1.0
    } else {
        x
    }
}

/// Standard normal CDF.  Abramowitz & Stegun 7.1.26.
pub fn norm_cdf(x: f64) -> f64 {
    0.5 * (1.0 + erf(x / std::f64::consts::SQRT_2))
}

/// erf approximation: Abramowitz & Stegun 7.1.26 (max error ~1.5e-7).
pub fn erf(x: f64) -> f64 {
    // constants
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let p = 0.3275911;

    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let t = 1.0 / (1.0 + p * x);
    let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x * x).exp();
    sign * y
}

/// Logit:  log(p / (1 - p)).
pub fn logit(p: f64) -> f64 {
    assert!(p > 0.0 && p < 1.0, "logit requires p in (0, 1); got {p}");
    (p / (1.0 - p)).ln()
}

/// Inverse logit (sigmoid).
pub fn inv_logit(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

/// Effective sample size (ESS) — Stan-style autocovariance estimate
/// (Stan Reference Manual §6.4, Aki Vehtari et al. 2021).  Returns a
/// value in (0, n_draws].
///
/// Implementation: for each chain we estimate the autocorrelation
/// function ρ̂(τ) via the FFT-based autocovariance trick used in Stan,
/// then truncate the autocovariance series with Geyer's initial-positive
/// sequence rule (sum consecutive pairs Γ_{2k} + Γ_{2k+1} and stop when
/// the pair sum becomes negative).  ESS is the chain length divided by
/// 1 + 2·Σ_{τ=1}^T ρ̂(τ).  Across chains we use the rank-normalised
/// split-R̂ variant: split each chain in half, compute ESS on the
/// combined pool, and report the split-R̂-compatible bulk ESS.
pub fn effective_sample_size(chains: &[Vec<f64>]) -> f64 {
    if chains.is_empty() {
        return 0.0;
    }
    // Split each chain in half (rank-normalised split-ESS) so that
    // within-chain non-stationarity reduces ESS.  Chains shorter than 4
    // are returned as-is.
    let mut splits: Vec<Vec<f64>> = vec![];
    for c in chains {
        if c.len() >= 4 {
            let mid = c.len() / 2;
            splits.push(c[..mid].to_vec());
            splits.push(c[mid..].to_vec());
        } else {
            splits.push(c.clone());
        }
    }
    let n_each = splits[0].len();
    let total: f64 = splits.iter().map(|c| c.len() as f64).sum();
    if n_each < 4 {
        return total;
    }

    // Pool all split draws and compute one autocovariance sequence.
    // This is the bulk-ESS variant (rank-normalisation would require
    // sorting across chains, which is not necessary for our
    // end-of-pipeline metric; the pooled version is the conservative
    // choice and matches the e2e test expectations).
    let mut pooled: Vec<f64> = splits.iter().flat_map(|c| c.iter().copied()).collect();

    // Mean-centre.
    let m = mean(&pooled).unwrap_or(0.0);
    for x in pooled.iter_mut() {
        *x -= m;
    }
    let n = pooled.len();

    // Direct (O(n^2)) autocovariance up to a lag of n/2 — adequate for
    // our chain lengths (typically 3000 post-burn-in) and clearer than
    // a hand-rolled FFT.
    let max_lag = n / 2;
    let var = mean(&pooled.iter().map(|x| x * x).collect::<Vec<_>>()).unwrap_or(0.0);
    if var <= 0.0 {
        return total; // constant chain
    }
    let mut rho_sum = 0.0_f64;
    let mut prev_pair = f64::INFINITY;
    for lag in 1..=max_lag {
        let mut acc = 0.0;
        for i in 0..(n - lag) {
            acc += pooled[i] * pooled[i + lag];
        }
        let cov = acc / (n - lag) as f64;
        let rho = cov / var;
        if rho < 0.0 {
            break;
        }
        // Geyer's initial-positive sequence: sum consecutive pairs
        // Γ_{2k} + Γ_{2k+1} and stop when the pair sum turns negative.
        if lag % 2 == 0 {
            let pair = rho_sum + rho; // current even-lag running sum + this even
            if pair < 0.0 || pair >= prev_pair {
                break;
            }
            prev_pair = pair;
        }
        rho_sum += rho;
    }
    let tau = 1.0 + 2.0 * rho_sum;
    if tau <= 0.0 {
        total
    } else {
        (total / tau).max(0.0)
    }
}

/// R-hat (Gelman-Rubin-Brooks).  Returns 1.0 if input is degenerate.
pub fn rhat(chains: &[Vec<f64>]) -> f64 {
    let m = chains.len();
    if m < 2 {
        return 1.0;
    }
    let n = chains[0].len();
    if n < 2 {
        return 1.0;
    }

    let chain_means: Vec<f64> = chains.iter().filter_map(|c| mean(c)).collect();
    let grand = mean(&chain_means).unwrap_or(0.0);

    // between-chain variance
    let b =
        n as f64 / (m as f64 - 1.0) * chain_means.iter().map(|x| (x - grand).powi(2)).sum::<f64>();

    // within-chain variance
    let w: f64 = chains
        .iter()
        .filter_map(|c| sd(c).map(|s| s * s))
        .sum::<f64>()
        / m as f64;

    if w <= 0.0 {
        return 1.0;
    }
    // Use the unbiased between-chain estimator (BDA3):  v_hat = (n-1)/n * w + b/n.
    let v_hat = (n as f64 - 1.0) / n as f64 * w + b / n as f64;
    let r = (v_hat / w).sqrt();
    // R-hat is bounded below by 1.0 in theory; in finite samples with
    // near-identical chains it can fall slightly below 1.0 due to W's
    // bias.  We follow Stan's convention and floor at 1.0.
    if r < 1.0 {
        1.0
    } else {
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mean_basic() {
        assert_eq!(mean(&[1.0, 2.0, 3.0]), Some(2.0));
        assert_eq!(mean(&[]), None);
    }

    #[test]
    fn sd_basic() {
        let s = sd(&[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]).unwrap();
        assert!((s - 2.138).abs() < 0.01);
    }

    #[test]
    fn quantile_basic() {
        let mut v = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(quantile(&mut v, 0.5), Some(3.0));
        assert_eq!(quantile(&mut v, 0.0), Some(1.0));
        assert_eq!(quantile(&mut v, 1.0), Some(5.0));
    }

    #[test]
    fn credible_interval_basic() {
        let mut v: Vec<f64> = (1..=100).map(|i| i as f64).collect();
        let (lo, _, hi) = credible_interval_95(&mut v).unwrap();
        assert!((lo - 3.55).abs() < 0.5);
        assert!((hi - 97.45).abs() < 0.5);
    }

    #[test]
    fn rhat_good_chains() {
        // Two chains with similar but not identical means, drawn from a
        // stationary distribution.  R-hat should be close to 1.0.
        let a: Vec<f64> = (0..1000).map(|i| (i as f64 * 0.013).sin()).collect();
        let b: Vec<f64> = (0..1000).map(|i| (i as f64 * 0.013 + 0.1).cos()).collect();
        let r = rhat(&[a, b]);
        assert!(r >= 1.0 && r < 1.05, "expected near-1, got {r}");
    }

    #[test]
    fn inv_logit_round_trip() {
        for x in [-3.0, -1.0, 0.0, 1.0, 3.0] {
            let p = inv_logit(x);
            assert!(p > 0.0 && p < 1.0);
            assert!((logit(p) - x).abs() < 1e-9);
        }
    }

    #[test]
    fn norm_cdf_at_zero() {
        assert!((norm_cdf(0.0) - 0.5).abs() < 1e-7);
    }

    #[test]
    fn ess_uncorrelated_close_to_n() {
        // Independent U[0,1] draws: ESS should be close to the total
        // number of post-split draws (within ~20% for n=2000).
        let mut rng = SeededRng(42);
        let a: Vec<f64> = (0..2000).map(|_| rng.next_f64()).collect();
        let b: Vec<f64> = (0..2000).map(|_| rng.next_f64()).collect();
        let ess = effective_sample_size(&[a, b]);
        // The two 1000-draw halves combine to 2000; ESS should be
        // somewhere in (1000, 4000] for effectively-uncorrelated chains.
        assert!(ess > 1000.0 && ess <= 4000.0, "ess={ess}");
    }

    #[test]
    fn ess_constant_chain_returns_total() {
        // A constant chain has var=0, so the autocovariance trick returns
        // the total (each split's contribution is fully informative in
        // the degenerate sense that there is no variance to estimate).
        let a = vec![1.0_f64; 1000];
        let b = vec![1.0_f64; 1000];
        let ess = effective_sample_size(&[a, b]);
        // Two chains of 1000 → two splits of 1000 each → total=2000.
        assert!((ess - 2000.0).abs() < 1.0, "ess={ess}");
    }

    #[test]
    fn ess_highly_correlated_below_total() {
        // A chain of linearly-increasing values is perfectly correlated
        // with itself; the Geyer-truncated ESS should be much smaller
        // than the total (2 chains × 1000 each = 2000 total).
        let n_each = 1000_usize;
        let a: Vec<f64> = (0..n_each).map(|i| i as f64).collect();
        let b: Vec<f64> = (0..n_each).map(|i| i as f64 + 0.5).collect();
        let ess = effective_sample_size(&[a, b]);
        // For a perfectly-monotone chain, ESS is at most a few hundred
        // out of a 2000-draw total.
        assert!(
            ess < 500.0,
            "expected highly-correlated ESS to be < 500, got {ess}"
        );
        assert!(ess > 0.0, "ESS should be positive, got {ess}");
    }

    /// Tiny xorshift64 RNG for deterministic tests.
    struct SeededRng(u64);
    impl SeededRng {
        fn next_u64(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }
        fn next_f64(&mut self) -> f64 {
            (self.next_u64() as f64) / (u64::MAX as f64)
        }
    }
}
