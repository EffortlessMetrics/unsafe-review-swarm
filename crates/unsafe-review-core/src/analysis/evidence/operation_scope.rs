use super::{compact_code, strip_block_comments_and_literals};

pub(super) fn code_before_operation(lower: &str, expression: &str) -> Option<String> {
    let compact = compact_code(&strip_block_comments_and_literals(lower));
    let expression = compact_code(&expression.to_ascii_lowercase());
    if expression.is_empty() {
        return None;
    }
    compact
        .find(&expression)
        .map(|operation_pos| compact[..operation_pos].to_string())
}

#[cfg(test)]
mod tests {
    use super::code_before_operation;

    #[test]
    fn ignores_comments_and_literals_when_locating_operation() {
        let before = code_before_operation(
            r#"
            // unsafe { ptr.read() }
            let _note = "unsafe { ptr.read() }";
            assert!(idx < len);
            unsafe { ptr.read() }
            "#,
            "unsafe { ptr.read() }",
        )
        .expect("operation should be found");

        assert!(before.contains("assert!(idx<len);"));
    }
}
