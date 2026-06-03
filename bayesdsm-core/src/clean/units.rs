//! Unit harmonization (plan §4.1).
//!
//! Recognised input units: `mg/kg`, `µg/kg`, `ug/kg`, `g/kg`, `ppm`, `ppb`.
//! Output unit: the value of `config.standard_concentration_unit` (default
//! `mg/kg`).  Unknown units cause STOP.

use crate::error::{BayesDsmError, Result};

/// Compute the conversion factor κ_u to go from `from` to `to`.
pub fn kappa(from: &str, to: &str) -> Result<f64> {
    let f = unit_to_mg_per_kg(from)?;
    let t = unit_to_mg_per_kg(to)?;
    if t == 0.0 {
        return Err(BayesDsmError::Stop {
            module: "clean".into(),
            code: "E1401".into(),
            message: "target unit has zero mass per kg".into(),
        });
    }
    Ok(f / t)
}

fn unit_to_mg_per_kg(u: &str) -> Result<f64> {
    match u.trim() {
        "mg/kg" => Ok(1.0),
        "µg/kg" | "ug/kg" => Ok(1e-3),
        "g/kg" => Ok(1000.0),
        "ppm" => Ok(1.0),  // ppm ≈ mg/kg for sediment
        "ppb" => Ok(1e-3), // ppb ≈ µg/kg
        other => Err(BayesDsmError::Stop {
            module: "clean".into(),
            code: "E1402".into(),
            message: format!("unrecognised unit '{other}'"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ug_kg_to_mg_kg() {
        assert!((kappa("ug/kg", "mg/kg").unwrap() - 1e-3).abs() < 1e-12);
    }

    #[test]
    fn g_kg_to_mg_kg() {
        assert!((kappa("g/kg", "mg/kg").unwrap() - 1000.0).abs() < 1e-9);
    }

    #[test]
    fn mg_kg_to_mg_kg() {
        assert!((kappa("mg/kg", "mg/kg").unwrap() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn unknown_unit() {
        assert!(kappa("foo/bar", "mg/kg").is_err());
    }
}
