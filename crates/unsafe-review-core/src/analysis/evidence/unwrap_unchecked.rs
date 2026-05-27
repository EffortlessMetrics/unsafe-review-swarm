use super::{
    branch_still_open_at_operation, compact_code, contains_receiver_fragment,
    has_assignment_to_identifier, is_receiver_path_char, is_simple_identifier, is_some_binding,
    matching_code_block_end, strip_block_comments_and_literals,
};

pub(super) fn has_unwrap_unchecked_infallible_result_evidence(lower: &str) -> bool {
    let compact = compact_code(lower);
    let Some(context) = unwrap_unchecked_receiver_context(&compact) else {
        return false;
    };
    has_infallible_assignment_to_receiver(context)
}

pub(super) fn has_unwrap_unchecked_receiver_state_evidence(lower: &str) -> bool {
    let compact = compact_code(&strip_block_comments_and_literals(lower));
    let Some(context) = unwrap_unchecked_receiver_context(&compact) else {
        return false;
    };

    has_receiver_positive_branch_guard(context, "is_some")
        || has_receiver_positive_branch_guard(context, "is_ok")
        || has_receiver_if_let_as_ref_guard(context, "some")
        || has_receiver_let_else_as_ref_guard(context, "some")
        || has_receiver_match_as_ref_guard(context, "some")
        || has_receiver_if_let_as_ref_guard(context, "ok")
        || has_receiver_let_else_as_ref_guard(context, "ok")
        || has_receiver_match_as_ref_guard(context, "ok")
        || has_receiver_early_return_guard(context, "is_none")
        || has_receiver_early_return_guard(context, "is_err")
}

// Receiver state evidence only applies to the exact unwrap_unchecked receiver
// and must remain fresh until the unsafe call.
#[derive(Clone, Copy)]
struct ReceiverEvidenceContext<'a> {
    before_call: &'a str,
    receiver: &'a str,
}

impl ReceiverEvidenceContext<'_> {
    fn has_assignment_after_branch(self, after_branch: &str) -> bool {
        is_simple_identifier(self.receiver)
            && has_assignment_to_identifier(after_branch, self.receiver)
    }
}

fn has_infallible_assignment_to_receiver(context: ReceiverEvidenceContext<'_>) -> bool {
    let before_call = context.before_call;
    let receiver = context.receiver;
    let let_assignment = format!("let{receiver}=");
    let assignment = format!("{receiver}=");
    before_call.split(';').any(|statement| {
        statement.contains("fallibility::infallible")
            && (contains_receiver_fragment(statement, &let_assignment)
                || contains_receiver_fragment(statement, &assignment))
    })
}

fn unwrap_unchecked_receiver_context(compact: &str) -> Option<ReceiverEvidenceContext<'_>> {
    let call_pos = compact.find(".unwrap_unchecked(")?;
    let before_call = &compact[..call_pos];
    let receiver_start = before_call
        .char_indices()
        .rev()
        .find_map(|(idx, ch)| (!is_receiver_path_char(ch)).then_some(idx + ch.len_utf8()))
        .unwrap_or(0);
    let receiver = &before_call[receiver_start..];
    (!receiver.is_empty()).then_some(ReceiverEvidenceContext {
        before_call,
        receiver,
    })
}

fn has_receiver_early_return_guard(context: ReceiverEvidenceContext<'_>, predicate: &str) -> bool {
    let before_call = context.before_call;
    let receiver = context.receiver;
    let guard = format!("if{receiver}.{predicate}(){{");
    let Some((_prefix, after_guard)) = before_call.split_once(&guard) else {
        return false;
    };
    let guard_returned = after_guard
        .split_once('}')
        .map_or(after_guard, |(guard_body, _after)| guard_body)
        .contains("return");
    guard_returned && !context.has_assignment_after_branch(after_guard)
}

fn has_receiver_positive_branch_guard(
    context: ReceiverEvidenceContext<'_>,
    predicate: &str,
) -> bool {
    let guard = format!("if{}.{predicate}(){{", context.receiver);
    has_open_receiver_branch_guard(context, &guard)
}

fn has_receiver_if_let_as_ref_guard(
    context: ReceiverEvidenceContext<'_>,
    constructor: &str,
) -> bool {
    let guard = format!("iflet{constructor}(_)={}.as_ref(){{", context.receiver);
    has_open_receiver_branch_guard(context, &guard)
}

fn has_receiver_let_else_as_ref_guard(
    context: ReceiverEvidenceContext<'_>,
    constructor: &str,
) -> bool {
    let before_call = context.before_call;
    let guard = format!("let{constructor}(_)={}.as_ref()else{{", context.receiver);
    let mut search_from = 0usize;
    while let Some(offset) = before_call[search_from..].find(&guard) {
        let guard_start = search_from + offset;
        let after_guard = &before_call[guard_start + guard.len()..];
        let (guard_body, after_guard_body) = matching_code_block_end(after_guard)
            .map_or((after_guard, ""), |body_end| {
                (&after_guard[..body_end], &after_guard[body_end + 1..])
            });
        if guard_body.contains("return") && !context.has_assignment_after_branch(after_guard_body) {
            return true;
        }
        search_from = guard_start + guard.len();
    }
    false
}

fn has_receiver_match_as_ref_guard(
    context: ReceiverEvidenceContext<'_>,
    constructor: &str,
) -> bool {
    let before_call = context.before_call;
    let marker = format!("match{}.as_ref(){{", context.receiver);
    let mut cursor = before_call;
    let mut offset = 0usize;
    while let Some(pos) = cursor.find(&marker) {
        let after_match_start = offset + pos + marker.len();
        let after_match = &before_call[after_match_start..];
        if let Some(branch_after_marker) =
            match_constructor_branch_after_marker(after_match, constructor)
            && branch_still_open_at_operation(branch_after_marker)
            && !context.has_assignment_after_branch(branch_after_marker)
        {
            return true;
        }
        let next = pos + marker.len();
        offset += next;
        cursor = &cursor[next..];
    }
    false
}

fn match_constructor_branch_after_marker<'a>(
    after_match: &'a str,
    constructor: &str,
) -> Option<&'a str> {
    let marker = format!("{constructor}(");
    let constructor_pos = after_match.find(&marker)?;
    let after_constructor = &after_match[constructor_pos + marker.len()..];
    let (binding, after_binding) = after_constructor.split_once(")=>{")?;
    is_some_binding(binding).then_some(after_binding)
}

fn has_open_receiver_branch_guard(context: ReceiverEvidenceContext<'_>, guard: &str) -> bool {
    let before_call = context.before_call;
    let mut search_from = 0;
    while let Some(offset) = before_call[search_from..].find(guard) {
        let guard_start = search_from + offset;
        let after_guard = &before_call[guard_start + guard.len()..];
        let mut depth = 1usize;
        for ch in after_guard.chars() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
        }
        if depth > 0 && !context.has_assignment_after_branch(after_guard) {
            return true;
        }
        search_from = guard_start + guard.len();
    }
    false
}
