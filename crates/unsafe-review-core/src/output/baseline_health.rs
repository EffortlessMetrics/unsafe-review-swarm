//! Output projection for `baseline status` / `baseline refresh --dry-run` (issue #1893).
//!
//! Human and JSON renderers both read the same [`BaselineHealthReport`] /
//! [`BaselineRefreshPlan`] values, so the two surfaces always report identical bucket
//! counts and entry identities (issue #1893 acceptance criterion).

use crate::policy::baseline_health::{BaselineHealthReport, BaselineRefreshPlan};
use serde::Serialize;

const STATUS_TRUST_BOUNDARY: &str = "Advisory baseline-ledger health report only. It classifies existing SPEC-0030 baseline/coverage-movement signals per ledger entry; a baseline pass is a no-new-debt statement only, not a proof of memory safety, not UB-free status, not Miri-clean status, and not a site-execution claim. This command reads policy files only; it writes nothing.";
const REFRESH_TRUST_BOUNDARY: &str = "Advisory dry-run refresh preview only; it writes nothing to policy, source, or snapshot files. The plan previews what a future, separately-approved apply mode could change — it is not applied here, and it is not a safety, UB-free, Miri-clean, or site-execution claim.";

#[derive(Serialize)]
struct BaselineStatusJson<'a> {
    schema_version: &'static str,
    tool: &'static str,
    mode: &'static str,
    trust_boundary: &'static str,
    #[serde(flatten)]
    report: &'a BaselineHealthReport,
}

#[derive(Serialize)]
struct BaselineRefreshJson<'a> {
    schema_version: &'static str,
    tool: &'static str,
    mode: &'static str,
    trust_boundary: &'static str,
    #[serde(flatten)]
    plan: &'a BaselineRefreshPlan,
}

pub(crate) fn render_status_json(report: &BaselineHealthReport) -> String {
    let payload = BaselineStatusJson {
        schema_version: "0.1",
        tool: "unsafe-review",
        mode: "baseline-status",
        trust_boundary: STATUS_TRUST_BOUNDARY,
        report,
    };
    match serde_json::to_string_pretty(&payload) {
        Ok(text) => text,
        Err(err) => format!("{{\n  \"error\": \"baseline status serialization failed: {err}\"\n}}"),
    }
}

pub(crate) fn render_refresh_json(plan: &BaselineRefreshPlan) -> String {
    let payload = BaselineRefreshJson {
        schema_version: "0.1",
        tool: "unsafe-review",
        mode: "baseline-refresh-dry-run",
        trust_boundary: REFRESH_TRUST_BOUNDARY,
        plan,
    };
    match serde_json::to_string_pretty(&payload) {
        Ok(text) => text,
        Err(err) => {
            format!("{{\n  \"error\": \"baseline refresh serialization failed: {err}\"\n}}")
        }
    }
}

pub(crate) fn render_status_human(report: &BaselineHealthReport) -> String {
    let counts = &report.counts;
    let mut out = String::new();
    out.push_str("unsafe-review baseline status\n");
    out.push_str(&format!("audit date: {}\n\n", report.today));
    out.push_str("Buckets:\n");
    out.push_str(&format!(
        "  active_unchanged: {}\n",
        counts.active_unchanged
    ));
    out.push_str(&format!("  active_improved: {}\n", counts.active_improved));
    out.push_str(&format!("  active_worsened: {}\n", counts.active_worsened));
    out.push_str(&format!("  resolved: {}\n", counts.resolved));
    out.push_str(&format!("  review_due: {}\n", counts.review_due));
    out.push_str(&format!(
        "  snapshot_missing_or_invalid: {}\n",
        counts.snapshot_missing_or_invalid
    ));
    out.push_str(&format!(
        "  duplicate_or_conflicting_entry: {}\n",
        counts.duplicate_or_conflicting_entry
    ));
    out.push_str(&format!(
        "  suppression_overlap: {}\n",
        counts.suppression_overlap
    ));
    out.push_str(&format!(
        "  identity_unmatched: {}\n",
        counts.identity_unmatched
    ));
    out.push_str(&format!("  new_unbaselined: {}\n", counts.new_unbaselined));
    out.push('\n');
    if let Some(err) = &report.snapshot_load_error {
        out.push_str(&format!(
            "note: coverage snapshot file failed to parse: {err}\n\n"
        ));
    }
    if counts.is_fully_healthy() {
        out.push_str(
            "All ledger entries are active_unchanged or resolved; nothing needs attention.\n\n",
        );
    } else {
        out.push_str("Entries needing attention:\n");
        for entry in report
            .entries
            .iter()
            .filter(|entry| entry.bucket.needs_attention())
        {
            out.push_str(&format!(
                "  {}  {}  {}\n",
                entry.card_id,
                entry.bucket.as_str(),
                entry.detail
            ));
        }
        out.push('\n');
    }
    out.push_str("All entries:\n");
    for entry in &report.entries {
        out.push_str(&format!(
            "  {}  {}  {}\n",
            entry.card_id,
            entry.bucket.as_str(),
            entry.detail
        ));
    }
    out.push('\n');
    out.push_str(&format!("trust boundary: {STATUS_TRUST_BOUNDARY}\n"));
    out
}

pub(crate) fn render_refresh_human(plan: &BaselineRefreshPlan) -> String {
    let summary = &plan.summary;
    let mut out = String::new();
    out.push_str("unsafe-review baseline refresh --dry-run\n");
    out.push_str(&format!("audit date: {}\n\n", plan.today));
    out.push_str("this preview writes nothing to policy, source, or snapshot files.\n\n");
    out.push_str("Plan summary:\n");
    out.push_str(&format!("  keep: {}\n", summary.keep));
    out.push_str(&format!("  update_snapshot: {}\n", summary.update_snapshot));
    out.push_str(&format!("  mark_resolved: {}\n", summary.mark_resolved));
    out.push_str(&format!(
        "  advance_review_after (owner review required): {}\n",
        summary.advance_review_after
    ));
    out.push_str(&format!(
        "  add_new_debt (separate explicit decision required): {}\n",
        summary.add_new_debt
    ));
    out.push_str(&format!(
        "  conflict (human resolution required): {}\n",
        summary.conflict
    ));
    out.push('\n');
    out.push_str("Per-entry plan:\n");
    for entry in &plan.entries {
        out.push_str(&format!(
            "  {}  {} -> {}{}  {}\n",
            entry.card_id,
            entry.bucket.as_str(),
            entry.action.as_str(),
            if entry.auto_eligible {
                " (auto-eligible)"
            } else {
                " (never auto-applied)"
            },
            entry.detail
        ));
    }
    out.push('\n');
    out.push_str(&format!("trust boundary: {REFRESH_TRUST_BOUNDARY}\n"));
    out
}
