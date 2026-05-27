use super::{branch_still_open_at_operation, compact_code, is_receiver_path_char};

pub(super) fn has_encode_utf8_remaining_capacity_evidence(lower: &str) -> bool {
    let compact = compact_code(lower);
    compact.contains("encode_utf8(c,ptr,remaining_cap)")
        && compact.contains("remaining_cap=self.capacity()-len")
        && compact.contains("ptr")
}

pub(super) fn has_unchecked_constructor_availability_evidence(
    expression: &str,
    lower: &str,
) -> bool {
    let compact_expression = compact_code(&expression.to_ascii_lowercase());
    let Some(receiver) = unchecked_constructor_receiver(&compact_expression) else {
        return false;
    };
    let compact = compact_code(lower);
    let before_call = compact
        .find(&compact_expression)
        .map_or(compact.as_str(), |call_pos| &compact[..call_pos]);
    has_unchecked_constructor_availability_guard(before_call, receiver)
}

fn has_unchecked_constructor_availability_guard(before_call: &str, receiver: &str) -> bool {
    let predicate = format!("{receiver}::is_available()");
    has_unchecked_constructor_availability_assertion(before_call, &predicate)
        || has_open_unchecked_constructor_availability_branch(before_call, &predicate)
        || has_unchecked_constructor_unavailable_early_return(before_call, &predicate)
}

fn has_unchecked_constructor_availability_assertion(before_call: &str, predicate: &str) -> bool {
    [
        format!("assert!({predicate})"),
        format!("assert!({predicate},"),
        format!("debug_assert!({predicate})"),
        format!("debug_assert!({predicate},"),
    ]
    .iter()
    .any(|pattern| before_call.contains(pattern))
}

fn has_open_unchecked_constructor_availability_branch(before_call: &str, predicate: &str) -> bool {
    let guard = format!("if{predicate}{{");
    let mut search_from = 0;
    while let Some(offset) = before_call[search_from..].find(&guard) {
        let guard_start = search_from + offset;
        let after_guard = &before_call[guard_start + guard.len()..];
        if branch_still_open_at_operation(after_guard) {
            return true;
        }
        search_from = guard_start + guard.len();
    }
    false
}

fn has_unchecked_constructor_unavailable_early_return(before_call: &str, predicate: &str) -> bool {
    let guard = format!("if!{predicate}{{");
    let mut search_from = 0;
    while let Some(offset) = before_call[search_from..].find(&guard) {
        let guard_start = search_from + offset;
        let after_guard = &before_call[guard_start + guard.len()..];
        let guard_body = after_guard
            .split_once('}')
            .map_or(after_guard, |(guard_body, _after)| guard_body);
        if guard_body.contains("return") {
            return true;
        }
        search_from = guard_start + guard.len();
    }
    false
}

fn unchecked_constructor_receiver(compact_expression: &str) -> Option<&str> {
    let call_pos = compact_expression.find("::new_unchecked")?;
    let before_call = &compact_expression[..call_pos];
    let receiver_start = before_call
        .char_indices()
        .rev()
        .find_map(|(idx, ch)| (!is_receiver_path_char(ch)).then_some(idx + ch.len_utf8()))
        .unwrap_or(0);
    let receiver = &before_call[receiver_start..];
    (!receiver.is_empty()).then_some(receiver)
}
