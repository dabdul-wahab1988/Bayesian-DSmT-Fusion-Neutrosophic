# Mapping: manuscript outline → Rust code

This file makes every manuscript claim traceable to a specific Rust module
and SQLite table. The chapter is structured around the **§-numbered**
mathematical plan (`plan.txt`); the scientific narrative
(`refined_outline.md`) is realised by the same code with the same numbering.

## Module map (`plan.txt` §18)

| `plan.txt` section | Topic | Code module | Test |
|---|---|---|---|
| §1 | Pipeline diagram | `bayesdsm-cli/src/main.rs` (`cmd_*`) | `bayesdsm-core/tests/e2e.rs::full_pipeline_runs_and_invariants_hold` |
| §2 | Primary input variables | `bayesdsm-core/src/ingest/csv_read.rs` + `ingest/sqlite_insert.rs` | `ingest::tests::*` |
| §3 | Rust ingestion + SHA-256 | `audit/hashing.rs`, `ingest/sqlite_insert.rs` | `audit::hashing::tests` |
| §4.1 | Unit harmonisation | `clean/units.rs` | `clean::units::tests::*` |
| §4.2 | Non-detect handling | `clean/nondetect.rs` | `clean::nondetect::tests::*` |
| §5.1 | Site-metal concentration summary | `features/mod.rs::aggregate` | `features::tests::*` |
| §5.2 | Contamination factor | `features/contamination_factor.rs` | `features::contamination_factor::tests::*` |
| §5.3 | Enrichment factor | `features/enrichment_factor.rs` | `features::enrichment_factor::tests::*` |
| §5.4 | Igeo | `features/igeo.rs` | `features::igeo::tests::*` |
| §5.5 | PLI | `features/pli.rs` | `features::pli::tests::*` |
| §5.6 | Normalised feature | `features/normalize.rs` | `features::normalize::tests::*` |
| §6.1 | Bayesian lognormal enrichment | `bayes/lognormal_enrichment.rs` | `bayes::lognormal_enrichment::tests::*` |
| §6.2 | Bayesian metal-burden | `bayes/burden.rs` | `bayes::burden::tests::*` |
| §7.1 | Dirichlet source support | `bayes/dirichlet_source.rs` | `bayes::dirichlet_source::tests::*` |
| §7.2 | Supervised multinomial | `bayes/dirichlet_source.rs::supervised` | `bayes::dirichlet_source::tests::*` |
| §8.1 | Latent logistic hotspot | `bayes/hotspot_latent.rs` | `bayes::hotspot_latent::tests::*` |
| §8.2 | Supervised hotspot | `bayes/hotspot_latent.rs::supervised` | `bayes::hotspot_latent::tests::*` |
| §9 | R-hat, ESS | `bayes/diagnostics.rs`, `math/stats.rs` | `math::stats::tests::rhat_good_chains` |
| §10 | Posterior summaries | `bayes/posterior_summary.rs` | `bayes::posterior_summary::tests::*` |
| §11.1 | Uncertainty score from CI | `belief/posterior_to_belief.rs` | `belief::posterior_to_belief::tests::*` |
| §11.2 | Hotspot belief | `belief/posterior_to_belief.rs::hotspot_belief` | `belief::posterior_to_belief::tests::*` |
| §11.3 | Source belief | `belief/posterior_to_belief.rs::source_belief` | `belief::posterior_to_belief::tests::*` |
| §12 | DSmT conjunctive fusion | `dsmt/fusion.rs` | `dsmt::fusion::tests::simple_consensus` |
| §12.1 | Code-friendly DSmT algorithm | `dsmt/canonicalize.rs`, `dsmt/free_model.rs`, `dsmt/hybrid_model.rs` | `dsmt::canonicalize::tests::*` |
| §13 | Pignistic transformation | `dsmt/pignistic.rs` | `dsmt::pignistic::tests::pignistic_preserves_total_mass` |
| §14.1 | Criterion risk score | `neutrosophic/criterion_score.rs` | `neutrosophic::criterion_score::tests::*` |
| §14.2 | Criterion confidence | `neutrosophic/membership.rs` | `neutrosophic::membership::tests::sums_to_q` |
| §14.3 | T / F / I | `neutrosophic/membership.rs::triplet` | `neutrosophic::membership::tests::*` |
| §14.4 | Criterion score | `neutrosophic/membership.rs::criterion_score` | `neutrosophic::membership::tests::*` |
| §14.5 | Final priority score | `neutrosophic/mod.rs` | `neutrosophic::tests::*` |
| §15 | Uncertainty propagation | `neutrosophic/uncertainty.rs`, `neutrosophic/mod.rs` | `neutrosophic::uncertainty::tests::*` |
| §17.1 | Invalid concentration | `audit/failure_rules.rs::check_concentration_positive` | `audit::failure_rules::tests::*` |
| §17.2 | Invalid background | `audit/failure_rules.rs::check_background_positive` | `audit::failure_rules::tests::*` |
| §17.3 | Invalid weights | `audit/failure_rules.rs::check_weights_sum_to_one` | `audit::failure_rules::tests::*` |
| §17.4 | Invalid posterior p | `bayes/posterior_summary.rs` (post-assert) | `bayes::posterior_summary::tests::*` |
| §17.5 | Invalid belief mass | `belief/mass_validation.rs` (logical check) | `bayesdsm-core/tests/e2e.rs` (end-to-end) |
| §17.6 | Invalid DSmT expression | `dsmt/expression.rs::parse` | `dsmt::expression::tests::*` |
| §17.7 | Invalid T/F/I | `neutrosophic/membership.rs::Triplet::check` | `neutrosophic::membership::tests::*` |
| §17.8 | Non-finite priority | `audit/failure_rules.rs::check_priority_finite` | `neutrosophic::mod.rs` (call) |

## Manuscript outline (`refined_outline.md`) → code

| Manuscript section | Quantitative claim | Code / Table |
|---|---|---|
| §2 (Background) | uses contamination factor, EF, Igeo, PLI | `features::*` |
| §2 | Bayesian calibration of enrichment | `bayes/lognormal_enrichment.rs` → `posterior_summaries` |
| §2 | source-attribution under overlap | `bayes/dirichlet_source.rs` → `posterior_summaries` |
| §2 | hotspot intensity with uncertainty | `bayes/hotspot_latent.rs` → `posterior_summaries` |
| §3 (Methods) | generalised belief assignments | `belief/posterior_to_belief.rs` → `belief_assignments` |
| §3 | DSmT fusion (free + hybrid) | `dsmt/fusion.rs` → `dsmt_fusion` |
| §3 | neutrosophic truth/falsity/indeterminacy | `neutrosophic/membership.rs` → `neutrosophic_memberships` |
| §3 | weighted priority + rank bands | `neutrosophic/mod.rs` → `rankings` |
| §3 | sensitivity to enrich threshold | `bayesdsm-cli::cmd_sensitivity` → `sensitivity_results` |
| §3 | independent validation (Spearman, top-k) | `bayesdsm-cli::cmd_validate` → `warnings` |
| §3 | audit / reproducibility | `audit/run_manifest.rs` → `runs`; `audit/hashing.rs` → `input_files.sha256` |
| §4 (Results) | SQLite single source of truth | schema `0001_init.sql` + `0002_views.sql` |
| §4 | R-ready views | `v_rankings_full`, `v_neutrosophic_long`, `v_posterior_summaries_wide`, `v_belief_assignments_long`, `v_dsmt_fused_long`, `v_features_wide`, `v_cleaned_metals_long`, `v_run_manifest` |
| §5 (Discussion) | conflict-preserving fusion | `dsmt/fusion.rs` records `conflict_mass` per site |
| §5 | uncertainty bands | `rankings.rank_ci_lower/upper`, `rank_stability` |

## STOP / WARN / DOWNGRADE codes

All rules in `plan.txt §17` are implemented in
`bayesdsm-core/src/audit/failure_rules.rs` and called from the boundary of
every module. Codes are stable identifiers you can grep for:

| Code | Section | Trigger |
|---|---|---|
| `E1201` | §17.1 | `y_clean ≤ 0` without a non-detect flag |
| `E1202` | §17.2 | `B_m ≤ 0` |
| `E1203` | §17.3 | `|Σw_c − 1| > 1e-8` |
| `E1204` | §17.4 | `p ∉ [0,1]` |
| `E1205` | §17.5 | `m(A) < 0` or `|Σm − 1| > 1e-8` |
| `E1801` | §17.6 | unknown hypothesis symbol in DSmT expression |
| `E1407` | §17.7 | T / F / I outside `[0,1]` |
| `E1208` | §17.8 | `Priority = NaN / ±∞` |
| `E0901` | §9   | `R̂ > 1.01` (WARN) / `> 1.05` (DOWNGRADE) |

Codes `E0001–E0099` are reserved for `primary_input_contract.md §12`
package-level STOPs (missing required file/column, invalid site references,
non-positive background, etc.) which are surfaced during `ingest` and `audit`.
