use crate::domain::{OperationFamily, SafetyObligation};

use super::from_specs;

pub(super) fn obligations(family: &OperationFamily) -> Option<Vec<SafetyObligation>> {
    let specs = match family {
        OperationFamily::AtomicPointerState => &[
            (
                "state-transition",
                "atomic pointer state transition preserves ownership invariants",
            ),
            (
                "ordering",
                "atomic ordering is strong enough for readers and drop paths",
            ),
        ][..],
        OperationFamily::UnsafeFnCall => &[(
            "callee-contract",
            "callee safety preconditions are satisfied",
        )],
        OperationFamily::UnsafeImplSendSync => &[(
            "thread-safety",
            "internal mutation and aliasing invariants uphold Send/Sync contract",
        )],
        OperationFamily::Ffi => &[
            (
                "abi",
                "foreign declaration matches ABI and layout on both sides",
            ),
            (
                "ownership",
                "ownership, lifetime, and nullability contract is explicit",
            ),
        ],
        OperationFamily::PinUnchecked => &[(
            "pin",
            "value will not move and projections preserve pinning invariants",
        )],
        OperationFamily::StaticMut => &[(
            "global-state",
            "all access is synchronized and does not violate aliasing rules",
        )],
        OperationFamily::InlineAsm => &[(
            "asm",
            "inline assembly obeys register, memory, and target invariants",
        )],
        OperationFamily::TargetFeature => &[(
            "target-feature",
            "callers only execute this path on supported hardware",
        )],
        _ => return None,
    };
    Some(from_specs(specs))
}
