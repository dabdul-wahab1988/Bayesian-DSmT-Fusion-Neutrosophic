# `bayesdsm` CLI reference

All subcommands accept the global flag `--db <path>` (default
`bayesdsm.sqlite`). Subcommands write a row to `runs` and append to
`warnings` / `failures` as appropriate. Each subcommand is also a
checkpoint: you can re-run any of them after fixing upstream data.

| # | Subcommand | What it does | Reads | Writes |
|---|---|---|---|---|
| 1 | `init` | Create / migrate the SQLite schema. With `--force`, drops and recreates all objects. | – | 13 raw + 13 output tables, 8 views |
| 2 | `ingest` | Read every CSV in `--input-dir` matching `primary_input_contract.md`. SHA-256 hash, validate, insert. | CSVs in dir | `raw_*`, `input_files` |
| 3 | `audit` | Post-ingestion audits: referential integrity, range checks, leakage, hash re-check. | `raw_*` | `warnings` |
| 4 | `clean` | Unit harmonisation + non-detect handling (DL/√2 by default; `censored_bayes` available via config). | `raw_metal_concentrations` | `cleaned_metals` |
| 5 | `features` | CF, EF, Igeo, PLI, site-level normalisations. | `cleaned_metals`, `raw_*` | `features` |
| 6 | `bayes` | Run the three MCMC models (lognormal enrichment, evidence-weighted Dirichlet source, latent-logistic hotspot). | `features` | `posterior_summaries`, `posterior_draws`, `bayesian_diagnostics` |
| 7 | `belief` | Posterior-to-belief: build focal mass functions for the hotspot and source layers. | `posterior_summaries` | `belief_assignments` |
| 8 | `dsmt` | DSmT conjunctive fusion + generalised pignistic transformation. | `belief_assignments` | `dsmt_fusion` |
| 9 | `neutrosophic` | Build T/F/I triplets per criterion, score, weight, sort. | `features`, `posterior_summaries`, `dsmt_fusion` | `neutrosophic_memberships`, `rankings` |
| 10 | `rank` | Final ranking with band assignment and rank-CI / stability propagation. | `dsmt_fusion` (via posterior_draws) | `rankings` (UPDATE) |
| 11 | `validate` | If `raw_validation_labels` has rows with `independent_of_features = 1`, compute Spearman, top-k overlap, calibration. | `rankings`, `raw_validation_labels` | `warnings` (no new table) |
| 12 | `sensitivity` | Re-run `bayes → neutrosophic` under each perturbation in a fixed grid; record per-site `priority_score`. | `config` (perturbations), all upstream | `sensitivity_results` |
| 13 | `export` | Materialise 8 R-ready views to CSV under `--out-dir`. | views | `out/<view>.csv`, `export_manifest` |

## Flags

| Flag | Subcommand | Meaning |
|---|---|---|
| `--db <path>` | global | SQLite file (default `bayesdsm.sqlite`). |
| `--force` | `init` | Drop and recreate every object. |
| `--input-dir <path>` | `ingest` | Directory containing the primary input CSVs. |
| `--seed <u64>` | `bayes` | Override the `random_seed` from `config`. |
| `--out-dir <path>` | `export`, `sensitivity` | Where to write CSVs (default `out/`, `out/sensitivity`). |

## Exit codes

* `0` — subcommand completed.
* non-zero — `STOP` raised; see `failures` table for `code` and `message`.

## STOP / WARN / DOWNGRADE codes

STOP rows are written to the `failures` table with a machine-readable `code`
and human-readable `message`. The validation rule implementations live in
`bayesdsm-core/src/audit/failure_rules.rs`.
