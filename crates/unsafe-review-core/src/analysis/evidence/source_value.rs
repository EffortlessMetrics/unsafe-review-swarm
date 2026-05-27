use super::is_simple_identifier;

pub(super) fn source_value_identifier(argument: &str) -> Option<&str> {
    if is_simple_identifier(argument) {
        return Some(argument);
    }
    let referenced = argument.strip_prefix('&')?;
    is_simple_identifier(referenced).then_some(referenced)
}
