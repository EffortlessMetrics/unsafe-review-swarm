//! Additive first-pr base/head movement section (#2271 PR2).
//!
//! After the head (diff-scoped) output is computed, the base commit is
//! materialized through [`super::base_materialization`], analyzed at repo
//! scope with the head's analyzer settings (complete census), and joined with
//! the head output through the frozen
//! [`evaluate_base_head_movement`](unsafe_review_core::evaluate_base_head_movement)
//! evaluator. The result renders as one stdout overview line plus a
//! `movement.json` bundle file. Ledger, policy, counters, outcome, and every
//! existing projection stay untouched.
//!
//! Two conservative rules keep this surface honest:
//!
//! * Resolved verdicts from the evaluator are mapped to unknown with a
//!   removal-not-established reason: a diff-scoped head analysis cannot
//!   distinguish subject removal from scope-gated emission. Resolved stays
//!   with the ledger and outcome paths.
//! * Materialization or base-analysis failure degrades to a single-entry
//!   unknown report carrying the cause; the head output, exit code path, and
//!   policy gate behave exactly as today.

use super::base_materialization::materialize_base;
use unsafe_review_core::{
    AnalysisMode, AnalyzeInput, AnalyzeOutput, DiffSource, MovementReport, MovementSummary,
    PolicyMode, Scope, SubjectMovement, SubjectMovementEntry, evaluate_base_head_movement,
};

/// Status of the additive movement section for one first-pr run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MovementSectionStatus {
    Complete,
    Unknown,
}

impl MovementSectionStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Unknown => "unknown",
        }
    }
}

/// The additive movement section: reconciled report plus provenance for the
/// `movement.json` bundle file and the stdout overview line.
pub(crate) struct MovementSection {
    pub(crate) report: MovementReport,
    pub(crate) status: MovementSectionStatus,
    /// Present only when `status` is unknown: the fail-soft cause.
    pub(crate) reason: Option<String>,
    pub(crate) base_sha: Option<String>,
    pub(crate) head_sha: Option<String>,
}

/// Conservative no-resolved rule for this surface: a base-only subject is
/// never claimed resolved here, because the diff-scoped head analysis cannot
/// distinguish genuine removal from scope-gated emission. The summary is
/// rebuilt from the mapped entries so counts and per-card verdicts reconcile
/// exactly.
fn apply_no_resolved_rule(report: MovementReport) -> MovementReport {
    let mut subjects: Vec<SubjectMovementEntry> = Vec::with_capacity(report.subjects.len());
    for mut entry in report.subjects {
        if entry.movement == SubjectMovement::Resolved {
            entry.movement = SubjectMovement::Unknown;
            entry.reason = "removal is not established on this surface: the head analysis is diff-scoped, so a base-only subject is reported unknown rather than resolved; resolved verdicts stay with the ledger and outcome paths".to_string();
        }
        subjects.push(entry);
    }
    let mut summary = MovementSummary::default();
    for entry in &subjects {
        match entry.movement {
            SubjectMovement::Introduced => summary.introduced += 1,
            SubjectMovement::Worsened => summary.worsened += 1,
            SubjectMovement::Improved => summary.improved += 1,
            SubjectMovement::Resolved => summary.resolved += 1,
            SubjectMovement::Inherited => summary.inherited += 1,
            SubjectMovement::Unknown => summary.unknown += 1,
        }
    }
    summary.subjects = subjects.len();
    MovementReport { summary, subjects }
}

/// A single-entry unknown report carrying a fail-soft cause. Counts reconcile
/// by construction: one subject, one unknown.
fn unknown_section(
    reason: String,
    base_sha: Option<String>,
    head_sha: Option<String>,
) -> MovementSection {
    let report = MovementReport {
        summary: MovementSummary {
            introduced: 0,
            worsened: 0,
            improved: 0,
            resolved: 0,
            inherited: 0,
            unknown: 1,
            subjects: 1,
        },
        subjects: vec![SubjectMovementEntry {
            subject: "base-head-movement".to_string(),
            base_card_id: None,
            head_card_id: None,
            movement: SubjectMovement::Unknown,
            reason: reason.clone(),
            ledger_baseline_state: "unknown".to_string(),
            ledger_outcome_movement: "unknown".to_string(),
        }],
    };
    MovementSection {
        report,
        status: MovementSectionStatus::Unknown,
        reason: Some(reason),
        base_sha,
        head_sha,
    }
}

/// Best-effort HEAD resolution for the movement envelope. `None` carries no
/// claim; it never fails the run.
fn resolve_head_sha(root: &std::path::Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("rev-parse")
        .arg("--verify")
        .arg("HEAD")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if sha.is_empty() { None } else { Some(sha) }
}

/// Compute the additive movement section for a completed head output.
///
/// `base_ref` is the configured `--base` value (already defaulted by parse);
/// `None` means the run has no base commit (e.g. `--diff` input) and degrades
/// to unknown with the cause. The base tree is analyzed with the head's
/// analyzer settings (draft mode, advisory policy, same card cap) at repo
/// scope; the head side reuses the already-computed diff-scoped output.
pub(crate) fn compute_movement_section(
    root: &std::path::Path,
    base_ref: Option<&str>,
    max_cards: Option<usize>,
    head: &AnalyzeOutput,
) -> MovementSection {
    let head_sha = resolve_head_sha(root);
    let Some(base_ref) = base_ref else {
        return unknown_section(
            "no base ref available: this run has no --base value (for example --diff input), so no base commit can be materialized".to_string(),
            None,
            head_sha,
        );
    };
    let materialized = match materialize_base(root, base_ref) {
        Ok(materialized) => materialized,
        Err(err) => {
            return unknown_section(err.to_string(), None, head_sha);
        }
    };
    // The head side analyzes the requested `--root` (which may be a subtree
    // of the work tree); the base census must cover the corresponding
    // directory under the materialized worktree, not the worktree top, or
    // root-relative subject identities stop matching. A requested root
    // outside the repository tree degrades to unknown with the cause.
    let base_root = match materialized.analysis_root_for(root) {
        Ok(base_root) => base_root,
        Err(err) => {
            return unknown_section(err.to_string(), None, head_sha);
        }
    };
    let base_sha = resolve_head_sha(materialized.path());
    let base = match unsafe_review_core::analyze(AnalyzeInput {
        root: base_root,
        scope: Scope::Repo,
        diff: DiffSource::NoneRepoScan,
        mode: AnalysisMode::Draft,
        policy: PolicyMode::Advisory,
        include_unchanged_tests: true,
        max_cards,
    }) {
        Ok(base) => base,
        Err(err) => {
            return unknown_section(format!("base analysis failed: {err}"), base_sha, head_sha);
        }
    };
    let report = apply_no_resolved_rule(evaluate_base_head_movement(&base, head));
    MovementSection {
        report,
        status: MovementSectionStatus::Complete,
        reason: None,
        base_sha,
        head_sha,
    }
}

/// Render the `movement.json` bundle file: the reconciled report with
/// per-subject verdicts and reasons, plus run provenance and the trust
/// boundary. The schema is additive and versioned; it changes no existing
/// projection.
pub(crate) fn render_movement_json(section: &MovementSection) -> String {
    let value = serde_json::json!({
        "schema_version": "movement/v1",
        "tool": "unsafe-review",
        "mode": "base_head_movement",
        "source": "first_pr",
        "status": section.status.as_str(),
        "reason": section.reason.as_deref(),
        "base_sha": section.base_sha.as_deref(),
        "head_sha": section.head_sha.as_deref(),
        "summary": section.report.summary,
        "subjects": section.report.subjects,
        "trust_boundary": "Advisory base/head subject attribution only: inherited context is not a safety claim, introduced is not a defect verdict, and unknown (including every base-only subject on this diff-scoped surface, where removal is never claimed) is not evidence either way. It is not memory-safety proof, not UB-free status, not Miri-clean status, not site-execution evidence, and not a blocking gate.",
    });
    let mut rendered = serde_json::to_string_pretty(&value)
        .unwrap_or_else(|err| format!("{{\n  \"error\": \"movement rendering failed: {err}\"\n}}"));
    rendered.push('\n');
    rendered
}

/// One-line terminal summary of the movement section. Always names the
/// `movement` surface and the six reconciled bucket counts; the failure leg
/// additionally carries the cause.
pub(crate) fn format_movement_overview_line(section: &MovementSection) -> String {
    let summary = &section.report.summary;
    let mut line = format!(
        "- Base/head movement: introduced {}, worsened {}, improved {}, resolved {}, inherited {}, unknown {}",
        summary.introduced,
        summary.worsened,
        summary.improved,
        summary.resolved,
        summary.inherited,
        summary.unknown,
    );
    if let Some(reason) = &section.reason {
        let one_line = reason.split_whitespace().collect::<Vec<_>>().join(" ");
        line.push_str(&format!("; {one_line}"));
    }
    line.push_str(" (see movement.json)");
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn movement_overview_line_collapses_multiline_reason_to_single_line() {
        let section = unknown_section(
            "git merge-base failed for base 'nope':\nfatal: no such ref".to_string(),
            None,
            None,
        );
        let line = format_movement_overview_line(&section);
        assert!(
            !line.contains('\n'),
            "overview line must stay single-line; got:\n{line}"
        );
        assert!(
            !line.contains('\r'),
            "overview line must stay single-line; got:\n{line}"
        );
        assert!(
            line.contains("git merge-base failed for base 'nope': fatal: no such ref"),
            "overview line must carry the collapsed cause; got:\n{line}"
        );
        let rendered = render_movement_json(&section);
        assert!(
            rendered.contains("git merge-base failed for base 'nope':"),
            "movement.json must keep the full detail; got:\n{rendered}"
        );
        assert!(
            rendered.contains("fatal: no such ref"),
            "movement.json must keep the full detail; got:\n{rendered}"
        );
    }
}
