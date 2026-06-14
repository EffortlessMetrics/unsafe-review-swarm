use super::raw_pointer_alignment::has_alignment_guard;
use super::write_bytes::{has_bool_write_bytes_pointer_context, has_u8_write_bytes_context};
use crate::analysis::scanner::ScannedSite;
use crate::domain::{EvidenceState, OperationFamily};

pub(super) fn alignment_discharge_state(site: &ScannedSite, lower: &str) -> EvidenceState {
    let family = &site.operation.family;
    if family == &OperationFamily::RawPointerWrite && has_u8_write_bytes_context(site, lower) {
        EvidenceState::present("u8 raw write alignment evidence was detected")
    } else if family == &OperationFamily::RawPointerWrite
        && has_bool_write_bytes_pointer_context(site, lower)
    {
        EvidenceState::present("bool raw write alignment evidence was detected")
    } else if has_alignment_guard(site, lower) {
        EvidenceState::present("Alignment guard code was detected")
    } else if has_debug_assert_hint(lower) {
        EvidenceState::missing(
            "`debug_assert!` documents the intended invariant in debug builds, \
             but it is not release-runtime guard evidence. \
             Add an executable guard, witness receipt, or focused test reach.",
        )
    } else {
        EvidenceState::missing("No alignment guard code was detected")
    }
}

fn has_debug_assert_hint(lower: &str) -> bool {
    lower.contains("debug_assert!(")
        || lower.contains("debug_assert_eq!(")
        || lower.contains("debug_assert_ne!(")
}
