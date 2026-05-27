pub(super) fn let_binding_name(left_side: &str) -> Option<&str> {
    let let_pos = left_side.rfind("let")?;
    let rest = &left_side[let_pos + "let".len()..];
    let rest = rest.strip_prefix("mut").unwrap_or(rest);
    let end = rest
        .char_indices()
        .find_map(|(idx, ch)| (!(ch == '_' || ch.is_ascii_alphanumeric())).then_some(idx))
        .unwrap_or(rest.len());
    (end > 0).then_some(&rest[..end])
}

pub(super) fn is_simple_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}
