pub(super) fn condition_has_top_level_conjunct(condition: &str, predicate: &str) -> bool {
    let condition = strip_balanced_outer_parens(condition.trim());
    split_top_level_conditions(condition, b'&')
        .into_iter()
        .any(|conjunct| strip_balanced_outer_parens(conjunct.trim()) == predicate)
}

pub(super) fn condition_has_top_level_disjunct(condition: &str, predicate: &str) -> bool {
    let condition = strip_balanced_outer_parens(condition.trim());
    split_top_level_conditions(condition, b'|')
        .into_iter()
        .any(|conjunct| strip_balanced_outer_parens(conjunct.trim()) == predicate)
}

fn split_top_level_conditions(condition: &str, operator: u8) -> Vec<&str> {
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
