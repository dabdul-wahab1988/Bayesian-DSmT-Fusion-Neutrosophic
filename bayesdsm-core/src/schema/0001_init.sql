-- 0001_init.sql
-- SQLite is the single source of truth.
-- All raw inputs are immutable; all internal outputs are written by their owning module.

PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;

-- =========================================================================
-- AUDIT / RUN / WARNINGS / FAILURES
-- =========================================================================

CREATE TABLE IF NOT EXISTS schema_migrations (
    version     INTEGER PRIMARY KEY,
    applied_at  TEXT NOT NULL DEFAULT (datetime('now')),
    description TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS input_files (
    file_id        INTEGER PRIMARY KEY AUTOINCREMENT,
    file_role      TEXT NOT NULL UNIQUE,   -- e.g. 'site_metadata'
    path           TEXT NOT NULL,
    sha256         TEXT NOT NULL,
    row_count      INTEGER NOT NULL,
    column_count   INTEGER NOT NULL,
    imported_at    TEXT NOT NULL DEFAULT (datetime('now')),
    schema_status  TEXT NOT NULL           -- 'ok' | 'failed'
);

CREATE TABLE IF NOT EXISTS runs (
    run_id        INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id    TEXT,
    random_seed   INTEGER NOT NULL,
    started_at    TEXT NOT NULL DEFAULT (datetime('now')),
    finished_at   TEXT,
    status        TEXT NOT NULL,           -- 'running' | 'ok' | 'failed' | 'warn'
    module        TEXT,
    git_rev       TEXT
);

CREATE TABLE IF NOT EXISTS warnings (
    warning_id   INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id       INTEGER REFERENCES runs(run_id),
    module       TEXT NOT NULL,
    severity     TEXT NOT NULL,            -- 'WARN' | 'DOWNGRADE'
    code         TEXT NOT NULL,
    message      TEXT NOT NULL,
    context_json TEXT
);

CREATE TABLE IF NOT EXISTS failures (
    failure_id   INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id       INTEGER REFERENCES runs(run_id),
    module       TEXT NOT NULL,
    code         TEXT NOT NULL,
    message      TEXT NOT NULL,
    stopped_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Mirror of raw_config_parameters after coercion/validation.
CREATE TABLE IF NOT EXISTS config (
    parameter_name  TEXT PRIMARY KEY,
    parameter_value TEXT NOT NULL,
    parameter_type  TEXT NOT NULL,
    module          TEXT NOT NULL,
    description     TEXT
);

-- =========================================================================
-- RAW INPUTS  (immutable; written by `ingest` only)
-- =========================================================================

CREATE TABLE IF NOT EXISTS raw_sites (
    site_id            TEXT PRIMARY KEY,
    site_name          TEXT,
    latitude           REAL NOT NULL,
    longitude          REAL NOT NULL,
    catchment          TEXT,
    administrative_area TEXT,
    site_type          TEXT,
    land_use_context   TEXT,
    notes              TEXT
);

CREATE TABLE IF NOT EXISTS raw_sampling_events (
    sample_id    TEXT PRIMARY KEY,
    site_id      TEXT NOT NULL REFERENCES raw_sites(site_id),
    campaign_id  TEXT,
    sample_date  TEXT NOT NULL,
    matrix       TEXT NOT NULL,
    depth_min_cm REAL,
    depth_max_cm REAL,
    replicate_id TEXT,
    qa_flag      TEXT,
    sampler      TEXT,
    notes        TEXT
);

CREATE TABLE IF NOT EXISTS raw_metal_concentrations (
    concentration_id  TEXT PRIMARY KEY,
    sample_id         TEXT NOT NULL REFERENCES raw_sampling_events(sample_id),
    site_id           TEXT NOT NULL REFERENCES raw_sites(site_id),
    metal             TEXT NOT NULL,
    value             REAL NOT NULL,
    unit              TEXT NOT NULL,
    detect_flag       INTEGER NOT NULL CHECK (detect_flag IN (0,1)),
    detection_limit   REAL,
    analytical_method TEXT,
    lab_id            TEXT,
    qa_flag           TEXT,
    notes             TEXT
);

CREATE TABLE IF NOT EXISTS raw_background_values (
    background_id     TEXT PRIMARY KEY,
    metal             TEXT NOT NULL,
    background_value  REAL NOT NULL,
    unit              TEXT NOT NULL,
    background_type   TEXT NOT NULL,
    source_reference  TEXT,
    uncertainty_sd    REAL,
    valid_from        TEXT,
    valid_to          TEXT,
    notes             TEXT
);

CREATE TABLE IF NOT EXISTS raw_dsmt_hypotheses (
    hypothesis_id        TEXT PRIMARY KEY,
    hypothesis_symbol    TEXT NOT NULL UNIQUE,
    hypothesis_name      TEXT NOT NULL,
    description          TEXT NOT NULL,
    default_risk_weight  REAL NOT NULL,
    active               INTEGER NOT NULL CHECK (active IN (0,1)),
    notes                TEXT
);

CREATE TABLE IF NOT EXISTS raw_config_parameters (
    parameter_id     INTEGER PRIMARY KEY AUTOINCREMENT,
    parameter_name   TEXT NOT NULL,
    parameter_value  TEXT NOT NULL,
    parameter_type   TEXT NOT NULL,
    module           TEXT NOT NULL,
    description      TEXT,
    required         INTEGER
);

CREATE TABLE IF NOT EXISTS raw_source_indicators (
    indicator_id      TEXT PRIMARY KEY,
    site_id           TEXT NOT NULL REFERENCES raw_sites(site_id),
    hypothesis_id     TEXT NOT NULL REFERENCES raw_dsmt_hypotheses(hypothesis_id),
    indicator_name    TEXT NOT NULL,
    indicator_value   REAL NOT NULL,
    indicator_unit    TEXT,
    direction         TEXT NOT NULL,
    reliability_weight REAL NOT NULL,
    evidence_date     TEXT,
    data_source       TEXT,
    notes             TEXT
);

CREATE TABLE IF NOT EXISTS raw_exposure_indicators (
    exposure_id      TEXT PRIMARY KEY,
    site_id          TEXT NOT NULL REFERENCES raw_sites(site_id),
    receptor_type    TEXT NOT NULL,
    indicator_name   TEXT NOT NULL,
    indicator_value  REAL NOT NULL,
    indicator_unit   TEXT,
    direction        TEXT NOT NULL,
    reliability_weight REAL NOT NULL,
    evidence_date    TEXT,
    notes            TEXT
);

CREATE TABLE IF NOT EXISTS raw_confidence_indicators (
    confidence_id    TEXT PRIMARY KEY,
    site_id          TEXT NOT NULL REFERENCES raw_sites(site_id),
    indicator_name   TEXT NOT NULL,
    indicator_value  REAL NOT NULL,
    indicator_unit   TEXT,
    direction        TEXT NOT NULL,
    reliability_weight REAL NOT NULL,
    notes            TEXT
);

CREATE TABLE IF NOT EXISTS raw_leakage_rules (
    rule_id                TEXT PRIMARY KEY,
    variable_name          TEXT NOT NULL,
    rule_type              TEXT NOT NULL,
    reason                 TEXT NOT NULL,
    forbidden_for_module   TEXT NOT NULL,
    available_after_outcome INTEGER,
    notes                  TEXT
);

CREATE TABLE IF NOT EXISTS raw_stakeholder_weights (
    weight_id         TEXT PRIMARY KEY,
    criterion         TEXT NOT NULL,
    weight            REAL NOT NULL,
    weight_source     TEXT NOT NULL,
    stakeholder_group TEXT,
    notes             TEXT
);

CREATE TABLE IF NOT EXISTS raw_validation_labels (
    label_id                 TEXT PRIMARY KEY,
    site_id                  TEXT NOT NULL REFERENCES raw_sites(site_id),
    label_name               TEXT NOT NULL,
    label_value              TEXT NOT NULL,
    label_date               TEXT NOT NULL,
    label_source             TEXT NOT NULL,
    independent_of_features  INTEGER NOT NULL CHECK (independent_of_features IN (0,1)),
    notes                    TEXT
);

CREATE TABLE IF NOT EXISTS raw_dsmt_constraints (
    constraint_id   TEXT PRIMARY KEY,
    expression      TEXT NOT NULL,
    constraint_type TEXT NOT NULL,
    reason          TEXT NOT NULL,
    active          INTEGER NOT NULL CHECK (active IN (0,1)),
    notes           TEXT
);

CREATE TABLE IF NOT EXISTS raw_data_dictionary (
    variable_id     INTEGER PRIMARY KEY AUTOINCREMENT,
    variable_name   TEXT NOT NULL,
    table_name      TEXT NOT NULL,
    variable_type   TEXT NOT NULL,
    unit            TEXT,
    allowed_min     REAL,
    allowed_max     REAL,
    allowed_values  TEXT,
    description     TEXT
);

CREATE TABLE IF NOT EXISTS raw_allowed_values (
    field_name     TEXT NOT NULL,
    allowed_value  TEXT NOT NULL,
    description    TEXT,
    PRIMARY KEY (field_name, allowed_value)
);

-- =========================================================================
-- INTERNAL OUTPUTS  (written by their owning module; read by the next)
-- =========================================================================

CREATE TABLE IF NOT EXISTS cleaned_metals (
    row_id            INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id            INTEGER NOT NULL REFERENCES runs(run_id),
    site_id           TEXT NOT NULL,
    sample_id         TEXT NOT NULL,
    metal             TEXT NOT NULL,
    value_raw         REAL NOT NULL,
    unit_raw          TEXT NOT NULL,
    value_standard    REAL NOT NULL,
    unit_standard     TEXT NOT NULL,
    detect_flag       INTEGER NOT NULL,
    detection_limit   REAL,
    cleaning_method   TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS features (
    row_id                INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id                INTEGER NOT NULL REFERENCES runs(run_id),
    site_id               TEXT NOT NULL,
    feature_name          TEXT NOT NULL,
    feature_family        TEXT NOT NULL,
    feature_value         REAL,
    feature_value_normalized REAL,
    available_at          TEXT NOT NULL DEFAULT (datetime('now')),
    leakage_status        TEXT
);

CREATE TABLE IF NOT EXISTS posterior_summaries (
    row_id          INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id          INTEGER NOT NULL REFERENCES runs(run_id),
    site_id         TEXT NOT NULL,
    metal           TEXT,            -- nullable for non-metal quantities (e.g. hotspot)
    quantity        TEXT NOT NULL,    -- 'enrichment_probability' | 'source_support_probability' | 'hotspot_probability'
    posterior_mean  REAL NOT NULL,
    posterior_sd    REAL,
    ci_lower_95     REAL,
    ci_upper_95     REAL,
    rhat            REAL,
    ess_bulk        REAL,
    ess_tail        REAL,
    model_name      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS bayesian_diagnostics (
    row_id      INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id      INTEGER NOT NULL REFERENCES runs(run_id),
    quantity    TEXT NOT NULL,
    site_id     TEXT,
    metal       TEXT,
    chain       INTEGER NOT NULL,
    draws       INTEGER NOT NULL,
    rhat        REAL NOT NULL,
    ess_bulk    REAL NOT NULL,
    ess_tail    REAL NOT NULL
);

CREATE TABLE IF NOT EXISTS posterior_draws (
    row_id     INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id     INTEGER NOT NULL REFERENCES runs(run_id),
    site_id    TEXT NOT NULL,
    metal      TEXT,            -- nullable; for source/hotspot
    quantity   TEXT NOT NULL,   -- 'enrichment' | 'source_pi_h' | 'hotspot' | 'psi'
    draw_index INTEGER NOT NULL,
    value      REAL NOT NULL
);

CREATE TABLE IF NOT EXISTS belief_assignments (
    row_id          INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id          INTEGER NOT NULL REFERENCES runs(run_id),
    site_id         TEXT NOT NULL,
    evidence_layer  TEXT NOT NULL,   -- 'hotspot' | 'source' | 'chemical' | 'exposure' | 'confidence'
    hypothesis_expr TEXT NOT NULL,   -- DSmT expression
    belief_mass     REAL NOT NULL,
    uncertainty_score REAL NOT NULL,
    belief_rule     TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS dsmt_fusion (
    row_id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id                  INTEGER NOT NULL REFERENCES runs(run_id),
    site_id                 TEXT NOT NULL,
    hypothesis_expr         TEXT NOT NULL,
    fused_mass              REAL NOT NULL,
    conflict_mass           REAL NOT NULL,
    pignistic_probability   REAL,
    dominant_source_flag    INTEGER NOT NULL DEFAULT 0,
    dsmt_model              TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS neutrosophic_memberships (
    row_id          INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id          INTEGER NOT NULL REFERENCES runs(run_id),
    site_id         TEXT NOT NULL,
    criterion       TEXT NOT NULL,
    truth           REAL NOT NULL,
    falsity         REAL NOT NULL,
    indeterminacy   REAL NOT NULL,
    criterion_score REAL NOT NULL
);

CREATE TABLE IF NOT EXISTS rankings (
    row_id              INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id              INTEGER NOT NULL REFERENCES runs(run_id),
    site_id             TEXT NOT NULL,
    priority_score      REAL NOT NULL,
    rank                INTEGER NOT NULL,
    rank_band           TEXT NOT NULL,
    rank_ci_lower       REAL,
    rank_ci_upper       REAL,
    rank_stability      REAL,
    dominant_source     TEXT,
    conflict_level      REAL,
    indeterminacy_level REAL,
    recommended_action  TEXT
);

CREATE TABLE IF NOT EXISTS sensitivity_results (
    row_id          INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id          INTEGER NOT NULL REFERENCES runs(run_id),
    parameter_name  TEXT NOT NULL,
    perturbation    TEXT NOT NULL,
    site_id         TEXT,
    quantity        TEXT NOT NULL,
    value           REAL NOT NULL
);

CREATE TABLE IF NOT EXISTS export_manifest (
    export_id   INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id      INTEGER NOT NULL REFERENCES runs(run_id),
    view_name   TEXT NOT NULL,
    exported_at TEXT NOT NULL DEFAULT (datetime('now')),
    row_count   INTEGER NOT NULL,
    path        TEXT NOT NULL
);
