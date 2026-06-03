//! `belief` module — read posteriors + features, write belief_assignments.

pub mod focal_elements;
pub mod posterior_to_belief;

use std::collections::HashMap;

use rusqlite::{params, Connection};

use crate::belief::posterior_to_belief::{
    chemical_belief, confidence_belief, exposure_belief, hotspot_belief, source_belief, Layer,
};
use crate::db;
use crate::error::Result;
use crate::ingest::sqlite_insert;

pub fn run(conn: &mut Connection, run_id: i64, source_run: i64) -> Result<()> {
    let tx = conn.transaction()?;
    let feature_run = db::latest_run_for_table(&tx, "features")?;

    // Read config
    let delta_single = sqlite_insert::get_config_f64(&tx, "single_source_margin", 1.0)?;
    let tau_union = sqlite_insert::get_config_f64(&tx, "union_support_threshold", 0.25)?;
    let w_max = sqlite_insert::get_config_f64(&tx, "belief_uncertainty_width_max", 1.0)?;
    let u_chem = sqlite_insert::get_config_f64(&tx, "belief_chemical_uncertainty", 0.1)?;
    let u_exp = sqlite_insert::get_config_f64(&tx, "belief_exposure_uncertainty", 0.1)?;
    let u_conf = sqlite_insert::get_config_f64(&tx, "belief_confidence_uncertainty", 0.05)?;
    let t_e = sqlite_insert::get_config_f64(&tx, "enrichment_threshold", 1.5)?;
    // metal_to_hypothesis is a JSON object string: e.g. {"As":"H1","Pb":"H2"}.
    // If absent, the chemical layer contributes only ignorance.
    let metal_to_hyp_json = sqlite_insert::get_config(&tx, "metal_to_hypothesis", "{}")
        .unwrap_or_else(|_| "{}".to_string());
    let metal_to_hyp: HashMap<String, String> = {
        let v: serde_json::Value =
            serde_json::from_str(&metal_to_hyp_json).unwrap_or_else(|_| serde_json::json!({}));
        let mut out = HashMap::new();
        if let Some(obj) = v.as_object() {
            for (metal, hyp_id) in obj {
                if let Some(s) = hyp_id.as_str() {
                    // Resolve against the active hypothesis_id list
                    // (resolved at site iteration time below).
                    out.insert(metal.clone(), s.to_string());
                }
            }
        }
        out
    };
    // Note: `metal_to_hyp` maps metal → hypothesis_id (string).  We
    // resolve to its 0-based index per-site using `id_to_idx` below.

    // Load sites + hypothesis symbols.  We keep the *unsorted* symbol list
    // (in raw_dsmt_hypotheses insertion order) so that the focal index
    // produced by `source_belief` lines up with the actual symbol names.
    let sites: Vec<String> = {
        let mut s = tx.prepare("SELECT site_id FROM raw_sites ORDER BY site_id")?;
        let v: Vec<String> = s
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(s);
        v
    };
    let hyp_index: Vec<(String, String)> = {
        let mut s = tx.prepare(
            "SELECT hypothesis_id, hypothesis_symbol FROM raw_dsmt_hypotheses
             WHERE active = 1 ORDER BY hypothesis_id",
        )?;
        let v: Vec<(String, String)> = s
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(s);
        v
    };
    let hyp_symbols: Vec<String> = hyp_index.iter().map(|(_, s)| s.clone()).collect();
    let n = hyp_index.len();
    let id_to_idx: HashMap<String, usize> = hyp_index
        .iter()
        .enumerate()
        .map(|(i, (id, _))| (id.clone(), i))
        .collect();
    // Resolve metal_to_hyp from hypothesis_id (string) to its 0-based index.
    let metal_to_hyp_idx: HashMap<String, usize> = metal_to_hyp
        .iter()
        .filter_map(|(m, hid)| id_to_idx.get(hid).map(|i| (m.clone(), *i)))
        .collect();
    // Risk weights (one per hypothesis) for the exposure layer.
    let risk_weights: Vec<f64> = {
        let mut s = tx.prepare(
            "SELECT default_risk_weight FROM raw_dsmt_hypotheses
             WHERE active = 1 ORDER BY hypothesis_id",
        )?;
        let v: Vec<f64> = s
            .query_map([], |r| r.get::<_, f64>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(s);
        v
    };

    // Helper: read one (site) -> posterior_summary + ci
    let read_posterior = |q: &str| -> Result<HashMap<String, (f64, f64, f64)>> {
        let mut out = HashMap::new();
        let mut s = tx.prepare(
            "SELECT site_id, posterior_mean, ci_lower_95, ci_upper_95
             FROM posterior_summaries WHERE run_id = ?1 AND quantity = ?2",
        )?;
        let v: Vec<(String, f64, f64, f64)> = s
            .query_map(params![source_run, q], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, f64>(1)?,
                    r.get::<_, f64>(2)?,
                    r.get::<_, f64>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(s);
        for (s, m, lo, hi) in v {
            out.insert(s, (m, lo, hi));
        }
        Ok(out)
    };

    let hotspot_post = read_posterior("hotspot_probability")?;
    // source:  per (site, hypothesis_id) -> mean
    let mut source_post: HashMap<String, Vec<(String, f64)>> = HashMap::new();
    {
        let mut s = tx.prepare(
            "SELECT site_id, metal, posterior_mean FROM posterior_summaries
             WHERE run_id = ?1 AND quantity = 'source_support_probability'",
        )?;
        let v: Vec<(String, String, f64)> = s
            .query_map(params![source_run], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, f64>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(s);
        for (s, h, m) in v {
            source_post.entry(s).or_default().push((h, m));
        }
    }

    // Read per-site features for the new layers.
    let exposure_by_site: HashMap<String, f64> = {
        let mut s = tx.prepare(
            "SELECT site_id, feature_value_normalized FROM features
             WHERE run_id = ?1 AND feature_name = 'exposure_norm'",
        )?;
        let v: Vec<(String, f64)> = s
            .query_map(params![feature_run], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(s);
        v.into_iter().collect()
    };
    let confidence_by_site: HashMap<String, f64> = {
        let mut s = tx.prepare(
            "SELECT site_id, feature_value_normalized FROM features
             WHERE run_id = ?1 AND feature_name = 'confidence'",
        )?;
        let v: Vec<(String, f64)> = s
            .query_map(params![feature_run], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(s);
        v.into_iter().collect()
    };
    // Per-site per-metal EF (raw value, not normalised, so we can
    // compare against `t_e`).
    let ef_by_site_metal: HashMap<String, Vec<(String, f64)>> = {
        let mut s = tx.prepare(
            "SELECT site_id, feature_name, feature_value FROM features
             WHERE run_id = ?1 AND feature_name LIKE 'ef_%'",
        )?;
        let v: Vec<(String, String, f64)> = s
            .query_map(params![feature_run], |r| {
                let name: String = r.get(1)?;
                let metal = name.strip_prefix("ef_").unwrap_or(&name).to_string();
                Ok((r.get::<_, String>(0)?, metal, r.get::<_, f64>(2)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(s);
        let mut out: HashMap<String, Vec<(String, f64)>> = HashMap::new();
        for (s, m, v) in v {
            out.entry(s).or_default().push((m, v));
        }
        out
    };

    // Replace prior belief_assignments for this run.
    tx.execute(
        "DELETE FROM belief_assignments WHERE run_id = ?1",
        params![run_id],
    )?;
    let mut ins = tx.prepare(
        "INSERT INTO belief_assignments
         (run_id, site_id, evidence_layer, hypothesis_expr, belief_mass, uncertainty_score, belief_rule)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;

    // Helper: render a focal set using the (hypothesis-stable) symbol list.
    let render = |k: &Vec<usize>| -> String {
        if !k.is_empty()
            && k.len() == hyp_symbols.len()
            && (0..hyp_symbols.len()).all(|i| k.contains(&i))
        {
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
        }
    };

    for site in &sites {
        // ===== Hotspot layer =====
        if let Some((m, lo, hi)) = hotspot_post.get(site).copied() {
            let ci_w = ((hi - lo) / w_max.max(1e-12)).clamp(0.0, 1.0);
            let m_h = hotspot_belief(m, ci_w)?;
            focal_elements::validate(&m_h)?;
            for (k, v) in &m_h {
                let expr = if k.len() == 2 {
                    "Θ_H".to_string()
                } else if k[0] == 0 {
                    "θ_H".to_string()
                } else {
                    "θ_NH".to_string()
                };
                ins.execute(params![
                    run_id,
                    site,
                    Layer::Hotspot.as_str(),
                    expr,
                    v,
                    ci_w,
                    "hotspot_p_ci"
                ])?;
            }
        }

        // ===== Source layer =====
        if let Some(rows) = source_post.get(site) {
            let mut p_vec = vec![0.0; n];
            for (h_id, m) in rows {
                if let Some(&i) = id_to_idx.get(h_id) {
                    p_vec[i] = *m;
                }
            }
            let mut u = 0.0;
            let mut u_n = 0;
            {
                let mut s = tx.prepare(
                    "SELECT metal, ci_lower_95, ci_upper_95 FROM posterior_summaries
                     WHERE run_id = ?1 AND quantity = 'source_support_probability'
                       AND site_id = ?2",
                )?;
                let v: Vec<(String, f64, f64)> = s
                    .query_map(params![source_run, site], |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, f64>(1)?,
                            r.get::<_, f64>(2)?,
                        ))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                drop(s);
                for (_h, lo, hi) in v {
                    u += (hi - lo).clamp(0.0, 1.0);
                    u_n += 1;
                }
            }
            if u_n > 0 {
                u /= u_n as f64;
            } else {
                u = 0.2;
            }
            let overlap = vec![vec![0.0; n]; n];
            let m_s = source_belief(&p_vec, u, delta_single, tau_union, &overlap, n)?;
            focal_elements::validate(&m_s)?;
            for (k, v) in &m_s {
                let expr = render(k);
                ins.execute(params![
                    run_id,
                    site,
                    Layer::Source.as_str(),
                    expr,
                    v,
                    u,
                    "dirichlet_to_belief"
                ])?;
            }
        }

        // ===== Chemical layer =====
        if let Some(efs) = ef_by_site_metal.get(site) {
            let m_c = chemical_belief(efs, t_e, u_chem, &metal_to_hyp_idx, n)?;
            focal_elements::validate(&m_c)?;
            for (k, v) in &m_c {
                let expr = render(k);
                ins.execute(params![
                    run_id,
                    site,
                    Layer::Chemical.as_str(),
                    expr,
                    v,
                    u_chem,
                    "chemical_fingerprint"
                ])?;
            }
        }

        // ===== Exposure layer =====
        let exp_norm = exposure_by_site.get(site).copied().unwrap_or(0.0);
        let m_e = exposure_belief(exp_norm, &risk_weights, u_exp, n)?;
        focal_elements::validate(&m_e)?;
        for (k, v) in &m_e {
            let expr = render(k);
            ins.execute(params![
                run_id,
                site,
                Layer::Exposure.as_str(),
                expr,
                v,
                u_exp,
                "exposure_weighted"
            ])?;
        }

        // ===== Confidence layer =====
        let conf = confidence_by_site.get(site).copied().unwrap_or(0.5);
        let m_c2 = confidence_belief(conf, u_conf)?;
        focal_elements::validate(&m_c2)?;
        // The confidence frame is a synthetic 2-element space; render
        // with the conventional θ_c / θ_nc / Θ_c labels.
        for (k, v) in &m_c2 {
            let expr = if k.len() == 2 {
                "Θ_C".to_string()
            } else if k[0] == 0 {
                "θ_C".to_string()
            } else {
                "θ_NC".to_string()
            };
            ins.execute(params![
                run_id,
                site,
                Layer::Confidence.as_str(),
                expr,
                v,
                u_conf,
                "confidence_belief"
            ])?;
        }
    }

    drop(ins);
    tx.commit()?;
    Ok(())
}
