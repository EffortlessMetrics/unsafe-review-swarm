//! CI lane ledger gate (`check-ci-lanes`).
//!
//! Extracted from `main.rs` as part of #1806 (xtask modularization). Validates
//! `policy/ci-lane-whitelist.toml`: required keys per lane, known statuses, and
//! the policy-contracts workflow presence check.

use crate::{
    CI_LANE_LEDGER, CI_LANE_STATUSES, parse_toml_file, require_file, require_known,
    require_toml_string, required_table_string, toml_array, toml_table,
};
use std::collections::BTreeSet;
use std::path::Path;

const REQUIRED_LANE_KEYS: &[&str] = &[
    "owner",
    "intent",
    "proof_obligation",
    "cost_estimate",
    "trigger_policy",
    "review_after",
];

pub(crate) fn check() -> Result<(), String> {
    let lanes = parse_lanes()?;
    let lane_ids = collect_lane_ids(lanes)?;
    println!("check-ci-lanes: ok ({} lanes)", lane_ids.len());
    Ok(())
}

fn parse_lanes() -> Result<Vec<toml::Value>, String> {
    let value = parse_toml_file(Path::new(CI_LANE_LEDGER))?;
    require_toml_string(&value, "schema_version", CI_LANE_LEDGER)?;
    let lanes = toml_array(&value, "lane", CI_LANE_LEDGER)?;
    if lanes.is_empty() {
        return Err(format!("{CI_LANE_LEDGER} must list at least one lane"));
    }
    Ok(lanes.to_vec())
}

fn collect_lane_ids(lanes: Vec<toml::Value>) -> Result<BTreeSet<String>, String> {
    let mut ids = BTreeSet::new();
    for (idx, lane) in lanes.iter().enumerate() {
        let lane_id = validate_lane(lane, idx)?;
        if !ids.insert(lane_id.to_string()) {
            return Err(format!(
                "{CI_LANE_LEDGER} contains duplicate lane `{lane_id}`"
            ));
        }
    }
    Ok(ids)
}

fn validate_lane(lane: &toml::Value, idx: usize) -> Result<&str, String> {
    let table = toml_table(lane, CI_LANE_LEDGER, "lane", idx)?;
    let lane_id = required_table_string(table, "id", CI_LANE_LEDGER, "lane", idx)?;
    for key in REQUIRED_LANE_KEYS {
        required_table_string(table, key, CI_LANE_LEDGER, "lane", idx)?;
    }
    let status = required_table_string(table, "status", CI_LANE_LEDGER, "lane", idx)?;
    require_known(status, CI_LANE_STATUSES, CI_LANE_LEDGER, "status")?;
    if lane_id == "policy-contracts" {
        require_file(".github/workflows/policy-contracts.yml")?;
    }
    Ok(lane_id)
}
