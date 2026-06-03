//! Centralised STOP / WARN / DOWNGRADE rule registry.
//!
//! Every module that mutates state should call into this module at its
//! boundary to validate numerical invariants from `plan.txt` §17 and the
//! package-level STOP rules in `primary_input_contract.md` §12.

use rusqlite::{params, Connection};

use crate::error::{BayesDsmError, Result};

/// Stop the workflow with a typed error.
pub fn stop(module: &str, code: &str, message: impl Into<String>) -> BayesDsmError {
    BayesDsmError::Stop {
        module: module.to_string(),
        code: code.to_string(),
        message: message.into(),
    }
}

/// §17.1 — cleaned concentration must be positive (unless non-detect with valid DL).
pub fn check_cleaned_positive(value: f64, detect_flag: i64, dl: Option<f64>) -> Result<()> {
    if value <= 0.0 {
        if detect_flag == 0 && dl.unwrap_or(0.0) > 0.0 {
            return Ok(());
        }
        return Err(stop(
            "clean",
            "E1701",
            format!("cleaned value must be > 0 (got {value})"),
        ));
    }
    Ok(())
}

/// §17.2 — background must be > 0.
pub fn check_background_positive(b: f64) -> Result<()> {
    if b <= 0.0 {
        return Err(stop(
            "ingest",
            "E1702",
            format!("background must be > 0 (got {b})"),
        ));
    }
    Ok(())
}

/// §17.3 — weights must sum to 1 within 1e-8.
pub fn check_weights_sum_to_one(weights: &[(String, f64)]) -> Result<()> {
    let s: f64 = weights.iter().map(|(_, w)| w).sum();
    if (s - 1.0).abs() > 1e-8 {
        return Err(stop(
            "neutrosophic",
            "E1703",
            format!("weights must sum to 1 (got {s})"),
        ));
    }
    for (c, w) in weights {
        if *w < 0.0 {
            return Err(stop(
                "neutrosophic",
                "E1703",
                format!("weight for criterion '{c}' is negative ({w})"),
            ));
        }
    }
    Ok(())
}

/// §17.4 — posterior probability must lie in [0, 1].
pub fn check_probability(p: f64, what: &str) -> Result<()> {
    if !(0.0..=1.0).contains(&p) {
        return Err(stop(
            "bayes",
            "E1704",
            format!("{what} must be in [0,1] (got {p})"),
        ));
    }
    Ok(())
}

/// §17.5 — belief mass validation.  All masses non-negative, sum to 1.
pub fn check_mass_conservation(masses: &[(String, f64)], module: &str) -> Result<()> {
    let s: f64 = masses.iter().map(|(_, m)| m).sum();
    for (a, m) in masses {
        if *m < 0.0 {
            return Err(stop(
                module,
                "E1705",
                format!("belief mass for '{a}' is negative ({m})"),
            ));
        }
    }
    if (s - 1.0).abs() > 1e-8 {
        return Err(stop(
            module,
            "E1705",
            format!("belief masses must sum to 1 (got {s})"),
        ));
    }
    Ok(())
}

/// §17.6 — DSmT expression must be in the hyper-power set.  The expression
/// parser in `dsmt::expression` is responsible for producing canonical
/// expressions; this is the final post-canonicalize gate.
pub fn check_dsmt_expression_nonempty(expr: &str) -> Result<()> {
    if expr.is_empty() || expr == "∅" {
        return Err(stop(
            "dsmt",
            "E1706",
            "DSmT expression is empty after canonicalization",
        ));
    }
    Ok(())
}

/// §17.7 — neutrosophic memberships must lie in [0, 1].
pub fn check_neutrosophic(t: f64, f: f64, i: f64) -> Result<()> {
    for (name, v) in [("T", t), ("F", f), ("I", i)] {
        if !(0.0..=1.0).contains(&v) {
            return Err(stop(
                "neutrosophic",
                "E1707",
                format!("{name} must be in [0,1] (got {v})"),
            ));
        }
    }
    Ok(())
}

/// §17.8 — priority score must be finite.
pub fn check_priority_finite(p: f64) -> Result<()> {
    if !p.is_finite() {
        return Err(stop(
            "rank",
            "E1708",
            format!("priority score is not finite ({p})"),
        ));
    }
    Ok(())
}

/// §9 — R-hat diagnostics.  Returns severity (None, Some("WARN"), Some("DOWNGRADE")).
pub fn rhat_severity(rhat: f64) -> Option<&'static str> {
    if rhat > 1.05 {
        Some("DOWNGRADE")
    } else if rhat > 1.01 {
        Some("WARN")
    } else {
        None
    }
}

/// Insert a warning row.  Helper to keep warnings consistent.
pub fn insert_warning(
    conn: &Connection,
    run_id: i64,
    module: &str,
    severity: &str,
    code: &str,
    message: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO warnings (run_id, module, severity, code, message)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![run_id, module, severity, code, message],
    )?;
    Ok(())
}

/// Insert a failure row.  The caller is expected to also return a `Stop` error.
pub fn insert_failure(
    conn: &Connection,
    run_id: i64,
    module: &str,
    code: &str,
    message: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO failures (run_id, module, code, message) VALUES (?1, ?2, ?3, ?4)",
        params![run_id, module, code, message],
    )?;
    Ok(())
}
