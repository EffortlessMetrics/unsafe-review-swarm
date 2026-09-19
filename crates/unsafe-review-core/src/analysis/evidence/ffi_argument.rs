use super::nonnull::receiver_has_null_guard;
use super::operation_scope::code_before_site_operation;
use super::{compact_code, strip_block_comments_and_literals};
use crate::analysis::scanner::ScannedSite;

/// Structural call-to-parameter mapping for FFI call sites, preserved for
/// the #2306 callee-contract model. These helpers report observations only:
/// which arguments sit at raw-pointer parameter positions, and which carry
/// a dominating null check. Per #2315 neither fact establishes a callee
/// requirement, and no obligation synthesizer may treat them as one until
/// an explicit supported requirement source exists.
///
/// Pointer-typed call arguments of an `FfiCall` site: bare identifiers
/// passed at positions the same-context `extern` declaration types as raw
/// pointers.
///
/// The declaration lookup is deliberately narrow: the callee's `fn name(`
/// declaration must appear in the site's context window, be followed by `;`
/// (a foreign declaration, not a local definition), and sit inside an
/// `extern` block (no `}` between the block keyword and the declaration).
/// Callees declared outside the context window yield no mapping rather than
/// a guessed one. Function-pointer parameters are skipped.
pub(crate) fn ffi_pointer_call_arguments(expression: &str, lower: &str) -> Vec<String> {
    let Some(callee) = call_callee_name(expression) else {
        return Vec::new();
    };
    let Some(params) = extern_declaration_params(lower, &callee) else {
        return Vec::new();
    };
    let args = call_arguments(expression, &callee);
    split_top_level(&params)
        .iter()
        .enumerate()
        .filter(|(_, param)| is_raw_pointer_param(param) && !is_fn_pointer_param(param))
        .filter_map(|(idx, _)| args.get(idx))
        .filter_map(|arg| bare_identifier(arg))
        .collect()
}

/// Bare-identifier call arguments with a dominating null check in the
/// site-anchored guard scope: `if arg.is_null() { return ...; }` or an open
/// `if !arg.is_null() {` branch. An observed check is a caller-established
/// fact only; it never creates the callee requirement it might one day be
/// matched against (#2315).
pub(crate) fn ffi_guarded_call_arguments(site: &ScannedSite, lower: &str) -> Vec<String> {
    let expression = &site.operation.expression;
    let Some(callee) = call_callee_name(expression) else {
        return Vec::new();
    };
    let guard_scope =
        code_before_site_operation(site, lower, expression).unwrap_or_else(|| lower.to_string());
    let guard_compact = compact_code(&strip_block_comments_and_literals(&guard_scope));
    call_arguments(expression, &callee)
        .iter()
        .filter_map(|arg| bare_identifier(arg))
        .filter(|arg| receiver_has_null_guard(&guard_compact, arg))
        .collect()
}

/// Final path segment of the called expression: `ffi_strlen` from
/// `unsafe { ffi_strlen(s) }`, `ffi_strlen(s)`, or `libc::ffi_strlen(s)`.
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
    if name.is_empty() || !is_bare_identifier(name) {
        return None;
    }
    Some(name.to_ascii_lowercase())
}

/// Parameter list of the same-context foreign declaration
/// `fn <callee>(...)`, without the surrounding parens.
fn extern_declaration_params(lower: &str, callee: &str) -> Option<String> {
    let marker = format!("{callee}(");
    let mut search_from = 0usize;
    while let Some(offset) = lower[search_from..].find(&marker) {
        let abs_pos = search_from + offset;
        // The call marker must be a declaration: preceded (across optional
        // whitespace) by a standalone `fn` keyword, not by a path, dot, or
        // another identifier tail.
        if is_fn_declaration_at(lower, abs_pos)
            && is_foreign_declaration(lower, abs_pos)
            && let Some(params) = matched_params(&lower[abs_pos + marker.len() - 1..])
        {
            return Some(params);
        }
        search_from = abs_pos + marker.len();
    }
    None
}

fn is_fn_declaration_at(lower: &str, name_pos: usize) -> bool {
    let before = lower[..name_pos].trim_end();
    let Some(fn_start) = before.strip_suffix("fn") else {
        return false;
    };
    fn_start
        .chars()
        .next_back()
        .is_none_or(|ch| !ch.is_ascii_alphanumeric() && ch != '_')
}

/// A same-context `fn` item is a foreign declaration when it is
/// semicolon-terminated (no body) and sits inside an `extern` block.
fn is_foreign_declaration(lower: &str, name_pos: usize) -> bool {
    let after_fn = &lower[name_pos..];
    let Some(open) = after_fn.find('(') else {
        return false;
    };
    let Some(params) = matched_params(&after_fn[open..]) else {
        return false;
    };
    // A foreign declaration ends the item with `;` (after an optional
    // return type); a local definition opens a `{` body instead.
    let tail = after_fn[open + params.len() + 2..].trim_start();
    let mut terminator = None;
    for ch in tail.chars() {
        if ch == ';' || ch == '{' {
            terminator = Some(ch);
            break;
        }
    }
    if terminator != Some(';') {
        return false;
    }
    let before = &lower[..name_pos];
    before
        .rfind("extern")
        .is_some_and(|block| !before[block..].contains('}'))
}

/// Text inside balanced parens starting at the opening paren.
fn matched_params(text_after_name: &str) -> Option<String> {
    let mut depth = 0usize;
    for (idx, ch) in text_after_name.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(text_after_name[1..idx].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

fn is_raw_pointer_param(param: &str) -> bool {
    param.contains("*const") || param.contains("*mut")
}

fn is_fn_pointer_param(param: &str) -> bool {
    param.contains("fn(") || param.contains("fn (")
}

/// Top-level comma-split call arguments for the site's own callee call.
fn call_arguments(expression: &str, callee: &str) -> Vec<String> {
    let compact = compact_code(&expression.to_ascii_lowercase());
    let marker = format!("{callee}(");
    let Some(start) = compact.find(&marker) else {
        return Vec::new();
    };
    let Some(params) = matched_params(&compact[start + callee.len()..]) else {
        return Vec::new();
    };
    split_top_level(&params)
}

fn split_top_level(params: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut depth = 0usize;
    let mut current = String::new();
    for ch in params.chars() {
        match ch {
            '(' | '[' | '{' => {
                depth += 1;
                current.push(ch);
            }
            ')' | ']' | '}' => {
                depth = depth.saturating_sub(1);
                current.push(ch);
            }
            ',' if depth == 0 => {
                args.push(current.trim().to_string());
                current = String::new();
            }
            _ => current.push(ch),
        }
    }
    if !current.trim().is_empty() {
        args.push(current.trim().to_string());
    }
    args
}

fn is_bare_identifier(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        && !text.chars().all(|ch| ch.is_ascii_digit())
        && !matches!(text, "true" | "false" | "self")
}

/// A bare identifier argument, or `None` for literals, `_`, and complex
/// expressions. Only a named value can carry a validity obligation its
/// caller is responsible for.
fn bare_identifier(arg: &str) -> Option<String> {
    let trimmed = arg.trim();
    if trimmed == "_" || !is_bare_identifier(trimmed) {
        return None;
    }
    Some(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DECL: &str = "\nunsafe extern \"c\" {\nfn ffi_strlen(s: *const c_char) -> usize;\n}\n";

    fn context_with_decl(extra: &str) -> String {
        format!("{DECL}{extra}")
    }

    #[test]
    fn pointer_argument_resolves_through_same_context_declaration() {
        let args = ffi_pointer_call_arguments(
            "unsafe { ffi_strlen(s) }",
            &context_with_decl("pub fn unguarded(s: *const c_char) -> usize {\n"),
        );
        assert_eq!(args, vec!["s".to_string()]);
    }

    #[test]
    fn integer_parameters_yield_no_pointer_arguments() {
        let lower = context_with_decl("fn ffi_add(a: i32, b: i32) -> i32;\n").replace(
            "fn ffi_strlen(s: *const c_char) -> usize;",
            "fn ffi_add(a: i32, b: i32) -> i32;",
        );
        let args = ffi_pointer_call_arguments("unsafe { ffi_add(a, b) }", &lower);
        assert!(args.is_empty());
    }

    #[test]
    fn local_fn_definitions_are_not_foreign_declarations() {
        let lower = "pub fn ffi_strlen(s: *const c_char) -> usize {\n1\n}\n";
        let args = ffi_pointer_call_arguments("unsafe { ffi_strlen(s) }", lower);
        assert!(args.is_empty());
    }

    #[test]
    fn literals_and_underscore_never_carry_argument_obligations() {
        let lower = context_with_decl("");
        assert!(ffi_pointer_call_arguments("unsafe { ffi_strlen(0) }", &lower).is_empty());
        assert!(ffi_pointer_call_arguments("unsafe { ffi_strlen(_) }", &lower).is_empty());
        assert!(ffi_pointer_call_arguments("unsafe { ffi_strlen(self) }", &lower).is_empty());
    }

    #[test]
    fn argument_at_non_pointer_position_is_ignored() {
        let lower = "\nunsafe extern \"c\" {\nfn ffi_copy(dst: *mut u8, n: usize);\n}\n";
        let args = ffi_pointer_call_arguments("unsafe { ffi_copy(dst, n) }", lower);
        assert_eq!(args, vec!["dst".to_string()]);
    }
}
