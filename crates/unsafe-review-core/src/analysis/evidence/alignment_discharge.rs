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
    } else {
        EvidenceState::missing("No alignment guard code was detected")
    }
}
