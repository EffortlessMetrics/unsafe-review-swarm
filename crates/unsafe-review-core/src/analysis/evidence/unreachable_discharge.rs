use super::unreachable_unchecked::has_unreachable_unchecked_infallible_path_evidence;
use crate::domain::{EvidenceState, OperationFamily};

pub(super) fn unreachable_discharge_state(family: &OperationFamily, lower: &str) -> EvidenceState {
    if family == &OperationFamily::UnreachableUnchecked
        && has_unreachable_unchecked_infallible_path_evidence(lower)
    {
        EvidenceState::present(
            "Infallible error-path evidence was detected before unreachable_unchecked",
        )
    } else {
        EvidenceState::missing("No obligation-specific guard code was detected")
    }
}
