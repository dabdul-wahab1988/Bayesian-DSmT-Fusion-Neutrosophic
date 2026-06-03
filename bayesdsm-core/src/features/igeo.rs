//! Geoaccumulation index (plan §5.4):  Igeo = log2(C / (1.5 * B)).

use std::collections::HashMap;

use rusqlite::Connection;

use crate::error::Result;
use crate::features::contamination_factor::geo_mean;

pub fn compute(conn: &Connection, run_id: i64) -> Result<Vec<(String, String, f64)>> {
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
    let mut bg: HashMap<String, f64> = HashMap::new();
    let mut stmt = conn.prepare("SELECT metal, background_value FROM raw_background_values")?;
    for r in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?)))? {
        let (m, v) = r?;
        bg.insert(m, v);
    }
    let mut out = Vec::new();
    for ((site, metal), xs) in by {
        if let Some(&b) = bg.get(&metal) {
            let gm = geo_mean(&xs);
            if gm > 0.0 && b > 0.0 {
                out.push((site, metal, (gm / (1.5 * b)).log2()));
            }
        }
    }
    Ok(out)
}
