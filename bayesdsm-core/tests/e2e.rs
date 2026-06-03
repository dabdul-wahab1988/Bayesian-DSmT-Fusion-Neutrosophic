//! End-to-end integration test.
//!
//! Drives the full pipeline (init → ingest → audit → clean → features → bayes
//! → belief → dsmt → neutrosophic → rank → export) on the synthetic fixture
//! by calling the library API directly, then asserts the contract-level
//! invariants documented in `plan.txt` and `primary_input_contract.md`.

use std::path::PathBuf;

use bayesdsm_core::audit::run_manifest;
use bayesdsm_core::bayes;
use bayesdsm_core::belief;
use bayesdsm_core::clean;
use bayesdsm_core::db;
use bayesdsm_core::dsmt;
use bayesdsm_core::export;
use bayesdsm_core::features;
use bayesdsm_core::ingest::sqlite_insert;
use bayesdsm_core::neutrosophic;
use bayesdsm_core::validate;

fn fixture_dir() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop(); // up to pacakage/
    p.push("tests/synthetic");
    p
}

fn tmp_db_path(name: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("bayesdsm-it-{name}-{}.sqlite", std::process::id()));
    if p.exists() {
        let _ = std::fs::remove_file(&p);
    }
    p
}

/// Run the full pipeline on a fresh DB and return the open connection.
/// `target_out_dir` is where R-ready CSVs are written.
fn run_full_pipeline(
    db_path: &std::path::Path,
    fixture: &std::path::Path,
    target_out_dir: &std::path::Path,
) -> rusqlite::Connection {
    let mut conn = db::open(db_path).expect("open db");
    db::migrate(&mut conn).expect("migrate");

    // ----- ingest -----
    let project = conn
        .query_row(
            "SELECT parameter_value FROM config WHERE parameter_name = 'project_id'",
            [],
            |r| r.get::<_, String>(0),
        )
        .ok();
    let _ingest_run = run_manifest::start_run(&mut conn, project.as_deref(), 42).unwrap();
    let report = sqlite_insert::ingest_dir(&mut conn, fixture).expect("ingest_dir");
    assert!(!report.ingested.is_empty(), "ingested nothing");
    run_manifest::finish_run_ok(&mut conn, _ingest_run).unwrap();

    // ----- audit -----
    let _audit_counts = sqlite_insert::audit_row_counts(&mut conn).expect("audit_row_counts");

    // ----- clean -----
    let clean_run = run_manifest::start_run(&mut conn, project.as_deref(), 42).unwrap();
    clean::run(&mut conn, clean_run).expect("clean");
    run_manifest::finish_run_ok(&mut conn, clean_run).unwrap();

    // ----- features -----
    let feat_run = run_manifest::start_run(&mut conn, project.as_deref(), 42).unwrap();
    features::run(&mut conn, feat_run).expect("features");
    run_manifest::finish_run_ok(&mut conn, feat_run).unwrap();

    // ----- bayes -----
    let bayes_run = run_manifest::start_run(&mut conn, project.as_deref(), 42).unwrap();
    bayes::run(&mut conn, bayes_run, 42).expect("bayes");
    run_manifest::finish_run_ok(&mut conn, bayes_run).unwrap();

    // ----- belief -----
    let belief_run = run_manifest::start_run(&mut conn, project.as_deref(), 42).unwrap();
    let bayes_source: i64 = conn
        .query_row("SELECT MAX(run_id) FROM posterior_summaries", [], |r| {
            r.get(0)
        })
        .unwrap();
    belief::run(&mut conn, belief_run, bayes_source).expect("belief");
    run_manifest::finish_run_ok(&mut conn, belief_run).unwrap();

    // ----- dsmt -----
    let dsmt_run = run_manifest::start_run(&mut conn, project.as_deref(), 42).unwrap();
    let belief_source: i64 = conn
        .query_row("SELECT MAX(run_id) FROM belief_assignments", [], |r| {
            r.get(0)
        })
        .unwrap();
    dsmt::run(&mut conn, dsmt_run, belief_source).expect("dsmt");
    run_manifest::finish_run_ok(&mut conn, dsmt_run).unwrap();

    // ----- neutrosophic + rank (combined in one module call) -----
    let neut_run = run_manifest::start_run(&mut conn, project.as_deref(), 42).unwrap();
    let dsmt_source: i64 = conn
        .query_row("SELECT MAX(run_id) FROM dsmt_fusion", [], |r| r.get(0))
        .unwrap();
    neutrosophic::run(&mut conn, neut_run, dsmt_source).expect("neutrosophic");
    run_manifest::finish_run_ok(&mut conn, neut_run).unwrap();

    // ----- export -----
    let _ = std::fs::remove_dir_all(target_out_dir);
    std::fs::create_dir_all(target_out_dir).unwrap();
    let last_run: i64 = conn
        .query_row("SELECT MAX(run_id) FROM runs", [], |r| r.get(0))
        .unwrap();
    let _out = export::run(&mut conn, last_run, target_out_dir).expect("export");

    conn
}

#[test]
fn full_pipeline_runs_and_invariants_hold() {
    let db_path = tmp_db_path("happy");
    let fixture = fixture_dir();
    let out_dir = std::env::temp_dir().join(format!("bayesdsm-out-{}", std::process::id()));

    let conn = run_full_pipeline(&db_path, &fixture, &out_dir);

    // All expected output tables populated.
    let n = |t: &str| -> i64 {
        conn.query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get(0))
            .unwrap_or(0)
    };
    assert!(n("cleaned_metals") > 0, "cleaned_metals empty");
    assert!(n("features") > 0, "features empty");
    assert!(n("posterior_summaries") > 0, "posterior_summaries empty");
    assert!(n("posterior_draws") > 0, "posterior_draws empty");
    assert!(n("bayesian_diagnostics") > 0, "bayesian_diagnostics empty");
    assert!(n("belief_assignments") > 0, "belief_assignments empty");
    assert!(n("dsmt_fusion") > 0, "dsmt_fusion empty");
    assert!(
        n("neutrosophic_memberships") > 0,
        "neutrosophic_memberships empty"
    );
    assert!(n("rankings") > 0, "rankings empty");
    assert!(
        n("runs") >= 7,
        "expected at least 7 run rows, got {}",
        n("runs")
    );

    // Regression checks for run-lineage and evidence-layer integrity.
    assert!(
        n_where(
            &conn,
            "features",
            "feature_family IN ('cf','ef','igeo','pli')"
        ) > 0,
        "chemical feature families were not generated"
    );
    assert!(
        n_where(
            &conn,
            "posterior_summaries",
            "quantity = 'enrichment_probability'"
        ) > 0,
        "enrichment posterior summaries missing"
    );
    assert!(
        n_where(&conn, "belief_assignments", "evidence_layer = 'chemical'") > 0,
        "chemical belief layer missing"
    );
    assert!(
        n_where(&conn, "belief_assignments", "evidence_layer = 'exposure'") > 0,
        "exposure belief layer missing"
    );
    assert!(
        n_where(&conn, "belief_assignments", "evidence_layer = 'hotspot'") > 0,
        "hotspot belief layer should be recorded"
    );
    assert!(
        n_where(
            &conn,
            "dsmt_fusion",
            "hypothesis_expr IN ('θ_H','θ_NH','Θ_H')"
        ) == 0,
        "binary hotspot frame leaked into source DSmT fusion"
    );
    let max_priority: f64 = conn
        .query_row("SELECT MAX(priority_score) FROM rankings", [], |r| r.get(0))
        .unwrap();
    assert!(max_priority > 0.0, "all final priority scores are zero");

    let max_diag_draws: i64 = conn
        .query_row("SELECT MAX(draws) FROM bayesian_diagnostics", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(
        max_diag_draws, 2000,
        "mcmc_iterations config alias should control diagnostics.draws"
    );
    let distinct_source_draws: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM (
                SELECT site_id, metal, COUNT(DISTINCT ROUND(value, 12)) AS n_distinct
                FROM posterior_draws
                WHERE quantity = 'source_pi'
                GROUP BY site_id, metal
                HAVING n_distinct > 1
             )",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        distinct_source_draws > 0,
        "source Dirichlet draws are repeated rather than independent"
    );

    // Σm(A) = 1 (to 1e-6) for every site in the latest belief_assignments.
    let latest_belief: i64 = conn
        .query_row("SELECT MAX(run_id) FROM belief_assignments", [], |r| {
            r.get(0)
        })
        .unwrap_or(0);
    let mut stmt = conn
        .prepare(
            "SELECT site_id, SUM(belief_mass) FROM belief_assignments
             WHERE run_id = ?1 GROUP BY site_id, evidence_layer",
        )
        .unwrap();
    let rows: Vec<(String, f64)> = stmt
        .query_map([latest_belief], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?))
        })
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert!(
        !rows.is_empty(),
        "no belief rows for latest run {latest_belief}"
    );
    for (site, sum) in &rows {
        assert!(
            (sum - 1.0).abs() < 1e-6,
            "belief mass conservation violated for site {site} (sum={sum})"
        );
    }
    drop(stmt);

    // Every site has a rank in [1, N_s].
    let n_sites: i64 = conn
        .query_row("SELECT COUNT(*) FROM raw_sites", [], |r| r.get(0))
        .unwrap();
    let latest_rank: i64 = conn
        .query_row("SELECT MAX(run_id) FROM rankings", [], |r| r.get(0))
        .unwrap_or(0);
    let mut stmt = conn
        .prepare("SELECT MIN(rank), MAX(rank) FROM rankings WHERE run_id = ?1")
        .unwrap();
    let (lo, hi): (i64, i64) = stmt
        .query_row([latest_rank], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap();
    assert_eq!(lo, 1, "min rank should be 1");
    assert!(hi <= n_sites, "max rank {hi} > n_sites {n_sites}");
    drop(stmt);

    let (ci_lo, ci_hi): (f64, f64) = conn
        .query_row(
            "SELECT MIN(rank_ci_lower), MAX(rank_ci_upper) FROM rankings WHERE run_id = ?1",
            [latest_rank],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert!(
        ci_lo >= 1.0,
        "rank_ci_lower should be on rank scale, got {ci_lo}"
    );
    assert!(
        ci_hi <= n_sites as f64,
        "rank_ci_upper should be on rank scale, got {ci_hi}"
    );

    // All 8 R-ready views present and non-empty (or at least queryable).
    let views = [
        "v_rankings_full",
        "v_neutrosophic_long",
        "v_posterior_summaries_wide",
        "v_belief_assignments_long",
        "v_dsmt_fused_long",
        "v_features_wide",
        "v_cleaned_metals_long",
        "v_run_manifest",
    ];
    for v in &views {
        let cnt: i64 = conn
            .query_row(&format!("SELECT COUNT(*) FROM {v}"), [], |r| r.get(0))
            .unwrap();
        assert!(cnt >= 0, "view {v} should exist");
    }

    // All exported CSVs exist.
    for v in &views {
        let p = out_dir.join(format!("{v}.csv"));
        assert!(p.exists(), "expected export {p:?} to exist");
    }

    // Cleanup
    drop(conn);
    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_dir_all(&out_dir);
}

fn n_where(conn: &rusqlite::Connection, table: &str, predicate: &str) -> i64 {
    conn.query_row(
        &format!("SELECT COUNT(*) FROM {table} WHERE {predicate}"),
        [],
        |r| r.get(0),
    )
    .unwrap_or(0)
}

#[test]
fn dsmt_mass_conservation_in_fusion_table() {
    // For the latest dsmt run, Σ fused_mass over the focal sets per site
    // should be 1 to 1e-6.
    let db_path = tmp_db_path("dsmt");
    let fixture = fixture_dir();
    let out_dir = std::env::temp_dir().join(format!("bayesdsm-out-dsmt-{}", std::process::id()));

    let conn = run_full_pipeline(&db_path, &fixture, &out_dir);

    let latest: i64 = conn
        .query_row("SELECT MAX(run_id) FROM dsmt_fusion", [], |r| r.get(0))
        .unwrap();
    let mut stmt = conn
        .prepare(
            "SELECT site_id, SUM(fused_mass) FROM dsmt_fusion
             WHERE run_id = ?1 GROUP BY site_id",
        )
        .unwrap();
    let rows: Vec<(String, f64)> = stmt
        .query_map([latest], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?))
        })
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert!(!rows.is_empty(), "no dsmt_fusion rows");
    for (site, s) in &rows {
        // Plan §12.1 step 6: after transfer-to-union, Σ m_fused = 1
        // (conflict is embedded in the union mass; `conflict_mass` is
        // tracked separately for diagnostics).
        assert!(
            (s - 1.0).abs() < 1e-6,
            "dsmt mass conservation violated for {site}: {s}"
        );
    }
    drop(stmt);
    drop(conn);
    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn validate_writes_metric_warnings() {
    // Run the full pipeline, then call validate and assert the
    // synthetic fixture's `validation_labels` produce a warning row
    // with Spearman/top-k/calibration JSON.
    let db_path = tmp_db_path("validate");
    let fixture = fixture_dir();
    let out_dir = std::env::temp_dir().join(format!("bayesdsm-out-val-{}", std::process::id()));

    let mut conn = run_full_pipeline(&db_path, &fixture, &out_dir);

    // Count validation warnings before.
    let before: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM warnings WHERE module = 'validate'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let emitted = validate::run(&mut conn).expect("validate");
    assert!(
        emitted >= 1,
        "expected at least one validate row, got {emitted}"
    );

    let after: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM warnings WHERE module = 'validate'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(after > before, "validate did not append to warnings");

    // The context_json must contain a parseable object with the
    // expected top-level keys.
    let ctx: String = conn
        .query_row(
            "SELECT context_json FROM warnings WHERE module = 'validate' LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(&ctx).expect("parse context_json");
    for k in [
        "spearman_rho",
        "top1_overlap",
        "top3_overlap",
        "mean_p_yes",
        "mean_p_no",
        "brier_score",
        "n_sites",
    ] {
        assert!(v.get(k).is_some(), "context_json missing key '{k}': {ctx}");
    }

    drop(conn);
    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn failure_modes_zero_background_stops_at_ingest() {
    // A non-positive background should fail the package at the audit step
    // (we trigger it via `audit_row_counts` which exercises the validation).
    use std::io::Write;
    let tmp = std::env::temp_dir().join(format!("bayesdsm-bad-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    // Copy the entire synthetic fixture, then overwrite background_values.csv
    // with a row that has B_m <= 0 (which the contract says → STOP).
    let src = fixture_dir();
    for entry in std::fs::read_dir(&src).unwrap() {
        let entry = entry.unwrap();
        let dest = tmp.join(entry.file_name());
        std::fs::copy(entry.path(), &dest).unwrap();
    }
    let bad = tmp.join("background_values.csv");
    let mut f = std::fs::File::create(&bad).unwrap();
    writeln!(
        f,
        "site_id,metal,background_value,background_source,uncertainty"
    )
    .unwrap();
    writeln!(f, "S1,As,0.0,test,0.1").unwrap();
    writeln!(f, "S1,Cd,1.5,test,0.1").unwrap();
    writeln!(f, "S1,Cr,50.0,test,5.0").unwrap();
    writeln!(f, "S1,Ni,30.0,test,3.0").unwrap();
    writeln!(f, "S1,Pb,25.0,test,2.5").unwrap();
    drop(f);

    let db_path = tmp_db_path("bad");
    let mut conn = db::open(&db_path).expect("open db");
    db::migrate(&mut conn).expect("migrate");
    let _ingest_run = run_manifest::start_run(&mut conn, None, 42).unwrap();
    // The ingest may or may not fail; what we care about is that the audit
    // surfaces an error.  Try ingest first; if it succeeds, audit must fail.
    let ingest_res = sqlite_insert::ingest_dir(&mut conn, &tmp);
    if let Err(e) = ingest_res {
        // Ingest itself rejected the bad value — that is acceptable.
        eprintln!("[ok] ingest rejected bad background: {e}");
    } else {
        // Ingest let it through; the audit must STOP.
        let audit_res = sqlite_insert::audit_row_counts(&mut conn);
        if let Err(e) = audit_res {
            eprintln!("[ok] audit rejected bad background: {e}");
        } else {
            panic!("expected STOP on non-positive background, got Ok");
        }
    }
    drop(conn);
    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_dir_all(&tmp);
}
