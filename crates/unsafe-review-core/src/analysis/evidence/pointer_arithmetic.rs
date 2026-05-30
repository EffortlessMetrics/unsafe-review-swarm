use super::{
    branch_still_open_at_operation, compact_code, contains_simple_assignment_to,
    matching_call_argument_end, receiver_before_marker, strip_block_comments_and_literals,
};

pub(super) fn has_pointer_arithmetic_bounds_guard(expression: &str, lower: &str) -> bool {
    let Some(offset) = pointer_arithmetic_offset(expression) else {
        return false;
    };
    let lower = strip_block_comments_and_literals(lower);
    let compact = compact_code(&lower);
    let context = PointerArithmeticBoundsApplicability::new(&compact, offset);

    has_pointer_arithmetic_bounds_assertion(&context)
        || has_pointer_arithmetic_bounds_open_branch(&context)
}

pub(super) fn has_slice_end_pointer_arithmetic_evidence(lower: &str) -> bool {
    let lower = strip_block_comments_and_literals(lower);
    let compact = compact_code(&lower);
    lower
        .lines()
        .filter_map(SliceEndPointerArithmeticApplicability::from_line)
        .any(|context| context.has_same_slice_end_pointer(&compact))
}

fn pointer_arithmetic_offset(expression: &str) -> Option<String> {
    let compact = compact_code(&expression.to_ascii_lowercase());
    for marker in [".add(", ".offset("] {
        let Some(_receiver) = receiver_before_marker(&compact, marker) else {
            continue;
        };
        let call_pos = compact.find(marker)? + marker.len();
        let argument_text = &compact[call_pos..];
        let argument_end = matching_call_argument_end(argument_text)?;
        let offset = &argument_text[..argument_end];
        if !offset.is_empty() {
            return Some(offset.to_string());
        }
    }
    None
}

struct PointerArithmeticBoundsApplicability<'a> {
    before_operation: &'a str,
    offset: String,
}

impl<'a> PointerArithmeticBoundsApplicability<'a> {
    fn new(before_operation: &'a str, offset: String) -> Self {
        Self {
            before_operation,
            offset,
        }
    }

    fn targets_stay_fresh_after(&self, bound_targets: &[String], evidence: &str) -> bool {
        !contains_simple_assignment_to(evidence, &self.offset)
            && bound_targets
                .iter()
                .all(|target| !contains_simple_assignment_to(evidence, target))
    }
}

fn has_pointer_arithmetic_bounds_assertion(
    context: &PointerArithmeticBoundsApplicability<'_>,
) -> bool {
    ["assert!(", "debug_assert!("].into_iter().any(|prefix| {
        let mut cursor = context.before_operation;
        let mut offset = 0usize;
        while let Some(pos) = cursor.find(prefix) {
            let statement_start = offset + pos + prefix.len();
            let after_prefix = &context.before_operation[statement_start..];
            let statement_end = after_prefix.find(';').unwrap_or(after_prefix.len());
            let statement = &after_prefix[..statement_end];
            if let Some(bound_targets) =
                same_offset_bounds_freshness_targets(statement, &context.offset)
                && context.targets_stay_fresh_after(&bound_targets, &after_prefix[statement_end..])
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

fn has_pointer_arithmetic_bounds_open_branch(
    context: &PointerArithmeticBoundsApplicability<'_>,
) -> bool {
    let mut cursor = context.before_operation;
    let mut offset = 0usize;
    while let Some(pos) = cursor.find("if") {
        let start = offset + pos;
        let before = context.before_operation[..start].chars().next_back();
        if before.is_some_and(is_receiver_path_char) {
            let next = pos + 2;
            offset += next;
            cursor = &cursor[next..];
            continue;
        }
        let after_if = &context.before_operation[start + 2..];
        if let Some(brace_pos) = after_if.find('{') {
            let condition = &after_if[..brace_pos];
            let after_body_start = &after_if[brace_pos + 1..];
            if let Some(bound_targets) =
                same_offset_bounds_freshness_targets(condition, &context.offset)
                && branch_still_open_at_operation(after_body_start)
                && context.targets_stay_fresh_after(&bound_targets, after_body_start)
            {
                return true;
            }
        }
        let next = pos + 2;
        offset += next;
        cursor = &cursor[next..];
    }
    false
}

fn same_offset_bounds_freshness_targets(condition: &str, offset: &str) -> Option<Vec<String>> {
    if condition.contains("||") {
        return None;
    }
    let mut freshness_targets = Vec::new();
    let mut matched = false;
    for op in [">=", "<=", "<", ">"] {
        let mut cursor = condition;
        let mut cursor_offset = 0usize;
        while let Some(pos) = cursor.find(op) {
            let op_start = cursor_offset + pos;
            let op_end = op_start + op.len();
            let left = comparison_left_operand(condition, op_start);
            let right = comparison_right_operand(condition, op_end);
            if operand_is_target(left, offset) && operand_mentions_bounds(right) {
                matched = true;
                push_bound_freshness_target(&mut freshness_targets, right);
            } else if operand_is_target(right, offset) && operand_mentions_bounds(left) {
                matched = true;
                push_bound_freshness_target(&mut freshness_targets, left);
            }
            let next = pos + op.len();
            cursor_offset += next;
            cursor = &cursor[next..];
        }
    }
    matched.then_some(freshness_targets)
}

fn comparison_left_operand(compact: &str, op_start: usize) -> &str {
    let left = &compact[..op_start];
    let start = left
        .rfind(['{', ';', ',', '=', '!'])
        .map_or(0, |idx| idx + 1);
    &left[start..]
}

fn comparison_right_operand(compact: &str, op_end: usize) -> &str {
    let right = &compact[op_end..];
    let end = right
        .find(['{', '}', ';', ',', '=', ')', '&', '|'])
        .unwrap_or(right.len());
    &right[..end]
}

fn operand_is_target(operand: &str, target: &str) -> bool {
    compact_code(operand) == target
}

fn operand_mentions_bounds(operand: &str) -> bool {
    let operand = compact_code(operand);
    operand.contains(".len()")
        || operand.contains(".capacity()")
        || operand.contains("num_ctrl_bytes()")
        || operand.contains("num_ctrl_bytes(")
        || contains_identifier(&operand, "len")
        || contains_identifier(&operand, "capacity")
}

fn push_bound_freshness_target(targets: &mut Vec<String>, operand: &str) {
    let operand = compact_code(operand);
    if is_simple_bound_identifier(&operand) && !targets.iter().any(|target| target == &operand) {
        targets.push(operand);
    }
}

fn is_simple_bound_identifier(operand: &str) -> bool {
    operand == "len" || operand == "capacity"
}

fn contains_identifier(text: &str, needle: &str) -> bool {
    text.split(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
        .any(|token| token == needle)
}

fn is_receiver_path_char(ch: char) -> bool {
    ch == '_' || ch == ':' || ch == '.' || ch.is_ascii_alphanumeric()
}

struct SliceEndPointerArithmeticApplicability {
    pointer_binding: String,
    slice_expr: String,
}

impl SliceEndPointerArithmeticApplicability {
    fn from_line(line: &str) -> Option<Self> {
        let line = compact_code(line);
        let after_let = line.strip_prefix("let")?;
        let (binding, expr) = after_let.split_once('=')?;
        let slice_expr = expr.strip_suffix(".as_ptr();")?;
        (!binding.is_empty() && !slice_expr.is_empty()).then_some(Self {
            pointer_binding: binding.to_string(),
            slice_expr: slice_expr.to_string(),
        })
    }

    fn has_same_slice_end_pointer(&self, compact: &str) -> bool {
        compact.contains(&format!(
            "{}.add({}.len())",
            self.pointer_binding, self.slice_expr
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_arithmetic_bounds_guard_uses_same_offset() {
        assert!(has_pointer_arithmetic_bounds_guard(
            "unsafe { self.ctrl.add(index) }",
            "debug_assert!(index < self.num_ctrl_bytes()); unsafe { self.ctrl.add(index) }",
        ));
        assert!(!has_pointer_arithmetic_bounds_guard(
            "unsafe { self.ctrl.add(index) }",
            "debug_assert!(other < self.num_ctrl_bytes()); unsafe { self.ctrl.add(index) }",
        ));
    }

    #[test]
    fn pointer_arithmetic_bounds_guard_rejects_stale_offset() {
        assert!(!has_pointer_arithmetic_bounds_guard(
            "unsafe { self.ctrl.add(index) }",
            "debug_assert!(index < self.num_ctrl_bytes()); index = fallback; unsafe { self.ctrl.add(index) }",
        ));
    }

    #[test]
    fn pointer_arithmetic_bounds_guard_rejects_stale_bound_identifier() {
        assert!(!has_pointer_arithmetic_bounds_guard(
            "unsafe { self.ctrl.add(index) }",
            "let mut len = self.num_ctrl_bytes(); debug_assert!(index < len); len = 0; unsafe { self.ctrl.add(index) }",
        ));
        assert!(has_pointer_arithmetic_bounds_guard(
            "unsafe { self.ctrl.add(index) }",
            "let len = self.num_ctrl_bytes(); debug_assert!(index < len); unsafe { self.ctrl.add(index) }",
        ));
    }

    #[test]
    fn pointer_arithmetic_bounds_guard_rejects_disjunctive_branch() {
        assert!(!has_pointer_arithmetic_bounds_guard(
            "unsafe { self.ctrl.add(index) }",
            "if index < self.num_ctrl_bytes() || allow_unchecked {",
        ));
        assert!(has_pointer_arithmetic_bounds_guard(
            "unsafe { self.ctrl.add(index) }",
            "if index < self.num_ctrl_bytes() && allow_checked {",
        ));
    }

    #[test]
    fn applicability_uses_same_binding_and_slice_length() -> Result<(), String> {
        let context =
            SliceEndPointerArithmeticApplicability::from_line("let start = haystack.as_ptr();")
                .ok_or_else(|| "start should trace to haystack.as_ptr()".to_string())?;

        assert!(context.has_same_slice_end_pointer(
            "letstart=haystack.as_ptr();letend=start.add(haystack.len());"
        ));
        Ok(())
    }

    #[test]
    fn applicability_rejects_other_pointer_binding() -> Result<(), String> {
        let context =
            SliceEndPointerArithmeticApplicability::from_line("let start = haystack.as_ptr();")
                .ok_or_else(|| "start should trace to haystack.as_ptr()".to_string())?;

        assert!(!context.has_same_slice_end_pointer(
            "letstart=haystack.as_ptr();letend=other.add(haystack.len());"
        ));
        Ok(())
    }

    #[test]
    fn applicability_rejects_other_slice_length() -> Result<(), String> {
        let context =
            SliceEndPointerArithmeticApplicability::from_line("let start = haystack.as_ptr();")
                .ok_or_else(|| "start should trace to haystack.as_ptr()".to_string())?;

        assert!(!context.has_same_slice_end_pointer(
            "letstart=haystack.as_ptr();letend=start.add(other.len());"
        ));
        Ok(())
    }
}
