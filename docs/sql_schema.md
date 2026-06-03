# SQL schema reference

The schema is the **single source of truth**. There are two SQL files under
`bayesdsm-core/src/schema/`:

* `0001_init.sql` — tables (audit, raw inputs, internal outputs, exports)
* `0002_views.sql` — eight R-ready views

The 13 internal output tables that match `primary_input_contract.md §7` and
`plan.txt §16` are reproduced below for quick reference.

## Audit & run

| Table | Purpose | Written by |
|---|---|---|
| `schema_migrations` | Applied migrations log | `db::migrate` |
| `input_files` | SHA-256 of every primary input | `ingest` |
| `runs` | One row per `bayesdsm` subcommand invocation | every `cmd_*` |
| `warnings` | `WARN` / `DOWNGRADE` rows | any module |
| `failures` | `STOP` rows | any module |
| `config` | Coerced mirror of `raw_config_parameters` | `ingest` |

## Raw inputs (immutable; written by `ingest` only)

| Table | Required? | Contract section |
|---|---|---|
| `raw_sites` | required | `site_metadata.csv` |
| `raw_sampling_events` | required | `sampling_events.csv` |
| `raw_metal_concentrations` | required | `metal_concentrations.csv` |
| `raw_background_values` | required | `background_values.csv` |
| `raw_dsmt_hypotheses` | required | `dsmt_hypotheses.csv` |
| `raw_config_parameters` | required | `config_parameters.csv` |
| `raw_source_indicators` | required | `source_indicators.csv` |
| `raw_exposure_indicators` | required | `exposure_indicators.csv` |
| `raw_confidence_indicators` | required | `confidence_indicators.csv` |
| `raw_leakage_rules` | required | `leakage_rules.csv` |
| `raw_stakeholder_weights` | required | `stakeholder_weights.csv` |
| `raw_validation_labels` | recommended | `validation_labels.csv` |
| `raw_dsmt_constraints` | recommended | `dsmt_constraints.csv` |
| `raw_data_dictionary` | optional | `data_dictionary.csv` |
| `raw_allowed_values` | optional | `allowed_values.csv` |

## Internal outputs (one table per pipeline step)

| Table | Module | Plan § | Notes |
|---|---|---|---|
| `cleaned_metals` | `clean` | §16.1 | One row per (sample, metal) with unit-harmonised value and detect flag. |
| `features` | `features` | §16.2 | `feature_name ∈ {cf_<metal>, ef_<metal>, igeo_<metal>, pli, exposure_norm, missingness, confidence, uncertainty_penalty, …}`. |
| `posterior_summaries` | `bayes` | §16.3 | `quantity ∈ {enrichment_probability, source_support_probability, hotspot_probability}`. |
| `bayesian_diagnostics` | `bayes` | §9 | One row per scalar quantity, per chain. |
| `posterior_draws` | `bayes` | §10 | Long-format draws; needed by §15 uncertainty propagation. |
| `belief_assignments` | `belief` | §16.4 | `evidence_layer ∈ {hotspot, source, chemical, exposure, confidence}`. |
| `dsmt_fusion` | `dsmt` | §16.5 | Includes `conflict_mass` and `pignistic_probability` per focal element. |
| `neutrosophic_memberships` | `neutrosophic` | §16.6 | One row per (site, criterion). |
| `rankings` | `rank` | §16.7 | Final rank + band + CI + stability + recommended action. |
| `sensitivity_results` | `sensitivity` | – | `parameter_name`, `perturbation`, `quantity=value` per site. |
| `export_manifest` | `export` | – | One row per exported CSV. |

## R-ready views (`0002_views.sql`)

| View | Grain | Useful for |
|---|---|---|
| `v_rankings_full` | one row per site (latest run) | final ranking plots |
| `v_neutrosophic_long` | one row per (site, criterion) | T/F/I heatmap |
| `v_posterior_summaries_wide` | one row per site, columns per quantity | model comparison |
| `v_belief_assignments_long` | one row per (site, layer, hypothesis) | belief mass inspection |
| `v_dsmt_fused_long` | one row per (site, hypothesis) | conflict bar chart |
| `v_features_wide` | one row per site, columns per feature | driver analysis |
| `v_cleaned_metals_long` | one row per (sample, metal) | QA plots |
| `v_run_manifest` | one row per `runs` row with warnings / failures joined | run summary |

`export` materialises each of these to `<out-dir>/<view>.csv` and records
the file in `export_manifest`.
