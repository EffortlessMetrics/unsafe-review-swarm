use super::unsafe_fn_call::{
    has_encode_utf8_remaining_capacity_evidence, has_unchecked_constructor_availability_evidence,
};
use crate::domain::{EvidenceState, OperationFamily};

pub(super) fn callee_contract_discharge_state(
    family: &OperationFamily,
    expression: &str,
    lower: &str,
) -> EvidenceState {
    if family == &OperationFamily::UnsafeFnCall
        && has_encode_utf8_remaining_capacity_evidence(lower)
    {
        EvidenceState::present("Unsafe call argument guard code was detected")
    } else if family == &OperationFamily::UnsafeFnCall
        && has_unchecked_constructor_availability_evidence(expression, lower)
    {
        EvidenceState::present("Unchecked constructor availability guard code was detected")
    } else {
        EvidenceState::missing("No obligation-specific guard code was detected")
    }
}
