use crate::api::{Scope, Summary};
use crate::domain::coverage::CoverageBlock;
use crate::domain::{ReviewCard, ReviewClass};
use crate::policy::{PolicyState, SnapshotCoverage};
use crate::util::slug;
use std::collections::BTreeSet;
use std::path::PathBuf;

/// Summarize card counts and compute SPEC-0030 movement fields.
///
/// `scanned_sites` is the number of unsafe seams the scanner found in scope,
/// counted **before** any `max_cards` spread-selection cap is applied.  This
/// value is projected as `summary.unsafe_sites` so the field is distinct from
/// `cards` when the card set was capped.  On uncapped runs the two are equal.
///
/// **Movement definitions (SPEC-0030)**:
/// - `new_gaps`: open actionable cards not in the baseline ledger, constrained to
///   changed-line sites on a diff-scoped run.
/// - `worsened_gaps`: baseline cards whose coverage regressed since the snapshot.
/// - `improved_gaps`: baseline cards whose evidence coverage improved (pure improvement:
///   at least one slot advanced, no slot regressed).  Requires a saved coverage snapshot.
///   Precedence: worsened > improved > inherited.  A card is only counted improved if it
///   is not already counted worsened.  An improved card is still advisory, still open, and
///   still present — it is NOT resolved, NOT safe, NOT UB-free, and NOT Miri-clean.
/// - `resolved_gaps`: baseline ledger entries whose card is no longer present **and whose
///   file was in the scanned candidate set**.  On a diff-scoped run a baseline card whose
///   file was not scanned is counted as `inherited` (out-of-scope), not `resolved`.
/// - `inherited_gaps`: cards classified `BaselineKnown` (matched baseline, still open),
///   plus out-of-scope baseline IDs on a diff-scoped run.
#[allow(
    clippy::too_many_arguments,
    reason = "file stats + cards + scope + policy + scanned-files are all needed together; extracting a struct would obscure call sites without simplifying the logic"
)]
pub(super) fn summarize(
    rust_files: usize,
    changed_files: usize,
    changed_rust_files: usize,
    changed_non_rust_files: usize,
    scanned_sites: usize,
    cards: &[ReviewCard],
    scope: &Scope,
    baseline_ids: &BTreeSet<String>,
    policy_state: &PolicyState,
    scanned_files: &[PathBuf],
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
        unsafe_sites: scanned_sites,
        cards: cards.len(),
        ..Summary::default()
    };
    let mut worsened = 0usize;
    let mut improved = 0usize;
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
            // Worsened / improved detection: compare current coverage against the saved snapshot.
            // Precedence: worsened > improved > inherited (unchanged).
            if let Some(snapshot) = policy_state.snapshot_for(&card.id.0) {
                let current_cov = coverage_block_to_snapshot(&CoverageBlock::derive(card));
                if snapshot.is_worsened_by(&current_cov) {
                    worsened += 1;
                } else if snapshot.is_improved_by(&current_cov) {
                    improved += 1;
                }
                // else: inherited/unchanged — no counter change
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
    // resolved_gaps = baseline IDs that have no current card AND whose file was in scope.
    //
    // On a diff-scoped run, only candidate files are scanned.  A baseline card whose file
    // was not touched by the diff is absent from `current_ids` not because the gap was
    // fixed, but because we never scanned that file.  Counting it as `resolved` would be
    // wrong — the PR is judged only on what it changed (SPEC-0030 §diff-scope constraint).
    // Such out-of-scope IDs are counted as `inherited` instead.
    //
    // When `scanned_files` is empty (full repo scan or repo-mode) every unmatched baseline
    // ID is resolved — the whole codebase was scanned.
    //
    // The file slug matching mirrors the card identity scheme: path separators are collapsed
    // to underscores, then `slug()` lowercases and collapses non-alphanumeric runs to dashes.
    // Each card ID embeds the file slug at position 2 (zero-indexed), flanked by `-` delimiters.
    let (resolved_count, extra_inherited) = {
        let unmatched = baseline_ids
            .iter()
            .filter(|id| !current_ids.contains(id.as_str()));
        if scanned_files.is_empty() {
            // Full scan: every unmatched baseline ID is resolved, no extra inherited.
            (unmatched.count(), 0usize)
        } else {
            // Diff-scoped: split unmatched IDs by whether their file was in the candidate set.
            let mut resolved = 0usize;
            let mut extra_inh = 0usize;
            for id in unmatched {
                let in_scope = scanned_files.iter().any(|path| {
                    let file_str = path.to_string_lossy().replace(['/', '\\'], "_");
                    let file_slug = slug(&file_str);
                    id.contains(&format!("-{file_slug}-"))
                });
                if in_scope {
                    resolved += 1;
                } else {
                    extra_inh += 1;
                }
            }
            (resolved, extra_inh)
        }
    };
    summary.resolved_gaps = resolved_count;
    // Out-of-scope baseline IDs are inherited: the PR didn't touch those files,
    // so we cannot say whether those gaps are still present or resolved.
    summary.inherited_gaps += extra_inherited;
    summary.worsened_gaps = worsened;
    summary.improved_gaps = improved;
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
