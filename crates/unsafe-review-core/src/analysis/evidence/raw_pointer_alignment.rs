use super::{
    branch_still_open_at_operation, code_before_operation, compact_code, compact_if_guards,
    contains_executable_return, contains_receiver_fragment, contains_simple_assignment_to,
    is_receiver_path_char, matching_code_block_end, receiver_before_marker,
    strip_block_comments_and_literals,
};
use crate::analysis::scanner::ScannedSite;

pub(super) fn has_alignment_guard(site: &ScannedSite, lower: &str) -> bool {
    let stripped = strip_block_comments_and_literals(lower);
    let compact = compact_code(&stripped);
    if let Some(receiver) = raw_pointer_alignment_receiver(&site.operation.expression) {
        let guard_scope = code_before_operation(lower, &site.operation.expression)
            .unwrap_or_else(|| lower.to_string());
        let guard_compact = compact_code(&strip_block_comments_and_literals(&guard_scope));
        return RawPointerAlignmentApplicability::new(&guard_compact, &receiver)
            .has_same_receiver_alignment_evidence();
    }
    stripped.contains("is_aligned")
        || stripped.contains("align_offset")
        || stripped.contains("addr() %")
        || stripped.contains("as usize %")
        || compact.contains("addr()%")
        || compact.contains("asusize)%")
        || compact.contains("asusize%")
}

struct RawPointerAlignmentApplicability<'a> {
    guard_scope: &'a str,
    same_receiver_target: String,
}

impl<'a> RawPointerAlignmentApplicability<'a> {
    fn new(guard_scope: &'a str, receiver: &str) -> Self {
        Self {
            guard_scope,
            same_receiver_target: compact_code(&receiver.to_ascii_lowercase()),
        }
    }

    fn has_same_receiver_alignment_evidence(&self) -> bool {
        has_same_receiver_alignment_condition_guard(self.guard_scope, &self.same_receiver_target)
    }
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
        contains_executable_return(guard_body)
            && !contains_simple_assignment_to(after_guard_body, receiver)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applicability_uses_same_receiver_alignment_guard() {
        let context = RawPointerAlignmentApplicability::new("assert!(ptr.is_aligned());", "ptr");

        assert!(context.has_same_receiver_alignment_evidence());
    }

    #[test]
    fn applicability_rejects_other_receiver_alignment_guard() {
        let context = RawPointerAlignmentApplicability::new("assert!(other.is_aligned());", "ptr");

        assert!(!context.has_same_receiver_alignment_evidence());
    }

    #[test]
    fn applicability_rejects_stale_receiver_after_guard() {
        let context =
            RawPointerAlignmentApplicability::new("assert!(ptr.is_aligned());ptr=other;", "ptr");

        assert!(!context.has_same_receiver_alignment_evidence());
    }
}
