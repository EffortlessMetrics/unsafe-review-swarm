//! Output projection for `baseline status` / `baseline refresh --dry-run` (issue #1893).
//!
//! Human and JSON renderers both read the same [`BaselineHealthReport`] /
//! [`BaselineRefreshPlan`] values, so the two surfaces always report identical bucket
//! counts and entry identities (issue #1893 acceptance criterion).
//!
//! Repo-controlled strings (`card_id`, `detail`, `snapshot_load_error`, dates) reach
//! these renderers verbatim from ledger/snapshot files and ReviewCard identities.
//! [`escape_control_chars`] neutralizes ESC/C0/C1 control characters, CR/LF, the Unicode
//! line/paragraph separators, bidirectional-format controls, and zero-width characters
//! before they hit a human terminal (terminal-injection hardening); JSON rendering is
//! untouched — `serde_json` already escapes control characters per the JSON spec, and
//! JSON consumers need the exact original value, not a display-safe one.

use crate::policy::baseline_health::{BaselineHealthReport, BaselineRefreshPlan};
use serde::Serialize;

const STATUS_TRUST_BOUNDARY: &str = "Advisory baseline-ledger health report only. It scans repository source and policy files to classify existing SPEC-0030 baseline/coverage-movement signals per ledger entry and writes nothing; a baseline pass is a no-new-debt statement only, not a proof of memory safety, not UB-free status, not Miri-clean status, and not a site-execution claim.";
const REFRESH_TRUST_BOUNDARY: &str = "Advisory dry-run refresh preview only; it leaves repository policy, source, and snapshot state unchanged, writing a plan artifact only when --out is explicitly given. The plan previews what a future, separately-approved apply mode could change — it is not applied here, and it is not a safety, UB-free, Miri-clean, or site-execution claim.";

/// Escape ESC (`0x1B`), every other C0 control byte, DEL (`0x7F`), C1 control bytes
/// (`0x80`..=`0x9F`), CR/LF, the Unicode line/paragraph separators (`U+2028`, `U+2029`),
/// Unicode bidirectional-format controls, and zero-width characters as `\xNN`/`\u{NNNN}`
/// before printing repo-controlled text to a human terminal (terminal-injection
/// hardening, issue #1893 review finding). These families matter because a malicious
/// ledger/snapshot string can otherwise use `U+2028`/`U+2029` as a hard line break to
/// split a rendered row (the same vector as CR/LF), RTL/LTR overrides or isolates
/// (`U+202A`..=`U+202E`, `U+2066`..=`U+2069`) to visually reorder one, or zero-width
/// characters (`U+200B`..=`U+200D`, `U+FEFF`) to hide or misalign one — spoofing the
/// health/plan output the operator reads. JSON rendering must never call this — it would
/// corrupt the value JSON consumers expect to round-trip.
fn escape_control_chars(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        let code = ch as u32;
        let is_control = code <= 0x1F || code == 0x7F || (0x80..=0x9F).contains(&code);
        let is_bidi_or_zero_width = matches!(
            ch,
            '\u{200B}'
                | '\u{200C}'
                | '\u{200D}'
                | '\u{FEFF}'
                // Unicode line/paragraph separators most terminals render as a hard
                // line break — same row-splitting vector as CR/LF.
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202A}'
                | '\u{202B}'
                | '\u{202C}'
                | '\u{202D}'
                | '\u{202E}'
                | '\u{2066}'
                | '\u{2067}'
                | '\u{2068}'
                | '\u{2069}'
        );
        if is_control {
            out.push_str(&format!("\\x{code:02x}"));
        } else if is_bidi_or_zero_width {
            out.push_str(&format!("\\u{{{code:04x}}}"));
        } else {
            out.push(ch);
        }
    }
    out
}

#[derive(Serialize)]
struct BaselineStatusJson<'a> {
    schema_version: &'static str,
    tool: &'static str,
    mode: &'static str,
    trust_boundary: &'static str,
    /// Sum of every bucket count (`counts.total()`) — a convenience total so
    /// consumers do not have to sum the ten buckets themselves.
    total: usize,
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
        total: report.counts.total(),
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
    out.push_str(&format!(
        "audit date: {}\n\n",
        escape_control_chars(&report.today)
    ));
    if let Some(err) = &report.card_scan_error {
        out.push_str(&format!(
            "warning: the repo-wide card scan could not run because the baseline ledger \
             failed strict validation: {}\n\
             `resolved` below means \"no current card was found\" in this degraded, \
             scan-unavailable sense, not a confirmed disappearance — affected entries may \
             still be present in the repository. `identity_unmatched`, \
             `duplicate_or_conflicting_entry`, and `suppression_overlap` are unaffected.\n\n",
            escape_control_chars(err)
        ));
    }
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
            "note: coverage snapshot file failed to parse: {}\n\n",
            escape_control_chars(err)
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
                escape_control_chars(&entry.card_id),
                entry.bucket.as_str(),
                escape_control_chars(&entry.detail)
            ));
        }
        out.push('\n');
    }
    out.push_str("All entries:\n");
    for entry in &report.entries {
        out.push_str(&format!(
            "  {}  {}  {}\n",
            escape_control_chars(&entry.card_id),
            entry.bucket.as_str(),
            escape_control_chars(&entry.detail)
        ));
    }
    out.push('\n');
    out.push_str(&format!("total: {}\n", counts.total()));
    out.push('\n');
    out.push_str(&format!("trust boundary: {STATUS_TRUST_BOUNDARY}\n"));
    out
}

pub(crate) fn render_refresh_human(plan: &BaselineRefreshPlan) -> String {
    let summary = &plan.summary;
    let mut out = String::new();
    out.push_str("unsafe-review baseline refresh --dry-run\n");
    out.push_str(&format!(
        "audit date: {}\n\n",
        escape_control_chars(&plan.today)
    ));
    out.push_str(
        "this preview leaves repository policy, source, and snapshot state unchanged; \
         it writes a plan artifact only when --out <dir> is explicitly given.\n\n",
    );
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
            escape_control_chars(&entry.card_id),
            entry.bucket.as_str(),
            entry.action.as_str(),
            if entry.auto_eligible {
                " (auto-eligible)"
            } else {
                " (never auto-applied)"
            },
            escape_control_chars(&entry.detail)
        ));
    }
    out.push('\n');
    out.push_str(&format!("trust boundary: {REFRESH_TRUST_BOUNDARY}\n"));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::baseline_health::{BaselineHealthCounts, BaselineHealthEntry, HealthBucket};

    const INJECTED_CARD_ID: &str = "UR-evil\u{1b}[31m-src-lib-rs-owner-op-c1\ninjected line";

    fn report_with_injected_card_id() -> BaselineHealthReport {
        let counts = BaselineHealthCounts {
            new_unbaselined: 1,
            ..BaselineHealthCounts::default()
        };
        BaselineHealthReport {
            today: "2026-07-18".to_string(),
            entries: vec![BaselineHealthEntry {
                card_id: INJECTED_CARD_ID.to_string(),
                bucket: HealthBucket::NewUnbaselined,
                detail: "detail with \u{1b} escape and \nnewline".to_string(),
            }],
            counts,
            snapshot_load_error: None,
            card_scan_error: None,
        }
    }

    #[test]
    fn escape_control_chars_neutralizes_esc_and_newline() {
        let escaped = escape_control_chars("a\u{1b}[31mb\nc\rd");
        assert_eq!(escaped, "a\\x1b[31mb\\x0ac\\x0dd");
        assert!(!escaped.contains('\u{1b}'));
        assert!(!escaped.contains('\n'));
        assert!(!escaped.contains('\r'));
    }

    #[test]
    fn escape_control_chars_leaves_plain_text_unchanged() {
        assert_eq!(escape_control_chars("plain text 123"), "plain text 123");
    }

    #[test]
    fn escape_control_chars_neutralizes_bidi_and_zero_width() {
        // RTL override + zero-width space + a bidi isolate + line/paragraph separators —
        // all real terminal-spoofing vectors a malicious ledger/snapshot string carries.
        let escaped = escape_control_chars("a\u{202e}b\u{200b}c\u{2066}d\u{2028}e\u{2029}f");
        assert_eq!(
            escaped,
            "a\\u{202e}b\\u{200b}c\\u{2066}d\\u{2028}e\\u{2029}f"
        );
        for spoof in [
            '\u{202e}', '\u{202d}', '\u{202a}', '\u{2066}', '\u{2069}', '\u{200b}', '\u{200c}',
            '\u{200d}', '\u{feff}', '\u{2028}', '\u{2029}',
        ] {
            assert!(
                !escaped.contains(spoof),
                "bidi/zero-width {spoof:?} must not survive escaping"
            );
            let one = escape_control_chars(&spoof.to_string());
            assert_eq!(one, format!("\\u{{{:04x}}}", spoof as u32), "for {spoof:?}");
        }
    }

    #[test]
    fn human_status_output_neutralizes_injected_control_chars() {
        let report = report_with_injected_card_id();
        let human = render_status_human(&report);
        assert!(
            !human.contains('\u{1b}'),
            "raw ESC byte must not reach the terminal: {human}"
        );
        // Every line must be a genuine line — no bare `\n` smuggled inside a card_id.
        for line in human.lines() {
            assert!(!line.contains('\u{1b}'), "{line}");
        }
        assert!(human.contains("\\x1b"), "{human}");
        assert!(human.contains("\\x0a"), "{human}");
    }

    #[test]
    fn json_status_output_preserves_card_id_verbatim() -> Result<(), String> {
        let report = report_with_injected_card_id();
        let json_text = render_status_json(&report);
        let value: serde_json::Value = serde_json::from_str(&json_text)
            .map_err(|err| format!("render_status_json must produce valid JSON: {err}"))?;
        assert_eq!(
            value["entries"][0]["card_id"].as_str(),
            Some(INJECTED_CARD_ID),
            "JSON must round-trip the exact original card_id, control chars included"
        );
        Ok(())
    }

    #[test]
    fn refresh_human_output_neutralizes_injected_control_chars() {
        let health = report_with_injected_card_id();
        let plan = crate::policy::baseline_health::build_refresh_plan(&health);
        let human = render_refresh_human(&plan);
        assert!(!human.contains('\u{1b}'), "{human}");
        for line in human.lines() {
            assert!(!line.contains('\u{1b}'), "{line}");
        }
    }

    #[test]
    fn status_json_exposes_convenience_total() -> Result<(), String> {
        let report = report_with_injected_card_id();
        let json_text = render_status_json(&report);
        let value: serde_json::Value =
            serde_json::from_str(&json_text).map_err(|err| format!("invalid JSON: {err}"))?;
        assert_eq!(value["total"], 1);
        Ok(())
    }
}
