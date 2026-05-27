use super::{
    branch_still_open_at_operation, compact_code, compact_contains_identifier,
    contains_simple_assignment_to, is_receiver_path_char, matching_call_argument_end,
    matching_code_block_end, receiver_before_marker, split_top_level_arguments,
    strip_block_comments_and_literals,
};

pub(super) fn has_copy_slice_range_evidence(expression: &str, before_call: &str) -> bool {
    CopyRangeApplicability::from_expression(expression)
        .is_some_and(|context| context.has_applicable_range_evidence(before_call))
}

struct CopyRangeApplicability {
    same_source_slice: SliceCountBoundTarget,
    same_destination_slice: SliceCountBoundTarget,
}

impl CopyRangeApplicability {
    fn from_expression(expression: &str) -> Option<Self> {
        let (src, dst, count) = copy_call_arguments(expression)?;
        let src_receiver = copy_source_slice_receiver(&src)?;
        let dst_receiver = copy_destination_slice_receiver(&dst)?;

        Some(Self {
            same_source_slice: SliceCountBoundTarget::new(&src_receiver, &count)?,
            same_destination_slice: SliceCountBoundTarget::new(&dst_receiver, &count)?,
        })
    }

    fn has_applicable_range_evidence(&self, before_call: &str) -> bool {
        self.same_source_slice.has_bound_guard(before_call)
            && self.same_destination_slice.has_bound_guard(before_call)
    }
}

struct SliceCountBoundTarget {
    receiver: String,
    count: String,
    count_lte_len: String,
    len_gte_count: String,
    count_gt_len: String,
    len_lt_count: String,
}

impl SliceCountBoundTarget {
    fn new(receiver: &str, count: &str) -> Option<Self> {
        let receiver = compact_code(receiver);
        let count = compact_code(count);
        if receiver.is_empty() || count.is_empty() {
            return None;
        }

        let len = format!("{receiver}.len()");
        Some(Self {
            count_lte_len: format!("{count}<={len}"),
            len_gte_count: format!("{len}>={count}"),
            count_gt_len: format!("{count}>{len}"),
            len_lt_count: format!("{len}<{count}"),
            receiver,
            count,
        })
    }

    fn has_bound_guard(&self, before_call: &str) -> bool {
        has_slice_count_bound_predicate(
            before_call,
            &self.count_lte_len,
            &self.receiver,
            &self.count,
        ) || has_slice_count_bound_predicate(
            before_call,
            &self.len_gte_count,
            &self.receiver,
            &self.count,
        ) || has_slice_count_early_return(
            before_call,
            &self.count_gt_len,
            &self.receiver,
            &self.count,
        ) || has_slice_count_early_return(
            before_call,
            &self.len_lt_count,
            &self.receiver,
            &self.count,
        )
    }
}

fn copy_call_arguments(expression: &str) -> Option<(String, String, String)> {
    let compact = compact_code(&expression.to_ascii_lowercase());
    for marker in ["copy_nonoverlapping(", "ptr::copy("] {
        let Some(call_pos) = compact.find(marker) else {
            continue;
        };
        let after_marker = &compact[call_pos + marker.len()..];
        let Some(end) = matching_call_argument_end(after_marker) else {
            continue;
        };
        let args = split_top_level_arguments(&after_marker[..end]);
        if args.len() == 3 && args.iter().all(|arg| !arg.is_empty()) {
            return Some((
                args[0].to_string(),
                args[1].to_string(),
                args[2].to_string(),
            ));
        }
    }
    None
}

fn copy_source_slice_receiver(argument: &str) -> Option<String> {
    receiver_before_marker(argument, ".as_ptr()").map(str::to_string)
}

fn copy_destination_slice_receiver(argument: &str) -> Option<String> {
    receiver_before_marker(argument, ".as_mut_ptr()").map(str::to_string)
}

fn has_slice_count_bound_predicate(
    before_call: &str,
    predicate: &str,
    receiver: &str,
    count: &str,
) -> bool {
    has_slice_count_assertion_guard(before_call, predicate, receiver, count)
        || has_open_slice_count_branch_guard(before_call, predicate, receiver, count)
}

fn has_slice_count_assertion_guard(
    before_call: &str,
    predicate: &str,
    receiver: &str,
    count: &str,
) -> bool {
    ["assert!(", "debug_assert!("].into_iter().any(|prefix| {
        let mut search_from = 0;
        while let Some(offset) = before_call[search_from..].find(prefix) {
            let call_start = search_from + offset + prefix.len();
            let after_prefix = &before_call[call_start..];
            let Some(call_end) = matching_call_argument_end(after_prefix) else {
                search_from = call_start;
                continue;
            };
            let args = split_top_level_arguments(&after_prefix[..call_end]);
            let after_call = &after_prefix[call_end..];
            let statement_end = after_call.find(';').unwrap_or(after_call.len());
            let after_guard = &after_call[statement_end..];
            if args
                .first()
                .is_some_and(|condition| condition_has_top_level_conjunct(condition, predicate))
                && !has_slice_count_assignment(after_guard, receiver, count)
            {
                return true;
            }
            search_from = call_start + call_end;
        }
        false
    })
}

fn has_open_slice_count_branch_guard(
    before_call: &str,
    predicate: &str,
    receiver: &str,
    count: &str,
) -> bool {
    let mut search_from = 0;
    while let Some(offset) = before_call[search_from..].find("if") {
        let guard_start = search_from + offset;
        let before = before_call[..guard_start].chars().next_back();
        if before.is_some_and(is_receiver_path_char) {
            search_from = guard_start + 2;
            continue;
        }
        let after_if = &before_call[guard_start + 2..];
        if let Some(brace_pos) = after_if.find('{') {
            let condition = &after_if[..brace_pos];
            let after_guard = &after_if[brace_pos + 1..];
            if condition_has_top_level_conjunct(condition, predicate)
                && branch_still_open_at_operation(after_guard)
                && !has_slice_count_assignment(after_guard, receiver, count)
            {
                return true;
            }
        }
        search_from = guard_start + 2;
    }
    false
}

fn condition_has_top_level_conjunct(condition: &str, predicate: &str) -> bool {
    let condition = strip_balanced_outer_parens(condition.trim());
    split_top_level_conjuncts(condition)
        .into_iter()
        .any(|conjunct| strip_balanced_outer_parens(conjunct.trim()) == predicate)
}

fn condition_has_top_level_disjunct(condition: &str, predicate: &str) -> bool {
    let condition = strip_balanced_outer_parens(condition.trim());
    split_top_level_disjuncts(condition)
        .into_iter()
        .any(|disjunct| strip_balanced_outer_parens(disjunct.trim()) == predicate)
}

fn split_top_level_conjuncts(condition: &str) -> Vec<&str> {
    split_top_level_condition_operands(condition, b'&')
}

fn split_top_level_disjuncts(condition: &str) -> Vec<&str> {
    split_top_level_condition_operands(condition, b'|')
}

fn split_top_level_condition_operands(condition: &str, operator: u8) -> Vec<&str> {
    let mut conjuncts = Vec::new();
    let mut start = 0usize;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;
    let bytes = condition.as_bytes();
    let mut idx = 0usize;
    while idx < bytes.len() {
        match bytes[idx] {
            b'(' => paren_depth += 1,
            b')' => paren_depth = paren_depth.saturating_sub(1),
            b'[' => bracket_depth += 1,
            b']' => bracket_depth = bracket_depth.saturating_sub(1),
            b'{' => brace_depth += 1,
            b'}' => brace_depth = brace_depth.saturating_sub(1),
            byte if byte == operator
                && idx + 1 < bytes.len()
                && bytes[idx + 1] == operator
                && paren_depth == 0
                && bracket_depth == 0
                && brace_depth == 0 =>
            {
                conjuncts.push(condition[start..idx].trim());
                idx += 2;
                start = idx;
                continue;
            }
            _ => {}
        }
        idx += 1;
    }
    conjuncts.push(condition[start..].trim());
    conjuncts
}

fn strip_balanced_outer_parens(mut text: &str) -> &str {
    loop {
        let Some(inner) = text
            .strip_prefix('(')
            .and_then(|inner| inner.strip_suffix(')'))
        else {
            return text;
        };
        if !outer_parens_enclose_whole_expression(text) {
            return text;
        }
        text = inner.trim();
    }
}

fn outer_parens_enclose_whole_expression(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.first() != Some(&b'(') || bytes.last() != Some(&b')') {
        return false;
    }
    let mut depth = 0usize;
    for (idx, byte) in bytes.iter().enumerate() {
        match byte {
            b'(' => depth += 1,
            b')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 && idx != bytes.len() - 1 {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0
}

fn has_slice_count_early_return(
    before_call: &str,
    predicate: &str,
    receiver: &str,
    count: &str,
) -> bool {
    let mut search_from = 0;
    while let Some(offset) = before_call[search_from..].find("if") {
        let guard_start = search_from + offset;
        let before = before_call[..guard_start].chars().next_back();
        if before.is_some_and(is_receiver_path_char) {
            search_from = guard_start + 2;
            continue;
        }
        let after_if = &before_call[guard_start + 2..];
        if let Some(brace_pos) = after_if.find('{') {
            let condition = &after_if[..brace_pos];
            let after_guard = &after_if[brace_pos + 1..];
            let (guard_body, after_guard_body) = matching_code_block_end(after_guard)
                .map_or((after_guard, ""), |body_end| {
                    (&after_guard[..body_end], &after_guard[body_end + 1..])
                });
            if condition_has_top_level_disjunct(condition, predicate)
                && guard_body_contains_return(guard_body)
                && !has_slice_count_assignment(after_guard_body, receiver, count)
            {
                return true;
            }
        }
        search_from = guard_start + 2;
    }
    false
}

fn guard_body_contains_return(guard_body: &str) -> bool {
    let code = strip_block_comments_and_literals(guard_body);
    compact_contains_identifier(&code, "return")
}

fn has_slice_count_assignment(compact: &str, receiver: &str, count: &str) -> bool {
    contains_simple_assignment_to(compact, receiver)
        || contains_simple_assignment_to(compact, count)
}
