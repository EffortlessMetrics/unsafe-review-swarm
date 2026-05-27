use super::box_raw_origin::has_drop_in_place_box_origin_evidence;
use super::nonnull::has_nullability_guard;
use super::vec_from_raw_parts::has_vec_from_raw_parts_origin_pointer_live_evidence;
use crate::analysis::scanner::ScannedSite;
use crate::domain::{EvidenceState, OperationFamily};

pub(super) fn pointer_live_discharge_state(site: &ScannedSite, lower: &str) -> EvidenceState {
    let family = &site.operation.family;
    if family == &OperationFamily::VecFromRawParts
        && has_vec_from_raw_parts_origin_pointer_live_evidence(&site.operation.expression, lower)
    {
        EvidenceState::present(
            "Vec::from_raw_parts same-origin pointer/capacity evidence was detected",
        )
    } else if family == &OperationFamily::DropInPlace
        && has_drop_in_place_box_origin_evidence(&site.operation.expression, lower)
    {
        EvidenceState::present("Box::into_raw origin evidence was detected")
    } else if has_nullability_guard(site, lower) {
        EvidenceState::present("Nullability guard code was detected")
    } else if family == &OperationFamily::VecFromRawParts {
        EvidenceState::missing(
            "No Vec::from_raw_parts same-origin pointer/capacity evidence was detected",
        )
    } else {
        EvidenceState::missing("No nullability guard code was detected")
    }
}
