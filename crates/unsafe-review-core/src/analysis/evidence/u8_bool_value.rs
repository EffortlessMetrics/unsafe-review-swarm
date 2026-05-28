use super::{
    any_compact_if_condition, branch_still_open_at_operation, condition_has_top_level_conjunct,
    condition_has_top_level_disjunct, contains_executable_return, has_assignment_to_identifier,
    has_fresh_guard_pattern, matching_code_block_end,
};

pub(super) fn has_u8_bool_value_guard(before_call: &str, argument: &str) -> bool {
    U8BoolValueApplicability::new(before_call, argument).has_valid_value_evidence()
}

pub(super) fn u8_bool_valid_value_predicates(target: &str) -> [String; 8] {
    [
        format!("{target}<=1"),
        format!("1>={target}"),
        format!("{target}<2"),
        format!("2>{target}"),
        format!("matches!({target},0|1)"),
        format!("matches!({target},1|0)"),
        format!("{target}==0||{target}==1"),
        format!("{target}==1||{target}==0"),
    ]
}

// U8-to-bool value evidence applies only to the same byte value that reaches
// the unsafe operation, and the byte must stay fresh after the evidence.
struct U8BoolValueApplicability<'a> {
    before_call: &'a str,
    same_source_value_target: &'a str,
}

impl<'a> U8BoolValueApplicability<'a> {
    fn new(before_call: &'a str, same_source_value_target: &'a str) -> Self {
        Self {
            before_call,
            same_source_value_target,
        }
    }

    fn has_valid_value_evidence(&self) -> bool {
        self.valid_value_predicates()
            .iter()
            .any(|predicate| self.has_value_predicate_guard(predicate))
            || self.has_invalid_early_return_guard()
    }

    fn valid_value_predicates(&self) -> [String; 8] {
        u8_bool_valid_value_predicates(self.same_source_value_target)
    }

    fn has_value_predicate_guard(&self, predicate: &str) -> bool {
        [
            format!("assert!({predicate})"),
            format!("assert!({predicate},"),
            format!("debug_assert!({predicate})"),
            format!("debug_assert!({predicate},"),
        ]
        .iter()
        .any(|pattern| self.has_fresh_assertion_guard(pattern))
            || self.has_open_positive_branch_guard(predicate)
    }

    fn has_fresh_assertion_guard(&self, pattern: &str) -> bool {
        has_fresh_guard_pattern(self.before_call, pattern, self.same_source_value_target)
    }

    fn has_open_positive_branch_guard(&self, predicate: &str) -> bool {
        any_compact_if_condition(self.before_call, |condition, after_guard| {
            condition_has_top_level_conjunct(condition, predicate)
                && self.open_branch_preserves_applicability(after_guard)
        })
    }

    fn open_branch_preserves_applicability(&self, after_guard: &str) -> bool {
        branch_still_open_at_operation(after_guard)
            && self.source_value_stays_fresh_after(after_guard)
    }

    fn has_invalid_early_return_guard(&self) -> bool {
        self.has_invalid_byte_returning_branch(&format!("{}>1", self.same_source_value_target))
            || self
                .has_invalid_byte_returning_branch(&format!("1<{}", self.same_source_value_target))
            || self
                .has_invalid_byte_returning_branch(&format!("{}>=2", self.same_source_value_target))
            || self
                .has_invalid_byte_returning_branch(&format!("2<={}", self.same_source_value_target))
    }

    fn has_invalid_byte_returning_branch(&self, predicate: &str) -> bool {
        any_compact_if_condition(self.before_call, |condition, after_guard| {
            condition_has_top_level_disjunct(condition, predicate)
                && self.returning_branch_preserves_applicability(after_guard)
        })
    }

    fn returning_branch_preserves_applicability(&self, after_guard: &str) -> bool {
        let (guard_body, after_branch) = matching_code_block_end(after_guard)
            .map_or((after_guard, ""), |body_end| {
                (&after_guard[..body_end], &after_guard[body_end + 1..])
            });
        contains_executable_return(guard_body) && self.source_value_stays_fresh_after(after_branch)
    }

    fn source_value_stays_fresh_after(&self, evidence: &str) -> bool {
        !has_assignment_to_identifier(evidence, self.same_source_value_target)
    }
}
