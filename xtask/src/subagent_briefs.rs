//! Offline structural checks for bounded subagent briefs.
//!
//! Briefs reference an accepted work spec; they are not work contracts,
//! schedulers, or repository authority stores.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::{parse_toml_file, read_to_string, workspace_path};

const SCHEMA: &str = "docs/schemas/bounded-subagent-brief.schema.json";
const VALID_ROOT: &str = "plans/subagent-briefs/examples";
const INVALID_ROOT: &str = "plans/subagent-briefs/fixtures/invalid";
const ACTIONS: &[&str] = &[
    "investigate",
    "challenge_plan",
    "build",
    "verify",
    "review",
    "triage_ci",
    "audit_cleanup",
];
const INVALID_FIXTURES: &[&str] = &[
    "broad-write-scope.toml",
    "case-variant-authority.toml",
    "case-variant-work-spec-authority.toml",
    "case-variant-work-spec.toml",
    "contradictory-authority.toml",
    "global-lane-authority.toml",
    "invalid-external-authority.toml",
    "invalid-github-character.toml",
    "invalid-github-consecutive-hyphens.toml",
    "invalid-github-dot-repo.toml",
    "invalid-github-dotdot-repo.toml",
    "invalid-github-leading-zero.toml",
    "invalid-github-signed-number.toml",
    "invalid-github-trailing-dot.toml",
    "invalid-github-whitespace.toml",
    "invalid-issue-authority-route.toml",
    "invalid-pr-authority-route.toml",
    "invalid-trailing-slash-authority.toml",
    "invalid-work-spec-authority.toml",
    "mismatched-work-spec.toml",
    "nested-work-spec.toml",
    "nonexistent-adr-authority.toml",
    "nonexistent-spec-authority.toml",
    "nonexistent-work-spec.toml",
    "prefix-spec-authority.toml",
    "read-only-mutation-objective.toml",
    "read-only-mutation-proof.toml",
    "read-only-mutation-stop-when.toml",
    "read-only-camelcase-tool.toml",
    "read-only-copied-objective.toml",
    "read-only-copies-objective.toml",
    "read-only-copy-objective.toml",
    "read-only-copying-objective.toml",
    "read-only-cp-objective.toml",
    "read-only-generate-objective.toml",
    "read-only-overwrite-objective.toml",
    "read-only-rm-proof.toml",
    "read-only-synonym-objective.toml",
    "read-only-touch-objective.toml",
    "read-only-touch-stop-when.toml",
    "read-only-underscored-tool.toml",
    "read-only-write-scope.toml",
    "review-missing-exact-head.toml",
    "whitespace-authority.toml",
    "whitespace-objective.toml",
    "writer-missing-admission.toml",
    "writer-missing-edit-cage.toml",
    "writer-missing-issue.toml",
    "writer-missing-work-spec.toml",
    "writer-missing-worktree.toml",
];
const REQUIRED: &[&str] = &[
    "schema",
    "work_item",
    "basis",
    "action",
    "capability",
    "objective",
    "read_scope",
    "write_scope",
    "authorities",
    "proof_obligations",
    "non_goals",
    "stop_when",
    "return_schema",
];
const FORBIDDEN: &[&str] = &[
    "active_issue",
    "active_lane",
    "current_goal",
    "default_goal",
    "priority",
    "queue",
    "rank",
    "schedule",
    "model",
    "models",
    "agent_count",
    "private_reasoning",
];

pub(crate) fn check() -> Result<(), String> {
    check_schema()?;
    let valid = toml_files(&workspace_path(VALID_ROOT))?;
    let mut actions = BTreeSet::new();
    for path in &valid {
        let value = parse_toml_file(path)?;
        let action = validate(&value, &path.display().to_string())?;
        if !actions.insert(action.to_string()) {
            return Err(format!("{VALID_ROOT} duplicates action `{action}`"));
        }
    }
    let expected: BTreeSet<String> = ACTIONS.iter().map(|value| (*value).to_string()).collect();
    if actions != expected {
        return Err(format!(
            "{VALID_ROOT} must cover exactly [{}]",
            ACTIONS.join(", ")
        ));
    }

    let invalid = toml_files(&workspace_path(INVALID_ROOT))?;
    let found: BTreeSet<String> = invalid
        .iter()
        .filter_map(|path| path.file_name()?.to_str().map(str::to_string))
        .collect();
    let registered: BTreeSet<String> = INVALID_FIXTURES
        .iter()
        .map(|name| (*name).to_string())
        .collect();
    if found != registered {
        let missing: Vec<_> = registered.difference(&found).cloned().collect();
        let unexpected: Vec<_> = found.difference(&registered).cloned().collect();
        return Err(format!(
            "{INVALID_ROOT} fixture manifest mismatch; missing [{}], unexpected [{}]",
            missing.join(", "),
            unexpected.join(", ")
        ));
    }
    for path in &invalid {
        let value = parse_toml_file(path)?;
        let error = match validate(&value, &path.display().to_string()) {
            Ok(action) => {
                return Err(format!(
                    "{} is invalid but passed as `{action}`",
                    path.display()
                ));
            }
            Err(error) => error,
        };
        let expected = invalid_expectation(path)?;
        if !error.contains(expected) {
            return Err(format!(
                "{} failed for the wrong reason: expected `{expected}`, got `{error}`",
                path.display()
            ));
        }
    }
    println!(
        "{}",
        serde_json::json!({
            "schema": "bounded-subagent-brief-check-v1",
            "status": "ok",
            "valid": valid.len(),
            "invalid": invalid.len(),
        })
    );
    Ok(())
}

fn invalid_expectation(path: &Path) -> Result<&'static str, String> {
    match path.file_name().and_then(|name| name.to_str()) {
        Some("read-only-write-scope.toml") => Ok("must have empty write_scope"),
        Some("writer-missing-issue.toml") => Ok("field `issue` must be a string"),
        Some("writer-missing-work-spec.toml") => Ok("field `work_spec` must be a string"),
        Some("writer-missing-admission.toml") => Ok("field `admission` must be a table"),
        Some("writer-missing-worktree.toml") => Ok("field `worktree` must be a string"),
        Some("writer-missing-edit-cage.toml") => Ok("requires a write_scope edit cage"),
        Some("review-missing-exact-head.toml") => Ok("requires basis.pr and basis.head_sha"),
        Some("global-lane-authority.toml") => Ok("appoints global or runtime authority"),
        Some("invalid-external-authority.toml") => Ok("must name a non-empty HTTPS resource"),
        Some("invalid-github-character.toml") => Ok("must be a GitHub issues URL"),
        Some("invalid-github-consecutive-hyphens.toml") => Ok("must be a GitHub issues URL"),
        Some("invalid-github-dot-repo.toml") => Ok("must be a GitHub issues URL"),
        Some("invalid-github-dotdot-repo.toml") => Ok("must be a GitHub pull URL"),
        Some("invalid-github-leading-zero.toml") => Ok("must be a GitHub issues URL"),
        Some("invalid-github-signed-number.toml") => Ok("must be a GitHub issues URL"),
        Some("invalid-github-trailing-dot.toml") => Ok("must be a GitHub pull URL"),
        Some("invalid-github-whitespace.toml") => Ok("must be a GitHub issues URL"),
        Some("invalid-issue-authority-route.toml") => Ok("must be a GitHub issues URL"),
        Some("invalid-pr-authority-route.toml") => Ok("must be a GitHub pull URL"),
        Some("invalid-trailing-slash-authority.toml") => {
            Ok("must resolve with canonical case to a tracked repository file")
        }
        Some("invalid-work-spec-authority.toml") => Ok("must reference one direct work spec"),
        Some("contradictory-authority.toml") => Ok("duplicates authority"),
        Some("case-variant-authority.toml") => Ok("duplicates authority"),
        Some("case-variant-work-spec-authority.toml") => {
            Ok("must resolve with canonical case to a tracked repository file")
        }
        Some("case-variant-work-spec.toml") => {
            Ok("must resolve with canonical case to a tracked repository file")
        }
        Some("broad-write-scope.toml") => Ok("must be a normalized repository-relative path"),
        Some("whitespace-authority.toml") => Ok("must not contain surrounding whitespace"),
        Some("whitespace-objective.toml") => Ok("field `objective` must not be empty"),
        Some("read-only-mutation-objective.toml") => Ok("requests mutation in objective"),
        Some("read-only-mutation-proof.toml") => Ok("requests mutation in proof_obligations[0]"),
        Some("read-only-mutation-stop-when.toml") => Ok("requests mutation in stop_when[0]"),
        Some("read-only-camelcase-tool.toml") => Ok("requests mutation in objective"),
        Some("read-only-copy-objective.toml") => Ok("requests mutation in objective"),
        Some("read-only-copied-objective.toml") => Ok("requests mutation in objective"),
        Some("read-only-copies-objective.toml") => Ok("requests mutation in objective"),
        Some("read-only-copying-objective.toml") => Ok("requests mutation in objective"),
        Some("read-only-cp-objective.toml") => Ok("requests mutation in objective"),
        Some("read-only-generate-objective.toml") => Ok("requests mutation in objective"),
        Some("read-only-overwrite-objective.toml") => Ok("requests mutation in objective"),
        Some("read-only-rm-proof.toml") => Ok("requests mutation in proof_obligations[0]"),
        Some("read-only-synonym-objective.toml") => Ok("requests mutation in objective"),
        Some("read-only-touch-objective.toml") => Ok("requests mutation in objective"),
        Some("read-only-touch-stop-when.toml") => Ok("requests mutation in stop_when[0]"),
        Some("read-only-underscored-tool.toml") => Ok("requests mutation in objective"),
        Some("nonexistent-work-spec.toml") => {
            Ok("must resolve with canonical case to a tracked repository file")
        }
        Some("nested-work-spec.toml") => Ok("must reference one direct work spec"),
        Some("nonexistent-spec-authority.toml") => Ok("does not resolve to one tracked spec"),
        Some("prefix-spec-authority.toml") => Ok("does not declare canonical identifier"),
        Some("nonexistent-adr-authority.toml") => Ok("does not resolve to one tracked adr"),
        Some("mismatched-work-spec.toml") => Ok("declares issue"),
        Some(name) => Err(format!("{INVALID_ROOT} has unregistered fixture `{name}`")),
        None => Err(format!("{} has no file name", path.display())),
    }
}

fn check_schema() -> Result<(), String> {
    let text = read_to_string(&workspace_path(SCHEMA))?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|error| format!("{SCHEMA} is not valid JSON: {error}"))?;
    if value.get("$schema").and_then(serde_json::Value::as_str)
        != Some("https://json-schema.org/draft/2020-12/schema")
    {
        return Err(format!("{SCHEMA} must use JSON Schema 2020-12"));
    }
    for pointer in [
        "/properties/objective/pattern",
        "/$defs/strings/items/pattern",
        "/$defs/non_empty_strings/items/pattern",
        "/$defs/admission/properties/worktree/pattern",
    ] {
        if value.pointer(pointer).and_then(serde_json::Value::as_str) != Some("\\S") {
            return Err(format!(
                "{SCHEMA} `{pointer}` must reject whitespace-only strings with `\\S`"
            ));
        }
    }
    if value
        .pointer("/$defs/authority/anyOf/1/pattern")
        .and_then(serde_json::Value::as_str)
        != Some("^(policy|artifact):[A-Za-z0-9_-][A-Za-z0-9._-]*(/[A-Za-z0-9_-][A-Za-z0-9._-]*)*$")
    {
        return Err(format!(
            "{SCHEMA} path-bearing authorities must reject trailing-slash directory shapes"
        ));
    }
    if value
        .pointer("/$defs/authority/anyOf/2/pattern")
        .and_then(serde_json::Value::as_str)
        != Some("^work_spec:plans/work-specs/examples/[A-Za-z0-9_-][A-Za-z0-9._-]*\\.toml$")
    {
        return Err(format!(
            "{SCHEMA} work_spec authorities must reference one direct typed example"
        ));
    }
    for pointer in [
        "/$defs/authority/anyOf/3/pattern",
        "/$defs/authority/anyOf/4/pattern",
        "/$defs/work_item/properties/issue/pattern",
        "/$defs/basis/properties/pr/pattern",
    ] {
        let pattern = value
            .pointer(pointer)
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("{SCHEMA} `{pointer}` must be a string"))?;
        if !pattern.contains("github\\.com/(?![A-Za-z0-9-]*--)") {
            return Err(format!(
                "{SCHEMA} `{pointer}` must reject adjacent GitHub owner hyphens"
            ));
        }
    }
    Ok(())
}

fn toml_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut paths = Vec::new();
    for entry in
        fs::read_dir(root).map_err(|error| format!("failed to read {}: {error}", root.display()))?
    {
        let path = entry
            .map_err(|error| format!("failed to read directory entry: {error}"))?
            .path();
        if path.extension().and_then(|value| value.to_str()) == Some("toml") {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

fn validate<'a>(value: &'a toml::Value, path: &str) -> Result<&'a str, String> {
    let table = value
        .as_table()
        .ok_or_else(|| format!("{path} must contain a TOML table"))?;
    for key in table.keys() {
        if FORBIDDEN.contains(&key.as_str()) {
            return Err(format!("{path} contains global authority field `{key}`"));
        }
        if matches!(key.as_str(), "issue" | "work_spec" | "authority") {
            return Err(format!(
                "{path} field `{key}` contradicts the bounded nested authority"
            ));
        }
        if !REQUIRED.contains(&key.as_str()) && key != "admission" {
            return Err(format!("{path} contains unknown field `{key}`"));
        }
    }
    for field in REQUIRED {
        if !table.contains_key(*field) {
            return Err(format!("{path} is missing required field `{field}`"));
        }
    }
    string(table, "schema", path, Some("bounded-subagent-brief-v1"))?;
    let action = known_string(table, "action", path, ACTIONS)?;
    let capability = known_string(table, "capability", path, &["read_only", "write"])?;
    let objective = string(table, "objective", path, None)?;
    string(
        table,
        "return_schema",
        path,
        Some("bounded-subagent-result-v1"),
    )?;

    let work_item = subtable(table, "work_item", path)?;
    only_fields(work_item, &["issue", "work_spec"], path, "work_item")?;
    let issue = string(work_item, "issue", path, None)?;
    validate_url(issue, path, "work_item.issue", "issues")?;
    let work_spec = string(work_item, "work_spec", path, None)?;
    validate_work_spec(work_spec, issue, path, "work_item.work_spec")?;

    let basis = subtable(table, "basis", path)?;
    only_fields(basis, &["base_sha", "pr", "head_sha"], path, "basis")?;
    sha(string(basis, "base_sha", path, None)?, path, "base_sha")?;
    let pr = optional_string(basis, "pr", path)?;
    let head = optional_string(basis, "head_sha", path)?;
    if pr.is_some() != head.is_some() {
        return Err(format!(
            "{path} basis.pr and basis.head_sha must be supplied together"
        ));
    }
    if let Some(value) = pr {
        validate_url(value, path, "basis.pr", "pull")?;
    }
    if let Some(value) = head {
        sha(value, path, "head_sha")?;
    }
    if matches!(action, "verify" | "review") && pr.is_none() {
        return Err(format!(
            "{path} action `{action}` requires basis.pr and basis.head_sha"
        ));
    }

    array(table, "read_scope", path, true)?;
    let write_scope = array(table, "write_scope", path, false)?;
    for value in write_scope {
        repository_relative_path(value.as_str().unwrap_or_default(), path, "write_scope")?;
    }
    distinct_authorities(array(table, "authorities", path, true)?, issue, path)?;
    let proof_obligations = array(table, "proof_obligations", path, true)?;
    array(table, "non_goals", path, true)?;
    let stop_when = array(table, "stop_when", path, true)?;

    if action == "build" {
        if capability != "write" {
            return Err(format!("{path} build capability must equal `write`"));
        }
        if write_scope.is_empty() {
            return Err(format!("{path} build requires a write_scope edit cage"));
        }
        let admission = subtable(table, "admission", path)?;
        only_fields(admission, &["state", "worktree"], path, "admission")?;
        string(admission, "state", path, Some("admitted"))?;
        string(admission, "worktree", path, None)?;
    } else {
        if capability != "read_only" {
            return Err(format!(
                "{path} read-only action `{action}` capability must equal `read_only`"
            ));
        }
        if !write_scope.is_empty() {
            return Err(format!(
                "{path} read-only action `{action}` must have empty write_scope"
            ));
        }
        if table.contains_key("admission") {
            return Err(format!(
                "{path} read-only action `{action}` must not carry admission"
            ));
        }
        reject_mutation_text(objective, path, "objective")?;
        for (index, value) in proof_obligations.iter().enumerate() {
            reject_mutation_text(
                value.as_str().unwrap_or_default(),
                path,
                &format!("proof_obligations[{index}]"),
            )?;
        }
        for (index, value) in stop_when.iter().enumerate() {
            reject_mutation_text(
                value.as_str().unwrap_or_default(),
                path,
                &format!("stop_when[{index}]"),
            )?;
        }
    }
    Ok(action)
}

fn only_fields(
    table: &toml::value::Table,
    allowed: &[&str],
    path: &str,
    context: &str,
) -> Result<(), String> {
    for key in table.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(format!("{path} {context} contains unknown field `{key}`"));
        }
    }
    Ok(())
}

fn string<'a>(
    table: &'a toml::value::Table,
    field: &str,
    path: &str,
    exact: Option<&str>,
) -> Result<&'a str, String> {
    let value = table
        .get(field)
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("{path} field `{field}` must be a string"))?;
    if value.trim().is_empty() {
        return Err(format!("{path} field `{field}` must not be empty"));
    }
    if let Some(expected) = exact
        && value != expected
    {
        return Err(format!("{path} field `{field}` must equal `{expected}`"));
    }
    Ok(value)
}

fn known_string<'a>(
    table: &'a toml::value::Table,
    field: &str,
    path: &str,
    allowed: &[&str],
) -> Result<&'a str, String> {
    let value = string(table, field, path, None)?;
    if allowed.contains(&value) {
        Ok(value)
    } else {
        Err(format!(
            "{path} field `{field}` must be one of [{}]",
            allowed.join(", ")
        ))
    }
}

fn optional_string<'a>(
    table: &'a toml::value::Table,
    field: &str,
    path: &str,
) -> Result<Option<&'a str>, String> {
    match table.get(field) {
        None => Ok(None),
        Some(value) => {
            let value = value
                .as_str()
                .ok_or_else(|| format!("{path} field `{field}` must be a string"))?;
            if value.trim().is_empty() {
                return Err(format!("{path} field `{field}` must not be empty"));
            }
            Ok(Some(value))
        }
    }
}

fn subtable<'a>(
    table: &'a toml::value::Table,
    field: &str,
    path: &str,
) -> Result<&'a toml::value::Table, String> {
    table
        .get(field)
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{path} field `{field}` must be a table"))
}

fn array<'a>(
    table: &'a toml::value::Table,
    field: &str,
    path: &str,
    non_empty: bool,
) -> Result<&'a Vec<toml::Value>, String> {
    let values = table
        .get(field)
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{path} field `{field}` must be an array"))?;
    if non_empty && values.is_empty() {
        return Err(format!("{path} field `{field}` must not be empty"));
    }
    for (index, value) in values.iter().enumerate() {
        let value = value
            .as_str()
            .ok_or_else(|| format!("{path} field `{field}[{index}]` must be a string"))?;
        if value.trim().is_empty() {
            return Err(format!("{path} field `{field}[{index}]` must not be empty"));
        }
    }
    Ok(values)
}

fn distinct_authorities(values: &[toml::Value], issue: &str, path: &str) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for value in values {
        let authority = value
            .as_str()
            .ok_or_else(|| format!("{path} authorities must contain strings"))?;
        if authority.trim() != authority {
            return Err(format!(
                "{path} authority `{authority}` must not contain surrounding whitespace"
            ));
        }
        let normalized = authority.to_ascii_lowercase();
        if normalized.starts_with("global:") {
            return Err(format!(
                "{path} authority `{authority}` appoints global or runtime authority"
            ));
        }
        if !seen.insert(normalized) {
            return Err(format!("{path} duplicates authority `{authority}`"));
        }
        validate_authority(authority, issue, path)?;
    }
    Ok(())
}

fn repository_relative_path(value: &str, path: &str, field: &str) -> Result<(), String> {
    let trimmed = value.strip_suffix('/').unwrap_or(value);
    let valid = !trimmed.is_empty()
        && trimmed != "."
        && value.trim() == value
        && !value.contains(['\\', '*', '?', ':'])
        && trimmed.split('/').all(|component| {
            !component.is_empty()
                && component != "."
                && component != ".."
                && !component.starts_with('.')
                && component
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || "._-".contains(character))
        });
    if valid {
        Ok(())
    } else {
        Err(format!(
            "{path} field `{field}` value `{value}` must be a normalized repository-relative path"
        ))
    }
}

fn validate_authority(authority: &str, issue: &str, path: &str) -> Result<(), String> {
    let (kind, value) = authority.split_once(':').ok_or_else(|| {
        format!("{path} authority `{authority}` must use a typed authority grammar")
    })?;
    match kind {
        "spec" | "adr" => {
            if value.is_empty()
                || !value.starts_with(|character: char| character.is_ascii_uppercase())
                || !value.chars().all(|character| {
                    character.is_ascii_uppercase()
                        || character.is_ascii_digit()
                        || "._-".contains(character)
                })
            {
                return Err(format!(
                    "{path} authority `{authority}` has an invalid identifier"
                ));
            }
            resolve_document_authority(kind, value, path)?;
        }
        "policy" | "artifact" => {
            repository_relative_path(value, path, "authority")?;
            tracked_repository_file(value, path, "authority")?;
        }
        "work_spec" => validate_work_spec(value, issue, path, "authority")?,
        "issue" | "pr" => {
            validate_url(
                value,
                path,
                "authority",
                if kind == "issue" { "issues" } else { "pull" },
            )?;
        }
        "external"
            if value.strip_prefix("https://").is_some_and(|resource| {
                !resource.is_empty() && !resource.chars().any(char::is_whitespace)
            }) => {}
        "external" => {
            return Err(format!(
                "{path} authority `{authority}` must name a non-empty HTTPS resource"
            ));
        }
        _ => {
            return Err(format!(
                "{path} authority `{authority}` must use a typed authority grammar"
            ));
        }
    }
    Ok(())
}

fn tracked_repository_file(value: &str, path: &str, field: &str) -> Result<(), String> {
    let output = std::process::Command::new("git")
        .args(["ls-files", "--error-unmatch", "--", value])
        .current_dir(workspace_path(""))
        .output()
        .map_err(|error| format!("{path} failed to resolve {field} `{value}` with git: {error}"))?;
    let stdout = String::from_utf8(output.stdout).map_err(|error| {
        format!("{path} git output for {field} `{value}` is not UTF-8: {error}")
    })?;
    let exact = output.status.success() && stdout.lines().any(|tracked| tracked == value);
    if exact && workspace_path(value).is_file() {
        Ok(())
    } else {
        Err(format!(
            "{path} {field} `{value}` must resolve with canonical case to a tracked repository file"
        ))
    }
}

fn resolve_document_authority(kind: &str, identifier: &str, path: &str) -> Result<(), String> {
    let root = if kind == "spec" {
        "docs/specs"
    } else {
        "docs/adr"
    };
    let prefix = format!("{root}/{identifier}-");
    let output = std::process::Command::new("git")
        .args(["ls-files", "--", root])
        .current_dir(workspace_path(""))
        .output()
        .map_err(|error| format!("{path} failed to resolve {kind} `{identifier}`: {error}"))?;
    let stdout = String::from_utf8(output.stdout).map_err(|error| {
        format!("{path} git output for {kind} `{identifier}` is not UTF-8: {error}")
    })?;
    let matches: Vec<&str> = stdout
        .lines()
        .filter(|candidate| {
            candidate.starts_with(&prefix)
                && candidate.ends_with(".md")
                && !candidate[prefix.len()..].contains('/')
        })
        .collect();
    if !output.status.success() || matches.len() != 1 || !workspace_path(matches[0]).is_file() {
        Err(format!(
            "{path} authority `{kind}:{identifier}` does not resolve to one tracked {kind} document"
        ))
    } else {
        let document = read_to_string(&workspace_path(matches[0]))?;
        let declared = document
            .lines()
            .next()
            .and_then(|heading| heading.strip_prefix("# "))
            .and_then(|heading| heading.split_whitespace().next())
            .map(|token| token.trim_end_matches(':'));
        if declared == Some(identifier) {
            Ok(())
        } else {
            Err(format!(
                "{path} authority `{kind}:{identifier}` resolved document does not declare canonical identifier `{identifier}` in its heading"
            ))
        }
    }
}

fn validate_work_spec(reference: &str, issue: &str, path: &str, field: &str) -> Result<(), String> {
    repository_relative_path(reference, path, field)?;
    let name = reference
        .strip_prefix("plans/work-specs/examples/")
        .and_then(|value| value.strip_suffix(".toml"));
    if name.is_none_or(|value| value.is_empty() || value.contains('/')) {
        return Err(format!(
            "{path} work_item.work_spec must reference one direct work spec under plans/work-specs/examples/*.toml"
        ));
    }
    tracked_repository_file(reference, path, field)?;
    let work_spec_path = workspace_path(reference);
    if !work_spec_path.is_file() {
        return Err(format!(
            "{path} work_item.work_spec `{reference}` does not resolve to an accepted work spec"
        ));
    }
    let work_spec = parse_toml_file(&work_spec_path)?;
    crate::work_specs::validate_work_spec(&work_spec, &work_spec_path.display().to_string())?;
    let declared_issue = work_spec
        .get("issue")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| {
            format!(
                "{} field `issue` must be a string",
                work_spec_path.display()
            )
        })?;
    if declared_issue != issue {
        return Err(format!(
            "{path} work_item.work_spec `{reference}` declares issue `{declared_issue}`, not `{issue}`"
        ));
    }
    Ok(())
}

fn reject_mutation_text(value: &str, path: &str, field: &str) -> Result<(), String> {
    // Closed vocabulary: directive prose may use arbitrary nouns, but only these
    // normalized operation forms can appoint repository mutation.
    const MUTATION_OPERATION_FORMS: &[&str] = &[
        "add",
        "added",
        "adding",
        "adds",
        "append",
        "appended",
        "appending",
        "appends",
        "change",
        "changed",
        "changes",
        "changing",
        "commit",
        "commits",
        "committed",
        "committing",
        "copy",
        "copied",
        "copies",
        "copying",
        "cp",
        "create",
        "created",
        "creates",
        "creating",
        "delete",
        "deleted",
        "deletes",
        "deleting",
        "deploy",
        "deployed",
        "deploying",
        "deploys",
        "edit",
        "edited",
        "editing",
        "edits",
        "fix",
        "fixed",
        "fixes",
        "fixing",
        "generate",
        "generated",
        "generates",
        "generating",
        "implement",
        "implemented",
        "implementing",
        "implements",
        "merge",
        "merged",
        "merges",
        "merging",
        "move",
        "moved",
        "moves",
        "moving",
        "overwrite",
        "overwritten",
        "overwrites",
        "overwriting",
        "overwrote",
        "modify",
        "modified",
        "modifies",
        "modifying",
        "patch",
        "patched",
        "patches",
        "patching",
        "push",
        "pushed",
        "pushes",
        "pushing",
        "publish",
        "published",
        "publishes",
        "publishing",
        "refactor",
        "refactored",
        "refactoring",
        "refactors",
        "rewrite",
        "rewrites",
        "rewriting",
        "rewritten",
        "rewrote",
        "drop",
        "dropped",
        "dropping",
        "drops",
        "truncate",
        "truncated",
        "truncates",
        "truncating",
        "insert",
        "inserted",
        "inserting",
        "inserts",
        "replace",
        "replaced",
        "replaces",
        "replacing",
        "swap",
        "swapped",
        "swapping",
        "swaps",
        "split",
        "splits",
        "splitting",
        "consolidate",
        "consolidated",
        "consolidates",
        "consolidating",
        "remove",
        "removed",
        "removes",
        "removing",
        "rename",
        "renamed",
        "renames",
        "renaming",
        "reply",
        "replied",
        "replies",
        "replying",
        "resolve",
        "resolved",
        "resolves",
        "resolving",
        "retry",
        "retried",
        "retries",
        "retrying",
        "rerun",
        "rerunning",
        "reruns",
        "stage",
        "staged",
        "stages",
        "staging",
        "tag",
        "tagged",
        "tagging",
        "tags",
        "touch",
        "rm",
        "update",
        "updated",
        "updates",
        "updating",
        "write",
        "writes",
        "writing",
        "wrote",
    ];
    let words = normalized_operation_words(value);
    if let Some(word) = words
        .iter()
        .find(|word| MUTATION_OPERATION_FORMS.contains(&word.as_str()))
    {
        Err(format!(
            "{path} read-only action requests mutation in {field} via `{word}`"
        ))
    } else {
        Ok(())
    }
}

fn normalized_operation_words(value: &str) -> Vec<String> {
    let mut normalized = String::with_capacity(value.len());
    let mut previous_is_lower_or_digit = false;
    for character in value.chars() {
        if character.is_ascii_uppercase() && previous_is_lower_or_digit {
            normalized.push(' ');
        }
        if character.is_ascii_alphanumeric() {
            normalized.push(character.to_ascii_lowercase());
            previous_is_lower_or_digit =
                character.is_ascii_lowercase() || character.is_ascii_digit();
        } else {
            normalized.push(' ');
            previous_is_lower_or_digit = false;
        }
    }
    normalized.split_whitespace().map(str::to_string).collect()
}

fn sha(value: &str, path: &str, field: &str) -> Result<(), String> {
    if value.len() != 40
        || !value
            .chars()
            .all(|character| character.is_ascii_digit() || matches!(character, 'a'..='f'))
    {
        return Err(format!(
            "{path} basis.{field} must be a full lowercase hexadecimal SHA"
        ));
    }
    Ok(())
}

fn validate_url(url: &str, path: &str, field: &str, segment: &str) -> Result<(), String> {
    let parts: Vec<&str> = url.split('/').collect();
    if parts.len() != 7
        || parts[0] != "https:"
        || !parts[1].is_empty()
        || parts[2] != "github.com"
        || !valid_github_owner(parts[3])
        || !valid_github_repo(parts[4])
        || parts[5] != segment
        || !canonical_github_route_number(parts[6])
    {
        return Err(format!(
            "{path} field `{field}` must be a GitHub {segment} URL"
        ));
    }
    Ok(())
}

fn canonical_github_route_number(value: &str) -> bool {
    let mut characters = value.chars();
    characters
        .next()
        .is_some_and(|character| matches!(character, '1'..='9'))
        && characters.all(|character| character.is_ascii_digit())
}

fn valid_github_owner(value: &str) -> bool {
    value.len() <= 39
        && value.starts_with(|character: char| character.is_ascii_alphanumeric())
        && value.ends_with(|character: char| character.is_ascii_alphanumeric())
        && !value.contains("--")
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
}

fn valid_github_repo(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value != "."
        && value != ".."
        && !value.ends_with('.')
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
}

#[cfg(test)]
mod tests {
    use super::{check_schema, validate, validate_url};

    #[test]
    fn schema_rejects_whitespace_only_generic_strings() -> Result<(), String> {
        check_schema()
    }

    #[test]
    fn rejects_global_lane_authority() -> Result<(), String> {
        rejects(
            &valid().replace("spec:UNSAFE-REVIEW-SPEC-0044", "global:active-lane"),
            "global or runtime authority",
        )
    }

    #[test]
    fn rejects_read_only_write_scope() -> Result<(), String> {
        rejects(
            &valid().replace("write_scope = []", "write_scope = [\"docs/\"]"),
            "empty write_scope",
        )
    }

    #[test]
    fn rejects_exact_head_without_identity() -> Result<(), String> {
        rejects(
            &valid().replace("action = \"investigate\"", "action = \"review\""),
            "requires basis.pr and basis.head_sha",
        )
    }

    #[test]
    fn rejects_nested_work_spec_reference() -> Result<(), String> {
        rejects(
            &valid().replace(
                "plans/work-specs/examples/UNSAFE-REVIEW-WORK-1924.toml",
                "plans/work-specs/examples/nested/UNSAFE-REVIEW-WORK-1924.toml",
            ),
            "must reference one direct work spec",
        )
    }

    #[test]
    fn resolves_spec_and_adr_authorities() -> Result<(), String> {
        for authority in ["spec:UNSAFE-REVIEW-SPEC-0044", "adr:UNSAFE-REVIEW-ADR-0001"] {
            let source = valid().replace("spec:UNSAFE-REVIEW-SPEC-0044", authority);
            let value: toml::Value =
                toml::from_str(&source).map_err(|error| format!("test TOML: {error}"))?;
            validate(&value, "test.toml")?;
        }
        Ok(())
    }

    #[test]
    fn rejects_spec_index_prefix_but_accepts_canonical_spec_heading() -> Result<(), String> {
        let canonical: toml::Value =
            toml::from_str(&valid()).map_err(|error| format!("test TOML: {error}"))?;
        validate(&canonical, "canonical-spec.toml")?;
        rejects(
            &valid().replace(
                "spec:UNSAFE-REVIEW-SPEC-0044",
                "spec:UNSAFE-REVIEW-SPEC-START",
            ),
            "does not declare canonical identifier",
        )
    }

    #[test]
    fn rejects_nonexistent_document_authorities() -> Result<(), String> {
        for (authority, expected) in [
            ("spec:UNSAFE-REVIEW-SPEC-9999", "tracked spec document"),
            ("adr:UNSAFE-REVIEW-ADR-9999", "tracked adr document"),
        ] {
            rejects(
                &valid().replace("spec:UNSAFE-REVIEW-SPEC-0044", authority),
                expected,
            )?;
        }
        Ok(())
    }

    #[test]
    fn rejects_invalid_github_identity_segments() -> Result<(), String> {
        for issue in [
            "https://github.com/Effortless Metrics/unsafe-review-swarm/issues/1924",
            "https://github.com/Effortless@Metrics/unsafe-review-swarm/issues/1924",
            "https://github.com/EffortlessMetrics/unsafe review/issues/1924",
            "https://github.com/EffortlessMetrics/unsafe@review/issues/1924",
        ] {
            rejects(
                &valid().replace(
                    "https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1924",
                    issue,
                ),
                "must be a GitHub issues URL",
            )?;
        }
        Ok(())
    }

    #[test]
    fn rejects_github_owner_adjacent_hyphens() -> Result<(), String> {
        rejects(
            &valid().replace(
                "https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1924",
                "https://github.com/acme--team/unsafe-review-swarm/issues/1924",
            ),
            "must be a GitHub issues URL",
        )
    }

    #[test]
    fn accepts_canonical_github_route_numbers() -> Result<(), String> {
        for number in ["1", "42", "184467440737095516160"] {
            validate_url(
                &format!(
                    "https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/{number}"
                ),
                "test.toml",
                "work_item.issue",
                "issues",
            )?;
        }
        Ok(())
    }

    #[test]
    fn rejects_noncanonical_github_route_numbers() -> Result<(), String> {
        for number in ["+1", "01"] {
            if validate_url(
                &format!(
                    "https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/{number}"
                ),
                "test.toml",
                "work_item.issue",
                "issues",
            )
            .is_ok()
            {
                return Err(format!("noncanonical route number `{number}` passed"));
            }
        }
        Ok(())
    }

    #[test]
    fn rejects_github_repository_dot_segments() -> Result<(), String> {
        for (repository, route) in [(".", "issues"), ("..", "pull")] {
            if validate_url(
                &format!("https://github.com/EffortlessMetrics/{repository}/{route}/1"),
                "test.toml",
                "authority",
                route,
            )
            .is_ok()
            {
                return Err(format!("URI dot segment `{repository}` passed"));
            }
        }
        Ok(())
    }

    #[test]
    fn rejects_github_repository_trailing_dot() -> Result<(), String> {
        if validate_url(
            "https://github.com/EffortlessMetrics/unsafe-review-swarm./pull/1",
            "test.toml",
            "authority",
            "pull",
        )
        .is_ok()
        {
            return Err("GitHub repository with trailing dot passed".to_string());
        }
        Ok(())
    }

    #[test]
    fn rejects_read_only_mutation_text() -> Result<(), String> {
        rejects(
            &valid().replace("Answer one bounded question.", "Edit one bounded file."),
            "requests mutation in objective",
        )
    }

    #[test]
    fn rejects_read_only_mutation_proof() -> Result<(), String> {
        rejects(
            &valid().replace("Return exact paths.", "Commit the inspected result."),
            "requests mutation in proof_obligations[0]",
        )
    }

    #[test]
    fn rejects_common_repository_mutation_terms() -> Result<(), String> {
        for term in [
            "add",
            "append",
            "stage",
            "rename",
            "move",
            "overwrite",
            "publish",
            "deploy",
            "tag",
            "refactor",
            "rewrite",
            "drop",
            "truncate",
            "insert",
            "replace",
            "swap",
            "split",
            "consolidate",
        ] {
            rejects(
                &valid().replace(
                    "Answer one bounded question.",
                    &format!("{term} one bounded artifact."),
                ),
                "requests mutation in objective",
            )?;
        }
        Ok(())
    }

    #[test]
    fn rejects_empty_external_authority() -> Result<(), String> {
        rejects(
            &valid().replace("spec:UNSAFE-REVIEW-SPEC-0044", "external:https://"),
            "must name a non-empty HTTPS resource",
        )
    }

    #[test]
    fn rejects_underscored_mutation_tool_name() -> Result<(), String> {
        rejects(
            &valid().replace(
                "Answer one bounded question.",
                "Use apply_patch on AGENTS.md.",
            ),
            "requests mutation in objective",
        )
    }

    #[test]
    fn rejects_camelcase_mutation_tool_name() -> Result<(), String> {
        rejects(
            &valid().replace(
                "Answer one bounded question.",
                "Use applyPatch on AGENTS.md.",
            ),
            "requests mutation in objective",
        )
    }

    #[test]
    fn rejects_generate_operation_forms() -> Result<(), String> {
        for term in ["generate", "generated", "generates", "generating"] {
            rejects(
                &valid().replace(
                    "Answer one bounded question.",
                    &format!("{term} one bounded artifact."),
                ),
                "requests mutation in objective",
            )?;
        }
        Ok(())
    }

    #[test]
    fn rejects_copy_operation_forms() -> Result<(), String> {
        for term in ["copy", "copied", "copies", "copying", "cp"] {
            rejects(
                &valid().replace(
                    "Answer one bounded question.",
                    &format!("{term} AGENTS.md to docs/backup.md."),
                ),
                "requests mutation in objective",
            )?;
        }
        Ok(())
    }

    #[test]
    fn rejects_shell_mutation_commands_in_each_directive_field() -> Result<(), String> {
        for (original, replacement, expected) in [
            (
                "Answer one bounded question.",
                "Run touch docs/new.md.",
                "requests mutation in objective",
            ),
            (
                "Return exact paths.",
                "Run rm -f docs/old.md.",
                "requests mutation in proof_obligations[0]",
            ),
            (
                "The question is answered.",
                "Stop after touch docs/new.md.",
                "requests mutation in stop_when[0]",
            ),
        ] {
            rejects(&valid().replace(original, replacement), expected)?;
        }
        Ok(())
    }

    #[test]
    fn rejects_case_variant_path_authorities() -> Result<(), String> {
        rejects(
            &valid().replace(
                "spec:UNSAFE-REVIEW-SPEC-0044",
                "artifact:docs/contributing/AGENT-ORCHESTRATION.md\", \"artifact:docs/contributing/agent-orchestration.md",
            ),
            "duplicates authority",
        )
    }

    #[test]
    fn accepts_canonical_tracked_path_authorities() -> Result<(), String> {
        let source = valid().replace(
            "spec:UNSAFE-REVIEW-SPEC-0044",
            "artifact:docs/contributing/AGENT-ORCHESTRATION.md\", \"work_spec:plans/work-specs/examples/UNSAFE-REVIEW-WORK-1924.toml",
        );
        let value: toml::Value =
            toml::from_str(&source).map_err(|error| format!("test TOML: {error}"))?;
        validate(&value, "test.toml").map(|_| ())
    }

    #[test]
    fn rejects_noncanonical_tracked_path_spelling() -> Result<(), String> {
        rejects(
            &valid().replace(
                "spec:UNSAFE-REVIEW-SPEC-0044",
                "artifact:docs/contributing/agent-orchestration.md",
            ),
            "must resolve with canonical case to a tracked repository file",
        )
    }

    #[test]
    fn rejects_mutation_in_stop_condition() -> Result<(), String> {
        rejects(
            &valid().replace(
                "The question is answered.",
                "Stop after committing the patch.",
            ),
            "requests mutation in stop_when[0]",
        )
    }

    #[test]
    fn rejects_overwritten_inflection() -> Result<(), String> {
        rejects(
            &valid().replace(
                "Return exact paths.",
                "Confirm the validator is overwritten.",
            ),
            "requests mutation in proof_obligations[0]",
        )
    }

    #[test]
    fn rejects_broad_write_scope() -> Result<(), String> {
        let source = valid()
            .replace("action = \"investigate\"", "action = \"build\"")
            .replace("capability = \"read_only\"", "capability = \"write\"")
            .replace("write_scope = []", "write_scope = [\".\"]")
            + "\n[admission]\nstate = \"admitted\"\nworktree = \"E:/worktree\"\n";
        rejects(&source, "normalized repository-relative path")
    }

    #[test]
    fn rejects_whitespace_obscured_authority() -> Result<(), String> {
        rejects(
            &valid().replace(
                "spec:UNSAFE-REVIEW-SPEC-0044",
                " spec:UNSAFE-REVIEW-SPEC-0044",
            ),
            "surrounding whitespace",
        )
    }

    fn rejects(source: &str, expected: &str) -> Result<(), String> {
        let value: toml::Value =
            toml::from_str(source).map_err(|error| format!("test TOML: {error}"))?;
        let error = match validate(&value, "test.toml") {
            Ok(action) => return Err(format!("invalid `{action}` brief passed")),
            Err(error) => error,
        };
        if !error.contains(expected) {
            return Err(format!("unexpected error: {error}"));
        }
        Ok(())
    }

    fn valid() -> String {
        r#"schema = "bounded-subagent-brief-v1"
action = "investigate"
capability = "read_only"
objective = "Answer one bounded question."
read_scope = ["docs/specs/UNSAFE-REVIEW-SPEC-0044-issue-linked-work-specs.md"]
write_scope = []
authorities = ["spec:UNSAFE-REVIEW-SPEC-0044"]
proof_obligations = ["Return exact paths."]
non_goals = ["Do not select another issue."]
stop_when = ["The question is answered."]
return_schema = "bounded-subagent-result-v1"

[work_item]
issue = "https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1924"
work_spec = "plans/work-specs/examples/UNSAFE-REVIEW-WORK-1924.toml"

[basis]
base_sha = "0f306ea0b6737c13df7abb97bdebb819eaeffdc7"
"#
        .to_string()
    }
}
