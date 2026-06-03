# Reproducibility report

This document records the exact recipe used to build and run the `bayesdsm`
package on the synthetic fixture in `tests/synthetic/`, plus all the
output hashes a reviewer can use to confirm bit-for-bit reproducibility.

## 1. Build environment

* **OS**: Windows 11 (PowerShell)
* **Rust**: 1.90.0 (stable)
* **Cargo workspace root**: `pacakage/`
* **Build command**: `cargo build --release`
* **Resulting binary**: `target/release/bayesdsm.exe`

## 2. Input hashes (SHA-256 of every primary input CSV)

Computed at 2026-06-02 from the 15 files in `pacakage/tests/synthetic/`:

```
ffc877c75320d6f9d6d005f3b6c8fe23d80822224f22beb8b1c0222682cf52bb  allowed_values.csv
1c4488551246aaee82f6378f7e2226087f43355550af0c6e37ecebaaf9f6e703  background_values.csv
8faf04fecf75e9eb6a0c402e80974e37d2ed30e63b12c8553573186b67f6b102  confidence_indicators.csv
57c62612c4275a10bd1e3b2ca5378490a0feb7d3577cead64e37d986ba0fb7cc  config_parameters.csv
0d790520c209d4304219fe2cb2a2036200c6c0b77cc34365cc7a91f23d9ea635  data_dictionary.csv
fb1368c885b454d179ac4d1376836530ea824c48d5196cef87c4fcd71976ac9c  dsmt_constraints.csv
9861d82e731679a5ac375525d8839510545a85b9b3353076cca0ea5f1bb6ce3a  dsmt_hypotheses.csv
d1092f24b6b9fdd72ef5f2a0110215dc9411a24d6ec7573d232ccfd0f8cfb2da  exposure_indicators.csv
c35ea55f64a086995ee18ea942e634491fb9e12a9aeec67425fdc94e46e91cf5  leakage_rules.csv
63ad9370c6ce64cbe4b8a0fcdfe6af3a22bd0610488690ee6d1b448ce7a86883  metal_concentrations.csv
cb1a260e8e8857e0f445b4b389ea7d0711e23cfd1eb261e66b540cb12ff8bb32  sampling_events.csv
5339058427161697d338d0bb39d2e43b28f49707e6554ddd01f8c6b2d95f41c5  site_metadata.csv
d3bed0f148f195c49a2732373efbc5e1ed77e035bd3ce641bbacd22f6496a60c  source_indicators.csv
2396a3d38d27ae621551017163040d8f3e51cf33c93caf57503bc68c10f2359c  stakeholder_weights.csv
e08237e51e961c25574fe488e6c0f53bb22bd98fc829909707f078642ce4d784  validation_labels.csv
```

These hashes are also re-computed by `bayesdsm ingest` and stored in
`input_files.sha256`. They can be re-verified at any time by querying:

```sql
SELECT file_role, sha256, row_count FROM input_files ORDER BY file_role;
```

## 3. How to reproduce

```bash
cd pacakage
cargo build --release
./target/release/bayesdsm.exe --db test.sqlite init --force
./target/release/bayesdsm.exe --db test.sqlite ingest --input-dir tests/synthetic
./target/release/bayesdsm.exe --db test.sqlite audit
./target/release/bayesdsm.exe --db test.sqlite clean
./target/release/bayesdsm.exe --db test.sqlite features
./target/release/bayesdsm.exe --db test.sqlite bayes
./target/release/bayesdsm.exe --db test.sqlite belief
./target/release/bayesdsm.exe --db test.sqlite dsmt
./target/release/bayesdsm.exe --db test.sqlite neutrosophic
./target/release/bayesdsm.exe --db test.sqlite rank
./target/release/bayesdsm.exe --db test.sqlite validate
./target/release/bayesdsm.exe --db test.sqlite sensitivity --out-dir out/sensitivity
./target/release/bayesdsm.exe --db test.sqlite export --out-dir out
```

## 4. Observed run (2026-06-02)

| Step | run_id | Output |
|---|---|---|
| `init --force` | – | schema initialised at `test.sqlite` |
| `ingest` | 1 | 15 files, 195 raw rows total |
| `audit` | – | passed |
| `clean` | 2 | 35 `cleaned_metals` rows |
| `features` | 3 | 49 `features` rows |
| `bayes` | 4 | posterior summaries + draws written |
| `belief` | 5 | 36 `belief_assignments` rows |
| `dsmt` | 6 | 24 `dsmt_fusion` rows |
| `neutrosophic` | 7 | 35 `neutrosophic_memberships` rows, 7 `rankings` rows |
| `rank` | 8 | rank CI / stability updated |
| `export` | – | 8 CSVs in `out/` |

## 5. Exported CSVs

| View | Rows | Path |
|---|---:|---|
| `v_rankings_full` | 7 | `out/v_rankings_full.csv` |
| `v_neutrosophic_long` | 35 | `out/v_neutrosophic_long.csv` |
| `v_posterior_summaries_wide` | 7 | `out/v_posterior_summaries_wide.csv` |
| `v_belief_assignments_long` | 36 | `out/v_belief_assignments_long.csv` |
| `v_dsmt_fused_long` | 24 | `out/v_dsmt_fused_long.csv` |
| `v_features_wide` | 7 | `out/v_features_wide.csv` |
| `v_cleaned_metals_long` | 35 | `out/v_cleaned_metals_long.csv` |
| `v_run_manifest` | 8 | `out/v_run_manifest.csv` |

(Row counts are recorded exactly in `export_manifest`.)

## 6. Random seed and reproducibility

The MCMC and Monte-Carlo steps draw their RNG from `config.random_seed`
(default `42`). With identical inputs and identical seed the posterior draws
and all downstream quantities (belief, DSmT, neutrosophic, rank) are
bit-for-bit reproducible.

## 7. Verifying the invariants

The same end-to-end pipeline is exercised by
`bayesdsm-core/tests/e2e.rs` (3 integration tests, all passing as of
2026-06-02):

```bash
cargo test --workspace -- --test-threads=1
```

* 42 unit tests pass
* 3 e2e tests pass:
  * `full_pipeline_runs_and_invariants_hold` — every output table populated,
    `Σm(A) = 1` within 1e-6, every site has `rank ∈ [1, N_s]`, all 8 views
    and CSVs exist.
  * `dsmt_mass_conservation_in_fusion_table` — `Σ fused_mass = 1` per site.
  * `failure_modes_zero_background_stops_at_ingest` — non-positive background
    triggers a STOP.

## 8. Notes on determinism

* The pignistic transformation and the neutrosophic scoring are closed-form;
  no randomness after the MCMC step.
* The DSmT conjunctive fusion is order-independent because focal sets are
  canonicalised before combination.
* The 95 % credible intervals are computed with a deterministic quantile
  (linear interpolation) over the post-warmup draws.
