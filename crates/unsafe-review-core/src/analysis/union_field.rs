use crate::domain::OperationFamily;
use std::collections::{BTreeMap, BTreeSet};

pub(super) type UnionDeclarations = BTreeMap<String, BTreeSet<String>>;

/// Collect `union Name { field: Type, ... }` declarations in this file,
/// including `pub` unions. Maps union name to declared field names.
pub(super) fn union_declarations(lines: &[&str]) -> UnionDeclarations {
    let mut unions: UnionDeclarations = BTreeMap::new();
    let mut current: Option<(String, usize)> = None;
    for raw in lines {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if current.is_none() {
            if let Some(name) = parse_union_name(line) {
                collect_fields_from_segment(line, &mut unions, &name);
                let delta = brace_delta_signed(line);
                if delta > 0 {
                    current = Some((name, delta as usize));
                }
                continue;
            }
            continue;
        }
        if let Some((name, depth)) = current.take() {
            collect_fields_from_segment(line, &mut unions, &name);
            let new_depth = depth.saturating_add_signed(brace_delta_signed(line));
            if new_depth > 0 {
                current = Some((name, new_depth));
            }
        }
    }
    unions
}

fn brace_delta_signed(line: &str) -> isize {
    line.matches('{').count() as isize - line.matches('}').count() as isize
}

fn collect_fields_from_segment(line: &str, unions: &mut UnionDeclarations, union_name: &str) {
    // Only consider text inside the outermost braces when present; otherwise
    // the whole line is a field line inside a multi-line union body.
    let segment = if let Some(open) = line.find('{') {
        let after = &line[open + 1..];
        after.strip_suffix('}').unwrap_or(after)
    } else if line.contains('}') {
        return;
    } else {
        line
    };
    let entry = unions.entry(union_name.to_string()).or_default();
    for part in segment.split(',') {
        let part = part.trim().trim_end_matches('}').trim();
        if part.is_empty() {
            continue;
        }
        let part = strip_visibility(part);
        let Some((field, _ty)) = part.split_once(':') else {
            continue;
        };
        let field = field.trim();
        if !field.is_empty()
            && field
                .chars()
                .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
            && field
                .chars()
                .next()
                .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
        {
            entry.insert(field.to_string());
        }
    }
}

fn strip_visibility(text: &str) -> &str {
    let mut rest = text.trim_start();
    if let Some(after) = rest.strip_prefix("pub ") {
        rest = after.trim_start();
    } else if rest.starts_with("pub(")
        && let Some((_v, after)) = rest.split_once(')')
    {
        rest = after.trim_start();
    }
    rest
}

fn parse_union_name(line: &str) -> Option<String> {
    let rest = strip_visibility(line);
    let rest = rest.strip_prefix("union ")?.trim_start();
    parse_ident(rest)
}

fn parse_ident(rest: &str) -> Option<String> {
    let mut name = String::new();
    for ch in rest.chars() {
        if ch == '_' || ch.is_ascii_alphanumeric() {
            name.push(ch);
        } else {
            break;
        }
    }
    (!name.is_empty()).then_some(name)
}

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}
fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

/// Last path segment of a type spelling, e.g. `crate::Word` -> `Word`,
/// `ManuallyDrop<Word>` -> `ManuallyDrop`, `Word` -> `Word`.
fn type_base_name(ty: &str) -> String {
    let ty = ty.trim().trim_start_matches('&').trim();
    let ty = ty.strip_prefix("mut ").unwrap_or(ty).trim();
    let ty = ty.split('<').next().unwrap_or(ty).trim();
    let ty = ty.split('[').next().unwrap_or(ty).trim();
    ty.rsplit("::").next().unwrap_or(ty).trim().to_string()
}

#[derive(Clone, Debug)]
struct BindingState {
    // Scope stack; each map is binding name -> Some(union type) if
    // union-typed, None if non-union. Innermost scope is last.
    scopes: Vec<BTreeMap<String, Option<String>>>,
}

impl BindingState {
    fn new() -> Self {
        Self {
            scopes: vec![BTreeMap::new()],
        }
    }

    fn insert(&mut self, name: String, union_ty: Option<String>) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, union_ty);
        }
    }

    fn get(&self, name: &str) -> Option<Option<String>> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }

    fn push_scope(&mut self) {
        self.scopes.push(BTreeMap::new());
    }

    fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }
}

/// Parse a `let` binding starting at `pos` where text[pos..] starts with
/// "let ". Returns (name, union_type_if_any, end_offset_of_binding_head)
/// union_type is Some only when annotation or struct-literal init names a
/// declared union. Any other let binding yields None (non-union shadow).
fn parse_let_binding(
    text: &str,
    unions: &UnionDeclarations,
) -> Option<(String, Option<String>, usize)> {
    let rest = text.strip_prefix("let ")?.trim_start();
    let rest = rest.strip_prefix("mut ").unwrap_or(rest).trim_start();
    let name = parse_ident(rest)?;
    let after_name = &rest[name.len()..];
    let after_trim = after_name.trim_start();
    let union_ty = if let Some(after_colon) = after_trim.strip_prefix(':') {
        // type annotation up to `=` or `;`
        let end = after_colon.find(['=', ';']).unwrap_or(after_colon.len());
        let base = type_base_name(&after_colon[..end]);
        unions.contains_key(&base).then_some(base)
    } else if let Some(after_eq) = after_trim.strip_prefix('=') {
        let init = after_eq.trim_start();
        // Struct-literal init: `UnionType { ... }` or `path::UnionType {`
        let ident = parse_ident(init).or_else(|| {
            // qualified path: take last segment before `{`
            let before_brace = init.split('{').next().unwrap_or(init);
            let last = before_brace.trim().rsplit("::").next().unwrap_or("").trim();
            (!last.is_empty()).then(|| last.to_string())
        })?;
        let after_ident_pos = init.find(&ident).map(|p| p + ident.len()).unwrap_or(0);
        let after_ident = init[after_ident_pos..].trim_start();
        // For qualified paths, after_ident check differs; use brace presence
        let is_literal = after_ident.starts_with('{')
            || init
                .split('{')
                .next()
                .is_some_and(|p| p.trim_end().ends_with(&ident));
        if is_literal && unions.contains_key(&ident) {
            // Ensure the literal is not ManuallyDrop etc: ident checked above
            // Also ensure `{` follows the type name (allowing qualified path)
            let brace_ok = {
                let before_brace = init.split('{').next().unwrap_or("");
                before_brace.trim_end().ends_with(&ident) || after_ident.starts_with('{')
            };
            brace_ok.then_some(ident)
        } else {
            None
        }
    } else {
        None
    };
    let name_len = name.len();
    Some((name, union_ty, text.len() - rest.len() + name_len))
}

fn apply_params(state: &mut BindingState, scope: &str, unions: &UnionDeclarations) {
    let params_search = match scope.find('>') {
        Some(gt) if scope[..gt].contains('<') => &scope[gt + 1..],
        _ => scope,
    };
    if let Some(paren) = params_search.find('(')
        && let Some(close) = matching_close_paren(params_search, paren)
    {
        for param in split_top_level_commas(&params_search[paren + 1..close]) {
            let Some((binding, ty)) = param.split_once(':') else {
                continue;
            };
            let binding = binding
                .trim()
                .trim_start_matches("mut ")
                .trim()
                .trim_start_matches('&')
                .trim()
                .trim_start_matches("mut ")
                .trim();
            if binding.is_empty() || !binding.chars().all(is_ident_continue) {
                continue;
            }
            let base = type_base_name(ty);
            let union_ty = unions.contains_key(&base).then_some(base);
            state.insert(binding.to_string(), union_ty);
        }
    }
}

fn split_top_level_commas(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth_angle = 0usize;
    let mut depth_paren = 0usize;
    let mut start = 0usize;
    for (idx, ch) in text.char_indices() {
        match ch {
            '<' => depth_angle += 1,
            '>' => depth_angle = depth_angle.saturating_sub(1),
            '(' | '[' => depth_paren += 1,
            ')' | ']' => depth_paren = depth_paren.saturating_sub(1),
            ',' if depth_angle == 0 && depth_paren == 0 => {
                parts.push(&text[start..idx]);
                start = idx + 1;
            }
            _ => {}
        }
    }
    if start < text.len() {
        parts.push(&text[start..]);
    }
    parts
}

fn matching_close_paren(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (idx, ch) in text[open..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open + idx);
                }
            }
            _ => {}
        }
    }
    None
}

fn apply_scoped_text(state: &mut BindingState, text: &str, unions: &UnionDeclarations) {
    let mut i = 0usize;
    while i < text.len() {
        match text.as_bytes()[i] {
            b'{' => {
                state.push_scope();
                i += 1;
            }
            b'}' => {
                state.pop_scope();
                i += 1;
            }
            _ if text[i..].starts_with("let ")
                && (i == 0
                    || text[..i]
                        .chars()
                        .next_back()
                        .is_none_or(|ch| !is_ident_continue(ch))) =>
            {
                if let Some((name, union_ty, _)) = parse_let_binding(&text[i..], unions) {
                    state.insert(name, union_ty);
                }
                i += 4;
            }
            _ => {
                i += text[i..].chars().next().map_or(1, |ch| ch.len_utf8());
            }
        }
    }
}

fn initial_bindings(source: &str, start: usize, unions: &UnionDeclarations) -> BindingState {
    let mut state = BindingState::new();
    let prefix = &source[..start.min(source.len())];
    let scope_start = prefix.rfind("fn ").unwrap_or(0);
    let scope = &prefix[scope_start..];
    apply_params(&mut state, scope, unions);
    // Skip the fn header so its opening brace does not add a spurious
    // scope; scoped scanning starts at the fn body.
    let body_start = scope.find('{').map_or(scope.len(), |pos| pos + 1);
    apply_scoped_text(&mut state, &scope[body_start..], unions);
    state
}

/// Family for a union field access inside an unsafe block. Scans the block
/// sequentially so `let` rebindings inside the block update the receiver map
/// before later accesses are judged.
pub(super) fn union_field_use_family(
    block: &str,
    unions: &UnionDeclarations,
    source: &str,
    start: usize,
) -> Option<OperationFamily> {
    if unions.is_empty() {
        return None;
    }
    let mut state = initial_bindings(source, start, unions);
    let mut i = 0usize;
    while i < block.len() {
        match block.as_bytes()[i] {
            b'{' => {
                state.push_scope();
                i += 1;
                continue;
            }
            b'}' => {
                state.pop_scope();
                i += 1;
                continue;
            }
            _ => {}
        }
        // let binding?
        if block[i..].starts_with("let ")
            && (i == 0
                || block[..i]
                    .chars()
                    .next_back()
                    .is_none_or(|ch| !is_ident_continue(ch)))
        {
            if let Some((name, union_ty, _)) = parse_let_binding(&block[i..], unions) {
                state.insert(name, union_ty);
            }
            i += 4;
            continue;
        }
        // identifier start?
        if i < block.len() && is_ident_start(block.as_bytes()[i] as char) {
            // parse ident
            let rest = &block[i..];
            if let Some(name) = parse_ident(rest) {
                let before = block[..i].chars().next_back();
                let before_ok = before.is_none_or(|ch| !is_ident_continue(ch) && ch != '.');
                if before_ok
                    && let Some(Some(union_ty)) = state.get(&name)
                    && let Some(after_dot) = rest[name.len()..].strip_prefix('.')
                    && let Some(field) = parse_ident(after_dot)
                    && !after_dot[field.len()..].trim_start().starts_with('(')
                    && unions
                        .get(&union_ty)
                        .is_some_and(|fields| fields.contains(&field))
                {
                    return Some(OperationFamily::UnionFieldRead);
                }
                i += name.len();
                continue;
            }
        }
        // advance by one char (UTF-8 safe: advance to next char boundary)
        let ch_len = block[i..].chars().next().map_or(1, |ch| ch.len_utf8());
        i += ch_len;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decls() -> UnionDeclarations {
        union_declarations(&["union Word {", "    bits: u64,", "    bytes: [u8; 8],", "}"])
    }

    #[test]
    fn collects_pub_and_plain_union_fields() {
        let unions = union_declarations(&[
            "pub union Word { bits: u64, bytes: [u8; 8] }",
            "union Other {",
            "    a: u32,",
            "    b: f32,",
            "}",
        ]);
        assert!(
            unions
                .get("Word")
                .is_some_and(|f| f.contains("bits") && f.contains("bytes")),
            "{unions:?}"
        );
        assert!(
            unions
                .get("Other")
                .is_some_and(|f| f.contains("a") && f.contains("b")),
            "{unions:?}"
        );
    }

    #[test]
    fn scope_exited_union_binding_does_not_retype_outer_receiver() {
        let unions = decls();
        let source =
            "fn f(s: u64) -> u64 { { let s: Word = Word { bits: 1 }; } unsafe { s.bits } }";
        let start = source.find("unsafe {").unwrap_or(0);
        assert_eq!(
            union_field_use_family("unsafe { s.bits }", &unions, source, start),
            None
        );
        // Access inside the inner scope still routes.
        let inner = "fn f() -> u64 { { let w: Word = Word { bits: 1 }; unsafe { w.bits } } }";
        let inner_start = inner.find("unsafe {").unwrap_or(0);
        assert_eq!(
            union_field_use_family("unsafe { w.bits }", &unions, inner, inner_start),
            Some(OperationFamily::UnionFieldRead)
        );
    }

    #[test]
    fn typed_receiver_routes_and_undeclared_field_does_not() {
        let unions = decls();
        let source = "fn f() { let w: Word = Word { bits: 0 }; unsafe { w.bits } }";
        let start = source.find("unsafe {").unwrap_or(0);
        assert_eq!(
            union_field_use_family("unsafe { w.bits }", &unions, source, start),
            Some(OperationFamily::UnionFieldRead)
        );
        assert_eq!(
            union_field_use_family("unsafe { w.nope }", &unions, source, start),
            None
        );
    }
}
