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
        let entry_context = format!("{path_display} workflow[{idx}]");
        let path = required_toml_string(entry, "path", &entry_context)?.to_string();
        let permissions = required_toml_string(entry, "permissions", &entry_context)?.to_string();
        let reason = required_toml_string(entry, "reason", &entry_context)?;
        if reason.len() < 16 {
            return Err(format!("{entry_context} reason is too terse"));
        }
        let review_after = required_toml_string(entry, "review_after", &entry_context)?;
        if !looks_like_iso_date(review_after) {
            return Err(format!("{entry_context} review_after must use YYYY-MM-DD"));
        }
        let actions = entry
            .get("actions")
            .and_then(toml::Value::as_array)
            .ok_or_else(|| format!("{entry_context} is missing actions array"))?;
        let mut action_set = BTreeSet::new();
        for (action_idx, action) in actions.iter().enumerate() {
            let Some(action) = action.as_str() else {
                return Err(format!(
                    "{entry_context} actions[{action_idx}] must be a string"
                ));
            };
            if action.trim().is_empty() {
                return Err(format!("{entry_context} actions[{action_idx}] is empty"));
            }
            action_set.insert(action.to_string());
        }
        if action_set.is_empty() {
            return Err(format!("{entry_context} must list at least one action"));
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
    let expected_permissions = policy_permission_set(&policy.permissions)?;
    let declared_permissions = workflow_declared_permissions(text);
    if declared_permissions != expected_permissions {
        return Err(format!(
            "{path} must declare workflow permissions `{}` (found `{}`)",
            policy.permissions,
            format_permission_set(&declared_permissions)
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

fn policy_permission_set(permissions: &str) -> Result<BTreeSet<String>, String> {
    let out: BTreeSet<String> = permissions
        .split(',')
        .map(str::trim)
        .filter(|permission| !permission.is_empty())
        .map(str::to_string)
        .collect();
    if out.is_empty() {
        return Err("workflow allowlist permission set must not be empty".to_string());
    }
    Ok(out)
}

fn workflow_declared_permissions(text: &str) -> BTreeSet<String> {
    let mut permissions = BTreeSet::new();
    let mut permissions_indent: Option<usize> = None;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = line
            .chars()
            .take_while(|ch| matches!(ch, ' ' | '\t'))
            .count();

        if let Some(block_indent) = permissions_indent {
            if indent <= block_indent {
                permissions_indent = None;
            } else {
                if looks_like_permission_entry(trimmed) {
                    permissions.insert(trimmed.to_string());
                }
                continue;
            }
        }

        if let Some(scalar) = trimmed.strip_prefix("permissions:") {
            let scalar = scalar.trim();
            if scalar.is_empty() {
                permissions_indent = Some(indent);
            } else {
                permissions.insert(scalar.to_string());
            }
            continue;
        }
    }

    permissions
}

fn looks_like_permission_entry(line: &str) -> bool {
    let Some((scope, access)) = line.split_once(':') else {
        return false;
    };
    !scope.trim().is_empty() && !access.trim().is_empty() && !line.starts_with('-')
}

fn format_permission_set(permissions: &BTreeSet<String>) -> String {
    if permissions.is_empty() {
        return "<none>".to_string();
    }
    permissions.iter().cloned().collect::<Vec<_>>().join(", ")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_permission_parser_ignores_permission_text_outside_permissions_block() {
        let text = r#"
permissions:
  id-token: write
jobs:
  note:
    runs-on: ubuntu-latest
    env:
      contents: read
    steps:
      - run: echo "note"
"#;

        let permissions = workflow_declared_permissions(text);
        assert!(!permissions.contains("contents: read"));
        assert_eq!(permissions, BTreeSet::from(["id-token: write".to_string()]));
    }

    #[test]
    fn workflow_policy_accepts_exact_multiple_permissions() -> Result<(), String> {
        let text = r#"
jobs:
  review:
    permissions:
      contents: read
      pull-requests: write
    steps:
      - uses: actions/checkout@v6
"#;

        check_workflow_text_against_policy(
            ".github/workflows/droid-pr-review.yml",
            text,
            &WorkflowPolicyEntry {
                path: ".github/workflows/droid-pr-review.yml".to_string(),
                permissions: "contents: read, pull-requests: write".to_string(),
                actions: BTreeSet::from(["actions/checkout@v6".to_string()]),
            },
        )
    }

    #[test]
    fn workflow_policy_rejects_extra_permissions() -> Result<(), String> {
        let text = r#"
permissions:
  contents: read
  pull-requests: write
  id-token: write
jobs:
  review:
    steps:
      - uses: actions/checkout@v6
"#;

        let Err(err) = check_workflow_text_against_policy(
            ".github/workflows/droid-pr-review.yml",
            text,
            &WorkflowPolicyEntry {
                path: ".github/workflows/droid-pr-review.yml".to_string(),
                permissions: "contents: read, pull-requests: write".to_string(),
                actions: BTreeSet::from(["actions/checkout@v6".to_string()]),
            },
        ) else {
            return Err("extra permissions should fail".to_string());
        };

        assert!(err.contains("id-token: write"));
        Ok(())
    }
}
