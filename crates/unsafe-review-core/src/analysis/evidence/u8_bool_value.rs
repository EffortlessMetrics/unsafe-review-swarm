use super::{
    has_assignment_to_identifier, has_fresh_guard_pattern,
    has_open_positive_branch_guard_for_identifiers,
};

pub(super) fn has_u8_bool_value_guard(before_call: &str, argument: &str) -> bool {
    u8_bool_valid_value_predicates(argument)
        .iter()
        .any(|predicate| has_u8_bool_value_predicate_guard(before_call, predicate, argument))
        || has_u8_bool_invalid_early_return_guard(before_call, argument)
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

fn has_u8_bool_value_predicate_guard(before_call: &str, predicate: &str, argument: &str) -> bool {
    [
        format!("assert!({predicate})"),
        format!("assert!({predicate},"),
        format!("debug_assert!({predicate})"),
        format!("debug_assert!({predicate},"),
    ]
    .iter()
    .any(|pattern| has_fresh_guard_pattern(before_call, pattern, argument))
        || has_open_positive_branch_guard(before_call, predicate, argument)
}

fn has_open_positive_branch_guard(before_call: &str, predicate: &str, argument: &str) -> bool {
    has_open_positive_branch_guard_for_identifiers(before_call, predicate, &[argument])
}

fn has_u8_bool_invalid_early_return_guard(before_call: &str, argument: &str) -> bool {
    has_invalid_byte_returning_branch(before_call, &format!("{argument}>1"), argument)
        || has_invalid_byte_returning_branch(before_call, &format!("1<{argument}"), argument)
        || has_invalid_byte_returning_branch(before_call, &format!("{argument}>=2"), argument)
        || has_invalid_byte_returning_branch(before_call, &format!("2<={argument}"), argument)
}

fn has_invalid_byte_returning_branch(before_call: &str, predicate: &str, argument: &str) -> bool {
    let guard = format!("if{predicate}{{");
    let mut search_from = 0;
    while let Some(offset) = before_call[search_from..].find(&guard) {
        let guard_start = search_from + offset;
        let after_guard = &before_call[guard_start + guard.len()..];
        let guard_end = after_guard.find('}').unwrap_or(after_guard.len());
        let guard_body = &after_guard[..guard_end];
        let after_branch = &after_guard[guard_end..];
        if guard_body.contains("return") && !has_assignment_to_identifier(after_branch, argument) {
            return true;
        }
        search_from = guard_start + guard.len();
    }
    false
}
