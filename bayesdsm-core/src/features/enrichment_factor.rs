//! Enrichment factor (plan §5.3).

use std::collections::HashMap;

use rusqlite::Connection;

use crate::error::Result;
use crate::features::contamination_factor::geo_mean;

pub fn compute(conn: &Connection, run_id: i64) -> Result<Vec<(String, String, f64)>> {
    // Per-site, per-metal geometric mean from cleaned_metals.
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

    // Reference element: from config; default "Al".  Optional.
    let ref_elem = {
        let v: Option<String> = conn
            .query_row(
                "SELECT parameter_value FROM config WHERE parameter_name = 'reference_element'",
                [],
                |r| r.get(0),
            )
            .ok();
        v.unwrap_or_else(|| "Al".to_string())
    };

    // Pull per-site reference value (geo mean of ref element concentrations).
    let mut ref_value: HashMap<String, f64> = HashMap::new();
    for ((s, m), xs) in &by {
        if *m == ref_elem {
            ref_value.insert(s.clone(), geo_mean(xs));
        }
    }
    let bg_ref = bg.get(&ref_elem).copied();

    let mut out = Vec::new();
    for ((site, metal), xs) in by {
        if metal == ref_elem {
            continue;
        }
        let gm = geo_mean(&xs);
        let b = match bg.get(&metal) {
            Some(v) => *v,
            None => continue,
        };
        let v = match (&bg_ref, ref_value.get(&site)) {
            (Some(br), Some(rv)) if *br > 0.0 && *rv > 0.0 => (gm / rv) / (b / br),
            _ => gm / b, // EF ≡ CF if no reference element available (plan §5.3 fallback)
        };
        out.push((site, metal, v));
    }
    Ok(out)
}
