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
    "contradictory-authority.toml",
    "global-lane-authority.toml",
    "mismatched-work-spec.toml",
    "nonexistent-work-spec.toml",
    "read-only-mutation-objective.toml",
    "read-only-mutation-proof.toml",
    "read-only-mutation-stop-when.toml",
    "read-only-overwrite-objective.toml",
    "read-only-write-scope.toml",
    "review-missing-exact-head.toml",
    "whitespace-authority.toml",
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
        Some("contradictory-authority.toml") => Ok("duplicates authority"),
        Some("broad-write-scope.toml") => Ok("must be a normalized repository-relative path"),
        Some("whitespace-authority.toml") => Ok("must not contain surrounding whitespace"),
        Some("read-only-mutation-objective.toml") => Ok("requests mutation in objective"),
        Some("read-only-mutation-proof.toml") => Ok("requests mutation in proof_obligations[0]"),
        Some("read-only-mutation-stop-when.toml") => Ok("requests mutation in stop_when[0]"),
        Some("read-only-overwrite-objective.toml") => Ok("requests mutation in objective"),
        Some("nonexistent-work-spec.toml") => Ok("does not resolve to an accepted work spec"),
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
    validate_work_spec(work_spec, issue, path)?;

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
    distinct_authorities(array(table, "authorities", path, true)?, path)?;
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

fn distinct_authorities(values: &[toml::Value], path: &str) -> Result<(), String> {
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
        validate_authority(authority, path)?;
        if !seen.insert(normalized) {
            return Err(format!("{path} duplicates authority `{authority}`"));
        }
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

fn validate_authority(authority: &str, path: &str) -> Result<(), String> {
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
        }
        "policy" | "work_spec" | "artifact" => {
            repository_relative_path(value, path, "authority")?;
        }
        "issue" | "pr" => {
            validate_url(
                value,
                path,
                "authority",
                if kind == "issue" { "issues" } else { "pull" },
            )?;
        }
        "external" if value.starts_with("https://") && !value.chars().any(char::is_whitespace) => {}
        _ => {
            return Err(format!(
                "{path} authority `{authority}` must use a typed authority grammar"
            ));
        }
    }
    Ok(())
}

fn validate_work_spec(reference: &str, issue: &str, path: &str) -> Result<(), String> {
    repository_relative_path(reference, path, "work_item.work_spec")?;
    if !reference.starts_with("plans/work-specs/examples/") || !reference.ends_with(".toml") {
        return Err(format!(
            "{path} work_item.work_spec must reference an accepted work spec under plans/work-specs/examples"
        ));
    }
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
    const MUTATION_WORDS: &[&str] = &[
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
        "update",
        "updated",
        "updates",
        "updating",
        "write",
        "writes",
        "writing",
        "wrote",
    ];
    if let Some(word) = value
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .map(str::to_ascii_lowercase)
        .find(|word| MUTATION_WORDS.contains(&word.as_str()))
    {
        Err(format!(
            "{path} read-only action requests mutation in {field} via `{word}`"
        ))
    } else {
        Ok(())
    }
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
        || parts[3].is_empty()
        || parts[4].is_empty()
        || parts[5] != segment
        || parts[6]
            .parse::<u64>()
            .ok()
            .filter(|number| *number > 0)
            .is_none()
    {
        return Err(format!(
            "{path} field `{field}` must be a GitHub {segment} URL"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate;

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
