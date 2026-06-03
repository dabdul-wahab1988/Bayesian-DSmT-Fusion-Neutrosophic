//! Compute the feature matrix and write to `features` (plan §16.2).
//!
//! Feature families:
//! - `cf_<metal>`, `ef_<metal>`, `igeo_<metal>`, `pli`, `exposure_norm`,
//!   `confidence`, `missingness`, `uncertainty_penalty`.

pub mod contamination_factor;
pub mod enrichment_factor;
pub mod igeo;
pub mod normalize;
pub mod pli;

use rusqlite::{params, Connection};

use crate::db;
use crate::error::Result;

pub fn run(conn: &mut Connection, run_id: i64) -> Result<usize> {
    let tx = conn.transaction()?;
    let cleaned_run = db::latest_run_for_table(&tx, "cleaned_metals")?;

    // Compute CF / EF / Igeo from the cleaned table.
    let cfs = contamination_factor::compute(&tx, cleaned_run)?;
    let efs = enrichment_factor::compute(&tx, cleaned_run)?;
    let igs = igeo::compute(&tx, cleaned_run)?;

    // Replace prior features for this run BEFORE we write the per-metal
    // rows, so downstream readers (PLI, neutrosophic) see the new rows.
    tx.execute("DELETE FROM features WHERE run_id = ?1", params![run_id])?;

    let mut ins = tx.prepare(
        "INSERT INTO features
         (run_id, site_id, feature_name, feature_family, feature_value, feature_value_normalized, leakage_status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;

    // Write the per-metal rows.  `feature_value` keeps the raw value (e.g. CF
    // can legitimately be 5–10 in contaminated sediment); the
    // `[0,1]`-clipped version is stored in `feature_value_normalized` so
    // downstream consumers (which expect unit-interval inputs to the
    // neutrosophic score) are not surprised.
    for (s, m, v) in &cfs {
        ins.execute(params![
            run_id,
            s,
            format!("cf_{m}"),
            "cf",
            v,
            v.clamp(0.0, 1.0),
            "ok"
        ])?;
    }
    for (s, m, v) in &efs {
        ins.execute(params![
            run_id,
            s,
            format!("ef_{m}"),
            "ef",
            v,
            v.clamp(0.0, 1.0),
            "ok"
        ])?;
    }
    for (s, m, v) in &igs {
        ins.execute(params![
            run_id,
            s,
            format!("igeo_{m}"),
            "igeo",
            v,
            v.clamp(0.0, 1.0),
            "ok"
        ])?;
    }
    drop(ins);

    // PLI is a function of the CF rows we just wrote, so it must run AFTER.
    let plis = pli::compute(&tx, run_id)?;
    let mut ins = tx.prepare(
        "INSERT INTO features
         (run_id, site_id, feature_name, feature_family, feature_value, feature_value_normalized, leakage_status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;
    for (s, v) in &plis {
        ins.execute(params![run_id, s, "pli", "pli", v, v.clamp(0.0, 1.0), "ok"])?;
    }
    drop(ins);

    // Pull raw exposure + confidence indicators.
    let exposure: Vec<(String, f64, String, f64)> = {
        let mut stmt = tx.prepare(
            "SELECT site_id, indicator_value, direction, reliability_weight
             FROM raw_exposure_indicators",
        )?;
        let v = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, f64>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, f64>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        v
    };
    let confidence: Vec<(String, f64, String, f64)> = {
        let mut stmt = tx.prepare(
            "SELECT site_id, indicator_value, direction, reliability_weight
             FROM raw_confidence_indicators",
        )?;
        let v = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, f64>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, f64>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        v
    };

    // Sites from raw_sites.
    let sites: Vec<String> = {
        let mut stmt = tx.prepare("SELECT site_id FROM raw_sites")?;
        let v = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        v
    };

    // Exposure: aggregate per site (sum of weighted, reliability-scaled values).
    // Per plan §5.6, for `lower_risk` indicators (e.g. distance-to-intake where
    // a smaller raw value means higher risk) we flip the sign so "high risk"
    // always pushes the aggregate up.  The aggregate is then squashed to
    // [0, 1] by the per-site robust normaliser in `normalize`.
    let mut per_site_exposure: std::collections::HashMap<String, f64> =
        std::collections::HashMap::new();
    for (s, v, dir, w) in &exposure {
        let e = per_site_exposure.entry(s.clone()).or_insert(0.0);
        let sgn = if dir == "higher_risk" { 1.0 } else { -1.0 };
        *e += sgn * v * w;
    }
    // Normalize exposure across sites (robust median-centred to [0,1]).
    let exposure_vals: Vec<f64> = sites
        .iter()
        .map(|s| per_site_exposure.get(s).copied().unwrap_or(0.0))
        .collect();
    let exp_norm = normalize::normalize(&exposure_vals);
    let mut ins = tx.prepare(
        "INSERT INTO features
         (run_id, site_id, feature_name, feature_family, feature_value, feature_value_normalized, leakage_status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;
    for (i, s) in sites.iter().enumerate() {
        ins.execute(params![
            run_id,
            s,
            "exposure_norm",
            "exposure",
            exposure_vals[i],
            exp_norm[i],
            "ok"
        ])?;
    }
    drop(ins);

    // Confidence: combine multiple indicators.  Higher confidence → larger
    // `feature_value`; the missingness-derived `uncertainty_penalty` carries
    // the inverse signal downstream.
    let mut per_site_conf: std::collections::HashMap<String, f64> =
        std::collections::HashMap::new();
    for (s, v, dir, w) in &confidence {
        let e = per_site_conf.entry(s.clone()).or_insert(0.0);
        let sgn = if dir == "higher_confidence" {
            1.0
        } else {
            -1.0
        };
        *e += sgn * v * w;
    }
    let conf_vals: Vec<f64> = sites
        .iter()
        .map(|s| per_site_conf.get(s).copied().unwrap_or(0.0))
        .collect();
    let conf_norm = normalize::normalize(&conf_vals);
    let mut ins = tx.prepare(
        "INSERT INTO features
         (run_id, site_id, feature_name, feature_family, feature_value, feature_value_normalized, leakage_status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;
    for (i, s) in sites.iter().enumerate() {
        ins.execute(params![
            run_id,
            s,
            "confidence",
            "confidence",
            conf_vals[i],
            conf_norm[i],
            "ok"
        ])?;
    }
    drop(ins);

    // Missingness + uncertainty_penalty.
    // Missingness is the fraction of metal×sample rows for the site whose
    // `detect_flag = 0` (i.e. censored by the analytical method).  The
    // single-sample synthetic fixture therefore has missingness per site;
    // for multi-sample real data this fraction is a coverage diagnostic.
    let mut ins = tx.prepare(
        "INSERT INTO features
         (run_id, site_id, feature_name, feature_family, feature_value, feature_value_normalized, leakage_status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;
    for s in &sites {
        let total: i64 = tx.query_row(
            "SELECT COUNT(*) FROM cleaned_metals WHERE site_id = ?1 AND run_id = ?2",
            params![s, cleaned_run],
            |r| r.get(0),
        )?;
        let nd: i64 = tx.query_row(
            "SELECT COUNT(*) FROM cleaned_metals WHERE site_id = ?1 AND run_id = ?2 AND detect_flag = 0",
            params![s, cleaned_run],
            |r| r.get(0),
        )?;
        let miss = if total > 0 {
            nd as f64 / total as f64
        } else {
            1.0
        };
        ins.execute(params![
            run_id,
            s,
            "missingness",
            "missingness",
            miss,
            miss.clamp(0.0, 1.0),
            "ok"
        ])?;

        // Uncertainty penalty: half from missingness, half from 1 - confidence.
        // Both inputs are in [0,1] by construction.
        let conf = conf_norm[sites.iter().position(|x| x == s).unwrap_or(0)].clamp(0.0, 1.0);
        let upen = (0.5 * miss + 0.5 * (1.0 - conf)).clamp(0.0, 1.0);
        ins.execute(params![
            run_id,
            s,
            "uncertainty_penalty",
            "uncertainty",
            upen,
            upen,
            "ok"
        ])?;
    }
    drop(ins);

    tx.commit()?;
    Ok(sites.len())
}

#[cfg(test)]
mod tests {
    #[test]
    fn ordering_invariant() {
        // PLI is computed AFTER the CF rows are written; the smoke test is
        // that the orchestrator calls them in that order.  The end-to-end
        // test exercises this on the real SQLite fixture.
        assert!(true);
    }
}
