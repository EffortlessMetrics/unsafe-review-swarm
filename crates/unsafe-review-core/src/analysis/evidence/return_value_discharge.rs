//! Discharge for the FFI `return_value` obligation: an explicit post-call
//! check on the bound return name with a diverging error arm.
//!
//! Recognized shape only: a single-comparison `if` on the bound name after
//! the call, where the bad-value arm diverges (`return`, `?`, `panic!`,
//! `unreachable!`, `todo!`, `unimplemented!`, `bail!`, or `Err(`
//! construction). For `==` the bad arm is the `else` arm; for `!=`, `<`,
//! `>`, `<=`, `>=` it is the `if` arm. Compound conditions, `else if`
//! chains, method-call checks, `assert!`, and `debug_assert!` never
//! discharge: the first three are unhandled shapes, the last two are not
//! release-runtime guards. A rebinding of the name before the check also
//! stays missing, since the check then tests a different value.

use super::code_text::contains_executable_return;
use super::receiver_path::is_receiver_path_char;
use crate::analysis::scanner::ScannedSite;
use crate::domain::EvidenceState;

const DIVERGE_MACROS: &[&str] = &["panic!", "unreachable!", "todo!", "unimplemented!", "bail!"];

pub(super) fn return_value_discharge_state(site: &ScannedSite) -> EvidenceState {
    let Some(name) = site.operation.bound_name.as_deref() else {
        return EvidenceState::missing(
            "No bound return name was recovered; the call value is discarded or nested",
        );
    };
    let name = name.to_ascii_lowercase();
    let after: Vec<String> = site
        .context_after
        .iter()
        .map(|line| line.to_ascii_lowercase())
        .collect();
    for (idx, line) in after.iter().enumerate() {
        if rebinds_name(line, &name) {
            return EvidenceState::missing(
                "The bound return name is rebound before any enforcing check",
            );
        }
        if let Some(check) = parse_return_check(line, &name)
            && arms_diverge(&after[idx..], check)
        {
            return EvidenceState::present(
                "Return-value check with a diverging error arm was detected",
            );
        }
    }
    EvidenceState::missing("No return-value check with a diverging error arm was detected")
}

/// A bound name is rebound by a `let NAME` prefix (shadowing counts) or a
/// plain `NAME =` assignment. Comparisons, arrows, paths, and the check
/// itself never match.
fn rebinds_name(line: &str, name: &str) -> bool {
    let trimmed = line.trim();
    if let Some(rest) = trimmed.strip_prefix("let ") {
        let rest = rest.strip_prefix("mut ").unwrap_or(rest);
        let head = rest.split([':', '=']).next().unwrap_or("").trim();
        return head == name;
    }
    let mut cursor = line;
    while let Some(pos) = cursor.find(name) {
        let before = cursor[..pos].chars().next_back();
        if before.is_some_and(is_receiver_path_char) {
            cursor = &cursor[pos + name.len()..];
            continue;
        }
        let mut tail = cursor[pos + name.len()..].trim_start();
        if let Some(after_colon) = tail.strip_prefix(':') {
            if after_colon.starts_with(':') {
                // `name::path`, not an annotation.
                cursor = &cursor[pos + name.len()..];
                continue;
            }
            match after_colon.trim_start().find('=') {
                Some(eq) => tail = after_colon.trim_start()[eq..].trim_start(),
                None => {
                    cursor = &cursor[pos + name.len()..];
                    continue;
                }
            }
        }
        if let Some(after_eq) = tail.strip_prefix('=')
            && !after_eq.starts_with(['=', '>'])
        {
            return true;
        }
        cursor = &cursor[pos + name.len()..];
    }
    false
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BadArm {
    InIf,
    InElse,
}

struct ReturnCheck {
    bad_arm: BadArm,
}

/// Parses a single-comparison `if` on the bound name. Returns which arm the
/// bad value takes, or `None` for anything else (including compound
/// conditions, call shapes, and `else if` chains, which stay missing).
fn parse_return_check(line: &str, name: &str) -> Option<ReturnCheck> {
    let cond = if_condition(line)?;
    if cond.contains("&&") || cond.contains("||") {
        return None;
    }
    let cond = strip_outer_parens(cond.trim());
    if cond.contains('(') || cond.contains(')') {
        return None;
    }
    let (left, operator, right) = split_comparison(cond)?;
    let name_on_left = is_name_operand(left, name);
    let name_on_right = is_name_operand(right, name);
    match operator {
        "==" => {
            if name_on_left || name_on_right {
                Some(ReturnCheck {
                    bad_arm: BadArm::InElse,
                })
            } else {
                None
            }
        }
        "!=" => {
            if name_on_left || name_on_right {
                Some(ReturnCheck {
                    bad_arm: BadArm::InIf,
                })
            } else {
                None
            }
        }
        "<" | ">" | "<=" | ">=" => {
            if name_on_left {
                Some(ReturnCheck {
                    bad_arm: BadArm::InIf,
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Extracts the `if` condition: text after whole-word `if` up to `{` or
/// end of line. Returns `None` for `else if` chains, which stay missing.
fn if_condition(line: &str) -> Option<&str> {
    let mut cursor = line;
    while let Some(pos) = cursor.find("if") {
        let before = cursor[..pos].chars().next_back();
        let after = cursor[pos + 2..].chars().next();
        if before.is_none_or(|ch| !is_receiver_path_char(ch))
            && after.is_none_or(|ch| !is_receiver_path_char(ch))
        {
            let rest = cursor[pos + 2..].trim_start();
            if rest.starts_with("let") {
                return None;
            }
            let cond = rest.split('{').next().unwrap_or(rest).trim();
            // Reject `else if`: an `else` keyword before this `if`.
            let head = line[..line.len() - cursor.len() + pos].trim_end();
            if head.ends_with("else") {
                return None;
            }
            return Some(cond);
        }
        cursor = &cursor[pos + 2..];
    }
    None
}

fn strip_outer_parens(cond: &str) -> &str {
    let stripped = cond.strip_prefix('(').unwrap_or(cond);
    if stripped.len() != cond.len() {
        return stripped.strip_suffix(')').unwrap_or(stripped).trim();
    }
    cond
}

fn split_comparison(cond: &str) -> Option<(&str, &str, &str)> {
    for operator in ["==", "!=", "<=", ">=", "<", ">"] {
        if let Some(pos) = cond.find(operator) {
            return Some((
                cond[..pos].trim(),
                operator,
                cond[pos + operator.len()..].trim(),
            ));
        }
    }
    None
}

fn is_name_operand(side: &str, name: &str) -> bool {
    side.trim() == name
}

/// Walks the `if` construct's arms and reports whether the bad-value arm
/// diverges. Only plain `if` / `if-else` shapes are handled.
fn arms_diverge(lines: &[String], check: ReturnCheck) -> bool {
    let joined = lines
        .iter()
        .take(8)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join("\n");
    let Some(braced) = joined.find('{') else {
        return false;
    };
    let Some((if_body, rest)) = balanced_body(&joined[braced..]) else {
        return false;
    };
    match check.bad_arm {
        BadArm::InIf => diverges(if_body),
        BadArm::InElse => {
            let tail = rest.trim_start();
            let tail = tail.strip_prefix("else").unwrap_or(tail);
            if tail == rest.trim_start() {
                return false;
            }
            let tail = tail.trim_start();
            if tail.starts_with("if") {
                // `else if` chain: unhandled shape, stays missing.
                return false;
            }
            let Some(braced) = tail.find('{') else {
                return false;
            };
            match balanced_body(&tail[braced..]) {
                Some((else_body, _)) => diverges(else_body),
                None => false,
            }
        }
    }
}

fn balanced_body(text: &str) -> Option<(&str, &str)> {
    let mut depth = 0i32;
    let mut end = None;
    for (idx, ch) in text.char_indices() {
        if ch == '{' {
            depth += 1;
        } else if ch == '}' {
            depth -= 1;
            if depth == 0 {
                end = Some(idx);
                break;
            }
        }
    }
    let end = end?;
    Some((&text[1..end], &text[end + 1..]))
}

fn diverges(body: &str) -> bool {
    if contains_executable_return(body) {
        return true;
    }
    if body.contains("err(") {
        return true;
    }
    if DIVERGE_MACROS
        .iter()
        .any(|marker| contains_macro_call(body, marker))
    {
        return true;
    }
    has_try_operator(body)
}

/// Reports a macro call: the `name!` marker with a non-identifier character
/// before the name, so `my_panic!` does not match `panic!`.
fn contains_macro_call(body: &str, marker: &str) -> bool {
    let mut cursor = body;
    while let Some(pos) = cursor.find(marker) {
        let before = cursor[..pos].chars().next_back();
        if before.is_none_or(|ch| !is_receiver_path_char(ch)) {
            return true;
        }
        cursor = &cursor[pos + marker.len()..];
    }
    false
}

/// Reports a `?` try operator: a `?` whose next non-whitespace character
/// closes the expression (`;`, `,`, `)`, `}`) or ends the body.
fn has_try_operator(body: &str) -> bool {
    let bytes = body.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'?' {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if j >= bytes.len() || matches!(bytes[j], b';' | b',' | b')' | b'}') {
                return true;
            }
        }
        i += 1;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        OperationFamily, SourceLocation, SourceRole, UnsafeOperation, UnsafeSite, UnsafeSiteKind,
    };
    use std::path::PathBuf;

    fn ffi_site(bound: Option<&str>, after: &[&str]) -> ScannedSite {
        ScannedSite {
            site: UnsafeSite {
                location: SourceLocation::new(PathBuf::from("src/lib.rs"), 7, 5),
                kind: UnsafeSiteKind::FfiCall,
                owner: Some("run".to_string()),
                visibility: "private".to_string(),
                public_api_surface: false,
                changed: true,
                snippet: "unsafe { checkable() }".to_string(),
                role: SourceRole::Unknown,
            },
            operation: UnsafeOperation {
                family: OperationFamily::Ffi,
                expression: "unsafe { checkable() }".to_string(),
                bound_name: bound.map(str::to_string),
            },
            context_before: Vec::new(),
            context_after: after.iter().map(ToString::to_string).collect(),
        }
    }

    fn present(site: &ScannedSite) -> bool {
        return_value_discharge_state(site).present
    }

    #[test]
    fn equality_check_with_err_else_arm_discharges() {
        let site = ffi_site(
            Some("result"),
            &[
                "if result == 0 {",
                "    Ok(())",
                "} else {",
                "    Err(\"checkable failed\")",
                "}",
            ],
        );
        assert!(present(&site));
    }

    #[test]
    fn inequality_early_return_discharges() {
        let site = ffi_site(
            Some("ret"),
            &["if ret != 0 {", "    return Err(Error::X);", "}"],
        );
        assert!(present(&site));
    }

    #[test]
    fn negative_comparison_early_return_discharges() {
        let site = ffi_site(Some("ret"), &["if ret < 0 {", "    return Err(e);", "}"]);
        assert!(present(&site));
    }

    #[test]
    fn yoda_equality_discharges() {
        let site = ffi_site(
            Some("result"),
            &[
                "if 0 == result {",
                "    ok();",
                "} else {",
                "    return Err(e);",
                "}",
            ],
        );
        assert!(present(&site));
    }

    #[test]
    fn question_mark_arm_discharges() {
        let site = ffi_site(Some("ret"), &["if ret != 0 {", "    failed()?;", "}"]);
        assert!(present(&site));
    }

    #[test]
    fn panic_arm_discharges() {
        let site = ffi_site(Some("ret"), &["if ret != 0 {", "    panic!(\"bad\");", "}"]);
        assert!(present(&site));
    }

    #[test]
    fn unchecked_binding_stays_missing() {
        let site = ffi_site(Some("result"), &["result"]);
        assert!(!present(&site));
    }

    #[test]
    fn equality_without_else_stays_missing() {
        let site = ffi_site(
            Some("result"),
            &["if result == 0 {", "    ok();", "}", "result"],
        );
        assert!(!present(&site));
    }

    #[test]
    fn debug_assert_only_stays_missing() {
        let site = ffi_site(Some("result"), &["debug_assert!(result == 0);", "result"]);
        assert!(!present(&site));
    }

    #[test]
    fn wrong_name_check_stays_missing() {
        let site = ffi_site(
            Some("result"),
            &[
                "if other == 0 {",
                "    ok();",
                "} else {",
                "    return Err(e);",
                "}",
            ],
        );
        assert!(!present(&site));
    }

    #[test]
    fn rebind_before_check_stays_missing() {
        let site = ffi_site(
            Some("result"),
            &[
                "result = other();",
                "if result == 0 {",
                "    ok();",
                "} else {",
                "    return Err(e);",
                "}",
            ],
        );
        assert!(!present(&site));
    }

    #[test]
    fn compound_condition_stays_missing() {
        let site = ffi_site(
            Some("ret"),
            &["if ret != 0 && ready {", "    return Err(e);", "}"],
        );
        assert!(!present(&site));
    }

    #[test]
    fn else_if_chain_stays_missing() {
        let site = ffi_site(
            Some("ret"),
            &[
                "if ret == 0 {",
                "    ok();",
                "} else if ret == 1 {",
                "    retry();",
                "} else {",
                "    return Err(e);",
                "}",
            ],
        );
        assert!(!present(&site));
    }

    #[test]
    fn call_shape_check_stays_missing() {
        let site = ffi_site(
            Some("ptr"),
            &["if ptr.is_null() {", "    return Err(e);", "}"],
        );
        assert!(!present(&site));
    }

    #[test]
    fn discarded_value_stays_missing() {
        let site = ffi_site(None, &["if result == 0 {", "    return Err(e);", "}"]);
        assert!(!present(&site));
    }
}
