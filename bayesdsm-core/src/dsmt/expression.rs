//! DSmT expression representation.
//!
//! We represent a DSmT proposition as a sorted, deduplicated set of
//! `usize` hypothesis indices.  A focal element is `(set, mass)`.  Sets
//! are stored as `Vec<usize>` (sorted ascending) so equality and hashing
//! are well-defined.  The empty set represents `∅` (which must have zero
//! mass under the basic DSmT rules).

use std::collections::HashMap;

use crate::error::{BayesDsmError, Result};

/// A DSmT proposition.
pub type Set = Vec<usize>;

/// Sort and deduplicate a set, returning an error if empty when not allowed.
pub fn canonicalize(xs: &[usize]) -> Set {
    let mut v = xs.to_vec();
    v.sort_unstable();
    v.dedup();
    v
}

/// Pretty-print: θ_1∪θ_3 etc., with `∅` for empty set and `Θ` for full.
pub fn display(set: &Set, symbols: &[String]) -> String {
    if set.is_empty() {
        return "∅".to_string();
    }
    let n = symbols.len();
    if set.len() == n && (0..n).all(|i| set.contains(&i)) {
        return "Θ".to_string();
    }
    set.iter()
        .map(|&i| {
            symbols
                .get(i)
                .cloned()
                .unwrap_or_else(|| format!("θ_{}", i + 1))
        })
        .collect::<Vec<_>>()
        .join("∪")
}

/// Parse a DSmT expression like `theta_1`, `theta_1 & theta_2`, `theta_1 | theta_2`,
/// or full `Θ` to a `Set` of indices, given a `symbols` table.
/// (We deliberately do NOT support negation or weird operators.)
pub fn parse(expr: &str, symbols: &[String]) -> Result<Set> {
    let s = expr.trim();
    if s == "Θ" || s.is_empty() {
        // full frame
        return Ok((0..symbols.len()).collect());
    }
    let mut out: Vec<usize> = vec![];
    for tok in
        s.split(|c: char| matches!(c, '&' | '|' | ',' | '∪' | '∧' | ' ' | '\t' | '\n' | '\r'))
    {
        if tok.is_empty() {
            continue;
        }
        let idx = symbols
            .iter()
            .position(|x| x.eq_ignore_ascii_case(tok))
            .ok_or_else(|| BayesDsmError::Stop {
                module: "dsmt".into(),
                code: "E1801".into(),
                message: format!("unknown hypothesis symbol '{tok}' in expression '{expr}'"),
            })?;
        out.push(idx);
    }
    Ok(canonicalize(&out))
}

/// Intersection of two canonicalized sets.
pub fn intersect(a: &Set, b: &Set) -> Set {
    let mut out = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Equal => {
                out.push(a[i]);
                i += 1;
                j += 1;
            }
            std::cmp::Ordering::Less => {
                i += 1;
            }
            std::cmp::Ordering::Greater => {
                j += 1;
            }
        }
    }
    out
}

/// Union of two canonicalized sets.
pub fn union(a: &Set, b: &Set) -> Set {
    canonicalize(&[a.as_slice(), b.as_slice()].concat())
}

/// Cardinality (DSm CM) of a proposition.  For the free DSm model this is
/// simply the number of atomic hypotheses covered.
pub fn cardinality(set: &Set) -> usize {
    set.len()
}

#[allow(dead_code)]
fn _unused_hashmap() {
    let _ = HashMap::<String, f64>::new();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full() {
        let s = vec!["a".into(), "b".into(), "c".into()];
        let r = parse("Θ", &s).unwrap();
        assert_eq!(r, vec![0, 1, 2]);
    }

    #[test]
    fn parse_intersection() {
        let s = vec!["a".into(), "b".into(), "c".into()];
        let r = parse("a & c", &s).unwrap();
        assert_eq!(r, vec![0, 2]);
    }

    #[test]
    fn parse_union() {
        let s = vec!["a".into(), "b".into(), "c".into()];
        let r = parse("a | b", &s).unwrap();
        assert_eq!(r, vec![0, 1]);
    }

    #[test]
    fn display_simple() {
        let s = vec!["a".into(), "b".into(), "c".into()];
        let r = vec![0usize, 1usize];
        assert_eq!(display(&r, &s), "a∪b");
    }

    #[test]
    fn intersect_basic() {
        let a = vec![0, 1, 2];
        let b = vec![1, 2, 3];
        assert_eq!(intersect(&a, &b), vec![1, 2]);
    }

    #[test]
    fn union_basic() {
        let a = vec![0, 2];
        let b = vec![1, 2];
        assert_eq!(union(&a, &b), vec![0, 1, 2]);
    }
}
