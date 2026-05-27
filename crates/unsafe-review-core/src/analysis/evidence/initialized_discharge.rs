use super::box_raw_origin::has_drop_in_place_box_origin_evidence;
use super::maybeuninit::maybeuninit_assume_init_discharge_state;
use super::set_len;
use super::vec_from_raw_parts::has_vec_from_raw_parts_origin_initialized_evidence;
use super::write_bytes::{
    has_bool_write_bytes_value_evidence, has_maybeuninit_raw_write_context,
    has_maybeuninit_slice_context, has_u8_write_bytes_context,
};
use crate::analysis::scanner::ScannedSite;
use crate::domain::{EvidenceState, OperationFamily};

pub(super) fn initialized_discharge_state(site: &ScannedSite, lower: &str) -> EvidenceState {
    let family = &site.operation.family;
    if let Some(state) = set_len::set_len_initialized_discharge_state(site) {
        state
    } else if let Some(state) =
        maybeuninit_assume_init_discharge_state(family, &site.operation.expression, lower)
    {
        state
    } else if family == &OperationFamily::SliceFromRawParts && has_maybeuninit_slice_context(lower)
    {
        EvidenceState::present("MaybeUninit slice element evidence was detected")
    } else if family == &OperationFamily::RawPointerWrite
        && has_maybeuninit_raw_write_context(site, lower)
    {
        EvidenceState::present("MaybeUninit raw write target evidence was detected")
    } else if family == &OperationFamily::RawPointerWrite && has_u8_write_bytes_context(site, lower)
    {
        EvidenceState::present("u8 write_bytes target evidence was detected")
    } else if family == &OperationFamily::RawPointerWrite
        && has_bool_write_bytes_value_evidence(site, lower)
    {
        EvidenceState::present("bool write_bytes value evidence was detected")
    } else if family == &OperationFamily::VecFromRawParts
        && has_vec_from_raw_parts_origin_initialized_evidence(&site.operation.expression, lower)
    {
        EvidenceState::present(
            "Vec::from_raw_parts same-origin initialized range evidence was detected",
        )
    } else if family == &OperationFamily::DropInPlace
        && has_drop_in_place_box_origin_evidence(&site.operation.expression, lower)
    {
        EvidenceState::present("Box::into_raw origin evidence was detected")
    } else {
        EvidenceState::missing("No obligation-specific guard code was detected")
    }
}
