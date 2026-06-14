use super::{
    code_before_operation, get_unchecked_receiver_and_index, has_copy_slice_range_evidence,
    has_get_unchecked_bounds_guard, has_length_or_bounds_guard,
    has_pointer_arithmetic_bounds_guard, has_raw_pointer_read_bounds_evidence,
    has_raw_pointer_write_bounds_evidence, has_write_bytes_bounds_evidence, set_len,
};
use crate::analysis::scanner::ScannedSite;
use crate::domain::OperationFamily;

pub(super) fn has_bounds_guard(site: &ScannedSite, lower: &str) -> bool {
    if site.operation.family == OperationFamily::GetUnchecked
        && let Some((receiver, index)) =
            get_unchecked_receiver_and_index(&site.operation.expression)
    {
        let guard_scope = code_before_operation(lower, &site.operation.expression)
            .unwrap_or_else(|| lower.to_string());
        return has_get_unchecked_bounds_guard(&guard_scope, &receiver, &index);
    }
    if site.operation.family == OperationFamily::RawPointerWrite
        && site
            .operation
            .expression
            .to_ascii_lowercase()
            .contains("write_bytes")
    {
        return has_write_bytes_bounds_evidence(&site.operation.expression);
    }
    let guard_scope = code_before_operation(lower, &site.operation.expression)
        .unwrap_or_else(|| lower.to_string());
    if matches!(
        site.operation.family,
        OperationFamily::RawPointerRead | OperationFamily::RawPointerReadUnaligned
    ) {
        return has_raw_pointer_read_bounds_evidence(&site.operation.expression, &guard_scope);
    }
    // RawPointerWrite and RawPointerWriteUnaligned (non-write_bytes): require a
    // same-destination scoped guard.  A co-located length comparison on an unrelated
    // variable must not discharge the bounds obligation (F3-6 same-destination discipline).
    if matches!(
        site.operation.family,
        OperationFamily::RawPointerWrite | OperationFamily::RawPointerWriteUnaligned
    ) {
        return has_raw_pointer_write_bounds_evidence(&site.operation.expression, &guard_scope);
    }
    if matches!(
        site.operation.family,
        OperationFamily::CopyNonOverlapping | OperationFamily::PtrCopy
    ) {
        if has_copy_slice_range_evidence(&site.operation.expression, &guard_scope) {
            return true;
        }
        // A generic length comparison does not prove both copy source and destination ranges.
        return false;
    }
    if site.operation.family == OperationFamily::PointerArithmetic {
        return has_pointer_arithmetic_bounds_guard(&site.operation.expression, &guard_scope);
    }
    has_length_or_bounds_guard(&guard_scope)
}

pub(super) fn has_capacity_guard(family: &OperationFamily, lower: &str) -> bool {
    if family == &OperationFamily::VecSetLen {
        return set_len::has_set_len_capacity_evidence(lower);
    }
    if family == &OperationFamily::VecFromRawParts {
        return false;
    }
    lower.contains("capacity") || lower.contains("cap()")
}
