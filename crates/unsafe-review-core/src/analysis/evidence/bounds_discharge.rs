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
    } else {
        EvidenceState::missing("No length or bounds guard code was detected")
    }
}
