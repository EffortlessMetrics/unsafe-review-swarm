use super::box_raw_origin::{
    has_box_from_raw_origin_evidence, has_drop_in_place_box_origin_evidence,
};
use super::vec_from_raw_parts::has_vec_from_raw_parts_origin_evidence;
use crate::domain::{EvidenceState, OperationFamily};

pub(super) fn ownership_discharge_state(
    family: &OperationFamily,
    expression: &str,
    lower: &str,
) -> EvidenceState {
    if (family == &OperationFamily::DropInPlace
        && has_drop_in_place_box_origin_evidence(expression, lower))
        || (family == &OperationFamily::BoxFromRaw
            && has_box_from_raw_origin_evidence(expression, lower))
        || (family == &OperationFamily::VecFromRawParts
            && has_vec_from_raw_parts_origin_evidence(expression, lower))
    {
        if family == &OperationFamily::VecFromRawParts {
            EvidenceState::present("ManuallyDrop Vec raw-parts ownership evidence was detected")
        } else {
            EvidenceState::present("Box::into_raw ownership evidence was detected")
        }
    } else {
        EvidenceState::missing("No obligation-specific guard code was detected")
    }
}
