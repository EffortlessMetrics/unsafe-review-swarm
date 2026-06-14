use super::{
    any_compact_if_condition, branch_still_open_at_operation, compact_code,
    condition_has_top_level_conjunct, condition_has_top_level_disjunct,
    contains_assignment_to_target, contains_executable_return, is_runtime_assert_at,
    matching_call_argument_end, matching_code_block_end, receiver_before_marker,
    split_top_level_arguments, strip_block_comments_and_literals,
};

pub(super) fn has_copy_slice_range_evidence(expression: &str, before_call: &str) -> bool {
    CopyRangeApplicability::from_expression(expression)
        .is_some_and(|context| context.has_applicable_range_evidence(before_call))
}

struct CopyRangeApplicability {
    same_source_slice: CopyRangeBoundTarget,
    same_destination_slice: CopyRangeBoundTarget,
}

impl CopyRangeApplicability {
    fn from_expression(expression: &str) -> Option<Self> {
        let (src, dst, count) = copy_call_arguments(expression)?;
        let src_receiver = copy_source_slice_receiver(&src)?;
        let dst_receiver = copy_destination_slice_receiver(&dst)?;

        Some(Self {
            same_source_slice: CopyRangeBoundTarget::new(&src_receiver, &count)?,
            same_destination_slice: CopyRangeBoundTarget::new(&dst_receiver, &count)?,
        })
    }

    fn has_applicable_range_evidence(&self, before_call: &str) -> bool {
        self.same_source_slice.has_bound_guard(before_call)
            && self.same_destination_slice.has_bound_guard(before_call)
    }
}

struct CopyRangeBoundTarget {
    slice_receiver: String,
    count: String,
    count_lte_len: String,
    len_gte_count: String,
    count_gt_len: String,
    len_lt_count: String,
}

impl CopyRangeBoundTarget {
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
            slice_receiver: receiver,
            count,
        })
    }

    fn has_bound_guard(&self, before_call: &str) -> bool {
        has_slice_count_bound_predicate(before_call, &self.count_lte_len, self)
            || has_slice_count_bound_predicate(before_call, &self.len_gte_count, self)
            || has_slice_count_early_return(before_call, &self.count_gt_len, self)
            || has_slice_count_early_return(before_call, &self.len_lt_count, self)
    }

    fn remains_fresh_after_guard(&self, after_guard: &str) -> bool {
        !self.has_stale_target_assignment(after_guard)
    }

    fn has_stale_target_assignment(&self, text: &str) -> bool {
        contains_assignment_to_target(text, &self.slice_receiver)
            || contains_assignment_to_target(text, &self.count)
    }
}

fn copy_call_arguments(expression: &str) -> Option<(String, String, String)> {
    let compact = compact_code(&strip_block_comments_and_literals(
        &expression.to_ascii_lowercase(),
    ));
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
    target: &CopyRangeBoundTarget,
) -> bool {
    has_slice_count_assertion_guard(before_call, predicate, target)
        || has_open_slice_count_branch_guard(before_call, predicate, target)
}

fn has_slice_count_assertion_guard(
    before_call: &str,
    predicate: &str,
    target: &CopyRangeBoundTarget,
) -> bool {
    // Only `assert!` is a release-runtime guard; `debug_assert!` is compiled out in release
    // builds and cannot satisfy a runtime bounds obligation.  `is_runtime_assert_at` ensures
    // that `assert!(` found inside `debug_assert!(` is not credited as a runtime guard.
    let prefix = "assert!(";
    let mut search_from = 0;
    while let Some(offset) = before_call[search_from..].find(prefix) {
        let abs_pos = search_from + offset;
        let call_start = abs_pos + prefix.len();
        let after_prefix = &before_call[call_start..];
        let Some(call_end) = matching_call_argument_end(after_prefix) else {
            search_from = call_start;
            continue;
        };
        if is_runtime_assert_at(before_call, abs_pos) {
            let args = split_top_level_arguments(&after_prefix[..call_end]);
            let after_call = &after_prefix[call_end..];
            let statement_end = after_call.find(';').unwrap_or(after_call.len());
            let after_guard = &after_call[statement_end..];
            if args
                .first()
                .is_some_and(|condition| condition_has_top_level_conjunct(condition, predicate))
                && target.remains_fresh_after_guard(after_guard)
            {
                return true;
            }
        }
        search_from = call_start + call_end;
    }
    false
}

fn has_open_slice_count_branch_guard(
    before_call: &str,
    predicate: &str,
    target: &CopyRangeBoundTarget,
) -> bool {
    any_compact_if_condition(before_call, |condition, after_guard| {
        condition_has_top_level_conjunct(condition, predicate)
            && branch_still_open_at_operation(after_guard)
            && target.remains_fresh_after_guard(after_guard)
    })
}

fn has_slice_count_early_return(
    before_call: &str,
    predicate: &str,
    target: &CopyRangeBoundTarget,
) -> bool {
    any_compact_if_condition(before_call, |condition, after_guard| {
        let (guard_body, after_guard_body) = matching_code_block_end(after_guard)
            .map_or((after_guard, ""), |body_end| {
                (&after_guard[..body_end], &after_guard[body_end + 1..])
            });
        condition_has_top_level_disjunct(condition, predicate)
            && contains_executable_return(guard_body)
            && target.remains_fresh_after_guard(after_guard_body)
    })
}
