use super::transmute::has_transmute_u8_bool_valid_value_evidence;
use super::unwrap_unchecked::{
    has_unwrap_unchecked_infallible_result_evidence, has_unwrap_unchecked_receiver_state_evidence,
};
use crate::analysis::scanner::ScannedSite;
use crate::domain::{EvidenceState, OperationFamily};

pub(super) fn valid_value_discharge_state(site: &ScannedSite, lower: &str) -> EvidenceState {
    let family = &site.operation.family;
    if family == &OperationFamily::UnwrapUnchecked
        && has_unwrap_unchecked_infallible_result_evidence(lower)
    {
        EvidenceState::present(
            "Infallible Result state evidence was detected before unwrap_unchecked",
        )
    } else if family == &OperationFamily::UnwrapUnchecked
        && has_unwrap_unchecked_receiver_state_evidence(lower)
    {
        EvidenceState::present(
            "Same-receiver Option/Result state evidence was detected before unwrap_unchecked",
        )
    } else if family == &OperationFamily::Transmute
        && has_transmute_u8_bool_valid_value_evidence(lower, &site.operation.expression)
    {
        EvidenceState::present("Transmute u8-to-bool valid-value evidence was detected")
    } else {
        EvidenceState::missing("No obligation-specific guard code was detected")
    }
}
