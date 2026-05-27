use super::utf8::has_from_utf8_unchecked_validation_evidence;
use crate::domain::{EvidenceState, OperationFamily};

pub(super) fn utf8_discharge_state(family: &OperationFamily, lower: &str) -> EvidenceState {
    if family == &OperationFamily::StrFromUtf8Unchecked
        && has_from_utf8_unchecked_validation_evidence(lower)
    {
        EvidenceState::present(
            "Same-buffer UTF-8 validation evidence was detected before from_utf8_unchecked",
        )
    } else {
        EvidenceState::missing("No obligation-specific guard code was detected")
    }
}
