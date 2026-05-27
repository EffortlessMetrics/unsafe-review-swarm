use super::zeroed::has_zeroed_known_valid_zero_type;
use crate::domain::{EvidenceState, OperationFamily};

pub(super) fn valid_zero_discharge_state(family: &OperationFamily, lower: &str) -> EvidenceState {
    if family == &OperationFamily::Zeroed && has_zeroed_known_valid_zero_type(lower) {
        EvidenceState::present("Known valid-zero target type evidence was detected before zeroed")
    } else {
        EvidenceState::missing("No obligation-specific guard code was detected")
    }
}
