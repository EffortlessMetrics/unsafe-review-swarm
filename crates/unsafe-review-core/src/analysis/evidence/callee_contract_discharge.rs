use super::site_context::site_snippet_offset;
use super::target_feature_guard::has_target_feature_detection_evidence;
use super::unsafe_fn_call::{
    has_encode_utf8_remaining_capacity_evidence, has_unchecked_constructor_availability_evidence,
};
use crate::analysis::scanner::ScannedSite;
use crate::domain::{EvidenceState, OperationFamily};

pub(super) fn callee_contract_discharge_state(site: &ScannedSite, lower: &str) -> EvidenceState {
    let family = &site.operation.family;
    if family == &OperationFamily::UnsafeFnCall
        && has_encode_utf8_remaining_capacity_evidence(lower)
    {
        EvidenceState::present("Unsafe call argument guard code was detected")
    } else if family == &OperationFamily::UnsafeFnCall
        && has_unchecked_constructor_availability_evidence(&site.operation.expression, lower)
    {
        EvidenceState::present("Unchecked constructor availability guard code was detected")
    } else if family == &OperationFamily::UnsafeFnCall
        && has_target_feature_detection_evidence(
            &site.operation.expression,
            lower,
            site_snippet_offset(site),
        )
    {
        EvidenceState::present(
            "Target-feature detection guard code was detected for the called function",
        )
    } else {
        EvidenceState::missing("No obligation-specific guard code was detected")
    }
}
