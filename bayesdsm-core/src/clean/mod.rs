//! `clean` module — convert raw rows into standardised `cleaned_metals`.

pub mod nondetect;
pub mod units;

use std::collections::HashMap;

use rusqlite::{params, Connection};

use crate::audit::failure_rules;
use crate::error::Result;
use crate::ingest::sqlite_insert;

pub fn run(conn: &mut Connection, run_id: i64) -> Result<usize> {
    let tx = conn.transaction()?;

    // Read config.
    let standard_unit = sqlite_insert::get_config(&tx, "standard_concentration_unit", "mg/kg")?;
    let mode_str = sqlite_insert::get_config(&tx, "nondetect_method", "dl_sqrt2")?;
    let mode = nondetect::parse_mode(&mode_str)?;
    let cleaning_method = match mode {
        nondetect::Mode::DlSqrt2 => "dl_sqrt2",
        nondetect::Mode::DlHalf => "dl_half",
        nondetect::Mode::CensoredBayes => "censored_bayes",
    };

    // Pull raw rows.
    let rows: Vec<(
        String,
        String,
        String,
        String,
        f64,
        String,
        i64,
        Option<f64>,
    )> = {
        let mut stmt = tx.prepare(
            "SELECT concentration_id, sample_id, site_id, metal, value, unit, detect_flag, detection_limit
             FROM raw_metal_concentrations",
        )?;
        let v: Vec<(
            String,
            String,
            String,
            String,
            f64,
            String,
            i64,
            Option<f64>,
        )> = stmt
            .query_map([], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get::<_, Option<f64>>(7)?,
                ))
            })?
            .collect::<rusqlite::Result<_>>()?;
        v
    };

    // Replace previous cleaned_metals for this run (idempotent).
    tx.execute(
        "DELETE FROM cleaned_metals WHERE run_id = ?1",
        params![run_id],
    )?;

    let mut n = 0usize;
    let mut ins = tx.prepare(
        "INSERT INTO cleaned_metals
         (run_id, site_id, sample_id, metal, value_raw, unit_raw, value_standard,
          unit_standard, detect_flag, detection_limit, cleaning_method)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
    )?;

    for (cid, sid, site, metal, value, unit, flag, dl) in rows {
        let _ = cid; // unused: row identity is implicit
        let kappa = units::kappa(&unit, &standard_unit)?;
        let value_standard = nondetect::substitute(flag, value, dl, mode)? * kappa;

        failure_rules::check_cleaned_positive(value_standard, flag, dl)?;

        ins.execute(params![
            run_id,
            site,
            sid,
            metal,
            value,
            unit,
            value_standard,
            standard_unit,
            flag,
            dl,
            cleaning_method,
        ])?;
        n += 1;
    }
    drop(ins);
    tx.commit()?;
    Ok(n)
}

/// Convenience: list `(site_id, sample_id, metal)` -> cleaned value.
pub fn load_cleaned(
    conn: &Connection,
    run_id: i64,
) -> Result<HashMap<(String, String, String), f64>> {
    let mut stmt = conn.prepare(
        "SELECT site_id, sample_id, metal, value_standard FROM cleaned_metals WHERE run_id = ?1",
    )?;
    let rows = stmt
        .query_map(params![run_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, f64>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut m = HashMap::new();
    for (s, p, mt, v) in rows {
        m.insert((s, p, mt), v);
    }
    Ok(m)
}
