use super::{
    branch_still_open_at_operation, compact_code, compact_if_guards, contains_executable_return,
    contains_simple_assignment_to, is_runtime_assert_at, is_simple_identifier, let_binding_name,
    matching_call_argument_end, matching_code_block_end, receiver_before_marker,
    split_top_level_pair, strip_block_comments_and_literals,
};

pub(super) fn has_raw_pointer_read_bounds_evidence(
    expression: &str,
    before_operation: &str,
) -> bool {
    let compact_expression = compact_code(&expression.to_ascii_lowercase());
    let before_operation = strip_block_comments_and_literals(before_operation);
    // When the read-pointer receiver shape cannot be parsed, do not fall back to the
    // unscoped length guard: a co-located length comparison on an unrelated variable
    // would wrongly discharge the bounds obligation (F3-7).
    let Some(pointer) = raw_pointer_read_pointer_receiver(&compact_expression) else {
        return false;
    };
    let before_operation = compact_code(&before_operation);
    RawPointerReadBoundsApplicability::new(&before_operation, pointer)
        .is_some_and(|context| context.has_same_origin_bounds_evidence())
}

/// Same-origin bounds evidence for a plain `ptr.write(value)` or free-fn write call.
///
/// The bounds obligation is discharged only when the length/size guard provably constrains
/// the SAME slice origin as the write destination — never a co-located unrelated comparison
/// (F3-6 same-destination scoping).
pub(super) fn has_raw_pointer_write_bounds_evidence(
    expression: &str,
    before_operation: &str,
) -> bool {
    let compact_expression = compact_code(&expression.to_ascii_lowercase());
    let before_operation = strip_block_comments_and_literals(before_operation);
    // When the write-destination pointer shape cannot be parsed, do not fall back to
    // the unscoped length guard: a co-located length comparison on an unrelated variable
    // would wrongly discharge the bounds obligation (F3-6).
    let Some(pointer) = raw_pointer_write_destination(&compact_expression) else {
        return false;
    };
    let before_operation = compact_code(&before_operation);
    RawPointerReadBoundsApplicability::new(&before_operation, pointer)
        .is_some_and(|context| context.has_same_origin_bounds_evidence())
}

fn raw_pointer_write_destination(compact_expression: &str) -> Option<&str> {
    if let Some(receiver) = receiver_before_marker(compact_expression, ".write(") {
        return Some(receiver);
    }
    if let Some(receiver) = receiver_before_marker(compact_expression, ".write_volatile(") {
        return Some(receiver);
    }
    raw_pointer_write_function_argument(compact_expression)
}

fn raw_pointer_write_function_argument(compact_expression: &str) -> Option<&str> {
    // Try each free-function write form; more specific markers first so that
    // `ptr::write_unaligned(` does not accidentally match `ptr::write(`.
    // Write forms are two-argument: `ptr::write(destination, value)` — only the
    // first argument (the destination pointer) is relevant for same-origin tracing.
    for marker in &[
        "ptr::write_unaligned(",
        "ptr::write_volatile(",
        "ptr::write(",
    ] {
        if let Some(call_pos) = compact_expression.find(marker) {
            let after_marker = &compact_expression[call_pos + marker.len()..];
            let argument_end = matching_call_argument_end(after_marker)?;
            let all_args = &after_marker[..argument_end];
            // Take only the first argument (before the first top-level comma).
            let first_arg = split_top_level_pair(all_args).map_or(all_args, |(first, _rest)| first);
            let argument = first_arg
                .split_once("as*")
                .map_or(first_arg, |(argument, _)| argument)
                .trim();
            return (!argument.is_empty()).then_some(argument);
        }
    }
    None
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
    // Try each free-function read form; the first match wins.
    for marker in &["ptr::read_unaligned(", "ptr::read_volatile(", "ptr::read("] {
        if let Some(call_pos) = compact_expression.find(marker) {
            let after_marker = &compact_expression[call_pos + marker.len()..];
            let argument_end = matching_call_argument_end(after_marker)?;
            let argument = after_marker[..argument_end]
                .split_once("as*")
                .map_or(&after_marker[..argument_end], |(argument, _)| argument)
                .trim();
            return (!argument.is_empty()).then_some(argument);
        }
    }
    None
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

struct RawPointerReadBoundsApplicability<'a> {
    before_operation: &'a str,
    same_origin_target: String,
}

impl<'a> RawPointerReadBoundsApplicability<'a> {
    fn new(before_operation: &'a str, pointer: &str) -> Option<Self> {
        Some(Self {
            before_operation,
            same_origin_target: pointer_origin_receiver_before(before_operation, pointer)?,
        })
    }

    fn has_same_origin_bounds_evidence(&self) -> bool {
        has_origin_len_size_guard(self.before_operation, &self.same_origin_target)
            || has_origin_len_capacity_equality_guard(
                self.before_operation,
                &self.same_origin_target,
            )
    }
}

fn has_origin_len_size_guard(compact: &str, origin: &str) -> bool {
    let len = format!("{origin}.len()");
    has_origin_len_size_assertion_guard(compact, &len, origin)
        || has_origin_len_size_open_positive_branch_guard(compact, &len, origin)
        || has_origin_len_size_early_return_guard(compact, &len, origin)
}

fn has_origin_len_size_assertion_guard(compact: &str, len: &str, origin: &str) -> bool {
    // Only `assert!` is a release-runtime guard; `debug_assert!` is compiled out in release
    // builds and cannot satisfy a runtime bounds obligation.  `is_runtime_assert_at` ensures
    // that `assert!(` found inside `debug_assert!(` is not credited as a runtime guard.
    let prefix = "assert!(";
    let mut cursor = compact;
    let mut offset = 0usize;
    while let Some(pos) = cursor.find(prefix) {
        let abs_pos = offset + pos;
        let statement_start = abs_pos + prefix.len();
        if is_runtime_assert_at(compact, abs_pos) {
            let after_prefix = &compact[statement_start..];
            let statement_end = after_prefix.find(';').unwrap_or(after_prefix.len());
            let statement = &after_prefix[..statement_end];
            let after_statement = &after_prefix[statement_end..];
            if origin_len_size_condition_is_positive(statement, len)
                && !contains_simple_assignment_to(after_statement, origin)
            {
                return true;
            }
        }
        let next = pos + prefix.len();
        offset += next;
        cursor = &cursor[next..];
    }
    false
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
        let (guard_body, after_guard_body) = matching_code_block_end(guard.after_body_start)
            .map_or((guard.after_body_start, ""), |body_end| {
                (
                    &guard.after_body_start[..body_end],
                    &guard.after_body_start[body_end + 1..],
                )
            });
        contains_executable_return(guard_body)
            && !contains_simple_assignment_to(after_guard_body, origin)
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
    // Only `assert_eq!` and `assert!` are release-runtime guards; `debug_assert*` variants are
    // compiled out in release builds and cannot satisfy a runtime bounds obligation.
    // `is_runtime_assert_at` guards against `assert!(` matching inside `debug_assert!(`.
    [("assert_eq!(", false), ("assert!(", true)]
        .into_iter()
        .any(|(prefix, requires_operator)| {
            let mut cursor = compact;
            let mut offset = 0usize;
            while let Some(pos) = cursor.find(prefix) {
                let abs_pos = offset + pos;
                let statement_start = abs_pos + prefix.len();
                if is_runtime_assert_at(compact, abs_pos) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applicability_uses_same_pointer_origin_bounds() -> Result<(), String> {
        let before_operation =
            "letptr=values.as_ptr();assert!(core::mem::size_of::<u8>()<=values.len());";
        let context = RawPointerReadBoundsApplicability::new(before_operation, "ptr")
            .ok_or_else(|| "ptr should trace to values".to_string())?;

        assert!(context.has_same_origin_bounds_evidence());
        Ok(())
    }

    #[test]
    fn applicability_rejects_other_origin_bounds() -> Result<(), String> {
        let before_operation =
            "letptr=other.as_ptr();assert!(core::mem::size_of::<u8>()<=values.len());";
        let context = RawPointerReadBoundsApplicability::new(before_operation, "ptr")
            .ok_or_else(|| "ptr should trace to other".to_string())?;

        assert!(!context.has_same_origin_bounds_evidence());
        Ok(())
    }

    #[test]
    fn applicability_rejects_stale_origin_after_guard() -> Result<(), String> {
        let before_operation = "letptr=values.as_ptr();assert!(core::mem::size_of::<u8>()<=values.len());values=other;";
        let context = RawPointerReadBoundsApplicability::new(before_operation, "ptr")
            .ok_or_else(|| "ptr should trace to values".to_string())?;

        assert!(!context.has_same_origin_bounds_evidence());
        Ok(())
    }
}
