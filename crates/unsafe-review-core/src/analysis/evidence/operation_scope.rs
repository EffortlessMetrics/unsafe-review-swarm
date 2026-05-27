use super::compact_code;

pub(super) fn code_before_operation(lower: &str, expression: &str) -> Option<String> {
    let compact = compact_code(lower);
    let expression = compact_code(&expression.to_ascii_lowercase());
    if expression.is_empty() {
        return None;
    }
    compact
        .find(&expression)
        .map(|operation_pos| compact[..operation_pos].to_string())
}
