//! Offline structural checks for bounded subagent results.
//!
//! Results reference an accepted work spec; they are not telemetry, voting,
//! or scheduler surfaces. Evidence identity, contradiction preservation,
//! and overflow budgeting are validated offline.

#![allow(dead_code, reason = "advisory result corpus for #1926")]
#![allow(clippy::unwrap_used, reason = "test fixtures for #1926")]
#![allow(clippy::expect_used, reason = "test fixtures for #1926")]
#![allow(clippy::unwrap_in_result, reason = "test fixtures for #1926")]

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::{parse_toml_file, read_to_string, workspace_path};

const SCHEMA: &str = "docs/schemas/bounded-subagent-result.schema.json";
const VALID_ROOT: &str = "plans/subagent-results/examples";
const INVALID_ROOT: &str = "plans/subagent-results/fixtures/invalid";
const ACTIONS: &[&str] = &[
    "investigate",
    "challenge_plan",
    "build",
    "verify",
    "review",
    "triage_ci",
    "audit_cleanup",
];
const VERDICTS: &[&str] = &["clear", "revise", "blocked", "not_proven"];
const INVALID_FIXTURES: &[&str] = &[
    "builder-self-proof.toml",
    "contradiction-missing-evidence.toml",
    "overflow-mismatch.toml",
    "overflow-missing-refs.toml",
    "read-only-with-changes.toml",
    "review-missing-head.toml",
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
    "confidence",
    "votes",
    "voting",
    "database",
    "transcript",
    "telemetry",
    "scheduler",
];
const REQUIRED: &[&str] = &[
    "schema",
    "work_item",
    "basis",
    "action",
    "verdict",
    "summary",
];
const OPTIONAL: &[&str] = &[
    "findings",
    "changes",
    "proof",
    "contradictions",
    "uncertainty",
    "recommended_next_action",
    "overflow",
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
        // Ensure evidence references are followable without prior chat.
        check_evidence_followable(&value, &path.display().to_string())?;
    }
    let expected: BTreeSet<String> = ACTIONS.iter().map(|v| (*v).to_string()).collect();
    if actions != expected {
        return Err(format!(
            "{VALID_ROOT} must cover exactly [{}]",
            ACTIONS.join(", ")
        ));
    }

    let invalid = toml_files(&workspace_path(INVALID_ROOT))?;
    let found: BTreeSet<String> = invalid
        .iter()
        .filter_map(|p| p.file_name()?.to_str().map(str::to_string))
        .collect();
    let registered: BTreeSet<String> = INVALID_FIXTURES.iter().map(|n| (*n).to_string()).collect();
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
            Err(e) => e,
        };
        let expected = invalid_expectation(path)?;
        if !error.contains(expected) {
            return Err(format!(
                "{} failed for wrong reason: expected `{expected}`, got `{error}`",
                path.display()
            ));
        }
    }

    // Reordered equivalent inputs must not change semantic meaning.
    check_reordered_equivalence()?;

    println!(
        "{}",
        serde_json::json!({
            "schema": "bounded-subagent-result-check-v1",
            "status": "ok",
            "valid": valid.len(),
            "invalid": invalid.len(),
        })
    );
    Ok(())
}

fn invalid_expectation(path: &Path) -> Result<&'static str, String> {
    match path.file_name().and_then(|n| n.to_str()) {
        Some("review-missing-head.toml") => Ok("requires basis.pr and basis.head_sha"),
        Some("read-only-with-changes.toml") => Ok("must have empty changes"),
        Some("builder-self-proof.toml") => Ok("builder self-green"),
        Some("contradiction-missing-evidence.toml") => Ok("evidence_b"),
        Some("overflow-mismatch.toml") => Ok("selected + omitted == total"),
        Some("overflow-missing-refs.toml") => Ok("overflow.refs must not be empty"),
        Some(name) => Err(format!("{INVALID_ROOT} has unregistered fixture `{name}`")),
        None => Err(format!("{} has no file name", path.display())),
    }
}

fn check_schema() -> Result<(), String> {
    let text = read_to_string(&workspace_path(SCHEMA))?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("{SCHEMA} is not valid JSON: {e}"))?;
    if value.get("$schema").and_then(serde_json::Value::as_str)
        != Some("https://json-schema.org/draft/2020-12/schema")
    {
        return Err(format!("{SCHEMA} must use JSON Schema 2020-12"));
    }
    for pointer in [
        "/properties/summary/pattern",
        "/$defs/finding/properties/claim/pattern",
        "/$defs/contradiction/properties/claim/pattern",
        "/$defs/uncertainty/properties/items/items/pattern",
        "/$defs/recommended_next_action/properties/action/pattern",
        "/$defs/overflow/properties/refs/items/pattern",
    ] {
        if value.pointer(pointer).and_then(serde_json::Value::as_str) != Some("\\S") {
            return Err(format!(
                "{SCHEMA} `{pointer}` must reject whitespace-only strings with `\\S`"
            ));
        }
    }
    for pointer in [
        "/$defs/work_item/properties/issue/pattern",
        "/$defs/basis/properties/pr/pattern",
    ] {
        let pat = value
            .pointer(pointer)
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("{SCHEMA} `{pointer}` must be a string"))?;
        if !pat.contains("github\\.com/(?![A-Za-z0-9-]*--)") {
            return Err(format!(
                "{SCHEMA} `{pointer}` must reject adjacent GitHub owner hyphens"
            ));
        }
    }
    if value
        .pointer("/$defs/repository_relative_path/pattern")
        .and_then(serde_json::Value::as_str)
        != Some("^[A-Za-z0-9_-][A-Za-z0-9._-]*(/[A-Za-z0-9_-][A-Za-z0-9._-]*)*/?$")
    {
        return Err(format!(
            "{SCHEMA} repository_relative_path must reject trailing-slash directory shapes"
        ));
    }
    Ok(())
}

fn toml_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut paths = Vec::new();
    for entry in
        fs::read_dir(root).map_err(|e| format!("failed to read {}: {e}", root.display()))?
    {
        let path = entry
            .map_err(|e| format!("failed to read directory entry: {e}"))?
            .path();
        if path.extension().and_then(|v| v.to_str()) == Some("toml") {
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
            return Err(format!("{path} contains forbidden field `{key}`"));
        }
        if !REQUIRED.contains(&key.as_str()) && !OPTIONAL.contains(&key.as_str()) {
            return Err(format!("{path} contains unknown field `{key}`"));
        }
    }
    for field in REQUIRED {
        if !table.contains_key(*field) {
            return Err(format!("{path} is missing required field `{field}`"));
        }
    }
    string(table, "schema", path, Some("bounded-subagent-result-v1"))?;
    let action = known_string(table, "action", path, ACTIONS)?;
    let verdict = known_string(table, "verdict", path, VERDICTS)?;
    let _ = verdict;
    let summary = string(table, "summary", path, None)?;
    if summary.len() > 400 {
        return Err(format!(
            "{path} field `summary` exceeds bounded 400 byte budget ({} bytes)",
            summary.len()
        ));
    }

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
    if let Some(v) = pr {
        validate_url(v, path, "basis.pr", "pull")?;
    }
    if let Some(v) = head {
        sha(v, path, "head_sha")?;
    }
    if matches!(action, "verify" | "review") && pr.is_none() {
        return Err(format!(
            "{path} action `{action}` requires basis.pr and basis.head_sha"
        ));
    }

    // findings
    if let Some(arr) = table.get("findings") {
        let findings = arr
            .as_array()
            .ok_or_else(|| format!("{path} field `findings` must be an array"))?;
        for (i, item) in findings.iter().enumerate() {
            let t = item
                .as_table()
                .ok_or_else(|| format!("{path} findings[{i}] must be a table"))?;
            only_fields(
                t,
                &["severity", "claim", "evidence"],
                path,
                &format!("findings[{i}]"),
            )?;
            let severity = string(t, "severity", path, None)?;
            if !["P0", "P1", "P2", "P3"].contains(&severity) {
                return Err(format!(
                    "{path} findings[{i}].severity must be one of [P0, P1, P2, P3]"
                ));
            }
            let claim = string(t, "claim", path, None)?;
            if claim.trim().is_empty() {
                return Err(format!("{path} findings[{i}].claim must not be empty"));
            }
            let evidence = t
                .get("evidence")
                .and_then(toml::Value::as_array)
                .ok_or_else(|| format!("{path} findings[{i}].evidence must be an array"))?;
            if evidence.is_empty() {
                return Err(format!("{path} findings[{i}].evidence must not be empty"));
            }
            for (j, ev) in evidence.iter().enumerate() {
                let s = ev.as_str().ok_or_else(|| {
                    format!("{path} findings[{i}].evidence[{j}] must be a string")
                })?;
                if s.trim().is_empty() || s.trim() != s {
                    return Err(format!(
                        "{path} findings[{i}].evidence[{j}] must be non-empty without surrounding whitespace"
                    ));
                }
            }
        }
    }

    // changes
    if let Some(arr) = table.get("changes") {
        let changes = arr
            .as_array()
            .ok_or_else(|| format!("{path} field `changes` must be an array"))?;
        for (i, v) in changes.iter().enumerate() {
            let s = v
                .as_str()
                .ok_or_else(|| format!("{path} changes[{i}] must be a string"))?;
            repository_relative_path(s, path, "changes")?;
        }
        if action != "build" && !changes.is_empty() {
            return Err(format!(
                "{path} read-only action `{action}` must have empty changes"
            ));
        }
    } else if action != "build" {
        // absent changes is okay for read-only (treated as empty)
    }

    // proof
    if let Some(arr) = table.get("proof") {
        let proof = arr
            .as_array()
            .ok_or_else(|| format!("{path} field `proof` must be an array"))?;
        for (i, v) in proof.iter().enumerate() {
            let s = v
                .as_str()
                .ok_or_else(|| format!("{path} proof[{i}] must be a string"))?;
            if s.trim().is_empty() || s.trim() != s {
                return Err(format!(
                    "{path} proof[{i}] must be non-empty without surrounding whitespace"
                ));
            }
            if is_builder_self_green(s) {
                return Err(format!(
                    "{path} proof[{i}] builder self-green cannot be encoded as independent proof"
                ));
            }
        }
    }

    // contradictions
    if let Some(arr) = table.get("contradictions") {
        let contradictions = arr
            .as_array()
            .ok_or_else(|| format!("{path} field `contradictions` must be an array"))?;
        for (i, item) in contradictions.iter().enumerate() {
            let t = item
                .as_table()
                .ok_or_else(|| format!("{path} contradictions[{i}] must be a table"))?;
            only_fields(
                t,
                &["claim", "evidence_a", "evidence_b"],
                path,
                &format!("contradictions[{i}]"),
            )?;
            string(t, "claim", path, None)?;
            let a = t
                .get("evidence_a")
                .and_then(toml::Value::as_array)
                .ok_or_else(|| format!("{path} contradictions[{i}].evidence_a must be an array"))?;
            let b = t
                .get("evidence_b")
                .and_then(toml::Value::as_array)
                .ok_or_else(|| format!("{path} contradictions[{i}].evidence_b must be an array"))?;
            if a.is_empty() {
                return Err(format!(
                    "{path} contradictions[{i}].evidence_a must not be empty"
                ));
            }
            if b.is_empty() {
                return Err(format!(
                    "{path} contradictions[{i}].evidence_b must not be empty"
                ));
            }
            for (j, ev) in a.iter().enumerate() {
                let s = ev.as_str().ok_or_else(|| {
                    format!("{path} contradictions[{i}].evidence_a[{j}] must be a string")
                })?;
                if s.trim().is_empty() || s.trim() != s {
                    return Err(format!(
                        "{path} contradictions[{i}].evidence_a[{j}] must be non-empty without surrounding whitespace"
                    ));
                }
            }
            for (j, ev) in b.iter().enumerate() {
                let s = ev.as_str().ok_or_else(|| {
                    format!("{path} contradictions[{i}].evidence_b[{j}] must be a string")
                })?;
                if s.trim().is_empty() || s.trim() != s {
                    return Err(format!(
                        "{path} contradictions[{i}].evidence_b[{j}] must be non-empty without surrounding whitespace"
                    ));
                }
            }
        }
    }

    // uncertainty
    if let Some(val) = table.get("uncertainty") {
        let t = val
            .as_table()
            .ok_or_else(|| format!("{path} field `uncertainty` must be a table"))?;
        only_fields(t, &["items"], path, "uncertainty")?;
        let items = t
            .get("items")
            .and_then(toml::Value::as_array)
            .ok_or_else(|| format!("{path} uncertainty.items must be an array"))?;
        if items.is_empty() {
            return Err(format!("{path} uncertainty.items must not be empty"));
        }
        for (i, v) in items.iter().enumerate() {
            let s = v
                .as_str()
                .ok_or_else(|| format!("{path} uncertainty.items[{i}] must be a string"))?;
            if s.trim().is_empty() || s.trim() != s {
                return Err(format!(
                    "{path} uncertainty.items[{i}] must be non-empty without surrounding whitespace"
                ));
            }
        }
    }

    // recommended_next_action
    if let Some(val) = table.get("recommended_next_action") {
        let t = val
            .as_table()
            .ok_or_else(|| format!("{path} field `recommended_next_action` must be a table"))?;
        only_fields(t, &["action"], path, "recommended_next_action")?;
        string(t, "action", path, None)?;
    }

    // overflow
    if let Some(val) = table.get("overflow") {
        let t = val
            .as_table()
            .ok_or_else(|| format!("{path} field `overflow` must be a table"))?;
        only_fields(
            t,
            &["selected", "omitted", "total", "refs"],
            path,
            "overflow",
        )?;
        let selected = t
            .get("selected")
            .and_then(toml::Value::as_integer)
            .ok_or_else(|| format!("{path} overflow.selected must be an integer"))?;
        let omitted = t
            .get("omitted")
            .and_then(toml::Value::as_integer)
            .ok_or_else(|| format!("{path} overflow.omitted must be an integer"))?;
        let total = t
            .get("total")
            .and_then(toml::Value::as_integer)
            .ok_or_else(|| format!("{path} overflow.total must be an integer"))?;
        if selected < 0 || omitted < 0 || total < 0 {
            return Err(format!("{path} overflow counts must be non-negative"));
        }
        if selected + omitted != total {
            return Err(format!(
                "{path} overflow counts must satisfy selected + omitted == total (got {selected}+{omitted}!={total})"
            ));
        }
        let refs = t
            .get("refs")
            .and_then(toml::Value::as_array)
            .ok_or_else(|| format!("{path} overflow.refs must be an array"))?;
        if refs.is_empty() {
            return Err(format!("{path} overflow.refs must not be empty"));
        }
        for (i, r) in refs.iter().enumerate() {
            let s = r
                .as_str()
                .ok_or_else(|| format!("{path} overflow.refs[{i}] must be a string"))?;
            if s.trim().is_empty() || s.trim() != s {
                return Err(format!(
                    "{path} overflow.refs[{i}] must be non-empty without surrounding whitespace"
                ));
            }
            if let Some(repo_path) = s.strip_prefix("artifact:") {
                if repo_path.is_empty() || repo_path.contains(' ') {
                    return Err(format!(
                        "{path} overflow.refs[{i}] artifact path must be repo-relative without whitespace"
                    ));
                }
                let p = workspace_path(repo_path);
                if !p.is_file() {
                    return Err(format!(
                        "{path} overflow.refs artifact `{repo_path}` must resolve to a tracked file"
                    ));
                }
            }
        }
    }

    Ok(action)
}

fn check_evidence_followable(value: &toml::Value, path: &str) -> Result<(), String> {
    let table = value.as_table().unwrap();
    // Check findings evidence followable via artifact or file existence where applicable.
    if let Some(findings) = table.get("findings").and_then(toml::Value::as_array) {
        for (i, item) in findings.iter().enumerate() {
            if let Some(evidence) = item.get("evidence").and_then(toml::Value::as_array) {
                for (j, ev) in evidence.iter().enumerate() {
                    let s = ev.as_str().unwrap_or("");
                    if let Some(repo_path) = s.strip_prefix("artifact:") {
                        // strip line suffix if present? overflow-artifact doesn't have line.
                        let clean = repo_path.split(':').next().unwrap_or(repo_path);
                        let p = workspace_path(clean);
                        if !p.is_file() {
                            return Err(format!(
                                "{path} findings[{i}].evidence[{j}] artifact `{clean}` must resolve to a tracked file"
                            ));
                        }
                    } else if s.contains(':') {
                        // file:line reference — verify file part exists
                        let file_part = s.split(':').next().unwrap_or("");
                        if !file_part.is_empty() && file_part.contains('/') {
                            // Only check if it looks like a repo file path
                            let p = workspace_path(file_part);
                            if file_part.starts_with("xtask/")
                                || file_part.starts_with("docs/")
                                || file_part.starts_with("plans/")
                            {
                                if !p.is_file() {
                                    return Err(format!(
                                        "{path} findings[{i}].evidence[{j}] file `{file_part}` must resolve to a tracked file"
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn check_reordered_equivalence() -> Result<(), String> {
    // Parse same logical content with different key order; semantic meaning must not change.
    let a = r#"
schema = "bounded-subagent-result-v1"
action = "investigate"
verdict = "not_proven"
summary = "Reordered"

[work_item]
issue = "https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1924"
work_spec = "plans/work-specs/examples/UNSAFE-REVIEW-WORK-1924.toml"

[basis]
base_sha = "0f306ea0b6737c13df7abb97bdebb819eaeffdc7"

[[findings]]
severity = "P2"
claim = "c"
evidence = ["xtask/src/subagent_results.rs:1"]

[uncertainty]
items = ["u"]

[recommended_next_action]
action = "next"
"#;
    let b = r#"
verdict = "not_proven"
summary = "Reordered"
action = "investigate"
schema = "bounded-subagent-result-v1"

[basis]
base_sha = "0f306ea0b6737c13df7abb97bdebb819eaeffdc7"

[work_item]
work_spec = "plans/work-specs/examples/UNSAFE-REVIEW-WORK-1924.toml"
issue = "https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1924"

[recommended_next_action]
action = "next"

[uncertainty]
items = ["u"]

[[findings]]
evidence = ["xtask/src/subagent_results.rs:1"]
claim = "c"
severity = "P2"
"#;
    let va: toml::Value =
        toml::from_str(a).map_err(|e| format!("reordered a parse failed: {e}"))?;
    let vb: toml::Value =
        toml::from_str(b).map_err(|e| format!("reordered b parse failed: {e}"))?;
    // Validate both are ok and produce same canonical JSON representation (sorted keys).
    validate(&va, "reordered-a.toml")?;
    validate(&vb, "reordered-b.toml")?;
    let ja = serde_json::to_value(&va).map_err(|e| format!("json a failed: {e}"))?;
    let jb = serde_json::to_value(&vb).map_err(|e| format!("json b failed: {e}"))?;
    let ca = canonical_json(&ja);
    let cb = canonical_json(&jb);
    if ca != cb {
        return Err("reordered equivalent inputs changed semantic meaning".to_string());
    }
    Ok(())
}

fn canonical_json(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Object(map) => {
            let mut keys: Vec<_> = map.keys().collect();
            keys.sort();
            let mut s = String::from("{");
            for (i, k) in keys.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                s.push_str(&serde_json::to_string(k).unwrap());
                s.push(':');
                s.push_str(&canonical_json(&map[*k]));
            }
            s.push('}');
            s
        }
        serde_json::Value::Array(arr) => {
            let mut s = String::from("[");
            for (i, v) in arr.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                s.push_str(&canonical_json(v));
            }
            s.push(']');
            s
        }
        _ => serde_json::to_string(value).unwrap(),
    }
}

fn is_builder_self_green(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    // builder self-green or variants
    (lower.contains("builder") && lower.contains("green"))
        || lower.contains("self-green")
        || lower.contains("self_green")
        || (lower.contains("self") && lower.contains("green"))
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
    if value.trim() != value {
        return Err(format!(
            "{path} field `{field}` must not contain surrounding whitespace"
        ));
    }
    if let Some(expected) = exact
        && value != expected
    {
        return Err(format!("{path} field `{field}` must equal `{expected}`"));
    }
    if !value.contains(|c: char| !c.is_whitespace()) {
        return Err(format!(
            "{path} field `{field}` must not be whitespace-only"
        ));
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
            if value.trim() != value {
                return Err(format!(
                    "{path} field `{field}` must not contain surrounding whitespace"
                ));
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

fn sha(value: &str, path: &str, field: &str) -> Result<(), String> {
    if value.len() != 40
        || !value
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, 'a'..='f'))
    {
        return Err(format!(
            "{path} basis.{field} must be a full lowercase hexadecimal SHA"
        ));
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
                    .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
        });
    if valid {
        Ok(())
    } else {
        Err(format!(
            "{path} field `{field}` value `{value}` must be a normalized repository-relative path"
        ))
    }
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
    let mut chars = value.chars();
    chars.next().is_some_and(|c| matches!(c, '1'..='9')) && chars.all(|c| c.is_ascii_digit())
}

fn valid_github_owner(value: &str) -> bool {
    value.len() <= 39
        && value.starts_with(|c: char| c.is_ascii_alphanumeric())
        && value.ends_with(|c: char| c.is_ascii_alphanumeric())
        && !value.contains("--")
        && value.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn valid_github_repo(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value != "."
        && value != ".."
        && !value.ends_with('.')
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

fn validate_work_spec(reference: &str, issue: &str, path: &str, field: &str) -> Result<(), String> {
    repository_relative_path(reference, path, field)?;
    let name = reference
        .strip_prefix("plans/work-specs/examples/")
        .and_then(|v| v.strip_suffix(".toml"));
    if name.is_none_or(|v| v.is_empty() || v.contains('/')) {
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

fn tracked_repository_file(value: &str, path: &str, field: &str) -> Result<(), String> {
    let output = std::process::Command::new("git")
        .args(["ls-files", "--error-unmatch", "--", value])
        .current_dir(workspace_path(""))
        .output()
        .map_err(|e| format!("{path} failed to resolve {field} `{value}` with git: {e}"))?;
    let stdout = String::from_utf8(output.stdout)
        .map_err(|e| format!("{path} git output for {field} `{value}` is not UTF-8: {e}"))?;
    let exact = output.status.success() && stdout.lines().any(|tracked| tracked == value);
    if exact && workspace_path(value).is_file() {
        Ok(())
    } else {
        Err(format!(
            "{path} {field} `{value}` must resolve with canonical case to a tracked repository file"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{INVALID_FIXTURES, VALID_ROOT, check_reordered_equivalence, validate};
    use crate::{parse_toml_file, workspace_path};

    #[test]
    fn result_valid_examples_pass() -> Result<(), String> {
        for path in crate::subagent_results::toml_files(&workspace_path(VALID_ROOT))? {
            let v = parse_toml_file(&path)?;
            validate(&v, &path.display().to_string())?;
        }
        Ok(())
    }

    #[test]
    fn result_invalid_fixtures_fail() -> Result<(), String> {
        for name in INVALID_FIXTURES {
            let path = workspace_path(&format!("plans/subagent-results/fixtures/invalid/{name}"));
            let v = parse_toml_file(&path)?;
            match validate(&v, &path.display().to_string()) {
                Ok(_) => return Err(format!("{name} invalid fixture unexpectedly passed")),
                Err(e) => {
                    let expected = crate::subagent_results::invalid_expectation(&path)?;
                    if !e.contains(expected) {
                        return Err(format!(
                            "{name} wrong error: expected `{expected}`, got `{e}`"
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    #[test]
    fn result_exact_head_without_pr_fails() -> Result<(), String> {
        let src = r#"
schema = "bounded-subagent-result-v1"
action = "review"
verdict = "clear"
summary = "x"

[work_item]
issue = "https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1924"
work_spec = "plans/work-specs/examples/UNSAFE-REVIEW-WORK-1924.toml"

[basis]
base_sha = "0f306ea0b6737c13df7abb97bdebb819eaeffdc7"
"#;
        let v: toml::Value = toml::from_str(src).map_err(|e| format!("parse: {e}"))?;
        let e = validate(&v, "test.toml").unwrap_err();
        if !e.contains("requires basis.pr and basis.head_sha") {
            return Err(format!("wrong error for missing head: {e}"));
        }
        Ok(())
    }

    #[test]
    fn result_read_only_changes_fail() -> Result<(), String> {
        let src = r#"
schema = "bounded-subagent-result-v1"
action = "investigate"
verdict = "clear"
summary = "x"
changes = ["docs/README.md"]

[work_item]
issue = "https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1924"
work_spec = "plans/work-specs/examples/UNSAFE-REVIEW-WORK-1924.toml"

[basis]
base_sha = "0f306ea0b6737c13df7abb97bdebb819eaeffdc7"
"#;
        let v: toml::Value = toml::from_str(src).map_err(|e| format!("parse: {e}"))?;
        let e = validate(&v, "test.toml").unwrap_err();
        if !e.contains("must have empty changes") {
            return Err(format!("wrong error for read-only changes: {e}"));
        }
        Ok(())
    }

    #[test]
    fn result_builder_self_green_is_not_proof() -> Result<(), String> {
        let src = r#"
schema = "bounded-subagent-result-v1"
action = "build"
verdict = "clear"
summary = "x"
changes = ["docs/README.md"]
proof = ["builder:self-green all good"]

[work_item]
issue = "https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1924"
work_spec = "plans/work-specs/examples/UNSAFE-REVIEW-WORK-1924.toml"

[basis]
base_sha = "0f306ea0b6737c13df7abb97bdebb819eaeffdc7"
"#;
        let v: toml::Value = toml::from_str(src).map_err(|e| format!("parse: {e}"))?;
        let e = validate(&v, "test.toml").unwrap_err();
        if !e.contains("builder self-green") {
            return Err(format!("wrong error for builder self-green: {e}"));
        }
        Ok(())
    }

    #[test]
    fn result_contradiction_preserves_both_sides() -> Result<(), String> {
        let src = r#"
schema = "bounded-subagent-result-v1"
action = "review"
verdict = "revise"
summary = "x"

[work_item]
issue = "https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1924"
work_spec = "plans/work-specs/examples/UNSAFE-REVIEW-WORK-1924.toml"

[basis]
base_sha = "0f306ea0b6737c13df7abb97bdebb819eaeffdc7"
pr = "https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/2124"
head_sha = "2222222222222222222222222222222222222222"

[[contradictions]]
claim = "c"
evidence_a = ["xtask/src/subagent_results.rs:1"]
evidence_b = ["docs/README.md:1"]

[uncertainty]
items = ["u"]

[recommended_next_action]
action = "next"
"#;
        let v: toml::Value = toml::from_str(src).map_err(|e| format!("parse: {e}"))?;
        validate(&v, "test.toml")?;
        // missing side should fail
        let src2 = src.replace("evidence_b = [\"docs/README.md:1\"]", "evidence_b = []");
        let v2: toml::Value = toml::from_str(&src2).map_err(|e| format!("parse: {e}"))?;
        let e = validate(&v2, "test.toml").unwrap_err();
        if !e.contains("evidence_b") {
            return Err(format!("wrong error for missing side: {e}"));
        }
        Ok(())
    }

    #[test]
    fn result_large_log_overflow_bounded() -> Result<(), String> {
        let v = parse_toml_file(&workspace_path(
            "plans/delegation-examples/valid-overflow.toml",
        ))?;
        // reuse shared validation for overflow via validate overflow directly?
        // Ensure delegation overflow logic still passes and our overflow validation matches
        let table = v.as_table().unwrap();
        let overflow = table.get("overflow").unwrap().as_table().unwrap();
        let selected = overflow.get("selected").unwrap().as_integer().unwrap();
        let omitted = overflow.get("omitted").unwrap().as_integer().unwrap();
        let total = overflow.get("total").unwrap().as_integer().unwrap();
        if selected + omitted != total {
            return Err("overflow counts mismatch".to_string());
        }
        Ok(())
    }

    #[test]
    fn result_reordered_equivalence() -> Result<(), String> {
        check_reordered_equivalence()
    }

    #[test]
    fn result_forbidden_confidence_voting_rejected() -> Result<(), String> {
        let src = r#"
schema = "bounded-subagent-result-v1"
action = "investigate"
verdict = "clear"
summary = "x"
confidence = 0.9

[work_item]
issue = "https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1924"
work_spec = "plans/work-specs/examples/UNSAFE-REVIEW-WORK-1924.toml"

[basis]
base_sha = "0f306ea0b6737c13df7abb97bdebb819eaeffdc7"
"#;
        let v: toml::Value = toml::from_str(src).map_err(|e| format!("parse: {e}"))?;
        let e = validate(&v, "test.toml").unwrap_err();
        if !e.contains("forbidden field `confidence`") {
            return Err(format!("wrong error for confidence: {e}"));
        }
        Ok(())
    }
}
