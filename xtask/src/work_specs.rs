//! Offline structural checks for issue-linked work-spec contracts.
//!
//! The content schema deliberately lives outside cargo-allow's fixed artifact
//! kind vocabulary. Until cargo-allow supports a first-class `work_spec` kind,
//! the example is registered as a draft `plan_item` while this checker
//! validates the machine-readable contract itself.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::{parse_toml_file, read_to_string, workspace_path};

const WORK_SPEC_SCHEMA: &str = "docs/schemas/issue-work-spec.schema.json";
const VALID_EXAMPLES_ROOT: &str = "plans/work-specs/examples";
const INVALID_FIXTURES_ROOT: &str = "plans/work-specs/fixtures/invalid";
const WORK_KINDS: &[&str] = &[
    "correctness",
    "ux",
    "interop",
    "robustness",
    "maintenance",
    "research",
];
const REQUIRED_TOP_LEVEL: &[&str] = &[
    "schema_version",
    "issue",
    "kind",
    "objective",
    "user_outcome",
    "claim_boundary",
    "scope",
    "invariant",
    "acceptance",
    "integration",
    "risk",
    "rollback",
];
const OPTIONAL_TOP_LEVEL: &[&str] = &[
    "dependencies",
    "blockers",
    "affected_files",
    "affected_symbols",
    "linked_specs",
    "linked_adrs",
    "compatibility",
    "delivery",
];
const FORBIDDEN_SCHEDULING_FIELDS: &[&str] = &[
    "active_goal",
    "current_task",
    "default_goal",
    "priority",
    "queue",
    "rank",
    "schedule",
    "status",
];

pub(crate) fn check() -> Result<(), String> {
    let schema_path = workspace_path(WORK_SPEC_SCHEMA);
    let schema_text = read_to_string(&schema_path)?;
    let schema: serde_json::Value = serde_json::from_str(&schema_text)
        .map_err(|err| format!("{WORK_SPEC_SCHEMA} is not valid JSON: {err}"))?;
    if schema.get("$schema").and_then(serde_json::Value::as_str)
        != Some("https://json-schema.org/draft/2020-12/schema")
    {
        return Err(format!(
            "{WORK_SPEC_SCHEMA} must declare the JSON Schema 2020-12 dialect"
        ));
    }

    let valid_paths = toml_files(&workspace_path(VALID_EXAMPLES_ROOT))?;
    if valid_paths.is_empty() {
        return Err(format!(
            "{VALID_EXAMPLES_ROOT} must contain at least one valid work-spec example"
        ));
    }
    for path in &valid_paths {
        check_file(path)?;
    }

    let invalid_paths = toml_files(&workspace_path(INVALID_FIXTURES_ROOT))?;
    if invalid_paths.is_empty() {
        return Err(format!(
            "{INVALID_FIXTURES_ROOT} must contain invalid work-spec fixtures"
        ));
    }
    for path in &invalid_paths {
        let value = parse_toml_file(path)?;
        if validate_work_spec(&value, &path.display().to_string()).is_ok() {
            return Err(format!(
                "{} is marked invalid but passes work-spec validation",
                path.display()
            ));
        }
    }

    println!(
        "check-work-specs: ok ({} valid examples, {} invalid fixtures)",
        valid_paths.len(),
        invalid_paths.len()
    );
    Ok(())
}

fn check_file(path: &Path) -> Result<(), String> {
    let value = parse_toml_file(path)?;
    validate_work_spec(&value, &path.display().to_string())
}

fn toml_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = fs::read_dir(root).map_err(|err| {
        format!(
            "failed to read work-spec directory {}: {err}",
            root.display()
        )
    })?;
    let mut paths = Vec::new();
    for entry in entries {
        let path = entry
            .map_err(|err| format!("failed to read work-spec directory entry: {err}"))?
            .path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("toml") {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

fn validate_work_spec(value: &toml::Value, path: &str) -> Result<(), String> {
    let table = value
        .as_table()
        .ok_or_else(|| format!("{path} must contain a TOML table"))?;
    let allowed: BTreeSet<&str> = REQUIRED_TOP_LEVEL
        .iter()
        .chain(OPTIONAL_TOP_LEVEL)
        .copied()
        .collect();
    for key in table.keys() {
        if FORBIDDEN_SCHEDULING_FIELDS.contains(&key.as_str()) {
            return Err(format!(
                "{path} contains scheduling field `{key}`; GitHub owns queue state"
            ));
        }
        if !allowed.contains(key.as_str()) {
            return Err(format!("{path} contains unknown work-spec field `{key}`"));
        }
    }
    for key in REQUIRED_TOP_LEVEL {
        if !table.contains_key(*key) {
            return Err(format!("{path} is missing required field `{key}`"));
        }
    }

    require_string(table, "schema_version", path, Some("1"))?;
    let issue = require_string(table, "issue", path, None)?;
    validate_issue_url(issue, path)?;
    require_known_string(table, "kind", path, WORK_KINDS)?;
    for field in ["objective", "user_outcome", "claim_boundary"] {
        require_string(table, field, path, None)?;
    }

    let scope = require_table(table, "scope", path)?;
    require_string_array(scope, "included", path, "scope")?;
    require_string_array(scope, "excluded", path, "scope")?;

    let invariants = require_table_array(table, "invariant", path)?;
    let mut invariant_ids = BTreeSet::new();
    for (index, invariant) in invariants.iter().enumerate() {
        let invariant = invariant
            .as_table()
            .ok_or_else(|| format!("{path} invariant[{index}] must be a table"))?;
        let context = format!("{path} invariant[{index}]");
        let id = require_string(invariant, "id", &context, None)?;
        require_id(id, "INV", path, "invariant", index)?;
        if !invariant_ids.insert(id) {
            return Err(format!("{path} duplicates invariant id `{id}`"));
        }
        require_string(invariant, "text", &context, None)?;
    }

    let acceptances = require_table_array(table, "acceptance", path)?;
    let mut acceptance_ids = BTreeSet::new();
    for (index, acceptance) in acceptances.iter().enumerate() {
        let acceptance = acceptance
            .as_table()
            .ok_or_else(|| format!("{path} acceptance[{index}] must be a table"))?;
        let context = format!("{path} acceptance[{index}]");
        let id = require_string(acceptance, "id", &context, None)?;
        require_id(id, "AC", path, "acceptance", index)?;
        if !acceptance_ids.insert(id) {
            return Err(format!("{path} duplicates acceptance id `{id}`"));
        }
        require_string(acceptance, "text", &context, None)?;
        require_string_array(acceptance, "proof", path, &format!("acceptance[{index}]"))?;
    }

    let integrations = require_table_array(table, "integration", path)?;
    for (index, integration) in integrations.iter().enumerate() {
        let integration = integration
            .as_table()
            .ok_or_else(|| format!("{path} integration[{index}] must be a table"))?;
        let context = format!("{path} integration[{index}]");
        require_string(integration, "surface", &context, None)?;
        require_string(integration, "expected", &context, None)?;
    }

    let risks = require_table_array(table, "risk", path)?;
    let mut risk_ids = BTreeSet::new();
    for (index, risk) in risks.iter().enumerate() {
        let risk = risk
            .as_table()
            .ok_or_else(|| format!("{path} risk[{index}] must be a table"))?;
        let context = format!("{path} risk[{index}]");
        let id = require_string(risk, "id", &context, None)?;
        require_id(id, "RISK", path, "risk", index)?;
        if !risk_ids.insert(id) {
            return Err(format!("{path} duplicates risk id `{id}`"));
        }
        require_string(risk, "mitigation", &context, None)?;
    }

    let rollback = require_table(table, "rollback", path)?;
    require_string(rollback, "strategy", path, None)?;

    for field in [
        "dependencies",
        "blockers",
        "affected_files",
        "affected_symbols",
        "linked_specs",
        "linked_adrs",
    ] {
        if table.contains_key(field) {
            require_string_array(table, field, path, "work spec")?;
        }
    }
    if let Some(compatibility) = table.get("compatibility") {
        let compatibility = compatibility
            .as_table()
            .ok_or_else(|| format!("{path} compatibility must be a table"))?;
        require_string(compatibility, "posture", path, None)?;
    }
    if let Some(delivery) = table.get("delivery") {
        let delivery = delivery
            .as_table()
            .ok_or_else(|| format!("{path} delivery must be a table"))?;
        for field in ["pr", "closeout"] {
            if let Some(value) = delivery.get(field) {
                let link = value.as_str().ok_or_else(|| {
                    format!("{path} delivery.{field} must be a string URL when present")
                })?;
                if !link.is_empty() {
                    validate_https_url(link, path, &format!("delivery.{field}"))?;
                }
            }
        }
    }

    Ok(())
}

fn require_string<'a>(
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
        return Err(format!(
            "{path} field `{field}` must equal `{expected}`, got `{value}`"
        ));
    }
    Ok(value)
}

fn require_known_string<'a>(
    table: &'a toml::value::Table,
    field: &str,
    path: &str,
    allowed: &[&str],
) -> Result<&'a str, String> {
    let value = require_string(table, field, path, None)?;
    if allowed.contains(&value) {
        Ok(value)
    } else {
        Err(format!(
            "{path} field `{field}` must be one of [{}], got `{value}`",
            allowed.join(", ")
        ))
    }
}

fn require_table<'a>(
    table: &'a toml::value::Table,
    field: &str,
    path: &str,
) -> Result<&'a toml::value::Table, String> {
    table
        .get(field)
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{path} field `{field}` must be a table"))
}

fn require_table_array<'a>(
    table: &'a toml::value::Table,
    field: &str,
    path: &str,
) -> Result<&'a Vec<toml::Value>, String> {
    let values = table
        .get(field)
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{path} field `{field}` must be an array of tables"))?;
    if values.is_empty() {
        return Err(format!("{path} field `{field}` must not be empty"));
    }
    for (index, value) in values.iter().enumerate() {
        if !value.is_table() {
            return Err(format!("{path} field `{field}[{index}]` must be a table"));
        }
    }
    Ok(values)
}

fn require_string_array(
    table: &toml::value::Table,
    field: &str,
    path: &str,
    context: &str,
) -> Result<(), String> {
    let values = table
        .get(field)
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{path} {context}.{field} must be an array of strings"))?;
    for (index, value) in values.iter().enumerate() {
        let string = value
            .as_str()
            .ok_or_else(|| format!("{path} {context}.{field}[{index}] must be a string"))?;
        if string.trim().is_empty() {
            return Err(format!(
                "{path} {context}.{field}[{index}] must not be empty"
            ));
        }
    }
    Ok(())
}

fn require_id(id: &str, prefix: &str, path: &str, field: &str, index: usize) -> Result<(), String> {
    let expected = format!("{prefix}-");
    if !id.starts_with(&expected) || id.len() == expected.len() {
        return Err(format!(
            "{path} {field}[{index}] id `{id}` must start with `{expected}`"
        ));
    }
    if !id
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.'))
    {
        return Err(format!(
            "{path} {field}[{index}] id `{id}` contains unsupported characters"
        ));
    }
    Ok(())
}

fn validate_issue_url(url: &str, path: &str) -> Result<(), String> {
    let parts: Vec<&str> = url.split('/').collect();
    if parts.len() != 7
        || parts[0] != "https:"
        || !parts[1].is_empty()
        || parts[2] != "github.com"
        || parts[5] != "issues"
        || parts[3].is_empty()
        || parts[4].is_empty()
        || parts[6]
            .parse::<u64>()
            .ok()
            .filter(|number| *number > 0)
            .is_none()
    {
        return Err(format!(
            "{path} field `issue` must be an https://github.com/<owner>/<repo>/issues/<number> URL"
        ));
    }
    Ok(())
}

fn validate_https_url(url: &str, path: &str, field: &str) -> Result<(), String> {
    if !url.starts_with("https://") || url.contains(char::is_whitespace) {
        return Err(format!("{path} field `{field}` must be an HTTPS URL"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_work_spec;

    #[test]
    fn rejects_scheduler_fields() -> Result<(), String> {
        let value: toml::Value = toml::from_str(&valid_spec().replace(
            "schema_version = \"1\"",
            "schema_version = \"1\"\npriority = \"high\"",
        ))
        .map_err(|error| format!("valid test TOML: {error}"))?;
        let error = match validate_work_spec(&value, "test.toml") {
            Ok(()) => return Err("scheduler field unexpectedly passed".to_string()),
            Err(error) => error,
        };
        if !error.contains("scheduling field `priority`") {
            return Err(format!("unexpected scheduler-field error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn rejects_duplicate_acceptance_ids() -> Result<(), String> {
        let duplicate = format!(
            "{}\n[[acceptance]]\nid = \"AC-1\"\ntext = \"Repeat\"\nproof = [\"rtk cargo test -p xtask\"]\n",
            valid_spec()
        );
        let value: toml::Value =
            toml::from_str(&duplicate).map_err(|error| format!("valid test TOML: {error}"))?;
        let error = match validate_work_spec(&value, "test.toml") {
            Ok(()) => return Err("duplicate acceptance id unexpectedly passed".to_string()),
            Err(error) => error,
        };
        if !(error.contains("acceptance") && error.contains("AC-1")) {
            return Err(format!("unexpected duplicate-id error: {error}"));
        }
        Ok(())
    }

    fn valid_spec() -> String {
        r#"schema_version = "1"
issue = "https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1900"
kind = "correctness"
objective = "Define a bounded work-spec contract."
user_outcome = "Controllers and builders share stable acceptance IDs."
claim_boundary = "This structures intent; it does not prove implementation correctness."

[scope]
included = ["The work-spec schema"]
excluded = ["Scheduling the repository portfolio"]

[[invariant]]
id = "INV-1"
text = "The artifact must not choose the next issue."

[[acceptance]]
id = "AC-1"
text = "Run the checker."
proof = ["rtk cargo test -p xtask"]

[[integration]]
surface = "pr_body"
expected = "The PR can report stable acceptance IDs."

[[risk]]
id = "RISK-1"
mitigation = "Keep validation offline and structural."

[rollback]
strategy = "Remove the example and supersede the schema through a linked spec change."
"#
        .to_string()
    }
}
