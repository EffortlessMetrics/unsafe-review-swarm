use crate::api::{Scope, Summary};
use crate::domain::coverage::CoverageBlock;
use crate::domain::{ReviewCard, ReviewClass};
use crate::policy::{PolicyState, SnapshotCoverage};
use std::collections::BTreeSet;

/// Summarize card counts and compute SPEC-0030 movement fields.
///
/// **Movement definitions (SPEC-0030)**:
/// - `new_gaps`: open actionable cards not in the baseline ledger, constrained to
///   changed-line sites on a diff-scoped run.
/// - `worsened_gaps`: baseline cards whose coverage regressed since the snapshot.
/// - `resolved_gaps`: baseline ledger entries whose card is no longer present.
/// - `inherited_gaps`: cards classified `BaselineKnown` (matched baseline, still open).
#[allow(
    clippy::too_many_arguments,
    reason = "file stats + cards + scope + policy are all needed together; extracting a struct would obscure call sites without simplifying the logic"
)]
pub(super) fn summarize(
    rust_files: usize,
    changed_files: usize,
    changed_rust_files: usize,
    changed_non_rust_files: usize,
    cards: &[ReviewCard],
    scope: &Scope,
    baseline_ids: &BTreeSet<String>,
    policy_state: &PolicyState,
) -> Summary {
    let diff_scoped = matches!(scope, Scope::Diff);
    let current_ids = cards
        .iter()
        .map(|card| card.id.0.as_str())
        .collect::<BTreeSet<_>>();
    let mut summary = Summary {
        rust_files,
        changed_files,
        changed_rust_files,
        changed_non_rust_files,
        unsafe_sites: cards.len(),
        cards: cards.len(),
        ..Summary::default()
    };
    let mut worsened = 0usize;
    for card in cards {
        if card.class.is_actionable() {
            summary.open_actionable_gaps += 1;
            // diff-scoped: only count new gaps on changed lines; repo-mode: all new gaps count.
            if !diff_scoped || card.site.changed {
                summary.new_gaps += 1;
            }
        }
        if card.class == ReviewClass::BaselineKnown {
            summary.inherited_gaps += 1;
            // worsened detection: compare current coverage block against the saved snapshot.
            if let Some(snapshot) = policy_state.snapshot_for(&card.id.0) {
                let current_cov = coverage_block_to_snapshot(&CoverageBlock::derive(card));
                if snapshot.is_worsened_by(&current_cov) {
                    worsened += 1;
                }
            }
        }
        match &card.class {
            ReviewClass::ContractMissing => summary.contract_missing += 1,
            ReviewClass::GuardMissing => summary.guard_missing += 1,
            ReviewClass::GuardedUnwitnessed => summary.guarded_unwitnessed += 1,
            ReviewClass::UnsafeUnreached => summary.unsafe_unreached += 1,
            ReviewClass::RequiresLoom => summary.requires_loom += 1,
            ReviewClass::MiriUnsupported => summary.miri_unsupported += 1,
            ReviewClass::StaticUnknown => summary.static_unknown += 1,
            _ => {}
        }
    }
    // resolved_gaps = baseline IDs that have no current card.
    summary.resolved_gaps = baseline_ids
        .iter()
        .filter(|id| !current_ids.contains(id.as_str()))
        .count();
    summary.worsened_gaps = worsened;
    summary
}

/// Convert a CoverageBlock to the SnapshotCoverage representation for comparison.
pub(super) fn coverage_block_to_snapshot(block: &CoverageBlock) -> SnapshotCoverage {
    SnapshotCoverage {
        contract_coverage: block.contract_coverage.as_str().to_string(),
        guard_coverage: block.guard_coverage.as_str().to_string(),
        test_reach_coverage: block.test_reach_coverage.as_str().to_string(),
        witness_receipt_coverage: block.witness_receipt_coverage.as_str().to_string(),
    }
}
