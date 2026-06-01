use crate::domain::{OperationFamily, SafetyObligation};

use super::from_specs;

pub(super) fn obligations(family: &OperationFamily) -> Option<Vec<SafetyObligation>> {
    let specs = match family {
        OperationFamily::VecFromRawParts => &[
            (
                "pointer-live",
                "pointer was allocated by a compatible allocator for `capacity` elements",
            ),
            ("alignment", "pointer is aligned for the element type"),
            ("initialized", "first `len` elements are initialized"),
            ("capacity", "`len` is at most `capacity`"),
            (
                "ownership",
                "the constructed Vec receives unique ownership and will not double-free",
            ),
        ][..],
        OperationFamily::MaybeUninitAssumeInit => &[(
            "initialized",
            "all fields/elements are initialized and valid before `assume_init`",
        )],
        OperationFamily::VecSetLen => &[
            ("capacity", "new length is at most capacity"),
            (
                "initialized",
                "elements in the extended range are initialized",
            ),
        ],
        OperationFamily::DropInPlace => &[
            ("pointer-live", "pointer is valid for dropping one value"),
            ("initialized", "pointed-to value is initialized"),
            (
                "ownership",
                "value will not be dropped again or observed after drop",
            ),
        ],
        OperationFamily::CopyNonOverlapping => &[
            ("non-overlap", "source and destination do not overlap"),
            ("valid-range", "both ranges are valid for count elements"),
        ],
        OperationFamily::PtrCopy => &[
            ("valid-range", "both ranges are valid for count elements"),
            (
                "initialized",
                "source range is initialized for count elements",
            ),
        ],
        OperationFamily::PtrReplace => &[
            (
                "pointer-live",
                "destination pointer is valid for read and write",
            ),
            (
                "alignment",
                "destination pointer is aligned for the value type",
            ),
            (
                "initialized",
                "destination value is initialized before replace",
            ),
            (
                "ownership",
                "returned old value and replacement value preserve drop ownership",
            ),
        ],
        OperationFamily::BoxFromRaw => &[(
            "ownership",
            "raw pointer was produced by compatible allocator and is uniquely owned",
        )],
        _ => return None,
    };
    Some(from_specs(specs))
}
