use super::AllowedRepairs;
use crate::domain::{OperationFamily, ReviewCard};

pub(super) fn build(card: &ReviewCard) -> AllowedRepairs {
    let mut repairs = Vec::new();
    match card.operation.family {
        OperationFamily::RawPointerDeref | OperationFamily::RawPointerRead | OperationFamily::RawPointerWrite => add_raw_pointer_repairs(card, &mut repairs, true),
        OperationFamily::RawPointerReadUnaligned | OperationFamily::RawPointerWriteUnaligned => add_raw_pointer_repairs(card, &mut repairs, false),
        OperationFamily::CopyNonOverlapping => {
            if missing_discharge(card, "valid-range") { repairs.push("add guards proving `count` fits both source and destination ranges before this copy".to_string()); }
            if missing_discharge(card, "non-overlap") { repairs.push("prove source and destination ranges do not overlap, or use `ptr::copy` only if overlap is intended".to_string()); }
        }
        OperationFamily::PtrCopy => {
            if missing_discharge(card, "valid-range") { repairs.push("add guards proving `count` fits both source and destination ranges before this copy".to_string()); }
            if missing_discharge(card, "initialized") { repairs.push("show that the source range is initialized for the copied element count".to_string()); }
        }
        OperationFamily::VecSetLen => {
            if missing_discharge(card, "capacity") { repairs.push("add a same-vector capacity guard before `set_len` for the requested length".to_string()); }
            if missing_discharge(card, "initialized") { repairs.push("initialize the extended element range before calling `set_len`".to_string()); }
        }
        OperationFamily::MaybeUninitAssumeInit if missing_discharge(card, "initialized") => {
            repairs.push("write or construct the same `MaybeUninit` slot before `assume_init`".to_string());
            repairs.push("keep the initialization branch open to the unsafe site and do not reassign the slot afterward".to_string());
        }
        OperationFamily::Transmute => {
            if missing_discharge(card, "layout") {
                repairs.push("prove the source and destination layouts are compatible before this transmute".to_string());
            }
            if missing_discharge(card, "valid-value") {
                repairs.push("prove the source value is in the destination type's valid-value domain before this transmute".to_string());
            }
        }
        OperationFamily::Zeroed if missing_discharge(card, "valid-zero") => {
            repairs.push("prove the all-zero bit pattern is valid for this target type before `zeroed`".to_string());
            repairs.push("prefer an explicit constructor or `MaybeUninit` path when zero is not a valid value".to_string());
        }
        OperationFamily::UnwrapUnchecked if missing_discharge(card, "valid-value") => {
            repairs.push("add a same-receiver `Some` or `Ok` guard on an open path before `unwrap_unchecked`".to_string());
            repairs.push("preserve the same receiver value between the guard and `unwrap_unchecked`".to_string());
        }
        OperationFamily::UnreachableUnchecked if missing_discharge(card, "unreachable") => {
            repairs.push("prove the same control-flow path is unreachable before `unreachable_unchecked`".to_string());
            repairs.push("prefer a safe return, error, or panic path if reachability is uncertain".to_string());
        }
        OperationFamily::StrFromUtf8Unchecked if missing_discharge(card, "utf8") => repairs.push("validate the same byte buffer as UTF-8 on an open path before calling `from_utf8_unchecked`".to_string()),
        OperationFamily::NonNullUnchecked if missing_discharge(card, "non-null") => repairs.push("add a same-pointer non-null guard before `NonNull::new_unchecked`".to_string()),
        OperationFamily::GetUnchecked if missing_discharge(card, "bounds") => {
            repairs.push("add a same-slice length/range guard before `get_unchecked` for the same index".to_string());
            repairs.push("preserve the same index value between the guard and unchecked access".to_string());
        }
        OperationFamily::BoxFromRaw if missing_discharge(card, "ownership") => {
            repairs.push("prove the same raw pointer came from `Box::into_raw` with a compatible allocator before `Box::from_raw`".to_string());
            repairs.push("show unique ownership of that pointer so it will not be double-freed or reused after reconstruction".to_string());
        }
        OperationFamily::DropInPlace => {
            if missing_discharge(card, "pointer-live") {
                repairs.push("prove the pointer is live and valid for dropping one value before `drop_in_place`".to_string());
            }
            if missing_discharge(card, "initialized") {
                repairs.push("show the pointed-to value is initialized before `drop_in_place`".to_string());
            }
            if missing_discharge(card, "ownership") {
                repairs.push("show ownership of the pointee so it will not be dropped again or observed after `drop_in_place`".to_string());
            }
        }
        OperationFamily::PinUnchecked if missing_discharge(card, "pin") => {
            repairs.push("prove the value will not move after `Pin::new_unchecked`".to_string());
            repairs.push("show projections preserve the same pinning invariant for this value".to_string());
            repairs.push("prefer a safe `Pin::new` or pinned-owner construction path when the invariant cannot be shown locally".to_string());
        }
        OperationFamily::UnsafeImplSendSync => {
            repairs.push("document or add evidence for the thread-safety invariant of this unsafe impl".to_string());
            repairs.push("route concurrency-sensitive evidence through Loom or Shuttle when the invariant depends on interleavings".to_string());
        }
        OperationFamily::Ffi => {
            repairs.push("document the ABI, ownership, and lifetime contract for this FFI boundary".to_string());
            repairs.push("attach sanitizer or cargo-careful receipt evidence after running the scoped command outside unsafe-review".to_string());
        }
        OperationFamily::TargetFeature if missing_discharge(card, "target-feature") => {
            repairs.push("prove callers reach this `target_feature` path only after a matching runtime or compile-time feature check".to_string());
            repairs.push("route unsupported callers to a non-`target_feature` fallback or keep dispatch behind explicit cfg/feature gating".to_string());
        }
        OperationFamily::StaticMut if missing_discharge(card, "global-state") => {
            repairs.push("prove all access to this `static mut` is synchronized or constrained to one execution context".to_string());
            repairs.push("show the global state invariant avoids aliased mutable references and data races".to_string());
            repairs.push("prefer an `UnsafeCell`, atomic, lock, or one-time initialization owner when the invariant cannot be localized".to_string());
        }
        OperationFamily::InlineAsm if missing_discharge(card, "asm") => {
            repairs.push("document the register, memory, clobber, options, and target-feature invariants for this `asm!` block".to_string());
            repairs.push("prefer a safe intrinsic or narrower wrapper when the assembly invariant cannot be reviewed locally".to_string());
        }
        _ => {}
    }
    if missing_kind(card, "contract") {
        repairs.push("add or expose the local safety contract for this card".to_string());
    }
    if missing_kind(card, "test") {
        repairs
            .push("add or point to a focused test that exercises this owner or seam".to_string());
    }
    if missing_kind(card, "witness") {
        repairs.push("attach a scoped witness receipt after running the suggested command outside unsafe-review".to_string());
    }

    let has_card_scoped_repairs = !repairs.is_empty();
    if !has_card_scoped_repairs {
        repairs.push(card.next_action.summary.clone());
    }
    AllowedRepairs {
        repairs: dedupe_preserve_order(repairs),
        has_card_scoped_repairs,
    }
}

fn add_raw_pointer_repairs(card: &ReviewCard, repairs: &mut Vec<String>, alignment_required: bool) {
    if missing_discharge(card, "pointer-live") {
        repairs.push("add a same-pointer live/nullability guard before this operation".to_string());
    }
    if missing_discharge(card, "bounds") {
        repairs.push(
            "add a same-pointer or same-buffer bounds guard before this operation".to_string(),
        );
    }
    if alignment_required && missing_discharge(card, "alignment") {
        repairs.push("add a same-pointer alignment guard, or switch to an unaligned operation only if unaligned input is intended".to_string());
    }
    if missing_discharge(card, "initialized") {
        repairs.push(
            "show that memory is initialized for the accessed type before this operation"
                .to_string(),
        );
    }
    if missing_discharge(card, "allocation") {
        repairs.push(
            "show that the access stays inside one live allocation for this pointer".to_string(),
        );
    }
}
fn missing_discharge(card: &ReviewCard, key: &str) -> bool {
    card.obligation_evidence
        .iter()
        .any(|e| e.obligation.key == key && !e.discharge.present)
}
fn missing_kind(card: &ReviewCard, kind: &str) -> bool {
    card.missing.iter().any(|m| m.kind == kind)
}
fn dedupe_preserve_order(repairs: Vec<String>) -> Vec<String> {
    let mut deduped = Vec::new();
    for repair in repairs {
        if !deduped.contains(&repair) {
            deduped.push(repair);
        }
    }
    deduped
}
