/// Returns the byte span `(start, end)` of the first argument list of the first
/// `call_name(...)` call found on `line` (not counting the outer parens themselves).
/// Used to bind co-occurrence checks to the same call expression.
fn call_args_span(line: &str, call_name: &str) -> Option<(usize, usize)> {
    let mut cursor = line;
    let mut base = 0usize;
    while let Some(pos) = cursor.find(call_name) {
        let name_end = pos + call_name.len();
        let before = cursor[..pos].chars().next_back();
        let starts_on_boundary = before.is_none_or(|ch| !is_ident_continue(ch));
        let after_name = &cursor[name_end..];
        let trimmed = after_name.trim_start();
        let trim_offset = after_name.len() - trimmed.len();
        if starts_on_boundary && trimmed.starts_with('(') {
            // found the open paren; now find its matching close paren
            let args_start = base + name_end + trim_offset + 1; // byte after '('
            let after_open = &trimmed[1..];
            if let Some(close) = matching_close_paren(after_open) {
                let args_end = args_start + close;
                return Some((args_start, args_end));
            }
        }
        // advance past this occurrence and keep scanning
        let advance = name_end;
        base += advance;
        cursor = &cursor[advance..];
    }
    None
}

/// Finds the offset of the `)` that closes the open paren that was just consumed.
/// `text` is the text *after* the `(` has already been consumed.
fn matching_close_paren(text: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (idx, ch) in text.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' if depth == 0 => return Some(idx),
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    None
}

pub(super) fn is_atomic_pointer_state_transition(line: &str) -> bool {
    is_atomic_swap_null_transition(line) || is_atomic_pointer_fetch_state_transition(line)
}

/// Checks that the line contains `.swap(ptr::null_mut(), Ordering::…)` where
/// `ptr::null_mut` and `Ordering::` both appear *inside* the argument list of
/// that `swap` call — not merely anywhere on the line.
fn is_atomic_swap_null_transition(line: &str) -> bool {
    if !contains_call_name(line, "swap") {
        return false;
    }
    let Some((args_start, args_end)) = call_args_span(line, "swap") else {
        return false;
    };
    let args = &line[args_start..args_end];
    args.contains("ptr::null_mut") && args.contains("Ordering::")
}

fn is_atomic_pointer_fetch_state_transition(line: &str) -> bool {
    let compact = compact_whitespace(line);
    if !compact.contains("from_ptr(") {
        return false;
    }
    // Require the fetch_and/fetch_or/fetch_xor call to be nested *inside*
    // the argument list of the `from_ptr(...)` call, not just anywhere on the line.
    let Some((args_start, args_end)) = call_args_span(line, "from_ptr") else {
        return false;
    };
    let args = &line[args_start..args_end];
    ["fetch_and", "fetch_or", "fetch_xor"]
        .iter()
        .any(|name| contains_call_name(args, name))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swap_null_mut_in_args_is_detected() {
        assert!(is_atomic_pointer_state_transition(
            "self.head.swap(ptr::null_mut(), Ordering::AcqRel)"
        ));
    }

    #[test]
    fn swap_null_mut_co_occurrence_on_line_is_not_detected() {
        // swap call exists, ptr::null_mut exists, Ordering:: exists —
        // but ptr::null_mut and Ordering:: are NOT in swap's arg list.
        assert!(!is_atomic_pointer_state_transition(
            "let _x = ptr::null_mut::<u8>(); let _o = Ordering::Relaxed; other.swap(val, Ordering::SeqCst)"
        ));
    }

    #[test]
    fn swap_without_null_mut_in_args_is_not_detected() {
        // swap with Ordering but no null_mut inside the parens
        assert!(!is_atomic_pointer_state_transition(
            "self.head.swap(other_ptr, Ordering::AcqRel)"
        ));
    }

    #[test]
    fn fetch_and_nested_in_from_ptr_is_detected() {
        assert!(is_atomic_pointer_state_transition(
            "unsafe { Shared::from_ptr(self.data.fetch_and(mask, order)) }"
        ));
    }

    #[test]
    fn fetch_or_nested_in_from_ptr_is_detected() {
        assert!(is_atomic_pointer_state_transition(
            "unsafe { Shared::from_ptr(self.data.fetch_or(mask, order)) }"
        ));
    }

    #[test]
    fn fetch_xor_nested_in_from_ptr_is_detected() {
        assert!(is_atomic_pointer_state_transition(
            "unsafe { Shared::from_ptr(self.data.fetch_xor(mask, order) as *mut T) }"
        ));
    }

    #[test]
    fn from_ptr_and_fetch_or_co_occurrence_on_line_is_not_detected() {
        // from_ptr exists and fetch_or exists, but fetch_or is NOT inside from_ptr's args
        assert!(!is_atomic_pointer_state_transition(
            "let p = SomeType::from_ptr(raw); let _ = atomic.fetch_or(mask, Ordering::Relaxed);"
        ));
    }

    #[test]
    fn call_args_span_locates_swap_args() -> Result<(), String> {
        let line = "self.head.swap(ptr::null_mut(), Ordering::AcqRel)";
        let (s, e) = call_args_span(line, "swap")
            .ok_or_else(|| "expected call_args_span to find swap args".to_string())?;
        let args = &line[s..e];
        assert!(args.contains("ptr::null_mut"));
        assert!(args.contains("Ordering::AcqRel"));
        Ok(())
    }

    #[test]
    fn call_args_span_does_not_cross_call_boundary() -> Result<(), String> {
        // swap args do NOT include tokens that appear only outside the parens
        let line = "let _n = ptr::null_mut::<u8>(); self.head.swap(other, Ordering::SeqCst); let _o = Ordering::Relaxed;";
        let (s, e) = call_args_span(line, "swap")
            .ok_or_else(|| "expected call_args_span to find swap".to_string())?;
        let args = &line[s..e];
        // ptr::null_mut is outside the parens here
        assert!(!args.contains("null_mut"));
        Ok(())
    }
}
