use super::{compact_code, strip_block_comments_and_literals};

pub(super) fn has_slice_end_pointer_arithmetic_evidence(lower: &str) -> bool {
    let lower = strip_block_comments_and_literals(lower);
    let compact = compact_code(&lower);
    lower
        .lines()
        .filter_map(SliceEndPointerArithmeticApplicability::from_line)
        .any(|context| context.has_same_slice_end_pointer(&compact))
}

struct SliceEndPointerArithmeticApplicability {
    pointer_binding: String,
    slice_expr: String,
}

impl SliceEndPointerArithmeticApplicability {
    fn from_line(line: &str) -> Option<Self> {
        let line = compact_code(line);
        let after_let = line.strip_prefix("let")?;
        let (binding, expr) = after_let.split_once('=')?;
        let slice_expr = expr.strip_suffix(".as_ptr();")?;
        (!binding.is_empty() && !slice_expr.is_empty()).then_some(Self {
            pointer_binding: binding.to_string(),
            slice_expr: slice_expr.to_string(),
        })
    }

    fn has_same_slice_end_pointer(&self, compact: &str) -> bool {
        compact.contains(&format!(
            "{}.add({}.len())",
            self.pointer_binding, self.slice_expr
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applicability_uses_same_binding_and_slice_length() -> Result<(), String> {
        let context =
            SliceEndPointerArithmeticApplicability::from_line("let start = haystack.as_ptr();")
                .ok_or_else(|| "start should trace to haystack.as_ptr()".to_string())?;

        assert!(context.has_same_slice_end_pointer(
            "letstart=haystack.as_ptr();letend=start.add(haystack.len());"
        ));
        Ok(())
    }

    #[test]
    fn applicability_rejects_other_pointer_binding() -> Result<(), String> {
        let context =
            SliceEndPointerArithmeticApplicability::from_line("let start = haystack.as_ptr();")
                .ok_or_else(|| "start should trace to haystack.as_ptr()".to_string())?;

        assert!(!context.has_same_slice_end_pointer(
            "letstart=haystack.as_ptr();letend=other.add(haystack.len());"
        ));
        Ok(())
    }

    #[test]
    fn applicability_rejects_other_slice_length() -> Result<(), String> {
        let context =
            SliceEndPointerArithmeticApplicability::from_line("let start = haystack.as_ptr();")
                .ok_or_else(|| "start should trace to haystack.as_ptr()".to_string())?;

        assert!(!context.has_same_slice_end_pointer(
            "letstart=haystack.as_ptr();letend=start.add(other.len());"
        ));
        Ok(())
    }
}
