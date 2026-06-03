//! Neutrosophic module orchestrator: build T/F/I per criterion, score, rank.
//!
//! Plan §14 + §15.  All numbers in this module are derived from upstream
//! tables (`features`, `posterior_summaries`, `dsmt_fusion`,
//! `posterior_draws`) — the module writes only `neutrosophic_memberships`
//! and `rankings`.

pub mod criterion_score;
pub mod membership;
pub mod uncertainty;

use std::collections::HashMap;

use rusqlite::{params, Connection};

use crate::db;
use crate::error::Result;
use crate::ingest::sqlite_insert;
use crate::neutrosophic::criterion_score::{score_one, weighted_priority};
use crate::neutrosophic::membership::Triplet;
use crate::neutrosophic::uncertainty::{mode_band, rank_ci, stability};

pub fn run(conn: &mut Connection, run_id: i64, source_run: i64) -> Result<()> {
    let tx = conn.transaction()?;
    let feature_run = db::latest_run_for_table(&tx, "features")?;
    let bayes_run = db::latest_run_for_table(&tx, "posterior_summaries")?;

    let eta = sqlite_insert::get_config_f64(&tx, "neutrosophic_indeterminacy_penalty", 0.5)?;
    let band_crit = sqlite_insert::get_config_f64(&tx, "ranking_band_critical", 0.80)?;
    let band_high = sqlite_insert::get_config_f64(&tx, "ranking_band_high", 0.65)?;
    let band_mod = sqlite_insert::get_config_f64(&tx, "ranking_band_moderate", 0.45)?;

    // Load sites.
    let sites: Vec<String> = {
        let mut s = tx.prepare("SELECT site_id FROM raw_sites ORDER BY site_id")?;
        let v: Vec<String> = s
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        v
    };

    // Load stakeholder weights.
    let weights: Vec<(String, f64)> = {
        let mut s = tx.prepare("SELECT criterion, weight FROM raw_stakeholder_weights")?;
        let v: Vec<(String, f64)> = s
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        canonicalize_weights(&v)
    };
    crate::audit::failure_rules::check_weights_sum_to_one(&weights)?;

    // Helpers.
    let read_feature = |name: &str| -> Result<HashMap<String, f64>> {
        let mut s = tx.prepare(
            "SELECT site_id, feature_value_normalized FROM features WHERE run_id = ?1 AND feature_name = ?2",
        )?;
        let rows: Vec<(String, f64)> = s
            .query_map(params![feature_run, name], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut out = HashMap::new();
        for (s, v) in rows {
            out.insert(s, v);
        }
        Ok(out)
    };

    let missingness = read_feature("missingness")?;
    let confidence = read_feature("confidence")?;
    let exposure = read_feature("exposure_norm")?;
    let upen = read_feature("uncertainty_penalty")?;

    let hotspot_p: HashMap<String, f64> = {
        let mut s = tx.prepare(
            "SELECT site_id, posterior_mean FROM posterior_summaries
             WHERE run_id = ?1 AND quantity = 'hotspot_probability'",
        )?;
        let v: Vec<(String, f64)> = s
            .query_map(params![bayes_run], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        v.into_iter().collect()
    };

    let source_weights: HashMap<String, f64> = {
        let mut s = tx.prepare(
            "SELECT hypothesis_symbol, default_risk_weight
             FROM raw_dsmt_hypotheses WHERE active = 1 ORDER BY hypothesis_id",
        )?;
        let v: Vec<(String, f64)> = s
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        v.into_iter().collect()
    };

    // r_{i,source} = Σ_h risk_weight_h · BetP_i(θ_h).
    let source_risk: HashMap<String, f64> = {
        let mut s = tx.prepare(
            "SELECT site_id, hypothesis_expr, pignistic_probability FROM dsmt_fusion
             WHERE run_id = ?1",
        )?;
        let v: Vec<(String, String, f64)> = s
            .query_map(params![source_run], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<f64>>(2)?.unwrap_or(0.0),
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut out: HashMap<String, f64> = HashMap::new();
        for (site, expr, p) in v {
            if let Some(w) = source_weights.get(&expr) {
                *out.entry(site).or_insert(0.0) += w * p;
            }
        }
        out
    };

    // Dominant source per site: the hypothesis with the largest pignistic
    // probability.  Read from dsmt_fusion where `dominant_source_flag = 1`.
    let dominant_source: HashMap<String, String> = {
        let mut s = tx.prepare(
            "SELECT site_id, hypothesis_expr FROM dsmt_fusion
             WHERE run_id = ?1 AND dominant_source_flag = 1",
        )?;
        let v: Vec<(String, String)> = s
            .query_map(params![source_run], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        v.into_iter().collect()
    };

    let conflict_by_site: HashMap<String, f64> = {
        let mut s = tx.prepare(
            "SELECT site_id, MAX(conflict_mass) FROM dsmt_fusion
             WHERE run_id = ?1 GROUP BY site_id",
        )?;
        let v: Vec<(String, f64)> = s
            .query_map(params![source_run], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<f64>>(1)?.unwrap_or(0.0),
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        v.into_iter().collect()
    };

    tx.execute(
        "DELETE FROM neutrosophic_memberships WHERE run_id = ?1",
        params![run_id],
    )?;
    tx.execute("DELETE FROM rankings WHERE run_id = ?1", params![run_id])?;

    let mut nins = tx.prepare(
        "INSERT INTO neutrosophic_memberships
         (run_id, site_id, criterion, truth, falsity, indeterminacy, criterion_score)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;
    let mut rins = tx.prepare(
        "INSERT INTO rankings
         (run_id, site_id, priority_score, rank, rank_band, rank_ci_lower, rank_ci_upper,
          rank_stability, dominant_source, conflict_level, indeterminacy_level, recommended_action)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
    )?;

    // ----- uncertainty propagation (plan §15) -----
    // First pass: compute per-draw `rank_stability` for every site from
    // the hotspot probability draws.  We rank all sites per draw and
    // ask: "for what fraction of draws does this site end up in the
    // band of its point-estimate rank?"  This `Stability_i` is the
    // (real, not proxied) rank-stability that feeds into `r_verify`.
    // A full per-draw rerun of belief + DSmT + neutrosophic is a
    // substantial extension tracked separately; here we propagate
    // hotspot uncertainty (the dominant source of rank uncertainty)
    // and document that explicitly in REPRODUCIBILITY.md.
    let n_sites = sites.len();
    let mut per_site_p: HashMap<String, Vec<f64>> = HashMap::new();
    {
        let mut s = tx.prepare(
            "SELECT site_id, draw_index, value FROM posterior_draws
             WHERE run_id = ?1 AND quantity = 'hotspot' ORDER BY draw_index",
        )?;
        let rows: Vec<(String, i64, f64)> = s
            .query_map(params![bayes_run], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, f64>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (site, _idx, v) in rows {
            per_site_p.entry(site).or_default().push(v);
        }
    }

    // For each draw, rank the sites by hotspot probability; record
    // each site's per-draw rank.
    let n_draws = per_site_p.values().map(|v| v.len()).max().unwrap_or(0);
    let mut per_site_ranks: HashMap<String, Vec<f64>> = HashMap::new();
    if n_draws > 0 {
        for d in 0..n_draws {
            let mut v: Vec<(String, f64)> = vec![];
            for site in &sites {
                if let Some(p) = per_site_p.get(site).and_then(|x| x.get(d)) {
                    v.push((site.clone(), *p));
                }
            }
            v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            for (i, (s, _)) in v.iter().enumerate() {
                per_site_ranks
                    .entry(s.clone())
                    .or_default()
                    .push((i + 1) as f64);
            }
        }
    }

    // We need a placeholder `priority` order to assign bands to per-draw
    // ranks (band is determined by the priority of the site currently
    // sitting at a given rank).  Compute it from the point-estimate
    // priorities before the site loop so the `r_verify` term can look
    // up `rank_stability` for any site.
    // First we re-run the criterion scores without `r_verify` (using
    // the placeholder 0) to get a provisional priority order, then
    // use that to compute per-draw stability, then redo the criterion
    // loop with the real `r_verify`.  This is a two-pass approach;
    // the alternative is to do a single pass and write rankings
    // after stability is known, but the single-pass approach is
    // cleaner because the band depends on the *priority*, which
    // depends on the *r_verify*, which depends on the *stability*,
    // which depends on the *band*.  We resolve the cycle by
    // computing an initial priority order using the placeholder
    // r_verify=0, then re-evaluating r_verify with the real
    // stability, and accepting that the band may shift as a
    // consequence (the user is told this in the docs).
    let mut priority: Vec<(String, f64, f64)> = vec![];
    for site in &sites {
        let conflict = conflict_by_site.get(site).copied().unwrap_or(0.0);
        let unc = upen.get(site).copied().unwrap_or(0.0);
        // First pass: r_verify = 0 (placeholder), so r_verify has no
        // contribution.  This gives us a "no-stability" priority
        // ordering that we use to assign bands to per-draw ranks.
        let r_verify_pp = 0.0_f64;
        let _ = r_verify_pp;
        let r_contam = hotspot_p.get(site).copied().unwrap_or(0.0);
        let r_source = source_risk.get(site).copied().unwrap_or(0.0);
        let r_exposure = exposure.get(site).copied().unwrap_or(0.0);
        let r_conf = confidence.get(site).copied().unwrap_or(0.0);
        let miss = missingness.get(site).copied().unwrap_or(0.0);
        let k = conflict;
        let q = ((1.0 - miss).max(0.0) * (1.0 - unc).max(0.0) * (1.0 - k).max(0.0)).clamp(0.0, 1.0);
        let criteria = [
            ("contamination_intensity".to_string(), r_contam),
            ("source_plausibility".to_string(), r_source),
            ("exposure_relevance".to_string(), r_exposure),
            ("confidence".to_string(), r_conf),
            ("field_verification".to_string(), 0.0_f64),
        ];
        let mut site_scores: Vec<(String, f64)> = vec![];
        let mut triplets: Vec<(String, Triplet, f64)> = vec![];
        for (name, r) in &criteria {
            let (t, s) = score_one(*r, q, eta);
            t.check()?;
            site_scores.push((name.clone(), s));
            triplets.push((name.clone(), t, s));
        }
        let p = weighted_priority(&site_scores, &weights)?;
        crate::audit::failure_rules::check_priority_finite(p)?;
        let avg_i = triplets.iter().map(|(_, t, _)| t.i).sum::<f64>() / triplets.len() as f64;
        priority.push((site.clone(), p, avg_i));
    }
    priority.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Now compute per-draw stability for every site using the
    // provisional priority order (bands depend on the rank position
    // in this order).
    let mut per_site_stab: HashMap<String, f64> = HashMap::new();
    let mut per_site_ci: HashMap<String, (f64, f64)> = HashMap::new();
    for (site, mut ranks) in per_site_ranks {
        let (lo, hi) = rank_ci(&mut ranks);
        let bands: Vec<String> = ranks
            .iter()
            .map(|&r| band_for(r, n_sites, band_crit, band_high, band_mod, &priority))
            .collect();
        let mode = mode_band(&bands);
        let st = stability(&bands, &mode);
        per_site_stab.insert(site.clone(), st);
        per_site_ci.insert(site, (lo, hi));
    }

    // Second pass: re-evaluate r_verify using the real per-draw
    // rank_stability (plan §14.1: RankInstability = 1 - Stability_i).
    let mut final_priority: Vec<(String, f64, f64)> = vec![];
    let mut final_triplets: HashMap<String, Vec<(String, Triplet, f64)>> = HashMap::new();
    for site in &sites {
        let conflict = conflict_by_site.get(site).copied().unwrap_or(0.0);
        let unc = upen.get(site).copied().unwrap_or(0.0);
        // Real RankInstability: 1 - per-draw Stability_i (default 0
        // when the site has no draws, which contributes nothing to
        // r_verify and is the most conservative choice).
        let stab = per_site_stab.get(site).copied().unwrap_or(0.0);
        let rank_instability = (1.0 - stab).clamp(0.0, 1.0);
        let r_verify = (0.4 * conflict + 0.4 * unc + 0.2 * rank_instability).clamp(0.0, 1.0);

        let r_contam = hotspot_p.get(site).copied().unwrap_or(0.0);
        let r_source = source_risk.get(site).copied().unwrap_or(0.0);
        let r_exposure = exposure.get(site).copied().unwrap_or(0.0);
        let r_conf = confidence.get(site).copied().unwrap_or(0.0);
        let miss = missingness.get(site).copied().unwrap_or(0.0);
        let k = conflict;
        let q = ((1.0 - miss).max(0.0) * (1.0 - unc).max(0.0) * (1.0 - k).max(0.0)).clamp(0.0, 1.0);

        let criteria = [
            ("contamination_intensity".to_string(), r_contam),
            ("source_plausibility".to_string(), r_source),
            ("exposure_relevance".to_string(), r_exposure),
            ("confidence".to_string(), r_conf),
            ("field_verification".to_string(), r_verify),
        ];
        let mut site_scores: Vec<(String, f64)> = vec![];
        let mut triplets: Vec<(String, Triplet, f64)> = vec![];
        for (name, r) in &criteria {
            let (t, s) = score_one(*r, q, eta);
            t.check()?;
            nins.execute(params![run_id, site, name, t.t, t.f, t.i, s])?;
            site_scores.push((name.clone(), s));
            triplets.push((name.clone(), t, s));
        }

        let p = weighted_priority(&site_scores, &weights)?;
        crate::audit::failure_rules::check_priority_finite(p)?;
        let avg_i = triplets.iter().map(|(_, t, _)| t.i).sum::<f64>() / triplets.len() as f64;
        final_priority.push((site.clone(), p, avg_i));
        final_triplets.insert(site.clone(), triplets);
    }
    drop(nins);

    final_priority.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    for (i, (site, p, ind)) in final_priority.iter().enumerate() {
        let band = if *p >= band_crit {
            "Critical"
        } else if *p >= band_high {
            "High"
        } else if *p >= band_mod {
            "Moderate"
        } else {
            "Low"
        };
        let rank = (i + 1) as i64;
        // Per-draw rank CI (2.5/97.5 percentiles) and stability.
        let (ci_lo, ci_hi) = per_site_ci
            .get(site)
            .copied()
            .unwrap_or((rank as f64, rank as f64));
        let stability_val = per_site_stab.get(site).copied().unwrap_or(0.0);
        let dominant = dominant_source
            .get(site)
            .cloned()
            .unwrap_or_else(|| "Θ".to_string());
        // Soft, decision-support language — these are *hints*, not directives
        // (per refined_outline.md prohibited-claims list).
        let rec = match band {
            "Critical" => {
                "Verification strongly suggested; expand sampling and re-test in next campaign."
            }
            "High" => "Consider targeted verification within the current season.",
            "Moderate" => "Maintain routine monitoring; reassess in next cycle.",
            _ => "Continue passive monitoring; no immediate action indicated.",
        };
        let conflict = conflict_by_site.get(site).copied().unwrap_or(0.0);
        rins.execute(params![
            run_id,
            site,
            p,
            rank,
            band,
            ci_lo,
            ci_hi,
            stability_val,
            dominant,
            conflict,
            ind,
            rec,
        ])?;
    }
    drop(rins);

    tx.commit()?;
    Ok(())
}

fn canonicalize_weights(raw: &[(String, f64)]) -> Vec<(String, f64)> {
    let mut acc: HashMap<String, f64> = HashMap::new();
    for (criterion, weight) in raw {
        let canonical = match criterion.as_str() {
            "contamination" | "enrichment" | "contamination_intensity" => "contamination_intensity",
            "source" | "source_evidence" | "source_plausibility" => "source_plausibility",
            "exposure" | "exposure_relevance" => "exposure_relevance",
            "confidence" => "confidence",
            "field_verification" | "verification" => "field_verification",
            other => other,
        };
        *acc.entry(canonical.to_string()).or_insert(0.0) += *weight;
    }
    let order = [
        "contamination_intensity",
        "source_plausibility",
        "exposure_relevance",
        "confidence",
        "field_verification",
    ];
    let mut out = Vec::new();
    for key in order {
        if let Some(w) = acc.remove(key) {
            out.push((key.to_string(), w));
        }
    }
    out.extend(acc);
    out
}

fn band_for(
    rank: f64,
    n: usize,
    crit: f64,
    high: f64,
    mod_: f64,
    priorities: &[(String, f64, f64)],
) -> String {
    // Band is determined by the *priority* associated with the rank position
    // (i.e. the band of the site currently sitting at that rank), not the
    // rank of the current site.  This is the canonical "rank-band" mapping.
    let idx = (rank as usize).saturating_sub(1).min(n.saturating_sub(1));
    let p = priorities.get(idx).map(|x| x.1).unwrap_or(0.0);
    if p >= crit {
        "Critical".to_string()
    } else if p >= high {
        "High".to_string()
    } else if p >= mod_ {
        "Moderate".to_string()
    } else {
        "Low".to_string()
    }
}
