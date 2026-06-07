use crate::api::{Scope, Summary};
use crate::domain::{ReviewCard, ReviewClass};
use std::collections::BTreeSet;

/// Summarize card counts and compute SPEC-0030 movement fields.
///
/// **Movement definitions (SPEC-0030)**:
/// - `new_gaps`: open actionable cards not in the baseline ledger, constrained to
///   changed-line sites on a diff-scoped run.
/// - `worsened_gaps`: baseline cards whose coverage regressed; always 0 until a coverage
///   snapshot mechanism (`baseline init`) lands (deferred to a follow-up slice).
/// - `resolved_gaps`: baseline ledger entries whose card is no longer present.
/// - `inherited_gaps`: cards classified `BaselineKnown` (matched baseline, still open).
pub(super) fn summarize(
    rust_files: usize,
    changed_files: usize,
    changed_rust_files: usize,
    changed_non_rust_files: usize,
    cards: &[ReviewCard],
    scope: &Scope,
    baseline_ids: &BTreeSet<String>,
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
    // worsened_gaps is always 0: detecting coverage regression requires a saved coverage
    // snapshot that `baseline init` would produce (deferred per SPEC-0030 scope note).
    summary.worsened_gaps = 0;
    summary
}
