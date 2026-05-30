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

pub(super) fn source_before_operation(lower: &str, expression: &str) -> Option<String> {
    let cleaned = strip_block_comments_and_literals(lower);
    let expression = compact_code(&expression.to_ascii_lowercase());
    if expression.is_empty() {
        return None;
    }

    let mut compact = String::with_capacity(cleaned.len());
    let mut source_offsets = Vec::new();
    for (idx, ch) in cleaned.char_indices() {
        if !ch.is_ascii_whitespace() {
            compact.push(ch);
            source_offsets.push(idx);
        }
    }

    compact
        .find(&expression)
        .map(|operation_pos| cleaned[..source_offsets[operation_pos]].to_string())
}

#[cfg(test)]
mod tests {
    use super::{code_before_operation, source_before_operation};

    #[test]
    fn ignores_comments_and_literals_when_locating_operation() -> Result<(), String> {
        let before = code_before_operation(
            r#"
            // unsafe { ptr.read() }
            let _note = "unsafe { ptr.read() }";
            assert!(idx < len);
            unsafe { ptr.read() }
            "#,
            "unsafe { ptr.read() }",
        )
        .ok_or_else(|| "operation should be found".to_string())?;

        assert!(before.contains("assert!(idx<len);"));
        Ok(())
    }

    #[test]
    fn source_before_operation_preserves_binding_whitespace() -> Result<(), String> {
        let before = source_before_operation(
            r#"
            let mut slot: MaybeUninit<u32> = MaybeUninit::<u32>::new(7);
            unsafe { slot.assume_init_read() }
            "#,
            "unsafe { slot.assume_init_read() }",
        )
        .ok_or_else(|| "operation should be found".to_string())?;

        assert!(before.contains("let mut slot: MaybeUninit<u32>"));
        assert!(!before.contains("letmutslot"));
        Ok(())
    }
}
