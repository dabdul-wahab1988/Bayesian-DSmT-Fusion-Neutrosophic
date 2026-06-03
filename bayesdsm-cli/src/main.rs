//! `bayesdsm` CLI — 13 subcommands.

use std::path::PathBuf;

use anyhow::{anyhow, Result};
use bayesdsm_core::audit::run_manifest;
use bayesdsm_core::bayes;
use bayesdsm_core::belief;
use bayesdsm_core::clean;
use bayesdsm_core::db;
use bayesdsm_core::dsmt;
use bayesdsm_core::error::BayesDsmError;
use bayesdsm_core::export;
use bayesdsm_core::features;
use bayesdsm_core::ingest::sqlite_insert;
use bayesdsm_core::neutrosophic;
use bayesdsm_core::validate;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "bayesdsm",
    version,
    about = "Bayesian-DSmT-Neutrosophic sediment-metal hotspot prioritization (Rust + SQLite)."
)]
struct Cli {
    /// Path to the SQLite database file.
    #[arg(
        long,
        global = true,
        required = false,
        default_value = "bayesdsm.sqlite"
    )]
    db: PathBuf,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Create / migrate the SQLite schema.
    Init {
        /// Force re-initialization (drops all data).
        #[arg(long)]
        force: bool,
    },
    /// Ingest primary input CSVs from a directory.
    Ingest {
        #[arg(long)]
        input_dir: PathBuf,
    },
    /// Run post-ingestion audits and write warnings.
    Audit,
    /// Clean + standardize raw concentrations.
    Clean,
    /// Compute the feature matrix.
    Features,
    /// Run the three Bayesian models.
    Bayes {
        /// Random seed override (else from config).
        #[arg(long)]
        seed: Option<u64>,
    },
    /// Posterior-to-belief transformation.
    Belief,
    /// DSmT fusion + pignistic.
    Dsmt,
    /// Neutrosophic memberships.
    Neutrosophic,
    /// Final ranking with uncertainty bands (alias of `neutrosophic`).
    Rank,
    /// Validate against independent labels (if present).
    Validate,
    /// Sensitivity analysis: re-run pipeline under perturbations.
    Sensitivity {
        #[arg(long, default_value = "out/sensitivity")]
        out_dir: PathBuf,
    },
    /// Export R-ready views to CSV.
    Export {
        #[arg(long, default_value = "out")]
        out_dir: PathBuf,
    },
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let cli = Cli::parse();
    let mut conn = db::open(&cli.db)?;
    match cli.cmd {
        Cmd::Init { force } => cmd_init(&mut conn, force),
        Cmd::Ingest { input_dir } => cmd_ingest(&mut conn, &input_dir),
        Cmd::Audit => cmd_audit(&mut conn),
        Cmd::Clean => cmd_clean(&mut conn),
        Cmd::Features => cmd_features(&mut conn),
        Cmd::Bayes { seed } => cmd_bayes(&mut conn, seed),
        Cmd::Belief => cmd_belief(&mut conn),
        Cmd::Dsmt => cmd_dsmt(&mut conn),
        Cmd::Neutrosophic => cmd_neutrosophic(&mut conn),
        Cmd::Rank => cmd_rank(&mut conn),
        Cmd::Validate => cmd_validate(&mut conn),
        Cmd::Sensitivity { out_dir } => cmd_sensitivity(&mut conn, &out_dir),
        Cmd::Export { out_dir } => cmd_export(&mut conn, &out_dir),
    }
}

fn latest_run_id(conn: &rusqlite::Connection) -> Result<i64> {
    let id: i64 = conn.query_row(
        "SELECT run_id FROM runs ORDER BY run_id DESC LIMIT 1",
        [],
        |r| r.get(0),
    )?;
    Ok(id)
}

/// Find the most recent run_id that wrote into the given table.
/// Used to look up the source run for downstream pipeline steps.
fn latest_run_for_table(conn: &rusqlite::Connection, table: &str) -> Result<i64> {
    let id: i64 = conn
        .query_row(
            &format!("SELECT run_id FROM {table} ORDER BY run_id DESC LIMIT 1"),
            [],
            |r| r.get(0),
        )
        .map_err(|e| {
            anyhow!(
                "no rows in {}; run the prior pipeline step first ({})",
                table,
                e
            )
        })?;
    Ok(id)
}

fn start_run(conn: &mut rusqlite::Connection) -> Result<i64> {
    let seed: i64 = conn
        .query_row(
            "SELECT COALESCE(CAST(parameter_value AS INTEGER), 42) FROM config WHERE parameter_name = 'random_seed'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(42);
    let project_id: Option<String> = conn
        .query_row(
            "SELECT parameter_value FROM config WHERE parameter_name = 'project_id'",
            [],
            |r| r.get(0),
        )
        .ok();
    Ok(run_manifest::start_run(conn, project_id.as_deref(), seed)?)
}

fn handle<F: FnOnce(&mut rusqlite::Connection, i64) -> bayesdsm_core::error::Result<()>>(
    conn: &mut rusqlite::Connection,
    module: &str,
    f: F,
) -> Result<()> {
    let run_id = start_run(conn)?;
    let module = module.to_string();
    match f(conn, run_id) {
        Ok(()) => {
            run_manifest::finish_run_ok(conn, run_id)?;
            eprintln!("[ok] {module} (run_id={run_id})");
            Ok(())
        }
        Err(e) => {
            let _ = run_manifest::finish_run_failed(conn, run_id, &module);
            if let BayesDsmError::Stop {
                code,
                message,
                module: m,
            } = &e
            {
                let _ = bayesdsm_core::audit::failure_rules::insert_failure(
                    conn, run_id, &m, code, message,
                );
            }
            Err(anyhow!(e))
        }
    }
}

fn cmd_init(conn: &mut rusqlite::Connection, force: bool) -> Result<()> {
    if force {
        // Wipe all schema objects so the file can be reused without
        // having to close and reopen the connection (which on Windows
        // is racy against the still-open file handle).
        if db::is_initialized(conn) {
            // Drop user-defined views first (may depend on tables).
            let views: Vec<String> = {
                let mut s = conn.prepare(
                    "SELECT name FROM sqlite_master WHERE type='view' AND name NOT LIKE 'sqlite_%'",
                )?;
                let v: Vec<String> = s
                    .query_map([], |r| r.get::<_, String>(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                v
            };
            for v in &views {
                conn.execute(&format!("DROP VIEW IF EXISTS {v}"), [])?;
            }
            let tables: Vec<String> = {
                let mut s = conn.prepare(
                    "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
                )?;
                let v: Vec<String> = s
                    .query_map([], |r| r.get::<_, String>(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                v
            };
            for t in &tables {
                conn.execute(&format!("DROP TABLE IF EXISTS {t}"), [])?;
            }
        }
    }
    db::migrate(conn)?;
    eprintln!("[ok] schema initialized at {}", conn.path().unwrap_or(""));
    Ok(())
}

fn cmd_ingest(conn: &mut rusqlite::Connection, input_dir: &std::path::Path) -> Result<()> {
    if !db::is_initialized(conn) {
        return Err(anyhow!(
            "DB not initialized; run `bayesdsm init --db ...` first"
        ));
    }
    let _run_id = start_run(conn)?;
    let report = sqlite_insert::ingest_dir(conn, input_dir)?;
    eprintln!("[ok] ingested {} files", report.ingested.len());
    for (r, t, n) in &report.ingested {
        eprintln!("    {r:30} -> {t} ({n} rows)");
    }
    if !report.missing.is_empty() {
        eprintln!("[warn] optional files not present: {:?}", report.missing);
    }
    Ok(())
}

fn cmd_audit(conn: &mut rusqlite::Connection) -> Result<()> {
    if !db::is_initialized(conn) {
        return Err(anyhow!("DB not initialized"));
    }
    let counts = sqlite_insert::audit_row_counts(conn)?;
    for (t, n) in &counts {
        eprintln!("    {t:35} rows={n}");
    }
    eprintln!("[ok] audit passed");
    Ok(())
}

fn cmd_clean(conn: &mut rusqlite::Connection) -> Result<()> {
    handle(conn, "clean", |c, rid| {
        let n = clean::run(c, rid)?;
        eprintln!("    cleaned_metals rows: {n}");
        Ok(())
    })
}

fn cmd_features(conn: &mut rusqlite::Connection) -> Result<()> {
    handle(conn, "features", |c, rid| {
        features::run(c, rid)?;
        eprintln!("    features written");
        Ok(())
    })
}

fn cmd_bayes(conn: &mut rusqlite::Connection, seed_override: Option<u64>) -> Result<()> {
    if !db::is_initialized(conn) {
        return Err(anyhow!("DB not initialized"));
    }
    let run_id = start_run(conn)?;
    let seed = match seed_override {
        Some(s) => s,
        None => bayesdsm_core::audit::run_manifest::read_seed(conn)? as u64,
    };
    match bayes::run(conn, run_id, seed) {
        Ok(()) => {
            run_manifest::finish_run_ok(conn, run_id)?;
            eprintln!("[ok] bayes (run_id={run_id})");
            Ok(())
        }
        Err(e) => {
            let _ = run_manifest::finish_run_failed(conn, run_id, "bayes");
            if let BayesDsmError::Stop {
                code,
                message,
                module: m,
            } = &e
            {
                let _ = bayesdsm_core::audit::failure_rules::insert_failure(
                    conn, run_id, &m, code, message,
                );
            }
            Err(anyhow!(e))
        }
    }
}

fn cmd_belief(conn: &mut rusqlite::Connection) -> Result<()> {
    // Read posteriors from the most recent bayes run.
    if !db::is_initialized(conn) {
        return Err(anyhow!("DB not initialized"));
    }
    let source_run = latest_run_for_table(conn, "posterior_summaries")?;
    let new_run = start_run(conn)?;
    match belief::run(conn, new_run, source_run) {
        Ok(()) => {
            run_manifest::finish_run_ok(conn, new_run)?;
            eprintln!("[ok] belief (run_id={new_run}, source_run={source_run})");
            Ok(())
        }
        Err(e) => {
            let _ = run_manifest::finish_run_failed(conn, new_run, "belief");
            if let BayesDsmError::Stop {
                code,
                message,
                module: m,
            } = &e
            {
                let _ = bayesdsm_core::audit::failure_rules::insert_failure(
                    conn, new_run, &m, code, message,
                );
            }
            Err(anyhow!(e))
        }
    }
}

fn cmd_dsmt(conn: &mut rusqlite::Connection) -> Result<()> {
    if !db::is_initialized(conn) {
        return Err(anyhow!("DB not initialized"));
    }
    let source_run = latest_run_for_table(conn, "belief_assignments")?;
    let new_run = start_run(conn)?;
    match dsmt::run(conn, new_run, source_run) {
        Ok(()) => {
            run_manifest::finish_run_ok(conn, new_run)?;
            eprintln!("[ok] dsmt (run_id={new_run}, source_run={source_run})");
            Ok(())
        }
        Err(e) => {
            let _ = run_manifest::finish_run_failed(conn, new_run, "dsmt");
            if let BayesDsmError::Stop {
                code,
                message,
                module: m,
            } = &e
            {
                let _ = bayesdsm_core::audit::failure_rules::insert_failure(
                    conn, new_run, &m, code, message,
                );
            }
            Err(anyhow!(e))
        }
    }
}

fn cmd_neutrosophic(conn: &mut rusqlite::Connection) -> Result<()> {
    if !db::is_initialized(conn) {
        return Err(anyhow!("DB not initialized"));
    }
    let source_run = latest_run_for_table(conn, "dsmt_fusion")?;
    let new_run = start_run(conn)?;
    match neutrosophic::run(conn, new_run, source_run) {
        Ok(()) => {
            run_manifest::finish_run_ok(conn, new_run)?;
            eprintln!("[ok] neutrosophic (run_id={new_run}, source_run={source_run})");
            Ok(())
        }
        Err(e) => {
            let _ = run_manifest::finish_run_failed(conn, new_run, "neutrosophic");
            if let BayesDsmError::Stop {
                code,
                message,
                module: m,
            } = &e
            {
                let _ = bayesdsm_core::audit::failure_rules::insert_failure(
                    conn, new_run, &m, code, message,
                );
            }
            Err(anyhow!(e))
        }
    }
}

fn cmd_rank(conn: &mut rusqlite::Connection) -> Result<()> {
    if !db::is_initialized(conn) {
        return Err(anyhow!("DB not initialized"));
    }
    let source_run = latest_run_for_table(conn, "dsmt_fusion")?;
    let new_run = start_run(conn)?;
    match neutrosophic::run(conn, new_run, source_run) {
        Ok(()) => {
            run_manifest::finish_run_ok(conn, new_run)?;
            eprintln!("[ok] rank (run_id={new_run}, source_run={source_run})");
            Ok(())
        }
        Err(e) => {
            let _ = run_manifest::finish_run_failed(conn, new_run, "rank");
            if let BayesDsmError::Stop {
                code,
                message,
                module: m,
            } = &e
            {
                let _ = bayesdsm_core::audit::failure_rules::insert_failure(
                    conn, new_run, &m, code, message,
                );
            }
            Err(anyhow!(e))
        }
    }
}

fn cmd_validate(conn: &mut rusqlite::Connection) -> Result<()> {
    if !db::is_initialized(conn) {
        return Err(anyhow!("DB not initialized"));
    }
    let emitted = validate::run(conn)?;
    if emitted == 0 {
        eprintln!("[info] no independent validation labels; skipping");
        return Ok(());
    }
    eprintln!("[ok] validation: wrote {emitted} metric group(s) to warnings.context_json");
    Ok(())
}

fn cmd_sensitivity(conn: &mut rusqlite::Connection, _out_dir: &std::path::Path) -> Result<()> {
    if !db::is_initialized(conn) {
        return Err(anyhow!("DB not initialized"));
    }
    // Save the current value of `enrichment_threshold` so we can restore it.
    let original: String = conn
        .query_row(
            "SELECT parameter_value FROM config WHERE parameter_name = 'enrichment_threshold'",
            [],
            |r| r.get::<_, String>(0),
        )
        .unwrap_or_else(|_| "1.5".to_string());

    let parent_run = latest_run_id(conn)?;
    let perturbations = [
        ("baseline", original.clone()),
        ("threshold_1.0", "1.0".to_string()),
        ("threshold_2.0", "2.0".to_string()),
    ];
    let mut ok_runs: Vec<i64> = vec![];
    let mut last_err: Option<anyhow::Error> = None;

    for (label, value) in &perturbations {
        // Apply the perturbation.
        conn.execute(
            "UPDATE config SET parameter_value = ?2 WHERE parameter_name = 'enrichment_threshold'",
            rusqlite::params!["enrichment_threshold", value],
        )?;
        let result: std::result::Result<i64, anyhow::Error> = (|| {
            let feature_run = start_run(conn)?;
            features::run(conn, feature_run).map_err(anyhow::Error::from)?;
            run_manifest::finish_run_ok(conn, feature_run)?;

            let bayes_run = start_run(conn)?;
            bayes::run(conn, bayes_run, 42).map_err(anyhow::Error::from)?;
            run_manifest::finish_run_ok(conn, bayes_run)?;

            let belief_run = start_run(conn)?;
            belief::run(conn, belief_run, bayes_run).map_err(anyhow::Error::from)?;
            run_manifest::finish_run_ok(conn, belief_run)?;

            let dsmt_run = start_run(conn)?;
            dsmt::run(conn, dsmt_run, belief_run).map_err(anyhow::Error::from)?;
            run_manifest::finish_run_ok(conn, dsmt_run)?;

            let rank_run = start_run(conn)?;
            neutrosophic::run(conn, rank_run, dsmt_run).map_err(anyhow::Error::from)?;
            run_manifest::finish_run_ok(conn, rank_run)?;
            Ok(rank_run)
        })();
        match result {
            Ok(run) => {
                // Record per-site priority for this perturbation.
                conn.execute(
                    "INSERT INTO sensitivity_results (run_id, parameter_name, perturbation, quantity, value)
                     SELECT ?1, 'enrichment_threshold', ?2, 'priority_score', priority_score
                     FROM rankings WHERE run_id = ?1",
                    rusqlite::params![run, *value],
                )?;
                run_manifest::finish_run_ok(conn, run)?;
                ok_runs.push(run);
                eprintln!("[ok] sensitivity perturbation {label}={value} (run_id={run})");
            }
            Err(e) => {
                last_err = Some(anyhow!("perturbation {label}={value} failed: {e}"));
                break;
            }
        }
    }
    // Restore the original config value.
    conn.execute(
        "UPDATE config SET parameter_value = ?2 WHERE parameter_name = 'enrichment_threshold'",
        rusqlite::params!["enrichment_threshold", &original],
    )?;
    match last_err {
        None => {
            eprintln!(
                "[ok] sensitivity complete: {} runs (parent_run_id={})",
                ok_runs.len(),
                parent_run
            );
            Ok(())
        }
        Some(e) => Err(e),
    }
}

fn cmd_export(conn: &mut rusqlite::Connection, out_dir: &std::path::Path) -> Result<()> {
    if !db::is_initialized(conn) {
        return Err(anyhow!("DB not initialized"));
    }
    let run_id = latest_run_id(conn)?;
    let out = export::run(conn, run_id, out_dir)?;
    for (v, p, n) in &out {
        eprintln!("    exported {v:30} -> {p} ({n} rows)");
    }
    Ok(())
}
