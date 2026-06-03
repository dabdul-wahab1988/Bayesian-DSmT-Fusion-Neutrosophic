//! Linear-algebra helpers (small fixed-size vectors / matrices used by
//! MCMC and DSmT).  We deliberately keep this small and dependency-free.

/// Sigmoid in the log-sum-exp form:  `exp(a) / sum exp(b)`.  Numerically stable.
pub fn softmax(logits: &[f64]) -> Vec<f64> {
    if logits.is_empty() {
        return vec![];
    }
    let m = logits.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let exps: Vec<f64> = logits.iter().map(|x| (x - m).exp()).collect();
    let z: f64 = exps.iter().sum();
    exps.iter().map(|e| e / z).collect()
}

/// Sample one categorical index from `probs` using a U(0,1) draw `u`.
pub fn categorical(probs: &[f64], u: f64) -> Option<usize> {
    let mut cum = 0.0;
    for (i, p) in probs.iter().enumerate() {
        cum += p;
        if u <= cum {
            return Some(i);
        }
    }
    if probs.is_empty() {
        None
    } else {
        Some(probs.len() - 1)
    }
}

/// log-space sum-exp:  `log(sum exp(xs))` without overflow.
pub fn log_sum_exp(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        return f64::NEG_INFINITY;
    }
    let m = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    if m == f64::NEG_INFINITY {
        return f64::NEG_INFINITY;
    }
    let s: f64 = xs.iter().map(|x| (x - m).exp()).sum();
    m + s.ln()
}
