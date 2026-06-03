//! Per-CSV validators.  Each function checks the rules in
//! `primary_input_contract.md` and returns typed errors.  These are
//! invoked from `ingest::sqlite_insert` before inserting into `raw_*`.

use rusqlite::Connection;

use crate::error::{BayesDsmError, Result};

pub fn validate_sites(headers: &[String], rows: &[Vec<String>]) -> Result<()> {
    let id = col(headers, "site_id")?;
    let lat = col(headers, "latitude")?;
    let lon = col(headers, "longitude")?;
    let mut seen = std::collections::HashSet::new();
    for r in rows {
        let sid = r[id].trim();
        if sid.is_empty() {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1201".into(),
                message: "site_id must not be blank".into(),
            });
        }
        if !seen.insert(sid.to_string()) {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1202".into(),
                message: format!("duplicate site_id: {sid}"),
            });
        }
        let lat_v: f64 = r[lat].parse().map_err(|_| BayesDsmError::Stop {
            module: "ingest".into(),
            code: "E1203".into(),
            message: format!("latitude for {sid} is not a float"),
        })?;
        let lon_v: f64 = r[lon].parse().map_err(|_| BayesDsmError::Stop {
            module: "ingest".into(),
            code: "E1203".into(),
            message: format!("longitude for {sid} is not a float"),
        })?;
        if !(-90.0..=90.0).contains(&lat_v) || !(-180.0..=180.0).contains(&lon_v) {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1203".into(),
                message: format!("coordinates out of range for {sid}"),
            });
        }
    }
    Ok(())
}

pub fn validate_sampling_events(
    headers: &[String],
    rows: &[Vec<String>],
    conn: &Connection,
) -> Result<()> {
    let sid = col(headers, "sample_id")?;
    let site = col(headers, "site_id")?;
    let date = col(headers, "sample_date")?;
    let dmin = header_idx(headers, "depth_min_cm");
    let dmax = header_idx(headers, "depth_max_cm");

    let mut seen = std::collections::HashSet::new();
    for r in rows {
        let s = r[sid].trim();
        if s.is_empty() || !seen.insert(s.to_string()) {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1204".into(),
                message: format!("sample_id issue: {s}"),
            });
        }
        let site_id = r[site].trim();
        if !site_exists(conn, site_id)? {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1205".into(),
                message: format!("sample {s} references unknown site_id {site_id}"),
            });
        }
        if chrono::NaiveDate::parse_from_str(r[date].trim(), "%Y-%m-%d").is_err() {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1206".into(),
                message: format!("sample_date for {s} is not YYYY-MM-DD"),
            });
        }
        if let (Some(i), Some(j)) = (dmin, dmax) {
            if !r[i].is_empty() && !r[j].is_empty() {
                let a: f64 = r[i].parse().unwrap_or(0.0);
                let b: f64 = r[j].parse().unwrap_or(0.0);
                if a > b {
                    return Err(BayesDsmError::Stop {
                        module: "ingest".into(),
                        code: "E1207".into(),
                        message: format!("depth_min > depth_max for {s}"),
                    });
                }
            }
        }
    }
    Ok(())
}

pub fn validate_metal_concentrations(
    headers: &[String],
    rows: &[Vec<String>],
    conn: &Connection,
) -> Result<()> {
    let cid = col(headers, "concentration_id")?;
    let sid = col(headers, "sample_id")?;
    let site = col(headers, "site_id")?;
    let metal = col(headers, "metal")?;
    let value = col(headers, "value")?;
    let unit = col(headers, "unit")?;
    let flag = col(headers, "detect_flag")?;
    let dl = header_idx(headers, "detection_limit");

    let mut seen = std::collections::HashSet::new();
    for r in rows {
        let c = r[cid].trim();
        if c.is_empty() || !seen.insert(c.to_string()) {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1208".into(),
                message: format!("concentration_id issue: {c}"),
            });
        }
        if !sample_exists(conn, r[sid].trim())? {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1209".into(),
                message: format!("concentration {c} references unknown sample_id {}", r[sid]),
            });
        }
        if !site_exists(conn, r[site].trim())? {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1209".into(),
                message: format!("concentration {c} references unknown site_id {}", r[site]),
            });
        }
        if r[metal].trim().is_empty() {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1210".into(),
                message: format!("concentration {c} has blank metal"),
            });
        }
        let v: f64 = r[value].parse().map_err(|_| BayesDsmError::Stop {
            module: "ingest".into(),
            code: "E1211".into(),
            message: format!("concentration {c} value is not a float"),
        })?;
        if v < 0.0 {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1211".into(),
                message: format!("concentration {c} value is negative"),
            });
        }
        if !is_known_unit(r[unit].trim()) {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1212".into(),
                message: format!("concentration {c} has unrecognised unit '{}'", r[unit]),
            });
        }
        let f: i64 = r[flag].parse().map_err(|_| BayesDsmError::Stop {
            module: "ingest".into(),
            code: "E1213".into(),
            message: format!("detect_flag for {c} is not 0 or 1"),
        })?;
        if f != 0 && f != 1 {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1213".into(),
                message: format!("detect_flag for {c} must be 0 or 1"),
            });
        }
        if f == 1 && v <= 0.0 {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1214".into(),
                message: format!("detect_flag=1 but value <= 0 for {c}"),
            });
        }
        if f == 0 {
            if let Some(i) = dl {
                let d: f64 = r[i].parse().unwrap_or(0.0);
                if d <= 0.0 {
                    return Err(BayesDsmError::Stop {
                        module: "ingest".into(),
                        code: "E1215".into(),
                        message: format!("detect_flag=0 but detection_limit <= 0 for {c}"),
                    });
                }
            } else {
                return Err(BayesDsmError::Stop {
                    module: "ingest".into(),
                    code: "E1215".into(),
                    message: format!("detect_flag=0 but detection_limit missing for {c}"),
                });
            }
        }
    }
    Ok(())
}

pub fn validate_background_values(headers: &[String], rows: &[Vec<String>]) -> Result<()> {
    let bid = col(headers, "background_id")?;
    let metal = col(headers, "metal")?;
    let val = col(headers, "background_value")?;
    let unit = col(headers, "unit")?;
    let btype = col(headers, "background_type")?;
    let unc = header_idx(headers, "uncertainty_sd");

    let mut seen = std::collections::HashSet::new();
    for r in rows {
        let b = r[bid].trim();
        if b.is_empty() || !seen.insert(b.to_string()) {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1216".into(),
                message: format!("background_id issue: {b}"),
            });
        }
        if r[metal].trim().is_empty() {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1217".into(),
                message: format!("background {b} has blank metal"),
            });
        }
        let v: f64 = r[val].parse().map_err(|_| BayesDsmError::Stop {
            module: "ingest".into(),
            code: "E1218".into(),
            message: format!("background_value for {b} is not a float"),
        })?;
        if v <= 0.0 {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1218".into(),
                message: format!("background_value for {b} must be > 0"),
            });
        }
        if !is_known_unit(r[unit].trim()) {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1219".into(),
                message: format!("background {b} has unrecognised unit '{}'", r[unit]),
            });
        }
        if !is_known_bg_type(r[btype].trim()) {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1220".into(),
                message: format!("background {b} has unrecognised type '{}'", r[btype]),
            });
        }
        if let Some(i) = unc {
            if !r[i].is_empty() {
                let u: f64 = r[i].parse().unwrap_or(-1.0);
                if u < 0.0 {
                    return Err(BayesDsmError::Stop {
                        module: "ingest".into(),
                        code: "E1221".into(),
                        message: format!("uncertainty_sd for {b} is negative"),
                    });
                }
            }
        }
    }
    Ok(())
}

pub fn validate_dsmt_hypotheses(headers: &[String], rows: &[Vec<String>]) -> Result<()> {
    let hid = col(headers, "hypothesis_id")?;
    let sym = col(headers, "hypothesis_symbol")?;
    let name = col(headers, "hypothesis_name")?;
    let desc = col(headers, "description")?;
    let weight = col(headers, "default_risk_weight")?;
    let active = col(headers, "active")?;

    let mut seen_id = std::collections::HashSet::new();
    let mut seen_sym = std::collections::HashSet::new();
    let mut n_active = 0;
    for r in rows {
        let h = r[hid].trim();
        if h.is_empty() || !seen_id.insert(h.to_string()) {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1222".into(),
                message: format!("hypothesis_id issue: {h}"),
            });
        }
        if !seen_sym.insert(r[sym].trim().to_string()) {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1223".into(),
                message: format!("duplicate hypothesis_symbol: {}", r[sym]),
            });
        }
        if r[name].trim().is_empty() || r[desc].trim().is_empty() {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1224".into(),
                message: format!("hypothesis {h} missing name or description"),
            });
        }
        let w: f64 = r[weight].parse().map_err(|_| BayesDsmError::Stop {
            module: "ingest".into(),
            code: "E1225".into(),
            message: format!("default_risk_weight for {h} is not a float"),
        })?;
        if !(0.0..=1.0).contains(&w) {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1225".into(),
                message: format!("default_risk_weight for {h} not in [0,1]"),
            });
        }
        let a: i64 = r[active].parse().map_err(|_| BayesDsmError::Stop {
            module: "ingest".into(),
            code: "E1226".into(),
            message: format!("active flag for {h} is not 0 or 1"),
        })?;
        if a == 1 {
            n_active += 1;
        }
    }
    if n_active < 2 {
        return Err(BayesDsmError::Stop {
            module: "ingest".into(),
            code: "E1227".into(),
            message: format!("at least two active hypotheses required (got {n_active})"),
        });
    }
    Ok(())
}

pub fn validate_config_parameters(headers: &[String], rows: &[Vec<String>]) -> Result<()> {
    let name = col(headers, "parameter_name")?;
    let val = col(headers, "parameter_value")?;
    let ptype = col(headers, "parameter_type")?;
    let module = col(headers, "module")?;
    let required: Vec<String> = vec![
        "project_id",
        "random_seed",
        "standard_concentration_unit",
        "nondetect_method",
        "reference_element",
        "enrichment_threshold",
        "bayes_model_mode",
        "source_model_mode",
        "hotspot_model_mode",
        "dsmt_model",
        "belief_uncertainty_width_max",
        "single_source_margin",
        "union_support_threshold",
        "neutrosophic_indeterminacy_penalty",
        "ranking_band_critical",
        "ranking_band_high",
        "ranking_band_moderate",
        "sqlite_path",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    let present: std::collections::HashSet<String> =
        rows.iter().map(|r| r[name].trim().to_string()).collect();
    for r in &required {
        if !present.contains(r) {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1228".into(),
                message: format!("required config parameter '{r}' missing"),
            });
        }
    }
    for r in rows {
        let t = r[ptype].trim();
        match t {
            "int" | "float" | "bool" | "string" | "enum" => {}
            _ => {
                return Err(BayesDsmError::Stop {
                    module: "ingest".into(),
                    code: "E1229".into(),
                    message: format!("invalid parameter_type '{}'", t),
                })
            }
        }
        if t == "int" && r[val].trim().parse::<i64>().is_err() {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1230".into(),
                message: format!("parameter {} should be int", r[name]),
            });
        }
        if t == "float" && r[val].trim().parse::<f64>().is_err() {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1230".into(),
                message: format!("parameter {} should be float", r[name]),
            });
        }
        if r[name].trim() == "random_seed" && r[val].trim().parse::<i64>().is_err() {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1230".into(),
                message: "random_seed must be integer".into(),
            });
        }
        if r[name].trim() == "dsmt_model" && !["free", "hybrid", "shafer"].contains(&r[val].trim())
        {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1231".into(),
                message: "dsmt_model must be free, hybrid, or shafer".into(),
            });
        }
        if r[name].trim() == "nondetect_method"
            && !["dl_sqrt2", "dl_half", "censored_bayes"].contains(&r[val].trim())
        {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1231".into(),
                message: "nondetect_method must be dl_sqrt2, dl_half, or censored_bayes".into(),
            });
        }
        if r[module].trim().is_empty() {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1232".into(),
                message: format!("module is blank for parameter {}", r[name]),
            });
        }
    }
    Ok(())
}

pub fn validate_source_indicators(
    headers: &[String],
    rows: &[Vec<String>],
    conn: &Connection,
) -> Result<()> {
    let iid = col(headers, "indicator_id")?;
    let site = col(headers, "site_id")?;
    let hyp = col(headers, "hypothesis_id")?;
    let name = col(headers, "indicator_name")?;
    let val = col(headers, "indicator_value")?;
    let dir = col(headers, "direction")?;
    let rel = col(headers, "reliability_weight")?;

    let mut seen = std::collections::HashSet::new();
    for r in rows {
        let i = r[iid].trim();
        if i.is_empty() || !seen.insert(i.to_string()) {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1233".into(),
                message: format!("indicator_id issue: {i}"),
            });
        }
        if !site_exists(conn, r[site].trim())? {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1234".into(),
                message: format!(
                    "source indicator {i} references unknown site_id {}",
                    r[site]
                ),
            });
        }
        if !hypothesis_exists(conn, r[hyp].trim())? {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1234".into(),
                message: format!(
                    "source indicator {i} references unknown hypothesis_id {}",
                    r[hyp]
                ),
            });
        }
        if r[name].trim().is_empty() {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1235".into(),
                message: format!("source indicator {i} has blank name"),
            });
        }
        let _v: f64 = r[val].parse().map_err(|_| BayesDsmError::Stop {
            module: "ingest".into(),
            code: "E1236".into(),
            message: format!("indicator_value for {i} is not a float"),
        })?;
        if !["higher_risk", "lower_risk"].contains(&r[dir].trim()) {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1237".into(),
                message: format!("direction for {i} must be higher_risk or lower_risk"),
            });
        }
        let w: f64 = r[rel].parse().map_err(|_| BayesDsmError::Stop {
            module: "ingest".into(),
            code: "E1238".into(),
            message: format!("reliability_weight for {i} is not a float"),
        })?;
        if !(0.0..=1.0).contains(&w) {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1238".into(),
                message: format!("reliability_weight for {i} not in [0,1]"),
            });
        }
    }
    Ok(())
}

pub fn validate_exposure_indicators(
    headers: &[String],
    rows: &[Vec<String>],
    conn: &Connection,
) -> Result<()> {
    let eid = col(headers, "exposure_id")?;
    let site = col(headers, "site_id")?;
    let rt = col(headers, "receptor_type")?;
    let dir = col(headers, "direction")?;
    let rel = col(headers, "reliability_weight")?;

    let mut seen = std::collections::HashSet::new();
    for r in rows {
        let i = r[eid].trim();
        if i.is_empty() || !seen.insert(i.to_string()) {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1239".into(),
                message: format!("exposure_id issue: {i}"),
            });
        }
        if !site_exists(conn, r[site].trim())? {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1240".into(),
                message: format!("exposure {i} references unknown site_id {}", r[site]),
            });
        }
        if ![
            "water_intake",
            "settlement",
            "fishery",
            "wetland",
            "irrigation",
        ]
        .contains(&r[rt].trim())
        {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1241".into(),
                message: format!("receptor_type '{}' not recognised for {i}", r[rt]),
            });
        }
        if !["higher_risk", "lower_risk"].contains(&r[dir].trim()) {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1241".into(),
                message: format!("direction for {i} must be higher_risk or lower_risk"),
            });
        }
        let w: f64 = r[rel].parse().map_err(|_| BayesDsmError::Stop {
            module: "ingest".into(),
            code: "E1242".into(),
            message: format!("reliability_weight for {i} is not a float"),
        })?;
        if !(0.0..=1.0).contains(&w) {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1242".into(),
                message: format!("reliability_weight for {i} not in [0,1]"),
            });
        }
    }
    Ok(())
}

pub fn validate_confidence_indicators(
    headers: &[String],
    rows: &[Vec<String>],
    conn: &Connection,
) -> Result<()> {
    let cid = col(headers, "confidence_id")?;
    let site = col(headers, "site_id")?;
    let dir = col(headers, "direction")?;
    let rel = col(headers, "reliability_weight")?;
    let mut seen = std::collections::HashSet::new();
    for r in rows {
        let i = r[cid].trim();
        if i.is_empty() || !seen.insert(i.to_string()) {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1243".into(),
                message: format!("confidence_id issue: {i}"),
            });
        }
        if !site_exists(conn, r[site].trim())? {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1244".into(),
                message: format!("confidence {i} references unknown site_id {}", r[site]),
            });
        }
        if !["higher_confidence", "lower_confidence"].contains(&r[dir].trim()) {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1245".into(),
                message: format!("direction for {i} must be higher_confidence or lower_confidence"),
            });
        }
        let w: f64 = r[rel].parse().map_err(|_| BayesDsmError::Stop {
            module: "ingest".into(),
            code: "E1246".into(),
            message: format!("reliability_weight for {i} is not a float"),
        })?;
        if !(0.0..=1.0).contains(&w) {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1246".into(),
                message: format!("reliability_weight for {i} not in [0,1]"),
            });
        }
    }
    Ok(())
}

pub fn validate_leakage_rules(headers: &[String], rows: &[Vec<String>]) -> Result<()> {
    let rid = col(headers, "rule_id")?;
    let var = col(headers, "variable_name")?;
    let rt = col(headers, "rule_type")?;
    let reason = col(headers, "reason")?;
    let forbidden = col(headers, "forbidden_for_module")?;
    let mut seen = std::collections::HashSet::new();
    for r in rows {
        let i = r[rid].trim();
        if i.is_empty() || !seen.insert(i.to_string()) {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1247".into(),
                message: format!("rule_id issue: {i}"),
            });
        }
        if !["exclude", "warn", "time_restrict", "group_restrict"].contains(&r[rt].trim()) {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1248".into(),
                message: format!("rule_type '{}' not recognised for {i}", r[rt]),
            });
        }
        if r[var].trim().is_empty() || r[reason].trim().is_empty() {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1249".into(),
                message: format!("rule {i} missing variable_name or reason"),
            });
        }
        if !["bayes", "validation", "ranking", "all"].contains(&r[forbidden].trim()) {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1250".into(),
                message: format!("forbidden_for_module '{}' for {i}", r[forbidden]),
            });
        }
    }
    Ok(())
}

pub fn validate_stakeholder_weights(headers: &[String], rows: &[Vec<String>]) -> Result<()> {
    let wid = col(headers, "weight_id")?;
    let crit = col(headers, "criterion")?;
    let w = col(headers, "weight")?;
    let src = col(headers, "weight_source")?;
    let mut seen = std::collections::HashSet::new();
    let mut total = 0.0_f64;
    for r in rows {
        let i = r[wid].trim();
        if i.is_empty() || !seen.insert(i.to_string()) {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1251".into(),
                message: format!("weight_id issue: {i}"),
            });
        }
        if r[crit].trim().is_empty() {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1252".into(),
                message: format!("criterion blank for {i}"),
            });
        }
        let wv: f64 = r[w].parse().map_err(|_| BayesDsmError::Stop {
            module: "ingest".into(),
            code: "E1253".into(),
            message: format!("weight for {i} is not a float"),
        })?;
        if wv < 0.0 {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1253".into(),
                message: format!("weight for {i} is negative"),
            });
        }
        if !["stakeholder", "entropy", "equal", "expert"].contains(&r[src].trim()) {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1254".into(),
                message: format!("weight_source '{}' for {i}", r[src]),
            });
        }
        total += wv;
    }
    if (total - 1.0).abs() > 1e-8 {
        return Err(BayesDsmError::Stop {
            module: "ingest".into(),
            code: "E1255".into(),
            message: format!("stakeholder_weights must sum to 1 (got {total})"),
        });
    }
    Ok(())
}

pub fn validate_validation_labels(
    headers: &[String],
    rows: &[Vec<String>],
    conn: &Connection,
) -> Result<()> {
    let lid = col(headers, "label_id")?;
    let site = col(headers, "site_id")?;
    let lname = col(headers, "label_name")?;
    let ldate = col(headers, "label_date")?;
    let lsrc = col(headers, "label_source")?;
    let indep = col(headers, "independent_of_features")?;
    let mut seen = std::collections::HashSet::new();
    for r in rows {
        let i = r[lid].trim();
        if i.is_empty() || !seen.insert(i.to_string()) {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1256".into(),
                message: format!("label_id issue: {i}"),
            });
        }
        if !site_exists(conn, r[site].trim())? {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1257".into(),
                message: format!("label {i} references unknown site_id {}", r[site]),
            });
        }
        if r[lname].trim().is_empty() {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1258".into(),
                message: format!("label_name blank for {i}"),
            });
        }
        if chrono::NaiveDate::parse_from_str(r[ldate].trim(), "%Y-%m-%d").is_err() {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1259".into(),
                message: format!("label_date for {i} is not YYYY-MM-DD"),
            });
        }
        if !["field", "regulatory", "expert", "historical"].contains(&r[lsrc].trim()) {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1260".into(),
                message: format!("label_source '{}' for {i}", r[lsrc]),
            });
        }
        let f: i64 = r[indep].parse().unwrap_or(-1);
        if f != 0 && f != 1 {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1261".into(),
                message: format!("independent_of_features must be 0 or 1 for {i}"),
            });
        }
    }
    Ok(())
}

pub fn validate_dsmt_constraints(headers: &[String], rows: &[Vec<String>]) -> Result<()> {
    let cid = col(headers, "constraint_id")?;
    let expr = col(headers, "expression")?;
    let ctype = col(headers, "constraint_type")?;
    let reason = col(headers, "reason")?;
    let active = col(headers, "active")?;
    let mut seen = std::collections::HashSet::new();
    for r in rows {
        let i = r[cid].trim();
        if i.is_empty() || !seen.insert(i.to_string()) {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1262".into(),
                message: format!("constraint_id issue: {i}"),
            });
        }
        if r[expr].trim().is_empty() {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1263".into(),
                message: format!("expression blank for {i}"),
            });
        }
        if !["empty", "allowed", "forced_union"].contains(&r[ctype].trim()) {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1264".into(),
                message: format!("constraint_type '{}' for {i}", r[ctype]),
            });
        }
        if r[reason].trim().is_empty() {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1265".into(),
                message: format!("reason blank for {i}"),
            });
        }
        let a: i64 = r[active].parse().unwrap_or(-1);
        if a != 0 && a != 1 {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1266".into(),
                message: format!("active must be 0 or 1 for {i}"),
            });
        }
    }
    Ok(())
}

pub fn validate_data_dictionary(headers: &[String], rows: &[Vec<String>]) -> Result<()> {
    let var = col(headers, "variable_name")?;
    let tab = col(headers, "table_name")?;
    let vt = col(headers, "variable_type")?;
    for r in rows {
        if r[var].trim().is_empty() || r[tab].trim().is_empty() {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1267".into(),
                message: "data_dictionary: blank variable or table".into(),
            });
        }
        if !["numeric", "categorical", "date", "text"].contains(&r[vt].trim()) {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1268".into(),
                message: format!("variable_type '{}' not recognised", r[vt]),
            });
        }
    }
    Ok(())
}

pub fn validate_allowed_values(headers: &[String], rows: &[Vec<String>]) -> Result<()> {
    let f = col(headers, "field_name")?;
    let v = col(headers, "allowed_value")?;
    for r in rows {
        if r[f].trim().is_empty() || r[v].trim().is_empty() {
            return Err(BayesDsmError::Stop {
                module: "ingest".into(),
                code: "E1269".into(),
                message: "allowed_values: blank field or value".into(),
            });
        }
    }
    Ok(())
}

// ===========================================================================
// helpers
// ===========================================================================

fn col(headers: &[String], name: &str) -> Result<usize> {
    headers
        .iter()
        .position(|h| h.eq_ignore_ascii_case(name))
        .ok_or_else(|| BayesDsmError::Stop {
            module: "ingest".into(),
            code: "E1200".into(),
            message: format!("required column '{name}' missing"),
        })
}

fn header_idx(headers: &[String], name: &str) -> Option<usize> {
    headers.iter().position(|h| h.eq_ignore_ascii_case(name))
}

fn is_known_unit(u: &str) -> bool {
    matches!(u, "mg/kg" | "µg/kg" | "ug/kg" | "g/kg" | "ppm" | "ppb")
}

fn is_known_bg_type(t: &str) -> bool {
    matches!(
        t,
        "local" | "regional" | "crustal" | "regulatory" | "literature"
    )
}

fn site_exists(conn: &Connection, site_id: &str) -> Result<bool> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM raw_sites WHERE site_id = ?1",
        [site_id],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

fn sample_exists(conn: &Connection, sample_id: &str) -> Result<bool> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM raw_sampling_events WHERE sample_id = ?1",
        [sample_id],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

fn hypothesis_exists(conn: &Connection, h: &str) -> Result<bool> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM raw_dsmt_hypotheses WHERE hypothesis_id = ?1 AND active = 1",
        [h],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

// Suppress unused param warning for conn arg in some checks.
#[allow(dead_code)]
fn _force_use_params(_c: &Connection) {}
