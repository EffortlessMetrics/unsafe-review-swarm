pub(super) fn is_atomic_pointer_state_transition(line: &str) -> bool {
    (contains_call_name(line, "swap")
        && line.contains("ptr::null_mut")
        && line.contains("Ordering::"))
        || is_atomic_pointer_fetch_state_transition(line)
}

fn is_atomic_pointer_fetch_state_transition(line: &str) -> bool {
    let compact = compact_whitespace(line);
    compact.contains("from_ptr(")
        && ["fetch_and", "fetch_or", "fetch_xor"]
            .iter()
            .any(|name| contains_call_name(line, name))
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
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
