use super::{
    branch_still_open_at_operation, compact_code, compact_if_guards, contains_simple_assignment_to,
    has_length_or_bounds_guard, is_simple_identifier, let_binding_name, matching_call_argument_end,
    receiver_before_marker,
};

pub(super) fn has_raw_pointer_read_bounds_evidence(
    expression: &str,
    before_operation: &str,
) -> bool {
    let compact_expression = compact_code(&expression.to_ascii_lowercase());
    let Some(pointer) = raw_pointer_read_pointer_receiver(&compact_expression) else {
        return has_length_or_bounds_guard(before_operation);
    };
    let before_operation = compact_code(before_operation);
    let Some(origin) = pointer_origin_receiver_before(&before_operation, pointer) else {
        return false;
    };

    has_origin_len_size_guard(&before_operation, &origin)
        || has_origin_len_capacity_equality_guard(&before_operation, &origin)
}

fn raw_pointer_read_pointer_receiver(compact_expression: &str) -> Option<&str> {
    if let Some(receiver) = receiver_before_marker(compact_expression, ".cast::<") {
        return Some(receiver);
    }
    if let Some(receiver) = receiver_before_marker(compact_expression, ".read(") {
        return Some(receiver);
    }
    if let Some(receiver) = receiver_before_marker(compact_expression, ".read_volatile(") {
        return Some(receiver);
    }
    raw_pointer_read_function_argument(compact_expression)
}

fn raw_pointer_read_function_argument(compact_expression: &str) -> Option<&str> {
    let marker = "ptr::read(";
    let call_pos = compact_expression.find(marker)? + marker.len();
    let after_marker = &compact_expression[call_pos..];
    let argument_end = matching_call_argument_end(after_marker)?;
    let argument = after_marker[..argument_end]
        .split_once("as*")
        .map_or(&after_marker[..argument_end], |(argument, _)| argument)
        .trim();
    (!argument.is_empty()).then_some(argument)
}

fn pointer_origin_receiver_before(before_operation: &str, pointer: &str) -> Option<String> {
    if pointer.contains(".as_ptr()") || pointer.contains(".as_mut_ptr()") {
        return pointer_origin_receiver(pointer).map(str::to_string);
    }
    let mut current_origin = None;
    for statement in before_operation.split(';') {
        let Some((left, right)) = statement.rsplit_once('=') else {
            continue;
        };
        let Some(binding) = assignment_binding_name(left) else {
            continue;
        };
        if binding != pointer {
            continue;
        }
        current_origin = pointer_origin_receiver(right).map(str::to_string);
    }
    current_origin
}

fn pointer_origin_receiver(expression: &str) -> Option<&str> {
    let expression = pointer_expression_before_type_change(expression);
    expression
        .strip_suffix(".as_ptr()")
        .or_else(|| expression.strip_suffix(".as_mut_ptr()"))
        .filter(|receiver| !receiver.is_empty())
}

fn pointer_expression_before_type_change(expression: &str) -> &str {
    expression
        .find(".cast::<")
        .or_else(|| expression.find(".cast()"))
        .or_else(|| expression.find("as*const"))
        .or_else(|| expression.find("as*mut"))
        .map_or(expression, |cast_pos| &expression[..cast_pos])
}

fn assignment_binding_name(left_side: &str) -> Option<&str> {
    if let Some(binding) = let_binding_name(left_side) {
        return Some(binding);
    }
    is_simple_identifier(left_side).then_some(left_side)
}

fn has_origin_len_size_guard(compact: &str, origin: &str) -> bool {
    let len = format!("{origin}.len()");
    has_origin_len_size_assertion_guard(compact, &len, origin)
        || has_origin_len_size_open_positive_branch_guard(compact, &len, origin)
        || has_origin_len_size_early_return_guard(compact, &len, origin)
}

fn has_origin_len_size_assertion_guard(compact: &str, len: &str, origin: &str) -> bool {
    ["assert!(", "debug_assert!("].into_iter().any(|prefix| {
        let mut cursor = compact;
        let mut offset = 0usize;
        while let Some(pos) = cursor.find(prefix) {
            let statement_start = offset + pos + prefix.len();
            let after_prefix = &compact[statement_start..];
            let statement_end = after_prefix.find(';').unwrap_or(after_prefix.len());
            let statement = &after_prefix[..statement_end];
            let after_statement = &after_prefix[statement_end..];
            if origin_len_size_condition_is_positive(statement, len)
                && !contains_simple_assignment_to(after_statement, origin)
            {
                return true;
            }
            let next = pos + prefix.len();
            offset += next;
            cursor = &cursor[next..];
        }
        false
    })
}

fn has_origin_len_size_open_positive_branch_guard(compact: &str, len: &str, origin: &str) -> bool {
    compact_if_guards(compact).any(|guard| {
        origin_len_size_condition_is_positive(guard.condition, len)
            && branch_still_open_at_operation(guard.after_body_start)
            && !contains_simple_assignment_to(guard.after_body_start, origin)
    })
}

fn has_origin_len_size_early_return_guard(compact: &str, len: &str, origin: &str) -> bool {
    compact_if_guards(compact).any(|guard| {
        if !origin_len_size_condition_is_negative(guard.condition, len) {
            return false;
        }
        let (guard_body, after_guard_body) = guard
            .after_body_start
            .split_once('}')
            .map_or((guard.after_body_start, ""), |(guard_body, after)| {
                (guard_body, after)
            });
        guard_body.contains("return") && !contains_simple_assignment_to(after_guard_body, origin)
    })
}

fn origin_len_size_condition_is_positive(condition: &str, len: &str) -> bool {
    condition.contains("size_of")
        && (condition.contains(&format!("{len}>"))
            || condition.contains(&format!("<{len}"))
            || condition.contains(&format!("<={len}")))
}

fn origin_len_size_condition_is_negative(condition: &str, len: &str) -> bool {
    condition.contains("size_of")
        && (condition.contains(&format!("{len}<"))
            || condition.contains(&format!(">{len}"))
            || condition.contains(&format!(">={len}")))
}

fn has_origin_len_capacity_equality_guard(compact: &str, origin: &str) -> bool {
    let len = format!("{origin}.len()");
    let capacity = format!("{origin}.capacity()");
    let cap = format!("{origin}.cap()");
    has_origin_len_capacity_assertion_guard(compact, &len, &capacity, &cap, origin)
        || has_origin_len_capacity_open_positive_branch_guard(
            compact, &len, &capacity, &cap, origin,
        )
}

fn has_origin_len_capacity_assertion_guard(
    compact: &str,
    len: &str,
    capacity: &str,
    cap: &str,
    origin: &str,
) -> bool {
    [
        ("assert_eq!(", false),
        ("debug_assert_eq!(", false),
        ("assert!(", true),
        ("debug_assert!(", true),
    ]
    .into_iter()
    .any(|(prefix, requires_operator)| {
        let mut cursor = compact;
        let mut offset = 0usize;
        while let Some(pos) = cursor.find(prefix) {
            let statement_start = offset + pos + prefix.len();
            let after_prefix = &compact[statement_start..];
            let statement_end = after_prefix.find(';').unwrap_or(after_prefix.len());
            let statement = &after_prefix[..statement_end];
            let after_statement = &after_prefix[statement_end..];
            if origin_len_capacity_condition_matches(statement, len, capacity, cap)
                && (!requires_operator || statement.contains("=="))
                && !contains_simple_assignment_to(after_statement, origin)
            {
                return true;
            }
            let next = pos + prefix.len();
            offset += next;
            cursor = &cursor[next..];
        }
        false
    })
}

fn has_origin_len_capacity_open_positive_branch_guard(
    compact: &str,
    len: &str,
    capacity: &str,
    cap: &str,
    origin: &str,
) -> bool {
    compact_if_guards(compact).any(|guard| {
        origin_len_capacity_condition_matches(guard.condition, len, capacity, cap)
            && guard.condition.contains("==")
            && branch_still_open_at_operation(guard.after_body_start)
            && !contains_simple_assignment_to(guard.after_body_start, origin)
    })
}

fn origin_len_capacity_condition_matches(
    condition: &str,
    len: &str,
    capacity: &str,
    cap: &str,
) -> bool {
    condition.contains(len) && (condition.contains(capacity) || condition.contains(cap))
}
