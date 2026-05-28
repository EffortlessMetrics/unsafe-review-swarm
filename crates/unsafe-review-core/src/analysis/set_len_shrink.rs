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
    set_len_argument == "last_index"
        && has_last_index_minus_one_assignment(before_call, receiver)
        && has_non_empty_or_empty_guard(before_call, receiver)
}

fn has_last_index_minus_one_assignment(before_call: &str, receiver: &str) -> bool {
    before_call.contains(&format!("last_index={receiver}.len-1"))
        || before_call.contains(&format!("last_index={receiver}.len()-1"))
}

fn has_non_empty_or_empty_guard(before_call: &str, receiver: &str) -> bool {
    before_call.contains(&format!("{receiver}.len==0"))
        || before_call.contains(&format!("{receiver}.len()==0"))
        || before_call.contains(&format!("{receiver}.len>0"))
        || before_call.contains(&format!("{receiver}.len()>0"))
        || before_call.contains(&format!("!{receiver}.is_empty()"))
}

fn detects_start_bounded_shrink(before_call: &str, receiver: &str, set_len_argument: &str) -> bool {
    set_len_argument == "start"
        && has_start_bounds(before_call)
        && has_len_binding(before_call, receiver)
}

fn has_start_bounds(before_call: &str) -> bool {
    before_call.contains("start<=len")
        || (before_call.contains("start<=end") && before_call.contains("end<=len"))
}

fn has_len_binding(before_call: &str, receiver: &str) -> bool {
    before_call.contains(&format!("len={receiver}.len()"))
        || before_call.contains(&format!("letlen={receiver}.len()"))
}

fn detects_new_len_shrink(before_call: &str, receiver: &str, set_len_argument: &str) -> bool {
    set_len_argument == "new_len"
        && (new_len_checked_against_len(before_call, receiver)
            || new_len_derived_from_subtraction(before_call, receiver)
            || new_len_from_bound_len(before_call, receiver))
}

fn new_len_checked_against_len(before_call: &str, receiver: &str) -> bool {
    (before_call.contains("new_len<=") || before_call.contains("new_len<"))
        && before_call.contains(&format!("{receiver}.len()"))
}

fn new_len_derived_from_subtraction(before_call: &str, receiver: &str) -> bool {
    before_call.contains(&format!("new_len={receiver}.len()-"))
}

fn new_len_from_bound_len(before_call: &str, receiver: &str) -> bool {
    has_len_binding(before_call, receiver) && before_call.contains("new_len=len-")
}
