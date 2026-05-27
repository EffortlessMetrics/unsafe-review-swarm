use super::{
    branch_still_open_at_operation, compact_code, contains_simple_assignment_to, let_binding_name,
    matching_call_argument_end, matching_code_block_end, receiver_before_marker,
    split_top_level_arguments, strip_block_comments_and_literals,
};

pub(super) fn has_vec_from_raw_parts_capacity_evidence(expression: &str, lower: &str) -> bool {
    let compact = compact_code(&strip_block_comments_and_literals(lower));
    let compact_expression = compact_code(&expression.to_ascii_lowercase());
    let Some((_ptr, len, cap)) = vec_from_raw_parts_arguments(&compact_expression) else {
        return false;
    };
    let call_pos = compact
        .find(&compact_expression)
        .or_else(|| compact.find("vec::from_raw_parts("));
    let Some(call_pos) = call_pos else {
        return false;
    };
    let before_call = &compact[..call_pos];
    has_len_cap_bound_guard(before_call, len, cap)
}

pub(super) fn has_vec_from_raw_parts_origin_len_cap_evidence(
    expression: &str,
    lower: &str,
) -> bool {
    let compact = compact_code(lower);
    let compact_expression = compact_code(&expression.to_ascii_lowercase());
    let Some((_ptr, len, cap)) = vec_from_raw_parts_arguments(&compact_expression) else {
        return false;
    };
    let call_pos = compact
        .find(&compact_expression)
        .or_else(|| compact.find("vec::from_raw_parts("));
    let Some(call_pos) = call_pos else {
        return false;
    };
    has_vec_from_raw_parts_same_origin_len_cap(&compact[..call_pos], len, cap)
}

pub(super) fn has_vec_from_raw_parts_origin_initialized_evidence(
    expression: &str,
    lower: &str,
) -> bool {
    let compact = compact_code(lower);
    let compact_expression = compact_code(&expression.to_ascii_lowercase());
    let Some((ptr, len, _cap)) = vec_from_raw_parts_arguments(&compact_expression) else {
        return false;
    };
    let call_pos = compact
        .find(&compact_expression)
        .or_else(|| compact.find("vec::from_raw_parts("));
    let Some(call_pos) = call_pos else {
        return false;
    };
    let before_call = &compact[..call_pos];
    let Some(ptr_receiver) = vec_raw_parts_pointer_origin_receiver_before(before_call, ptr) else {
        return false;
    };
    vec_raw_parts_len_origin_receiver(before_call, len)
        .is_some_and(|receiver| receiver == ptr_receiver)
}

pub(super) fn has_vec_from_raw_parts_origin_pointer_live_evidence(
    expression: &str,
    lower: &str,
) -> bool {
    let compact = compact_code(lower);
    let compact_expression = compact_code(&expression.to_ascii_lowercase());
    let Some((ptr, _len, cap)) = vec_from_raw_parts_arguments(&compact_expression) else {
        return false;
    };
    let call_pos = compact
        .find(&compact_expression)
        .or_else(|| compact.find("vec::from_raw_parts("));
    let Some(call_pos) = call_pos else {
        return false;
    };
    let before_call = &compact[..call_pos];
    let Some(ptr_receiver) = vec_raw_parts_pointer_origin_receiver_before(before_call, ptr) else {
        return false;
    };
    vec_raw_parts_capacity_origin_receiver(before_call, cap)
        .is_some_and(|receiver| receiver == ptr_receiver)
}

pub(super) fn has_vec_from_raw_parts_origin_evidence(expression: &str, lower: &str) -> bool {
    let compact = compact_code(lower);
    let compact_expression = compact_code(&expression.to_ascii_lowercase());
    let Some((ptr, _len, _cap)) = vec_from_raw_parts_arguments(&compact_expression) else {
        return false;
    };
    let call_pos = compact
        .find(&compact_expression)
        .or_else(|| compact.find("vec::from_raw_parts("));
    let Some(call_pos) = call_pos else {
        return false;
    };
    has_same_pointer_vec_raw_parts_origin_before(&compact[..call_pos], ptr)
}

fn vec_from_raw_parts_arguments(compact_expression: &str) -> Option<(&str, &str, &str)> {
    let marker = "from_raw_parts(";
    let call_pos = compact_expression.find(marker)?;
    let after_marker = &compact_expression[call_pos + marker.len()..];
    let end = matching_call_argument_end(after_marker)?;
    let args = split_top_level_arguments(&after_marker[..end]);
    if args.len() == 3 && args.iter().all(|arg| !arg.is_empty()) {
        Some((args[0], args[1], args[2]))
    } else {
        None
    }
}

fn has_len_cap_bound_guard(before_call: &str, len: &str, cap: &str) -> bool {
    let len = compact_code(len);
    let cap = compact_code(cap);
    if len.is_empty() || cap.is_empty() {
        return false;
    }
    let len_lte_cap = format!("{len}<={cap}");
    let cap_gte_len = format!("{cap}>={len}");
    let len_gt_cap = format!("{len}>{cap}");
    let cap_lt_len = format!("{cap}<{len}");
    has_len_cap_bound_predicate(before_call, &len_lte_cap, &len, &cap)
        || has_len_cap_bound_predicate(before_call, &cap_gte_len, &len, &cap)
        || has_len_cap_early_return(before_call, &len_gt_cap, &len, &cap)
        || has_len_cap_early_return(before_call, &cap_lt_len, &len, &cap)
}

fn has_len_cap_bound_predicate(before_call: &str, predicate: &str, len: &str, cap: &str) -> bool {
    [
        format!("assert!({predicate})"),
        format!("assert!({predicate},"),
        format!("debug_assert!({predicate})"),
        format!("debug_assert!({predicate},"),
    ]
    .iter()
    .any(|pattern| has_fresh_len_cap_guard_pattern(before_call, pattern, len, cap))
        || has_open_len_cap_branch_guard(before_call, predicate, len, cap)
}

fn has_fresh_len_cap_guard_pattern(before_call: &str, pattern: &str, len: &str, cap: &str) -> bool {
    let mut search_from = 0;
    while let Some(offset) = before_call[search_from..].find(pattern) {
        let pattern_start = search_from + offset;
        let after_pattern = &before_call[pattern_start + pattern.len()..];
        let statement_end = after_pattern.find(';').unwrap_or(after_pattern.len());
        let after_guard = &after_pattern[statement_end..];
        if !has_len_cap_assignment(after_guard, len, cap) {
            return true;
        }
        search_from = pattern_start + pattern.len();
    }
    false
}

fn has_open_len_cap_branch_guard(before_call: &str, predicate: &str, len: &str, cap: &str) -> bool {
    let guard = format!("if{predicate}{{");
    let mut search_from = 0;
    while let Some(offset) = before_call[search_from..].find(&guard) {
        let guard_start = search_from + offset;
        let after_guard = &before_call[guard_start + guard.len()..];
        if branch_still_open_at_operation(after_guard)
            && !has_len_cap_assignment(after_guard, len, cap)
        {
            return true;
        }
        search_from = guard_start + guard.len();
    }
    false
}

fn has_len_cap_early_return(before_call: &str, predicate: &str, len: &str, cap: &str) -> bool {
    let guard = format!("if{predicate}{{");
    let mut search_from = 0;
    while let Some(offset) = before_call[search_from..].find(&guard) {
        let guard_start = search_from + offset;
        let after_guard = &before_call[guard_start + guard.len()..];
        let (guard_body, after_guard_body) = matching_code_block_end(after_guard)
            .map_or((after_guard, ""), |body_end| {
                (&after_guard[..body_end], &after_guard[body_end + 1..])
            });
        if guard_body.contains("return") && !has_len_cap_assignment(after_guard_body, len, cap) {
            return true;
        }
        search_from = guard_start + guard.len();
    }
    false
}

fn has_len_cap_assignment(compact: &str, len: &str, cap: &str) -> bool {
    contains_simple_assignment_to(compact, len) || contains_simple_assignment_to(compact, cap)
}

fn has_vec_from_raw_parts_same_origin_len_cap(before_call: &str, len: &str, cap: &str) -> bool {
    vec_raw_parts_len_origin_receiver(before_call, len).is_some_and(|receiver| {
        vec_raw_parts_capacity_origin_receiver(before_call, cap) == Some(receiver)
    })
}

fn has_same_pointer_vec_raw_parts_origin_before(before_call: &str, pointer: &str) -> bool {
    vec_raw_parts_pointer_origin_receiver_before(before_call, pointer).is_some()
}

fn vec_raw_parts_pointer_origin_receiver_before(
    before_call: &str,
    pointer: &str,
) -> Option<String> {
    let mut prior_statements = String::new();
    for statement in before_call.split(';') {
        let Some((left, right)) = statement.split_once('=') else {
            prior_statements.push_str(statement);
            prior_statements.push(';');
            continue;
        };
        let Some(binding) = let_binding_name(left) else {
            prior_statements.push_str(statement);
            prior_statements.push(';');
            continue;
        };
        if binding == pointer
            && let Some(receiver) = vec_raw_pointer_receiver(right)
            && vec_raw_pointer_receiver_has_manually_drop_origin(&prior_statements, receiver)
        {
            return Some(receiver.to_string());
        }
        prior_statements.push_str(statement);
        prior_statements.push(';');
    }
    None
}

fn vec_raw_pointer_receiver(right_side: &str) -> Option<&str> {
    receiver_before_marker(right_side, ".as_mut_ptr(")
        .or_else(|| receiver_before_marker(right_side, ".as_ptr("))
}

fn vec_raw_pointer_receiver_has_manually_drop_origin(before_call: &str, receiver: &str) -> bool {
    before_call.split(';').any(|statement| {
        let Some((left, right)) = statement.split_once('=') else {
            return false;
        };
        let Some(binding) = let_binding_name(left) else {
            return false;
        };
        binding == receiver && right.contains("manuallydrop::new(")
    })
}

fn vec_raw_parts_len_origin_receiver(before_call: &str, len: &str) -> Option<String> {
    let len = compact_code(len);
    if len.is_empty() {
        return None;
    }

    let mut origin_receivers = Vec::new();
    for statement in before_call.split(';') {
        let Some((left, right)) = statement.split_once('=') else {
            continue;
        };
        let Some(binding) = let_binding_name(left) else {
            continue;
        };
        if right.contains("manuallydrop::new(") {
            origin_receivers.push(binding.to_string());
        }
        if binding == len
            && let Some(receiver) = receiver_before_marker(right, ".len(")
            && origin_receivers.iter().any(|origin| origin == receiver)
        {
            return Some(receiver.to_string());
        }
    }
    None
}

fn vec_raw_parts_capacity_origin_receiver(before_call: &str, cap: &str) -> Option<String> {
    let cap = compact_code(cap);
    if cap.is_empty() {
        return None;
    }

    let mut origin_receivers = Vec::new();
    for statement in before_call.split(';') {
        let Some((left, right)) = statement.split_once('=') else {
            continue;
        };
        let Some(binding) = let_binding_name(left) else {
            continue;
        };
        if right.contains("manuallydrop::new(") {
            origin_receivers.push(binding.to_string());
        }
        if binding == cap
            && let Some(receiver) = receiver_before_marker(right, ".capacity(")
            && origin_receivers.iter().any(|origin| origin == receiver)
        {
            return Some(receiver.to_string());
        }
    }
    None
}
