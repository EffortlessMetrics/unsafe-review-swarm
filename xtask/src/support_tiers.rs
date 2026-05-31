#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use std::path::Path;

use crate::{markdown_table_columns, read_to_string, workspace_path};

pub(crate) const SUPPORT_TIERS_DOC: &str = "docs/status/SUPPORT_TIERS.md";
pub(crate) const SUPPORT_SUMMARY_DOC: &str = "docs/status/SUPPORT_SUMMARY.md";
const KNOWN_SUPPORT_TIERS: &[&str] = &["scaffold", "experimental", "planned", "deferred"];
const SUPPORT_PROOF_TERMS: &[&str] = &[
    "test",
    "tests",
    "fixture",
    "fixtures",
    "golden",
    "goldens",
    "e2e",
    "xtask",
    "workflow",
    "handoff",
    "dogfood",
    "parser",
    "renderer",
    "manifest",
    "serde",
    "round-trip",
    "adr",
];
const KNOWN_SUPPORT_SUMMARY_POSTURES: &[&str] = &["Experimental", "Deferred or planned"];
pub(crate) const SUPPORT_SUMMARY_REQUIRED_PHRASES: &[&str] = &[
    "memory-safety proof",
    "UB-free claim",
    "Miri-clean claim",
    "site-execution proof",
    "calibrated policy gate",
    "SUPPORT_TIERS.md",
];

pub(crate) fn check_support_tiers() -> Result<(), String> {
    let path = SUPPORT_TIERS_DOC;
    let text = read_to_string(Path::new(path))?;
    check_support_tiers_text(path, &text)?;
    check_support_summary()?;
    println!("check-support-tiers: ok");
    Ok(())
}

pub(crate) fn check_support_tiers_text(path: &str, text: &str) -> Result<(), String> {
    let mut rows = 0usize;
    for (line_no, line) in text.lines().enumerate() {
        let Some(row) = support_tier_row_from_line(line, path, line_no + 1)? else {
            continue;
        };
        rows += 1;
        if !KNOWN_SUPPORT_TIERS.contains(&row.tier) {
            return Err(format!(
                "{path}:{} uses unknown support tier `{}`",
                line_no + 1,
                row.tier
            ));
        }
        if matches!(row.tier, "scaffold" | "experimental")
            && !support_proof_cell_has_evidence_term(row.proof)
        {
            return Err(format!(
                "{path}:{} proof for `{}` must name concrete evidence such as tests, fixtures, dogfood, workflows, or an ADR",
                line_no + 1,
                row.capability
            ));
        }
    }
    if rows == 0 {
        return Err(format!("{path} has no support-tier rows"));
    }
    Ok(())
}

fn check_support_summary() -> Result<(), String> {
    let path = SUPPORT_SUMMARY_DOC;
    let text = read_to_string(Path::new(path))?;
    check_support_summary_text(path, &text)
}

pub(crate) fn check_support_summary_text(path: &str, text: &str) -> Result<(), String> {
    for phrase in SUPPORT_SUMMARY_REQUIRED_PHRASES {
        if !text.contains(phrase) {
            return Err(format!(
                "{path} must include trust-boundary phrase `{phrase}`"
            ));
        }
    }

    let mut rows = 0usize;
    for (line_no, line) in text.lines().enumerate() {
        let Some(posture) = support_summary_posture_from_row(line) else {
            continue;
        };
        rows += 1;
        if !KNOWN_SUPPORT_SUMMARY_POSTURES.contains(&posture) {
            return Err(format!(
                "{path}:{} uses unknown support summary posture `{posture}`",
                line_no + 1
            ));
        }
    }
    if rows == 0 {
        return Err(format!("{path} has no current-posture rows"));
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn support_tier_from_row(line: &str) -> Option<&str> {
    let Ok(Some(row)) = support_tier_row_from_line(line, "support tier table", 0) else {
        return None;
    };
    Some(row.tier)
}

pub(crate) fn support_capability_from_row(line: &str) -> Option<&str> {
    let Ok(Some(row)) = support_tier_row_from_line(line, "support tier table", 0) else {
        return None;
    };
    Some(row.capability)
}

struct SupportTierRow<'a> {
    capability: &'a str,
    tier: &'a str,
    proof: &'a str,
}

fn support_tier_row_from_line<'a>(
    line: &'a str,
    path: &str,
    line_no: usize,
) -> Result<Option<SupportTierRow<'a>>, String> {
    if !line.starts_with('|') || line.contains("---") || line.contains("Capability") {
        return Ok(None);
    }
    let columns = markdown_table_columns(line);
    if columns.len() != 5 {
        return Err(format!(
            "{path}:{line_no} support-tier rows must have 5 columns, found {}",
            columns.len()
        ));
    }
    for (idx, name) in [
        (0, "Capability"),
        (1, "Tier"),
        (2, "Surface"),
        (3, "Proof"),
        (4, "Known limits"),
    ] {
        reject_placeholder_cell(path, line_no, name, columns[idx])?;
    }
    Ok(Some(SupportTierRow {
        capability: columns[0],
        tier: columns[1],
        proof: columns[3],
    }))
}

fn reject_placeholder_cell(
    path: &str,
    line_no: usize,
    column: &str,
    value: &str,
) -> Result<(), String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty()
        || matches!(
            normalized.as_str(),
            "-" | "n/a" | "na" | "none" | "todo" | "tbd" | "placeholder"
        )
    {
        Err(format!(
            "{path}:{line_no} `{column}` cell must not be empty or placeholder"
        ))
    } else {
        Ok(())
    }
}

fn support_proof_cell_has_evidence_term(proof: &str) -> bool {
    let proof = proof.to_ascii_lowercase();
    SUPPORT_PROOF_TERMS.iter().any(|term| proof.contains(term))
}

pub(crate) fn support_summary_posture_from_row(line: &str) -> Option<&str> {
    if !line.starts_with('|') || line.contains("---") || line.contains("Surface") {
        return None;
    }
    let columns = line
        .split('|')
        .map(str::trim)
        .filter(|column| !column.is_empty())
        .collect::<Vec<_>>();
    (columns.len() == 4).then(|| columns[1])
}

pub(crate) fn support_tier_capabilities() -> Result<BTreeSet<String>, String> {
    let path = workspace_path(SUPPORT_TIERS_DOC);
    let text = read_to_string(&path)?;
    let mut capabilities = BTreeSet::new();
    for line in text.lines() {
        if let Some(capability) = support_capability_from_row(line) {
            capabilities.insert(capability.to_string());
        }
    }
    if capabilities.is_empty() {
        return Err(format!("{} has no support-tier rows", path.display()));
    }
    Ok(capabilities)
}
