//! Pollution load index (plan §5.5):  PLI_i = (prod CF_im)^{1/M}.

use std::collections::HashMap;

use rusqlite::Connection;

use crate::error::Result;

pub fn compute(conn: &Connection, run_id: i64) -> Result<Vec<(String, f64)>> {
    // We reuse CF from the features table for this run.
    let mut cf_by_site: HashMap<String, HashMap<String, f64>> = HashMap::new();
    let mut stmt = conn.prepare(
        "SELECT site_id, feature_name, feature_value FROM features
         WHERE run_id = ?1 AND feature_family = 'cf'",
    )?;
    for r in stmt.query_map([run_id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, f64>(2)?,
        ))
    })? {
        let (s, name, v) = r?;
        if let Some(metal) = name.strip_prefix("cf_") {
            cf_by_site
                .entry(s)
                .or_default()
                .insert(metal.to_string(), v);
        }
    }
    let mut out = Vec::new();
    for (site, cfs) in cf_by_site {
        let mut log_sum = 0.0;
        let mut n = 0;
        for v in cfs.values() {
            if *v > 0.0 {
                log_sum += v.ln();
                n += 1;
            }
        }
        if n > 0 {
            out.push((site, (log_sum / n as f64).exp()));
        }
    }
    Ok(out)
}
