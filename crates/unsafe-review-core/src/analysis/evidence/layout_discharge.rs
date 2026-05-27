use super::transmute::has_transmute_layout_size_evidence;
use crate::domain::{EvidenceState, OperationFamily};

pub(super) fn layout_discharge_state(family: &OperationFamily, lower: &str) -> EvidenceState {
    if family == &OperationFamily::Transmute && has_transmute_layout_size_evidence(lower) {
        EvidenceState::present("Transmute layout size evidence was detected")
    } else {
        EvidenceState::missing("No obligation-specific guard code was detected")
    }
}
