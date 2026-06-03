//! Non-detect handling (plan §4.2).
//!
//! Two modes are supported:
//! - `dl_sqrt2` (Mode A): y_clean = DL / sqrt(2)
//! - `dl_half`  (Mode A variant): y_clean = DL / 2
//! - `censored_bayes` (Mode B): defers to the Bayesian model by inserting a
//!   censored-likelihood term; here we return a placeholder `y = DL/2` for
//!   the cleaned value and record a `censored` flag so the Bayesian module
//!   can pick it up.

use crate::error::{BayesDsmError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    DlSqrt2,
    DlHalf,
    CensoredBayes,
}

pub fn parse_mode(s: &str) -> Result<Mode> {
    match s.trim() {
        "dl_sqrt2" => Ok(Mode::DlSqrt2),
        "dl_half" => Ok(Mode::DlHalf),
        "censored_bayes" => Ok(Mode::CensoredBayes),
        other => Err(BayesDsmError::Stop {
            module: "clean".into(),
            code: "E1403".into(),
            message: format!("unknown nondetect_method '{other}'"),
        }),
    }
}

pub fn substitute(detect_flag: i64, value: f64, dl: Option<f64>, mode: Mode) -> Result<f64> {
    if detect_flag == 1 {
        if value <= 0.0 {
            return Err(BayesDsmError::Stop {
                module: "clean".into(),
                code: "E1404".into(),
                message: "detected value must be > 0".into(),
            });
        }
        return Ok(value);
    }
    // Non-detect
    let d = dl.ok_or_else(|| BayesDsmError::Stop {
        module: "clean".into(),
        code: "E1405".into(),
        message: "non-detect without detection_limit".into(),
    })?;
    if d <= 0.0 {
        return Err(BayesDsmError::Stop {
            module: "clean".into(),
            code: "E1405".into(),
            message: "non-detect with non-positive detection_limit".into(),
        });
    }
    Ok(match mode {
        Mode::DlSqrt2 => d / 2f64.sqrt(),
        Mode::DlHalf => d / 2.0,
        Mode::CensoredBayes => d / 2.0, // placeholder; Bayesian module uses censoring.
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detected_passthrough() {
        assert_eq!(substitute(1, 5.0, None, Mode::DlSqrt2).unwrap(), 5.0);
    }

    #[test]
    fn dl_sqrt2() {
        let v = substitute(0, 0.0, Some(2.0), Mode::DlSqrt2).unwrap();
        assert!((v - 2.0f64.sqrt()).abs() < 1e-12);
    }

    #[test]
    fn dl_half() {
        assert_eq!(substitute(0, 0.0, Some(2.0), Mode::DlHalf).unwrap(), 1.0);
    }

    #[test]
    fn missing_dl_stop() {
        assert!(substitute(0, 0.0, None, Mode::DlSqrt2).is_err());
    }
}
