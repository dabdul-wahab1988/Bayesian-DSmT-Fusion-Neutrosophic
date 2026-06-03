//! Database connection helpers and migration runner.

use std::path::Path;

use rusqlite::{Connection, OptionalExtension};

use crate::error::{BayesDsmError, Result};

/// Open a connection with the conventions this package relies on.
///
/// Foreign keys are enforced; WAL is enabled.
pub fn open(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    // WAL is per-database; only set on file-backed DBs, not :memory:.
    if path != Path::new(":memory:") {
        conn.pragma_update(None, "journal_mode", "WAL")?;
    }
    Ok(conn)
}

/// Apply all bundled migrations, in order, idempotently.
pub fn migrate(conn: &Connection) -> Result<()> {
    let migrations: &[(&str, &str)] = &[
        ("0001_init", include_str!("schema/0001_init.sql")),
        ("0002_views", include_str!("schema/0002_views.sql")),
    ];

    // Ensure the migrations table exists; 0001 also creates it, but for fresh
    // databases we apply it first.
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version     INTEGER PRIMARY KEY,
            applied_at  TEXT NOT NULL DEFAULT (datetime('now')),
            description TEXT NOT NULL
        );",
    )?;

    for (name, sql) in migrations {
        let version: i64 = name
            .split('_')
            .next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let already: bool = conn
            .query_row(
                "SELECT 1 FROM schema_migrations WHERE version = ?1",
                [version],
                |_| Ok(true),
            )
            .optional()?
            .unwrap_or(false);

        if !already {
            conn.execute_batch(sql)?;
            conn.execute(
                "INSERT INTO schema_migrations (version, description) VALUES (?1, ?2)",
                rusqlite::params![version, name],
            )?;
        }
    }
    Ok(())
}

/// True if the schema has been initialized (i.e. `runs` table exists).
pub fn is_initialized(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type='table' AND name='runs'",
        [],
        |_| Ok(true),
    )
    .optional()
    .ok()
    .flatten()
    .unwrap_or(false)
}

/// Find the latest run_id that wrote rows to a pipeline output table.
///
/// Pipeline stages write their own output rows under a fresh run_id, so
/// downstream stages must resolve the relevant upstream table explicitly
/// instead of assuming all inputs live under the current run.
pub fn latest_run_for_table(conn: &Connection, table: &str) -> Result<i64> {
    conn.query_row(
        &format!("SELECT run_id FROM {table} ORDER BY run_id DESC LIMIT 1"),
        [],
        |r| r.get(0),
    )
    .map_err(|e| {
        BayesDsmError::Invalid(format!(
            "no rows in {table}; run the required prior pipeline step first ({e})"
        ))
    })
}
