# primary_input_contract.md

# Primary Input Contract
## Rust Package for Bayesian–DSmT–Neutrosophic Sediment-Metal Hotspot Prioritization

## 1. Purpose
This document defines the primary inputs supplied to the Rust package before the workflow runs.

A primary input is a user-supplied or externally supplied file. It is not produced by an earlier Rust module.

The Rust package should ingest these files, validate them, compute SHA-256 hashes, store immutable raw records in SQLite, and then generate internal outputs for later stages.

## 2. Critical design rule
Do not treat internal outputs as primary inputs.

Normally generated internally:

```text
cleaned_metals
features
posterior_summaries
belief_assignments
dsmt_fusion
neutrosophic_memberships
rankings
sensitivity_results
export_manifest
```

Exception: `posterior_summaries.csv` may be allowed as an optional external input only if Bayesian modelling was performed outside the Rust package. This must be explicitly configured with `bayes_model_mode = external_import`.

---

# 3. Required primary inputs

## 3.1 site_metadata.csv
Purpose: defines sampling sites and spatial context.

Required fields:

```text
site_id                 string, required, unique
site_name               string, optional
latitude                float, required
longitude               float, required
catchment               string, optional
administrative_area     string, optional
site_type               string, optional
land_use_context        string, optional
notes                   string, optional
```

Validation:

```text
site_id must be unique
latitude must be between -90 and 90
longitude must be between -180 and 180
site_id must not be blank
all other input tables referencing site_id must match this table
```

SQLite raw table:

```text
raw_sites
```

## 3.2 sampling_events.csv
Purpose: defines sampling events, sample IDs, dates, campaigns, depth, and QA status.

Required fields:

```text
sample_id       string, required, unique
site_id         string, required
campaign_id     string, optional
sample_date     date, required, YYYY-MM-DD
matrix          string, required, normally sediment
depth_min_cm    float, optional
depth_max_cm    float, optional
replicate_id    string, optional
qa_flag         string, optional, pass/warning/fail
sampler         string, optional
notes           string, optional
```

Validation:

```text
sample_id must be unique
site_id must exist in site_metadata.csv
sample_date must be valid ISO date
if depth_min_cm and depth_max_cm exist, depth_min_cm <= depth_max_cm
```

SQLite raw table:

```text
raw_sampling_events
```

## 3.3 metal_concentrations.csv
Purpose: raw measured sediment-metal concentrations.

Required fields:

```text
concentration_id    string, required, unique
sample_id           string, required
site_id             string, required
metal               string, required
value               float, required
unit                string, required
detect_flag         integer, required, 1 detected, 0 non-detect
detection_limit     float, optional
analytical_method   string, optional
lab_id              string, optional
qa_flag             string, optional
notes               string, optional
```

Validation:

```text
concentration_id must be unique
sample_id must exist in sampling_events.csv
site_id must exist in site_metadata.csv
metal must not be blank
unit must be recognized
if detect_flag = 1, value > 0
if detect_flag = 0, detection_limit > 0
negative values are not allowed
zero values are not allowed unless treated as non-detects
```

SQLite raw table:

```text
raw_metal_concentrations
```

## 3.4 background_values.csv
Purpose: defines background/reference concentrations used for enrichment and contamination calculations.

Required fields:

```text
background_id       string, required, unique
metal               string, required
background_value    float, required
unit                string, required
background_type     string, required, local/regional/crustal/regulatory/literature
source_reference    string, optional
uncertainty_sd      float, optional
valid_from          date, optional
valid_to            date, optional
notes               string, optional
```

Validation:

```text
background_id must be unique
metal must not be blank
background_value > 0
unit must be recognized
if uncertainty_sd exists, uncertainty_sd >= 0
```

SQLite raw table:

```text
raw_background_values
```

## 3.5 dsmt_hypotheses.csv
Purpose: defines the source hypotheses used in the DSmT frame.

Required fields:

```text
hypothesis_id           string, required, unique, e.g. H1
hypothesis_symbol       string, required, unique, e.g. theta_1
hypothesis_name         string, required, e.g. mining
description             string, required
default_risk_weight     float, required
active                  integer, required, 1 active, 0 inactive
notes                   string, optional
```

Validation:

```text
default_risk_weight must be in [0,1]
at least two active hypotheses are required
one hypothesis may represent unresolved/mixed source
```

SQLite raw table:

```text
raw_dsmt_hypotheses
```

## 3.6 config_parameters.csv
Purpose: defines runtime settings, thresholds, modes, and reproducibility controls.

Required fields:

```text
parameter_name      string, required
parameter_value     string, required
parameter_type      string, required, int/float/bool/string/enum
module              string, required
description         string, optional
required            integer, optional
```

Required parameters:

```text
project_id
random_seed
standard_concentration_unit
nondetect_method
reference_element
enrichment_threshold
bayes_model_mode
source_model_mode
hotspot_model_mode
dsmt_model
belief_uncertainty_width_max
single_source_margin
union_support_threshold
neutrosophic_indeterminacy_penalty
ranking_band_critical
ranking_band_high
ranking_band_moderate
sqlite_path
```

Validation:

```text
required parameters must exist
numeric values must parse correctly
random_seed must be integer
dsmt_model must be free, hybrid, or shafer
nondetect_method must be dl_sqrt2, dl_half, or censored_bayes
```

SQLite raw table:

```text
raw_config_parameters
```

---

# 4. Strongly recommended primary inputs

## 4.1 source_indicators.csv
Purpose: provides evidence supporting source hypotheses.

Required fields:

```text
indicator_id          string, required, unique
site_id               string, required
hypothesis_id         string, required
indicator_name        string, required
indicator_value       float, required
indicator_unit        string, optional
direction             string, required, higher_risk/lower_risk
reliability_weight    float, required
evidence_date         date, optional
data_source           string, optional
notes                 string, optional
```

Validation:

```text
site_id must exist
hypothesis_id must exist
reliability_weight must be in [0,1]
direction must be higher_risk or lower_risk
```

SQLite raw table:

```text
raw_source_indicators
```

## 4.2 exposure_indicators.csv
Purpose: defines receptors and exposure relevance.

Required fields:

```text
exposure_id           string, required, unique
site_id               string, required
receptor_type         string, required, water_intake/settlement/fishery/wetland/irrigation
indicator_name        string, required
indicator_value       float, required
indicator_unit        string, optional
direction             string, required, higher_risk/lower_risk
reliability_weight    float, required
evidence_date         date, optional
notes                 string, optional
```

SQLite raw table:

```text
raw_exposure_indicators
```

## 4.3 confidence_indicators.csv
Purpose: defines evidence quality, uncertainty, missingness, and sampling confidence.

Required fields:

```text
confidence_id         string, required, unique
site_id               string, required
indicator_name        string, required
indicator_value       float, required
indicator_unit        string, optional
direction             string, required, higher_confidence/lower_confidence
reliability_weight    float, required
notes                 string, optional
```

SQLite raw table:

```text
raw_confidence_indicators
```

## 4.4 leakage_rules.csv
Purpose: prevents circular reasoning and invalid predictors.

Required fields:

```text
rule_id                 string, required, unique
variable_name           string, required
rule_type               string, required, exclude/warn/time_restrict/group_restrict
reason                  string, required
forbidden_for_module    string, required, bayes/validation/ranking/all
available_after_outcome integer, optional
notes                   string, optional
```

Validation:

```text
excluded variables must not enter prohibited modules
```

SQLite raw table:

```text
raw_leakage_rules
```

## 4.5 stakeholder_weights.csv
Purpose: defines criteria weights for neutrosophic ranking.

Required fields:

```text
weight_id           string, required, unique
criterion           string, required
weight              float, required
weight_source       string, required, stakeholder/entropy/equal/expert
stakeholder_group   string, optional
notes               string, optional
```

Validation:

```text
weight >= 0
weights used in one run must sum to 1 within 1e-8
```

SQLite raw table:

```text
raw_stakeholder_weights
```

---

# 5. Optional primary inputs

## 5.1 validation_labels.csv
Purpose: provides independent labels for validation.

Required fields:

```text
label_id                    string, required, unique
site_id                     string, required
label_name                  string, required, e.g. hotspot/source_label/priority_class
label_value                 string, required
label_date                  date, required
label_source                string, required, field/regulatory/expert/historical
independent_of_features     integer, required, 1 yes, 0 no
notes                       string, optional
```

Validation:

```text
site_id must exist
labels used for validation must be independent
if independent_of_features = 0, label cannot be used for confirmatory validation
label_date must not create temporal leakage
```

SQLite raw table:

```text
raw_validation_labels
```

## 5.2 dsmt_constraints.csv
Purpose: defines hybrid DSmT constraints.

Required fields:

```text
constraint_id       string, required, unique
expression          string, required, e.g. theta_1 & theta_3
constraint_type     string, required, empty/allowed/forced_union
reason              string, required
active              integer, required
notes               string, optional
```

Validation:

```text
all expressions must use valid hypothesis symbols
if dsmt_model = hybrid, constraints must be parsed before fusion
empty constraints force intersections to be treated as empty
```

SQLite raw table:

```text
raw_dsmt_constraints
```

## 5.3 data_dictionary.csv
Purpose: defines variable metadata.

Required fields:

```text
variable_name       string, required
table_name          string, required
variable_type       string, required, numeric/categorical/date/text
unit                string, optional
allowed_min         float, optional
allowed_max         float, optional
allowed_values      string, optional
description         string, optional
```

SQLite raw table:

```text
raw_data_dictionary
```

## 5.4 allowed_values.csv
Purpose: defines controlled vocabulary values.

Required fields:

```text
field_name          string, required
allowed_value       string, required
description         string, optional
```

SQLite raw table:

```text
raw_allowed_values
```

---

# 6. Optional external posterior input

## posterior_summaries.csv
Use only if Bayesian modelling is performed outside Rust.

Required fields:

```text
posterior_id        string, required, unique
external_run_id     string, required
site_id             string, required
metal               string, optional
quantity            string, required
posterior_mean      float, required
posterior_sd        float, optional
ci_lower_95         float, optional
ci_upper_95         float, optional
rhat                float, optional
ess_bulk            float, optional
ess_tail            float, optional
model_name          string, required
notes               string, optional
```

Validation:

```text
only allowed if bayes_model_mode = external_import
probabilities must be in [0,1]
credible interval lower <= mean <= upper where applicable
Rhat > 1.01 gives warning
Rhat > 1.05 downgrades claims
```

SQLite destination table:

```text
posterior_summaries
```

---

# 7. Internal outputs generated by Rust

| Internal output | Generated by module | SQLite table |
|---|---|---|
| Cleaned concentration records | clean | cleaned_metals |
| Feature matrix | features | features |
| Bayesian posterior summaries | bayes | posterior_summaries |
| Bayesian diagnostics | bayes | bayesian_diagnostics |
| Belief assignments | belief | belief_assignments |
| DSmT fused masses | dsmt | dsmt_fusion |
| Neutrosophic memberships | neutrosophic | neutrosophic_memberships |
| Final rankings | rank | rankings |
| Sensitivity outputs | sensitivity | sensitivity_results |
| Export records | export | export_manifest |

---

# 8. Recommended input folder layout

```text
project/
  data/
    primary_inputs/
      site_metadata.csv
      sampling_events.csv
      metal_concentrations.csv
      background_values.csv
      dsmt_hypotheses.csv
      config_parameters.csv
      source_indicators.csv
      exposure_indicators.csv
      confidence_indicators.csv
      leakage_rules.csv
      stakeholder_weights.csv
      validation_labels.csv
      dsmt_constraints.csv
      data_dictionary.csv
      allowed_values.csv
  db/
    project.sqlite
  outputs/
    tables/
    figures/
    supplements/
  logs/
```

---

# 9. Hashing requirement
For each primary input file:

```text
H_r = SHA256(F_r)
```

Store:

```text
input_files.file_id
input_files.file_role
input_files.path
input_files.sha256
input_files.row_count
input_files.imported_at
```

If a file changes, its hash changes and a new ingestion run must be created.

---

# 10. Minimal synthetic test dataset
A synthetic test dataset should contain at least:

```text
5 sites
1 sampling event per site
5 metals
1 background value per metal
3 source hypotheses
1 exposure indicator per site
1 confidence indicator per site
equal weights
free DSm model
```

This is sufficient to test the full computational flow, but not sufficient for scientific claims.

---

# 11. Claim rules based on inputs

If no validation labels exist, allowed claims are:

```text
candidate hotspot ranking
decision-support ranking
field-verification priority
uncertainty-aware screening
```

Not allowed:

```text
validated prediction
confirmed hotspot detection
superiority over existing methods
regulatory decision
```

If validation labels are expert-only, allowed claims are:

```text
expert-label agreement
exploratory validation
```

If independent field validation exists, the package may support:

```text
predictive performance
calibration
baseline comparison
validated prioritization, if performance is acceptable
```

---

# 12. Package-level STOP rules for inputs
The package must stop if:

```text
required file is missing
required column is missing
site_id references are invalid
concentration values are invalid
background values are non-positive
units are not recognized
DSmT hypotheses are fewer than two
weights do not sum to one
config is missing required parameter
external posterior import is used without explicit configuration
leakage rule excludes a variable but it is still used in a prohibited module
```
