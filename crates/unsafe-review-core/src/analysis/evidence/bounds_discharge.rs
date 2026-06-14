use super::obligation_guard::has_bounds_guard;
use super::pointer_arithmetic::has_slice_end_pointer_arithmetic_evidence;
use crate::analysis::scanner::ScannedSite;
use crate::domain::{EvidenceState, OperationFamily};

pub(super) fn bounds_discharge_state(site: &ScannedSite, lower: &str) -> EvidenceState {
    let family = &site.operation.family;
    if has_bounds_guard(site, lower)
        || (family == &OperationFamily::PointerArithmetic
            && has_slice_end_pointer_arithmetic_evidence(lower))
    {
        EvidenceState::present("Length or bounds guard code was detected")
    } else if has_debug_assert_hint(lower) {
        EvidenceState::missing(
            "`debug_assert!` documents the intended invariant in debug builds, \
             but it is not release-runtime guard evidence. \
             Add an executable guard, witness receipt, or focused test reach.",
        )
    } else {
        EvidenceState::missing("No length or bounds guard code was detected")
    }
}

fn has_debug_assert_hint(lower: &str) -> bool {
    lower.contains("debug_assert!(")
        || lower.contains("debug_assert_eq!(")
        || lower.contains("debug_assert_ne!(")
}
