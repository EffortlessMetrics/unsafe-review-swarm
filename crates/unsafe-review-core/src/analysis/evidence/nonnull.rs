use crate::analysis::scanner::ScannedSite;

use super::{
    any_compact_if_condition, any_marker_occurrence, any_marker_tail,
    branch_still_open_at_operation, code_before_operation, compact_code,
    condition_has_top_level_conjunct, condition_has_top_level_disjunct, contains_executable_return,
    contains_simple_assignment_to, ends_with_some_pattern, match_some_branch_after_marker,
    matching_code_block_end, strip_block_comments_and_literals,
};

pub(super) fn has_nullability_guard(site: &ScannedSite, lower: &str) -> bool {
    let stripped = strip_block_comments_and_literals(lower);
    let compact = compact_code(&stripped);
    if let Some(arg) = nonnull_new_unchecked_argument(&site.operation.expression) {
        let arg = compact_code(&arg.to_ascii_lowercase());
        let guard_scope = code_before_operation(lower, &site.operation.expression)
            .unwrap_or_else(|| lower.to_string());
        let guard_compact = compact_code(&strip_block_comments_and_literals(&guard_scope));
        let context = NonNullPointerContext::new(&guard_compact, arg);
        return context.has_nullability_guard();
    }
    stripped.contains("is_null") || compact.contains("nonnull::new(")
}

struct NonNullPointerContext<'a> {
    compact: &'a str,
    same_pointer_target: String,
    same_pointer_new_probe: String,
}

impl<'a> NonNullPointerContext<'a> {
    fn new(compact: &'a str, arg: String) -> Self {
        let same_pointer_new_probe = format!("nonnull::new({arg})");
        Self {
            compact,
            same_pointer_target: arg,
            same_pointer_new_probe,
        }
    }

    fn has_stale_pointer_assignment(&self, text: &str) -> bool {
        contains_simple_assignment_to(text, &self.same_pointer_target)
    }

    fn pointer_stays_fresh_after(&self, evidence: &str) -> bool {
        !self.has_stale_pointer_assignment(evidence)
    }

    fn statement_after_marker_preserves_applicability(&self, after_marker: &str) -> bool {
        let after_statement = after_marker
            .find(';')
            .map_or(after_marker, |end| &after_marker[end..]);
        self.pointer_stays_fresh_after(after_statement)
    }

    fn open_branch_preserves_applicability(&self, after_guard: &str) -> bool {
        branch_still_open_at_operation(after_guard) && self.pointer_stays_fresh_after(after_guard)
    }

    fn returning_guard_preserves_applicability(
        &self,
        guard_body: &str,
        after_guard_body: &str,
    ) -> bool {
        contains_executable_return(guard_body) && self.pointer_stays_fresh_after(after_guard_body)
    }

    fn returning_after_marker_preserves_applicability(&self, after_guard: &str) -> bool {
        let (guard_body, after_guard_body) = matching_code_block_end(after_guard)
            .map_or((after_guard, ""), |body_end| {
                (&after_guard[..body_end], &after_guard[body_end + 1..])
            });
        self.returning_guard_preserves_applicability(guard_body, after_guard_body)
    }

    fn has_nullability_guard(&self) -> bool {
        self.has_question_mark_guard()
            || self.has_if_let_guard()
            || self.has_let_else_guard()
            || self.has_match_some_guard()
            || self.has_null_early_return_guard()
            || self.has_non_null_open_branch_guard()
    }

    fn has_question_mark_guard(&self) -> bool {
        let marker = format!("{}?", self.same_pointer_new_probe);
        any_marker_tail(self.compact, &marker, |after_marker| {
            self.statement_after_marker_preserves_applicability(after_marker)
        })
    }

    fn has_if_let_guard(&self) -> bool {
        let marker = format!("={}{{", self.same_pointer_new_probe);
        any_marker_occurrence(self.compact, &marker, |marker_start, after_guard| {
            ends_with_some_pattern(&self.compact[..marker_start], "iflet")
                && self.open_branch_preserves_applicability(after_guard)
        })
    }

    fn has_let_else_guard(&self) -> bool {
        let marker = format!("={}else{{", self.same_pointer_new_probe);
        any_marker_occurrence(self.compact, &marker, |marker_start, after_guard| {
            ends_with_some_pattern(&self.compact[..marker_start], "let")
                && self.returning_after_marker_preserves_applicability(after_guard)
        })
    }

    fn has_match_some_guard(&self) -> bool {
        let marker = format!("match{}{{", self.same_pointer_new_probe);
        any_marker_tail(self.compact, &marker, |after_match| {
            if let Some(branch_after_marker) = match_some_branch_after_marker(after_match)
                && self.open_branch_preserves_applicability(branch_after_marker)
            {
                return true;
            }
            false
        })
    }

    fn has_null_early_return_guard(&self) -> bool {
        let predicate = format!("{}.is_null()", self.same_pointer_target);
        any_compact_if_condition(self.compact, |condition, after_guard| {
            condition_has_top_level_disjunct(condition, &predicate)
                && self.returning_after_marker_preserves_applicability(after_guard)
        })
    }

    fn has_non_null_open_branch_guard(&self) -> bool {
        let predicate = format!("!{}.is_null()", self.same_pointer_target);
        any_compact_if_condition(self.compact, |condition, after_guard| {
            condition_has_top_level_conjunct(condition, &predicate)
                && self.open_branch_preserves_applicability(after_guard)
        })
    }
}

fn nonnull_new_unchecked_argument(expression: &str) -> Option<String> {
    let compact = compact_code(&expression.to_ascii_lowercase());
    let marker = "nonnull::new_unchecked(";
    let start = compact.find(marker)? + marker.len();
    let rest = &compact[start..];
    let mut depth = 0usize;
    let mut end = rest.len();
    for (idx, ch) in rest.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' if depth == 0 => {
                end = idx;
                break;
            }
            ')' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    let arg = rest[..end].trim();
    (!arg.is_empty()).then(|| arg.to_string())
}
