//! Hybrid DSm model.  Adds a constraints table (raw_dsmt_constraints) and
//! consults it during fusion.

use std::collections::HashMap;

use crate::dsmt::canonicalize::Constraints;

pub fn load_constraints(rows: &[(String, String)]) -> Constraints {
    rows.iter()
        .map(|(e, t)| (e.clone(), t.clone()))
        .collect::<HashMap<_, _>>()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn load_empty() {
        let m = load_constraints(&[]);
        assert!(m.is_empty());
    }
}
