//! Stance and spec-coverage check gates.
//!
//! Extracted from main.rs as part of #1806 (xtask modularization).

use crate::{GateReport, parse_toml_file, require_toml_string};
use std::path::Path;

pub(crate) const STANCE_DECISIONS_LEDGER: &str = "policy/stance-decisions.toml";
pub(crate) const SPEC_COVERAGE_LEDGER: &str = "policy/spec-coverage.toml";

/// Pure (no file I/O) evaluation of a parsed stance-decisions ledger value.
///
/// Blocking findings: malformed schema, missing/empty identity, duplicate id, missing required
/// scalars (summary/rationale/owner/linked_spec), missing or empty linked_tests array.
/// A non-empty `proof_gap` is a tracked exception iff the stance also carries non-empty `owner`
/// AND non-empty `review_after`; otherwise it is a blocking finding.
pub(crate) fn evaluate_stance_decisions(
    value: &toml::Value,
    path: &str,
) -> Result<GateReport, String> {
    let mut report = GateReport {
        tracked: Vec::new(),
        blocking: Vec::new(),
    };

    let stances: &[toml::Value] = match value.get("stance") {
        None => &[],
        Some(v) => v
            .as_array()
            .ok_or_else(|| format!("{path} `stance` must be an array"))?,
    };

    let mut seen_ids: Vec<String> = Vec::new();

    for (idx, entry) in stances.iter().enumerate() {
        let table = entry
            .as_table()
            .ok_or_else(|| format!("{path} stance[{idx}] must be a table"))?;

        // Hard-require identity field `id` — blocking.
        let id = match table.get("id").and_then(toml::Value::as_str) {
            Some(s) if !s.trim().is_empty() => s.to_string(),
            _ => {
                report
                    .blocking
                    .push(format!("{path} stance[{idx}]: missing or empty `id`"));
                format!("<stance[{idx}]>")
            }
        };

        // Duplicate id — blocking.
        if seen_ids.contains(&id) {
            report.blocking.push(format!("stance `{id}`: duplicate id"));
        } else {
            seen_ids.push(id.clone());
        }

        // Required descriptive fields — blocking.
        for key in &["summary", "rationale", "owner", "linked_spec"] {
            match table.get(*key).and_then(toml::Value::as_str) {
                None => report
                    .blocking
                    .push(format!("stance `{id}`: missing `{key}`")),
                Some(s) if s.trim().is_empty() => {
                    report
                        .blocking
                        .push(format!("stance `{id}`: `{key}` is empty"));
                }
                _ => {}
            }
        }

        // linked_tests: required non-empty array — blocking.
        match table.get("linked_tests") {
            None => {
                report
                    .blocking
                    .push(format!("stance `{id}`: missing `linked_tests`"));
            }
            Some(arr_val) => match arr_val.as_array() {
                None => report
                    .blocking
                    .push(format!("stance `{id}`: `linked_tests` must be an array")),
                Some(arr) if arr.is_empty() => {
                    report
                        .blocking
                        .push(format!("stance `{id}`: `linked_tests` is empty"));
                }
                _ => {}
            },
        }

        // proof_gap: tracked exception iff owner + review_after are also non-empty; else blocking.
        if let Some(gap_str) = table
            .get("proof_gap")
            .and_then(toml::Value::as_str)
            .filter(|s| !s.trim().is_empty())
        {
            let owner = table
                .get("owner")
                .and_then(toml::Value::as_str)
                .map(|s| s.trim())
                .unwrap_or("")
                .to_string();
            let review_after = table
                .get("review_after")
                .and_then(toml::Value::as_str)
                .map(|s| s.trim())
                .unwrap_or("")
                .to_string();
            if !owner.is_empty() && !review_after.is_empty() {
                report.tracked.push(format!(
                    "stance `{id}`: proof_gap — {gap_str} (owner: {owner}, review_after: {review_after})"
                ));
            } else {
                report.blocking.push(format!(
                    "stance `{id}`: proof_gap requires non-empty owner + review_after to be a tracked exception"
                ));
            }
        }
    }

    Ok(report)
}

/// Enforcing gate: validates ledger shape of `policy/stance-decisions.toml`.
///
/// Structural violations and undocumented proof gaps are blocking. A stance with a non-empty
/// `proof_gap` passes only when it also carries non-empty `owner` and `review_after` (the
/// documented-gap path), which is printed as a tracked exception. There is no tracked-exception
/// path for stances that are missing required fields.
pub(crate) fn check_stance_decisions() -> Result<(), String> {
    let path = STANCE_DECISIONS_LEDGER;
    let value = parse_toml_file(Path::new(path))?;
    require_toml_string(&value, "schema_version", path)?;

    let num_stances = value
        .get("stance")
        .and_then(toml::Value::as_array)
        .map(|v| v.len())
        .unwrap_or(0);

    let report = evaluate_stance_decisions(&value, path)?;

    for f in &report.tracked {
        println!("{f} (tracked exception)");
    }
    for f in &report.blocking {
        println!("{f}");
    }

    if report.blocking.is_empty() {
        println!(
            "check-stance-decisions: ok ({num_stances} stances, {} tracked exception(s))",
            report.tracked.len()
        );
        Ok(())
    } else {
        Err(format!(
            "check-stance-decisions: {} blocking finding(s)",
            report.blocking.len()
        ))
    }
}

/// Pure (no file I/O) evaluation of the stance-coverage index in a parsed stance-decisions ledger.
///
/// For each stance, a PASS requires at least one non-empty corpus-evidence link: a non-empty
/// `fixtures` array, a non-empty `dogfood_targets` array, a non-empty `pr_corpus_cases` array,
/// or a non-empty `surfaces` array.  If none of those are present, the stance is blocking UNLESS
/// it carries a non-empty `coverage_gap` AND non-empty `owner` AND non-empty `review_after` —
/// in which case it is a tracked coverage gap (warn, pass).  Malformed entries are always blocking.
pub(crate) fn evaluate_stance_coverage(
    value: &toml::Value,
    path: &str,
) -> Result<GateReport, String> {
    let mut report = GateReport {
        tracked: Vec::new(),
        blocking: Vec::new(),
    };

    let stances: &[toml::Value] = match value.get("stance") {
        None => &[],
        Some(v) => v
            .as_array()
            .ok_or_else(|| format!("{path} `stance` must be an array"))?,
    };

    for (idx, entry) in stances.iter().enumerate() {
        let table = match entry.as_table() {
            Some(t) => t,
            None => {
                report
                    .blocking
                    .push(format!("{path} stance[{idx}]: must be a table"));
                continue;
            }
        };

        // Resolve the stance id for diagnostic messages; missing id is itself a structural error
        // but we still need a label for subsequent messages.
        let id = table
            .get("id")
            .and_then(toml::Value::as_str)
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("<stance[{idx}]>"));

        // Helper: does the table contain a non-empty string array under `key`?
        let has_non_empty_array = |key: &str| -> bool {
            table
                .get(key)
                .and_then(toml::Value::as_array)
                .map(|arr| !arr.is_empty())
                .unwrap_or(false)
        };

        let has_coverage = has_non_empty_array("fixtures")
            || has_non_empty_array("dogfood_targets")
            || has_non_empty_array("pr_corpus_cases")
            || has_non_empty_array("surfaces");

        if has_coverage {
            // Stance is covered — nothing to record.
            continue;
        }

        // No corpus evidence link present; check for a tracked coverage_gap exception.
        let gap = table
            .get("coverage_gap")
            .and_then(toml::Value::as_str)
            .map(|s| s.trim())
            .unwrap_or("")
            .to_string();

        if gap.is_empty() {
            report.blocking.push(format!(
                "stance `{id}`: no corpus evidence (fixtures / dogfood_targets / pr_corpus_cases / surfaces) \
                 and no coverage_gap; add real evidence or a tracked gap with owner + review_after"
            ));
            continue;
        }

        // Has a non-empty coverage_gap — valid only with owner + review_after.
        let owner = table
            .get("owner")
            .and_then(toml::Value::as_str)
            .map(|s| s.trim())
            .unwrap_or("")
            .to_string();
        let review_after = table
            .get("review_after")
            .and_then(toml::Value::as_str)
            .map(|s| s.trim())
            .unwrap_or("")
            .to_string();

        if !owner.is_empty() && !review_after.is_empty() {
            report.tracked.push(format!(
                "stance `{id}`: coverage_gap — {gap} (owner: {owner}, review_after: {review_after})"
            ));
        } else {
            report.blocking.push(format!(
                "stance `{id}`: coverage_gap requires non-empty owner + review_after to be a tracked exception"
            ));
        }
    }

    Ok(report)
}

/// Informational-then-enforcing gate: validates that every stance in
/// `policy/stance-decisions.toml` has at least one corpus-evidence link.
///
/// A stance with no evidence link is blocking unless it carries a `coverage_gap` with
/// non-empty `owner` and `review_after`, in which case it is a tracked coverage gap and
/// printed as "(tracked coverage gap)".  Structural errors are always blocking.
pub(crate) fn check_stance_coverage() -> Result<(), String> {
    let path = STANCE_DECISIONS_LEDGER;
    let value = parse_toml_file(Path::new(path))?;
    require_toml_string(&value, "schema_version", path)?;

    let num_stances = value
        .get("stance")
        .and_then(toml::Value::as_array)
        .map(|v| v.len())
        .unwrap_or(0);

    let report = evaluate_stance_coverage(&value, path)?;

    for f in &report.tracked {
        println!("{f} (tracked coverage gap)");
    }
    for f in &report.blocking {
        println!("{f}");
    }

    if report.blocking.is_empty() {
        println!(
            "check-stance-coverage: ok ({num_stances} stances, {} tracked coverage gap(s))",
            report.tracked.len()
        );
        Ok(())
    } else {
        Err(format!(
            "check-stance-coverage: {} blocking finding(s)",
            report.blocking.len()
        ))
    }
}

/// Pure (no file I/O) evaluation of a parsed spec-coverage ledger value.
///
/// Blocking findings: malformed schema, missing/empty identity, duplicate name, missing required
/// scalars (canonical_source), missing or empty surfaces array, and single_truth=false (always
/// blocking — a single-truth violation is a defect to fix, not a gap to ledger; there is no
/// tracked-exception path for spec-coverage).
pub(crate) fn evaluate_spec_coverage(
    value: &toml::Value,
    path: &str,
) -> Result<GateReport, String> {
    let mut report = GateReport {
        tracked: Vec::new(),
        blocking: Vec::new(),
    };

    let fields: &[toml::Value] = match value.get("field") {
        None => &[],
        Some(v) => v
            .as_array()
            .ok_or_else(|| format!("{path} `field` must be an array"))?,
    };

    let mut seen_names: Vec<String> = Vec::new();

    for (idx, entry) in fields.iter().enumerate() {
        let table = entry
            .as_table()
            .ok_or_else(|| format!("{path} field[{idx}] must be a table"))?;

        // Hard-require identity field `name` — blocking.
        let name = match table.get("name").and_then(toml::Value::as_str) {
            Some(s) if !s.trim().is_empty() => s.to_string(),
            _ => {
                report
                    .blocking
                    .push(format!("{path} field[{idx}]: missing or empty `name`"));
                format!("<field[{idx}]>")
            }
        };

        // Duplicate name — blocking.
        if seen_names.contains(&name) {
            report
                .blocking
                .push(format!("field `{name}`: duplicate name"));
        } else {
            seen_names.push(name.clone());
        }

        // canonical_source: required non-empty scalar — blocking.
        match table.get("canonical_source").and_then(toml::Value::as_str) {
            None => report
                .blocking
                .push(format!("field `{name}`: missing `canonical_source`")),
            Some(s) if s.trim().is_empty() => {
                report
                    .blocking
                    .push(format!("field `{name}`: `canonical_source` is empty"));
            }
            _ => {}
        }

        // surfaces: required non-empty array — blocking.
        match table.get("surfaces") {
            None => {
                report
                    .blocking
                    .push(format!("field `{name}`: missing `surfaces`"));
            }
            Some(arr_val) => match arr_val.as_array() {
                None => report
                    .blocking
                    .push(format!("field `{name}`: `surfaces` must be an array")),
                Some(arr) if arr.is_empty() => {
                    report
                        .blocking
                        .push(format!("field `{name}`: `surfaces` is empty"));
                }
                _ => {}
            },
        }

        // single_truth=false is always blocking — a single-truth violation is a defect, not a gap.
        // There is no tracked-exception path for spec-coverage.
        if let Some(false) = table.get("single_truth").and_then(toml::Value::as_bool) {
            let note = table
                .get("note")
                .and_then(toml::Value::as_str)
                .unwrap_or("");
            report
                .blocking
                .push(format!("field `{name}`: single_truth=false ({note})"));
        }
    }

    Ok(report)
}

/// Enforcing gate: validates ledger shape of `policy/spec-coverage.toml`.
///
/// Structural violations and single_truth=false fields are always blocking — a single-truth
/// violation is a defect to fix, not a gap to ledger. There is no tracked-exception path
/// for spec-coverage findings.
pub(crate) fn check_spec_coverage() -> Result<(), String> {
    let path = SPEC_COVERAGE_LEDGER;
    let value = parse_toml_file(Path::new(path))?;
    require_toml_string(&value, "schema_version", path)?;

    let num_fields = value
        .get("field")
        .and_then(toml::Value::as_array)
        .map(|v| v.len())
        .unwrap_or(0);

    let report = evaluate_spec_coverage(&value, path)?;

    for f in &report.blocking {
        println!("{f}");
    }

    if report.blocking.is_empty() {
        println!("check-spec-coverage: ok ({num_fields} fields, 0 blocking finding(s))");
        Ok(())
    } else {
        Err(format!(
            "check-spec-coverage: {} blocking finding(s)",
            report.blocking.len()
        ))
    }
}
