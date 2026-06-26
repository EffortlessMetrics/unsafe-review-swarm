//! Source-of-truth ledger gates (`check-goals`, `check-package-boundary`) and
//! the shared `.rails/index.toml` index parsers they (and `spec_status`) build on.
//!
//! Extracted from `main.rs` as part of #1806 (xtask modularization). Validates
//! `.rails/goals/active.toml` (schema, work items, lane/artifact cross-refs) and
//! `policy/package-boundary.toml` (package classification + Cargo.toml presence),
//! and exposes `source_truth_index_ids` / `source_truth_index_artifacts` for
//! `.rails/index.toml`, which `check_doc_artifacts_impl` and `spec_status` also use.

use crate::{
    ACTIVE_GOAL_MANIFEST, DOC_ARTIFACT_LEDGER, DocArtifactEntry, GOAL_WORK_ITEM_STATUSES,
    PACKAGE_BOUNDARY_LEDGER, PACKAGE_CLASSIFICATIONS, SOURCE_OF_TRUTH_INDEX,
    check_doc_artifacts_impl, parse_toml_file, require_file, require_known, require_toml_string,
    required_table_string, required_toml_string, toml_array, toml_str_array, toml_table,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub(crate) fn check_goals() -> Result<(), String> {
    let artifact_ids = check_doc_artifacts_impl()?;
    let source_index = parse_toml_file(Path::new(SOURCE_OF_TRUTH_INDEX))?;
    let indexed_artifact_ids = source_truth_index_ids(&source_index, "artifact")?;
    let indexed_lane_ids = source_truth_index_ids(&source_index, "lane")?;
    let value = parse_toml_file(Path::new(ACTIVE_GOAL_MANIFEST))?;
    require_toml_string(&value, "schema_version", ACTIVE_GOAL_MANIFEST)?;
    for key in ["id", "title", "status", "owner", "created", "objective"] {
        required_toml_string(&value, key, ACTIVE_GOAL_MANIFEST)?;
    }
    require_known(
        required_toml_string(&value, "status", ACTIVE_GOAL_MANIFEST)?,
        GOAL_WORK_ITEM_STATUSES,
        ACTIVE_GOAL_MANIFEST,
        "status",
    )?;
    let end_state = toml_array(&value, "end_state", ACTIVE_GOAL_MANIFEST)?;
    if end_state.is_empty() {
        return Err(format!(
            "{ACTIVE_GOAL_MANIFEST} end_state must not be empty"
        ));
    }
    for item in end_state {
        if item.as_str().is_none_or(|value| value.trim().is_empty()) {
            return Err(format!(
                "{ACTIVE_GOAL_MANIFEST} end_state entries must be non-empty strings"
            ));
        }
    }

    let work_items = toml_array(&value, "work_item", ACTIVE_GOAL_MANIFEST)?;
    if work_items.is_empty() {
        return Err(format!(
            "{ACTIVE_GOAL_MANIFEST} must list at least one work_item"
        ));
    }
    let mut ids = BTreeSet::new();
    for (idx, item) in work_items.iter().enumerate() {
        let table = toml_table(item, ACTIVE_GOAL_MANIFEST, "work_item", idx)?;
        let id = required_table_string(table, "id", ACTIVE_GOAL_MANIFEST, "work_item", idx)?;
        if !ids.insert(id.to_string()) {
            return Err(format!(
                "{ACTIVE_GOAL_MANIFEST} contains duplicate work_item `{id}`"
            ));
        }
        let status =
            required_table_string(table, "status", ACTIVE_GOAL_MANIFEST, "work_item", idx)?;
        require_known(
            status,
            GOAL_WORK_ITEM_STATUSES,
            ACTIVE_GOAL_MANIFEST,
            "work_item.status",
        )?;
        for key in ["proposal", "spec"] {
            if let Some(linked_id) = table.get(key).and_then(toml::Value::as_str)
                && !artifact_ids.contains(linked_id)
            {
                return Err(format!(
                    "{ACTIVE_GOAL_MANIFEST} work_item `{id}` references {key} `{linked_id}` not listed in {DOC_ARTIFACT_LEDGER}"
                ));
            }
            if let Some(linked_id) = table.get(key).and_then(toml::Value::as_str)
                && !indexed_artifact_ids.contains(linked_id)
            {
                return Err(format!(
                    "{ACTIVE_GOAL_MANIFEST} work_item `{id}` references {key} `{linked_id}` not listed in {SOURCE_OF_TRUTH_INDEX}"
                ));
            }
        }
        if !indexed_lane_ids.contains(id) {
            return Err(format!(
                "{ACTIVE_GOAL_MANIFEST} work_item `{id}` is not listed as a lane in {SOURCE_OF_TRUTH_INDEX}"
            ));
        }
        let plan = required_table_string(table, "plan", ACTIVE_GOAL_MANIFEST, "work_item", idx)?;
        require_file(plan)?;
        let commands = table.get("commands").ok_or_else(|| {
            format!("{ACTIVE_GOAL_MANIFEST} work_item `{id}` is missing commands")
        })?;
        let commands = toml_str_array(commands, ACTIVE_GOAL_MANIFEST, "commands")?;
        if commands.is_empty() {
            return Err(format!(
                "{ACTIVE_GOAL_MANIFEST} work_item `{id}` commands must not be empty"
            ));
        }
    }
    println!("check-goals: ok ({} work items)", ids.len());
    Ok(())
}

pub(crate) fn source_truth_index_ids(
    value: &toml::Value,
    kind: &str,
) -> Result<BTreeSet<String>, String> {
    let entries = toml_array(value, kind, SOURCE_OF_TRUTH_INDEX)?;
    let mut ids = BTreeSet::new();
    for (idx, entry) in entries.iter().enumerate() {
        let table = toml_table(entry, SOURCE_OF_TRUTH_INDEX, kind, idx)?;
        let id = required_table_string(table, "id", SOURCE_OF_TRUTH_INDEX, kind, idx)?;
        if !ids.insert(id.to_string()) {
            return Err(format!(
                "{SOURCE_OF_TRUTH_INDEX} contains duplicate {kind} id `{id}`"
            ));
        }
        let path = required_table_string(table, "path", SOURCE_OF_TRUTH_INDEX, kind, idx)?;
        require_file(path)?;
        required_table_string(table, "status", SOURCE_OF_TRUTH_INDEX, kind, idx)?;
        required_table_string(table, "owner", SOURCE_OF_TRUTH_INDEX, kind, idx)?;
    }
    Ok(ids)
}

pub(crate) fn source_truth_index_artifacts(
    value: &toml::Value,
) -> Result<BTreeMap<String, DocArtifactEntry>, String> {
    let entries = toml_array(value, "artifact", SOURCE_OF_TRUTH_INDEX)?;
    let mut artifacts = BTreeMap::new();
    for (idx, entry) in entries.iter().enumerate() {
        let table = toml_table(entry, SOURCE_OF_TRUTH_INDEX, "artifact", idx)?;
        let id = required_table_string(table, "id", SOURCE_OF_TRUTH_INDEX, "artifact", idx)?;
        if artifacts.contains_key(id) {
            return Err(format!(
                "{SOURCE_OF_TRUTH_INDEX} contains duplicate artifact id `{id}`"
            ));
        }
        let kind = required_table_string(table, "kind", SOURCE_OF_TRUTH_INDEX, "artifact", idx)?;
        let path = required_table_string(table, "path", SOURCE_OF_TRUTH_INDEX, "artifact", idx)?;
        let status =
            required_table_string(table, "status", SOURCE_OF_TRUTH_INDEX, "artifact", idx)?;
        let owner = required_table_string(table, "owner", SOURCE_OF_TRUTH_INDEX, "artifact", idx)?;
        require_file(path)?;
        artifacts.insert(
            id.to_string(),
            DocArtifactEntry {
                kind: kind.to_string(),
                path: path.to_string(),
                status: status.to_string(),
                owner: owner.to_string(),
            },
        );
    }
    Ok(artifacts)
}

pub(crate) fn check_package_boundary() -> Result<(), String> {
    let value = parse_toml_file(Path::new(PACKAGE_BOUNDARY_LEDGER))?;
    require_toml_string(&value, "schema_version", PACKAGE_BOUNDARY_LEDGER)?;
    let packages = toml_array(&value, "package", PACKAGE_BOUNDARY_LEDGER)?;
    if packages.is_empty() {
        return Err(format!(
            "{PACKAGE_BOUNDARY_LEDGER} must list at least one package"
        ));
    }
    let mut names = BTreeSet::new();
    for (idx, package) in packages.iter().enumerate() {
        let table = toml_table(package, PACKAGE_BOUNDARY_LEDGER, "package", idx)?;
        let name = required_table_string(table, "name", PACKAGE_BOUNDARY_LEDGER, "package", idx)?;
        if !names.insert(name.to_string()) {
            return Err(format!(
                "{PACKAGE_BOUNDARY_LEDGER} contains duplicate package `{name}`"
            ));
        }
        let path = required_table_string(table, "path", PACKAGE_BOUNDARY_LEDGER, "package", idx)?;
        let classification = required_table_string(
            table,
            "classification",
            PACKAGE_BOUNDARY_LEDGER,
            "package",
            idx,
        )?;
        require_known(
            classification,
            PACKAGE_CLASSIFICATIONS,
            PACKAGE_BOUNDARY_LEDGER,
            "classification",
        )?;
        required_table_string(table, "owner", PACKAGE_BOUNDARY_LEDGER, "package", idx)?;
        required_table_string(table, "reason", PACKAGE_BOUNDARY_LEDGER, "package", idx)?;
        require_file(&format!("{path}/Cargo.toml"))?;
    }
    println!("check-package-boundary: ok ({} packages)", names.len());
    Ok(())
}
