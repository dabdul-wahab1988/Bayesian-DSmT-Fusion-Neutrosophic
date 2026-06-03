# `bayesdsm`

Reproducible **Bayesian – Dezert-Smarandache (DSmT) – Neutrosophic** sediment-metal
hotspot prioritization. Pure-Rust CLI backed by a single SQLite database.

The package is the computational counterpart of the manuscript
`refined_outline.md` (scientific design) and the contract in
`primary_input_contract.md` (user-supplied inputs). The math is specified in
`plan.txt`; the Rust code is the realisation.

## What it does

```
       ┌────────┐    ┌─────────┐    ┌─────────┐    ┌──────────┐    ┌────────┐    ┌──────────┐
 CSVs  │ ingest │ →  │ features│ →  │  bayes  │ →  │ belief   │ →  │  dsmt  │ →  │neutrosoph│ → rank → export
       └────────┘    └─────────┘    └─────────┘    └──────────┘    └────────┘    └──────────┘
```

For every site in the input set the package produces:

* a posterior probability of contamination `p_hotspot`,
* a posterior source-attribution vector over the DSmT hypotheses,
* a pignistic single-value "dominant source" risk score,
* a neutrosophic triplet `(T, F, I)` per criterion,
* a final `priority_score ∈ [0, 1]` and a rank band
  (`Critical | High | Moderate | Low`) with a 95 % rank-CI and a stability index.

All intermediates are persisted in a single SQLite file — the only file the
package treats as state.

## Install

```bash
cd pacakage
cargo build --release
# Binary: target/release/bayesdsm.exe (Windows) or target/release/bayesdsm
```

## Quickstart

```bash
# 1. Initialise the schema (drops & recreates with --force).
bayesdsm --db test.sqlite init --force

# 2. Ingest a directory of CSVs that follows primary_input_contract.md.
bayesdsm --db test.sqlite ingest --input-dir tests/synthetic

# 3. Run the full pipeline.
bayesdsm --db test.sqlite audit
bayesdsm --db test.sqlite clean
bayesdsm --db test.sqlite features
bayesdsm --db test.sqlite bayes
bayesdsm --db test.sqlite belief
bayesdsm --db test.sqlite dsmt
bayesdsm --db test.sqlite neutrosophic
bayesdsm --db test.sqlite rank
bayesdsm --db test.sqlite export --out-dir out/

# 4. Validate against independent labels (no-op if absent).
bayesdsm --db test.sqlite validate

# 5. Re-run the pipeline under parameter perturbations.
bayesdsm --db test.sqlite sensitivity --out-dir out/sensitivity
```

Open `out/v_rankings_full.csv` for the final ranking.

## Input contract

See `primary_input_contract.md`. The package does not invent additional required
inputs. CSV column names, units, ranges, and STOP conditions are all in that
document and the `validate` step enforces them.

## Tests

```bash
cargo test --workspace -- --test-threads=1
```

* 42 unit tests cover individual numerical and algebraic steps (R-hat, ESS,
  neutrosophic triplet, DSmT canonicalize, …).
* 3 end-to-end tests in `bayesdsm-core/tests/e2e.rs` drive the full pipeline
  on `tests/synthetic/` and assert:
  * all 9 internal output tables are non-empty,
  * `Σm(A) = 1` for every site (within 1e-6),
  * every site has a rank in `[1, N_s]`,
  * all 8 R-ready SQL views exist and export to CSV,
  * a non-positive background value triggers a STOP at ingest/audit.

## Reproducibility

Every primary input CSV is SHA-256 hashed at ingest; the hash is stored in
`input_files.sha256`. The MCMC seed is taken from `config.random_seed`. With
identical inputs and identical seed, all posterior draws and downstream
quantities are bit-for-bit reproducible. See `REPRODUCIBILITY.md` for the full
recipe and a worked example.

## Mapping to the manuscript

See `docs/mapping.md` for the table that links every section of
`refined_outline.md` to the code module that implements it.

## Strict mode

By default, any condition listed under §17 of `plan.txt` (e.g. non-positive
background, weights not summing to 1) causes the offending subcommand to
`STOP` (exit non-zero) and write a row to `failures`.  There is no `--lenient`
flag in this build; the contract is strict by design.
