use super::{
    compact_code, contains_simple_assignment_to, let_binding_name, matching_call_argument_end,
    strip_block_comments_and_literals,
};

pub(super) fn has_drop_in_place_box_origin_evidence(expression: &str, lower: &str) -> bool {
    let compact = compact_code(&strip_block_comments_and_literals(lower));
    let compact_expression = compact_code(&expression.to_ascii_lowercase());
    let Some(pointer) = drop_in_place_argument(&compact_expression) else {
        return false;
    };
    let call_pos = compact
        .find(&compact_expression)
        .or_else(|| compact.find(&format!("drop_in_place({pointer})")));
    let Some(call_pos) = call_pos else {
        return false;
    };
    BoxRawOriginApplicability::new(&compact[..call_pos], pointer).has_same_pointer_box_into_raw()
}

fn drop_in_place_argument(compact_expression: &str) -> Option<&str> {
    let marker = "drop_in_place(";
    let call_pos = compact_expression.find(marker)? + marker.len();
    let argument_text = &compact_expression[call_pos..];
    let argument_end = matching_call_argument_end(argument_text)?;
    let argument = &argument_text[..argument_end];
    (!argument.is_empty()).then_some(argument)
}

fn box_into_raw_argument(right_side: &str) -> Option<&str> {
    let marker = "box::into_raw(";
    let call_pos = right_side.find(marker)? + marker.len();
    let argument_text = &right_side[call_pos..];
    let argument_end = matching_call_argument_end(argument_text)?;
    let argument = &argument_text[..argument_end];
    (!argument.is_empty()).then_some(argument)
}

pub(super) fn has_box_from_raw_origin_evidence(expression: &str, lower: &str) -> bool {
    let compact = compact_code(&strip_block_comments_and_literals(lower));
    let compact_expression = compact_code(&expression.to_ascii_lowercase());
    let Some(pointer) = box_from_raw_argument(&compact_expression) else {
        return false;
    };
    let call_pos = compact
        .find(&compact_expression)
        .or_else(|| compact.find(&format!("box::from_raw({pointer})")));
    let Some(call_pos) = call_pos else {
        return false;
    };
    BoxRawOriginApplicability::new(&compact[..call_pos], pointer).has_same_pointer_box_into_raw()
}

struct BoxRawOriginApplicability<'a> {
    before_call: &'a str,
    same_pointer_target: &'a str,
}

impl<'a> BoxRawOriginApplicability<'a> {
    fn new(before_call: &'a str, pointer: &'a str) -> Self {
        Self {
            before_call,
            same_pointer_target: pointer,
        }
    }

    fn has_same_pointer_box_into_raw(&self) -> bool {
        let mut offset = 0usize;
        for statement in self.before_call.split(';') {
            if self.statement_assigns_same_pointer_box_into_raw(statement)
                && self.pointer_stays_fresh_after_origin(offset, statement)
            {
                return true;
            }
            offset += statement.len() + 1;
        }
        false
    }

    fn statement_assigns_same_pointer_box_into_raw(&self, statement: &str) -> bool {
        let Some((left, right)) = statement.split_once('=') else {
            return false;
        };
        let Some(binding) = let_binding_name(left) else {
            return false;
        };
        binding == self.same_pointer_target && box_into_raw_argument(right).is_some()
    }

    fn pointer_stays_fresh_after_origin(&self, offset: usize, statement: &str) -> bool {
        let after_origin =
            &self.before_call[(offset + statement.len()).min(self.before_call.len())..];
        !contains_simple_assignment_to(after_origin, self.same_pointer_target)
    }
}

fn box_from_raw_argument(compact_expression: &str) -> Option<&str> {
    let marker = "box::from_raw(";
    let call_pos = compact_expression.find(marker)? + marker.len();
    let argument_text = &compact_expression[call_pos..];
    let argument_end = matching_call_argument_end(argument_text)?;
    let argument = &argument_text[..argument_end];
    (!argument.is_empty()).then_some(argument)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_applicability_requires_same_pointer_target() {
        let matching = BoxRawOriginApplicability::new("letptr=box::into_raw(value);", "ptr");
        let other = BoxRawOriginApplicability::new("letother=box::into_raw(value);", "ptr");

        assert!(matching.has_same_pointer_box_into_raw());
        assert!(!other.has_same_pointer_box_into_raw());
    }

    #[test]
    fn origin_applicability_rejects_stale_pointer_target() {
        let stale = BoxRawOriginApplicability::new(
            "letmutptr=box::into_raw(value);ptr=foreign_ptr;",
            "ptr",
        );

        assert!(!stale.has_same_pointer_box_into_raw());
    }
}
