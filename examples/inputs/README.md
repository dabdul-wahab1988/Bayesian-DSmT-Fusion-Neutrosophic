# `examples/inputs/`

This directory contains a fully-populated **worked example** that follows
`primary_input_contract.md` exactly. It is the same shape and schema as
`tests/synthetic/` (used by the integration tests) but is presented as a
realistic demonstration rather than a test fixture.

The 7 sites form a north-to-south transect down a hypothetical gold-mining-
impacted catchment in the Pra Basin (Ghana, West Africa):

| ID | Site | Site type |
|---|---|---|
| S1 | Upstream Reference | background / forest reserve |
| S2 | Mid-catchment Mining | mine-adjacent |
| S3 | Tailings Discharge | mine-adjacent |
| S4 | Confluence Zone | mixed / agricultural |
| S5 | Downstream Settlement | mixed / peri-urban |
| S6 | Edge: Non-Detect Heavy | mixed / peri-urban |
| S7 | Edge: Conflicting Indicators | mixed / industrial |

The metal panel is `As, Cd, Cr, Ni, Pb`. The DSmT frame has three
hypotheses: `θ_1 = Mining`, `θ_2 = Agriculture`, `θ_3 = Industrial`.

## How to use

```bash
# From pacakage/
cargo build --release
./target/release/bayesdsm --db example.sqlite init --force
./target/release/bayesdsm --db example.sqlite ingest --input-dir examples/inputs
./target/release/bayesdsm --db example.sqlite audit
./target/release/bayesdsm --db example.sqlite clean
./target/release/bayesdsm --db example.sqlite features
./target/release/bayesdsm --db example.sqlite bayes
./target/release/bayesdsm --db example.sqlite belief
./target/release/bayesdsm --db example.sqlite dsmt
./target/release/bayesdsm --db example.sqlite neutrosophic
./target/release/bayesdsm --db example.sqlite rank
./target/release/bayesdsm --db example.sqlite export --out-dir example-out
```

The final `example-out/v_rankings_full.csv` will contain 7 sites ranked
by `priority_score` with band assignments and a 95 % rank-CI.
