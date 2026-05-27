use super::{
    any_marker_tail, branch_still_open_at_operation, compact_code, contains_executable_return,
    contains_simple_assignment_to, match_some_branch_after_marker, matching_call_argument_end,
    matching_code_block_end, receiver_before_marker, strip_block_comments_and_literals,
};

pub(super) fn get_unchecked_receiver_and_index(expression: &str) -> Option<(String, String)> {
    let compact = compact_code(&expression.to_ascii_lowercase());
    for marker in [".get_unchecked_mut(", ".get_unchecked("] {
        let Some(receiver) = receiver_before_marker(&compact, marker) else {
            continue;
        };
        let call_pos = compact.find(marker)? + marker.len();
        let argument_text = &compact[call_pos..];
        let argument_end = matching_call_argument_end(argument_text)?;
        let index = &argument_text[..argument_end];
        if !receiver.is_empty() && !index.is_empty() {
            return Some((receiver.to_string(), index.to_string()));
        }
    }
    None
}

pub(super) fn has_get_unchecked_bounds_guard(lower: &str, receiver: &str, index: &str) -> bool {
    let compact = compact_code(&strip_block_comments_and_literals(lower));
    let receiver = compact_code(&receiver.to_ascii_lowercase());
    let index = compact_code(&index.to_ascii_lowercase());
    if receiver.is_empty() || index.is_empty() {
        return false;
    }
    let context = GetUncheckedBoundsApplicability::from_before_operation(&compact, receiver, index);

    has_get_unchecked_bounds_predicate(&context, &context.index_lt_len_predicate())
        || has_get_unchecked_bounds_predicate(&context, &context.len_gt_index_predicate())
        || has_get_unchecked_bounds_early_return(&context, &context.index_gte_len_predicate())
        || has_get_unchecked_bounds_early_return(&context, &context.len_lte_index_predicate())
        || has_get_unchecked_get_probe_guard(&context)
}

// Bounds evidence for get_unchecked must target the same slice receiver and
// index as the unsafe operation, and those targets must stay fresh until the
// call.
struct GetUncheckedBoundsApplicability<'a> {
    before_operation: &'a str,
    same_slice_target: String,
    same_index_target: String,
    same_slice_len_expr: String,
    same_slice_get_probe_expr: String,
}

impl<'a> GetUncheckedBoundsApplicability<'a> {
    fn from_before_operation(
        before_operation: &'a str,
        operation_receiver: String,
        operation_index: String,
    ) -> Self {
        let same_slice_len_expr = format!("{operation_receiver}.len()");
        let same_slice_get_probe_expr = format!("{operation_receiver}.get({operation_index})");
        Self {
            before_operation,
            same_slice_target: operation_receiver,
            same_index_target: operation_index,
            same_slice_len_expr,
            same_slice_get_probe_expr,
        }
    }

    fn index_lt_len_predicate(&self) -> String {
        format!("{}<{}", self.same_index_target, self.same_slice_len_expr)
    }

    fn len_gt_index_predicate(&self) -> String {
        format!("{}>{}", self.same_slice_len_expr, self.same_index_target)
    }

    fn index_gte_len_predicate(&self) -> String {
        format!("{}>={}", self.same_index_target, self.same_slice_len_expr)
    }

    fn len_lte_index_predicate(&self) -> String {
        format!("{}<={}", self.same_slice_len_expr, self.same_index_target)
    }

    fn has_stale_target_assignment(&self, text: &str) -> bool {
        contains_simple_assignment_to(text, &self.same_slice_target)
            || contains_simple_assignment_to(text, &self.same_index_target)
    }

    fn target_stays_fresh_after(&self, evidence: &str) -> bool {
        !self.has_stale_target_assignment(evidence)
    }

    fn open_branch_marker_preserves_applicability(&self, marker: &str) -> bool {
        any_marker_tail(self.before_operation, marker, |after_guard| {
            self.open_branch_preserves_applicability(after_guard)
        })
    }

    fn assertion_marker_preserves_applicability(&self, marker: &str) -> bool {
        any_marker_tail(self.before_operation, marker, |after_assertion| {
            self.target_stays_fresh_after(after_assertion)
        })
    }

    fn returning_marker_preserves_applicability(&self, marker: &str) -> bool {
        any_marker_tail(self.before_operation, marker, |after_guard| {
            let (guard_body, after_guard_body) = matching_code_block_end(after_guard)
                .map_or((after_guard, ""), |body_end| {
                    (&after_guard[..body_end], &after_guard[body_end + 1..])
                });
            self.returning_guard_preserves_applicability(guard_body, after_guard_body)
        })
    }

    fn open_branch_preserves_applicability(&self, after_guard: &str) -> bool {
        branch_still_open_at_operation(after_guard) && self.target_stays_fresh_after(after_guard)
    }

    fn returning_guard_preserves_applicability(
        &self,
        guard_body: &str,
        after_guard_body: &str,
    ) -> bool {
        contains_executable_return(guard_body) && self.target_stays_fresh_after(after_guard_body)
    }
}

fn has_get_unchecked_bounds_predicate(
    context: &GetUncheckedBoundsApplicability<'_>,
    predicate: &str,
) -> bool {
    has_get_unchecked_open_bounds_branch(context, predicate)
        || has_get_unchecked_bounds_assertion(context, predicate)
}

fn has_get_unchecked_open_bounds_branch(
    context: &GetUncheckedBoundsApplicability<'_>,
    predicate: &str,
) -> bool {
    let marker = format!("if{predicate}{{");
    context.open_branch_marker_preserves_applicability(&marker)
}

fn has_get_unchecked_bounds_assertion(
    context: &GetUncheckedBoundsApplicability<'_>,
    predicate: &str,
) -> bool {
    ["assert!(", "debug_assert!("].into_iter().any(|prefix| {
        let marker = format!("{prefix}{predicate}");
        context.assertion_marker_preserves_applicability(&marker)
    })
}

fn has_get_unchecked_bounds_early_return(
    context: &GetUncheckedBoundsApplicability<'_>,
    predicate: &str,
) -> bool {
    let guard = format!("if{predicate}{{");
    context.returning_marker_preserves_applicability(&guard)
}

fn has_get_unchecked_get_probe_guard(context: &GetUncheckedBoundsApplicability<'_>) -> bool {
    has_get_unchecked_get_probe_open_branch(context)
        || has_get_unchecked_get_probe_early_return(context)
        || has_get_unchecked_get_probe_if_let_branch(context)
        || has_get_unchecked_get_probe_let_else(context)
        || has_get_unchecked_get_probe_match_branch(context)
}

fn has_get_unchecked_get_probe_open_branch(context: &GetUncheckedBoundsApplicability<'_>) -> bool {
    let marker = format!("if{}.is_some(){{", context.same_slice_get_probe_expr);
    context.open_branch_marker_preserves_applicability(&marker)
}

fn has_get_unchecked_get_probe_early_return(context: &GetUncheckedBoundsApplicability<'_>) -> bool {
    let marker = format!("if{}.is_none(){{", context.same_slice_get_probe_expr);
    context.returning_marker_preserves_applicability(&marker)
}

fn has_get_unchecked_get_probe_if_let_branch(
    context: &GetUncheckedBoundsApplicability<'_>,
) -> bool {
    let marker = format!("ifletsome(_)={}{{", context.same_slice_get_probe_expr);
    context.open_branch_marker_preserves_applicability(&marker)
}

fn has_get_unchecked_get_probe_let_else(context: &GetUncheckedBoundsApplicability<'_>) -> bool {
    let marker = format!("letsome(_)={}else{{", context.same_slice_get_probe_expr);
    context.returning_marker_preserves_applicability(&marker)
}

fn has_get_unchecked_get_probe_match_branch(context: &GetUncheckedBoundsApplicability<'_>) -> bool {
    let marker = format!("match{}{{", context.same_slice_get_probe_expr);
    any_marker_tail(context.before_operation, &marker, |after_match| {
        if let Some(branch_after_marker) = match_some_branch_after_marker(after_match)
            && context.open_branch_preserves_applicability(branch_after_marker)
        {
            return true;
        }
        false
    })
}
