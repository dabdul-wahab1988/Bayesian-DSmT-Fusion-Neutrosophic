//! Free DSm model.  Masses can be assigned to any element of D^Θ.

use crate::dsmt::expression::Set;

/// Focal list = (set, mass) pairs.  `set` is canonical (sorted, deduped).
pub type Focals = Vec<(Set, f64)>;

/// Make the union focal Θ (set of all atomic indices).
pub fn theta(n: usize) -> Set {
    (0..n).collect()
}

#[allow(dead_code)]
pub fn _unused() {}
