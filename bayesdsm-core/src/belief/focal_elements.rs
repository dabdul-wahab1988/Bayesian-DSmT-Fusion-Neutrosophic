//! Focal element helpers.

use std::collections::HashMap;

use crate::dsmt::expression::Set;

pub type Mass = HashMap<Set, f64>;

/// Sum of all masses.
pub fn total(m: &Mass) -> f64 {
    m.values().sum()
}

/// Mass conservation:  masses must sum to 1 within 1e-8 and be non-negative.
pub fn validate(m: &Mass) -> crate::error::Result<()> {
    let s = total(m);
    for (k, v) in m {
        if *v < 0.0 {
            return Err(crate::error::BayesDsmError::Stop {
                module: "belief".into(),
                code: "E1101".into(),
                message: format!("negative mass on focal set {:?} ({})", k, v),
            });
        }
    }
    if (s - 1.0).abs() > 1e-8 {
        return Err(crate::error::BayesDsmError::Stop {
            module: "belief".into(),
            code: "E1102".into(),
            message: format!("belief masses must sum to 1 (got {s})"),
        });
    }
    Ok(())
}
