//! Run manifest helpers — start/finish a run, record its status.

use rusqlite::{params, Connection};

use crate::error::Result;

pub fn start_run(conn: &Connection, project_id: Option<&str>, random_seed: i64) -> Result<i64> {
    conn.execute(
        "INSERT INTO runs (project_id, random_seed, status, module) VALUES (?1, ?2, 'running', 'init')",
        params![project_id, random_seed],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn finish_run_ok(conn: &Connection, run_id: i64) -> Result<()> {
    conn.execute(
        "UPDATE runs SET finished_at = datetime('now'), status = 'ok' WHERE run_id = ?1",
        params![run_id],
    )?;
    Ok(())
}

pub fn finish_run_warn(conn: &Connection, run_id: i64) -> Result<()> {
    conn.execute(
        "UPDATE runs SET finished_at = datetime('now'), status = 'warn' WHERE run_id = ?1",
        params![run_id],
    )?;
    Ok(())
}

pub fn finish_run_failed(conn: &Connection, run_id: i64, module: &str) -> Result<()> {
    conn.execute(
        "UPDATE runs SET finished_at = datetime('now'), status = 'failed', module = ?2 WHERE run_id = ?1",
        params![run_id, module],
    )?;
    Ok(())
}

pub fn read_seed(conn: &Connection) -> Result<i64> {
    let seed: i64 = conn
        .query_row(
            "SELECT COALESCE(CAST(parameter_value AS INTEGER), 42) FROM config WHERE parameter_name = 'random_seed'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(42);
    Ok(seed)
}
