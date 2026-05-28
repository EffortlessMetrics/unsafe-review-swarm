use super::{is_receiver_path_char, is_simple_identifier};

pub(super) fn has_fresh_guard_pattern(before_call: &str, pattern: &str, argument: &str) -> bool {
    has_fresh_guard_pattern_for_identifiers(before_call, pattern, &[argument])
}

pub(super) fn has_fresh_guard_pattern_for_identifiers(
    before_call: &str,
    pattern: &str,
    identifiers: &[&str],
) -> bool {
    let mut search_from = 0;
    while let Some(offset) = before_call[search_from..].find(pattern) {
        let pattern_start = search_from + offset;
        let after_pattern = &before_call[pattern_start + pattern.len()..];
        let after_guard = if pattern.ends_with(';') {
            after_pattern
        } else {
            let statement_end = after_pattern.find(';').unwrap_or(after_pattern.len());
            &after_pattern[statement_end..]
        };
        if !has_assignment_to_any_identifier(after_guard, identifiers) {
            return true;
        }
        search_from = pattern_start + pattern.len();
    }
    false
}

pub(super) fn has_open_positive_branch_guard_for_identifiers(
    before_call: &str,
    predicate: &str,
    identifiers: &[&str],
) -> bool {
    let guard = format!("if{predicate}{{");
    let mut search_from = 0;
    while let Some(offset) = before_call[search_from..].find(&guard) {
        let guard_start = search_from + offset;
        let after_guard = &before_call[guard_start + guard.len()..];
        let mut depth = 1usize;
        for ch in after_guard.chars() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
        }
        if depth > 0 && !has_assignment_to_any_identifier(after_guard, identifiers) {
            return true;
        }
        search_from = guard_start + guard.len();
    }
    false
}

pub(super) fn has_assignment_to_any_identifier(compact: &str, identifiers: &[&str]) -> bool {
    identifiers
        .iter()
        .any(|identifier| has_assignment_to_identifier(compact, identifier))
}

pub(super) fn has_assignment_to_identifier(compact: &str, identifier: &str) -> bool {
    if is_simple_identifier(identifier)
        && (compact.contains(&format!("let{identifier}="))
            || compact.contains(&format!("letmut{identifier}="))
            || compact.contains(&format!("let{identifier}:"))
            || compact.contains(&format!("letmut{identifier}:")))
    {
        return true;
    }

    let mut cursor = compact;
    let mut offset = 0usize;
    while let Some(pos) = cursor.find(identifier) {
        let start = offset + pos;
        let before = compact[..start].chars().next_back();
        let after_start = start + identifier.len();
        let after = &compact[after_start..];
        let ends_on_boundary = after
            .chars()
            .next()
            .is_none_or(|ch| !is_receiver_path_char(ch));
        if before.is_none_or(|ch| !is_receiver_path_char(ch))
            && ends_on_boundary
            && starts_assignment_operator(after)
        {
            return true;
        }
        let next = pos + identifier.len();
        offset += next;
        cursor = &cursor[next..];
    }
    false
}

fn starts_assignment_operator(after_identifier: &str) -> bool {
    if after_identifier.starts_with("==") || after_identifier.starts_with("=>") {
        return false;
    }
    after_identifier.starts_with('=')
        || ["+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "<<=", ">>="]
            .iter()
            .any(|operator| after_identifier.starts_with(operator))
}

#[cfg(test)]
mod tests {
    use super::has_assignment_to_identifier;

    #[test]
    fn detects_shadowing_bindings_as_assignments() {
        for code in [
            "letnew_len=values.capacity()+1;",
            "letmutnew_len=values.capacity()+1;",
            "letnew_len:usize=values.capacity()+1;",
            "letmutnew_len:usize=values.capacity()+1;",
        ] {
            assert!(
                has_assignment_to_identifier(code, "new_len"),
                "{code} should stale new_len evidence"
            );
        }
    }

    #[test]
    fn ignores_other_binding_names() {
        for code in ["letother_new_len=1;", "letwrappernew_len=1;"] {
            assert!(
                !has_assignment_to_identifier(code, "new_len"),
                "{code} should not stale new_len evidence"
            );
        }
    }
}
