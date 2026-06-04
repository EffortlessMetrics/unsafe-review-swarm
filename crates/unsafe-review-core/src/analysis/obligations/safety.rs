use crate::domain::{OperationFamily, SafetyObligation};

pub(crate) fn obligations_for(family: &OperationFamily) -> Vec<SafetyObligation> {
    match family {
        OperationFamily::RawPointerDeref
        | OperationFamily::RawPointerRead
        | OperationFamily::RawPointerWrite => raw_pointer_obligations(true),
        OperationFamily::RawPointerReadUnaligned | OperationFamily::RawPointerWriteUnaligned => {
            raw_pointer_obligations(false)
        }
        OperationFamily::SliceFromRawParts => slice_from_raw_parts_obligations(),
        OperationFamily::VecFromRawParts => vec_from_raw_parts_obligations(),
        OperationFamily::MaybeUninitAssumeInit => single(SafetyObligation::new(
            "initialized",
            "all fields/elements are initialized and valid before `assume_init`",
        )),
        OperationFamily::VecSetLen => vec_set_len_obligations(),
        OperationFamily::Transmute => transmute_obligations(),
        OperationFamily::Zeroed => single(SafetyObligation::new(
            "valid-zero",
            "all-zero bit pattern is a valid value for the target type",
        )),
        OperationFamily::DropInPlace => drop_in_place_obligations(),
        OperationFamily::AtomicPointerState => atomic_pointer_state_obligations(),
        OperationFamily::UnwrapUnchecked => single(SafetyObligation::new(
            "valid-value",
            "value is known to be `Some` or `Ok` before `unwrap_unchecked`",
        )),
        OperationFamily::UnreachableUnchecked => single(SafetyObligation::new(
            "unreachable",
            "control flow cannot reach this path before `unreachable_unchecked`",
        )),
        OperationFamily::UnsafeFnCall => single(SafetyObligation::new(
            "callee-contract",
            "callee safety preconditions are satisfied",
        )),
        OperationFamily::CopyNonOverlapping => copy_nonoverlapping_obligations(),
        OperationFamily::PtrCopy => ptr_copy_obligations(),
        OperationFamily::PtrReplace => ptr_replace_obligations(),
        OperationFamily::UnsafeImplSendSync => single(SafetyObligation::new(
            "thread-safety",
            "internal mutation and aliasing invariants uphold Send/Sync contract",
        )),
        OperationFamily::Ffi => ffi_obligations(),
        OperationFamily::PinUnchecked => single(SafetyObligation::new(
            "pin",
            "value will not move and projections preserve pinning invariants",
        )),
        OperationFamily::GetUnchecked => single(SafetyObligation::new(
            "bounds",
            "index is in bounds for the collection",
        )),
        OperationFamily::BoxFromRaw => single(SafetyObligation::new(
            "ownership",
            "raw pointer was produced by compatible allocator and is uniquely owned",
        )),
        OperationFamily::PointerArithmetic => single(SafetyObligation::new(
            "bounds",
            "pointer arithmetic stays in-bounds or one-past inside the same allocation",
        )),
        OperationFamily::NonNullUnchecked => single(SafetyObligation::new(
            "non-null",
            "pointer is non-null before constructing NonNull",
        )),
        OperationFamily::StaticMut => single(SafetyObligation::new(
            "global-state",
            "all access is synchronized and does not violate aliasing rules",
        )),
        OperationFamily::InlineAsm => single(SafetyObligation::new(
            "asm",
            "inline assembly obeys register, memory, and target invariants",
        )),
        OperationFamily::TargetFeature => single(SafetyObligation::new(
            "target-feature",
            "callers only execute this path on supported hardware",
        )),
        OperationFamily::PanicFromSafeJs => single(SafetyObligation::new(
            "panic-guard",
            "JS-derived signed values are range-checked before panicking conversions",
        )),
        OperationFamily::StableByteSourceGetterReentry => single(SafetyObligation::new(
            "byte-stability",
            "JS-owned bytes remain stable after getter reentry and before Rust/native materialization",
        )),
        OperationFamily::StableByteSourceRabAsync => single(SafetyObligation::new(
            "byte-stability",
            "RAB-backed JS bytes are snapshotted before async worker or helper materialization",
        )),
        OperationFamily::StableByteSourceSabRace => single(SafetyObligation::new(
            "byte-stability",
            "shared JS bytes are snapshotted before Rust/native borrowed-slice materialization",
        )),
        OperationFamily::StableByteSourceNativeFfiRead => single(SafetyObligation::new(
            "byte-stability",
            "JS-backed bytes are snapshotted or otherwise stabilized before native FFI pointer/length reads",
        )),
        OperationFamily::StrFromUtf8Unchecked => {
            single(SafetyObligation::new("utf8", "bytes are valid UTF-8"))
        }
        OperationFamily::Unknown => single(SafetyObligation::new(
            "unknown",
            "unsafe contract could not be inferred from this syntax shape",
        )),
    }
}

fn single(obligation: SafetyObligation) -> Vec<SafetyObligation> {
    vec![obligation]
}

fn raw_pointer_obligations(include_alignment: bool) -> Vec<SafetyObligation> {
    let mut obligations = vec![
        SafetyObligation::new(
            "pointer-live",
            "pointer is live and dereferenceable for the accessed type",
        ),
        SafetyObligation::new("bounds", "buffer has enough bytes for the accessed type"),
    ];

    if include_alignment {
        obligations.push(SafetyObligation::new(
            "alignment",
            "pointer is aligned for the accessed type",
        ));
    }

    obligations.extend([
        SafetyObligation::new("initialized", "memory is initialized for the accessed type"),
        SafetyObligation::new("allocation", "access remains inside one live allocation"),
    ]);
    obligations
}

fn slice_from_raw_parts_obligations() -> Vec<SafetyObligation> {
    vec![
        SafetyObligation::new("pointer-live", "pointer is valid for `len` elements"),
        SafetyObligation::new("alignment", "pointer is aligned for the element type"),
        SafetyObligation::new("initialized", "memory range is initialized"),
        SafetyObligation::new("allocation", "range fits in one allocation"),
    ]
}

fn vec_from_raw_parts_obligations() -> Vec<SafetyObligation> {
    vec![
        SafetyObligation::new(
            "pointer-live",
            "pointer was allocated by a compatible allocator for `capacity` elements",
        ),
        SafetyObligation::new("alignment", "pointer is aligned for the element type"),
        SafetyObligation::new("initialized", "first `len` elements are initialized"),
        SafetyObligation::new("capacity", "`len` is at most `capacity`"),
        SafetyObligation::new(
            "ownership",
            "the constructed Vec receives unique ownership and will not double-free",
        ),
    ]
}

fn vec_set_len_obligations() -> Vec<SafetyObligation> {
    vec![
        SafetyObligation::new("capacity", "new length is at most capacity"),
        SafetyObligation::new(
            "initialized",
            "elements in the extended range are initialized",
        ),
    ]
}

fn transmute_obligations() -> Vec<SafetyObligation> {
    vec![
        SafetyObligation::new("layout", "source and destination layouts are compatible"),
        SafetyObligation::new(
            "valid-value",
            "destination value satisfies Rust validity rules",
        ),
    ]
}

fn drop_in_place_obligations() -> Vec<SafetyObligation> {
    vec![
        SafetyObligation::new("pointer-live", "pointer is valid for dropping one value"),
        SafetyObligation::new("initialized", "pointed-to value is initialized"),
        SafetyObligation::new(
            "ownership",
            "value will not be dropped again or observed after drop",
        ),
    ]
}

fn atomic_pointer_state_obligations() -> Vec<SafetyObligation> {
    vec![
        SafetyObligation::new(
            "state-transition",
            "atomic pointer state transition preserves ownership invariants",
        ),
        SafetyObligation::new(
            "ordering",
            "atomic ordering is strong enough for readers and drop paths",
        ),
    ]
}

fn copy_nonoverlapping_obligations() -> Vec<SafetyObligation> {
    vec![
        SafetyObligation::new("non-overlap", "source and destination do not overlap"),
        SafetyObligation::new("valid-range", "both ranges are valid for count elements"),
    ]
}

fn ptr_copy_obligations() -> Vec<SafetyObligation> {
    vec![
        SafetyObligation::new("valid-range", "both ranges are valid for count elements"),
        SafetyObligation::new(
            "initialized",
            "source range is initialized for count elements",
        ),
    ]
}

fn ptr_replace_obligations() -> Vec<SafetyObligation> {
    vec![
        SafetyObligation::new(
            "pointer-live",
            "destination pointer is valid for read and write",
        ),
        SafetyObligation::new(
            "alignment",
            "destination pointer is aligned for the value type",
        ),
        SafetyObligation::new(
            "initialized",
            "destination value is initialized before replace",
        ),
        SafetyObligation::new(
            "ownership",
            "returned old value and replacement value preserve drop ownership",
        ),
    ]
}

fn ffi_obligations() -> Vec<SafetyObligation> {
    vec![
        SafetyObligation::new(
            "abi",
            "foreign declaration matches ABI and layout on both sides",
        ),
        SafetyObligation::new(
            "ownership",
            "ownership, lifetime, and nullability contract is explicit",
        ),
    ]
}
