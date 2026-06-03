//! `bayes` module: orchestrates the three Bayesian models, writes
//! `posterior_summaries`, `posterior_draws`, and `bayesian_diagnostics`.

pub mod burden;
pub mod diagnostics;
pub mod dirichlet_source;
pub mod hotspot_latent;
pub mod lognormal_enrichment;
pub mod mcmc;
pub mod posterior_summary;

use std::collections::HashMap;

use rusqlite::{params, Connection};

use crate::audit::failure_rules;
use crate::bayes::diagnostics::compute as compute_diag;
use crate::bayes::mcmc::run_chains;
use crate::bayes::posterior_summary::{summarise, PosteriorSummary};
use crate::db;
use crate::error::{BayesDsmError, Result};
use crate::ingest::sqlite_insert;
use crate::math::stats::{inv_logit, quantile, sd};

/// Per-(site, metal) rows read from cleaned_metals.
struct CleanedRow {
    site_id: String,
    /// Sample identifier; reserved for per-observation audit trail.
    /// Currently unused (the enrichment model aggregates across
    /// samples) but kept so the provenance is preserved end-to-end.
    #[allow(dead_code)]
    sample_id: String,
    metal: String,
    value_standard: f64,
    detect_flag: i64,
    detection_limit: Option<f64>,
}

fn load_cleaned(conn: &Connection, run_id: i64) -> Result<Vec<CleanedRow>> {
    let mut stmt = conn.prepare(
        "SELECT site_id, sample_id, metal, value_standard, detect_flag, detection_limit
         FROM cleaned_metals WHERE run_id = ?1",
    )?;
    let rows: Vec<CleanedRow> = stmt
        .query_map(params![run_id], |r| {
            Ok(CleanedRow {
                site_id: r.get(0)?,
                sample_id: r.get(1)?,
                metal: r.get(2)?,
                value_standard: r.get(3)?,
                detect_flag: r.get(4)?,
                detection_limit: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

fn load_sites(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT site_id FROM raw_sites ORDER BY site_id")?;
    let v = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(v)
}

fn load_metals(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt =
        conn.prepare("SELECT DISTINCT metal FROM raw_metal_concentrations ORDER BY metal")?;
    let v = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(v)
}

fn load_backgrounds(conn: &Connection) -> Result<HashMap<String, (f64, Option<f64>)>> {
    let mut stmt =
        conn.prepare("SELECT metal, background_value, uncertainty_sd FROM raw_background_values")?;
    let v = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, f64>(1)?,
                r.get::<_, Option<f64>>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut m = HashMap::new();
    for (k, val, unc) in v {
        failure_rules::check_background_positive(val)?;
        m.insert(k, (val, unc));
    }
    Ok(m)
}

fn load_hypotheses(conn: &Connection) -> Result<(Vec<String>, Vec<String>, HashMap<String, f64>)> {
    let mut stmt = conn.prepare(
        "SELECT hypothesis_id, hypothesis_symbol, default_risk_weight
         FROM raw_dsmt_hypotheses WHERE active = 1 ORDER BY hypothesis_id",
    )?;
    let rows: Vec<(String, String, f64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let ids: Vec<String> = rows.iter().map(|r| r.0.clone()).collect();
    let syms: Vec<String> = rows.iter().map(|r| r.1.clone()).collect();
    let weights: HashMap<String, f64> = rows.into_iter().map(|r| (r.0, r.2)).collect();
    Ok((ids, syms, weights))
}

pub fn run(conn: &mut Connection, run_id: i64, seed: u64) -> Result<()> {
    let sites = load_sites(conn)?;
    let metals = load_metals(conn)?;
    let bg = load_backgrounds(conn)?;
    let (hyp_ids, _hyp_syms, risk_weights) = load_hypotheses(conn)?;
    let cleaned_run = db::latest_run_for_table(conn, "cleaned_metals")?;
    let feature_run = db::latest_run_for_table(conn, "features")?;
    let cleaned = load_cleaned(conn, cleaned_run)?;

    let t_e = sqlite_insert::get_config_f64(conn, "enrichment_threshold", 1.5)?;
    let n_iter_default = sqlite_insert::get_config_i64(conn, "mcmc_iterations", 4000)?;
    let burn_in_default = sqlite_insert::get_config_i64(conn, "mcmc_burnin", 1000)?;
    let n_chains_default = sqlite_insert::get_config_i64(conn, "mcmc_chains", 2)?;
    let source_draws_default = sqlite_insert::get_config_i64(conn, "monte_carlo_draws", 500)?;
    let n_iter = sqlite_insert::get_config_i64(conn, "bayes_n_iter", n_iter_default)? as usize;
    let burn_in = sqlite_insert::get_config_i64(conn, "bayes_burn_in", burn_in_default)? as usize;
    let n_chains =
        sqlite_insert::get_config_i64(conn, "bayes_n_chains", n_chains_default)? as usize;
    let source_draws_n =
        sqlite_insert::get_config_i64(conn, "source_draws", source_draws_default)? as usize;

    // Build site-metal samples for the enrichment model.
    let mut sm_data: Vec<(String, String, Vec<f64>, Vec<f64>, Vec<bool>)> = vec![];
    {
        let mut map: HashMap<(String, String), (Vec<f64>, Vec<f64>, Vec<bool>)> = HashMap::new();
        for r in &cleaned {
            if r.value_standard <= 0.0 && r.detect_flag == 0 {
                if let Some(dl) = r.detection_limit {
                    if dl > 0.0 {
                        let e = map.entry((r.site_id.clone(), r.metal.clone())).or_insert((
                            vec![],
                            vec![],
                            vec![],
                        ));
                        e.2.push(false);
                        e.0.push(0.0);
                        e.1.push(dl.ln());
                    }
                }
            } else if r.value_standard > 0.0 && r.detect_flag == 1 {
                let e = map.entry((r.site_id.clone(), r.metal.clone())).or_insert((
                    vec![],
                    vec![],
                    vec![],
                ));
                e.2.push(true);
                e.0.push(r.value_standard.ln());
                e.1.push(0.0);
            }
        }
        for ((s, m), (logy, dl, det)) in map {
            sm_data.push((s, m, logy, dl, det));
        }
    }

    // ------ 1. Enrichment model + burden ------
    let mut posterior_draws: Vec<(String, String, String, usize, f64)> = vec![]; // (site, metal|None, quantity, idx, value)
    let mut summaries: Vec<(
        String,
        Option<String>,
        String,
        PosteriorSummary,
        String,
        f64,
        f64,
        f64,
    )> = vec![]; // site, metal, quantity, sum, model, rhat, ess_bulk, ess_tail

    // Build a (metal, a_m) weight vector for burden.  Use equal weights; allow override via stakeholder_weights when present.
    let a_m_weights: Vec<(String, f64)> = {
        let mut stmt = conn.prepare(
            "SELECT criterion, weight FROM raw_stakeholder_weights WHERE criterion LIKE 'metal_%'",
        )?;
        let v: Vec<(String, f64)> = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if !v.is_empty() {
            let s: f64 = v.iter().map(|(_, w)| w).sum();
            if (s - 1.0).abs() <= 1e-8 {
                v.into_iter()
                    .map(|(mut k, w)| {
                        if let Some(stripped) = k.strip_prefix("metal_") {
                            k = stripped.to_string();
                        }
                        (k, w)
                    })
                    .collect()
            } else {
                burden::check_weights(&v)?; // surfaces the STOP
                vec![]
            }
        } else {
            let m = metals.len().max(1) as f64;
            metals.iter().map(|x| (x.clone(), 1.0 / m)).collect()
        }
    };

    // per-metal shared sigma and tau (the prior is centred on log B_m
    // with scale `tau0`; no offset term is needed when the prior is
    // honest about the background).
    let tau0: f64 = 0.5;
    let sigma_default: f64 = 0.3;

    for (site, metal, logy, dl_log, detect) in &sm_data {
        let (b, _b_unc) = match bg.get(metal) {
            Some(v) => *v,
            None => continue,
        };
        // Build SiteMetal
        let sm = lognormal_enrichment::SiteMetal {
            site_id: site.clone(),
            metal: metal.clone(),
            log_y: logy.clone(),
            dl_log: dl_log.clone(),
            detect: detect.clone(),
        };
        // Two chains for R-hat.
        let chains = run_chains(
            seed.wrapping_add(hash_str(&format!("enr:{site}:{metal}"))),
            n_chains,
            n_iter,
            burn_in,
            sm.log_y
                .iter()
                .zip(sm.detect.iter())
                .filter(|(_, d)| **d)
                .map(|(v, _)| *v)
                .sum::<f64>()
                / (sm.detect.iter().filter(|d| **d).count().max(1) as f64),
            |mu| lognormal_enrichment::log_posterior(mu, sigma_default, &sm, b.ln(), tau0),
        );
        let ef_draws: Vec<f64> = chains
            .iter()
            .flat_map(|c| c.draws.iter().map(|mu| mu.exp() / b))
            .collect();
        for (i, v) in ef_draws.iter().enumerate() {
            posterior_draws.push((site.clone(), metal.clone(), "enrichment".into(), i, *v));
        }
        if ef_draws.is_empty() {
            return Err(BayesDsmError::Invalid(format!(
                "empty ef draws for site {site} metal {metal}"
            )));
        }
        let diag = compute_diag(&chains);

        // Per (site, metal): enrichment probability = P(EF > T_E).
        let mut enriched_draws: Vec<f64> = ef_draws
            .iter()
            .map(|v| if *v > t_e { 1.0 } else { 0.0 })
            .collect();
        let p_sum = summarise(&mut enriched_draws).ok_or_else(|| {
            BayesDsmError::Invalid(format!(
                "empty enrichment indicator draws for site {site} metal {metal}"
            ))
        })?;
        failure_rules::check_probability(p_sum.mean, "enrichment_probability")?;
        summaries.push((
            site.clone(),
            Some(metal.clone()),
            "enrichment_probability".into(),
            p_sum,
            "lognormal_enrichment".into(),
            diag.rhat,
            diag.ess_bulk,
            diag.ess_tail,
        ));

        // Per-site burden E_i = sum_m a_m * 1(P_enrich > T_E) is computed
        // downstream from the (site, metal, enrichment_probability)
        // summaries — no per-draw mean needed here.
    }

    // ------ 2. Source-support model (Dirichlet per draw) ------
    // Build per-site concentrations.
    let source_indicators: Vec<(String, String, String, f64, f64, String)> = {
        let mut stmt = conn.prepare(
            "SELECT indicator_id, site_id, hypothesis_id, indicator_value, reliability_weight, direction
             FROM raw_source_indicators",
        )?;
        let rows: Vec<(String, String, String, f64, f64, String)> = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, f64>(3)?,
                    r.get::<_, f64>(4)?,
                    r.get::<_, String>(5)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows
    };
    let alpha_0 = sqlite_insert::get_config_f64(conn, "dirichlet_alpha0", 1.0)?;
    let lambda = sqlite_insert::get_config_f64(conn, "dirichlet_lambda", 1.0)?;
    let concentrations = dirichlet_source::build_concentrations(
        &sites,
        &source_indicators,
        &hyp_ids,
        alpha_0,
        lambda,
    )?;

    let mut source_draws: Vec<Vec<Vec<f64>>> = vec![vec![]; sites.len()]; // site -> draws -> [h]
    for draw_idx in 0..source_draws_n {
        let s = dirichlet_source::sample(&concentrations, seed.wrapping_add(13 + draw_idx as u64))?;
        for (i, row) in s.iter().enumerate() {
            source_draws[i].push(row.clone());
        }
    }
    // Mean and CI per (site, h)
    for (i, site) in sites.iter().enumerate() {
        for (h_idx, h_id) in hyp_ids.iter().enumerate() {
            let mut col: Vec<f64> = source_draws[i].iter().map(|d| d[h_idx]).collect();
            let sum = summarise(&mut col).ok_or_else(|| {
                BayesDsmError::Invalid(format!("empty source draws for site {site} h {h_id}"))
            })?;
            // For the Dirichlet source model each "draw" is an iid sample,
            // so rhat = 1.0 and ESS = n_draws by construction.  We still
            // report the real n_draws so downstream tools can audit.
            summaries.push((
                site.clone(),
                Some(h_id.clone()),
                "source_support_probability".into(),
                sum,
                "dirichlet_source".into(),
                1.0,
                source_draws_n as f64,
                source_draws_n as f64,
            ));
            // store draws (subsample to limit row count: every 5th).
            for (k, draw) in source_draws[i].iter().enumerate().step_by(5) {
                posterior_draws.push((
                    site.clone(),
                    h_id.clone(),
                    "source_pi".into(),
                    k,
                    draw[h_idx],
                ));
            }
        }
    }

    // ------ 3. Hotspot model ------
    // We compute per-site:  E_i (posterior mean), S_i (mean over h of v_h * mean pi), X_i, U_i, then MCMC over psi.
    let g0 = sqlite_insert::get_config_f64(conn, "hotspot_gamma0", 0.0)?;
    let ge = sqlite_insert::get_config_f64(conn, "hotspot_gamma_e", 2.0)?;
    let gs = sqlite_insert::get_config_f64(conn, "hotspot_gamma_s", 1.5)?;
    let gx = sqlite_insert::get_config_f64(conn, "hotspot_gamma_x", 1.0)?;
    let gu = sqlite_insert::get_config_f64(conn, "hotspot_gamma_u", 1.0)?;
    let psi_sd = sqlite_insert::get_config_f64(conn, "hotspot_psi_sd", 1.0)?;

    // Pre-compute burden per site: mean over draws of sum a_m * 1(EF > T_E).
    // Re-derive from the (site, metal, enrichment_probability) summaries.
    let mut burden_by_site: HashMap<String, f64> = HashMap::new();
    for s in &sites {
        let mut acc = 0.0;
        for m in &metals {
            let p = summaries
                .iter()
                .find(|(st, mt, q, _, _, _, _, _)| {
                    st == s && mt.as_deref() == Some(m) && q == "enrichment_probability"
                })
                .map(|x| x.3.mean)
                .unwrap_or(0.0);
            let a = a_m_weights
                .iter()
                .find(|(mm, _)| mm == m)
                .map(|x| x.1)
                .unwrap_or(0.0);
            acc += a * p;
        }
        burden_by_site.insert(s.clone(), acc.clamp(0.0, 1.0));
    }

    // Pre-compute X_i: exposure (sum of weighted indicators, normalised).
    let x_by_site: HashMap<String, f64> = {
        let mut stmt = conn.prepare(
            "SELECT site_id, feature_value_normalized FROM features
             WHERE run_id = ?1 AND feature_name = 'exposure_norm'",
        )?;
        let v: Vec<(String, f64)> = stmt
            .query_map(params![feature_run], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        v.into_iter().collect()
    };
    // U_i: uncertainty_penalty.
    let u_by_site: HashMap<String, f64> = {
        let mut stmt = conn.prepare(
            "SELECT site_id, feature_value FROM features
             WHERE run_id = ?1 AND feature_name = 'uncertainty_penalty'",
        )?;
        let v: Vec<(String, f64)> = stmt
            .query_map(params![feature_run], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        v.into_iter().collect()
    };

    let mut hotspot_draws: HashMap<String, Vec<f64>> = HashMap::new();
    for site in &sites {
        let e_i = burden_by_site.get(site).copied().unwrap_or(0.0);
        let s_i = {
            let mut acc = 0.0;
            for (_h_idx, h_id) in hyp_ids.iter().enumerate() {
                let p_h = summaries
                    .iter()
                    .find(|(st, mt, q, _, _, _, _, _)| {
                        st == site
                            && mt.as_deref() == Some(h_id)
                            && q == "source_support_probability"
                    })
                    .map(|x| x.3.mean)
                    .unwrap_or(0.0);
                let v = risk_weights.get(h_id).copied().unwrap_or(0.0);
                acc += v * p_h;
            }
            acc
        };
        let x_i = x_by_site.get(site).copied().unwrap_or(0.0);
        let u_i = u_by_site.get(site).copied().unwrap_or(0.0);
        let prior_mean = g0 + ge * e_i + gs * s_i + gx * x_i - gu * u_i;

        let chains = run_chains(
            seed.wrapping_add(hash_str(&format!("hot:{site}"))),
            n_chains,
            n_iter,
            burn_in,
            prior_mean,
            |psi| hotspot_latent::log_posterior(psi, prior_mean, psi_sd),
        );
        let psi_draws: Vec<f64> = chains
            .iter()
            .flat_map(|c| c.draws.iter().copied())
            .collect();
        let p_draws: Vec<f64> = psi_draws.iter().map(|&p| inv_logit(p)).collect();
        for (i, p) in p_draws.iter().enumerate() {
            posterior_draws.push((site.clone(), String::new(), "hotspot".into(), i, *p));
        }
        let mut p_d = p_draws.clone();
        let sum = summarise(&mut p_d).ok_or_else(|| {
            BayesDsmError::Invalid(format!("empty hotspot draws for site {site}"))
        })?;
        let diag = compute_diag(&chains);
        summaries.push((
            site.clone(),
            None,
            "hotspot_probability".into(),
            sum,
            "hotspot_latent".into(),
            diag.rhat,
            diag.ess_bulk,
            diag.ess_tail,
        ));
        hotspot_draws.insert(site.clone(), p_draws);
    }

    // ------ Write outputs ------
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM posterior_summaries WHERE run_id = ?1",
        params![run_id],
    )?;
    tx.execute(
        "DELETE FROM posterior_draws WHERE run_id = ?1",
        params![run_id],
    )?;
    tx.execute(
        "DELETE FROM bayesian_diagnostics WHERE run_id = ?1",
        params![run_id],
    )?;
    let mut s_ins = tx.prepare(
        "INSERT INTO posterior_summaries
         (run_id, site_id, metal, quantity, posterior_mean, posterior_sd, ci_lower_95, ci_upper_95, rhat, ess_bulk, ess_tail, model_name)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
    )?;
    for (site, metal, q, sum, model, rhat, ess_bulk, ess_tail) in &summaries {
        s_ins.execute(params![
            run_id, site, metal, q, sum.mean, sum.sd, sum.ci_lo, sum.ci_hi, rhat, ess_bulk,
            ess_tail, model,
        ])?;
    }
    drop(s_ins);

    let mut d_ins = tx.prepare(
        "INSERT INTO posterior_draws (run_id, site_id, metal, quantity, draw_index, value)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    for (site, metal, q, idx, v) in &posterior_draws {
        d_ins.execute(params![run_id, site, metal, q, *idx as i64, v])?;
    }
    drop(d_ins);

    let mut dg_ins = tx.prepare(
        "INSERT INTO bayesian_diagnostics
         (run_id, quantity, site_id, metal, chain, draws, rhat, ess_bulk, ess_tail)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?;
    for (site, metal, q, _sum, model, rhat, ess_bulk, ess_tail) in &summaries {
        // No per-chain split in this summary; record one row with chain=0.
        dg_ins.execute(params![
            run_id,
            q,
            site,
            metal,
            0i64,
            n_iter as i64,
            rhat,
            ess_bulk,
            ess_tail,
        ])?;
        let _ = model;
    }
    drop(dg_ins);

    // Apply R-hat WARN/DOWNGRADE rules.
    for (_site, _metal, _q, _sum, _model, rhat, _ess_bulk, _ess_tail) in &summaries {
        if let Some(sev) = failure_rules::rhat_severity(*rhat) {
            failure_rules::insert_warning(
                &tx,
                run_id,
                "bayes",
                sev,
                "W0901",
                &format!(
                    "R-hat {} exceeds threshold for quantity {} site {}",
                    rhat, _q, _site
                ),
            )?;
        }
    }

    tx.commit()?;
    Ok(())
}

fn hash_str(s: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

#[allow(dead_code)]
fn _unused_quantile_sd() {
    let mut x = vec![1.0, 2.0, 3.0];
    let _ = quantile(&mut x, 0.5);
    let _ = sd(&[1.0, 2.0, 3.0]);
}
