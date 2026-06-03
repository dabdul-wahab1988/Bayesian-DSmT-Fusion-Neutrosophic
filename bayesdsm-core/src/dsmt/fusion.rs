//! DSmT conjunctive fusion (plan §12).
//!
//! For each site, given L focal assignments (one per evidence layer), the
//! conjunctive DSm rule computes:
//!   m_fused(C) += prod m_l(A_l)   over all tuples (A_1, ..., A_L) such
//!   that A_1 ∩ ... ∩ A_L = C.
//!
//! If the intersection is empty under the chosen model, mass is transferred
//! to A_1 ∪ ... ∪ A_L, and the conflict mass K accumulates.

use std::collections::HashMap;

use crate::dsmt::canonicalize::{canonical_intersect, DsmtModel};
use crate::dsmt::expression::{union, Set};

pub type Focal = (Set, f64);

#[derive(Debug, Default)]
pub struct FusionResult {
    /// fused_mass[set] = mass; the set is canonical.
    pub fused_mass: HashMap<Vec<usize>, f64>,
    pub conflict: f64,
}

/// `layers` is a vector of focal lists, one per evidence layer for a site.
pub fn fuse(
    layers: &[Vec<Focal>],
    model: &DsmtModel,
    constraints: &crate::dsmt::canonicalize::Constraints,
    symbols: &[String],
) -> FusionResult {
    if layers.is_empty() {
        return FusionResult::default();
    }
    // Cartesian product of focal indices.
    let mut tuples: Vec<Vec<usize>> = vec![vec![]];
    for l in layers {
        let mut next = Vec::new();
        for prefix in &tuples {
            for (i, _) in l.iter().enumerate() {
                let mut p = prefix.clone();
                p.push(i);
                next.push(p);
            }
        }
        tuples = next;
    }
    let mut out: HashMap<Vec<usize>, f64> = HashMap::new();
    let mut conflict = 0.0;
    for t in &tuples {
        let mut prod = 1.0;
        let mut cur: Option<Set> = None;
        let mut full_union: Option<Set> = None;
        for (li, idx) in t.iter().enumerate() {
            let (set, m) = &layers[li][*idx];
            prod *= m;
            match cur {
                None => {
                    cur = Some(set.clone());
                    full_union = Some(set.clone());
                }
                Some(c) => {
                    let new_full = union(&c, set);
                    full_union = Some(new_full.clone());
                    match canonical_intersect(&c, set, model, constraints, symbols) {
                        Some(r) => cur = Some(r),
                        None => {
                            // conflict — break out and route to union
                            cur = None;
                            break;
                        }
                    }
                }
            }
        }
        if let Some(c) = cur {
            *out.entry(c).or_insert(0.0) += prod;
        } else {
            // transfer-to-union (plan §12.1 step 6): when the intersection
            // is forced empty, the mass w is added to the union U of the
            // participating focal sets.  The conflict is *also* tracked
            // in `conflict` so downstream consumers can report it
            // (e.g. in the `conflict_mass` column of `dsmt_fusion`).
            let u = full_union.unwrap_or_default();
            *out.entry(u).or_insert(0.0) += prod;
            conflict += prod;
        }
    }
    // Per plan §12.1 final mass check: `Σ m_fused(C) = 1`.  When the
    // per-layer mass functions don't sum to 1 (e.g. a layer with a
    // single focal of mass 0.6), the conjunctive product is strictly
    // less than 1, and the residual `1 - Σ m_fused` is the "uncommitted"
    // mass — it is not conflict (which arises from incompatibilities
    // *between* layers) but from incompleteness in a single layer.
    // Following §12.1 step 6, any unfocused mass is transferred to the
    // full frame Θ (the union of all hypothesis indices present in
    // `symbols`); in the free DSm model Θ is the unique greatest
    // element of D^Θ, so this is well-defined and mass-conserving.
    let produced: f64 = out.values().sum();
    if produced < 1.0 {
        let theta: Vec<usize> = (0..symbols.len()).collect();
        if !theta.is_empty() {
            *out.entry(theta).or_insert(0.0) += 1.0 - produced;
        }
    } else if produced > 1.0 + 1e-9 {
        // Pathological: layers' focals summed to > 1.  This is a
        // contract violation (each layer's masses must sum to ≤ 1);
        // surface as a large conflict and let the orchestrator STOP.
        conflict += produced - 1.0;
    }
    FusionResult {
        fused_mass: out,
        conflict,
    }
}

/// In-place normalization to make masses sum to 1.
pub fn normalize_in_place(r: &mut FusionResult) {
    let s: f64 = r.fused_mass.values().sum();
    if s > 0.0 {
        for v in r.fused_mass.values_mut() {
            *v /= s;
        }
        r.conflict /= s;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsmt::canonicalize::DsmtModel;

    #[test]
    fn simple_consensus() {
        // Two layers, each with a single focal {0} with mass 0.6 and 0.7.
        // Intersection = {0}.  Fused mass on {0} = 0.6 * 0.7 = 0.42.
        // Plan §12.1 step 6:  after the conjunctive sum, conflict
        // (none here) is transferred to the union of the focals.  The
        // total mass remains 1, with 0.42 on {0} and 0.58 on {0,1} (= Θ).
        let l1 = vec![(vec![0usize], 0.6_f64)];
        let l2 = vec![(vec![0usize], 0.7_f64)];
        let r = fuse(
            &[l1, l2],
            &DsmtModel::Free,
            &Default::default(),
            &vec!["a".into(), "b".into()],
        );
        let total: f64 = r.fused_mass.values().sum();
        assert!((total - 1.0).abs() < 1e-9, "total={total}");
        assert!((r.fused_mass[&vec![0usize]] - 0.42).abs() < 1e-9);
        assert!((r.fused_mass[&vec![0usize, 1]] - 0.58).abs() < 1e-9);
    }
}
