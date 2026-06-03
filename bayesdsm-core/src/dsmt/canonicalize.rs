//! Canonicalization of DSmT expressions (free and hybrid models).
//!
//! - In the **free** model, intersections of distinct atomic hypotheses are
//!   NOT collapsed — they remain in the focal set as their own element.
//! - In the **hybrid** model, expressions declared as `empty` in
//!   `raw_dsmt_constraints` are forced to `∅`; on encountering them, the
//!   intersection result is replaced by `∅` and the calling fusion code
//!   routes the mass to the union (transfer-to-union).
//! - The **shafer** model is just the free model restricted so that any
//!   focal element with |set| > 1 is added to the union focal `Θ`.

use std::collections::HashMap;

use crate::dsmt::expression::{intersect, Set};
use crate::error::Result;

pub enum DsmtModel {
    Free,
    Hybrid,
    Shafer,
}

pub fn parse_model(s: &str) -> Result<DsmtModel> {
    match s.trim() {
        "free" => Ok(DsmtModel::Free),
        "hybrid" => Ok(DsmtModel::Hybrid),
        "shafer" => Ok(DsmtModel::Shafer),
        other => Err(crate::error::BayesDsmError::Stop {
            module: "dsmt".into(),
            code: "E1802".into(),
            message: format!("unknown dsmt_model '{other}'"),
        }),
    }
}

/// Constraints table:  expression -> constraint_type.
pub type Constraints = HashMap<String, String>;

/// Returns Some(set) if the intersection of `a` and `b` is allowed under the
/// given model; None if the result collapses to ∅ (the strict empty set
/// enforced by an `empty` constraint in the hybrid model).
pub fn canonical_intersect(
    a: &Set,
    b: &Set,
    model: &DsmtModel,
    constraints: &Constraints,
    symbols: &[String],
) -> Option<Set> {
    let r = intersect(a, b);
    match model {
        DsmtModel::Free => {
            // Free DSm model: any focal element is allowed, including
            // composites.  If a and b are disjoint atomic hypotheses their
            // intersection result is empty, which under free DSmT is
            // interpreted as the composite focal element A∪B (the union).
            if r.is_empty() {
                let mut u = a.clone();
                u.extend(b.iter().copied());
                u.sort_unstable();
                u.dedup();
                Some(u)
            } else {
                Some(r)
            }
        }
        DsmtModel::Hybrid => {
            // In the hybrid model, intersection of disjoint atoms is the
            // composite focal A&B (their union), then we look up the
            // constraint key against that composite.
            let composite = if r.is_empty() {
                let mut u = a.clone();
                u.extend(b.iter().copied());
                u.sort_unstable();
                u.dedup();
                u
            } else {
                r
            };
            let key = composite
                .iter()
                .map(|&i| symbols[i].clone())
                .collect::<Vec<_>>()
                .join("&");
            if constraints.get(&key).map(|v| v == "empty").unwrap_or(false) {
                return None;
            }
            Some(composite)
        }
        DsmtModel::Shafer => {
            // Shafer restricts to singletons + Θ; any multi-atom focal
            // (whether composite or disjoint) collapses to Θ.
            if r.is_empty() {
                Some((0..symbols.len()).collect())
            } else if r.len() > 1 {
                Some((0..symbols.len()).collect())
            } else {
                Some(r)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsmt::expression;

    #[test]
    fn free_model_keeps_intersection() {
        let s = vec!["a".into(), "b".into()];
        let r = canonical_intersect(&vec![0], &vec![1], &DsmtModel::Free, &HashMap::new(), &s);
        assert_eq!(r, Some(vec![0, 1]));
    }

    #[test]
    fn hybrid_respects_empty_constraint() {
        let s = vec!["a".into(), "b".into()];
        let mut c = HashMap::new();
        c.insert("a&b".to_string(), "empty".to_string());
        let r = canonical_intersect(&vec![0], &vec![1], &DsmtModel::Hybrid, &c, &s);
        assert!(r.is_none());
    }

    #[test]
    fn shafer_collapses_to_full() {
        let s = vec!["a".into(), "b".into()];
        let r = canonical_intersect(&vec![0], &vec![1], &DsmtModel::Shafer, &HashMap::new(), &s);
        assert_eq!(r, Some(vec![0, 1]));
        // Already full; if we then intersect with a subset:
        let r2 = canonical_intersect(
            &r.unwrap(),
            &vec![0],
            &DsmtModel::Shafer,
            &HashMap::new(),
            &s,
        );
        assert_eq!(r2, Some(vec![0]));
    }

    #[test]
    fn expression_parse_smoke() {
        let s = vec!["a".into(), "b".into(), "c".into()];
        let _ = expression::parse("a&b|c", &s).unwrap();
    }
}
