//! Manifest-only validation for read-only external pilot receipts.
//!
//! External pilots are adoption/usefulness evidence. The checked receipts record
//! exact external PR refs, first-use friction, artifact metrics, selected and
//! omitted comment counts, and human usefulness judgments. They are diagnostic
//! inputs only: no precision/recall, UB-free, Miri-clean, site-execution, or
//! memory-safety proof claim.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::{parse_toml_file, read_to_string, workspace_path};

const PILOT_DIR: &str = "docs/dogfood/pilots";
const README: &str = "docs/dogfood/pilots/README.md";
const SCHEMA_VERSION: &str = "external-pilot/v1";

const SOURCES: &[&str] = &["public-action", "local-equivalent-artifact-bundle"];
const STATUSES: &[&str] = &["recorded"];
const SURFACES: &[&str] = &[
    "agent_packet",
    "artifact_bundle",
    "cards",
    "comment_plan",
    "github_summary",
    "pr_summary",
    "setup",
    "usefulness_telemetry",
];
const JUDGMENT_LABELS: &[&str] = &[
    "actionable",
    "correct_but_not_worth_surfacing",
    "inherited",
    "duplicate",
    "human_only",
    "agent_ready",
    "unclear",
    "incorrect",
    "missed_expected_seam",
    "setup_friction",
    "artifact_friction",
];
const REQUIRED_ARTIFACT_KINDS: &[&str] = &[
    "review_kit",
    "cards",
    "comment_plan",
    "gate",
    "github_summary",
    "pr_summary",
    "repair_queue",
    "sarif",
    "lsp",
    "receipt_audit_json",
    "usefulness_telemetry",
];
const ARTIFACT_KINDS: &[&str] = &[
    "cards",
    "comment_plan",
    "gate",
    "github_summary",
    "lsp",
    "manual_candidates",
    "manual_repair_queue",
    "policy_report_json",
    "policy_report_md",
    "pr_summary",
    "receipt_audit_json",
    "receipt_audit_md",
    "repair_queue",
    "review_kit",
    "sarif",
    "tokmd_packets",
    "usefulness_telemetry",
    "witness_plan",
];

#[derive(Debug, PartialEq, Eq)]
struct RunMetrics {
    artifact_count: i64,
    artifact_total_bytes: i64,
}

#[derive(Debug, PartialEq, Eq)]
struct ArtifactMetrics {
    count: i64,
    total_bytes: i64,
}

pub(crate) fn check() -> Result<(), String> {
    check_readme()?;
    let paths = pilot_receipt_paths()?;
    if paths.is_empty() {
        return Err(format!(
            "{PILOT_DIR} must contain at least one *.toml pilot receipt"
        ));
    }

    for path in &paths {
        check_receipt(path)?;
    }

    super::external_pilot_rollup::check()?;

    println!("check-external-pilots: ok ({} receipt(s))", paths.len());
    Ok(())
}

pub(crate) fn receipt_paths_for_rollup() -> Result<Vec<std::path::PathBuf>, String> {
    pilot_receipt_paths()
}

fn check_readme() -> Result<(), String> {
    let text = read_to_string(&workspace_path(README))?;
    for needle in [
        "external-pilot/v1",
        "setup friction",
        "selected comments",
        "omitted cards",
        "runtime",
        "artifact size",
        "read-only",
        "not calibrated precision or recall",
        "not memory-safety proof",
        "not UB-free status",
        "not Miri-clean status",
        "not site-execution evidence",
        "no third-party comments or issues",
        "cargo run --locked -p xtask -- check-external-pilots",
    ] {
        if !text.contains(needle) {
            return Err(format!("{README} must document `{needle}`"));
        }
    }
    Ok(())
}

fn pilot_receipt_paths() -> Result<Vec<std::path::PathBuf>, String> {
    let dir = workspace_path(PILOT_DIR);
    if !dir.is_dir() {
        return Err(format!("{PILOT_DIR} is missing"));
    }

    let mut paths = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|err| format!("read {PILOT_DIR} failed: {err}"))? {
        let entry = entry.map_err(|err| format!("read {PILOT_DIR} entry failed: {err}"))?;
        let path = entry.path();
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if file_name == "README.md" || file_name.starts_with('.') || path.is_dir() {
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
            return Err(format!(
                "{PILOT_DIR} may contain only README.md and *.toml receipts; found {}",
                path.display()
            ));
        }
        paths.push(path);
    }
    paths.sort();
    Ok(paths)
}

fn check_receipt(path: &Path) -> Result<(), String> {
    let display = path.to_string_lossy().replace('\\', "/");
    let value = parse_toml_file(path)?;
    let table = root_table(&value, &display)?;

    let schema_version = required_string(table, "schema_version", &display)?;
    require_value(schema_version, SCHEMA_VERSION, &display, "schema_version")?;

    let id = required_string(table, "id", &display)?;
    let expected_file_name = format!("{id}.toml");
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            format!(
                "{} contains a pilot receipt with a non-UTF-8 name: {}",
                PILOT_DIR,
                path.display()
            )
        })?;
    if file_name != expected_file_name {
        return Err(format!(
            "{display} id `{id}` must match receipt filename `{expected_file_name}`"
        ));
    }
    check_slug(id, &display, "id")?;

    require_known(
        required_string(table, "status", &display)?,
        STATUSES,
        &display,
        "status",
    )?;
    check_date(required_string(table, "date", &display)?, &display, "date")?;
    required_string(table, "reviewer", &display)?;
    check_repository(required_string(table, "repository", &display)?, &display)?;
    check_positive_integer(required_integer(table, "pr", &display)?, &display, "pr")?;
    check_github_url(required_string(table, "url", &display)?, &display)?;
    require_known(
        required_string(table, "source", &display)?,
        SOURCES,
        &display,
        "source",
    )?;
    required_string(table, "acquisition_method", &display)?;
    required_string(table, "tool_version", &display)?;
    let base_sha = required_string(table, "base_sha", &display)?;
    check_git_sha(base_sha, &display, "base_sha")?;
    let head_sha = required_string(table, "head_sha", &display)?;
    check_git_sha(head_sha, &display, "head_sha")?;
    if base_sha == head_sha {
        return Err(format!("{display} base_sha and head_sha must differ"));
    }
    check_target_path(
        required_string(table, "diff_path", &display)?,
        &display,
        "diff_path",
    )?;
    check_sha256(
        required_string(table, "diff_sha256", &display)?,
        &display,
        "diff_sha256",
    )?;
    check_trust_boundary(
        required_string(table, "trust_boundary", &display)?,
        &display,
    )?;

    check_read_only(table, &display)?;
    let total_cards = check_card_inventory(table, &display)?;
    check_comment_plan(table, &display, total_cards)?;
    check_gate_summary(table, &display)?;
    let run_metrics = check_run(table, &display)?;
    let artifact_metrics = check_artifacts(table, &display)?;
    check_artifact_metrics(&run_metrics, &artifact_metrics, &display)?;
    check_judgments(table, &display)?;

    Ok(())
}

fn check_read_only(table: &toml::map::Map<String, toml::Value>, path: &str) -> Result<(), String> {
    let read_only = required_table(table, "read_only", path)?;
    for field in [
        "no_source_edits",
        "no_third_party_comments",
        "no_third_party_issues",
        "no_witnesses_run",
    ] {
        if !required_bool(read_only, field, path, "read_only")? {
            return Err(format!("{path} read_only.{field} must be true"));
        }
    }
    Ok(())
}

fn check_card_inventory(
    table: &toml::map::Map<String, toml::Value>,
    path: &str,
) -> Result<i64, String> {
    let cards = required_table(table, "card_inventory", path)?;
    let total = required_section_integer(cards, "total_cards", path, "card_inventory")?;
    check_nonnegative_integer(total, path, "card_inventory.total_cards")?;
    check_nonnegative_integer(
        required_section_integer(cards, "agent_ready", path, "card_inventory")?,
        path,
        "card_inventory.agent_ready",
    )?;
    check_nonnegative_integer(
        required_section_integer(cards, "human_only", path, "card_inventory")?,
        path,
        "card_inventory.human_only",
    )?;
    Ok(total)
}

fn check_comment_plan(
    table: &toml::map::Map<String, toml::Value>,
    path: &str,
    total_cards: i64,
) -> Result<(), String> {
    let comment_plan = required_table(table, "comment_plan", path)?;
    require_value(
        required_section_string(comment_plan, "mode", path, "comment_plan")?,
        "plan_only",
        path,
        "comment_plan.mode",
    )?;
    let selected = required_section_integer(comment_plan, "selected_count", path, "comment_plan")?;
    let omitted =
        required_section_integer(comment_plan, "not_selected_count", path, "comment_plan")?;
    check_nonnegative_integer(selected, path, "comment_plan.selected_count")?;
    check_nonnegative_integer(omitted, path, "comment_plan.not_selected_count")?;
    if selected + omitted != total_cards {
        return Err(format!(
            "{path} comment_plan selected_count + not_selected_count must equal card_inventory.total_cards"
        ));
    }
    required_section_string(comment_plan, "selection_reason", path, "comment_plan")?;
    Ok(())
}

fn check_gate_summary(
    table: &toml::map::Map<String, toml::Value>,
    path: &str,
) -> Result<(), String> {
    let gate = required_table(table, "gate_summary", path)?;
    require_value(
        required_section_string(gate, "status", path, "gate_summary")?,
        "advisory",
        path,
        "gate_summary.status",
    )?;
    for field in [
        "new_gaps",
        "worsened_gaps",
        "improved_gaps",
        "resolved_gaps",
        "inherited_gaps",
    ] {
        check_nonnegative_integer(
            required_section_integer(gate, field, path, "gate_summary")?,
            path,
            &format!("gate_summary.{field}"),
        )?;
    }
    Ok(())
}

fn check_run(
    table: &toml::map::Map<String, toml::Value>,
    path: &str,
) -> Result<RunMetrics, String> {
    let run = required_table(table, "run", path)?;
    check_nonnegative_integer(
        required_section_integer(run, "exit_code", path, "run")?,
        path,
        "run.exit_code",
    )?;
    check_positive_number(
        required_number(run, "elapsed_seconds", path, "run")?,
        path,
        "run.elapsed_seconds",
    )?;
    check_positive_integer(
        required_section_integer(run, "diff_bytes", path, "run")?,
        path,
        "run.diff_bytes",
    )?;
    let artifact_count = required_section_integer(run, "artifact_count", path, "run")?;
    check_positive_integer(artifact_count, path, "run.artifact_count")?;
    let artifact_total_bytes = required_section_integer(run, "artifact_total_bytes", path, "run")?;
    check_positive_integer(artifact_total_bytes, path, "run.artifact_total_bytes")?;
    check_positive_integer(
        required_section_integer(run, "rust_files_changed", path, "run")?,
        path,
        "run.rust_files_changed",
    )?;
    Ok(RunMetrics {
        artifact_count,
        artifact_total_bytes,
    })
}

fn check_artifacts(
    table: &toml::map::Map<String, toml::Value>,
    path: &str,
) -> Result<ArtifactMetrics, String> {
    let artifacts = required_array(table, "artifacts", path)?;
    let mut seen_kinds = BTreeSet::new();
    let mut seen_paths = BTreeSet::new();
    let mut total_bytes = 0i64;
    for (idx, artifact) in artifacts.iter().enumerate() {
        let artifact = array_table(artifact, path, "artifacts", idx)?;
        let kind = required_item_string(artifact, "kind", path, "artifacts", idx)?;
        require_known(kind, ARTIFACT_KINDS, path, "artifacts.kind")?;
        if !seen_kinds.insert(kind.to_string()) {
            return Err(format!("{path} artifacts[{idx}] duplicates kind `{kind}`"));
        }
        let artifact_path = required_item_string(artifact, "path", path, "artifacts", idx)?;
        check_target_path(artifact_path, path, "artifacts.path")?;
        if !seen_paths.insert(artifact_path.to_string()) {
            return Err(format!(
                "{path} artifacts[{idx}] duplicates path `{artifact_path}`"
            ));
        }
        let bytes = required_item_integer(artifact, "bytes", path, "artifacts", idx)?;
        check_positive_integer(bytes, path, "artifacts.bytes")?;
        total_bytes += bytes;
        check_sha256(
            required_item_string(artifact, "sha256", path, "artifacts", idx)?,
            path,
            "artifacts.sha256",
        )?;
    }

    for required in REQUIRED_ARTIFACT_KINDS {
        if !seen_kinds.contains(*required) {
            return Err(format!("{path} artifacts must include `{required}`"));
        }
    }
    let count = i64::try_from(artifacts.len())
        .map_err(|err| format!("{path} artifact count overflowed i64: {err}"))?;
    Ok(ArtifactMetrics { count, total_bytes })
}

fn check_artifact_metrics(
    run: &RunMetrics,
    artifacts: &ArtifactMetrics,
    path: &str,
) -> Result<(), String> {
    if run.artifact_count != artifacts.count {
        return Err(format!(
            "{path} run.artifact_count={} does not match listed artifact count {}",
            run.artifact_count, artifacts.count
        ));
    }
    if run.artifact_total_bytes != artifacts.total_bytes {
        return Err(format!(
            "{path} run.artifact_total_bytes={} does not match listed artifact bytes {}",
            run.artifact_total_bytes, artifacts.total_bytes
        ));
    }
    Ok(())
}

fn check_judgments(table: &toml::map::Map<String, toml::Value>, path: &str) -> Result<(), String> {
    let judgments = required_array(table, "judgments", path)?;
    if judgments.is_empty() {
        return Err(format!(
            "{path} must include at least one [[judgments]] row"
        ));
    }
    let mut has_friction = false;
    for (idx, judgment) in judgments.iter().enumerate() {
        let judgment = array_table(judgment, path, "judgments", idx)?;
        let surface = required_item_string(judgment, "surface", path, "judgments", idx)?;
        require_known(surface, SURFACES, path, "judgments.surface")?;
        let label = required_item_string(judgment, "label", path, "judgments", idx)?;
        require_known(label, JUDGMENT_LABELS, path, "judgments.label")?;
        if matches!(label, "setup_friction" | "artifact_friction") {
            has_friction = true;
        }
        required_item_string(judgment, "reason", path, "judgments", idx)?;
        required_item_string(judgment, "next_step", path, "judgments", idx)?;
    }
    if !has_friction {
        return Err(format!(
            "{path} must record at least one setup_friction or artifact_friction judgment"
        ));
    }
    Ok(())
}

fn root_table<'a>(
    value: &'a toml::Value,
    path: &str,
) -> Result<&'a toml::map::Map<String, toml::Value>, String> {
    value
        .as_table()
        .ok_or_else(|| format!("{path} root must be a TOML table"))
}

fn required_table<'a>(
    table: &'a toml::map::Map<String, toml::Value>,
    key: &str,
    path: &str,
) -> Result<&'a toml::map::Map<String, toml::Value>, String> {
    table
        .get(key)
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{path} is missing table `{key}`"))
}

fn required_array<'a>(
    table: &'a toml::map::Map<String, toml::Value>,
    key: &str,
    path: &str,
) -> Result<&'a Vec<toml::Value>, String> {
    table
        .get(key)
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{path} is missing array `{key}`"))
}

fn array_table<'a>(
    value: &'a toml::Value,
    path: &str,
    section: &str,
    idx: usize,
) -> Result<&'a toml::map::Map<String, toml::Value>, String> {
    value
        .as_table()
        .ok_or_else(|| format!("{path} {section}[{idx}] must be a table"))
}

fn required_string<'a>(
    table: &'a toml::map::Map<String, toml::Value>,
    key: &str,
    path: &str,
) -> Result<&'a str, String> {
    table
        .get(key)
        .and_then(toml::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{path} is missing non-empty string `{key}`"))
}

fn required_integer(
    table: &toml::map::Map<String, toml::Value>,
    key: &str,
    path: &str,
) -> Result<i64, String> {
    table
        .get(key)
        .and_then(toml::Value::as_integer)
        .ok_or_else(|| format!("{path} is missing integer `{key}`"))
}

fn required_number(
    table: &toml::map::Map<String, toml::Value>,
    key: &str,
    path: &str,
    section: &str,
) -> Result<f64, String> {
    let Some(value) = table.get(key) else {
        return Err(format!("{path} {section} is missing number `{key}`"));
    };
    if let Some(float) = value.as_float() {
        return Ok(float);
    }
    if let Some(integer) = value.as_integer() {
        return Ok(integer as f64);
    }
    Err(format!("{path} {section}.{key} must be a number"))
}

fn required_bool(
    table: &toml::map::Map<String, toml::Value>,
    key: &str,
    path: &str,
    section: &str,
) -> Result<bool, String> {
    table
        .get(key)
        .and_then(toml::Value::as_bool)
        .ok_or_else(|| format!("{path} {section} is missing boolean `{key}`"))
}

fn required_section_string<'a>(
    table: &'a toml::map::Map<String, toml::Value>,
    key: &str,
    path: &str,
    section: &str,
) -> Result<&'a str, String> {
    table
        .get(key)
        .and_then(toml::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{path} {section} is missing non-empty string `{key}`"))
}

fn required_section_integer(
    table: &toml::map::Map<String, toml::Value>,
    key: &str,
    path: &str,
    section: &str,
) -> Result<i64, String> {
    table
        .get(key)
        .and_then(toml::Value::as_integer)
        .ok_or_else(|| format!("{path} {section} is missing integer `{key}`"))
}

fn required_item_string<'a>(
    table: &'a toml::map::Map<String, toml::Value>,
    key: &str,
    path: &str,
    section: &str,
    idx: usize,
) -> Result<&'a str, String> {
    table
        .get(key)
        .and_then(toml::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{path} {section}[{idx}] is missing non-empty string `{key}`"))
}

fn required_item_integer(
    table: &toml::map::Map<String, toml::Value>,
    key: &str,
    path: &str,
    section: &str,
    idx: usize,
) -> Result<i64, String> {
    table
        .get(key)
        .and_then(toml::Value::as_integer)
        .ok_or_else(|| format!("{path} {section}[{idx}] is missing integer `{key}`"))
}

fn require_value(actual: &str, expected: &str, path: &str, field: &str) -> Result<(), String> {
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "{path} {field} must be `{expected}`, got `{actual}`"
        ))
    }
}

fn require_known(actual: &str, allowed: &[&str], path: &str, field: &str) -> Result<(), String> {
    if allowed.contains(&actual) {
        Ok(())
    } else {
        Err(format!(
            "{path} {field} uses unknown value `{actual}`; expected one of: {}",
            allowed.join(", ")
        ))
    }
}

fn check_slug(value: &str, path: &str, field: &str) -> Result<(), String> {
    let valid = !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !value.starts_with('-')
        && !value.ends_with('-')
        && !value.contains("--");
    if valid {
        Ok(())
    } else {
        Err(format!("{path} {field} must be lowercase kebab-case"))
    }
}

fn check_date(value: &str, path: &str, field: &str) -> Result<(), String> {
    let bytes = value.as_bytes();
    let valid = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(idx, byte)| idx == 4 || idx == 7 || byte.is_ascii_digit());
    if valid {
        Ok(())
    } else {
        Err(format!("{path} {field} `{value}` must use YYYY-MM-DD"))
    }
}

fn check_repository(value: &str, path: &str) -> Result<(), String> {
    let parts = value.split('/').collect::<Vec<_>>();
    if parts.len() == 2 && parts.iter().all(|part| !part.trim().is_empty()) {
        Ok(())
    } else {
        Err(format!("{path} repository `{value}` must use owner/repo"))
    }
}

fn check_github_url(value: &str, path: &str) -> Result<(), String> {
    if value.starts_with("https://github.com/") && value.contains("/pull/") {
        Ok(())
    } else {
        Err(format!(
            "{path} url `{value}` must be a GitHub pull request URL"
        ))
    }
}

fn check_git_sha(value: &str, path: &str, field: &str) -> Result<(), String> {
    let valid = value.len() == 40;
    if valid && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(format!("{path} {field} must be a 40-char git SHA"))
    }
}

fn check_sha256(value: &str, path: &str, field: &str) -> Result<(), String> {
    let valid = value.len() == 64;
    if valid && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(format!("{path} {field} must be a 64-char sha256"))
    }
}

fn check_target_path(value: &str, path: &str, field: &str) -> Result<(), String> {
    if value.starts_with('/') || value.contains('\\') || value.contains("..") {
        return Err(format!(
            "{path} {field} path must be relative, forward-slash only, and stay inside the workspace: {value}"
        ));
    }
    if value.starts_with("target/") {
        Ok(())
    } else {
        Err(format!(
            "{path} {field} path `{value}` must point under target/ because pilot artifacts are untracked evidence"
        ))
    }
}

fn check_positive_integer(value: i64, path: &str, field: &str) -> Result<(), String> {
    if value > 0 {
        Ok(())
    } else {
        Err(format!("{path} {field} must be positive"))
    }
}

fn check_nonnegative_integer(value: i64, path: &str, field: &str) -> Result<(), String> {
    if value >= 0 {
        Ok(())
    } else {
        Err(format!("{path} {field} must be non-negative"))
    }
}

fn check_positive_number(value: f64, path: &str, field: &str) -> Result<(), String> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(format!("{path} {field} must be a positive finite number"))
    }
}

fn check_trust_boundary(text: &str, path: &str) -> Result<(), String> {
    let lower = text.to_ascii_lowercase();
    for needle in [
        "static unsafe contract review",
        "not calibrated",
        "precision",
        "recall",
        "not a proof of memory safety",
        "not ub-free status",
        "not a miri result",
        "not miri-clean status",
        "not site-execution evidence",
        "not witness adequacy",
        "not policy readiness",
        "no third-party comments or issues",
    ] {
        if !lower.contains(needle) {
            return Err(format!("{path} trust_boundary must document `{needle}`"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_value(text: &str) -> Result<toml::Value, String> {
        text.parse::<toml::Table>()
            .map(toml::Value::Table)
            .map_err(|err| format!("parse synthetic receipt failed: {err}"))
    }

    fn check_text(path: &str, text: &str) -> Result<(), String> {
        let value = parse_value(text)?;
        let table = root_table(&value, path)?;
        check_git_sha(required_string(table, "base_sha", path)?, path, "base_sha")?;
        check_git_sha(required_string(table, "head_sha", path)?, path, "head_sha")?;
        check_sha256(
            required_string(table, "diff_sha256", path)?,
            path,
            "diff_sha256",
        )?;
        check_read_only(table, path)?;
        let total_cards = check_card_inventory(table, path)?;
        check_comment_plan(table, path, total_cards)?;
        check_gate_summary(table, path)?;
        let run_metrics = check_run(table, path)?;
        let artifact_metrics = check_artifacts(table, path)?;
        check_artifact_metrics(&run_metrics, &artifact_metrics, path)?;
        check_judgments(table, path)?;
        check_trust_boundary(required_string(table, "trust_boundary", path)?, path)
    }

    fn valid_receipt() -> &'static str {
        r#"
schema_version = "external-pilot/v1"
id = "synthetic-pilot"
status = "recorded"
date = "2026-06-19"
reviewer = "manual"
repository = "owner/repo"
pr = 1
url = "https://github.com/owner/repo/pull/1"
source = "local-equivalent-artifact-bundle"
acquisition_method = "repo-local cargo run"
tool_version = "0.3.8"
base_sha = "1111111111111111111111111111111111111111"
head_sha = "2222222222222222222222222222222222222222"
diff_path = "target/external-pilots/synthetic/diff.patch"
diff_sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
trust_boundary = "Static unsafe contract review external pilot receipt; not calibrated precision or recall, not a proof of memory safety, not UB-free status, not a Miri result, not Miri-clean status, not site-execution evidence, not witness adequacy, not policy readiness, and no third-party comments or issues were filed."

[read_only]
no_source_edits = true
no_third_party_comments = true
no_third_party_issues = true
no_witnesses_run = true

[card_inventory]
total_cards = 1
agent_ready = 0
human_only = 1

[comment_plan]
mode = "plan_only"
selected_count = 1
not_selected_count = 0
selection_reason = "bounded reviewer noise"

[gate_summary]
status = "advisory"
new_gaps = 1
worsened_gaps = 0
improved_gaps = 0
resolved_gaps = 0
inherited_gaps = 0

[run]
exit_code = 0
elapsed_seconds = 1.0
diff_bytes = 1
artifact_count = 11
artifact_total_bytes = 11
rust_files_changed = 1

[[artifacts]]
kind = "review_kit"
path = "target/external-pilots/synthetic/review-kit.json"
bytes = 1
sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"

[[artifacts]]
kind = "cards"
path = "target/external-pilots/synthetic/cards.json"
bytes = 1
sha256 = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"

[[artifacts]]
kind = "comment_plan"
path = "target/external-pilots/synthetic/comment-plan.json"
bytes = 1
sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"

[[artifacts]]
kind = "gate"
path = "target/external-pilots/synthetic/unsafe-review-gate.json"
bytes = 1
sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"

[[artifacts]]
kind = "github_summary"
path = "target/external-pilots/synthetic/github-summary.md"
bytes = 1
sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"

[[artifacts]]
kind = "pr_summary"
path = "target/external-pilots/synthetic/pr-summary.md"
bytes = 1
sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"

[[artifacts]]
kind = "repair_queue"
path = "target/external-pilots/synthetic/repair-queue.json"
bytes = 1
sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"

[[artifacts]]
kind = "sarif"
path = "target/external-pilots/synthetic/cards.sarif"
bytes = 1
sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"

[[artifacts]]
kind = "lsp"
path = "target/external-pilots/synthetic/lsp.json"
bytes = 1
sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"

[[artifacts]]
kind = "receipt_audit_json"
path = "target/external-pilots/synthetic/receipt-audit.json"
bytes = 1
sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"

[[artifacts]]
kind = "usefulness_telemetry"
path = "target/external-pilots/synthetic/usefulness-telemetry.json"
bytes = 1
sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"

[[judgments]]
surface = "setup"
label = "setup_friction"
reason = "The diff acquisition path required explicit raw patch output."
next_step = "Keep the friction visible in the first-use backlog."
"#
    }

    #[test]
    fn valid_receipt_text_passes() -> Result<(), String> {
        check_text("docs/dogfood/pilots/synthetic-pilot.toml", valid_receipt())
    }

    #[test]
    fn quiet_receipt_with_zero_cards_passes() -> Result<(), String> {
        let text = valid_receipt()
            .replace("total_cards = 1", "total_cards = 0")
            .replace(
                "agent_ready = 0\nhuman_only = 1",
                "agent_ready = 0\nhuman_only = 0",
            )
            .replace("selected_count = 1", "selected_count = 0")
            .replace("new_gaps = 1", "new_gaps = 0");
        check_text("docs/dogfood/pilots/quiet-synthetic-pilot.toml", &text)
    }

    #[test]
    fn unknown_judgment_label_fails() -> Result<(), String> {
        let text = valid_receipt().replace("setup_friction", "accuracy_score");
        let err = match check_text("docs/dogfood/pilots/synthetic-pilot.toml", &text) {
            Ok(_) => return Err("expected unknown label failure".to_string()),
            Err(err) => err,
        };
        if err.contains("unknown value `accuracy_score`") {
            Ok(())
        } else {
            Err(format!("unexpected error: {err}"))
        }
    }

    #[test]
    fn missing_trust_boundary_term_fails() -> Result<(), String> {
        let text = valid_receipt().replace("not site-execution evidence, ", "");
        let err = match check_text("docs/dogfood/pilots/synthetic-pilot.toml", &text) {
            Ok(_) => return Err("expected trust-boundary failure".to_string()),
            Err(err) => err,
        };
        if err.contains("site-execution") {
            Ok(())
        } else {
            Err(format!("unexpected error: {err}"))
        }
    }

    #[test]
    fn short_sha256_fails() -> Result<(), String> {
        let text = valid_receipt().replace(
            "diff_sha256 = \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"",
            "diff_sha256 = \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"",
        );
        let err = match check_text("docs/dogfood/pilots/synthetic-pilot.toml", &text) {
            Ok(_) => return Err("expected sha256 length failure".to_string()),
            Err(err) => err,
        };
        if err.contains("64-char sha256") {
            Ok(())
        } else {
            Err(format!("unexpected error: {err}"))
        }
    }

    #[test]
    fn stale_artifact_total_fails() -> Result<(), String> {
        let text =
            valid_receipt().replace("artifact_total_bytes = 11", "artifact_total_bytes = 12");
        let err = match check_text("docs/dogfood/pilots/synthetic-pilot.toml", &text) {
            Ok(_) => return Err("expected artifact metric failure".to_string()),
            Err(err) => err,
        };
        if err.contains("does not match listed artifact bytes") {
            Ok(())
        } else {
            Err(format!("unexpected error: {err}"))
        }
    }
}
