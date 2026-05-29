use crate::domain::OperationFamily;

pub(super) fn zeroed_operation_family(line: &str) -> Option<OperationFamily> {
    contains_call_name(line, "zeroed").then_some(OperationFamily::Zeroed)
}

fn contains_call_name(line: &str, name: &str) -> bool {
    let mut cursor = line;
    while let Some(pos) = cursor.find(name) {
        let before = cursor[..pos].chars().next_back();
        let after = &cursor[pos + name.len()..];
        let starts_on_boundary = before.is_none_or(|ch| !is_ident_continue(ch));
        if starts_on_boundary && call_suffix(after) {
            return true;
        }
        cursor = &after[after
            .char_indices()
            .next()
            .map_or(after.len(), |(idx, ch)| idx + ch.len_utf8())..];
    }
    false
}

fn call_suffix(after_name: &str) -> bool {
    let rest = after_name.trim_start();
    if rest.starts_with('(') {
        return true;
    }
    rest.strip_prefix("::")
        .is_some_and(|after_colons| after_colons.trim_start().starts_with('<'))
}

fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_zeroed_calls() {
        assert_eq!(
            zeroed_operation_family("unsafe { core::mem::zeroed::<u32>() }"),
            Some(OperationFamily::Zeroed)
        );
    }

    #[test]
    fn ignores_identifier_suffix_matches() {
        assert_eq!(
            zeroed_operation_family("unsafe { core::mem::not_zeroed::<u32>() }"),
            None
        );
    }
}
