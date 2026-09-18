use super::site_context::site_snippet_offset;
use super::utf8::has_from_utf8_unchecked_validation_evidence;
use crate::analysis::scanner::ScannedSite;
use crate::domain::{EvidenceState, OperationFamily};

pub(super) fn utf8_discharge_state(site: &ScannedSite, lower: &str) -> EvidenceState {
    if site.operation.family == OperationFamily::StrFromUtf8Unchecked
        && has_from_utf8_unchecked_validation_evidence(lower, site_snippet_offset(site))
    {
        EvidenceState::present(
            "Same-buffer UTF-8 validation evidence was detected before from_utf8_unchecked",
        )
    } else {
        EvidenceState::missing("No obligation-specific guard code was detected")
    }
}
