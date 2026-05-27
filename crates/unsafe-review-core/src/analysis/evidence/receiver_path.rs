pub(super) fn contains_receiver_fragment(compact: &str, fragment: &str) -> bool {
    let mut cursor = compact;
    let mut offset = 0usize;
    while let Some(pos) = cursor.find(fragment) {
        let start = offset + pos;
        let before = compact[..start].chars().next_back();
        if before.is_none_or(|ch| !is_receiver_path_char(ch)) {
            return true;
        }
        let next = pos + fragment.len();
        offset += next;
        cursor = &cursor[next..];
    }
    false
}

pub(super) fn contains_receiver_path(compact: &str, receiver: &str) -> bool {
    let mut cursor = compact;
    let mut offset = 0usize;
    while let Some(pos) = cursor.find(receiver) {
        let start = offset + pos;
        let end = start + receiver.len();
        let before = compact[..start].chars().next_back();
        let after = compact[end..].chars().next();
        if before.is_none_or(|ch| !is_receiver_path_char(ch))
            && after.is_none_or(|ch| ch == '.' || !is_receiver_path_char(ch))
        {
            return true;
        }
        let next = pos + receiver.len();
        offset += next;
        cursor = &cursor[next..];
    }
    false
}

pub(super) fn receiver_before_marker<'a>(compact: &'a str, marker: &str) -> Option<&'a str> {
    let pos = compact.find(marker)?;
    let before_marker = &compact[..pos];
    let receiver_start = before_marker
        .char_indices()
        .rev()
        .find_map(|(idx, ch)| (!is_receiver_path_char(ch)).then_some(idx + ch.len_utf8()))
        .unwrap_or(0);
    let receiver = &before_marker[receiver_start..];
    (!receiver.is_empty()).then_some(receiver)
}

pub(super) fn is_receiver_path_char(ch: char) -> bool {
    ch == '_' || ch == ':' || ch == '.' || ch.is_ascii_alphanumeric()
}
