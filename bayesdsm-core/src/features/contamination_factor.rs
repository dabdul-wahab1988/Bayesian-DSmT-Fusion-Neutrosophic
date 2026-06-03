//! Contamination factor (plan §5.2):  CF_im = geo_mean / B_m.

use std::collections::HashMap;

use rusqlite::Connection;

use crate::error::Result;

pub fn compute(conn: &Connection, run_id: i64) -> Result<Vec<(String, String, f64)>> {
    // Build site x metal -> geo mean from cleaned_metals.
    let mut by: HashMap<(String, String), Vec<f64>> = HashMap::new();
    let mut stmt = conn
        .prepare("SELECT site_id, metal, value_standard FROM cleaned_metals WHERE run_id = ?1")?;
    for r in stmt.query_map([run_id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, f64>(2)?,
        ))
    })? {
        let (s, m, v) = r?;
        by.entry((s, m)).or_default().push(v);
    }

    // Background map.
    let mut bg: HashMap<String, f64> = HashMap::new();
    let mut stmt = conn.prepare("SELECT metal, background_value FROM raw_background_values")?;
    for r in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?)))? {
        let (m, v) = r?;
        bg.insert(m, v);
    }

    let mut out = Vec::new();
    for ((site, metal), xs) in by {
        let gm = geo_mean(&xs);
        if let Some(&b) = bg.get(&metal) {
            if b > 0.0 && gm > 0.0 {
                out.push((site, metal, gm / b));
            }
        }
    }
    Ok(out)
}

pub fn geo_mean(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    let mut s = 0.0;
    for &x in xs {
        if x <= 0.0 {
            return 0.0;
        }
        s += x.ln();
    }
    (s / xs.len() as f64).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geo_mean_basic() {
        let xs = vec![1.0, 2.0, 4.0];
        // geometric mean = 2.0
        assert!((geo_mean(&xs) - 2.0).abs() < 1e-9);
    }
}
