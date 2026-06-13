use super::evidence::contains_simple_assignment_to;

pub(super) fn has_set_len_shrink_evidence(
    before_call: &str,
    receiver: &str,
    set_len_argument: &str,
) -> bool {
    detects_zero_length_shrink(set_len_argument)
        || detects_last_index_shrink(before_call, receiver, set_len_argument)
        || detects_start_bounded_shrink(before_call, receiver, set_len_argument)
        || detects_new_len_shrink(before_call, receiver, set_len_argument)
}

fn detects_zero_length_shrink(set_len_argument: &str) -> bool {
    set_len_argument == "0"
}

fn detects_last_index_shrink(before_call: &str, receiver: &str, set_len_argument: &str) -> bool {
    set_len_argument == "last_index" && has_fresh_last_index_shrink(before_call, receiver)
}

fn has_last_index_minus_one_assignment(before_call: &str, receiver: &str) -> bool {
    before_call.contains(&format!("last_index={receiver}.len-1"))
        || before_call.contains(&format!("last_index={receiver}.len()-1"))
}

fn has_fresh_last_index_shrink(before_call: &str, receiver: &str) -> bool {
    if !has_last_index_minus_one_assignment(before_call, receiver)
        || !has_non_empty_or_empty_guard(before_call, receiver)
    {
        return false;
    }

    non_empty_or_empty_guard_tails(before_call, receiver)
        .into_iter()
        .filter(|after_guard| !contains_simple_assignment_to(after_guard, receiver))
        .any(|_after_guard| {
            last_index_assignment_tails(before_call, receiver)
                .into_iter()
                .any(|after_assignment| {
                    ![receiver, "last_index"].iter().any(|identifier| {
                        contains_simple_assignment_to(after_assignment, identifier)
                    })
                })
        })
}

fn last_index_assignment_tails<'a>(before_call: &'a str, receiver: &str) -> Vec<&'a str> {
    [
        format!("last_index={receiver}.len-1"),
        format!("last_index={receiver}.len()-1"),
    ]
    .into_iter()
    .flat_map(|marker| marker_tails(before_call, &marker))
    .collect()
}

fn has_non_empty_or_empty_guard(before_call: &str, receiver: &str) -> bool {
    !non_empty_or_empty_guard_tails(before_call, receiver).is_empty()
}

fn non_empty_or_empty_guard_tails<'a>(before_call: &'a str, receiver: &str) -> Vec<&'a str> {
    [
        format!("{receiver}.len==0"),
        format!("{receiver}.len()==0"),
        format!("{receiver}.len>0"),
        format!("{receiver}.len()>0"),
        format!("!{receiver}.is_empty()"),
    ]
    .into_iter()
    .flat_map(|marker| marker_tails(before_call, &marker))
    .collect()
}

fn marker_tails<'a>(text: &'a str, marker: &str) -> Vec<&'a str> {
    let mut tails = Vec::new();
    let mut search_from = 0usize;
    while let Some(offset) = text[search_from..].find(marker) {
        let marker_end = search_from + offset + marker.len();
        tails.push(&text[marker_end..]);
        search_from = marker_end;
    }
    tails
}

fn detects_start_bounded_shrink(before_call: &str, receiver: &str, set_len_argument: &str) -> bool {
    set_len_argument == "start" && has_fresh_start_bound_len_binding(before_call, receiver)
}

fn has_start_bounds(before_call: &str) -> bool {
    before_call.contains("start<=len")
        || (before_call.contains("start<=end") && before_call.contains("end<=len"))
}

fn has_len_binding(before_call: &str, receiver: &str) -> bool {
    before_call.contains(&format!("len={receiver}.len()"))
        || before_call.contains(&format!("letlen={receiver}.len()"))
}

fn has_fresh_start_bound_len_binding(before_call: &str, receiver: &str) -> bool {
    for marker in [
        format!("len={receiver}.len()"),
        format!("letlen={receiver}.len()"),
    ] {
        let mut search_from = 0usize;
        while let Some(offset) = before_call[search_from..].find(&marker) {
            let binding_end = search_from + offset + marker.len();
            let after_binding = &before_call[binding_end..];
            if has_start_bounds(after_binding)
                && !["start", "end", "len", receiver]
                    .iter()
                    .any(|identifier| contains_simple_assignment_to(after_binding, identifier))
            {
                return true;
            }
            search_from = binding_end;
        }
    }
    false
}

fn detects_new_len_shrink(before_call: &str, receiver: &str, set_len_argument: &str) -> bool {
    set_len_argument == "new_len"
        && (new_len_checked_against_len(before_call, receiver)
            || new_len_derived_from_subtraction(before_call, receiver)
            || new_len_from_bound_len(before_call, receiver))
}

fn new_len_checked_against_len(before_call: &str, receiver: &str) -> bool {
    let receiver_len = format!("{receiver}.len()");
    // Require a JOINED predicate that actually bounds new_len against the same
    // receiver's .len(), not an independent occurrence of new_len< anywhere
    // near an unrelated .len() call.  Mirror the discipline used by
    // has_set_len_capacity_relation and has_len_cap_bound_guard.
    let new_len_lte_len = format!("new_len<={receiver_len}");
    let new_len_lt_len = format!("new_len<{receiver_len}");
    let len_gt_new_len = format!("{receiver_len}>new_len");
    let len_gte_new_len = format!("{receiver_len}>=new_len");
    before_call.contains(&new_len_lte_len)
        || before_call.contains(&new_len_lt_len)
        || before_call.contains(&len_gt_new_len)
        || before_call.contains(&len_gte_new_len)
}

fn new_len_derived_from_subtraction(before_call: &str, receiver: &str) -> bool {
    before_call.contains(&format!("new_len={receiver}.len()-"))
}

fn new_len_from_bound_len(before_call: &str, receiver: &str) -> bool {
    has_len_binding(before_call, receiver) && before_call.contains("new_len=len-")
}
