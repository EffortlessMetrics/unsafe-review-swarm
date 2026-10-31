use super::unsafe_fn_call::{
    has_encode_utf8_remaining_capacity_evidence, has_unchecked_constructor_availability_evidence,
};
use super::{
    branch_still_open_at_operation, compact_code, is_runtime_assert_at, matching_code_block_end,
};
use crate::analysis::scanner::ScannedSite;
use crate::domain::{EvidenceState, OperationFamily};

pub(super) const TARGET_FEATURE_DETECTED_DISCHARGE: &str =
    "Target-feature detection guard (`is_x86_feature_detected!`) was detected for the callee";

pub(super) fn callee_contract_discharge_state(
    site: &ScannedSite,
    family: &OperationFamily,
    expression: &str,
    lower: &str,
) -> EvidenceState {
    if family == &OperationFamily::UnsafeFnCall
        && has_encode_utf8_remaining_capacity_evidence(lower)
    {
        EvidenceState::present("Unsafe call argument guard code was detected")
    } else if family == &OperationFamily::UnsafeFnCall
        && has_unchecked_constructor_availability_evidence(expression, lower)
    {
        EvidenceState::present("Unchecked constructor availability guard code was detected")
    } else if family == &OperationFamily::UnsafeFnCall && has_target_feature_detected_guard(site) {
        EvidenceState::present(TARGET_FEATURE_DETECTED_DISCHARGE)
    } else {
        EvidenceState::missing("No obligation-specific guard code was detected")
    }
}

/// Remove block comments, line comments, string/char literals, and raw
/// strings so guard matching only sees executable code. The feature
/// literal of an executable `is_x86_feature_detected!("...")` invocation
/// is preserved.
fn strip_non_code(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0usize;
    while i < chars.len() {
        // Block comment (Rust allows nesting).
        if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
            let mut depth = 1usize;
            i += 2;
            while i < chars.len() && depth > 0 {
                if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            continue;
        }
        // Line comment.
        if chars[i] == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        // Raw string: r"..." or r#"..."# (not an identifier ending in r).
        if chars[i] == 'r'
            && (i == 0 || !(chars[i - 1] == '_' || chars[i - 1].is_ascii_alphanumeric()))
        {
            let mut j = i + 1;
            let mut hashes = 0usize;
            while chars.get(j) == Some(&'#') {
                hashes += 1;
                j += 1;
            }
            if chars.get(j) == Some(&'"') {
                j += 1;
                while j < chars.len() {
                    if chars[j] == '"' {
                        let mut k = j + 1;
                        let mut matched = 0usize;
                        while chars.get(k) == Some(&'#') {
                            matched += 1;
                            k += 1;
                        }
                        if matched == hashes {
                            j = k;
                            break;
                        }
                    }
                    j += 1;
                }
                i = j;
                continue;
            }
        }
        // String literal: keep only the feature literal of an executable
        // detection invocation.
        if chars[i] == '"' {
            let keep = out.trim_end().ends_with("is_x86_feature_detected!(");
            out.push('"');
            i += 1;
            let mut escaped = false;
            while i < chars.len() {
                let ch = chars[i];
                if escaped {
                    if keep {
                        out.push(ch);
                    }
                    escaped = false;
                    i += 1;
                    continue;
                }
                if ch == '\\' {
                    escaped = true;
                    i += 1;
                    continue;
                }
                if ch == '"' {
                    i += 1;
                    break;
                }
                if keep {
                    out.push(ch);
                }
                i += 1;
            }
            out.push('"');
            continue;
        }
        // Char literal ('x', '\n', '{'); lifetimes ('a) pass through.
        if chars[i] == '\''
            && i + 2 < chars.len()
            && (chars[i + 2] == '\'' || (chars[i + 1] == '\\' && chars.get(i + 3) == Some(&'\'')))
        {
            i += if chars[i + 1] == '\\' { 4 } else { 3 };
            out.push_str("''");
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn has_target_feature_detected_guard(site: &ScannedSite) -> bool {
    let Some((callee, feature)) = &site.callee_target_feature else {
        return false;
    };
    // Through-site text only: detections in context_after cannot dominate.
    // Case is preserved: the feature literal must match the callee's
    // declared feature exactly.
    let prefix = strip_non_code(&site.context_before.join("\n"));
    let mut text = prefix.clone();
    text.push('\n');
    text.push_str(&strip_non_code(&site.site.snippet));
    let compact = compact_code(&text);
    // Anchor to the site's own call: occurrences in context_before belong
    // to earlier calls and cannot discharge this site.
    let own_call_from = compact_code(&prefix).len();
    let call_pattern = format!("{callee}(");
    let mut search_from = own_call_from;
    while let Some(offset) = compact[search_from..].find(&call_pattern) {
        let call_pos = search_from + offset;
        // Require a bare-call boundary before the callee name.
        let before_ok = compact[..call_pos]
            .chars()
            .next_back()
            .is_none_or(|ch| !(ch == '_' || ch.is_ascii_alphanumeric()) && ch != '.' && ch != ':');
        if before_ok && guard_dominates(&compact[..call_pos], feature) {
            return true;
        }
        search_from = call_pos + call_pattern.len();
    }
    false
}

/// Block-scope chain (unique block ids, outermost first) in effect at
/// byte offset `upto` in compacted code.
fn scope_chain(text: &str, upto: usize) -> Vec<u32> {
    let mut stack: Vec<u32> = Vec::new();
    let mut next_id = 0u32;
    for ch in text[..upto.min(text.len())].chars() {
        match ch {
            '{' => {
                next_id += 1;
                stack.push(next_id);
            }
            '}' => {
                stack.pop();
            }
            _ => {}
        }
    }
    stack
}

fn at_dominating_scope(before_call: &str, guard_pos: usize) -> bool {
    let guard_chain = scope_chain(before_call, guard_pos);
    let call_chain = scope_chain(before_call, before_call.len());
    call_chain.starts_with(&guard_chain)
}

/// True when the guard block body unconditionally returns from the
/// enclosing function: a `return` token at the guard body's own block
/// level (not nested in any inner block, closure, or `fn`).
fn guard_block_returns(body: &str) -> bool {
    let mut depth = 0usize;
    let bytes = body.as_bytes();
    let mut i = 0usize;
    while i < body.len() {
        match bytes[i] {
            b'{' => {
                depth += 1;
                i += 1;
            }
            b'}' => {
                depth = depth.saturating_sub(1);
                i += 1;
            }
            _ if depth == 0 && body[i..].starts_with("return") => {
                let before_ok =
                    i == 0 || !(bytes[i - 1] == b'_' || bytes[i - 1].is_ascii_alphanumeric());
                if before_ok {
                    // Compacted code merges `return` with its value
                    // (`return0;`, `returnNone;`, `returnErr(..)`).
                    // Identifiers that merely start with `return` are
                    // assignments or variables: an `=` after the merged
                    // identifier, or a lowercase/`_` continuation that
                    // is not `true`/`false`, does not count.
                    let ident: String = body[i..]
                        .chars()
                        .take_while(|ch| *ch == '_' || ch.is_ascii_alphanumeric())
                        .collect();
                    let rest = &ident["return".len().min(ident.len())..];
                    let after = body[i + ident.len()..].chars().next();
                    if ident == "return"
                        || rest.chars().next().is_some_and(|ch| ch.is_ascii_digit())
                        || ((rest
                            .chars()
                            .next()
                            .is_some_and(|ch| ch.is_ascii_uppercase())
                            || rest == "true"
                            || rest == "false")
                            && after != Some('='))
                    {
                        return true;
                    }
                }
                i += 6;
            }
            _ => {
                i += body[i..].chars().next().map_or(1, |ch| ch.len_utf8());
            }
        }
    }
    false
}

fn guard_dominates(before_call: &str, feature: &str) -> bool {
    let needle = format!("is_x86_feature_detected!(\"{feature}\")");
    let mut search_from = 0usize;
    while let Some(offset) = before_call[search_from..].find(&needle) {
        let pos = search_from + offset;
        let after = &before_call[pos + needle.len()..];
        let before = &before_call[..pos];
        // Assert form: `assert!(` then only an optional path, then the
        // detection; the detection closes, then the assert closes or a
        // message argument follows. The assert must sit at a scope that
        // still dominates the call (not a closed sibling branch).
        if let Some(assert_pos) = before.rfind("assert!(")
            && is_path_only(&before[assert_pos + "assert!(".len()..])
            && is_runtime_assert_at(before, assert_pos)
            && at_dominating_scope(before_call, assert_pos)
            && (after.starts_with(')') || after.starts_with(','))
        {
            return true;
        }
        // Negated early-return: `if!` then only an optional path, then the
        // detection, then a closed block that returns from the function.
        // The guard `if` must itself be at a dominating scope.
        if has_keyword_before_path(before, "if!")
            && let Some(if_pos) = before.rfind("if!")
            && at_dominating_scope(before_call, if_pos)
            && let Some(rest) = after.strip_prefix('{')
            && let Some(body_end) = matching_code_block_end(rest)
            && guard_block_returns(&rest[..body_end])
        {
            return true;
        }
        // Positive branch: `if` then only an optional path, then the
        // detection, then a block the call is still inside.
        if has_keyword_before_path(before, "if")
            && !has_keyword_before_path(before, "if!")
            && let Some(rest) = after.strip_prefix('{')
            && branch_still_open_at_operation(rest)
        {
            return true;
        }
        search_from = pos + needle.len();
    }
    false
}

fn is_path_only(text: &str) -> bool {
    text.chars()
        .all(|ch| ch == '_' || ch == ':' || ch.is_ascii_alphanumeric())
}

/// True when the last `keyword` in `before` is followed only by an
/// optional qualified path (`std::`, `core::arch::`) up to the detection.
fn has_keyword_before_path(before: &str, keyword: &str) -> bool {
    let Some(pos) = before.rfind(keyword) else {
        return false;
    };
    // For "if", the keyword must not be glued to a preceding identifier
    // other than the compacted `else` prefix (`elseif...` still counts).
    if keyword == "if"
        && pos > 0
        && before[..pos]
            .chars()
            .next_back()
            .is_some_and(|ch| ch == '_' || ch.is_ascii_alphanumeric())
        && !before[..pos].ends_with("else")
    {
        return false;
    }
    is_path_only(&before[pos + keyword.len()..])
}
