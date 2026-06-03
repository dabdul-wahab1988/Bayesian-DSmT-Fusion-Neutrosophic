//! CSV reading utilities.

use std::fs::File;
use std::path::Path;

use rusqlite::Connection;

use crate::error::Result;

/// Read a CSV into a `Vec<Vec<String>>` of headers + rows.
pub fn read_csv(path: &Path) -> Result<(Vec<String>, Vec<Vec<String>>)> {
    let f = File::open(path)?;
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(f);
    let headers = rdr
        .headers()?
        .iter()
        .map(|s| s.trim().to_string())
        .collect();
    let mut rows = Vec::new();
    for rec in rdr.records() {
        let rec = rec?;
        rows.push(rec.iter().map(|s| s.to_string()).collect());
    }
    Ok((headers, rows))
}

/// Count rows (excluding header) of a CSV file by streaming.
pub fn count_rows(path: &Path) -> Result<usize> {
    let f = File::open(path)?;
    let mut rdr = csv::ReaderBuilder::new().has_headers(true).from_reader(f);
    Ok(rdr.records().count())
}

/// Insert raw rows into a SQLite table using a positional INSERT.  This
/// intentionally does NOT use prepared-statement caching for tables whose
/// column count varies; it is fast enough for the sizes we target and
/// keeps the code straightforward.  Cell values are coerced to the
/// declared column type (REAL/INTEGER) on the way in so that empty cells
/// in nullable columns become SQL NULL.
pub fn insert_raw(
    conn: &Connection,
    table: &str,
    cols: &[&str],
    rows: &[Vec<String>],
) -> Result<usize> {
    if cols.is_empty() || rows.is_empty() {
        return Ok(0);
    }
    // Resolve declared column types via PRAGMA so we can coerce "" -> NULL
    // and parse numerics correctly.  We build a name->type map and then
    // project it onto `cols` (the CSV header order) so the per-cell type
    // coercion below aligns with the column actually being inserted.
    let mut by_name: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let rows2: Vec<(String, String)> = stmt
            .query_map([], |r| Ok((r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (n, t) in rows2 {
            by_name.insert(n, t.to_uppercase());
        }
    }
    let col_types: Vec<String> = cols
        .iter()
        .map(|c| by_name.get(*c).cloned().unwrap_or_default())
        .collect();
    let placeholders: Vec<String> = (1..=cols.len()).map(|i| format!("?{i}")).collect();
    let sql = format!(
        "INSERT INTO {table} ({}) VALUES ({})",
        cols.join(", "),
        placeholders.join(", ")
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut n = 0usize;
    for row in rows {
        // Build a Vec<Box<dyn ToSql>> so that we can hold mixed types
        // (String, i64, f64, Option<...>) for a single row.
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::with_capacity(row.len());
        for (i, cell) in row.iter().enumerate() {
            let t = col_types.get(i).map(String::as_str).unwrap_or("");
            let v: Box<dyn rusqlite::ToSql> = if cell.is_empty() {
                Box::new(None::<String>)
            } else if t.contains("INT") {
                match cell.parse::<i64>() {
                    Ok(v) => Box::new(v),
                    Err(_) => Box::new(cell.clone()),
                }
            } else if t.contains("REAL")
                || t.contains("FLOAT")
                || t.contains("DOUB")
                || t.contains("NUM")
            {
                match cell.parse::<f64>() {
                    Ok(v) => Box::new(v),
                    Err(_) => Box::new(cell.clone()),
                }
            } else {
                Box::new(cell.clone())
            };
            params.push(v);
        }
        // Re-borrow as &[&dyn ToSql] via the slice-of-Box trick:
        let param_refs: Vec<&dyn rusqlite::ToSql> = params
            .iter()
            .map(|b| b.as_ref() as &dyn rusqlite::ToSql)
            .collect();
        stmt.execute(&param_refs[..])?;
        n += 1;
    }
    Ok(n)
}

/// Find the index of a column, returning a clear error if missing.
pub fn col_index(headers: &[String], name: &str) -> Result<usize> {
    headers
        .iter()
        .position(|h| h.eq_ignore_ascii_case(name))
        .ok_or_else(|| {
            crate::error::BayesDsmError::Invalid(format!(
                "required column '{name}' not found in CSV"
            ))
        })
}
