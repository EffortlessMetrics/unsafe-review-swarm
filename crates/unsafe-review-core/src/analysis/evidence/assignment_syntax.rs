use super::{is_receiver_path_char, is_simple_identifier};

pub(super) fn contains_simple_assignment_to(compact: &str, name: &str) -> bool {
    if !is_simple_identifier(name) {
        return false;
    }
    if compact.contains(&format!("let{name}="))
        || compact.contains(&format!("letmut{name}="))
        || compact.contains(&format!("let{name}:"))
        || compact.contains(&format!("letmut{name}:"))
    {
        return true;
    }
    let marker = format!("{name}=");
    let mut cursor = compact;
    let mut offset = 0usize;
    while let Some(pos) = cursor.find(&marker) {
        let start = offset + pos;
        let before = compact[..start].chars().next_back();
        let after_equals = compact[start + marker.len()..].chars().next();
        if before.is_none_or(|ch| !is_receiver_path_char(ch)) && after_equals != Some('=') {
            return true;
        }
        let next = pos + marker.len();
        offset += next;
        cursor = &cursor[next..];
    }
    false
}
