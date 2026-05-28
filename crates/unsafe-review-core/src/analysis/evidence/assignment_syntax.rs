use super::{is_receiver_path_char, is_simple_identifier};

pub(super) fn contains_simple_assignment_to(compact: &str, name: &str) -> bool {
    if !is_simple_identifier(name) {
        return false;
    }
    if compact.contains(&format!("let{name}="))
        || compact.contains(&format!("letmut{name}="))
        || compact.contains(&format!("let{name}:"))
        || compact.contains(&format!("letmut{name}:"))
    {
        return true;
    }
    let marker = name;
    let mut cursor = compact;
    let mut offset = 0usize;
    while let Some(pos) = cursor.find(&marker) {
        let start = offset + pos;
        let after_name_start = start + marker.len();
        let before = compact[..start].chars().next_back();
        let after_name = &compact[after_name_start..];
        if before.is_none_or(|ch| !is_receiver_path_char(ch))
            && starts_assignment_operator(after_name)
        {
            return true;
        }
        let next = pos + marker.len();
        offset += next;
        cursor = &cursor[next..];
    }
    false
}

fn starts_assignment_operator(value: &str) -> bool {
    value.starts_with("<<=")
        || value.starts_with(">>=")
        || value.starts_with("+=")
        || value.starts_with("-=")
        || value.starts_with("*=")
        || value.starts_with("/=")
        || value.starts_with("%=")
        || value.starts_with("&=")
        || value.starts_with("|=")
        || value.starts_with("^=")
        || (value.starts_with('=') && !value.starts_with("==") && !value.starts_with("=>"))
}

#[cfg(test)]
mod tests {
    use super::contains_simple_assignment_to;

    #[test]
    fn detects_plain_let_and_compound_assignments() {
        for code in [
            "letindex=values.len();",
            "letmutindex=0;",
            "index=values.len();",
            "index+=1;",
            "index-=1;",
            "index<<=1;",
            "index>>=1;",
        ] {
            assert!(
                contains_simple_assignment_to(code, "index"),
                "{code} should count as an assignment to index"
            );
        }
    }

    #[test]
    fn ignores_comparisons_and_other_targets() {
        for code in [
            "index==values.len()",
            "index=>fallback",
            "otherindex+=1;",
            "value.index+=1;",
            "other.index=1;",
        ] {
            assert!(
                !contains_simple_assignment_to(code, "index"),
                "{code} should not count as an assignment to index"
            );
        }
    }
}
