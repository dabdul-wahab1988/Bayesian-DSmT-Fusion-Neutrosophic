//! DSmT module — re-exports the submodules.  The high-level orchestrator
//! is `dsmt::run` (below).

pub mod canonicalize;
pub mod expression;
pub mod free_model;
pub mod fusion;
pub mod hybrid_model;
pub mod pignistic;

use std::collections::HashMap;

use rusqlite::{params, Connection};

use crate::dsmt::canonicalize::{parse_model, Constraints, DsmtModel};
use crate::dsmt::fusion::{fuse, normalize_in_place, Focal, FusionResult};
use crate::dsmt::pignistic::dominant;
use crate::error::Result;
use crate::ingest::sqlite_insert;

pub fn run(conn: &mut Connection, run_id: i64, source_run: i64) -> Result<()> {
    let tx = conn.transaction()?;

    // Read DSmT model from config.
    let model_str = sqlite_insert::get_config(&tx, "dsmt_model", "free")?;
    let model = parse_model(&model_str)?;

    // Load constraints if hybrid.
    let constraints: Constraints = if matches!(model, DsmtModel::Hybrid) {
        let mut s = tx.prepare(
            "SELECT expression, constraint_type FROM raw_dsmt_constraints WHERE active = 1",
        )?;
        let rows: Vec<(String, String)> = s
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows.into_iter().collect()
    } else {
        HashMap::new()
    };

    // Load hypothesis symbols.
    let hyp_symbols: Vec<String> = {
        let mut s = tx.prepare(
            "SELECT hypothesis_symbol FROM raw_dsmt_hypotheses
             WHERE active = 1 ORDER BY hypothesis_id",
        )?;
        let v: Vec<String> = s
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        v
    };
    let n_h = hyp_symbols.len();

    // Load sites.
    let sites: Vec<String> = {
        let mut s = tx.prepare("SELECT site_id FROM raw_sites ORDER BY site_id")?;
        let v: Vec<String> = s
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        v
    };

    // Replace prior rows.
    tx.execute("DELETE FROM dsmt_fusion WHERE run_id = ?1", params![run_id])?;
    let mut ins = tx.prepare(
        "INSERT INTO dsmt_fusion
         (run_id, site_id, hypothesis_expr, fused_mass, conflict_mass, pignistic_probability, dominant_source_flag, dsmt_model)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )?;

    for site in &sites {
        // Pull belief rows for this site, grouped by evidence_layer.
        let mut stmt = tx.prepare(
            "SELECT evidence_layer, hypothesis_expr, belief_mass FROM belief_assignments
             WHERE run_id = ?1 AND site_id = ?2",
        )?;
        let rows: Vec<(String, String, f64)> = stmt
            .query_map(params![source_run, site], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut by_layer: HashMap<String, Vec<(String, f64)>> = HashMap::new();
        for (l, e, m) in &rows {
            by_layer.entry(l.clone()).or_default().push((e.clone(), *m));
        }

        let mut layers: Vec<Vec<Focal>> = vec![];
        // The DSmT fusion operates only on the source-hypothesis frame
        // (Θ = {θ_1, …, θ_n}).  The hotspot and confidence belief layers
        // live on separate binary frames and are recorded for downstream
        // use, but they must not be fused with source hypotheses.
        let layer_order = ["source", "chemical", "exposure"];
        for l in &layer_order {
            if let Some(focal_pairs) = by_layer.get(*l) {
                let mut focals: Vec<Focal> = vec![];
                for (expr, m) in focal_pairs {
                    let set = expression::parse(expr, &hyp_symbols)?;
                    focals.push((set, *m));
                }
                if !focals.is_empty() {
                    layers.push(focals);
                }
            }
        }

        let fr = if layers.is_empty() {
            // No focal mass for this site — write a single Θ row with mass 1.0
            // and conflict 0.0 so downstream consumers (pignistic, neutrosophic)
            // have something to read.  This is a deliberate fallback, not a
            // product of the conjunctive sum.
            let mut f = FusionResult::default();
            let n = if n_h > 0 { n_h } else { 2 };
            f.fused_mass.insert((0..n).collect(), 1.0);
            f
        } else {
            let mut r = fuse(&layers, &model, &constraints, &hyp_symbols);
            normalize_in_place(&mut r);
            r
        };

        let (dom_idx, bp) = dominant(&fr.fused_mass, n_h.max(2));

        for (k, m) in &fr.fused_mass {
            let expr = if !k.is_empty() && k.len() == n_h && (0..n_h).all(|i| k.contains(&i)) {
                "Θ".to_string()
            } else {
                k.iter()
                    .map(|&i| {
                        hyp_symbols
                            .get(i)
                            .cloned()
                            .unwrap_or_else(|| format!("θ_{}", i + 1))
                    })
                    .collect::<Vec<_>>()
                    .join("∪")
            };
            let dom_flag = (k.len() == 1 && k[0] == dom_idx) as i64;
            ins.execute(params![
                run_id,
                site,
                expr,
                m,
                fr.conflict,
                0.0_f64,
                dom_flag,
                model_str.as_str()
            ])?;
        }
        for h in 0..n_h {
            let sym = &hyp_symbols[h];
            // If a focal for this hypothesis exists, set its pignistic
            // probability; otherwise insert a zero-mass focal so the
            // neutrosophic criterion `source_plausibility` sees a row for
            // every hypothesis.
            let updated = tx.execute(
                "UPDATE dsmt_fusion SET pignistic_probability = ?1
                 WHERE run_id = ?2 AND site_id = ?3 AND hypothesis_expr = ?4",
                params![bp[h], run_id, site, sym],
            )?;
            if updated == 0 {
                ins.execute(params![
                    run_id,
                    site,
                    sym,
                    0.0_f64,
                    fr.conflict,
                    bp[h],
                    0_i64,
                    model_str.as_str()
                ])?;
            }
        }
        // The Θ focal does not contribute to pignistic; its pignistic
        // probability is the sum of all singletons (which equals 1 by
        // construction, so it is harmless to record and useful for
        // debugging).
        let _ = tx.execute(
            "UPDATE dsmt_fusion SET pignistic_probability = ?1
             WHERE run_id = ?2 AND site_id = ?3 AND hypothesis_expr = 'Θ'",
            params![bp.iter().sum::<f64>(), run_id, site],
        );
    }
    drop(ins);
    tx.commit()?;
    Ok(())
}
