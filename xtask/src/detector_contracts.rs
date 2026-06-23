//! Detector-contracts ledger gate (`check-detector-contracts`).
//!
//! Extracted from `main.rs` as part of #1806 (xtask modularization). Validates
//! `policy/detector-contracts.toml`: structural shape, required fields, and
//! the negative-fixtures gap (blocking unless documented as a tracked
//! exception with `proof_gap` + `owner` + `review_after`).

use crate::{GateReport, parse_toml_file, require_toml_string, toml_str_array};
use std::path::Path;

pub(crate) const DETECTOR_CONTRACTS_LEDGER: &str = "policy/detector-contracts.toml";

/// Pure (no file I/O) evaluation of a parsed detector-contracts ledger value.
///
/// Blocking findings: malformed schema, missing/empty identity, duplicate id, empty required
/// arrays (obligations/positive_fixtures/surfaces), non-array where array required,
/// missing required scalars.
/// The negative_fixtures gap is blocking unless the contract carries non-empty `proof_gap` AND
/// non-empty `owner` AND non-empty `review_after` — in that case it is a tracked exception.
pub(crate) fn evaluate_detector_contracts(
    value: &toml::Value,
    path: &str,
) -> Result<GateReport, String> {
    let mut report = GateReport {
        tracked: Vec::new(),
        blocking: Vec::new(),
    };

    // [[contract]] key is absent in the empty scaffold — treat as zero entries.
    let Some(contracts_val) = value.get("contract") else {
        return Ok(report);
    };
    let contracts = contracts_val
        .as_array()
        .ok_or_else(|| format!("{path} `contract` must be an array"))?;

    let mut seen_ids: Vec<String> = Vec::new();

    for (idx, entry) in contracts.iter().enumerate() {
        let table = entry
            .as_table()
            .ok_or_else(|| format!("{path} contract[{idx}] must be a table"))?;

        // Hard-require identity field; use operation_family as the identity per spec.
        let id = match table.get("operation_family").and_then(toml::Value::as_str) {
            Some(s) if !s.trim().is_empty() => s.to_string(),
            _ => {
                report.blocking.push(format!(
                    "{path} contract[{idx}]: missing or empty `operation_family`"
                ));
                format!("<contract[{idx}]>")
            }
        };

        // Duplicate id check — blocking.
        if seen_ids.contains(&id) {
            report.blocking.push(format!(
                "{path} contract `{id}`: duplicate operation_family"
            ));
        } else {
            seen_ids.push(id.clone());
        }

        validate_detector_contract_string_array(
            table,
            &id,
            "obligations",
            "no obligations declared",
            "obligations array is empty",
            &mut report,
        );
        validate_detector_contract_string_array(
            table,
            &id,
            "positive_fixtures",
            "no positive_fixtures declared",
            "positive_fixtures array is empty",
            &mut report,
        );

        // negative_fixtures: empty/absent is blocking UNLESS proof_gap + owner + review_after
        // are all non-empty — then it is a tracked exception.
        let neg_gap = match table.get("negative_fixtures") {
            None => true,
            Some(arr_val) => {
                match toml_str_array(
                    arr_val,
                    path,
                    &format!("contract `{id}` `negative_fixtures`"),
                ) {
                    Ok(values) => values.is_empty(),
                    Err(err) => {
                        report.blocking.push(err);
                        false // already reported as blocking (wrong type or member)
                    }
                }
            }
        };
        if neg_gap {
            let proof_gap = table
                .get("proof_gap")
                .and_then(toml::Value::as_str)
                .map(|s| s.trim())
                .unwrap_or("")
                .to_string();
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
            if !proof_gap.is_empty() && !owner.is_empty() && !review_after.is_empty() {
                report.tracked.push(format!(
                    "contract `{id}`: no negative_fixtures (tracked exception — owner: {owner}, review_after: {review_after})"
                ));
            } else {
                report.blocking.push(format!(
                    "contract `{id}`: no negative_fixtures (add fixtures or document gap with proof_gap + owner + review_after)"
                ));
            }
        }

        validate_detector_contract_string_array(
            table,
            &id,
            "surfaces",
            "no surfaces declared",
            "surfaces array is empty",
            &mut report,
        );
    }

    // Handle optional [[exception]] entries — structural errors are blocking.
    if let Some(exceptions_val) = value.get("exception") {
        let exceptions = exceptions_val
            .as_array()
            .ok_or_else(|| format!("{path} `exception` must be an array"))?;
        let mut seen_exc_ids: Vec<String> = Vec::new();
        for (idx, exc) in exceptions.iter().enumerate() {
            let table = exc
                .as_table()
                .ok_or_else(|| format!("{path} exception[{idx}] must be a table"))?;
            let exc_id = match table.get("id").and_then(toml::Value::as_str) {
                Some(s) if !s.trim().is_empty() => s.to_string(),
                _ => {
                    report
                        .blocking
                        .push(format!("{path} exception[{idx}]: missing or empty `id`"));
                    format!("<exception[{idx}]>")
                }
            };
            if seen_exc_ids.contains(&exc_id) {
                report
                    .blocking
                    .push(format!("{path} exception `{exc_id}`: duplicate id"));
            } else {
                seen_exc_ids.push(exc_id);
            }
        }
    }

    Ok(report)
}

fn validate_detector_contract_string_array(
    table: &toml::map::Map<String, toml::Value>,
    id: &str,
    key: &str,
    missing_message: &str,
    empty_message: &str,
    report: &mut GateReport,
) {
    let Some(value) = table.get(key) else {
        report
            .blocking
            .push(format!("contract `{id}`: {missing_message}"));
        return;
    };

    match toml_str_array(
        value,
        DETECTOR_CONTRACTS_LEDGER,
        &format!("contract `{id}` `{key}`"),
    ) {
        Ok(values) if values.is_empty() => {
            report
                .blocking
                .push(format!("contract `{id}`: {empty_message}"));
        }
        Ok(_) => {}
        Err(err) => report.blocking.push(err),
    }
}

/// Enforcing gate: validates ledger shape of `policy/detector-contracts.toml`.
///
/// Structural violations (malformed TOML, missing identity, duplicate id, empty required arrays)
/// and undocumented negative-fixture gaps are blocking — they fail check-pr. A contract that
/// lacks negative_fixtures passes only when it carries a non-empty `proof_gap`, `owner`, and
/// `review_after` (the documented-gap path), which is printed as a tracked exception.
pub(crate) fn check_detector_contracts() -> Result<(), String> {
    let path = DETECTOR_CONTRACTS_LEDGER;
    let value = parse_toml_file(Path::new(path))?;
    require_toml_string(&value, "schema_version", path)?;

    let num_contracts = value
        .get("contract")
        .and_then(toml::Value::as_array)
        .map(|v| v.len())
        .unwrap_or(0);

    let report = evaluate_detector_contracts(&value, path)?;

    for f in &report.tracked {
        println!("{f} (tracked exception)");
    }
    for f in &report.blocking {
        println!("{f}");
    }

    if report.blocking.is_empty() {
        println!(
            "check-detector-contracts: ok ({num_contracts} contracts, {} tracked exception(s))",
            report.tracked.len()
        );
        Ok(())
    } else {
        Err(format!(
            "check-detector-contracts: {} blocking finding(s)",
            report.blocking.len()
        ))
    }
}
