use super::{
    branch_still_open_at_operation, code_before_operation, compact_code, compact_if_guards,
    contains_receiver_fragment, contains_simple_assignment_to, is_receiver_path_char,
    matching_code_block_end, receiver_before_marker,
};
use crate::analysis::scanner::ScannedSite;

pub(super) fn has_alignment_guard(site: &ScannedSite, lower: &str) -> bool {
    let compact = compact_code(lower);
    if let Some(receiver) = raw_pointer_alignment_receiver(&site.operation.expression) {
        let guard_scope = code_before_operation(lower, &site.operation.expression)
            .unwrap_or_else(|| lower.to_string());
        let guard_compact = compact_code(&guard_scope);
        return has_same_receiver_alignment_guard(&guard_compact, &receiver);
    }
    lower.contains("is_aligned")
        || lower.contains("align_offset")
        || lower.contains("addr() %")
        || lower.contains("as usize %")
        || compact.contains("addr()%")
        || compact.contains("asusize)%")
        || compact.contains("asusize%")
}

fn has_same_receiver_alignment_guard(compact: &str, receiver: &str) -> bool {
    let receiver = compact_code(&receiver.to_ascii_lowercase());
    has_same_receiver_alignment_condition_guard(compact, &receiver)
}

fn has_same_receiver_alignment_condition_guard(compact: &str, receiver: &str) -> bool {
    has_alignment_assertion_guard(compact, receiver)
        || has_alignment_open_positive_branch_guard(compact, receiver)
        || has_alignment_early_return_guard(compact, receiver)
}

fn has_alignment_assertion_guard(compact: &str, receiver: &str) -> bool {
    ["assert!(", "debug_assert!("].into_iter().any(|prefix| {
        let mut cursor = compact;
        let mut offset = 0usize;
        while let Some(pos) = cursor.find(prefix) {
            let statement_start = offset + pos + prefix.len();
            let after_prefix = &compact[statement_start..];
            let statement_end = after_prefix.find(';').unwrap_or(after_prefix.len());
            let statement = &after_prefix[..statement_end];
            let after_statement = &after_prefix[statement_end..];
            if alignment_condition_is_positive(statement, receiver)
                && !contains_simple_assignment_to(after_statement, receiver)
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

fn has_alignment_open_positive_branch_guard(compact: &str, receiver: &str) -> bool {
    compact_if_guards(compact).any(|guard| {
        alignment_condition_is_positive(guard.condition, receiver)
            && branch_still_open_at_operation(guard.after_body_start)
            && !contains_simple_assignment_to(guard.after_body_start, receiver)
    })
}

fn has_alignment_early_return_guard(compact: &str, receiver: &str) -> bool {
    compact_if_guards(compact).any(|guard| {
        if !alignment_condition_is_negative(guard.condition, receiver) {
            return false;
        }
        let (guard_body, after_guard_body) = matching_code_block_end(guard.after_body_start)
            .map_or((guard.after_body_start, ""), |body_end| {
                (
                    &guard.after_body_start[..body_end],
                    &guard.after_body_start[body_end + 1..],
                )
            });
        guard_body.contains("return") && !contains_simple_assignment_to(after_guard_body, receiver)
    })
}

fn alignment_condition_is_positive(condition: &str, receiver: &str) -> bool {
    if same_receiver_method_call(condition, receiver, "is_aligned") {
        return !condition.starts_with('!')
            && !condition.contains(".is_aligned()==false")
            && !condition.contains(".is_aligned()!=true");
    }
    (same_receiver_method_call(condition, receiver, "align_offset")
        || same_receiver_alignment_modulo(condition, receiver))
        && condition.contains("==0")
}

fn alignment_condition_is_negative(condition: &str, receiver: &str) -> bool {
    if same_receiver_method_call(condition, receiver, "is_aligned") {
        return condition.starts_with('!')
            || condition.contains(".is_aligned()==false")
            || condition.contains(".is_aligned()!=true");
    }
    (same_receiver_method_call(condition, receiver, "align_offset")
        || same_receiver_alignment_modulo(condition, receiver))
        && condition.contains("!=0")
}

fn same_receiver_alignment_modulo(compact: &str, receiver: &str) -> bool {
    contains_receiver_fragment(compact, &format!("{receiver}.addr()%"))
        || contains_receiver_fragment(compact, &format!("{receiver}asusize)%"))
        || contains_receiver_fragment(compact, &format!("{receiver}asusize%"))
        || contains_receiver_fragment(compact, &format!("({receiver}asusize)%"))
        || contains_receiver_fragment(compact, &format!("({receiver}asusize%"))
}

fn same_receiver_method_call(compact: &str, receiver: &str, method: &str) -> bool {
    let direct = format!("{receiver}.{method}");
    if contains_receiver_fragment(compact, &direct) {
        return true;
    }
    let cast_prefix = format!("{receiver}.cast");
    let mut cursor = compact;
    let mut offset = 0usize;
    while let Some(pos) = cursor.find(&cast_prefix) {
        let start = offset + pos;
        let before = compact[..start].chars().next_back();
        let starts_on_boundary = before.is_none_or(|ch| !is_receiver_path_char(ch));
        let after_receiver = &compact[start + receiver.len()..];
        let end = after_receiver
            .find([';', '{', '}'])
            .unwrap_or(after_receiver.len());
        if starts_on_boundary && after_receiver[..end].contains(&format!(".{method}")) {
            return true;
        }
        let next = pos + cast_prefix.len();
        offset += next;
        cursor = &cursor[next..];
    }
    false
}

fn raw_pointer_alignment_receiver(expression: &str) -> Option<String> {
    let compact = compact_code(&expression.to_ascii_lowercase());
    if let Some(receiver) = receiver_before_marker(&compact, ".cast::<") {
        return Some(receiver.to_string());
    }
    if let Some(receiver) = receiver_before_marker(&compact, ".read(") {
        return Some(receiver.to_string());
    }
    if let Some(receiver) = receiver_before_marker(&compact, ".read_volatile(") {
        return Some(receiver.to_string());
    }
    if let Some(receiver) = receiver_before_marker(&compact, ".write(") {
        return Some(receiver.to_string());
    }
    if let Some(receiver) = receiver_before_marker(&compact, ".write_volatile(") {
        return Some(receiver.to_string());
    }
    None
}
