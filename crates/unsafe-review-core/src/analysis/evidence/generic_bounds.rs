use super::{
    branch_still_open_at_operation, compact_code, compact_contains_identifier,
    strip_block_comments_and_literals,
};

pub(super) fn has_length_or_bounds_guard(lower: &str) -> bool {
    let lower = strip_block_comments_and_literals(lower);
    let compact = compact_code(&lower);
    has_bounds_assertion_guard(&compact)
        || has_bounds_open_positive_branch_guard(&compact)
        || has_len_capacity_equality_guard(&lower)
}

fn has_bounds_assertion_guard(compact: &str) -> bool {
    ["assert!(", "debug_assert!("].into_iter().any(|prefix| {
        let mut cursor = compact;
        let mut offset = 0usize;
        while let Some(pos) = cursor.find(prefix) {
            let statement_start = offset + pos + prefix.len();
            let after_prefix = &compact[statement_start..];
            let statement_end = after_prefix.find(';').unwrap_or(after_prefix.len());
            let statement = &after_prefix[..statement_end];
            if has_bounds_condition(statement) {
                return true;
            }
            let next = pos + prefix.len();
            offset += next;
            cursor = &cursor[next..];
        }
        false
    })
}

fn has_bounds_open_positive_branch_guard(compact: &str) -> bool {
    let mut cursor = compact;
    let mut offset = 0usize;
    while let Some(pos) = cursor.find("if") {
        let start = offset + pos;
        let before = compact[..start].chars().next_back();
        if before.is_some_and(is_receiver_path_char) {
            let next = pos + 2;
            offset += next;
            cursor = &cursor[next..];
            continue;
        }
        let after_if = &compact[start + 2..];
        if let Some(brace_pos) = after_if.find('{') {
            let condition = &after_if[..brace_pos];
            let after_body_start = &after_if[brace_pos + 1..];
            if has_bounds_condition(condition) && branch_still_open_at_operation(after_body_start) {
                return true;
            }
        }
        let next = pos + 2;
        offset += next;
        cursor = &cursor[next..];
    }
    false
}

fn has_bounds_condition(compact: &str) -> bool {
    for op in [">=", "<=", "<", ">"] {
        let mut cursor = compact;
        let mut offset = 0usize;
        while let Some(pos) = cursor.find(op) {
            let op_start = offset + pos;
            let op_end = op_start + op.len();
            let left = comparison_left_operand(compact, op_start);
            let right = comparison_right_operand(compact, op_end);
            if operand_mentions_bounds(left) || operand_mentions_bounds(right) {
                return true;
            }
            let next = pos + op.len();
            offset += next;
            cursor = &cursor[next..];
        }
    }
    false
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
    let end = right.find(['{', '}', ';', ',', '=']).unwrap_or(right.len());
    &right[..end]
}

fn operand_mentions_bounds(operand: &str) -> bool {
    operand.contains(".len()")
        || operand.contains(".capacity()")
        || operand.contains("num_ctrl_bytes()")
        || compact_contains_identifier(operand, "len")
        || compact_contains_identifier(operand, "capacity")
}

fn has_len_capacity_equality_guard(lower: &str) -> bool {
    let compact = compact_code(lower);
    let has_equality = compact.contains("==")
        || compact.contains("assert_eq!(")
        || compact.contains("debug_assert_eq!(");
    has_equality
        && compact.contains("len")
        && (compact.contains("capacity") || contains_word(&compact, "cap"))
}

fn contains_word(text: &str, word: &str) -> bool {
    text.split(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
        .any(|token| token == word)
}

fn is_receiver_path_char(ch: char) -> bool {
    ch == '_' || ch == ':' || ch == '.' || ch.is_ascii_alphanumeric()
}
