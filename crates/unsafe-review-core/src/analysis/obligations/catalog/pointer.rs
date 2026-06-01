use crate::domain::{OperationFamily, SafetyObligation};

use super::from_specs;

pub(super) fn obligations(family: &OperationFamily) -> Option<Vec<SafetyObligation>> {
    let specs = match family {
        OperationFamily::RawPointerDeref
        | OperationFamily::RawPointerRead
        | OperationFamily::RawPointerWrite => &[
            (
                "pointer-live",
                "pointer is live and dereferenceable for the accessed type",
            ),
            ("bounds", "buffer has enough bytes for the accessed type"),
            ("alignment", "pointer is aligned for the accessed type"),
            ("initialized", "memory is initialized for the accessed type"),
            ("allocation", "access remains inside one live allocation"),
        ][..],
        OperationFamily::RawPointerReadUnaligned | OperationFamily::RawPointerWriteUnaligned => &[
            (
                "pointer-live",
                "pointer is live and dereferenceable for the accessed type",
            ),
            ("bounds", "buffer has enough bytes for the accessed type"),
            ("initialized", "memory is initialized for the accessed type"),
            ("allocation", "access remains inside one live allocation"),
        ],
        OperationFamily::SliceFromRawParts => &[
            ("pointer-live", "pointer is valid for `len` elements"),
            ("alignment", "pointer is aligned for the element type"),
            ("initialized", "memory range is initialized"),
            ("allocation", "range fits in one allocation"),
        ],
        OperationFamily::PointerArithmetic => &[(
            "bounds",
            "pointer arithmetic stays in-bounds or one-past inside the same allocation",
        )],
        OperationFamily::NonNullUnchecked => &[(
            "non-null",
            "pointer is non-null before constructing NonNull",
        )],
        OperationFamily::GetUnchecked => &[("bounds", "index is in bounds for the collection")],
        _ => return None,
    };
    Some(from_specs(specs))
}
