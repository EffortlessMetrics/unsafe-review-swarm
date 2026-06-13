use super::{
    branch_still_open_at_operation, code_before_operation, compact_code, compact_if_guards,
    contains_executable_return, contains_receiver_fragment, contains_simple_assignment_to,
    is_receiver_path_char, matching_code_block_end, receiver_before_marker,
    strip_block_comments_and_literals,
};
use crate::analysis::scanner::ScannedSite;

pub(super) fn has_alignment_guard(site: &ScannedSite, lower: &str) -> bool {
    if let Some(receiver) = raw_pointer_alignment_receiver(&site.operation.expression) {
        let guard_scope = code_before_operation(lower, &site.operation.expression)
            .unwrap_or_else(|| lower.to_string());
        let guard_compact = compact_code(&strip_block_comments_and_literals(&guard_scope));
        return RawPointerAlignmentApplicability::new(&guard_compact, &receiver)
            .has_same_receiver_alignment_evidence();
    }
    false
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
    // Deref form: `*ptr` — the expression starts with `*` followed by receiver path chars.
    if let Some(receiver) = deref_expression_receiver(&compact) {
        return Some(receiver);
    }
    // Free-fn forms: `core::ptr::read(ptr)`, `ptr::read_volatile(ptr, ...)`, etc.
    // These use `::read(` / `::write(` not `.read(`, so the method-receiver path above does not
    // match them.  Extract the first argument of the call as the receiver.
    for marker in &[
        "::read(",
        "::read_volatile(",
        "::write(",
        "::write_volatile(",
    ] {
        if let Some(arg) = free_fn_first_arg(&compact, marker) {
            return Some(arg);
        }
    }
    None
}

/// Extract the raw-pointer identifier from a deref expression such as `*ptr` or `*self.ptr`.
/// Returns `None` when the expression does not start with `*` or the operand is not a plain
/// receiver path (e.g. `*(complex_expr)` is excluded so we do not produce a mis-anchored
/// receiver).
fn deref_expression_receiver(compact: &str) -> Option<String> {
    let rest = compact.strip_prefix('*')?;
    // Only accept simple receiver-path identifiers (letters, digits, `_`, `.`, `:`).
    // Reject parenthesised expressions like `*(ptr.add(n))` where extracting a receiver
    // would be unreliable.
    if rest.starts_with('(') {
        return None;
    }
    let receiver: String = rest
        .chars()
        .take_while(|ch| is_receiver_path_char(*ch))
        .collect();
    if receiver.is_empty() {
        return None;
    }
    Some(receiver)
}

/// Given a compact (whitespace-free) expression, find the free-function call marker
/// (e.g. `::read(`) and return the first argument — the pointer operand — as a
/// receiver string.  Only simple receiver-path arguments are accepted; complex
/// expressions inside the parens are excluded so we do not produce mis-anchored
/// receivers.
fn free_fn_first_arg(compact: &str, marker: &str) -> Option<String> {
    let pos = compact.find(marker)?;
    let after_open = &compact[pos + marker.len()..];
    // Extract chars that form a valid receiver path.
    let arg: String = after_open
        .chars()
        .take_while(|ch| is_receiver_path_char(*ch))
        .collect();
    if arg.is_empty() {
        return None;
    }
    // The argument must be followed by `,`, `)`, or end-of-string to confirm it is a
    // simple identifier and not a prefix of a more complex expression.
    let after_arg = &after_open[arg.len()..];
    let next = after_arg.chars().next();
    if next.is_none_or(|ch| ch == ',' || ch == ')') {
        Some(arg)
    } else {
        None
    }
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
