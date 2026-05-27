use super::compact_code;

pub(super) fn has_unreachable_unchecked_infallible_path_evidence(lower: &str) -> bool {
    let compact = compact_code(lower);
    let Some(call_pos) = compact.find("unreachable_unchecked(") else {
        return false;
    };
    let before_call = &compact[..call_pos];
    let Some(match_pos) = before_call.rfind("match") else {
        return false;
    };
    let match_context = &before_call[match_pos..];
    let Some((match_head, after_open)) = match_context.split_once('{') else {
        return false;
    };
    if !match_head.contains("fallibility::infallible") {
        return false;
    }

    let mut depth = 1usize;
    for ch in after_open.chars() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}
