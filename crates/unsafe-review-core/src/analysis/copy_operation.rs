use crate::domain::OperationFamily;

pub(super) fn copy_operation_family(line: &str) -> Option<OperationFamily> {
    if contains_call_name(line, "copy_nonoverlapping") {
        return Some(OperationFamily::CopyNonOverlapping);
    }
    if is_ptr_copy_call(line) {
        return Some(OperationFamily::PtrCopy);
    }
    if is_ptr_replace_call(line) {
        return Some(OperationFamily::PtrReplace);
    }
    None
}

fn is_ptr_copy_call(line: &str) -> bool {
    let compact = compact_whitespace(line);
    !compact.contains("copy_nonoverlapping")
        && (compact.contains("ptr::copy(")
            || compact.contains("core::ptr::copy(")
            || compact.contains("std::ptr::copy("))
}

fn is_ptr_replace_call(line: &str) -> bool {
    let compact = compact_whitespace(line);
    compact.contains("ptr::replace(")
        || compact.contains("core::ptr::replace(")
        || compact.contains("std::ptr::replace(")
}

fn contains_call_name(line: &str, name: &str) -> bool {
    let mut cursor = line;
    while let Some(pos) = cursor.find(name) {
        let before = cursor[..pos].chars().next_back();
        let after = &cursor[pos + name.len()..];
        let starts_on_boundary = before.is_none_or(|ch| !is_ident_continue(ch));
        if starts_on_boundary && call_suffix(after) {
            return true;
        }
        cursor = &after[after
            .char_indices()
            .next()
            .map_or(after.len(), |(idx, ch)| idx + ch.len_utf8())..];
    }
    false
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

fn compact_whitespace(text: &str) -> String {
    text.chars().filter(|ch| !ch.is_whitespace()).collect()
}
