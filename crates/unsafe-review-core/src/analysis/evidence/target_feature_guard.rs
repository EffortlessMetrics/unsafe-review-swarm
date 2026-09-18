use super::{is_receiver_path_char, is_runtime_assert_at, matching_call_argument_end};

/// Whether a dominating `is_x86_feature_detected!` check covers every target
/// feature the called function requires.
///
/// The canonical SIMD gating shape checks the feature before calling a
/// `#[target_feature]`-gated function. Credit requires linking the check to
/// the callee: the detected feature names must cover the callee's `enable`
/// set, looked up from a same-window `#[target_feature]` declaration for the
/// called name. Callees declared outside the context window (other files,
/// far-away items) stay uncredited. The aarch64 macro and aliased booleans
/// are out of scope for this first cut.
pub(super) fn has_target_feature_detection_evidence(
    expression: &str,
    lower: &str,
    snippet_offset: usize,
) -> bool {
    let Some(callee) = call_callee_name(expression) else {
        return false;
    };
    let compact = compact_preserving_literals(lower);
    let Some(call_pos) = own_call_position(&compact, &callee, snippet_offset) else {
        return false;
    };
    let before_call = &compact[..call_pos];
    let Some(required) = callee_required_features(&compact, &callee) else {
        return false;
    };
    required
        .iter()
        .all(|feature| has_dominating_detection(before_call, feature))
}

/// Final path segment of the called expression: `sum_avx2` from
/// `unsafe { sum_avx2(ptr) }`, `sum_avx2(ptr)`, `path::sum_avx2(ptr)`, or
/// `obj.sum_avx2(ptr)`. Deeper nesting (e.g. a call inside a `let` binding)
/// stays uncredited without dataflow.
fn call_callee_name(expression: &str) -> Option<String> {
    let mut text = expression.trim();
    if let Some(after_unsafe) = text.strip_prefix("unsafe") {
        let after_ws = after_unsafe.trim_start();
        text = after_ws
            .strip_prefix('{')
            .map(str::trim)
            .unwrap_or(after_ws);
    }
    let before_paren = text.split('(').next()?;
    let after_path = before_paren.rsplit("::").next()?;
    let name = after_path.rsplit('.').next()?.trim();
    if name.is_empty()
        || !name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return None;
    }
    Some(name.to_ascii_lowercase())
}

/// Lowercase, comment-free, whitespace-free text that keeps string literals
/// intact: feature names live inside `"..."`, which the standard evidence
/// pipeline empties.
fn compact_preserving_literals(lower: &str) -> String {
    let mut output = String::with_capacity(lower.len());
    let mut chars = lower.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            let mut prev = '\0';
            for comment_ch in chars.by_ref() {
                if prev == '*' && comment_ch == '/' {
                    break;
                }
                prev = comment_ch;
            }
            continue;
        }
        if ch == '/' && chars.peek() == Some(&'/') {
            for comment_ch in chars.by_ref() {
                if comment_ch == '\n' {
                    output.push('\n');
                    break;
                }
            }
            continue;
        }
        output.push(ch);
    }
    output
        .to_ascii_lowercase()
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect()
}

/// Position of the site's own `<callee>(` call: the first marker at or after
/// the snippet offset whose preceding char cannot extend an identifier.
/// Earlier same-name calls belong to other sites sharing the window. Falls
/// back to the first marker when the snippet holds none.
fn own_call_position(compact: &str, callee: &str, snippet_offset: usize) -> Option<usize> {
    let marker = format!("{callee}(");
    let mut search_from = 0usize;
    let mut first = None;
    while let Some(offset) = compact[search_from..].find(&marker) {
        let abs_pos = search_from + offset;
        if first.is_none() {
            first = Some(abs_pos);
        }
        let standalone = abs_pos == 0
            || compact[..abs_pos]
                .chars()
                .next_back()
                .is_none_or(|ch| !is_receiver_path_char(ch));
        if standalone && abs_pos >= snippet_offset {
            return Some(abs_pos);
        }
        search_from = abs_pos + marker.len();
    }
    first
}

/// Feature names from the `#[target_feature(enable = "...")]` declaration
/// whose `fn <callee>(` follows the attribute: every `"..."` literal inside
/// the attribute parens, comma-split. Returns `None` when no same-window
/// declaration for the callee exists.
fn callee_required_features(compact: &str, callee: &str) -> Option<Vec<String>> {
    let mut search_from = 0usize;
    while let Some(offset) = compact[search_from..].find("target_feature(") {
        let abs_pos = search_from + offset;
        let after_marker = &compact[abs_pos + "target_feature(".len()..];
        let Some(arg_end) = matching_call_argument_end(after_marker) else {
            search_from = abs_pos + "target_feature(".len();
            continue;
        };
        let args = &after_marker[..arg_end];
        let after_attr = &after_marker[arg_end..];
        // The attribute must belong to this fn: the first `fn` item after it
        // must be the callee.
        let Some(fn_pos) = after_attr.find("fn") else {
            search_from = abs_pos + "target_feature(".len();
            continue;
        };
        let after_fn =
            after_attr[fn_pos + "fn".len()..].trim_start_matches(['_', ':']);
        if !after_fn.starts_with(&format!("{callee}(")) {
            search_from = abs_pos + "target_feature(".len();
            continue;
        }
        let mut features = Vec::new();
        let mut rest = args;
        while let Some(open) = rest.find('"') {
            let after_open = &rest[open + 1..];
            let Some(close) = after_open.find('"') else {
                break;
            };
            for feature in after_open[..close].split(',') {
                let feature = feature.trim();
                if !feature.is_empty() {
                    features.push(feature.to_string());
                }
            }
            rest = &after_open[close + 1..];
        }
        if features.is_empty() {
            search_from = abs_pos + "target_feature(".len();
            continue;
        }
        return Some(features);
    }
    None
}

/// A runtime `is_x86_feature_detected!("feature")` check dominating the call:
/// open `if` branch, early `return` on the negated check, or a plain
/// `assert!`. Only the x86 macro in this cut.
fn has_dominating_detection(before_call: &str, feature: &str) -> bool {
    let check = format!("is_x86_feature_detected!(\"{feature}\")");
    let mut search_from = 0usize;
    while let Some(offset) = before_call[search_from..].find(&check) {
        let abs_pos = search_from + offset;
        let after_check = &before_call[abs_pos + check.len()..];
        if is_open_branch_guard(before_call, abs_pos, after_check)
            || is_negated_early_return(before_call, abs_pos)
            || is_assert_guard(before_call, abs_pos)
        {
            return true;
        }
        search_from = abs_pos + check.len();
    }
    false
}

/// `if ... check ... {` never closed before the call: the call executes only
/// when the check held.
fn is_open_branch_guard(before_call: &str, check_pos: usize, after_check: &str) -> bool {
    let Some(brace) = after_check.find('{') else {
        return false;
    };
    // The `{` must open the branch, not follow an unrelated statement: no
    // `}` or `;` between the check and the brace.
    let between = &after_check[..brace];
    if between.contains('}') || between.contains(';') {
        return false;
    }
    // The `if` keyword must precede the check without an intervening `;`.
    if !before_call[..check_pos]
        .rsplit(';')
        .next()
        .is_some_and(|s| s.contains("if"))
    {
        return false;
    }
    let mut depth = 0i32;
    for ch in after_check[brace..].chars() {
        if ch == '{' {
            depth += 1;
        } else if ch == '}' {
            depth -= 1;
            if depth == 0 {
                return false;
            }
        }
    }
    true
}

/// `if !check { return ...; }`: the negated check exits before the call.
fn is_negated_early_return(before_call: &str, check_pos: usize) -> bool {
    let head = before_call[..check_pos].rsplit(';').next().unwrap_or("");
    let Some(if_pos) = head.rfind("if") else {
        return false;
    };
    let condition = &head[if_pos + "if".len()..];
    if !condition.contains('!') {
        return false;
    }
    let after_check = &before_call[check_pos..];
    let Some(brace) = after_check.find('{') else {
        return false;
    };
    after_check[brace..].contains("return")
}

/// `assert!(check)` / `assert!(check, ...)` as a statement before the call.
/// Only plain `assert!` counts; `debug_assert!` is compiled out in release.
fn is_assert_guard(before_call: &str, check_pos: usize) -> bool {
    let stmt_start = before_call[..check_pos].rfind(';').map_or(0, |p| p + 1);
    let stmt = before_call[stmt_start..].trim_start();
    let Some(rel_pos) = stmt.find("assert!(") else {
        return false;
    };
    stmt[rel_pos..].starts_with("assert!(")
        && is_runtime_assert_at(before_call, stmt_start + rel_pos)
}
