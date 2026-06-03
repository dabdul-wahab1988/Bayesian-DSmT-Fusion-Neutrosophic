//! Orchestrates the ingest step: read CSVs from an input dir, validate,
//! insert into `raw_*` tables, register hashes in `input_files`, and
//! populate the `config` mirror of `raw_config_parameters`.

use std::collections::HashMap;
use std::path::Path;

use rusqlite::{params, Connection, Transaction};

use crate::audit::hashing::sha256_file;
use crate::error::{BayesDsmError, Result};
use crate::ingest::csv_read::{count_rows, insert_raw, read_csv};
use crate::ingest::validate;

const REQUIRED: &[(&str, &str)] = &[
    ("site_metadata", "raw_sites"),
    ("sampling_events", "raw_sampling_events"),
    ("metal_concentrations", "raw_metal_concentrations"),
    ("background_values", "raw_background_values"),
    ("dsmt_hypotheses", "raw_dsmt_hypotheses"),
    ("config_parameters", "raw_config_parameters"),
];

const RECOMMENDED: &[(&str, &str)] = &[
    ("source_indicators", "raw_source_indicators"),
    ("exposure_indicators", "raw_exposure_indicators"),
    ("confidence_indicators", "raw_confidence_indicators"),
    ("leakage_rules", "raw_leakage_rules"),
    ("stakeholder_weights", "raw_stakeholder_weights"),
];

const OPTIONAL: &[(&str, &str)] = &[
    ("validation_labels", "raw_validation_labels"),
    ("dsmt_constraints", "raw_dsmt_constraints"),
    ("data_dictionary", "raw_data_dictionary"),
    ("allowed_values", "raw_allowed_values"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Required,
    Recommended,
    Optional,
}

pub struct IngestReport {
    pub ingested: Vec<(String, String, usize)>, // (role, table, row_count)
    pub missing: Vec<String>,
}

pub fn ingest_dir(conn: &mut Connection, input_dir: &Path) -> Result<IngestReport> {
    let mut report = IngestReport {
        ingested: vec![],
        missing: vec![],
    };

    let tx = conn.transaction()?;

    // Required files first.
    for (role, table) in REQUIRED {
        let path = input_dir.join(format!("{role}.csv"));
        if !path.exists() {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1100".into(),
                message: format!("required input file missing: {}", path.display()),
            });
        }
        ingest_one(&tx, Role::Required, role, table, &path, &mut report)?;
    }

    for (role, table) in RECOMMENDED {
        let path = input_dir.join(format!("{role}.csv"));
        if path.exists() {
            ingest_one(&tx, Role::Recommended, role, table, &path, &mut report)?;
        } else {
            report.missing.push(role.to_string());
        }
    }

    for (role, table) in OPTIONAL {
        let path = input_dir.join(format!("{role}.csv"));
        if path.exists() {
            ingest_one(&tx, Role::Optional, role, table, &path, &mut report)?;
        } else {
            report.missing.push(role.to_string());
        }
    }

    // Populate the `config` mirror from `raw_config_parameters`.
    populate_config(&tx)?;

    tx.commit()?;
    Ok(report)
}

fn ingest_one(
    tx: &Transaction<'_>,
    role: Role,
    file_role: &str,
    table: &str,
    path: &Path,
    report: &mut IngestReport,
) -> Result<()> {
    let (headers, rows) = read_csv(path)?;
    let nrows = rows.len();
    let ncols = headers.len();
    let sha = sha256_file(path)?;

    // Validate first, before touching the database.
    validate_for_role(table, &headers, &rows, tx)?;

    // Drop existing rows for idempotency within a single ingest.
    tx.execute(&format!("DELETE FROM {table}"), [])?;

    // Insert.
    let col_refs: Vec<&str> = headers.iter().map(|s| s.as_str()).collect();
    insert_raw(tx, table, &col_refs, &rows)?;

    // Update or insert input_files row.
    tx.execute(
        "INSERT INTO input_files (file_role, path, sha256, row_count, column_count, schema_status)
         VALUES (?1, ?2, ?3, ?4, ?5, 'ok')
         ON CONFLICT(file_role) DO UPDATE SET
             path = excluded.path,
             sha256 = excluded.sha256,
             row_count = excluded.row_count,
             column_count = excluded.column_count,
             imported_at = datetime('now'),
             schema_status = excluded.schema_status",
        params![
            file_role,
            path.to_string_lossy().to_string(),
            sha,
            nrows as i64,
            ncols as i64
        ],
    )?;

    let role_str = match role {
        Role::Required => "required",
        Role::Recommended => "recommended",
        Role::Optional => "optional",
    };
    report
        .ingested
        .push((format!("{role_str}:{file_role}"), table.to_string(), nrows));
    Ok(())
}

fn validate_for_role(
    table: &str,
    headers: &[String],
    rows: &[Vec<String>],
    tx: &Transaction<'_>,
) -> Result<()> {
    match table {
        "raw_sites" => validate::validate_sites(headers, rows),
        "raw_sampling_events" => validate::validate_sampling_events(headers, rows, tx),
        "raw_metal_concentrations" => validate::validate_metal_concentrations(headers, rows, tx),
        "raw_background_values" => validate::validate_background_values(headers, rows),
        "raw_dsmt_hypotheses" => validate::validate_dsmt_hypotheses(headers, rows),
        "raw_config_parameters" => validate::validate_config_parameters(headers, rows),
        "raw_source_indicators" => validate::validate_source_indicators(headers, rows, tx),
        "raw_exposure_indicators" => validate::validate_exposure_indicators(headers, rows, tx),
        "raw_confidence_indicators" => validate::validate_confidence_indicators(headers, rows, tx),
        "raw_leakage_rules" => validate::validate_leakage_rules(headers, rows),
        "raw_stakeholder_weights" => validate::validate_stakeholder_weights(headers, rows),
        "raw_validation_labels" => validate::validate_validation_labels(headers, rows, tx),
        "raw_dsmt_constraints" => validate::validate_dsmt_constraints(headers, rows),
        "raw_data_dictionary" => validate::validate_data_dictionary(headers, rows),
        "raw_allowed_values" => validate::validate_allowed_values(headers, rows),
        _ => Err(BayesDsmError::Invalid(format!(
            "unknown raw table: {table}"
        ))),
    }
}

fn populate_config(tx: &Transaction<'_>) -> Result<()> {
    let mut stmt = tx.prepare(
        "SELECT parameter_name, parameter_value, parameter_type, module, description
         FROM raw_config_parameters",
    )?;
    let rows: Vec<(String, String, String, String, Option<String>)> = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut map: HashMap<String, (String, String, String, Option<String>)> = HashMap::new();
    for (n, v, t, m, d) in rows {
        map.insert(n, (v, t, m, d));
    }

    let mut ins = tx.prepare(
        "INSERT INTO config (parameter_name, parameter_value, parameter_type, module, description)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(parameter_name) DO UPDATE SET
            parameter_value = excluded.parameter_value,
            parameter_type  = excluded.parameter_type,
            module          = excluded.module,
            description     = excluded.description",
    )?;
    for (n, (v, t, m, d)) in &map {
        ins.execute(params![n, v, t, m, d])?;
    }
    Ok(())
}

pub fn get_config(conn: &Connection, name: &str, default: &str) -> Result<String> {
    let v: Option<String> = conn
        .query_row(
            "SELECT parameter_value FROM config WHERE parameter_name = ?1",
            [name],
            |r| r.get(0),
        )
        .ok();
    Ok(v.unwrap_or_else(|| default.to_string()))
}

pub fn get_config_i64(conn: &Connection, name: &str, default: i64) -> Result<i64> {
    let v: Option<String> = conn
        .query_row(
            "SELECT parameter_value FROM config WHERE parameter_name = ?1",
            [name],
            |r| r.get(0),
        )
        .ok();
    Ok(v.and_then(|s| s.parse::<i64>().ok()).unwrap_or(default))
}

pub fn get_config_f64(conn: &Connection, name: &str, default: f64) -> Result<f64> {
    let v: Option<String> = conn
        .query_row(
            "SELECT parameter_value FROM config WHERE parameter_name = ?1",
            [name],
            |r| r.get(0),
        )
        .ok();
    Ok(v.and_then(|s| s.parse::<f64>().ok()).unwrap_or(default))
}

/// Re-count rows for every `raw_*` table; used by `audit` to detect drift.
pub fn audit_row_counts(conn: &Connection) -> Result<HashMap<String, i64>> {
    let tables = [
        "raw_sites",
        "raw_sampling_events",
        "raw_metal_concentrations",
        "raw_background_values",
        "raw_dsmt_hypotheses",
        "raw_config_parameters",
        "raw_source_indicators",
        "raw_exposure_indicators",
        "raw_confidence_indicators",
        "raw_leakage_rules",
        "raw_stakeholder_weights",
        "raw_validation_labels",
        "raw_dsmt_constraints",
        "raw_data_dictionary",
        "raw_allowed_values",
    ];
    let mut out = HashMap::new();
    for t in tables {
        let n: i64 = conn.query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get(0))?;
        out.insert(t.to_string(), n);
    }
    // Verify counts against input_files.row_count for the matching role.
    let mut stmt = conn.prepare("SELECT file_role, row_count FROM input_files")?;
    let map: Vec<(String, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let role_to_table: HashMap<&str, &str> = HashMap::from([
        ("site_metadata", "raw_sites"),
        ("sampling_events", "raw_sampling_events"),
        ("metal_concentrations", "raw_metal_concentrations"),
        ("background_values", "raw_background_values"),
        ("dsmt_hypotheses", "raw_dsmt_hypotheses"),
        ("config_parameters", "raw_config_parameters"),
        ("source_indicators", "raw_source_indicators"),
        ("exposure_indicators", "raw_exposure_indicators"),
        ("confidence_indicators", "raw_confidence_indicators"),
        ("leakage_rules", "raw_leakage_rules"),
        ("stakeholder_weights", "raw_stakeholder_weights"),
        ("validation_labels", "raw_validation_labels"),
        ("dsmt_constraints", "raw_dsmt_constraints"),
        ("data_dictionary", "raw_data_dictionary"),
        ("allowed_values", "raw_allowed_values"),
    ]);
    for (role, declared) in map {
        if let Some(t) = role_to_table.get(role.as_str()) {
            if let Some(&actual) = out.get(*t) {
                if actual != declared {
                    return Err(BayesDsmError::Stop {
                        module: "audit".into(),
                        code: "E1500".into(),
                        message: format!(
                            "input_files/{role} declares {declared} rows but {t} has {actual}"
                        ),
                    });
                }
            }
        }
    }
    Ok(out)
}

#[allow(dead_code)]
fn _unused_count_rows() {
    let _ = count_rows;
}
