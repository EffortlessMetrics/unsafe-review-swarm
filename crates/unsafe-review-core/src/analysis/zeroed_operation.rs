use crate::domain::OperationFamily;

pub(super) fn zeroed_operation_family(line: &str) -> Option<OperationFamily> {
    contains_call_name(line, "zeroed").then_some(OperationFamily::Zeroed)
}

fn contains_call_name(line: &str, name: &str) -> bool {
    let mut cursor = line;
    while let Some(pos) = cursor.find(name) {
        let prefix = &cursor[..pos];
        let before = prefix.chars().next_back();
        let after = &cursor[pos + name.len()..];
        let starts_on_boundary = before.is_none_or(|ch| !is_ident_continue(ch));
        if starts_on_boundary && call_suffix(after) && !preceded_by_fn_keyword(prefix) {
            return true;
        }
        cursor = &after[after
            .char_indices()
            .next()
            .map_or(after.len(), |(idx, ch)| idx + ch.len_utf8())..];
    }
    false
}

/// Returns `true` when the text before the token ends with the keyword `fn`
/// (possibly with intervening whitespace), indicating this is a function
/// definition header rather than a call site.  The guard fires only when `fn`
/// is a standalone keyword — i.e. not preceded by an identifier-continuation
/// character such as `_` or a letter.
fn preceded_by_fn_keyword(prefix: &str) -> bool {
    let trimmed = prefix.trim_end();
    if let Some(rest) = trimmed.strip_suffix("fn") {
        // Confirm `fn` is a standalone keyword: the character before it must
        // not be an identifier-continuation character.
        rest.chars()
            .next_back()
            .is_none_or(|ch| !is_ident_continue(ch))
    } else {
        false
    }
}

fn call_suffix(after_name: &str) -> bool {
    let rest = after_name.trim_start();
    if rest.starts_with('(') {
        return true;
    }
    rest.strip_prefix("::")
        .is_some_and(|after_colons| after_colons.trim_start().starts_with('<'))
}

fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_zeroed_calls() {
        assert_eq!(
            zeroed_operation_family("unsafe { core::mem::zeroed::<u32>() }"),
            Some(OperationFamily::Zeroed)
        );
    }

    #[test]
    fn ignores_identifier_suffix_matches() {
        assert_eq!(
            zeroed_operation_family("unsafe { core::mem::not_zeroed::<u32>() }"),
            None
        );
    }

    #[test]
    fn rejects_zeroed_definition_header() {
        // A safe `fn zeroed(...)` definition must not card — `fn` keyword
        // directly precedes the token, so this is a definition, not a call.
        assert_eq!(
            zeroed_operation_family("pub fn zeroed(len: usize) -> Vec<u8> {"),
            None
        );
        // Indented variant.
        assert_eq!(
            zeroed_operation_family("    fn zeroed<T>(val: T) -> T {"),
            None
        );
        // `pub unsafe fn zeroed()` is still a definition header.
        assert_eq!(
            zeroed_operation_family("pub unsafe fn zeroed() -> u8 {"),
            None
        );
        // A plain call must still card.
        assert_eq!(
            zeroed_operation_family("    let x = mem::zeroed();"),
            Some(OperationFamily::Zeroed)
        );
        // `notfn` is not the `fn` keyword — call must still card.
        assert_eq!(
            zeroed_operation_family("    let notfn = zeroed();"),
            Some(OperationFamily::Zeroed)
        );
    }
}
