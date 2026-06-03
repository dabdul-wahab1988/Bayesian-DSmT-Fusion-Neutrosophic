//! R-ready exports: dump each `v_*` view to a CSV file in `--out-dir`.

use std::fs;
use std::io::Write;
use std::path::Path;

use rusqlite::Connection;

use crate::error::Result;

const VIEWS: &[&str] = &[
    "v_rankings_full",
    "v_neutrosophic_long",
    "v_posterior_summaries_wide",
    "v_belief_assignments_long",
    "v_dsmt_fused_long",
    "v_features_wide",
    "v_cleaned_metals_long",
    "v_run_manifest",
];

pub fn run(conn: &Connection, run_id: i64, out_dir: &Path) -> Result<Vec<(String, String, usize)>> {
    fs::create_dir_all(out_dir)?;
    let mut out = vec![];
    let tx = conn;
    for v in VIEWS {
        let path = out_dir.join(format!("{v}.csv"));
        let mut stmt = tx.prepare(&format!("SELECT * FROM {v}"))?;
        let col_count = stmt.column_count();
        let mut header = Vec::new();
        for c in 0..col_count {
            header.push(stmt.column_name(c)?.to_string());
        }
        let mut file = fs::File::create(&path)?;
        writeln!(file, "{}", header.join(","))?;
        let mut rows = stmt.query([])?;
        let mut n = 0usize;
        while let Some(row) = rows.next()? {
            let mut fields = Vec::new();
            for c in 0..col_count {
                let v: rusqlite::types::Value = row.get(c)?;
                fields.push(csv_escape(&format_value(v)));
            }
            writeln!(file, "{}", fields.join(","))?;
            n += 1;
        }
        // Update manifest.
        tx.execute(
            "INSERT INTO export_manifest (run_id, view_name, row_count, path)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![run_id, v, n as i64, path.to_string_lossy()],
        )?;
        out.push((v.to_string(), path.to_string_lossy().to_string(), n));
    }
    Ok(out)
}

fn format_value(v: rusqlite::types::Value) -> String {
    use rusqlite::types::Value::*;
    match v {
        Null => String::new(),
        Integer(i) => i.to_string(),
        Real(f) => format!("{f}"),
        Text(t) => t,
        Blob(_) => String::new(),
    }
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}
