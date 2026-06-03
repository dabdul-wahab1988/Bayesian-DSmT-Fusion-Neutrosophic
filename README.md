# bayesdsm

`bayesdsm` is a Rust and SQLite command-line package for reproducible
Bayesian-DSmT-neutrosophic sediment-metal hotspot prioritization.

The package ingests primary monitoring, source, exposure, confidence, and
validation CSV files into one SQLite database, runs the analysis pipeline, and
exports R-ready CSV views for reporting and manuscript figures.

## What The Pipeline Does

The implemented pipeline is:

```text
CSV inputs
  -> init/ingest/audit
  -> clean
  -> features
  -> bayes
  -> belief
  -> dsmt
  -> neutrosophic/rank
  -> validate/sensitivity/export
```

For each site, the pipeline estimates and stores:

- contamination and hotspot probabilities;
- source-support summaries over configured DSmT hypotheses;
- generalized belief assignments and fused DSmT masses;
- pignistic source-risk scores;
- neutrosophic truth, falsity, and indeterminacy memberships;
- final priority score, rank, rank band, rank uncertainty interval, and rank
  stability.

All operational state is held in the SQLite database selected with `--db`.
Generated databases, build outputs, local text drafts, and Word documents are
ignored by Git.

## Repository Layout

| Path | Purpose |
|---|---|
| `bayesdsm-cli/` | `bayesdsm` command-line interface. |
| `bayesdsm-core/` | Core ingestion, auditing, Bayesian, DSmT, neutrosophic, validation, and export logic. |
| `bayesdsm-core/src/schema/` | SQLite schema and R-ready views. |
| `docs/cli.md` | Subcommand and flag reference. |
| `docs/sql_schema.md` | Database schema notes. |
| `examples/inputs/` | Small example input CSV set. |
| `tests/synthetic/` | Synthetic fixture used by integration tests. |
| `primary_input_contract.md` | Required CSV files, columns, units, ranges, and validation rules. |
| `REPRODUCIBILITY.md` | Reproducibility recipe and expected fixture outputs. |

## Requirements

- Rust toolchain with Cargo.
- No external database server is required. SQLite is embedded through the Rust
  `rusqlite` bundled feature.

## Build

From the repository root:

```bash
cargo build --release
```

The release binary is created at:

- Windows: `target/release/bayesdsm.exe`
- Linux/macOS: `target/release/bayesdsm`

You can also run commands through Cargo:

```bash
cargo run -p bayesdsm-cli -- --help
```

## Quickstart

The commands below run the full pipeline on the committed synthetic fixture.

Windows PowerShell:

```powershell
.\target\release\bayesdsm.exe --db test.sqlite init --force
.\target\release\bayesdsm.exe --db test.sqlite ingest --input-dir tests\synthetic
.\target\release\bayesdsm.exe --db test.sqlite audit
.\target\release\bayesdsm.exe --db test.sqlite clean
.\target\release\bayesdsm.exe --db test.sqlite features
.\target\release\bayesdsm.exe --db test.sqlite bayes
.\target\release\bayesdsm.exe --db test.sqlite belief
.\target\release\bayesdsm.exe --db test.sqlite dsmt
.\target\release\bayesdsm.exe --db test.sqlite neutrosophic
.\target\release\bayesdsm.exe --db test.sqlite rank
.\target\release\bayesdsm.exe --db test.sqlite validate
.\target\release\bayesdsm.exe --db test.sqlite sensitivity --out-dir out\sensitivity
.\target\release\bayesdsm.exe --db test.sqlite export --out-dir out
```

Linux/macOS:

```bash
./target/release/bayesdsm --db test.sqlite init --force
./target/release/bayesdsm --db test.sqlite ingest --input-dir tests/synthetic
./target/release/bayesdsm --db test.sqlite audit
./target/release/bayesdsm --db test.sqlite clean
./target/release/bayesdsm --db test.sqlite features
./target/release/bayesdsm --db test.sqlite bayes
./target/release/bayesdsm --db test.sqlite belief
./target/release/bayesdsm --db test.sqlite dsmt
./target/release/bayesdsm --db test.sqlite neutrosophic
./target/release/bayesdsm --db test.sqlite rank
./target/release/bayesdsm --db test.sqlite validate
./target/release/bayesdsm --db test.sqlite sensitivity --out-dir out/sensitivity
./target/release/bayesdsm --db test.sqlite export --out-dir out
```

The main ranking output is `out/v_rankings_full.csv`.

## Input Data

Input directories must follow `primary_input_contract.md`. The pipeline expects
the contract CSVs for site metadata, sampling events, metal concentrations,
background values, confidence indicators, source indicators, exposure
indicators, stakeholder weights, DSmT hypotheses/constraints, configuration,
allowed values, data dictionary entries, leakage rules, and optional validation
labels.

`ingest` hashes every input CSV with SHA-256 and records the hash and row count
in `input_files`.

## CLI Commands

The binary exposes 13 subcommands:

| Command | Purpose |
|---|---|
| `init` | Create or migrate the SQLite schema. Use `--force` to drop and recreate objects. |
| `ingest` | Load contract CSV files from `--input-dir`. |
| `audit` | Run post-ingestion checks and write warnings/failures. |
| `clean` | Standardize raw concentrations and handle non-detects. |
| `features` | Compute CF, EF, Igeo, PLI, and normalized site features. |
| `bayes` | Run the Bayesian enrichment, source-support, and hotspot models. |
| `belief` | Convert posterior summaries into belief assignments. |
| `dsmt` | Run DSmT fusion and pignistic transformation. |
| `neutrosophic` | Build truth/falsity/indeterminacy memberships and scores. |
| `rank` | Update final ranks, uncertainty bands, and stability metrics. |
| `validate` | Evaluate rankings against independent validation labels when present. |
| `sensitivity` | Re-run downstream scoring under configured perturbations. |
| `export` | Export the eight R-ready SQLite views to CSV. |

Global and subcommand flags:

| Flag | Scope | Default |
|---|---|---|
| `--db <path>` | Global SQLite database path. | `bayesdsm.sqlite` |
| `--force` | `init` schema reset. | off |
| `--input-dir <path>` | `ingest` input directory. | required |
| `--seed <u64>` | `bayes` random seed override. | value from `config`, falling back to `42` |
| `--out-dir <path>` | `export` output directory. | `out` |
| `--out-dir <path>` | `sensitivity` output directory. | `out/sensitivity` |

See `docs/cli.md` for the detailed read/write table.

## Exported Views

`export` writes these R-ready views as CSV files:

- `v_rankings_full`
- `v_neutrosophic_long`
- `v_posterior_summaries_wide`
- `v_belief_assignments_long`
- `v_dsmt_fused_long`
- `v_features_wide`
- `v_cleaned_metals_long`
- `v_run_manifest`

The `export_manifest` table records the exported view names, paths, row counts,
and hashes.

## Validation And Tests

Run the workspace test suite with:

```bash
cargo test --workspace
```

Current verification from this repository state:

- CLI unit target: 0 tests.
- Core unit tests: 59 passed.
- End-to-end tests: 4 passed.
- Doctests: 0 tests.

The end-to-end suite runs the synthetic pipeline and checks mass conservation,
ranking/export invariants, validation warning output, and STOP behavior for an
invalid background value.

## Reproducibility

Reproducibility is built around the SQLite run manifest:

- each pipeline step writes a run record;
- input files are hashed at ingest;
- random draws use the configured seed;
- exports include manifest records for generated CSVs.

See `REPRODUCIBILITY.md` for the worked fixture recipe.

## Attribution

No AI attribution trailer is included in this repository.
