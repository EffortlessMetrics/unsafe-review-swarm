use crate::domain::{OperationFamily, ReviewCard};

use super::common;

const COPY_RANGE_GUARD_REPAIR: &str =
    "add guards proving the same `count` fits both source and destination ranges before this copy";

pub(super) fn add_operation_repairs(card: &ReviewCard, repairs: &mut Vec<String>) {
    match card.operation.family {
        OperationFamily::RawPointerDeref
        | OperationFamily::RawPointerRead
        | OperationFamily::RawPointerWrite => add_raw_pointer_repairs(card, repairs, true),
        OperationFamily::RawPointerReadUnaligned | OperationFamily::RawPointerWriteUnaligned => {
            add_raw_pointer_repairs(card, repairs, false)
        }
        OperationFamily::CopyNonOverlapping => {
            if common::missing_discharge(card, "valid-range") {
                repairs.push(COPY_RANGE_GUARD_REPAIR.to_string());
            }
            if common::missing_discharge(card, "non-overlap") {
                repairs.push("prove the same source and destination ranges do not overlap, or use `ptr::copy` only if overlap is intended".to_string());
            }
        }
        OperationFamily::PtrCopy => {
            if common::missing_discharge(card, "valid-range") {
                repairs.push(COPY_RANGE_GUARD_REPAIR.to_string());
            }
            if common::missing_discharge(card, "initialized") {
                repairs.push(
                    "show that the same source range is initialized for the copied element count"
                        .to_string(),
                );
            }
        }
        OperationFamily::PtrReplace => {
            if common::missing_discharge(card, "pointer-live") {
                repairs.push("prove the destination pointer is valid for both read and write before `ptr::replace`".to_string());
            }
            if common::missing_discharge(card, "alignment") {
                repairs.push(
                    "prove the destination pointer is aligned for the replaced value type"
                        .to_string(),
                );
            }
            if common::missing_discharge(card, "initialized") {
                repairs.push("show the destination slot contains an initialized old value before replacement".to_string());
            }
            if common::missing_discharge(card, "ownership") {
                repairs.push("show the returned old value and replacement value preserve drop ownership without double-drop or leak".to_string());
            }
        }
        OperationFamily::VecSetLen => {
            if common::missing_discharge(card, "capacity") {
                repairs.push(
                    "add a same-vector capacity guard before `set_len` for the requested length"
                        .to_string(),
                );
            }
            if common::missing_discharge(card, "initialized") {
                repairs.push("initialize the extended element range for this same vector and requested length before calling `set_len`".to_string());
            }
        }
        OperationFamily::MaybeUninitAssumeInit
            if common::missing_discharge(card, "initialized") =>
        {
            repairs.push(
                "write or construct the same `MaybeUninit` slot before `assume_init`".to_string(),
            );
            repairs.push("keep the initialization branch open to the unsafe site and do not reassign the slot afterward".to_string());
        }
        OperationFamily::Transmute => {
            if common::missing_discharge(card, "layout") {
                repairs.push(
                    "prove the source and destination layouts are compatible before this transmute"
                        .to_string(),
                );
            }
            if common::missing_discharge(card, "valid-value") {
                repairs.push("prove the source value is in the destination type's valid-value domain before this transmute".to_string());
            }
        }
        OperationFamily::Zeroed if common::missing_discharge(card, "valid-zero") => {
            repairs.push(
                "prove the all-zero bit pattern is valid for this target type before `zeroed`"
                    .to_string(),
            );
            repairs.push("prefer an explicit constructor or `MaybeUninit` path when zero is not a valid value".to_string());
        }
        OperationFamily::UnwrapUnchecked if common::missing_discharge(card, "valid-value") => {
            repairs.push("add a same-receiver `Some` or `Ok` guard on an open path before `unwrap_unchecked`".to_string());
            repairs.push(
                "preserve the same receiver value between the guard and `unwrap_unchecked`"
                    .to_string(),
            );
        }
        OperationFamily::UnreachableUnchecked if common::missing_discharge(card, "unreachable") => {
            repairs.push(
                "prove the same control-flow path is unreachable before `unreachable_unchecked`"
                    .to_string(),
            );
            repairs.push(
                "prefer a safe return, error, or panic path if reachability is uncertain"
                    .to_string(),
            );
        }
        OperationFamily::StrFromUtf8Unchecked if common::missing_discharge(card, "utf8") => {
            repairs.push("validate the same byte buffer as UTF-8 on an open path before calling `from_utf8_unchecked`".to_string());
            repairs.push(
                "preserve the same byte buffer between validation and the unchecked conversion"
                    .to_string(),
            );
        }
        OperationFamily::NonNullUnchecked if common::missing_discharge(card, "non-null") => {
            repairs.push(
                "add a same-pointer non-null guard before `NonNull::new_unchecked`".to_string(),
            );
            repairs.push(
                "preserve the same pointer value between the guard and `NonNull::new_unchecked`"
                    .to_string(),
            );
        }
        OperationFamily::GetUnchecked if common::missing_discharge(card, "bounds") => {
            repairs.push(
                "add a same-slice length/range guard before `get_unchecked` for the same index"
                    .to_string(),
            );
            repairs.push(
                "preserve the same index value between the guard and unchecked access".to_string(),
            );
        }
        OperationFamily::BoxFromRaw if common::missing_discharge(card, "ownership") => {
            repairs.push("prove the same raw pointer came from `Box::into_raw` with a compatible allocator before `Box::from_raw`".to_string());
            repairs.push("show unique ownership of that pointer so it will not be double-freed or reused after reconstruction".to_string());
        }
        OperationFamily::DropInPlace => {
            if common::missing_discharge(card, "pointer-live") {
                repairs.push("prove the pointer is live and valid for dropping one value before `drop_in_place`".to_string());
            }
            if common::missing_discharge(card, "initialized") {
                repairs.push(
                    "show the same pointed-to value is initialized before `drop_in_place`"
                        .to_string(),
                );
            }
            if common::missing_discharge(card, "ownership") {
                repairs.push("show ownership of the same pointee so it will not be dropped again or observed after `drop_in_place`".to_string());
            }
        }
        OperationFamily::SliceFromRawParts => {
            if common::missing_discharge(card, "pointer-live") {
                repairs.push("prove the same pointer is non-null and valid for `len` elements before `from_raw_parts`".to_string());
            }
            if common::missing_discharge(card, "alignment") {
                repairs.push(
                    "prove the same pointer is aligned for the slice element type".to_string(),
                );
            }
            if common::missing_discharge(card, "initialized") {
                repairs.push("show the entire same `ptr..ptr+len` range is initialized before constructing the slice".to_string());
            }
            if common::missing_discharge(card, "allocation") {
                repairs.push(
                    "show the same `ptr..ptr+len` range stays inside one live allocation"
                        .to_string(),
                );
            }
        }
        OperationFamily::VecFromRawParts => {
            if common::missing_discharge(card, "pointer-live") {
                repairs.push("prove the same pointer was allocated by a compatible allocator for `capacity` elements before `Vec::from_raw_parts`".to_string());
            }
            if common::missing_discharge(card, "alignment") {
                repairs
                    .push("prove the same pointer is aligned for the Vec element type".to_string());
            }
            if common::missing_discharge(card, "initialized") {
                repairs.push(
                "show the first `len` elements for this same pointer are initialized before reconstructing the Vec"
                    .to_string(),
            );
            }
            if common::missing_discharge(card, "capacity") {
                repairs
                    .push("add or expose a same-value guard proving `len <= capacity`".to_string());
            }
            if common::missing_discharge(card, "ownership") {
                repairs.push("show the reconstructed Vec receives unique ownership of these same raw parts and they will not be reused or double-freed".to_string());
            }
        }
        OperationFamily::PinUnchecked if common::missing_discharge(card, "pin") => {
            repairs.push("prove the value will not move after `Pin::new_unchecked`".to_string());
            repairs.push(
                "show projections preserve the same pinning invariant for this value".to_string(),
            );
            repairs.push("prefer a safe `Pin::new` or pinned-owner construction path when the invariant cannot be shown locally".to_string());
        }
        OperationFamily::UnsafeImplSendSync => {
            repairs.push("document or add evidence for the thread-safety invariant of this same unsafe impl owner and type-parameter bounds".to_string());
            repairs.push("route concurrency-sensitive evidence through Loom or Shuttle when the invariant depends on interleavings, and attach only a matching witness receipt after that run".to_string());
        }
        OperationFamily::AtomicPointerState => {
            if common::missing_discharge(card, "state-transition") {
                repairs.push("model the same atomic pointer state transition and ownership invariant in a focused Loom or Shuttle test".to_string());
            }
            if common::missing_discharge(card, "ordering") {
                repairs.push("show the chosen atomic ordering is strong enough for readers, writers, and drop paths".to_string());
            }
        }
        OperationFamily::Ffi => {
            repairs.push(
            "document the ABI, ownership, and lifetime contract for this same FFI boundary or call path"
                .to_string(),
        );
            repairs.push(
            "attach sanitizer or cargo-careful receipt evidence only after running the scoped command against this boundary; the receipt does not replace ABI or lifetime contract evidence"
                .to_string(),
        );
        }
        OperationFamily::TargetFeature if common::missing_discharge(card, "target-feature") => {
            repairs.push("prove callers reach this `target_feature` path only after a matching runtime or compile-time feature check".to_string());
            repairs.push("route unsupported callers to a non-`target_feature` fallback or keep dispatch behind explicit cfg/feature gating".to_string());
        }
        OperationFamily::StaticMut if common::missing_discharge(card, "global-state") => {
            repairs.push("prove all access to this `static mut` is synchronized or constrained to one execution context".to_string());
            repairs.push(
                "show the global state invariant avoids aliased mutable references and data races"
                    .to_string(),
            );
            repairs.push("prefer an `UnsafeCell`, atomic, lock, or one-time initialization owner when the invariant cannot be localized".to_string());
        }
        OperationFamily::InlineAsm if common::missing_discharge(card, "asm") => {
            repairs.push(
            "document the register, memory, clobber, options, and target-feature invariants for this same `asm!` block"
                .to_string(),
        );
            repairs.push(
            "prefer a safe intrinsic or narrower wrapper when this assembly invariant cannot be reviewed locally"
                .to_string(),
        );
        }
        OperationFamily::UnsafeFnCall if common::missing_discharge(card, "callee-contract") => {
            repairs.push("quote or link the callee safety contract and prove each precondition at this call site".to_string());
            repairs.push("preserve the same arguments and receiver between local guards and the unsafe function call".to_string());
            repairs.push("prefer a safe wrapper that enforces the callee preconditions before reaching this call".to_string());
        }
        _ => {}
    }
}

fn add_raw_pointer_repairs(card: &ReviewCard, repairs: &mut Vec<String>, alignment_required: bool) {
    common::add_if_missing_discharge(
        card,
        repairs,
        "pointer-live",
        "add a same-pointer live/nullability guard before this operation",
    );
    common::add_if_missing_discharge(
        card,
        repairs,
        "bounds",
        "add a same-pointer or same-buffer bounds guard before this operation",
    );
    if alignment_required && common::missing_discharge(card, "alignment") {
        repairs.push("add a same-pointer alignment guard, or switch to an unaligned operation only if unaligned input is intended".to_string());
    }
    common::add_if_missing_discharge(
        card,
        repairs,
        "initialized",
        "show that the same pointer or buffer range is initialized for the accessed type before this operation",
    );
    common::add_if_missing_discharge(
        card,
        repairs,
        "allocation",
        "show that the access stays inside one live allocation for this pointer",
    );
}
