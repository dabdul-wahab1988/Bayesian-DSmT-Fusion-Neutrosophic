-- 0002_views.sql
-- R-ready SQL views. Each view is a stable, documented entry point for downstream analysis.

-- 1. One row per site, with the final priority and all the diagnostics.
CREATE VIEW IF NOT EXISTS v_rankings_full AS
SELECT
    r.run_id,
    r.site_id,
    r.priority_score,
    r.rank,
    r.rank_band,
    r.rank_ci_lower,
    r.rank_ci_upper,
    r.rank_stability,
    r.dominant_source,
    r.conflict_level,
    r.indeterminacy_level,
    r.recommended_action,
    s.latitude,
    s.longitude
FROM rankings r
LEFT JOIN raw_sites s ON s.site_id = r.site_id;

-- 2. Long-format T/F/I per (site, criterion) for ternary plots.
CREATE VIEW IF NOT EXISTS v_neutrosophic_long AS
SELECT
    run_id, site_id, criterion,
    truth AS T, falsity AS F, indeterminacy AS I, criterion_score
FROM neutrosophic_memberships;

-- 3. One row per site, columns per (metal, quantity) for heatmaps.
CREATE VIEW IF NOT EXISTS v_posterior_summaries_wide AS
SELECT
    run_id, site_id,
    SUM(CASE WHEN quantity = 'enrichment_probability'     THEN posterior_mean END) AS enrichment_overall,
    SUM(CASE WHEN quantity = 'hotspot_probability'         THEN posterior_mean END) AS hotspot_probability,
    SUM(CASE WHEN quantity = 'source_support_probability'  THEN posterior_mean END) AS source_overall
FROM posterior_summaries
GROUP BY run_id, site_id;

-- 4. Belief assignments (long).
CREATE VIEW IF NOT EXISTS v_belief_assignments_long AS
SELECT
    run_id, site_id, evidence_layer, hypothesis_expr,
    belief_mass, uncertainty_score, belief_rule
FROM belief_assignments;

-- 5. DSmT fused masses (long).
CREATE VIEW IF NOT EXISTS v_dsmt_fused_long AS
SELECT
    run_id, site_id, hypothesis_expr,
    fused_mass, conflict_mass, pignistic_probability,
    dominant_source_flag, dsmt_model
FROM dsmt_fusion;

-- 6. Features wide: one row per site, columns per feature_name.
CREATE VIEW IF NOT EXISTS v_features_wide AS
SELECT
    run_id, site_id,
    MAX(CASE WHEN feature_name = 'pli'                   THEN feature_value END) AS pli,
    MAX(CASE WHEN feature_name = 'cf_As'                 THEN feature_value END) AS cf_As,
    MAX(CASE WHEN feature_name = 'cf_Cd'                 THEN feature_value END) AS cf_Cd,
    MAX(CASE WHEN feature_name = 'ef_As'                 THEN feature_value END) AS ef_As,
    MAX(CASE WHEN feature_name = 'igeo_As'               THEN feature_value END) AS igeo_As,
    MAX(CASE WHEN feature_name = 'exposure_norm'         THEN feature_value_normalized END) AS exposure_norm,
    MAX(CASE WHEN feature_name = 'uncertainty_penalty'   THEN feature_value END) AS uncertainty_penalty
FROM features
GROUP BY run_id, site_id;

-- 7. Cleaned metals (long).
CREATE VIEW IF NOT EXISTS v_cleaned_metals_long AS
SELECT
    run_id, site_id, sample_id, metal,
    value_standard, unit_standard, detect_flag, cleaning_method
FROM cleaned_metals;

-- 8. Run manifest joined with warnings and failures.
CREATE VIEW IF NOT EXISTS v_run_manifest AS
SELECT
    r.run_id, r.project_id, r.random_seed, r.started_at, r.finished_at,
    r.status, r.module,
    (SELECT COUNT(*) FROM warnings w WHERE w.run_id = r.run_id) AS n_warnings,
    (SELECT COUNT(*) FROM failures f WHERE f.run_id = r.run_id) AS n_failures
FROM runs r;

-- Indexes for the views.
CREATE INDEX IF NOT EXISTS idx_rankings_run        ON rankings(run_id);
CREATE INDEX IF NOT EXISTS idx_posterior_run       ON posterior_summaries(run_id);
CREATE INDEX IF NOT EXISTS idx_features_run        ON features(run_id);
CREATE INDEX IF NOT EXISTS idx_cleaned_run         ON cleaned_metals(run_id);
CREATE INDEX IF NOT EXISTS idx_belief_run          ON belief_assignments(run_id);
CREATE INDEX IF NOT EXISTS idx_dsmt_run            ON dsmt_fusion(run_id);
CREATE INDEX IF NOT EXISTS idx_neutro_run          ON neutrosophic_memberships(run_id);
CREATE INDEX IF NOT EXISTS idx_draws_run           ON posterior_draws(run_id, quantity, site_id);
CREATE INDEX IF NOT EXISTS idx_sites_geo           ON raw_sites(latitude, longitude);
