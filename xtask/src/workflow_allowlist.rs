use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::{
    WORKFLOW_ALLOWLIST, WORKFLOW_DIR, looks_like_iso_date, parse_toml_file, read_to_string,
    required_toml_string, workspace_path,
};

#[derive(Debug)]
pub(crate) struct WorkflowPolicyEntry {
    pub(crate) path: String,
    pub(crate) permissions: String,
    pub(crate) actions: BTreeSet<String>,
}

pub(crate) fn check_workflow_allowlist(
    allowlist: &Path,
    workflow_dir: &Path,
) -> Result<(), String> {
    let policies = workflow_policy_entries(allowlist)?;
    let mut by_path = BTreeMap::new();
    for entry in policies {
        let workflow_path = workspace_path(&entry.path);
        if !workflow_path.is_file() {
            return Err(format!(
                "{} lists missing workflow `{}`",
                allowlist.display(),
                entry.path
            ));
        }
        let text = read_to_string(&workflow_path)?;
        check_workflow_text_against_policy(&entry.path, &text, &entry)?;
        if by_path.insert(entry.path.clone(), entry).is_some() {
            return Err(format!(
                "{} contains duplicate workflow entry",
                allowlist.display()
            ));
        }
    }

    for workflow in workflow_files(workflow_dir)? {
        if !by_path.contains_key(&workflow) {
            return Err(format!(
                "{} is missing workflow allowlist entry for `{workflow}`",
                allowlist.display()
            ));
        }
    }

    Ok(())
}

fn workflow_policy_entries(allowlist: &Path) -> Result<Vec<WorkflowPolicyEntry>, String> {
    let value = parse_toml_file(allowlist)?;
    let path_display = allowlist.display().to_string();
    let entries = value
        .get("workflow")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{path_display} must contain [[workflow]] entries"))?;
    if entries.is_empty() {
        return Err(format!(
            "{path_display} must contain at least one workflow entry"
        ));
    }

    let mut out = Vec::new();
    for (idx, entry) in entries.iter().enumerate() {
        let path = required_toml_string(entry, "path", &format!("{path_display} workflow[{idx}]"))?
            .to_string();
        let permissions = required_toml_string(
            entry,
            "permissions",
            &format!("{path_display} workflow[{idx}]"),
        )?
        .to_string();
        let reason =
            required_toml_string(entry, "reason", &format!("{path_display} workflow[{idx}]"))?;
        if reason.len() < 16 {
            return Err(format!(
                "{path_display} workflow[{idx}] reason is too terse"
            ));
        }
        let review_after = required_toml_string(
            entry,
            "review_after",
            &format!("{path_display} workflow[{idx}]"),
        )?;
        if !looks_like_iso_date(review_after) {
            return Err(format!(
                "{path_display} workflow[{idx}] review_after must use YYYY-MM-DD"
            ));
        }
        let actions = entry
            .get("actions")
            .and_then(toml::Value::as_array)
            .ok_or_else(|| format!("{path_display} workflow[{idx}] is missing actions array"))?;
        let mut action_set = BTreeSet::new();
        for (action_idx, action) in actions.iter().enumerate() {
            let Some(action) = action.as_str() else {
                return Err(format!(
                    "{path_display} workflow[{idx}] actions[{action_idx}] must be a string"
                ));
            };
            if action.trim().is_empty() {
                return Err(format!(
                    "{path_display} workflow[{idx}] actions[{action_idx}] is empty"
                ));
            }
            action_set.insert(action.to_string());
        }
        if action_set.is_empty() {
            return Err(format!(
                "{path_display} workflow[{idx}] must list at least one action"
            ));
        }
        out.push(WorkflowPolicyEntry {
            path,
            permissions,
            actions: action_set,
        });
    }
    Ok(out)
}

pub(crate) fn check_workflow_text_against_policy(
    path: &str,
    text: &str,
    policy: &WorkflowPolicyEntry,
) -> Result<(), String> {
    if !workflow_declares_permission(text, &policy.permissions) {
        return Err(format!(
            "{path} must declare workflow permission `{}`",
            policy.permissions
        ));
    }

    let used_actions = workflow_used_actions(text);
    for action in &used_actions {
        if !policy.actions.contains(action) {
            return Err(format!(
                "{path} uses action `{action}` that is not listed in {WORKFLOW_ALLOWLIST}"
            ));
        }
    }
    for action in &policy.actions {
        if !used_actions.contains(action) {
            return Err(format!(
                "{WORKFLOW_ALLOWLIST} lists action `{action}` for {path}, but the workflow does not use it"
            ));
        }
    }
    Ok(())
}

fn workflow_declares_permission(text: &str, permission: &str) -> bool {
    text.lines().any(|line| line.trim() == "permissions:")
        && text.lines().any(|line| line.trim() == permission)
}

pub(crate) fn workflow_used_actions(text: &str) -> BTreeSet<String> {
    let mut actions = BTreeSet::new();
    for line in text.lines() {
        let trimmed = line.trim();
        let trimmed = trimmed.strip_prefix("- ").unwrap_or(trimmed);
        let Some(raw_action) = trimmed.strip_prefix("uses:") else {
            continue;
        };
        let action = raw_action
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim();
        if !action.is_empty() {
            actions.insert(action.to_string());
        }
    }
    actions
}

fn workflow_files(workflow_dir: &Path) -> Result<BTreeSet<String>, String> {
    let dir = workspace_path(&workflow_dir.display().to_string());
    let entries =
        std::fs::read_dir(&dir).map_err(|err| format!("read {} failed: {err}", dir.display()))?;
    let mut files = BTreeSet::new();
    for entry in entries {
        let entry = entry.map_err(|err| format!("read_dir entry failed: {err}"))?;
        let path = entry.path();
        let extension = path.extension().and_then(std::ffi::OsStr::to_str);
        if !matches!(extension, Some("yml" | "yaml")) {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(std::ffi::OsStr::to_str) else {
            return Err(format!("non-UTF-8 workflow file name: {}", path.display()));
        };
        files.insert(format!("{WORKFLOW_DIR}/{file_name}"));
    }
    if files.is_empty() {
        return Err(format!("{} contains no workflow files", dir.display()));
    }
    Ok(files)
}
