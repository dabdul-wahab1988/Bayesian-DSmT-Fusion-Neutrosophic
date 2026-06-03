//! Posterior-to-belief transformation (plan §11).

use std::collections::HashMap;

use crate::dsmt::expression::Set;
use crate::error::Result;
/// Layer tag for `belief_assignments.evidence_layer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Layer {
    Hotspot,
    Source,
    Chemical,
    Exposure,
    Confidence,
}

impl Layer {
    pub fn as_str(self) -> &'static str {
        match self {
            Layer::Hotspot => "hotspot",
            Layer::Source => "source",
            Layer::Chemical => "chemical",
            Layer::Exposure => "exposure",
            Layer::Confidence => "confidence",
        }
    }
}

/// Hotspot belief: frame Θ_H = {High, NotHigh}.  Plan §11.2.
pub fn hotspot_belief(p_hotspot: f64, ci_width: f64) -> Result<HashMap<Set, f64>> {
    let u = ci_width.clamp(0.0, 1.0);
    let m_h = (1.0 - u) * p_hotspot;
    let m_n = (1.0 - u) * (1.0 - p_hotspot);
    let m_u = u;
    let mut out = HashMap::new();
    out.insert(vec![0usize], m_h);
    out.insert(vec![1usize], m_n);
    out.insert(vec![0, 1], m_u);
    Ok(out)
}

/// Source belief: focal elements include singletons, unions, intersections
/// (when allowed), and ignorance.  Plan §11.3.
pub fn source_belief(
    p_source: &[f64],
    u_source: f64,
    delta_single: f64,
    tau_union: f64,
    overlap: &[Vec<f64>], // [h][g] in [0, 1]; if all zeros -> no overlap allowed
    n_hyp: usize,
) -> Result<HashMap<Set, f64>> {
    // Raw focal scores r(A).
    let mut r: HashMap<Set, f64> = HashMap::new();
    // Singletons
    for h in 0..n_hyp {
        let max_other = p_source
            .iter()
            .enumerate()
            .filter(|(g, _)| *g != h)
            .map(|(_, v)| *v)
            .fold(f64::NEG_INFINITY, f64::max);
        let sc = ((p_source[h] - max_other) * delta_single).max(0.0);
        if sc > 0.0 {
            r.insert(vec![h], sc);
        }
    }
    // Union support
    let u_set: Vec<usize> = (0..n_hyp).filter(|&h| p_source[h] >= tau_union).collect();
    if u_set.len() >= 2 {
        let avg: f64 = u_set.iter().map(|&h| p_source[h]).sum::<f64>() / u_set.len() as f64;
        if avg > 0.0 {
            r.insert(u_set.clone(), avg);
        }
    }
    // Intersection support
    for h in 0..n_hyp {
        for g in (h + 1)..n_hyp {
            if overlap[h][g] > 0.0 {
                let sc = overlap[h][g] * p_source[h].min(p_source[g]);
                if sc > 0.0 {
                    r.insert(vec![h, g], sc);
                }
            }
        }
    }
    // Ignorance
    let eps = 1e-6_f64;
    r.insert((0..n_hyp).collect(), eps);

    if r.is_empty() {
        r.insert((0..n_hyp).collect(), 1.0);
    }
    // Normalize to (1 - u), then add u to ignorance.
    let sum: f64 = r.values().sum();
    let mut masses: HashMap<Set, f64> = HashMap::new();
    for (k, v) in &r {
        masses.insert(k.clone(), (1.0 - u_source) * v / sum);
    }
    let ig_key: Set = (0..n_hyp).collect();
    let prev_ig = masses.get(&ig_key).copied().unwrap_or(0.0);
    masses.insert(ig_key, prev_ig + u_source);
    Ok(masses)
}

/// Chemical-fingerprint belief (plan §11.4).  For each site, the
/// `metal_to_hypothesis` map tells us which hypotheses a given metal
/// is diagnostic of.  Given the per-metal enrichment indicators (EF
/// exceeding `enrichment_threshold` ⇒ the metal "votes" for its
/// diagnostic hypotheses), the focal set is the union of all
/// hypotheses that received at least one vote, and the focal mass is
/// the proportion of metals at the site that voted (relative to the
/// total metal count).  Uncertainty `u_chem` flows to the full frame.
///
/// If `metal_to_hypothesis` is empty (i.e. no diagnostic mapping was
/// supplied), the chemical layer contributes only the ignorance mass.
pub fn chemical_belief(
    efs: &[(String, f64)], // (metal, EF value)
    t_e: f64,
    u_chem: f64,
    metal_to_hyp: &HashMap<String, usize>,
    n_hyp: usize,
) -> Result<HashMap<Set, f64>> {
    if efs.is_empty() || n_hyp == 0 {
        let mut m = HashMap::new();
        m.insert((0..n_hyp).collect(), 1.0);
        return Ok(m);
    }
    let mut votes: HashMap<usize, f64> = HashMap::new(); // h -> total weight
    let mut total = 0.0_f64;
    for (metal, ef) in efs {
        if *ef > t_e {
            total += 1.0;
            if let Some(&h) = metal_to_hyp.get(metal) {
                *votes.entry(h).or_insert(0.0) += 1.0;
            }
        }
    }
    let mut masses: HashMap<Set, f64> = HashMap::new();
    if total > 0.0 && !votes.is_empty() {
        // One focal set per hypothesis that received votes; mass is
        // proportional to its vote share.
        let v_sum: f64 = votes.values().sum();
        for (h, w) in &votes {
            masses.insert(vec![*h], (1.0 - u_chem) * w / v_sum);
        }
        let ig_key: Set = (0..n_hyp).collect();
        let prev_ig = masses.get(&ig_key).copied().unwrap_or(0.0);
        masses.insert(ig_key, prev_ig + u_chem);
    } else {
        // No diagnostic votes: contribute only ignorance (full frame = 1).
        let ig_key: Set = (0..n_hyp).collect();
        masses.insert(ig_key, 1.0);
    }
    Ok(masses)
}

/// Exposure belief (plan §11.5).  The frame is the set of hypothesis
/// indices; the focal set for each hypothesis is the singleton
/// {h}, with mass proportional to the risk weight for that
/// hypothesis, gated by the (normalized) exposure magnitude
/// `exposure_norm`.  Uncertainty `u_exp` flows to the full frame.
/// `exposure_norm` is used as a binary gate: if it is below
/// `exposure_gate` (default 0.0 ⇒ all positive exposures count),
/// the layer contributes only ignorance; otherwise the singleton
/// mass on {h} is `(1 - u_exp) · w_h / Σ w`.  In effect, the
/// exposure layer is a re-weighting of the prior, with the
/// exposure magnitude acting as a *gate* rather than a continuous
/// scaling (a continuous scaling would break the focal-set
/// conservation constraint Σ m = 1 unless the layer is rescaled,
/// which is not what the rest of the pipeline expects).
pub fn exposure_belief(
    exposure_norm: f64,   // in [0, 1]
    risk_weights: &[f64], // length n_hyp, each in [0, 1]
    u_exp: f64,
    n_hyp: usize,
) -> Result<HashMap<Set, f64>> {
    let e = exposure_norm.clamp(0.0, 1.0);
    let wsum: f64 = risk_weights.iter().sum();
    let mut masses: HashMap<Set, f64> = HashMap::new();
    let ig_key: Set = (0..n_hyp).collect();
    if e > 0.0 && wsum > 0.0 {
        for h in 0..n_hyp {
            let m = (1.0 - u_exp) * risk_weights[h] / wsum;
            if m > 0.0 {
                masses.insert(vec![h], m);
            }
        }
        let prev_ig = masses.get(&ig_key).copied().unwrap_or(0.0);
        masses.insert(ig_key.clone(), prev_ig + u_exp);
    } else {
        // No exposure signal: contribute only ignorance.
        masses.insert(ig_key.clone(), 1.0);
    }
    // Degenerate guard: if everything is zero (no exposure, no risk weights),
    // emit a single full-frame mass of 1.0 so the focal set conservation
    // check (Σ m = 1) still passes.
    if masses.values().sum::<f64>() <= 0.0 {
        masses.clear();
        masses.insert(ig_key, 1.0);
    }
    Ok(masses)
}

/// Confidence belief (plan §11.6).  Two-element frame: {confident, not_confident}.
/// The frame is encoded as a 2-element set in the hypothesis space, but
/// confidence is about the *measurement* itself, not the hypothesis.
/// We expose it via two focal elements: {0} (confident) with mass
/// (1 - u_conf) · conf, and {1} (not_confident) with mass (1 - u_conf) · (1 - conf).
/// Uncertainty flows to {0, 1}.
pub fn confidence_belief(conf: f64, u_conf: f64) -> Result<HashMap<Set, f64>> {
    let c = conf.clamp(0.0, 1.0);
    let u = u_conf.clamp(0.0, 1.0);
    let mut masses: HashMap<Set, f64> = HashMap::new();
    masses.insert(vec![0usize], (1.0 - u) * c);
    masses.insert(vec![1usize], (1.0 - u) * (1.0 - c));
    masses.insert(vec![0usize, 1], u);
    Ok(masses)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chemical_basic() {
        // Two metals, only "As" is enriched and diagnostic of H1.
        let efs = vec![("As".to_string(), 2.5), ("Pb".to_string(), 1.0)];
        let mut map = HashMap::new();
        map.insert("As".to_string(), 0_usize);
        let m = chemical_belief(&efs, 1.5, 0.1, &map, 3).unwrap();
        // Focal {0} has (1 - 0.1) * 1.0 = 0.9, full frame has 0.1.
        assert!((m[&vec![0_usize]] - 0.9).abs() < 1e-9);
        assert!((m[&vec![0, 1, 2]] - 0.1).abs() < 1e-9);
    }

    #[test]
    fn chemical_no_votes_is_ignorance() {
        // No EF exceeds threshold → only ignorance.
        let efs = vec![("As".to_string(), 1.0), ("Pb".to_string(), 0.5)];
        let mut map = HashMap::new();
        map.insert("As".to_string(), 0_usize);
        let m = chemical_belief(&efs, 1.5, 0.2, &map, 2).unwrap();
        // Only one entry, the full frame, with mass 1.0 (u + fallback).
        let sum: f64 = m.values().sum();
        assert!((sum - 1.0).abs() < 1e-9);
    }

    #[test]
    fn exposure_basic() {
        // Risk weights: hypothesis 0 has the highest weight, so it
        // receives the largest belief mass when exposure is positive.
        let w = vec![0.5, 0.2, 0.3];
        let m = exposure_belief(0.8, &w, 0.1, 3).unwrap();
        // 0.9 * 0.8 * w_h / 1.0; the largest mass is on hypothesis 0 (0.5).
        assert!(m[&vec![0_usize]] > m[&vec![1_usize]]);
        assert!(m[&vec![0_usize]] > m[&vec![2_usize]]);
        let sum: f64 = m.values().sum();
        assert!((sum - 1.0).abs() < 1e-9);
    }

    #[test]
    fn confidence_basic() {
        let m = confidence_belief(0.7, 0.2).unwrap();
        // {0}=0.56, {1}=0.24, {0,1}=0.20
        assert!((m[&vec![0_usize]] - 0.56).abs() < 1e-9);
        assert!((m[&vec![1_usize]] - 0.24).abs() < 1e-9);
        assert!((m[&vec![0_usize, 1]] - 0.20).abs() < 1e-9);
    }
}
