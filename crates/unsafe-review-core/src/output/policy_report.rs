use crate::api::AnalyzeOutput;
use crate::domain::ReviewClass;
use crate::output::markdown_table;
use serde::Serialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const TRUST_BOUNDARY: &str = "Advisory no-new-debt policy report only; this evaluates existing ReviewCards and policy ledgers, does not execute witnesses, does not prove safety, and does not enforce blocking policy.";

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PolicyReport {
    pub schema_version: String,
    pub tool: String,
    pub mode: String,
    pub policy: String,
    pub audit_date: String,
    pub trust_boundary: String,
    pub summary: PolicyReportSummary,
    pub cards: Vec<PolicyReportCard>,
    pub resolved_baseline: Vec<PolicyLedgerEntry>,
    pub expired_suppressions: Vec<PolicyLedgerEntry>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct PolicyReportSummary {
    pub cards: usize,
    pub new_gaps: usize,
    pub baseline_known: usize,
    pub suppressed: usize,
    pub resolved_baseline: usize,
    pub expired_suppressions: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PolicyReportCard {
    pub card_id: String,
    #[serde(rename = "class")]
    pub class_name: String,
    pub policy_status: String,
    pub missing_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PolicyLedgerEntry {
    pub card_id: String,
    pub owner: Option<String>,
    pub reason: Option<String>,
    pub review_after: Option<String>,
    pub expires: Option<String>,
}

pub(crate) fn evaluate(output: &AnalyzeOutput) -> Result<PolicyReport, String> {
    let audit_date = current_utc_date()?;
    evaluate_with_date(output, &audit_date)
}

fn evaluate_with_date(output: &AnalyzeOutput, audit_date: &str) -> Result<PolicyReport, String> {
    let baseline_entries = ledger_entries(
        &output
            .root
            .join("policy")
            .join("unsafe-review-baseline.toml"),
        LedgerKind::Baseline,
    )?;
    let suppression_entries = ledger_entries(
        &output
            .root
            .join("policy")
            .join("unsafe-review-suppressions.toml"),
        LedgerKind::Suppression,
    )?;
    let current_ids = output
        .cards
        .iter()
        .map(|card| card.id.0.clone())
        .collect::<BTreeSet<_>>();
    let resolved_baseline = baseline_entries
        .into_iter()
        .filter(|entry| !current_ids.contains(&entry.card_id))
        .collect::<Vec<_>>();
    let expired_suppressions = suppression_entries
        .into_iter()
        .filter(|entry| {
            entry
                .expires
                .as_deref()
                .is_some_and(|expires| expires < audit_date)
        })
        .collect::<Vec<_>>();
    let cards = output
        .cards
        .iter()
        .map(|card| PolicyReportCard {
            card_id: card.id.0.clone(),
            class_name: card.class.as_str().to_string(),
            policy_status: policy_status(&card.class).to_string(),
            missing_count: card.missing.len(),
        })
        .collect::<Vec<_>>();
    let summary = PolicyReportSummary {
        cards: output.cards.len(),
        new_gaps: cards
            .iter()
            .filter(|card| card.policy_status == "new_gap")
            .count(),
        baseline_known: cards
            .iter()
            .filter(|card| card.policy_status == "baseline_known")
            .count(),
        suppressed: cards
            .iter()
            .filter(|card| card.policy_status == "suppressed")
            .count(),
        resolved_baseline: resolved_baseline.len(),
        expired_suppressions: expired_suppressions.len(),
    };

    Ok(PolicyReport {
        schema_version: "0.1".to_string(),
        tool: "unsafe-review".to_string(),
        mode: "policy-report".to_string(),
        policy: "advisory".to_string(),
        audit_date: audit_date.to_string(),
        trust_boundary: TRUST_BOUNDARY.to_string(),
        summary,
        cards,
        resolved_baseline,
        expired_suppressions,
    })
}

pub(crate) fn render_json(report: &PolicyReport) -> String {
    match serde_json::to_string_pretty(report) {
        Ok(text) => text,
        Err(err) => format!("{{\n  \"error\": \"policy report serialization failed: {err}\"\n}}"),
    }
}

pub(crate) fn render_markdown(report: &PolicyReport) -> String {
    let mut out = String::new();
    out.push_str("# unsafe-review policy report\n\n");
    out.push_str("Advisory no-new-debt policy report from current ReviewCards and ledgers.\n\n");
    out.push_str("## Summary\n\n");
    out.push_str("| Cards | New gaps | Baseline known | Suppressed | Resolved baseline | Expired suppressions |\n");
    out.push_str("|---:|---:|---:|---:|---:|---:|\n");
    out.push_str(&format!(
        "| {} | {} | {} | {} | {} | {} |\n\n",
        report.summary.cards,
        report.summary.new_gaps,
        report.summary.baseline_known,
        report.summary.suppressed,
        report.summary.resolved_baseline,
        report.summary.expired_suppressions
    ));

    out.push_str("## Current cards\n\n");
    if report.cards.is_empty() {
        out.push_str("No current policy-relevant cards found.\n\n");
    } else {
        out.push_str("| Status | Card | Class | Missing evidence |\n");
        out.push_str("|---|---|---|---:|\n");
        for card in &report.cards {
            out.push_str(&format!(
                "| `{}` | `{}` | `{}` | {} |\n",
                markdown_cell(&card.policy_status),
                markdown_cell(&card.card_id),
                markdown_cell(&card.class_name),
                card.missing_count
            ));
        }
        out.push('\n');
    }

    render_ledger_section(
        &mut out,
        "Resolved baseline entries",
        &report.resolved_baseline,
    );
    render_ledger_section(
        &mut out,
        "Expired suppression entries",
        &report.expired_suppressions,
    );

    out.push_str("## Trust boundary\n\n");
    out.push_str(&report.trust_boundary);
    out.push('\n');
    out
}

fn render_ledger_section(out: &mut String, title: &str, entries: &[PolicyLedgerEntry]) {
    out.push_str("## ");
    out.push_str(title);
    out.push_str("\n\n");
    if entries.is_empty() {
        out.push_str("None.\n\n");
        return;
    }
    out.push_str("| Card | Owner | Review after | Expires | Reason |\n");
    out.push_str("|---|---|---|---|---|\n");
    for entry in entries {
        out.push_str(&format!(
            "| `{}` | {} | {} | {} | {} |\n",
            markdown_cell(&entry.card_id),
            optional_text(entry.owner.as_deref()),
            optional_text(entry.review_after.as_deref()),
            optional_text(entry.expires.as_deref()),
            optional_text(entry.reason.as_deref())
        ));
    }
    out.push('\n');
}

#[derive(Clone, Copy)]
enum LedgerKind {
    Baseline,
    Suppression,
}

fn ledger_entries(path: &Path, kind: LedgerKind) -> Result<Vec<PolicyLedgerEntry>, String> {
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let text =
        fs::read_to_string(path).map_err(|err| format!("read {} failed: {err}", path.display()))?;
    let value = text
        .parse::<toml::Table>()
        .map(toml::Value::Table)
        .map_err(|err| format!("{} is not valid TOML: {err}", path.display()))?;
    let status = value
        .get("status")
        .and_then(toml::Value::as_str)
        .unwrap_or("active");
    if status == "empty" {
        return Ok(Vec::new());
    }
    let entries = value
        .get("entries")
        .and_then(toml::Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    let mut report_entries = Vec::new();
    for entry in entries {
        let Some(entry) = entry.as_table() else {
            continue;
        };
        let Some(card_id) = entry.get("card_id").and_then(toml::Value::as_str) else {
            continue;
        };
        report_entries.push(PolicyLedgerEntry {
            card_id: card_id.to_string(),
            owner: optional_string(entry, "owner"),
            reason: optional_string(entry, "reason"),
            review_after: optional_string(entry, "review_after"),
            expires: match kind {
                LedgerKind::Baseline => None,
                LedgerKind::Suppression => optional_string(entry, "expires"),
            },
        });
    }
    Ok(report_entries)
}

fn optional_string(entry: &toml::map::Map<String, toml::Value>, key: &str) -> Option<String> {
    entry
        .get(key)
        .and_then(toml::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
}

fn policy_status(class: &ReviewClass) -> &'static str {
    match class {
        ReviewClass::BaselineKnown => "baseline_known",
        ReviewClass::Suppressed => "suppressed",
        class if class.is_actionable() => "new_gap",
        _ => "non_actionable",
    }
}

fn optional_text(value: Option<&str>) -> String {
    value
        .filter(|value| !value.is_empty())
        .map(markdown_cell)
        .unwrap_or_else(|| "-".to_string())
}

fn markdown_cell(value: &str) -> String {
    markdown_table::cell(value)
}

fn current_utc_date() -> Result<String, String> {
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("system clock before UNIX_EPOCH: {err}"))?
        .as_secs()
        / 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    Ok(format!("{year:04}-{month:02}-{day:02}"))
}

fn civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = year + if month <= 2 { 1 } else { 0 };
    (year as i32, month as u32, day as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{AnalysisMode, AnalyzeInput, DiffSource, PolicyMode, Scope, analyze};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn policy_report_counts_new_gaps_and_keeps_trust_boundary() -> Result<(), String> {
        let root = fixture_path("raw_pointer_alignment");
        let output = analyze(AnalyzeInput {
            root,
            scope: Scope::Repo,
            diff: DiffSource::NoneRepoScan,
            mode: AnalysisMode::Repo,
            policy: PolicyMode::Advisory,
            include_unchanged_tests: true,
            max_cards: None,
        })?;

        let report = evaluate_with_date(&output, "2026-05-18")?;

        assert_eq!(report.mode, "policy-report");
        assert_eq!(report.policy, "advisory");
        assert_eq!(report.summary.new_gaps, 1);
        assert_eq!(report.summary.baseline_known, 0);
        assert!(report.trust_boundary.contains("does not enforce blocking"));
        Ok(())
    }

    #[test]
    fn policy_report_counts_baseline_suppression_resolved_and_expired() -> Result<(), String> {
        let source = fixture_path("raw_pointer_alignment");
        let root = unique_temp_dir("unsafe-review-policy-report")?;
        copy_dir(&source, &root)?;
        let first = analyze(AnalyzeInput {
            root: root.clone(),
            scope: Scope::Repo,
            diff: DiffSource::NoneRepoScan,
            mode: AnalysisMode::Repo,
            policy: PolicyMode::Advisory,
            include_unchanged_tests: true,
            max_cards: None,
        })?;
        let card_id = first
            .cards
            .first()
            .ok_or_else(|| "fixture produced no card".to_string())?
            .id
            .0
            .clone();
        let policy = root.join("policy");
        fs::create_dir_all(&policy).map_err(|err| format!("create policy failed: {err}"))?;
        fs::write(
            policy.join("unsafe-review-baseline.toml"),
            format!(
                r#"schema_version = "0.1"
status = "active"

[[entries]]
card_id = "{card_id}"
owner = "core/policy"
reason = "accepted current debt"
evidence = "fixture"
review_after = "2026-08-01"

[[entries]]
card_id = "UR-resolved-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
owner = "core/policy"
reason = "resolved debt"
evidence = "fixture"
review_after = "2026-08-01"
"#
            ),
        )
        .map_err(|err| format!("write baseline failed: {err}"))?;
        fs::write(
            policy.join("unsafe-review-suppressions.toml"),
            r#"schema_version = "0.1"
status = "active"

[[entries]]
card_id = "UR-expired-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
owner = "core/policy"
reason = "old false positive"
evidence = "fixture"
expires = "2026-01-01"
"#,
        )
        .map_err(|err| format!("write suppression failed: {err}"))?;
        let output = analyze(AnalyzeInput {
            root: root.clone(),
            scope: Scope::Repo,
            diff: DiffSource::NoneRepoScan,
            mode: AnalysisMode::Repo,
            policy: PolicyMode::Advisory,
            include_unchanged_tests: true,
            max_cards: None,
        })?;

        let report = evaluate_with_date(&output, "2026-05-18")?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp root failed: {err}"))?;
        assert_eq!(report.summary.new_gaps, 0);
        assert_eq!(report.summary.baseline_known, 1);
        assert_eq!(report.summary.resolved_baseline, 1);
        assert_eq!(report.summary.expired_suppressions, 1);
        assert!(render_markdown(&report).contains("## Expired suppression entries"));
        Ok(())
    }

    #[test]
    fn policy_report_markdown_escapes_table_cells() {
        let report = PolicyReport {
            schema_version: "0.1".to_string(),
            tool: "unsafe-review".to_string(),
            mode: "policy-report".to_string(),
            policy: "advisory".to_string(),
            audit_date: "2026-05-20".to_string(),
            trust_boundary: "Advisory policy report only; not memory-safety proof.".to_string(),
            summary: PolicyReportSummary {
                cards: 1,
                new_gaps: 1,
                resolved_baseline: 1,
                expired_suppressions: 1,
                ..PolicyReportSummary::default()
            },
            cards: vec![PolicyReportCard {
                card_id: "UR-pipe|card-c1".to_string(),
                class_name: "guard|missing".to_string(),
                policy_status: "new|gap".to_string(),
                missing_count: 1,
            }],
            resolved_baseline: vec![PolicyLedgerEntry {
                card_id: "UR-resolved|card-c1".to_string(),
                owner: Some("team|unsafe".to_string()),
                reason: Some("resolved|by guard".to_string()),
                review_after: Some("2026-08-01|manual".to_string()),
                expires: None,
            }],
            expired_suppressions: vec![PolicyLedgerEntry {
                card_id: "UR-expired|card-c1".to_string(),
                owner: Some("team|unsafe".to_string()),
                reason: Some("old|false positive".to_string()),
                review_after: None,
                expires: Some("2026-05-01|expired".to_string()),
            }],
        };

        let markdown = render_markdown(&report);

        assert!(markdown.contains("`new\\|gap`"));
        assert!(markdown.contains("`UR-pipe\\|card-c1`"));
        assert!(markdown.contains("`guard\\|missing`"));
        assert!(markdown.contains("`UR-resolved\\|card-c1`"));
        assert!(markdown.contains("team\\|unsafe"));
        assert!(markdown.contains("resolved\\|by guard"));
        assert!(markdown.contains("2026-05-01\\|expired"));
        assert!(!markdown.contains("`UR-pipe|card-c1`"));
    }

    fn fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
    }

    fn unique_temp_dir(prefix: &str) -> Result<PathBuf, String> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system clock before UNIX_EPOCH: {err}"))?
            .as_nanos();
        Ok(std::env::temp_dir().join(format!("{prefix}-{nanos}")))
    }

    fn copy_dir(src: &Path, dst: &Path) -> Result<(), String> {
        fs::create_dir_all(dst).map_err(|err| format!("create {} failed: {err}", dst.display()))?;
        for entry in
            fs::read_dir(src).map_err(|err| format!("read {} failed: {err}", src.display()))?
        {
            let entry = entry.map_err(|err| format!("read_dir entry failed: {err}"))?;
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());
            if src_path.is_dir() {
                copy_dir(&src_path, &dst_path)?;
            } else {
                fs::copy(&src_path, &dst_path)
                    .map_err(|err| format!("copy {} failed: {err}", src_path.display()))?;
            }
        }
        Ok(())
    }
}
