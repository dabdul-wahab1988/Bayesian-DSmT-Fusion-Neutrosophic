# refined_outline.md

# Refined Scientific Outline
## Bayesian–DSmT Evidence Fusion with Neutrosophic Prioritization for Sediment-Metal Hotspots

## Working title
**An Auditable Bayesian–DSmT Framework for Uncertainty-Aware Sediment-Metal Hotspot Prioritization**

Alternative conservative titles:
1. Conflict-Preserving Evidence Fusion for Sediment-Metal Hotspot Ranking under Uncertainty
2. Bayesian Calibration and DSmT Evidence Fusion for Decision-Oriented Sediment Contamination Screening
3. A Reproducible Decision Pipeline for Ranking Sediment-Metal Hotspots with Uncertainty and Conflict Diagnostics
4. From Fragmented Sediment-Metal Evidence to Field-Verification Priorities: A Bayesian–DSmT–Neutrosophic Framework

## Safest high-impact framing
This chapter presents a reproducible decision-support framework for ranking candidate sediment-metal hotspots under uncertainty, source overlap, and evidential conflict. The contribution is not a new contamination index alone. The contribution is an auditable pipeline that structures heterogeneous evidence, quantifies uncertainty using Bayesian modelling, converts posterior outputs into generalized belief assignments, fuses overlapping/conflicting evidence using DSmT, and ranks sites using neutrosophic truth, falsity, and indeterminacy.

The framework supports prioritization and field-verification planning. It does not, by itself, prove causality, definitive source apportionment, regulatory compliance, or remediation readiness without independent validation.

## Central defensible claim
The chapter presents an auditable Bayesian–DSmT–neutrosophic workflow that converts heterogeneous chemical, source, exposure, and confidence evidence into uncertainty-aware rank bands and field-verification priorities for sediment-metal hotspot screening.

## Novelty statement
The novelty is the integration of three complementary layers:

1. **Bayesian calibration** — quantifies uncertainty in enrichment, source support, and hotspot intensity.
2. **DSmT evidence fusion** — preserves non-exclusive source hypotheses, source overlap, and evidential conflict.
3. **Neutrosophic prioritization** — converts fused evidence into truth, falsity, and indeterminacy memberships for decision ranking.

SQLite is treated as the single source of truth for reproducible implementation.

## Allowed claims
The manuscript may claim that the framework:
- supports transparent hotspot prioritization;
- preserves uncertainty and evidence conflict;
- allows non-exclusive source hypotheses;
- combines chemical, source, exposure, and confidence evidence;
- produces rank bands rather than rigid deterministic ranks;
- identifies sites where field verification may reduce uncertainty;
- can be implemented as a reproducible, auditable Rust + SQLite workflow.

## Prohibited claims
The manuscript must not claim:
- definitive source attribution;
- proven superiority over existing approaches without benchmark validation;
- causal identification of contamination sources;
- regulatory readiness;
- confirmed hotspot detection without external field validation;
- automatic remediation decision-making;
- uncertainty reduction unless quantitatively demonstrated;
- prediction performance if no independent validation labels exist.

## Structured abstract

### Background
Sediment-metal contamination assessment often relies on concentration exceedances, enrichment indices, receptor-model outputs, and expert judgement. These evidence sources are useful but frequently heterogeneous, uncertain, incomplete, and sometimes conflicting.

### Objective
To present a reproducible Bayesian–DSmT–neutrosophic framework for ranking candidate sediment-metal hotspots under uncertainty, source overlap, and evidential conflict.

### Methods
The framework structures chemical, source, exposure, and confidence evidence into an auditable evidence matrix. Bayesian models estimate enrichment probability, source-support probability, and hotspot-intensity probability. Posterior outputs are converted into generalized belief assignments and fused using DSmT under free or hybrid source models. Fused evidence is converted into neutrosophic truth, falsity, and indeterminacy memberships for multi-criteria ranking.

### Outputs
The workflow produces hotspot priority scores, rank bands, dominant fused source evidence, conflict diagnostics, indeterminacy levels, uncertainty bands, sensitivity results, and field-verification priorities.

### Conclusion
The framework supports transparent, uncertainty-aware sediment-metal hotspot prioritization. Without independent validation, outputs should be interpreted as structured prioritization hypotheses rather than confirmed hotspot classifications.

## Keywords
source contribution mapping; hotspot ranking; posterior predictive checks; stakeholder weighting; uncertainty bands; reproducible workflows.

---

# Manuscript outline

## 1. Introduction

### 1.1 Environmental problem
Sediment-metal contamination is a persistent environmental and public-health concern. Sediments act both as archives of historical contamination and as secondary sources that may release metals during resuspension, flooding, dredging, hydrodynamic remobilization, or land-use disturbance.

### 1.2 Methodological problem
Conventional assessment often relies on concentration exceedances, enrichment factors, contamination factors, pollution load indices, geoaccumulation indices, receptor-model outputs, or expert judgement. These tools are useful but often fragmented and may not propagate uncertainty or preserve conflict among evidence sources.

### 1.3 Problem statement
Sediment-metal hotspots are difficult to prioritize because available evidence is heterogeneous, uncertain, incomplete, and sometimes conflicting. A site may simultaneously reflect mining discharge, legacy tailings, geogenic background, remobilization, and mixed urban/industrial inputs.

### 1.4 Aim
To present a reproducible Bayesian–DSmT–neutrosophic framework for ranking candidate sediment-metal hotspots under uncertainty, overlapping source evidence, and decision conflict.

### 1.5 Objectives
1. Structure heterogeneous sediment-metal evidence into a decision-ready evidence matrix.
2. Use Bayesian modelling to quantify uncertainty in enrichment, source support, and hotspot intensity.
3. Convert posterior outputs into generalized belief assignments.
4. Fuse evidence using DSmT to preserve source overlap and conflict.
5. Rank sites using neutrosophic multi-criteria prioritization.
6. Identify priority sites for management attention and field verification.

## 2. Evidence problem and conceptual basis
A hotspot is not merely a site with high concentration. A management-relevant hotspot combines contamination intensity, source plausibility, ecological/exposure relevance, confidence, feasibility of action, and field-verification value.

Evidence layers:
1. Chemical evidence: concentrations, EF, CF, PLI, Igeo.
2. Source evidence: diagnostic ratios, receptor-model outputs, land-use/source proximity, geogenic plausibility.
3. Exposure evidence: water intakes, fisheries, irrigation points, settlements, wetlands, ecological zones.
4. Confidence evidence: sampling density, analytical uncertainty, missingness, posterior uncertainty, QA/QC status.

Bayesian modelling is needed because it provides posterior distributions rather than single values. DSmT is needed because source hypotheses are not always mutually exclusive. Neutrosophic prioritization is needed because final decisions require truth, falsity, and indeterminacy.

## 3. Bayesian–DSmT–neutrosophic framework
Overall sequence:
1. Primary input data
2. Rust ingestion and SQLite audit
3. Cleaning and feature engineering
4. Bayesian modelling in Rust
5. Posterior-to-belief transformation
6. DSmT fusion
7. Neutrosophic decision scoring
8. Sensitivity analysis
9. Final priority ranking
10. R-ready export

Bayesian outputs:
1. enrichment probability;
2. source-support probability;
3. hotspot-intensity probability.

Posterior-to-belief rules:
- strong support for one source assigns belief to that source;
- shared support assigns belief to a union;
- overlapping process evidence assigns belief to an intersection;
- wide credible intervals or missing evidence assign belief to ignorance;
- contradictory evidence contributes to conflict.

Example DSmT frame:
- theta_1: mining or ore-processing input;
- theta_2: legacy tailings or historical contamination;
- theta_3: geogenic background;
- theta_4: hydrodynamic remobilization;
- theta_5: urban or industrial mixed input;
- theta_6: unresolved or mixed source.

Use a free DSm model when all hypotheses may overlap. Use a hybrid DSm model when physical, geochemical, or regulatory constraints make some combinations impossible.

## 4. Implementation workflow
Required primary input tables:
1. site_metadata.csv
2. sampling_events.csv
3. metal_concentrations.csv
4. background_values.csv
5. dsmt_hypotheses.csv
6. config_parameters.csv

Recommended primary inputs:
1. source_indicators.csv
2. exposure_indicators.csv
3. confidence_indicators.csv
4. leakage_rules.csv
5. stakeholder_weights.csv

Optional primary inputs:
1. validation_labels.csv
2. dsmt_constraints.csv
3. data_dictionary.csv
4. allowed_values.csv

Processing steps:
1. schema validation and hashing;
2. unit harmonization;
3. non-detect handling;
4. missingness documentation;
5. duplicate detection;
6. feature calculation;
7. leakage audit;
8. Bayesian modelling;
9. belief assignment;
10. DSmT fusion;
11. neutrosophic ranking;
12. sensitivity analysis;
13. export.

## 5. Decision outputs
Each site should include:
- priority score;
- rank;
- rank band;
- rank uncertainty interval;
- dominant fused source evidence;
- conflict level;
- indeterminacy level;
- recommended action.

Figures and tables:
Yes. For a **Nature-style book chapter**, use **four strong multi-panel figures**, not many small figures. Each figure should carry a major scientific message and use clean labels, panel letters, limited colours, and strong visual hierarchy.

## Proposed four figures

### **Figure 1. End-to-end Bayesian–DSmT–neutrosophic decision pipeline**

**Purpose:** Show the full conceptual architecture of the chapter.

**Nature-style layout:**

```text
a. Primary evidence inputs
b. Rust + SQLite audit layer
c. Bayesian uncertainty calibration
d. Posterior-to-belief transformation
e. DSmT evidence fusion
f. Neutrosophic prioritization
g. Final rank bands and field-verification priorities
```

**Main visual idea:**
A left-to-right workflow diagram showing how raw sediment evidence becomes auditable hotspot priorities.

**Key message:**
The framework is not just a ranking method; it is a reproducible decision pipeline.

**Caption idea:**
*Figure 1. Reproducible Bayesian–DSmT–neutrosophic workflow for sediment-metal hotspot prioritization. Primary monitoring, source, exposure, and confidence evidence are ingested into an audited SQLite database, transformed through Bayesian uncertainty calibration, converted into generalized belief assignments, fused using DSmT, and ranked using neutrosophic decision criteria.*

This figure directly matches the chapter’s proposed pipeline of Bayesian calibration, posterior-to-belief transformation, DSmT fusion, neutrosophic scoring, sensitivity analysis, and final ranking.

---

### **Figure 2. Evidence matrix and Bayesian uncertainty calibration**

**Purpose:** Show how fragmented evidence becomes probabilistic evidence.

**Nature-style layout:**

```text
a. Site × evidence-layer matrix
b. Metal concentration distributions against background values
c. Posterior enrichment probability per metal/site
d. Posterior uncertainty bands
```

**Main visual idea:**
A heatmap plus posterior interval plots.

**Panel design:**

* **Panel a:** rows = sites; columns = evidence layers: chemistry, source, exposure, confidence.
* **Panel b:** concentration distributions compared with background/reference thresholds.
* **Panel c:** posterior probability of enrichment, e.g., (P(EF > 1.5)).
* **Panel d:** uncertainty bands showing wide vs narrow posterior intervals.

**Key message:**
Bayesian modelling converts raw and uneven evidence into calibrated probabilities with uncertainty.

**Caption idea:**
*Figure 2. Bayesian calibration of sediment-metal evidence. Heterogeneous evidence layers are organized into a site-level evidence matrix. Metal concentrations are modelled relative to background values to estimate posterior enrichment probabilities and credible intervals, allowing uncertainty to propagate into later evidence-fusion and ranking stages.*

This supports the outline’s idea that Bayesian modelling should produce enrichment probability, source-support probability, and hotspot-intensity probability rather than single deterministic scores.

---

### **Figure 3. DSmT source-fusion structure and conflict preservation**

**Purpose:** Explain why DSmT is needed.

**Nature-style layout:**

```text
a. Conventional exclusive-source model
b. DSmT overlapping-source model
c. Belief assignment to singletons, unions, and intersections
d. Conflict and ignorance outputs by site
```

**Main visual idea:**
Compare a rigid classification model with an overlapping DSmT source frame.

**Panel design:**

* **Panel a:** conventional mutually exclusive source classes.
* **Panel b:** overlapping source hypotheses: mining, tailings, geogenic, remobilization, urban/industrial, unresolved.
* **Panel c:** example belief masses:

  * (m(\theta_1))
  * (m(\theta_1 \cup \theta_2))
  * (m(\theta_1 \cap \theta_4))
  * (m(I_t))
* **Panel d:** site-level conflict/ignorance heatmap.

**Key message:**
Sediment-metal sources are not always mutually exclusive; DSmT preserves overlap and conflict instead of forcing premature classification.

**Caption idea:**
*Figure 3. DSmT representation of overlapping sediment-metal source evidence. Unlike exclusive classification schemes, the DSmT layer allows belief to be assigned to individual hypotheses, unions of hypotheses, intersections of hypotheses, and total ignorance. Conflict is retained as a diagnostic output rather than discarded.*

This figure is important because the DSmT source document explains that DSmT can work with free or hybrid models where hypotheses may overlap, unlike strictly exclusive Shafer-style models.

---

### **Figure 4. Final hotspot prioritization, rank stability, and field-verification value**

**Purpose:** Show the final decision output.

**Nature-style layout:**

```text
a. Hotspot priority map or spatial schematic
b. Ranked site plot with uncertainty bands
c. Truth–falsity–indeterminacy ternary or stacked bar plot
d. Field-verification priority matrix
```

**Main visual idea:**
A decision dashboard showing which sites are urgent, uncertain, conflicting, or worth revisiting.

**Panel design:**

* **Panel a:** map or site network coloured by rank band: Critical, High, Moderate, Low.
* **Panel b:** ranked priority scores with 95% uncertainty/rank-stability intervals.
* **Panel c:** neutrosophic membership bars:

  * Truth = support for high priority.
  * Falsity = support against high priority.
  * Indeterminacy = uncertainty/conflict/missingness.
* **Panel d:** matrix of:

  * high priority / high confidence;
  * high priority / high uncertainty;
  * moderate priority / high conflict;
  * low priority / low verification value.

**Key message:**
The framework produces not only a ranked list, but also uncertainty-aware rank bands and field-verification priorities.

**Caption idea:**
*Figure 4. Decision outputs from the Bayesian–DSmT–neutrosophic framework. Sites are ranked into management-priority bands while preserving posterior uncertainty, DSmT conflict, and neutrosophic indeterminacy. Field-verification priorities identify sites where new sampling would most improve decision confidence.*


- Table 1: Evidence layers and decision role
- Table 2: Belief-assignment rules
- Table 3: Final site ranking with uncertainty and verification priority

## 6. Discussion
Emphasize that the framework integrates uncertainty calibration, conflict-preserving fusion, and decision prioritization. Discuss relevance for mining-impacted catchments, industrial sediment systems, water-intake protection, remediation planning, and limited-budget field campaigns.

Limitations:
1. Belief-assignment rules require transparent justification.
2. Rankings depend on criteria and weights.
3. DSmT complexity increases as hypotheses grow.
4. Poor monitoring data can produce wide uncertainty bands.
5. The framework supports prioritization but does not replace field confirmation.
6. Without independent validation, outputs remain exploratory or decision-supportive.

## 7. Conclusion
Sediment-metal hotspot prioritization is difficult because evidence is fragmented, uncertain, and often non-exclusive. The proposed Bayesian–DSmT–neutrosophic framework calibrates uncertainty, preserves source overlap/conflict, and converts fused evidence into management-ready rank bands. Confirmed hotspot classification requires independent validation.

## Reviewer-proof limitation statement
This framework is intended for structured prioritization and field-verification planning. Rankings depend on data quality, background-value selection, source-frame design, posterior-to-belief rules, DSmT model assumptions, criterion weights, and validation availability. Without independent validation, outputs should be interpreted as uncertainty-aware prioritization hypotheses rather than confirmed hotspot classifications.
