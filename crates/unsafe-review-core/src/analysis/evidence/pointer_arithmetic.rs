use super::compact_code;

pub(super) fn has_slice_end_pointer_arithmetic_evidence(lower: &str) -> bool {
    let compact = compact_code(lower);
    for line in lower.lines() {
        let line = compact_code(line);
        let Some(after_let) = line.strip_prefix("let") else {
            continue;
        };
        let Some((binding, expr)) = after_let.split_once('=') else {
            continue;
        };
        let Some(slice_expr) = expr.strip_suffix(".as_ptr();") else {
            continue;
        };
        if !binding.is_empty()
            && !slice_expr.is_empty()
            && compact.contains(&format!("{binding}.add({slice_expr}.len())"))
        {
            return true;
        }
    }
    false
}
