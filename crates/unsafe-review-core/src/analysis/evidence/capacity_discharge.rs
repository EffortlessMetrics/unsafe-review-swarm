use super::obligation_guard::has_capacity_guard;
use super::site_context::code_context_through_site;
use super::vec_from_raw_parts::{
    has_vec_from_raw_parts_capacity_evidence, has_vec_from_raw_parts_origin_len_cap_evidence,
};
use crate::analysis::scanner::ScannedSite;
use crate::domain::{EvidenceState, OperationFamily};

pub(super) fn capacity_discharge_state(site: &ScannedSite, lower: &str) -> EvidenceState {
    let family = &site.operation.family;
    let capacity_scope = (family == &OperationFamily::VecSetLen)
        .then(|| code_context_through_site(site).to_ascii_lowercase());
    let capacity_lower = capacity_scope.as_deref().unwrap_or(lower);
    if family == &OperationFamily::VecFromRawParts
        && has_vec_from_raw_parts_capacity_evidence(&site.operation.expression, lower)
    {
        EvidenceState::present("Vec::from_raw_parts length/capacity guard code was detected")
    } else if family == &OperationFamily::VecFromRawParts
        && has_vec_from_raw_parts_origin_len_cap_evidence(&site.operation.expression, lower)
    {
        EvidenceState::present("Vec::from_raw_parts same-origin len/capacity evidence was detected")
    } else if has_capacity_guard(family, capacity_lower) {
        EvidenceState::present("Capacity guard code was detected")
    } else {
        EvidenceState::missing("No capacity guard code was detected")
    }
}
