pub(super) fn any_marker_occurrence(
    text: &str,
    marker: &str,
    mut applies: impl FnMut(usize, &str) -> bool,
) -> bool {
    let mut cursor = text;
    let mut offset = 0usize;
    while let Some(pos) = cursor.find(marker) {
        let marker_start = offset + pos;
        let after_marker = &text[marker_start + marker.len()..];
        if applies(marker_start, after_marker) {
            return true;
        }
        let next = pos + marker.len();
        offset += next;
        cursor = &cursor[next..];
    }
    false
}

pub(super) fn any_marker_tail(
    text: &str,
    marker: &str,
    mut applies: impl FnMut(&str) -> bool,
) -> bool {
    any_marker_occurrence(text, marker, |_marker_start, after_marker| {
        applies(after_marker)
    })
}
