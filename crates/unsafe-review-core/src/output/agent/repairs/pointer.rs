use super::push_if_missing_discharge;
use crate::domain::{OperationFamily, ReviewCard};

pub(super) fn add_for_family(card: &ReviewCard, repairs: &mut Vec<String>) {
    match card.operation.family {
        OperationFamily::RawPointerDeref
        | OperationFamily::RawPointerRead
        | OperationFamily::RawPointerWrite => add_raw_pointer_repairs(card, repairs, true),
        OperationFamily::RawPointerReadUnaligned | OperationFamily::RawPointerWriteUnaligned => {
            add_raw_pointer_repairs(card, repairs, false)
        }
        OperationFamily::CopyNonOverlapping => add_copy_non_overlapping_repairs(card, repairs),
        OperationFamily::PtrCopy => add_ptr_copy_repairs(card, repairs),
        OperationFamily::PtrReplace => add_ptr_replace_repairs(card, repairs),
        OperationFamily::DropInPlace => add_drop_in_place_repairs(card, repairs),
        OperationFamily::SliceFromRawParts => add_slice_from_raw_parts_repairs(card, repairs),
        OperationFamily::VecFromRawParts => add_vec_from_raw_parts_repairs(card, repairs),
        OperationFamily::PinUnchecked => add_pin_unchecked_repairs(card, repairs),
        OperationFamily::AtomicPointerState => add_atomic_pointer_state_repairs(card, repairs),
        _ => {}
    }
}

fn add_raw_pointer_repairs(card: &ReviewCard, repairs: &mut Vec<String>, alignment_required: bool) {
    push_if_missing_discharge(
        card,
        repairs,
        "pointer-live",
        "add a same-pointer live/nullability guard before this operation",
    );
    push_if_missing_discharge(
        card,
        repairs,
        "bounds",
        "add a same-pointer or same-buffer bounds guard before this operation",
    );
    if alignment_required {
        push_if_missing_discharge(
            card,
            repairs,
            "alignment",
            "add a same-pointer alignment guard, or switch to an unaligned operation only if unaligned input is intended",
        );
    }
    push_if_missing_discharge(
        card,
        repairs,
        "initialized",
        "show that the same pointer or buffer range is initialized for the accessed type before this operation",
    );
    push_if_missing_discharge(
        card,
        repairs,
        "allocation",
        "show that the access stays inside one live allocation for this pointer",
    );
}

fn add_copy_non_overlapping_repairs(card: &ReviewCard, repairs: &mut Vec<String>) {
    add_copy_range_repair(card, repairs);
    push_if_missing_discharge(
        card,
        repairs,
        "non-overlap",
        "prove the same source and destination ranges do not overlap, or use `ptr::copy` only if overlap is intended",
    );
}

fn add_ptr_copy_repairs(card: &ReviewCard, repairs: &mut Vec<String>) {
    add_copy_range_repair(card, repairs);
    push_if_missing_discharge(
        card,
        repairs,
        "initialized",
        "show that the same source range is initialized for the copied element count",
    );
}

fn add_copy_range_repair(card: &ReviewCard, repairs: &mut Vec<String>) {
    push_if_missing_discharge(
        card,
        repairs,
        "valid-range",
        "add guards proving the same `count` fits both source and destination ranges before this copy",
    );
}

fn add_ptr_replace_repairs(card: &ReviewCard, repairs: &mut Vec<String>) {
    push_if_missing_discharge(
        card,
        repairs,
        "pointer-live",
        "prove the destination pointer is valid for both read and write before `ptr::replace`",
    );
    push_if_missing_discharge(
        card,
        repairs,
        "alignment",
        "prove the destination pointer is aligned for the replaced value type",
    );
    push_if_missing_discharge(
        card,
        repairs,
        "initialized",
        "show the destination slot contains an initialized old value before replacement",
    );
    push_if_missing_discharge(
        card,
        repairs,
        "ownership",
        "show the returned old value and replacement value preserve drop ownership without double-drop or leak",
    );
}

fn add_drop_in_place_repairs(card: &ReviewCard, repairs: &mut Vec<String>) {
    push_if_missing_discharge(
        card,
        repairs,
        "pointer-live",
        "prove the pointer is live and valid for dropping one value before `drop_in_place`",
    );
    push_if_missing_discharge(
        card,
        repairs,
        "initialized",
        "show the same pointed-to value is initialized before `drop_in_place`",
    );
    push_if_missing_discharge(
        card,
        repairs,
        "ownership",
        "show ownership of the same pointee so it will not be dropped again or observed after `drop_in_place`",
    );
}

fn add_slice_from_raw_parts_repairs(card: &ReviewCard, repairs: &mut Vec<String>) {
    push_if_missing_discharge(
        card,
        repairs,
        "pointer-live",
        "prove the same pointer is non-null and valid for `len` elements before `from_raw_parts`",
    );
    push_if_missing_discharge(
        card,
        repairs,
        "alignment",
        "prove the same pointer is aligned for the slice element type",
    );
    push_if_missing_discharge(
        card,
        repairs,
        "initialized",
        "show the entire same `ptr..ptr+len` range is initialized before constructing the slice",
    );
    push_if_missing_discharge(
        card,
        repairs,
        "allocation",
        "show the same `ptr..ptr+len` range stays inside one live allocation",
    );
}

fn add_vec_from_raw_parts_repairs(card: &ReviewCard, repairs: &mut Vec<String>) {
    push_if_missing_discharge(
        card,
        repairs,
        "pointer-live",
        "prove the same pointer was allocated by a compatible allocator for `capacity` elements before `Vec::from_raw_parts`",
    );
    push_if_missing_discharge(
        card,
        repairs,
        "alignment",
        "prove the same pointer is aligned for the Vec element type",
    );
    push_if_missing_discharge(
        card,
        repairs,
        "initialized",
        "show the first `len` elements for this same pointer are initialized before reconstructing the Vec",
    );
    push_if_missing_discharge(
        card,
        repairs,
        "capacity",
        "add or expose a same-value guard proving `len <= capacity`",
    );
    push_if_missing_discharge(
        card,
        repairs,
        "ownership",
        "show the reconstructed Vec receives unique ownership of these same raw parts and they will not be reused or double-freed",
    );
}

fn add_pin_unchecked_repairs(card: &ReviewCard, repairs: &mut Vec<String>) {
    push_if_missing_discharge(
        card,
        repairs,
        "pin",
        "prove the value will not move after `Pin::new_unchecked`",
    );
    push_if_missing_discharge(
        card,
        repairs,
        "pin",
        "show projections preserve the same pinning invariant for this value",
    );
    push_if_missing_discharge(
        card,
        repairs,
        "pin",
        "prefer a safe `Pin::new` or pinned-owner construction path when the invariant cannot be shown locally",
    );
}

fn add_atomic_pointer_state_repairs(card: &ReviewCard, repairs: &mut Vec<String>) {
    push_if_missing_discharge(
        card,
        repairs,
        "state-transition",
        "model the same atomic pointer state transition and ownership invariant in a focused Loom or Shuttle test",
    );
    push_if_missing_discharge(
        card,
        repairs,
        "ordering",
        "show the chosen atomic ordering is strong enough for readers, writers, and drop paths",
    );
}
