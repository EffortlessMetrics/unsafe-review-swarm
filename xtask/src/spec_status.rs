use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::{
    SOURCE_OF_TRUTH_INDEX, markdown, markdown_files, markdown_table_columns, parse_toml_file,
    read_to_string, require_known, source_truth_index_ids, workspace_path,
};

pub(crate) const DASHBOARD: &str = "docs/specs/UNSAFE-REVIEW-SPEC-STATUS.md";
const HEADER: &[&str] = &[
    "Spec",
    "Status",
    "Implementation state",
    "Proof commands",
    "Last touched",
    "Notes",
];
const LIFECYCLE_STATUSES: &[&str] = &["accepted", "draft", "proposed"];
const XTASK_COMMANDS: &[&str] = &[
    "check-advisory-artifacts",
    "check-calibration",
    "check-ci-lanes",
    "check-corpus-backstop-schema",
    "check-detector-contracts",
    "check-doc-artifacts",
    "check-docs",
    "check-docs-automation",
    "check-dogfood",
    "check-first-hour",
    "check-first-pr-artifacts",
    "check-goals",
    "check-manual-candidate-examples",
    "check-package-boundary",
    "check-pr",
    "check-policy",
    "check-public-surfaces",
    "check-source-sync",
    "check-spec-status",
    "source-divergence",
];

pub(crate) fn check() -> Result<(), String> {
    let rows = check_dashboard_impl()?;
    println!("check-spec-status: ok ({rows} rows)");
    Ok(())
}

pub(crate) fn check_dashboard_impl() -> Result<usize, String> {
    let source = workspace_path(DASHBOARD);
    let text = read_to_string(&source)?;
    let rows = rows_from_text(&text)?;
    if rows.is_empty() {
        return Err(format!("{DASHBOARD} must list at least one spec row"));
    }

    let mut seen = BTreeSet::new();
    for row in &rows {
        if !seen.insert(row.spec_id.clone()) {
            return Err(format!(
                "{DASHBOARD} contains duplicate row for `{}`",
                row.spec_id
            ));
        }
        let Some(spec_file) = spec_file_path_for_id(&row.spec_id)? else {
            return Err(format!(
                "{DASHBOARD} references `{}` but no matching docs/specs file exists",
                row.spec_id
            ));
        };
        let status = lifecycle_status(&row.status);
        require_known(&status, LIFECYCLE_STATUSES, DASHBOARD, "status")?;
        let spec_status = file_lifecycle_status(&spec_file)?;
        require_known(
            &spec_status,
            LIFECYCLE_STATUSES,
            &spec_file.display().to_string(),
            "status",
        )?;
        check_lifecycle_match(
            &row.spec_id,
            &status,
            &spec_status,
            &spec_file.display().to_string(),
        )?;
        if row.implementation_state.trim().is_empty() {
            return Err(format!(
                "{DASHBOARD} row `{}` must describe implementation state",
                row.spec_id
            ));
        }
        if row.notes.trim().is_empty() {
            return Err(format!(
                "{DASHBOARD} row `{}` must include notes",
                row.spec_id
            ));
        }
        if !is_iso_date(&row.last_touched) {
            return Err(format!(
                "{DASHBOARD} row `{}` has invalid Last touched date `{}`",
                row.spec_id, row.last_touched
            ));
        }
        check_proof_commands(&row.spec_id, &row.proof_commands)?;
    }

    let source_index = parse_toml_file(&workspace_path(SOURCE_OF_TRUTH_INDEX))?;
    let indexed_artifact_ids = source_truth_index_ids(&source_index, "artifact")?;
    for id in indexed_artifact_ids
        .iter()
        .filter(|id| id.starts_with("UNSAFE-REVIEW-SPEC-"))
    {
        if !seen.contains(id) {
            return Err(format!(
                "{DASHBOARD} is missing source-of-truth indexed spec `{id}`"
            ));
        }
    }

    Ok(rows.len())
}

#[derive(Debug)]
pub(crate) struct SpecStatusRow {
    pub(crate) spec_id: String,
    pub(crate) status: String,
    implementation_state: String,
    proof_commands: String,
    pub(crate) last_touched: String,
    notes: String,
}

pub(crate) fn rows_from_text(text: &str) -> Result<Vec<SpecStatusRow>, String> {
    let mut rows = Vec::new();
    let mut in_table = false;
    let mut saw_header = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            if in_table {
                break;
            }
            continue;
        }
        let columns = markdown_table_columns(trimmed);
        if columns.len() != HEADER.len() {
            if in_table {
                return Err(format!(
                    "{DASHBOARD} has row with {} columns, expected {}: {trimmed}",
                    columns.len(),
                    HEADER.len()
                ));
            }
            continue;
        }
        let columns = columns.into_iter().map(str::trim).collect::<Vec<_>>();
        if columns == HEADER {
            in_table = true;
            saw_header = true;
            continue;
        }
        if in_table && is_markdown_separator_row(&columns) {
            continue;
        }
        if in_table {
            let spec_id = spec_id_from_status_cell(columns[0]).ok_or_else(|| {
                format!("{DASHBOARD} row is missing backticked spec id: {trimmed}")
            })?;
            rows.push(SpecStatusRow {
                spec_id,
                status: columns[1].to_string(),
                implementation_state: columns[2].to_string(),
                proof_commands: columns[3].to_string(),
                last_touched: columns[4].to_string(),
                notes: columns[5].to_string(),
            });
        }
    }
    if !saw_header {
        return Err(format!(
            "{DASHBOARD} is missing expected status table header"
        ));
    }
    Ok(rows)
}

fn is_markdown_separator_row(columns: &[&str]) -> bool {
    columns.iter().all(|column| {
        let value = column.trim();
        !value.is_empty() && value.chars().all(|ch| matches!(ch, '-' | ':' | ' '))
    })
}

fn spec_id_from_status_cell(cell: &str) -> Option<String> {
    let marker = "`UNSAFE-REVIEW-SPEC-";
    let start = cell.find(marker)? + 1;
    let rest = &cell[start..];
    let end = rest.find('`')?;
    Some(rest[..end].to_string())
}

fn spec_file_path_for_id(spec_id: &str) -> Result<Option<PathBuf>, String> {
    for path in markdown_files(&workspace_path("docs/specs"))? {
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            return Err(format!("non-UTF-8 spec file path: {}", path.display()));
        };
        if name.starts_with(spec_id) && name.ends_with(".md") {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

fn file_lifecycle_status(path: &Path) -> Result<String, String> {
    let text = read_to_string(path)?;
    lifecycle_status_from_text(&text, &path.display().to_string())
}

pub(crate) fn lifecycle_status_from_text(text: &str, source: &str) -> Result<String, String> {
    for line in text.lines() {
        let trimmed = line.trim();
        let status = trimmed
            .strip_prefix("Status:")
            .or_else(|| trimmed.strip_prefix("- Status:"));
        if let Some(status) = status {
            return Ok(lifecycle_status(status));
        }
    }
    Err(format!("{source} must include a Status header"))
}

fn lifecycle_status(status: &str) -> String {
    status
        .trim()
        .split([',', ' '])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
}

pub(crate) fn check_lifecycle_match(
    spec_id: &str,
    dashboard_status: &str,
    spec_status: &str,
    source: &str,
) -> Result<(), String> {
    if dashboard_status == spec_status {
        return Ok(());
    }
    Err(format!(
        "{DASHBOARD} row `{spec_id}` status `{dashboard_status}` must match {source} Status lifecycle `{spec_status}`"
    ))
}

pub(crate) fn check_proof_commands(spec_id: &str, proof_commands: &str) -> Result<(), String> {
    let spans = markdown::code_spans(proof_commands);
    let mut xtask_commands = 0usize;
    for span in spans {
        let Some(command) = span.strip_prefix("cargo run --locked -p xtask -- ") else {
            continue;
        };
        let Some(command_name) = command.split_whitespace().next() else {
            return Err(format!(
                "{DASHBOARD} row `{spec_id}` has empty xtask proof command"
            ));
        };
        xtask_commands += 1;
        require_known(command_name, XTASK_COMMANDS, DASHBOARD, "proof command")?;
    }
    if xtask_commands == 0 {
        return Err(format!(
            "{DASHBOARD} row `{spec_id}` must include at least one `cargo run --locked -p xtask -- ...` proof command"
        ));
    }
    Ok(())
}

fn is_iso_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(idx, byte)| idx == 4 || idx == 7 || byte.is_ascii_digit())
}
