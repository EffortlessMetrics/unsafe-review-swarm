use super::{compact_code, matching_generic_argument_end};

pub(super) fn has_zeroed_known_valid_zero_type(lower: &str) -> bool {
    let compact = compact_code(lower);
    let Some(context) = ZeroedTargetContext::from_compact(&compact) else {
        return false;
    };
    context.has_known_valid_zero_target_type()
}

// Valid-zero evidence is target-type scoped: only known valid-zero targets can
// discharge the zeroed obligation.
struct ZeroedTargetContext<'a> {
    target_type: &'a str,
}

impl<'a> ZeroedTargetContext<'a> {
    fn from_compact(compact: &'a str) -> Option<Self> {
        let marker = "zeroed::<";
        let start = compact.find(marker)? + marker.len();
        let after_marker = &compact[start..];
        let end = matching_generic_argument_end(after_marker)?;
        let target_type = &after_marker[..end];
        (!target_type.is_empty()).then_some(Self { target_type })
    }

    fn has_known_valid_zero_target_type(&self) -> bool {
        matches!(
            self.target_type,
            "()" | "bool"
                | "char"
                | "f32"
                | "f64"
                | "i8"
                | "i16"
                | "i32"
                | "i64"
                | "i128"
                | "isize"
                | "u8"
                | "u16"
                | "u32"
                | "u64"
                | "u128"
                | "usize"
        )
    }
}
