use super::transmute::has_transmute_layout_size_evidence;
use crate::analysis::scanner::ScannedSite;
use crate::domain::{EvidenceState, OperationFamily};

pub(super) fn layout_discharge_state(site: &ScannedSite, lower: &str) -> EvidenceState {
    if site.operation.family == OperationFamily::Transmute
        && has_transmute_layout_size_evidence(lower, &site.operation.expression)
    {
        EvidenceState::present("Transmute layout size evidence was detected")
    } else {
        EvidenceState::missing("No obligation-specific guard code was detected")
    }
}
