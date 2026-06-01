use crate::domain::{OperationFamily, SafetyObligation};

use super::from_specs;

pub(super) fn obligations(family: &OperationFamily) -> Option<Vec<SafetyObligation>> {
    let specs = match family {
        OperationFamily::Transmute => &[
            ("layout", "source and destination layouts are compatible"),
            (
                "valid-value",
                "destination value satisfies Rust validity rules",
            ),
        ][..],
        OperationFamily::Zeroed => &[(
            "valid-zero",
            "all-zero bit pattern is a valid value for the target type",
        )],
        OperationFamily::UnwrapUnchecked => &[(
            "valid-value",
            "value is known to be `Some` or `Ok` before `unwrap_unchecked`",
        )],
        OperationFamily::UnreachableUnchecked => &[(
            "unreachable",
            "control flow cannot reach this path before `unreachable_unchecked`",
        )],
        OperationFamily::StrFromUtf8Unchecked => &[("utf8", "bytes are valid UTF-8")],
        _ => return None,
    };
    Some(from_specs(specs))
}
