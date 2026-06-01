use crate::domain::{OperationFamily, ReviewCard};

use super::repair_list::RepairList;

pub(super) fn add_for_card(card: &ReviewCard, repairs: &mut RepairList) {
    match card.operation.family {
        OperationFamily::RawPointerDeref
        | OperationFamily::RawPointerRead
        | OperationFamily::RawPointerWrite => add_raw_pointer_repairs(card, repairs, true),
        OperationFamily::RawPointerReadUnaligned | OperationFamily::RawPointerWriteUnaligned => {
            add_raw_pointer_repairs(card, repairs, false)
        }
        OperationFamily::CopyNonOverlapping => add_copy_nonoverlapping_repairs(card, repairs),
        OperationFamily::PtrCopy => add_ptr_copy_repairs(card, repairs),
        OperationFamily::PtrReplace => add_ptr_replace_repairs(card, repairs),
        OperationFamily::VecSetLen => add_vec_set_len_repairs(card, repairs),
        OperationFamily::MaybeUninitAssumeInit => {
            add_maybe_uninit_assume_init_repairs(card, repairs)
        }
        OperationFamily::Transmute => add_transmute_repairs(card, repairs),
        OperationFamily::Zeroed => add_zeroed_repairs(card, repairs),
        OperationFamily::UnwrapUnchecked => add_unwrap_unchecked_repairs(card, repairs),
        OperationFamily::UnreachableUnchecked => add_unreachable_unchecked_repairs(card, repairs),
        OperationFamily::StrFromUtf8Unchecked => add_str_from_utf8_unchecked_repairs(card, repairs),
        OperationFamily::NonNullUnchecked => add_non_null_unchecked_repairs(card, repairs),
        OperationFamily::GetUnchecked => add_get_unchecked_repairs(card, repairs),
        OperationFamily::BoxFromRaw => add_box_from_raw_repairs(card, repairs),
        OperationFamily::DropInPlace => add_drop_in_place_repairs(card, repairs),
        OperationFamily::SliceFromRawParts => add_slice_from_raw_parts_repairs(card, repairs),
        OperationFamily::VecFromRawParts => add_vec_from_raw_parts_repairs(card, repairs),
        OperationFamily::PinUnchecked => add_pin_unchecked_repairs(card, repairs),
        OperationFamily::UnsafeImplSendSync => add_unsafe_impl_send_sync_repairs(repairs),
        OperationFamily::AtomicPointerState => add_atomic_pointer_state_repairs(card, repairs),
        OperationFamily::Ffi => add_ffi_repairs(repairs),
        OperationFamily::TargetFeature => add_target_feature_repairs(card, repairs),
        OperationFamily::StaticMut => add_static_mut_repairs(card, repairs),
        OperationFamily::InlineAsm => add_inline_asm_repairs(card, repairs),
        OperationFamily::UnsafeFnCall => add_unsafe_fn_call_repairs(card, repairs),
        _ => {}
    }
}

fn add_raw_pointer_repairs(card: &ReviewCard, repairs: &mut RepairList, alignment_required: bool) {
    repairs.push_if_missing_discharge(
        card,
        "pointer-live",
        "add a same-pointer live/nullability guard before this operation",
    );
    repairs.push_if_missing_discharge(
        card,
        "bounds",
        "add a same-pointer or same-buffer bounds guard before this operation",
    );
    if alignment_required && repairs.missing_discharge(card, "alignment") {
        repairs.push("add a same-pointer alignment guard, or switch to an unaligned operation only if unaligned input is intended");
    }
    repairs.push_if_missing_discharge(
        card,
        "initialized",
        "show that the same pointer or buffer range is initialized for the accessed type before this operation",
    );
    repairs.push_if_missing_discharge(
        card,
        "allocation",
        "show that the access stays inside one live allocation for this pointer",
    );
}

fn add_copy_nonoverlapping_repairs(card: &ReviewCard, repairs: &mut RepairList) {
    add_copy_range_repair(card, repairs);
    repairs.push_if_missing_discharge(
        card,
        "non-overlap",
        "prove the same source and destination ranges do not overlap, or use `ptr::copy` only if overlap is intended",
    );
}

fn add_ptr_copy_repairs(card: &ReviewCard, repairs: &mut RepairList) {
    add_copy_range_repair(card, repairs);
    repairs.push_if_missing_discharge(
        card,
        "initialized",
        "show that the same source range is initialized for the copied element count",
    );
}

fn add_copy_range_repair(card: &ReviewCard, repairs: &mut RepairList) {
    repairs.push_if_missing_discharge(
        card,
        "valid-range",
        "add guards proving the same `count` fits both source and destination ranges before this copy",
    );
}

fn add_ptr_replace_repairs(card: &ReviewCard, repairs: &mut RepairList) {
    repairs.push_if_missing_discharge(
        card,
        "pointer-live",
        "prove the destination pointer is valid for both read and write before `ptr::replace`",
    );
    repairs.push_if_missing_discharge(
        card,
        "alignment",
        "prove the destination pointer is aligned for the replaced value type",
    );
    repairs.push_if_missing_discharge(
        card,
        "initialized",
        "show the destination slot contains an initialized old value before replacement",
    );
    repairs.push_if_missing_discharge(
        card,
        "ownership",
        "show the returned old value and replacement value preserve drop ownership without double-drop or leak",
    );
}

fn add_vec_set_len_repairs(card: &ReviewCard, repairs: &mut RepairList) {
    repairs.push_if_missing_discharge(
        card,
        "capacity",
        "add a same-vector capacity guard before `set_len` for the requested length",
    );
    repairs.push_if_missing_discharge(
        card,
        "initialized",
        "initialize the extended element range for this same vector and requested length before calling `set_len`",
    );
}

fn add_maybe_uninit_assume_init_repairs(card: &ReviewCard, repairs: &mut RepairList) {
    if repairs.missing_discharge(card, "initialized") {
        repairs.push("write or construct the same `MaybeUninit` slot before `assume_init`");
        repairs.push("keep the initialization branch open to the unsafe site and do not reassign the slot afterward");
    }
}

fn add_transmute_repairs(card: &ReviewCard, repairs: &mut RepairList) {
    repairs.push_if_missing_discharge(
        card,
        "layout",
        "prove the source and destination layouts are compatible before this transmute",
    );
    repairs.push_if_missing_discharge(
        card,
        "valid-value",
        "prove the source value is in the destination type's valid-value domain before this transmute",
    );
}

fn add_zeroed_repairs(card: &ReviewCard, repairs: &mut RepairList) {
    if repairs.missing_discharge(card, "valid-zero") {
        repairs
            .push("prove the all-zero bit pattern is valid for this target type before `zeroed`");
        repairs.push(
            "prefer an explicit constructor or `MaybeUninit` path when zero is not a valid value",
        );
    }
}

fn add_unwrap_unchecked_repairs(card: &ReviewCard, repairs: &mut RepairList) {
    if repairs.missing_discharge(card, "valid-value") {
        repairs.push(
            "add a same-receiver `Some` or `Ok` guard on an open path before `unwrap_unchecked`",
        );
        repairs.push("preserve the same receiver value between the guard and `unwrap_unchecked`");
    }
}

fn add_unreachable_unchecked_repairs(card: &ReviewCard, repairs: &mut RepairList) {
    if repairs.missing_discharge(card, "unreachable") {
        repairs
            .push("prove the same control-flow path is unreachable before `unreachable_unchecked`");
        repairs.push("prefer a safe return, error, or panic path if reachability is uncertain");
    }
}

fn add_str_from_utf8_unchecked_repairs(card: &ReviewCard, repairs: &mut RepairList) {
    if repairs.missing_discharge(card, "utf8") {
        repairs.push("validate the same byte buffer as UTF-8 on an open path before calling `from_utf8_unchecked`");
        repairs
            .push("preserve the same byte buffer between validation and the unchecked conversion");
    }
}

fn add_non_null_unchecked_repairs(card: &ReviewCard, repairs: &mut RepairList) {
    if repairs.missing_discharge(card, "non-null") {
        repairs.push("add a same-pointer non-null guard before `NonNull::new_unchecked`");
        repairs
            .push("preserve the same pointer value between the guard and `NonNull::new_unchecked`");
    }
}

fn add_get_unchecked_repairs(card: &ReviewCard, repairs: &mut RepairList) {
    if repairs.missing_discharge(card, "bounds") {
        repairs
            .push("add a same-slice length/range guard before `get_unchecked` for the same index");
        repairs.push("preserve the same index value between the guard and unchecked access");
    }
}

fn add_box_from_raw_repairs(card: &ReviewCard, repairs: &mut RepairList) {
    if repairs.missing_discharge(card, "ownership") {
        repairs.push("prove the same raw pointer came from `Box::into_raw` with a compatible allocator before `Box::from_raw`");
        repairs.push("show unique ownership of that pointer so it will not be double-freed or reused after reconstruction");
    }
}

fn add_drop_in_place_repairs(card: &ReviewCard, repairs: &mut RepairList) {
    repairs.push_if_missing_discharge(
        card,
        "pointer-live",
        "prove the pointer is live and valid for dropping one value before `drop_in_place`",
    );
    repairs.push_if_missing_discharge(
        card,
        "initialized",
        "show the same pointed-to value is initialized before `drop_in_place`",
    );
    repairs.push_if_missing_discharge(
        card,
        "ownership",
        "show ownership of the same pointee so it will not be dropped again or observed after `drop_in_place`",
    );
}

fn add_slice_from_raw_parts_repairs(card: &ReviewCard, repairs: &mut RepairList) {
    repairs.push_if_missing_discharge(
        card,
        "pointer-live",
        "prove the same pointer is non-null and valid for `len` elements before `from_raw_parts`",
    );
    repairs.push_if_missing_discharge(
        card,
        "alignment",
        "prove the same pointer is aligned for the slice element type",
    );
    repairs.push_if_missing_discharge(
        card,
        "initialized",
        "show the entire same `ptr..ptr+len` range is initialized before constructing the slice",
    );
    repairs.push_if_missing_discharge(
        card,
        "allocation",
        "show the same `ptr..ptr+len` range stays inside one live allocation",
    );
}

fn add_vec_from_raw_parts_repairs(card: &ReviewCard, repairs: &mut RepairList) {
    repairs.push_if_missing_discharge(
        card,
        "pointer-live",
        "prove the same pointer was allocated by a compatible allocator for `capacity` elements before `Vec::from_raw_parts`",
    );
    repairs.push_if_missing_discharge(
        card,
        "alignment",
        "prove the same pointer is aligned for the Vec element type",
    );
    repairs.push_if_missing_discharge(
        card,
        "initialized",
        "show the first `len` elements for this same pointer are initialized before reconstructing the Vec",
    );
    repairs.push_if_missing_discharge(
        card,
        "capacity",
        "add or expose a same-value guard proving `len <= capacity`",
    );
    repairs.push_if_missing_discharge(
        card,
        "ownership",
        "show the reconstructed Vec receives unique ownership of these same raw parts and they will not be reused or double-freed",
    );
}

fn add_pin_unchecked_repairs(card: &ReviewCard, repairs: &mut RepairList) {
    if repairs.missing_discharge(card, "pin") {
        repairs.push("prove the value will not move after `Pin::new_unchecked`");
        repairs.push("show projections preserve the same pinning invariant for this value");
        repairs.push("prefer a safe `Pin::new` or pinned-owner construction path when the invariant cannot be shown locally");
    }
}

fn add_unsafe_impl_send_sync_repairs(repairs: &mut RepairList) {
    repairs.push("document or add evidence for the thread-safety invariant of this same unsafe impl owner and type-parameter bounds");
    repairs.push("route concurrency-sensitive evidence through Loom or Shuttle when the invariant depends on interleavings, and attach only a matching witness receipt after that run");
}

fn add_atomic_pointer_state_repairs(card: &ReviewCard, repairs: &mut RepairList) {
    repairs.push_if_missing_discharge(
        card,
        "state-transition",
        "model the same atomic pointer state transition and ownership invariant in a focused Loom or Shuttle test",
    );
    repairs.push_if_missing_discharge(
        card,
        "ordering",
        "show the chosen atomic ordering is strong enough for readers, writers, and drop paths",
    );
}

fn add_ffi_repairs(repairs: &mut RepairList) {
    repairs.push("document the ABI, ownership, and lifetime contract for this same FFI boundary or call path");
    repairs.push("attach sanitizer or cargo-careful receipt evidence only after running the scoped command against this boundary; the receipt does not replace ABI or lifetime contract evidence");
}

fn add_target_feature_repairs(card: &ReviewCard, repairs: &mut RepairList) {
    if repairs.missing_discharge(card, "target-feature") {
        repairs.push("prove callers reach this `target_feature` path only after a matching runtime or compile-time feature check");
        repairs.push("route unsupported callers to a non-`target_feature` fallback or keep dispatch behind explicit cfg/feature gating");
    }
}

fn add_static_mut_repairs(card: &ReviewCard, repairs: &mut RepairList) {
    if repairs.missing_discharge(card, "global-state") {
        repairs.push("prove all access to this `static mut` is synchronized or constrained to one execution context");
        repairs.push(
            "show the global state invariant avoids aliased mutable references and data races",
        );
        repairs.push("prefer an `UnsafeCell`, atomic, lock, or one-time initialization owner when the invariant cannot be localized");
    }
}

fn add_inline_asm_repairs(card: &ReviewCard, repairs: &mut RepairList) {
    if repairs.missing_discharge(card, "asm") {
        repairs.push("document the register, memory, clobber, options, and target-feature invariants for this same `asm!` block");
        repairs.push("prefer a safe intrinsic or narrower wrapper when this assembly invariant cannot be reviewed locally");
    }
}

fn add_unsafe_fn_call_repairs(card: &ReviewCard, repairs: &mut RepairList) {
    if repairs.missing_discharge(card, "callee-contract") {
        repairs.push("quote or link the callee safety contract and prove each precondition at this call site");
        repairs.push("preserve the same arguments and receiver between local guards and the unsafe function call");
        repairs.push("prefer a safe wrapper that enforces the callee preconditions before reaching this call");
    }
}
