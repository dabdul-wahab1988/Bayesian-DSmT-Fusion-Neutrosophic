//! Validate subcommand (plan §15 + REPRODUCIBILITY.md).
//!
//! Reads `raw_validation_labels` and the latest `rankings` /
//! `posterior_summaries`, then computes three agreement metrics and
//! writes them into `warnings.context_json` for the most recent run.
//!
//! - **Spearman ρ** between predicted rank (from `rankings`) and the
//!   expert's hotspot label.  Sites labelled "yes" are assigned score 1,
//!   "no" are assigned 0; the rank correlation is computed by
//!   rank-transforming both sides.
//! - **Top-k overlap** for k ∈ {1, 3, 5}: the intersection of the k
//!   highest-priority sites with the k sites the expert labelled "yes",
//!   normalised by k.
//! - **Calibration**: the mean predicted `hotspot_probability` among
//!   the "yes"-labelled sites vs the mean among the "no"-labelled
//!   sites, plus the Brier score.
//!
//! All metrics are written as a JSON object in a single warning row
//! (severity = "INFO", code = "V1500") so the R export can pick them
//! up via `v_run_manifest`.

use std::collections::HashMap;

use rusqlite::{params, Connection};
use serde_json::json;

use crate::error::Result;

/// Compute Spearman ρ between two parallel slices.  Returns `None` if
/// fewer than 3 paired observations or if either slice is constant.
pub fn spearman_rho(xs: &[f64], ys: &[f64]) -> Option<f64> {
    if xs.len() != ys.len() || xs.len() < 3 {
        return None;
    }
    let rx = rank_average(xs);
    let ry = rank_average(ys);
    pearson_rho(&rx, &ry)
}

/// Compute Pearson ρ between two parallel slices.
pub fn pearson_rho(xs: &[f64], ys: &[f64]) -> Option<f64> {
    if xs.len() != ys.len() || xs.len() < 2 {
        return None;
    }
    let n = xs.len() as f64;
    let mx = xs.iter().sum::<f64>() / n;
    let my = ys.iter().sum::<f64>() / n;
    let mut sxx = 0.0;
    let mut syy = 0.0;
    let mut sxy = 0.0;
    for (&x, &y) in xs.iter().zip(ys.iter()) {
        let dx = x - mx;
        let dy = y - my;
        sxx += dx * dx;
        syy += dy * dy;
        sxy += dx * dy;
    }
    if sxx <= 0.0 || syy <= 0.0 {
        return None;
    }
    Some(sxy / (sxx * syy).sqrt())
}

/// Average-rank transform with tie handling.
fn rank_average(xs: &[f64]) -> Vec<f64> {
    let n = xs.len();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| {
        xs[a]
            .partial_cmp(&xs[b])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut ranks = vec![0.0_f64; n];
    let mut i = 0;
    while i < n {
        let mut j = i;
        while j + 1 < n && xs[order[j + 1]] == xs[order[i]] {
            j += 1;
        }
        let avg = 0.5 * ((i + 1) as f64 + (j + 1) as f64);
        for k in i..=j {
            ranks[order[k]] = avg;
        }
        i = j + 1;
    }
    ranks
}

/// Top-k overlap:  |predicted_top_k ∩ expert_yes| / k.
/// `predicted_ranks` is the priority (higher = more priority).  The
/// k highest-priority sites are taken.  `expert_yes_sites` is a set of
/// site_ids labelled "yes".
pub fn topk_overlap(predicted: &[(String, f64)], expert_yes: &[String], k: usize) -> f64 {
    if k == 0 || predicted.is_empty() {
        return 0.0;
    }
    let mut sorted: Vec<(String, f64)> = predicted.to_vec();
    sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let top: std::collections::HashSet<&str> = sorted
        .iter()
        .take(k.min(sorted.len()))
        .map(|(s, _)| s.as_str())
        .collect();
    let yes: std::collections::HashSet<&str> = expert_yes.iter().map(|s| s.as_str()).collect();
    let intersect = top.intersection(&yes).count();
    intersect as f64 / k.min(sorted.len()) as f64
}

/// Brier score:  mean (predicted - observed)^2.
pub fn brier_score(predicted: &[f64], observed: &[f64]) -> Option<f64> {
    if predicted.len() != observed.len() || predicted.is_empty() {
        return None;
    }
    let s: f64 = predicted
        .iter()
        .zip(observed.iter())
        .map(|(p, o)| (p - o).powi(2))
        .sum();
    Some(s / predicted.len() as f64)
}

/// Main entry point:  compute validation metrics and write a warning
/// row with the JSON context.  Returns the number of independent
/// labels processed.
pub fn run(conn: &mut Connection) -> Result<usize> {
    let tx = conn.transaction()?;

    // Pull independent labels.
    let labels: Vec<(String, String, String)> = {
        let mut s = tx.prepare(
            "SELECT site_id, label_name, label_value FROM raw_validation_labels
             WHERE independent_of_features = 1",
        )?;
        let v: Vec<(String, String, String)> = s
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        v
    };
    if labels.is_empty() {
        tx.commit()?;
        return Ok(0);
    }

    // We need: (a) predicted ranks (from latest rankings), (b) predicted
    // hotspot probabilities (from latest posterior_summaries).
    let latest_rank_run: Option<i64> = tx
        .query_row("SELECT MAX(run_id) FROM rankings", [], |r| {
            r.get::<_, Option<i64>>(0)
        })
        .ok()
        .flatten();
    let latest_post_run: Option<i64> = tx
        .query_row("SELECT MAX(run_id) FROM posterior_summaries", [], |r| {
            r.get::<_, Option<i64>>(0)
        })
        .ok()
        .flatten();
    let (Some(rank_run), Some(post_run)) = (latest_rank_run, latest_post_run) else {
        tx.commit()?;
        return Ok(labels.len());
    };

    // Predicted rank per site.
    let ranks: HashMap<String, i64> = {
        let v = {
            let mut s = tx.prepare("SELECT site_id, rank FROM rankings WHERE run_id = ?1")?;
            let x: Vec<(String, i64)> = s
                .query_map(params![rank_run], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            x
        };
        v.into_iter().collect()
    };
    // Predicted priority (so higher = more priority) and hotspot prob.
    let priority: HashMap<String, f64> = {
        let v = {
            let mut s =
                tx.prepare("SELECT site_id, priority_score FROM rankings WHERE run_id = ?1")?;
            let x: Vec<(String, f64)> = s
                .query_map(params![rank_run], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            x
        };
        v.into_iter().collect()
    };
    let hotspot: HashMap<String, f64> = {
        let v = {
            let mut s = tx.prepare(
                "SELECT site_id, posterior_mean FROM posterior_summaries
                 WHERE run_id = ?1 AND quantity = 'hotspot_probability'",
            )?;
            let x: Vec<(String, f64)> = s
                .query_map(params![post_run], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            x
        };
        v.into_iter().collect()
    };

    // For each label_name (e.g. "hotspot", "source_label"), compute
    // metrics separately and emit one warning row each.  This keeps
    // the schema flexible when the user supplies multiple label
    // categories.
    let mut label_groups: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for (site, name, value) in &labels {
        label_groups
            .entry(name.clone())
            .or_default()
            .push((site.clone(), value.clone()));
    }

    let mut emitted = 0_usize;
    for (label_name, group) in &label_groups {
        // Pair each site in `group` with its predicted rank / probability.
        // Sites without a rank or probability are skipped.
        let mut paired_ranks: Vec<(f64, f64)> = vec![]; // (predicted_rank, label_score)
        let mut paired_probs: Vec<(f64, f64)> = vec![]; // (predicted_prob, label_score)
        let mut expert_yes: Vec<String> = vec![];
        let mut priority_pairs: Vec<(String, f64)> = vec![];
        for (site, val) in group {
            let score = match val.to_ascii_lowercase().as_str() {
                "yes" | "true" | "1" => 1.0,
                "no" | "false" | "0" => 0.0,
                _ => continue,
            };
            if let Some(r) = ranks.get(site) {
                paired_ranks.push((*r as f64, score));
            }
            if let Some(p) = hotspot.get(site) {
                paired_probs.push((*p, score));
            }
            if let Some(pri) = priority.get(site) {
                priority_pairs.push((site.clone(), *pri));
            }
            if score > 0.5 {
                expert_yes.push(site.clone());
            }
        }

        if paired_ranks.is_empty() && paired_probs.is_empty() {
            continue;
        }

        // Spearman on ranks (lower rank = higher priority, so we
        // negate to align direction: -rank vs label_score).
        let spearman: Option<f64> = if paired_ranks.len() >= 3 {
            let xs: Vec<f64> = paired_ranks.iter().map(|(r, _)| -r).collect();
            let ys: Vec<f64> = paired_ranks.iter().map(|(_, s)| *s).collect();
            spearman_rho(&xs, &ys)
        } else {
            None
        };

        // Top-k overlap.
        let top1 = topk_overlap(&priority_pairs, &expert_yes, 1);
        let top3 = topk_overlap(&priority_pairs, &expert_yes, 3);
        let top5 = topk_overlap(&priority_pairs, &expert_yes, 5);

        // Calibration: mean predicted p among yes / no.
        let (mean_p_yes, mean_p_no, brier) = if !paired_probs.is_empty() {
            let mut yes_p = vec![];
            let mut no_p = vec![];
            let mut all_p = vec![];
            let mut all_o = vec![];
            for (p, o) in &paired_probs {
                all_p.push(*p);
                all_o.push(*o);
                if *o > 0.5 {
                    yes_p.push(*p);
                } else {
                    no_p.push(*p);
                }
            }
            let my = if yes_p.is_empty() {
                0.0
            } else {
                yes_p.iter().sum::<f64>() / yes_p.len() as f64
            };
            let mn = if no_p.is_empty() {
                0.0
            } else {
                no_p.iter().sum::<f64>() / no_p.len() as f64
            };
            (my, mn, brier_score(&all_p, &all_o).unwrap_or(f64::NAN))
        } else {
            (0.0, 0.0, f64::NAN)
        };

        let n_paired = paired_ranks.len();

        let ctx = json!({
            "label_name": label_name,
            "n_sites": n_paired,
            "spearman_rho": spearman,
            "top1_overlap": top1,
            "top3_overlap": top3,
            "top5_overlap": top5,
            "mean_p_yes": mean_p_yes,
            "mean_p_no": mean_p_no,
            "brier_score": brier,
            "rank_run_id": rank_run,
            "post_run_id": post_run,
        });

        // Use a fresh warning row tied to the latest run.  Severity
        // "INFO" so it doesn't trigger the WARN/DOWNGRADE display
        // (the user can still see it in v_run_manifest).
        tx.execute(
            "INSERT INTO warnings (run_id, module, severity, code, message, context_json)
             VALUES (?1, 'validate', 'INFO', 'V1500', ?2, ?3)",
            params![
                rank_run,
                format!("validate: {} sites, label='{}'", n_paired, label_name),
                ctx.to_string(),
            ],
        )?;
        emitted += 1;
    }

    tx.commit()?;
    Ok(emitted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spearman_perfect_positive() {
        let xs = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let ys = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        let r = spearman_rho(&xs, &ys).unwrap();
        assert!((r - 1.0).abs() < 1e-9, "r={r}");
    }

    #[test]
    fn spearman_perfect_negative() {
        let xs = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let ys = vec![50.0, 40.0, 30.0, 20.0, 10.0];
        let r = spearman_rho(&xs, &ys).unwrap();
        assert!((r + 1.0).abs() < 1e-9, "r={r}");
    }

    #[test]
    fn topk_basic() {
        let pred = vec![
            ("A".to_string(), 0.9),
            ("B".to_string(), 0.7),
            ("C".to_string(), 0.5),
        ];
        let yes = vec!["A".to_string(), "C".to_string()];
        assert!((topk_overlap(&pred, &yes, 1) - 1.0).abs() < 1e-9);
        assert!((topk_overlap(&pred, &yes, 2) - 0.5).abs() < 1e-9);
        assert!((topk_overlap(&pred, &yes, 3) - 2.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn brier_perfect() {
        let p = vec![0.0, 1.0, 0.0, 1.0];
        let o = vec![0.0, 1.0, 0.0, 1.0];
        assert_eq!(brier_score(&p, &o), Some(0.0));
    }
}
