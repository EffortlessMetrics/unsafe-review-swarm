pub(super) fn has_set_len_shrink_evidence(lower: &str) -> bool {
    let compact = compact_code(lower);
    detects_zero_length_shrink(&compact)
        || detects_last_index_shrink(&compact)
        || detects_start_bounded_shrink(&compact)
        || detects_new_len_shrink(&compact)
}

fn compact_code(lower: &str) -> String {
    lower
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace())
        .collect()
}

fn detects_zero_length_shrink(compact: &str) -> bool {
    compact.contains(".set_len(0)")
}

fn detects_last_index_shrink(compact: &str) -> bool {
    compact.contains(".set_len(last_index)")
        && has_last_index_minus_one_assignment(compact)
        && has_non_empty_or_empty_guard(compact)
}

fn has_last_index_minus_one_assignment(compact: &str) -> bool {
    compact.contains("last_index=self.len-1")
        || compact.contains("last_index=self.len()-1")
        || (compact.contains("last_index=")
            && (compact.contains(".len-1") || compact.contains(".len()-1")))
}

fn has_non_empty_or_empty_guard(compact: &str) -> bool {
    compact.contains("self.len==0")
        || compact.contains("self.len()==0")
        || compact.contains(".len==0")
        || compact.contains(".len()==0")
        || compact.contains("self.len>0")
        || compact.contains("self.len()>0")
        || compact.contains("!self.is_empty()")
}

fn detects_start_bounded_shrink(compact: &str) -> bool {
    compact.contains(".set_len(start)") && has_start_bounds(compact) && has_len_binding(compact)
}

fn has_start_bounds(compact: &str) -> bool {
    compact.contains("start<=len")
        || (compact.contains("start<=end") && compact.contains("end<=len"))
}

fn has_len_binding(compact: &str) -> bool {
    compact.contains("len=self.len()")
        || (compact.contains("letlen=") && compact.contains(".len()"))
}

fn detects_new_len_shrink(compact: &str) -> bool {
    compact.contains(".set_len(new_len)")
        && (new_len_checked_against_len(compact)
            || new_len_derived_from_subtraction(compact)
            || new_len_from_bound_len(compact))
}

fn new_len_checked_against_len(compact: &str) -> bool {
    (compact.contains("new_len<=") || compact.contains("new_len<")) && compact.contains(".len()")
}

fn new_len_derived_from_subtraction(compact: &str) -> bool {
    compact.contains("new_len=") && compact.contains(".len()-")
}

fn new_len_from_bound_len(compact: &str) -> bool {
    compact.contains("len=self.len()") && compact.contains("new_len=len-")
}
