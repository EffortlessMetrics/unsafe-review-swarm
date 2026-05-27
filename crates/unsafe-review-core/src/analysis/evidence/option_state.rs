pub(super) fn match_some_branch_after_marker(after_match: &str) -> Option<&str> {
    let some_pos = after_match.find("some(")?;
    let after_some = &after_match[some_pos + "some(".len()..];
    let (binding, after_binding) = after_some.split_once(")=>{")?;
    is_some_binding(binding).then_some(after_binding)
}

pub(super) fn ends_with_some_pattern(before_marker: &str, keyword: &str) -> bool {
    let prefix = format!("{keyword}some(");
    let Some(pattern_start) = before_marker.rfind(&prefix) else {
        return false;
    };
    let binding_with_close = &before_marker[pattern_start + prefix.len()..];
    let Some(binding) = binding_with_close.strip_suffix(')') else {
        return false;
    };
    is_some_binding(binding)
}

pub(super) fn is_some_binding(binding: &str) -> bool {
    !binding.is_empty()
        && (binding == "_"
            || binding
                .chars()
                .all(|ch| ch == '_' || ch.is_ascii_alphanumeric()))
}
