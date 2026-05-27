use super::is_receiver_path_char;

pub(super) fn branch_still_open_at_operation(after_guard: &str) -> bool {
    let mut depth = 1usize;
    for ch in after_guard.chars() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

pub(super) fn matching_code_block_end(text_after_open: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (idx, ch) in text_after_open.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' if depth == 0 => return Some(idx),
            '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    None
}

pub(super) struct CompactIfGuard<'a> {
    pub(super) condition: &'a str,
    pub(super) after_body_start: &'a str,
}

pub(super) fn compact_if_guards(compact: &str) -> impl Iterator<Item = CompactIfGuard<'_>> {
    let mut guards = Vec::new();
    let mut cursor = compact;
    let mut offset = 0usize;
    while let Some(pos) = cursor.find("if") {
        let start = offset + pos;
        let before = compact[..start].chars().next_back();
        if before.is_some_and(is_receiver_path_char) {
            let next = pos + 2;
            offset += next;
            cursor = &cursor[next..];
            continue;
        }
        let after_if = &compact[start + 2..];
        if let Some(brace_pos) = after_if.find('{') {
            guards.push(CompactIfGuard {
                condition: &after_if[..brace_pos],
                after_body_start: &after_if[brace_pos + 1..],
            });
        }
        let next = pos + 2;
        offset += next;
        cursor = &cursor[next..];
    }
    guards.into_iter()
}
