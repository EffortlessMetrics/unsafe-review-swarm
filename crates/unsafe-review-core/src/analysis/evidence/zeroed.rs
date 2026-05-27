use super::{compact_code, matching_generic_argument_end};

pub(super) fn has_zeroed_known_valid_zero_type(lower: &str) -> bool {
    let compact = compact_code(lower);
    let Some(target_type) = zeroed_target_type(&compact) else {
        return false;
    };
    matches!(
        target_type,
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

fn zeroed_target_type(compact: &str) -> Option<&str> {
    let marker = "zeroed::<";
    let start = compact.find(marker)? + marker.len();
    let after_marker = &compact[start..];
    let end = matching_generic_argument_end(after_marker)?;
    let target_type = &after_marker[..end];
    (!target_type.is_empty()).then_some(target_type)
}
