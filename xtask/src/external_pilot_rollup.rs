//! Deterministic maintainer-facing aggregation for committed external pilots.

use std::collections::BTreeMap;
use std::fs;

use serde_json::json;

const JSON_PATH: &str = "docs/dogfood/reports/external-pilot-usefulness-rollup.json";
const MARKDOWN_PATH: &str = "docs/dogfood/reports/external-pilot-usefulness-rollup.md";

pub(crate) fn write() -> Result<(), String> {
    let (json_text, markdown) = generate()?;
    let json_path = crate::workspace_path(JSON_PATH);
    let markdown_path = crate::workspace_path(MARKDOWN_PATH);
    let report_dir = json_path
        .parent()
        .ok_or_else(|| format!("{JSON_PATH} has no parent directory"))?;
    fs::create_dir_all(report_dir)
        .map_err(|err| format!("create pilot report directory failed: {err}"))?;
    fs::write(&json_path, json_text).map_err(|err| format!("write {JSON_PATH} failed: {err}"))?;
    fs::write(&markdown_path, markdown)
        .map_err(|err| format!("write {MARKDOWN_PATH} failed: {err}"))?;
    println!("external-pilot-rollup: wrote {JSON_PATH} and {MARKDOWN_PATH}");
    Ok(())
}

pub(crate) fn check() -> Result<(), String> {
    let (expected_json, expected_markdown) = generate()?;
    let actual_json = crate::read_to_string(&crate::workspace_path(JSON_PATH))?;
    let actual_markdown = crate::read_to_string(&crate::workspace_path(MARKDOWN_PATH))?;
    if actual_json != expected_json {
        return Err(format!(
            "{JSON_PATH} is stale; run `cargo run --locked -p xtask -- external-pilot-rollup`"
        ));
    }
    if actual_markdown != expected_markdown {
        return Err(format!(
            "{MARKDOWN_PATH} is stale; run `cargo run --locked -p xtask -- external-pilot-rollup`"
        ));
    }
    Ok(())
}

fn generate() -> Result<(String, String), String> {
    let paths = super::external_pilots::receipt_paths_for_rollup()?;
    let mut projects = BTreeMap::<String, usize>::new();
    let mut sources = BTreeMap::<String, usize>::new();
    let mut judgments = BTreeMap::<String, usize>::new();
    let mut movement = BTreeMap::<&str, i64>::new();
    let mut totals = BTreeMap::<&str, i64>::new();
    let mut setup_friction = 0usize;
    let mut artifact_friction = 0usize;
    let mut quiet = 0usize;
    let mut public_action = 0usize;
    let mut inherited = 0usize;
    let mut improved_or_resolved = 0usize;

    for path in &paths {
        let display = path.to_string_lossy().replace('\\', "/");
        let value = crate::parse_toml_file(path)?;
        let table = value
            .as_table()
            .ok_or_else(|| format!("{display} root must be a TOML table"))?;
        let project = required_string(table, "repository", &display)?;
        *projects.entry(project.to_string()).or_default() += 1;
        let source = required_string(table, "source", &display)?;
        *sources.entry(source.to_string()).or_default() += 1;
        let inventory = required_table(table, "card_inventory", &display)?;
        let total_cards = required_integer(inventory, "total_cards", &display)?;
        *totals.entry("cards").or_default() += total_cards;
        let plan = required_table(table, "comment_plan", &display)?;
        *totals.entry("selected_comments").or_default() +=
            required_integer(plan, "selected_count", &display)?;
        *totals.entry("omitted_comments").or_default() +=
            required_integer(plan, "not_selected_count", &display)?;
        if total_cards == 0 {
            quiet += 1;
        }
        if source == "public-action" {
            public_action += 1;
        }
        let gate = required_table(table, "gate_summary", &display)?;
        for key in [
            "new_gaps",
            "worsened_gaps",
            "improved_gaps",
            "resolved_gaps",
            "inherited_gaps",
        ] {
            let value = required_integer(gate, key, &display)?;
            *movement.entry(key).or_default() += value;
            if key == "inherited_gaps" && value > 0 {
                inherited += 1;
            }
            if matches!(key, "improved_gaps" | "resolved_gaps") && value > 0 {
                improved_or_resolved += 1;
            }
        }
        let rows = table
            .get("judgments")
            .and_then(toml::Value::as_array)
            .ok_or_else(|| format!("{display} is missing judgments"))?;
        for row in rows {
            let row = row
                .as_table()
                .ok_or_else(|| format!("{display} judgment is not a table"))?;
            let label = required_string(row, "label", &display)?;
            *judgments.entry(label.to_string()).or_default() += 1;
            setup_friction += usize::from(label == "setup_friction");
            artifact_friction += usize::from(label == "artifact_friction");
        }
    }

    let json_value = json!({
        "schema_version": "external-pilot-usefulness-rollup/v1",
        "receipt_count": paths.len(),
        "project_count": projects.len(),
        "projects": projects,
        "sources": sources,
        "coverage": {
            "quiet": quiet > 0,
            "inherited_only": inherited > 0,
            "new_gap": movement.get("new_gaps").copied().unwrap_or(0) > 0,
            "resolved_or_improved": improved_or_resolved > 0,
            "public_action": public_action > 0,
        },
        "movement": movement,
        "totals": totals,
        "judgments": judgments,
        "friction": {"setup": setup_friction, "artifact": artifact_friction},
        "next_lane": if setup_friction >= artifact_friction { "preview-only pilot setup" } else { "artifact discoverability" },
        "trust_boundary": "Diagnostic external-pilot usefulness evidence only; not calibrated precision or recall, not proof, not UB-free, not Miri-clean, not site-execution evidence, not policy readiness, and not a merge verdict.",
    });
    let json_text = serde_json::to_string_pretty(&json_value)
        .map_err(|err| format!("serialize pilot rollup failed: {err}"))?
        + "\n";
    Ok((json_text, render_markdown(&json_value)))
}

fn render_markdown(value: &serde_json::Value) -> String {
    let count = value["receipt_count"].as_u64().unwrap_or(0);
    let projects = value["project_count"].as_u64().unwrap_or(0);
    let coverage = &value["coverage"];
    let mut out = String::from("# External Pilot Usefulness Rollup\n\n");
    out.push_str(
        "Generated from committed `docs/dogfood/pilots/*.toml` receipts; do not edit by hand.\n\n",
    );
    out.push_str(
        "This is diagnostic product-usefulness evidence, not calibration or a safety claim.\n\n",
    );
    out.push_str("## Current readout\n\n| Measure | Result |\n|---|---:|\n");
    out.push_str(&format!(
        "| Exact receipts | {count} |\n| Projects | {projects} |\n"
    ));
    for (label, key) in [
        ("Quiet case", "quiet"),
        ("Inherited-only case", "inherited_only"),
        ("New-gap case", "new_gap"),
        ("Resolved/improved case", "resolved_or_improved"),
        ("Public Action case", "public_action"),
    ] {
        out.push_str(&format!(
            "| {label} | {} |\n",
            if coverage[key].as_bool().unwrap_or(false) {
                "yes"
            } else {
                "not yet"
            }
        ));
    }
    out.push_str("\n## Evidence-led next lane\n\n");
    out.push_str(&format!("The current bounded UX follow-up is **{}**, selected from recorded friction counts rather than raw card reduction.\n\n", value["next_lane"].as_str().unwrap_or("unassigned")));
    let missing = missing_matrix_cases(coverage);
    if missing.is_empty() {
        out.push_str("No pilot matrix cases are currently missing. This report does not convert matrix coverage into a product-readiness claim.\n");
    } else {
        out.push_str(&format!(
            "The missing matrix {} {} explicit release work: {}. This report does not convert their absence into a positive claim.\n",
            if missing.len() == 1 { "case" } else { "cases" },
            if missing.len() == 1 { "remains" } else { "remain" },
            missing.join(", ")
        ));
    }
    out.push_str("\n## Trust boundary\n\n");
    out.push_str("This is diagnostic external-pilot usefulness evidence only: not calibrated precision or recall, not proof, not UB-free, not Miri-clean, not site-execution evidence, not witness adequacy, not policy readiness, and not a merge verdict.\n");
    out
}

fn missing_matrix_cases(coverage: &serde_json::Value) -> Vec<&'static str> {
    [
        ("quiet", "quiet"),
        ("inherited_only", "inherited-only"),
        ("new_gap", "new-gap"),
        ("resolved_or_improved", "resolved/improved"),
        ("public_action", "a real public Action run"),
    ]
    .into_iter()
    .filter_map(|(key, label)| (!coverage[key].as_bool().unwrap_or(false)).then_some(label))
    .collect()
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

#[cfg(test)]
mod tests {
    use super::render_markdown;
    use serde_json::json;

    #[test]
    fn markdown_lists_only_missing_matrix_cases() -> Result<(), String> {
        let present_and_absent = json!({
            "receipt_count": 10,
            "project_count": 7,
            "coverage": {
                "quiet": true,
                "inherited_only": true,
                "new_gap": true,
                "resolved_or_improved": true,
                "public_action": false
            },
            "next_lane": "preview-only pilot setup"
        });
        let markdown = render_markdown(&present_and_absent);
        let missing = missing_release_work(&markdown)?;
        if missing != "a real public Action run" {
            return Err(format!("unexpected current missing-case list: {missing}"));
        }

        let all_absent = json!({
            "receipt_count": 0,
            "project_count": 0,
            "coverage": {
                "quiet": false,
                "inherited_only": false,
                "new_gap": false,
                "resolved_or_improved": false,
                "public_action": false
            },
            "next_lane": "unassigned"
        });
        let markdown = render_markdown(&all_absent);
        let missing = missing_release_work(&markdown)?;
        let expected =
            "quiet, inherited-only, new-gap, resolved/improved, a real public Action run";
        if missing != expected {
            return Err(format!(
                "unexpected all-absent missing-case list: {missing}"
            ));
        }

        let all_present = json!({
            "receipt_count": 10,
            "project_count": 7,
            "coverage": {
                "quiet": true,
                "inherited_only": true,
                "new_gap": true,
                "resolved_or_improved": true,
                "public_action": true
            },
            "next_lane": "unassigned"
        });
        let markdown = render_markdown(&all_present);
        let expected = "No pilot matrix cases are currently missing. This report does not convert matrix coverage into a product-readiness claim.";
        if !markdown.contains(expected) {
            return Err(format!("missing complete-matrix prose: {markdown}"));
        }
        if markdown.contains("explicit release work:") {
            return Err(format!(
                "complete matrix unexpectedly rendered a missing-case list: {markdown}"
            ));
        }
        Ok(())
    }

    fn missing_release_work(markdown: &str) -> Result<&str, String> {
        let (_, after_prefix) = markdown
            .split_once("explicit release work: ")
            .ok_or_else(|| "missing release-work prefix".to_string())?;
        after_prefix
            .split_once(". This report")
            .map(|(missing, _)| missing)
            .ok_or_else(|| "missing release-work sentence boundary".to_string())
    }
}
