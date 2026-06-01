use crate::domain::{OperationFamily, SafetyObligation};

macro_rules! obligation {
    ($key:literal, $description:literal $(,)?) => {
        SafetyObligation {
            key: $key.to_string(),
            description: $description.to_string(),
        }
    };
}

pub(super) fn obligations_for(family: &OperationFamily) -> Vec<SafetyObligation> {
    match family {
        OperationFamily::RawPointerDeref
        | OperationFamily::RawPointerRead
        | OperationFamily::RawPointerWrite => raw_pointer_access_obligations(true),
        OperationFamily::RawPointerReadUnaligned | OperationFamily::RawPointerWriteUnaligned => {
            raw_pointer_access_obligations(false)
        }
        OperationFamily::SliceFromRawParts => vec![
            obligation!("pointer-live", "pointer is valid for `len` elements"),
            obligation!("alignment", "pointer is aligned for the element type"),
            obligation!("initialized", "memory range is initialized"),
            obligation!("allocation", "range fits in one allocation"),
        ],
        OperationFamily::VecFromRawParts => vec![
            obligation!(
                "pointer-live",
                "pointer was allocated by a compatible allocator for `capacity` elements",
            ),
            obligation!("alignment", "pointer is aligned for the element type"),
            obligation!("initialized", "first `len` elements are initialized"),
            obligation!("capacity", "`len` is at most `capacity`"),
            obligation!(
                "ownership",
                "the constructed Vec receives unique ownership and will not double-free",
            ),
        ],
        OperationFamily::MaybeUninitAssumeInit => vec![obligation!(
            "initialized",
            "all fields/elements are initialized and valid before `assume_init`",
        )],
        OperationFamily::VecSetLen => vec![
            obligation!("capacity", "new length is at most capacity"),
            obligation!(
                "initialized",
                "elements in the extended range are initialized",
            ),
        ],
        OperationFamily::Transmute => vec![
            obligation!("layout", "source and destination layouts are compatible"),
            obligation!(
                "valid-value",
                "destination value satisfies Rust validity rules",
            ),
        ],
        OperationFamily::Zeroed => vec![obligation!(
            "valid-zero",
            "all-zero bit pattern is a valid value for the target type",
        )],
        OperationFamily::DropInPlace => vec![
            obligation!("pointer-live", "pointer is valid for dropping one value"),
            obligation!("initialized", "pointed-to value is initialized"),
            obligation!(
                "ownership",
                "value will not be dropped again or observed after drop",
            ),
        ],
        OperationFamily::AtomicPointerState => vec![
            obligation!(
                "state-transition",
                "atomic pointer state transition preserves ownership invariants",
            ),
            obligation!(
                "ordering",
                "atomic ordering is strong enough for readers and drop paths",
            ),
        ],
        OperationFamily::UnwrapUnchecked => vec![obligation!(
            "valid-value",
            "value is known to be `Some` or `Ok` before `unwrap_unchecked`",
        )],
        OperationFamily::UnreachableUnchecked => vec![obligation!(
            "unreachable",
            "control flow cannot reach this path before `unreachable_unchecked`",
        )],
        OperationFamily::UnsafeFnCall => vec![obligation!(
            "callee-contract",
            "callee safety preconditions are satisfied",
        )],
        OperationFamily::CopyNonOverlapping => vec![
            obligation!("non-overlap", "source and destination do not overlap"),
            obligation!("valid-range", "both ranges are valid for count elements"),
        ],
        OperationFamily::PtrCopy => vec![
            obligation!("valid-range", "both ranges are valid for count elements"),
            obligation!(
                "initialized",
                "source range is initialized for count elements",
            ),
        ],
        OperationFamily::PtrReplace => vec![
            obligation!(
                "pointer-live",
                "destination pointer is valid for read and write",
            ),
            obligation!(
                "alignment",
                "destination pointer is aligned for the value type",
            ),
            obligation!(
                "initialized",
                "destination value is initialized before replace",
            ),
            obligation!(
                "ownership",
                "returned old value and replacement value preserve drop ownership",
            ),
        ],
        OperationFamily::UnsafeImplSendSync => vec![obligation!(
            "thread-safety",
            "internal mutation and aliasing invariants uphold Send/Sync contract",
        )],
        OperationFamily::Ffi => vec![
            obligation!(
                "abi",
                "foreign declaration matches ABI and layout on both sides",
            ),
            obligation!(
                "ownership",
                "ownership, lifetime, and nullability contract is explicit",
            ),
        ],
        OperationFamily::PinUnchecked => vec![obligation!(
            "pin",
            "value will not move and projections preserve pinning invariants",
        )],
        OperationFamily::GetUnchecked => {
            vec![obligation!(
                "bounds",
                "index is in bounds for the collection",
            )]
        }
        OperationFamily::BoxFromRaw => vec![obligation!(
            "ownership",
            "raw pointer was produced by compatible allocator and is uniquely owned",
        )],
        OperationFamily::PointerArithmetic => vec![obligation!(
            "bounds",
            "pointer arithmetic stays in-bounds or one-past inside the same allocation",
        )],
        OperationFamily::NonNullUnchecked => vec![obligation!(
            "non-null",
            "pointer is non-null before constructing NonNull",
        )],
        OperationFamily::StaticMut => vec![obligation!(
            "global-state",
            "all access is synchronized and does not violate aliasing rules",
        )],
        OperationFamily::InlineAsm => vec![obligation!(
            "asm",
            "inline assembly obeys register, memory, and target invariants",
        )],
        OperationFamily::TargetFeature => vec![obligation!(
            "target-feature",
            "callers only execute this path on supported hardware",
        )],
        OperationFamily::StrFromUtf8Unchecked => vec![obligation!("utf8", "bytes are valid UTF-8")],
        OperationFamily::Unknown => vec![obligation!(
            "unknown",
            "unsafe contract could not be inferred from this syntax shape",
        )],
    }
}

fn raw_pointer_access_obligations(include_alignment: bool) -> Vec<SafetyObligation> {
    let mut obligations = vec![
        obligation!(
            "pointer-live",
            "pointer is live and dereferenceable for the accessed type",
        ),
        obligation!("bounds", "buffer has enough bytes for the accessed type"),
    ];
    if include_alignment {
        obligations.push(obligation!(
            "alignment",
            "pointer is aligned for the accessed type",
        ));
    }
    obligations.extend([
        obligation!("initialized", "memory is initialized for the accessed type"),
        obligation!("allocation", "access remains inside one live allocation"),
    ]);
    obligations
}
