use super::push_if_missing_discharge;
use crate::domain::{OperationFamily, ReviewCard};

pub(super) fn add_for_family(card: &ReviewCard, repairs: &mut Vec<String>) {
    match card.operation.family {
        OperationFamily::UnsafeImplSendSync => add_unsafe_impl_repairs(repairs),
        OperationFamily::Ffi => add_ffi_repairs(repairs),
        OperationFamily::TargetFeature => add_target_feature_repairs(card, repairs),
        OperationFamily::StaticMut => add_static_mut_repairs(card, repairs),
        OperationFamily::InlineAsm => add_inline_asm_repairs(card, repairs),
        OperationFamily::UnsafeFnCall => add_unsafe_fn_call_repairs(card, repairs),
        _ => {}
    }
}

fn add_unsafe_impl_repairs(repairs: &mut Vec<String>) {
    repairs.push("document or add evidence for the thread-safety invariant of this same unsafe impl owner and type-parameter bounds".to_string());
    repairs.push("route concurrency-sensitive evidence through Loom or Shuttle when the invariant depends on interleavings, and attach only a matching witness receipt after that run".to_string());
}

fn add_ffi_repairs(repairs: &mut Vec<String>) {
    repairs.push(
        "document the ABI, ownership, and lifetime contract for this same FFI boundary or call path"
            .to_string(),
    );
    repairs.push(
        "attach sanitizer or cargo-careful receipt evidence only after running the scoped command against this boundary; the receipt does not replace ABI or lifetime contract evidence"
            .to_string(),
    );
}

fn add_target_feature_repairs(card: &ReviewCard, repairs: &mut Vec<String>) {
    push_if_missing_discharge(
        card,
        repairs,
        "target-feature",
        "prove callers reach this `target_feature` path only after a matching runtime or compile-time feature check",
    );
    push_if_missing_discharge(
        card,
        repairs,
        "target-feature",
        "route unsupported callers to a non-`target_feature` fallback or keep dispatch behind explicit cfg/feature gating",
    );
}

fn add_static_mut_repairs(card: &ReviewCard, repairs: &mut Vec<String>) {
    push_if_missing_discharge(
        card,
        repairs,
        "global-state",
        "prove all access to this `static mut` is synchronized or constrained to one execution context",
    );
    push_if_missing_discharge(
        card,
        repairs,
        "global-state",
        "show the global state invariant avoids aliased mutable references and data races",
    );
    push_if_missing_discharge(
        card,
        repairs,
        "global-state",
        "prefer an `UnsafeCell`, atomic, lock, or one-time initialization owner when the invariant cannot be localized",
    );
}

fn add_inline_asm_repairs(card: &ReviewCard, repairs: &mut Vec<String>) {
    push_if_missing_discharge(
        card,
        repairs,
        "asm",
        "document the register, memory, clobber, options, and target-feature invariants for this same `asm!` block",
    );
    push_if_missing_discharge(
        card,
        repairs,
        "asm",
        "prefer a safe intrinsic or narrower wrapper when this assembly invariant cannot be reviewed locally",
    );
}

fn add_unsafe_fn_call_repairs(card: &ReviewCard, repairs: &mut Vec<String>) {
    push_if_missing_discharge(
        card,
        repairs,
        "callee-contract",
        "quote or link the callee safety contract and prove each precondition at this call site",
    );
    push_if_missing_discharge(
        card,
        repairs,
        "callee-contract",
        "preserve the same arguments and receiver between local guards and the unsafe function call",
    );
    push_if_missing_discharge(
        card,
        repairs,
        "callee-contract",
        "prefer a safe wrapper that enforces the callee preconditions before reaching this call",
    );
}
