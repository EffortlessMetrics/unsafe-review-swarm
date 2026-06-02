use crate::api::Summary;
use crate::domain::{ReviewCard, ReviewClass};

pub(super) fn summarize(
    rust_files: usize,
    changed_rust_files: usize,
    cards: &[ReviewCard],
) -> Summary {
    let mut summary = Summary {
        rust_files,
        changed_rust_files,
        unsafe_sites: cards.len(),
        cards: cards.len(),
        ..Summary::default()
    };
    for card in cards {
        if card.class.is_actionable() {
            summary.open_actionable_gaps += 1;
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
    summary
}
