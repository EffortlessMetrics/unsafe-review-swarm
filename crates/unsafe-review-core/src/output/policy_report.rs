use crate::api::AnalyzeOutput;
use crate::domain::ReviewClass;
use crate::output::{NO_CHANGED_GAPS_LIMITATION, NO_CHANGED_GAPS_MESSAGE};
use crate::policy::{LedgerEntry as PolicyLedgerRecord, LedgerKind, load_ledger_entries};
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const TRUST_BOUNDARY: &str = "Advisory no-new-debt policy report only; this is static unsafe contract review over existing ReviewCards and policy ledgers. It does not execute witnesses, is not a proof of memory safety, not UB-free status, not Miri-clean status, not a site-execution claim unless a matching witness receipt says so, and does not enforce blocking policy.";

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PolicyReport {
    pub schema_version: String,
    pub tool: String,
    pub mode: String,
    pub policy: String,
    pub audit_date: String,
    pub trust_boundary: String,
    pub limitations: Vec<String>,
    pub classification_explanations: PolicyReportClassificationExplanations,
    pub summary: PolicyReportSummary,
    pub cards: Vec<PolicyReportCard>,
    pub resolved_baseline: Vec<PolicyLedgerEntry>,
    pub unmatched_baseline: Vec<PolicyLedgerEntry>,
    pub expired_suppressions: Vec<PolicyLedgerEntry>,
    pub invalid_ledger_entries: Vec<PolicyInvalidLedgerEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PolicyReportClassificationExplanations {
    pub new_gap: String,
    pub baseline_known: String,
    pub suppressed: String,
    pub resolved_baseline: String,
    pub expired_suppression: String,
    pub non_actionable: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct PolicyReportSummary {
    pub cards: usize,
    pub new_gaps: usize,
    /// Baseline cards whose coverage regressed (always 0 until baseline-init snapshot lands).
    pub worsened_gaps: usize,
    pub resolved_baseline: usize,
    pub inherited_gaps: usize,
    pub baseline_known: usize,
    pub suppressed: usize,
    pub unmatched_baseline: usize,
    pub expired_suppressions: usize,
    pub invalid_ledger_entries: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PolicyReportCard {
    pub card_id: String,
    #[serde(rename = "class")]
    pub class_name: String,
    pub operation: String,
    pub operation_family: String,
    pub proof_path: String,
    pub policy_status: String,
    pub policy_reason: String,
    /// SPEC-0030 baseline posture: `new_gap`, `inherited`, `non_actionable`, or `suppressed`.
    pub baseline_state: String,
    /// Whether the card's unsafe site is on a changed line (diff-scoped attribution, SPEC-0030).
    pub changed_line: bool,
    pub missing_count: usize,
    pub next_action: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PolicyLedgerEntry {
    pub card_id: String,
    pub owner: Option<String>,
    pub reason: Option<String>,
    pub evidence: Option<String>,
    pub review_after: Option<String>,
    pub expires: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PolicyInvalidLedgerEntry {
    pub path: String,
    pub reason: String,
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
        .map(|card| {
            let status = policy_status(&card.class);
            let baseline_state = policy_baseline_state(status).to_string();
            PolicyReportCard {
                card_id: card.id.0.clone(),
                class_name: card.class.as_str().to_string(),
                operation: card.operation.expression.clone(),
                operation_family: card.operation.family.as_str().to_string(),
                proof_path: card.proof_path.as_str().to_string(),
                policy_status: status.as_str().to_string(),
                policy_reason: policy_reason(status).to_string(),
                baseline_state,
                changed_line: card.site.changed,
                missing_count: card.missing.len(),
                next_action: card.next_action.summary.clone(),
            }
        })
        .collect::<Vec<_>>();
    let summary = PolicyReportSummary {
        cards: output.cards.len(),
        new_gaps: cards
            .iter()
            .filter(|card| card.policy_status == "new_gap")
            .count(),
        worsened_gaps: 0, // always 0 until baseline-init coverage snapshot lands (SPEC-0030 note)
        resolved_baseline: resolved_baseline.len(),
        inherited_gaps: cards
            .iter()
            .filter(|card| card.policy_status == "baseline_known")
            .count(),
        baseline_known: cards
            .iter()
            .filter(|card| card.policy_status == "baseline_known")
            .count(),
        suppressed: cards
            .iter()
            .filter(|card| card.policy_status == "suppressed")
            .count(),
        unmatched_baseline: resolved_baseline.len(),
        expired_suppressions: expired_suppressions.len(),
        invalid_ledger_entries: 0,
    };

    Ok(PolicyReport {
        schema_version: "0.1".to_string(),
        tool: "unsafe-review".to_string(),
        mode: "policy-report".to_string(),
        policy: "advisory".to_string(),
        audit_date: audit_date.to_string(),
        trust_boundary: TRUST_BOUNDARY.to_string(),
        limitations: policy_report_limitations(),
        classification_explanations: PolicyReportClassificationExplanations::default(),
        summary,
        cards,
        unmatched_baseline: resolved_baseline.clone(),
        resolved_baseline,
        expired_suppressions,
        invalid_ledger_entries: Vec::new(),
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
    markdown_sections::render_heading(&mut out);
    markdown_sections::render_summary(&mut out, report);
    markdown_sections::render_reviewer_front_panel(&mut out, report);
    markdown_sections::render_classification_explanations(&mut out, report);
    markdown_sections::render_current_cards(&mut out, report);

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

    out.push_str("## Limitations\n\n");
    for limitation in &report.limitations {
        out.push_str("- ");
        out.push_str(limitation);
        out.push('\n');
    }
    out.push('\n');

    out.push_str("## Trust boundary\n\n");
    out.push_str(&report.trust_boundary);
    out.push('\n');
    out
}

mod markdown_sections {
    use super::*;

    pub(super) fn render_heading(out: &mut String) {
        out.push_str("# unsafe-review policy report\n\n");
        out.push_str(
            "Advisory no-new-debt policy report from current ReviewCards and ledgers.\n\n",
        );
    }

    pub(super) fn render_summary(out: &mut String, report: &PolicyReport) {
        out.push_str("## Summary\n\n");
        out.push_str("| Cards | New gaps | Worsened | Resolved | Inherited | Baseline known | Suppressed | Expired suppressions |\n");
        out.push_str("|---:|---:|---:|---:|---:|---:|---:|---:|\n");
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} |\n\n",
            report.summary.cards,
            report.summary.new_gaps,
            report.summary.worsened_gaps,
            report.summary.resolved_baseline,
            report.summary.inherited_gaps,
            report.summary.baseline_known,
            report.summary.suppressed,
            report.summary.expired_suppressions
        ));
    }

    pub(super) fn render_reviewer_front_panel(out: &mut String, report: &PolicyReport) {
        out.push_str("## Reviewer front panel\n\n");
        out.push_str(&format!(
            "- Movement: {} new gap(s), {} worsened, {} resolved, {} inherited\n",
            report.summary.new_gaps,
            report.summary.worsened_gaps,
            report.summary.resolved_baseline,
            report.summary.inherited_gaps
        ));
        out.push_str(&format!(
            "- Current ledger-covered cards: {} baseline-known, {} suppressed\n",
            report.summary.baseline_known, report.summary.suppressed
        ));
        out.push_str(&format!(
            "- Ledger cleanup: {} resolved baseline entries, {} expired suppression entries, {} invalid ledger entries\n",
            report.summary.resolved_baseline,
            report.summary.expired_suppressions,
            report.summary.invalid_ledger_entries
        ));
        if report.summary.new_gaps > 0
            || report.summary.expired_suppressions > 0
            || report.summary.invalid_ledger_entries > 0
        {
            out.push_str(
                "- Next action: review new gaps and stale ledger entries before treating this as no-new-debt evidence.\n",
            );
        } else if report.summary.resolved_baseline > 0 {
            out.push_str(
                "- Next action: consider pruning or updating resolved baseline entries after reviewer confirmation.\n",
            );
        } else {
            out.push_str(
                "- Next action: keep exact-card ledger entries current; no blocking decision was made.\n",
            );
        }
        out.push_str(
            "- Boundary: this is advisory policy simulation only; it does not enforce blocking policy.\n\n",
        );
    }

    pub(super) fn render_classification_explanations(out: &mut String, report: &PolicyReport) {
        out.push_str("## Classification explanations\n\n");
        out.push_str("| Classification | Meaning |\n");
        out.push_str("|---|---|\n");
        out.push_str(&format!(
            "| `new_gap` | {} |\n",
            markdown_cell(&report.classification_explanations.new_gap)
        ));
        out.push_str(&format!(
            "| `baseline_known` | {} |\n",
            markdown_cell(&report.classification_explanations.baseline_known)
        ));
        out.push_str(&format!(
            "| `suppressed` | {} |\n",
            markdown_cell(&report.classification_explanations.suppressed)
        ));
        out.push_str(&format!(
            "| `resolved_baseline` | {} |\n",
            markdown_cell(&report.classification_explanations.resolved_baseline)
        ));
        out.push_str(&format!(
            "| `expired_suppression` | {} |\n",
            markdown_cell(&report.classification_explanations.expired_suppression)
        ));
        out.push('\n');
    }

    pub(super) fn render_current_cards(out: &mut String, report: &PolicyReport) {
        out.push_str("## Current cards\n\n");
        if report.cards.is_empty() {
            out.push_str(NO_CHANGED_GAPS_MESSAGE);
            out.push('\n');
            out.push_str(NO_CHANGED_GAPS_LIMITATION);
            out.push_str("\n\n");
            return;
        }

        out.push_str("| Status | Baseline | Changed | Reason | Card | Class | Proof path | Operation family | Operation | Missing evidence | Next action |\n");
        out.push_str("|---|---|---|---|---|---|---|---|---|---:|---|\n");
        for card in &report.cards {
            out.push_str(&format!(
                "| `{}` | `{}` | {} | {} | `{}` | `{}` | `{}` | `{}` | `{}` | {} | {} |\n",
                card.policy_status,
                card.baseline_state,
                if card.changed_line { "yes" } else { "no" },
                markdown_cell(&card.policy_reason),
                card.card_id,
                card.class_name,
                card.proof_path,
                card.operation_family,
                markdown_cell(&card.operation),
                card.missing_count,
                markdown_cell(&card.next_action)
            ));
        }
        out.push('\n');
    }
}

fn render_ledger_section(out: &mut String, title: &str, entries: &[PolicyLedgerEntry]) {
    out.push_str("## ");
    out.push_str(title);
    out.push_str("\n\n");
    if entries.is_empty() {
        out.push_str("None.\n\n");
        return;
    }
    out.push_str("| Card | Owner | Review after | Expires | Reason | Evidence |\n");
    out.push_str("|---|---|---|---|---|---|\n");
    for entry in entries {
        out.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} |\n",
            entry.card_id,
            optional_text(entry.owner.as_deref()),
            optional_text(entry.review_after.as_deref()),
            optional_text(entry.expires.as_deref()),
            optional_text(entry.reason.as_deref()),
            optional_text(entry.evidence.as_deref())
        ));
    }
    out.push('\n');
}

fn ledger_entries(path: &Path, kind: LedgerKind) -> Result<Vec<PolicyLedgerEntry>, String> {
    load_ledger_entries(path, kind).map(|entries| entries.into_iter().map(From::from).collect())
}

impl From<PolicyLedgerRecord> for PolicyLedgerEntry {
    fn from(entry: PolicyLedgerRecord) -> Self {
        Self {
            card_id: entry.card_id,
            owner: Some(entry.owner),
            reason: Some(entry.reason),
            evidence: Some(entry.evidence),
            review_after: entry.review_after,
            expires: entry.expires,
        }
    }
}

fn policy_status(class: &ReviewClass) -> PolicyStatus {
    match class {
        ReviewClass::BaselineKnown => PolicyStatus::BaselineKnown,
        ReviewClass::Suppressed => PolicyStatus::Suppressed,
        class if class.is_actionable() => PolicyStatus::NewGap,
        _ => PolicyStatus::NonActionable,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PolicyStatus {
    NewGap,
    BaselineKnown,
    Suppressed,
    NonActionable,
}

impl PolicyStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::NewGap => "new_gap",
            Self::BaselineKnown => "baseline_known",
            Self::Suppressed => "suppressed",
            Self::NonActionable => "non_actionable",
        }
    }
}

fn policy_reason(status: PolicyStatus) -> &'static str {
    match status {
        PolicyStatus::NewGap => {
            "Exact ReviewCard identity was not found in the baseline ledger or active suppression ledger."
        }
        PolicyStatus::BaselineKnown => "Exact ReviewCard identity matched a baseline ledger entry.",
        PolicyStatus::Suppressed => {
            "Exact ReviewCard identity matched an active suppression ledger entry."
        }
        PolicyStatus::NonActionable => {
            "ReviewCard class is not actionable under the advisory policy report."
        }
    }
}

/// SPEC-0030 baseline posture label for the policy report card (separate from `policy_status`).
fn policy_baseline_state(status: PolicyStatus) -> &'static str {
    match status {
        PolicyStatus::NewGap => "new_gap",
        PolicyStatus::BaselineKnown => "inherited",
        PolicyStatus::Suppressed => "suppressed",
        PolicyStatus::NonActionable => "non_actionable",
    }
}

impl Default for PolicyReportClassificationExplanations {
    fn default() -> Self {
        Self {
            new_gap: policy_reason(PolicyStatus::NewGap).to_string(),
            baseline_known: policy_reason(PolicyStatus::BaselineKnown).to_string(),
            suppressed: policy_reason(PolicyStatus::Suppressed).to_string(),
            resolved_baseline:
                "Baseline ledger entry no longer appears in the current ReviewCard set.".to_string(),
            expired_suppression:
                "Suppression ledger entry expiry date is before the report audit date.".to_string(),
            non_actionable: policy_reason(PolicyStatus::NonActionable).to_string(),
        }
    }
}

fn policy_report_limitations() -> Vec<String> {
    vec![
        "Advisory report only; it does not change command exit status or enforce blocking policy.".to_string(),
        "Matching is exact counted ReviewCard identity only; broad path, owner, or operation-family suppression is not supported.".to_string(),
        "Manual candidates are not policy-report inputs and remain separate advisory artifacts.".to_string(),
        "The report does not execute witnesses, post comments, edit source, or prove memory safety.".to_string(),
        "Malformed ledger entries fail the report instead of being recovered into invalid_ledger_entries.".to_string(),
    ]
}

fn optional_text(value: Option<&str>) -> String {
    value
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| "-".to_string())
}

fn markdown_cell(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
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
    use std::fs;
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
        assert_eq!(report.limitations.len(), 5);
        assert!(report.limitations[0].contains("Advisory report only"));
        assert!(report.limitations.iter().any(|limitation| {
            limitation.contains("Manual candidates are not policy-report inputs")
        }));
        assert!(
            report
                .classification_explanations
                .new_gap
                .contains("baseline ledger or active suppression ledger")
        );
        assert_eq!(report.summary.new_gaps, 1);
        assert_eq!(report.summary.baseline_known, 0);
        assert_eq!(report.summary.unmatched_baseline, 0);
        assert_eq!(report.summary.invalid_ledger_entries, 0);
        assert!(report.unmatched_baseline.is_empty());
        assert!(report.invalid_ledger_entries.is_empty());
        let card = report
            .cards
            .first()
            .ok_or_else(|| "policy report produced no cards".to_string())?;
        assert_eq!(card.operation, "unsafe { ptr.cast::<Header>().read() }");
        assert_eq!(card.operation_family, "raw_pointer_read");
        assert_eq!(card.policy_status, "new_gap");
        assert!(
            card.policy_reason
                .contains("was not found in the baseline ledger")
        );
        assert!(card.next_action.contains("Add or expose"));
        let markdown = render_markdown(&report);
        assert!(markdown.contains("## Reviewer front panel"));
        // SPEC-0030: movement summary replaces "New unbaselined gaps".
        assert!(markdown.contains("- Movement: 1 new gap(s), 0 worsened, 0 resolved, 0 inherited"));
        assert!(
            markdown.contains("- Current ledger-covered cards: 0 baseline-known, 0 suppressed")
        );
        assert!(markdown.contains(
            "review new gaps and stale ledger entries before treating this as no-new-debt evidence"
        ));
        assert!(markdown.contains("advisory policy simulation only"));
        assert!(markdown.contains("Manual candidates are not policy-report inputs"));
        assert!(markdown.contains("## Classification explanations"));
        assert!(markdown.contains("Exact ReviewCard identity was not found"));
        assert!(markdown.contains("Operation family | Operation"));
        assert!(markdown.contains("| Status | Baseline | Changed |"));
        assert!(markdown.contains("| `raw_pointer_read` |"));
        assert!(markdown.contains("unsafe { ptr.cast::<Header>().read() }"));
        assert!(markdown.contains("Add or expose"));
        assert!(markdown.contains("## Limitations"));
        assert!(report.trust_boundary.contains("does not enforce blocking"));
        assert!(report.trust_boundary.contains("not Miri-clean status"));
        assert!(report.trust_boundary.contains("not a site-execution claim"));
        assert!(report.trust_boundary.contains("matching witness receipt"));
        Ok(())
    }

    #[test]
    fn policy_report_empty_markdown_uses_standard_advisory_wording() -> Result<(), String> {
        let root = fixture_path("safe_code_no_cards");
        let output = analyze(AnalyzeInput {
            root,
            scope: Scope::Diff,
            diff: DiffSource::NoneRepoScan,
            mode: AnalysisMode::Draft,
            policy: PolicyMode::Advisory,
            include_unchanged_tests: true,
            max_cards: None,
        })?;

        let report = evaluate_with_date(&output, "2026-05-18")?;
        let markdown = render_markdown(&report);

        assert!(markdown.contains(NO_CHANGED_GAPS_MESSAGE));
        assert!(markdown.contains(NO_CHANGED_GAPS_LIMITATION));
        assert!(!markdown.contains("All clear"));
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
        assert_eq!(report.summary.unmatched_baseline, 1);
        assert_eq!(report.summary.expired_suppressions, 1);
        assert_eq!(report.summary.invalid_ledger_entries, 0);
        assert_eq!(
            report.resolved_baseline[0].evidence.as_deref(),
            Some("fixture")
        );
        assert_eq!(report.unmatched_baseline, report.resolved_baseline);
        assert_eq!(
            report.expired_suppressions[0].evidence.as_deref(),
            Some("fixture")
        );
        let markdown = render_markdown(&report);
        assert!(markdown.contains("## Reviewer front panel"));
        assert!(markdown.contains("- Ledger cleanup: 1 resolved baseline entries, 1 expired suppression entries, 0 invalid ledger entries"));
        assert!(markdown.contains("| Card | Owner | Review after | Expires | Reason | Evidence |"));
        assert!(markdown.contains("## Expired suppression entries"));
        assert!(markdown.contains("fixture"));
        Ok(())
    }

    #[test]
    fn policy_report_rejects_malformed_ledger_metadata() -> Result<(), String> {
        let source = fixture_path("raw_pointer_alignment");
        let root = unique_temp_dir("unsafe-review-policy-report-invalid")?;
        copy_dir(&source, &root)?;
        let output = analyze(AnalyzeInput {
            root: root.clone(),
            scope: Scope::Repo,
            diff: DiffSource::NoneRepoScan,
            mode: AnalysisMode::Repo,
            policy: PolicyMode::Advisory,
            include_unchanged_tests: true,
            max_cards: None,
        })?;
        let policy = root.join("policy");
        fs::create_dir_all(&policy).map_err(|err| format!("create policy failed: {err}"))?;
        fs::write(
            policy.join("unsafe-review-baseline.toml"),
            r#"schema_version = "0.1"
status = "active"

[[entries]]
card_id = "UR-invalid-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
owner = "core/policy"
reason = "accepted current debt"
review_after = "2026-08-01"
"#,
        )
        .map_err(|err| format!("write invalid baseline failed: {err}"))?;

        let err = match evaluate_with_date(&output, "2026-05-18") {
            Ok(_) => return Err("missing evidence should reject the policy report".to_string()),
            Err(err) => err,
        };

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp root failed: {err}"))?;
        assert!(err.contains("missing string `evidence`"));
        Ok(())
    }

    #[test]
    fn policy_status_and_reason_cover_known_classes() {
        let cases = [
            (ReviewClass::BaselineKnown, "baseline_known"),
            (ReviewClass::Suppressed, "suppressed"),
            (ReviewClass::GuardMissing, "new_gap"),
            (ReviewClass::GuardedAndWitnessed, "non_actionable"),
        ];

        for (class, expected_status) in cases {
            let status = policy_status(&class);
            assert_eq!(status.as_str(), expected_status);
            assert!(!policy_reason(status).is_empty());
        }
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
