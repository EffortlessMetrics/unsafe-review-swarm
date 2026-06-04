use super::owner_context::{context_before_site, find_owner};
use super::text_detection::{LineCommentState, line_for_text_detection};
use super::{
    ScannedSite, contains_any, context_slice, first_non_ws_column, one_line, visibility_for_snippet,
};
use crate::domain::{OperationFamily, SourceLocation, UnsafeOperation, UnsafeSite, UnsafeSiteKind};
use crate::input::diff::DiffIndex;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

#[derive(Clone, Debug)]
struct JsSignedLine {
    idx: usize,
    line_no: usize,
    text: String,
    owner: String,
}

pub(super) fn detect_panic_from_safe_js_sites(
    rel: &PathBuf,
    diff: Option<&DiffIndex>,
    repo_mode: bool,
    lines: &[&str],
) -> Vec<ScannedSite> {
    let mut by_owner = BTreeMap::<String, Vec<JsSignedLine>>::new();
    for line in executable_owner_lines(lines) {
        by_owner.entry(line.owner.clone()).or_default().push(line);
    }

    let mut sites = Vec::new();
    for (owner, mut owner_lines) in by_owner {
        owner_lines.sort_by_key(|line| line.line_no);
        let signed_sources = js_signed_source_bindings(&owner_lines);
        for (sink_idx, sink) in owner_lines.iter().enumerate() {
            if !is_panicking_unsigned_conversion(&sink.text) {
                continue;
            }
            let source_binding = try_from_argument(&sink.text)
                .and_then(|arg| simple_identifier_from_expression(&arg));
            let has_direct_source = is_js_signed_source(&sink.text);
            let has_bound_source = source_binding
                .as_deref()
                .is_some_and(|binding| signed_sources.contains(binding));
            if !(has_direct_source || has_bound_source) {
                continue;
            }
            if has_sign_or_range_guard(&owner_lines, sink_idx, source_binding.as_deref(), sink) {
                continue;
            }
            if !panic_from_safe_js_changed(diff, repo_mode, rel, sink) {
                continue;
            }

            let raw = lines[sink.idx];
            let context_before = context_before_site(lines, sink.idx);
            let context_after = context_slice(
                lines,
                (sink.idx + 1).min(lines.len()),
                (sink.idx + 8).min(lines.len()),
            );
            sites.push(ScannedSite {
                site: UnsafeSite {
                    location: SourceLocation::new(
                        rel.clone(),
                        sink.line_no,
                        first_non_ws_column(raw),
                    ),
                    kind: UnsafeSiteKind::Operation,
                    owner: Some(owner.clone()),
                    visibility: visibility_for_snippet(raw.trim()).to_string(),
                    public_api_surface: false,
                    changed: true,
                    snippet: sink.text.clone(),
                },
                operation: UnsafeOperation {
                    family: OperationFamily::PanicFromSafeJs,
                    expression: panic_from_safe_js_expression(sink, source_binding.as_deref()),
                },
                context_before,
                context_after,
            });
        }
    }
    sites
}

fn executable_owner_lines(lines: &[&str]) -> Vec<JsSignedLine> {
    let mut out = Vec::new();
    let mut state = LineCommentState::default();
    for (idx, raw) in lines.iter().enumerate() {
        let detection_line = line_for_text_detection(raw, &mut state);
        let text = detection_line.trim();
        if text.is_empty() {
            continue;
        }
        let Some(owner) = find_owner(lines, idx) else {
            continue;
        };
        out.push(JsSignedLine {
            idx,
            line_no: idx + 1,
            text: text.to_string(),
            owner,
        });
    }
    out
}

fn js_signed_source_bindings(lines: &[JsSignedLine]) -> BTreeSet<String> {
    lines
        .iter()
        .filter(|line| {
            is_js_signed_source(&line.text) && !line_has_inline_nonnegative_guard(&line.text)
        })
        .filter_map(|line| let_binding_name(&line.text))
        .collect()
}

fn is_panicking_unsigned_conversion(line: &str) -> bool {
    is_unsigned_try_from(line)
        && (line.contains(".expect(")
            || (line.contains(".unwrap()") && !line.contains("unwrap_unchecked")))
}

fn is_unsigned_try_from(line: &str) -> bool {
    [
        "usize::try_from",
        "u64::try_from",
        "u32::try_from",
        "u16::try_from",
        "u8::try_from",
    ]
    .iter()
    .any(|needle| line.contains(needle))
}

fn is_js_signed_source(line: &str) -> bool {
    line.contains(".to_int32(")
        || line.contains(".to_int32()")
        || line.contains("coerce::<i32>")
        || line.contains("coerce::<i64>")
        || ((line.contains(" as i32") || line.contains(" as i64"))
            && contains_any(
                line,
                &[
                    "argument",
                    "arguments",
                    "argv",
                    "js_value",
                    "jsvalue",
                    "value",
                ],
            ))
}

fn has_sign_or_range_guard(
    lines: &[JsSignedLine],
    sink_idx: usize,
    source_binding: Option<&str>,
    sink: &JsSignedLine,
) -> bool {
    if line_has_inline_nonnegative_guard(&sink.text) {
        return true;
    }
    let Some(binding) = source_binding else {
        return false;
    };
    lines
        .iter()
        .enumerate()
        .take(sink_idx)
        .any(|(idx, _line)| line_guards_nonnegative_binding(lines, idx, binding))
}

fn line_has_inline_nonnegative_guard(line: &str) -> bool {
    line.contains(".max(0)")
        || line.contains(".max(0i32)")
        || line.contains(".max(0_i32)")
        || line.contains("saturating_abs(")
}

fn line_guards_nonnegative_binding(lines: &[JsSignedLine], idx: usize, binding: &str) -> bool {
    let line = &lines[idx].text;
    if !line_mentions_identifier(line, binding) {
        return false;
    }
    let compact = line.split_whitespace().collect::<String>();
    let guards_negative = compact.contains(&format!("{binding}<0"))
        || compact.contains(&format!("{binding}<=0"))
        || compact.contains(&format!("0>{binding}"))
        || compact.contains(&format!("0>={binding}"))
        || compact.contains(&format!("{binding}.is_negative()"));
    guards_negative && nearby_guard_returns_or_errors(lines, idx)
}

fn nearby_guard_returns_or_errors(lines: &[JsSignedLine], idx: usize) -> bool {
    for line in lines.iter().skip(idx).take(4) {
        if line_returns_or_errors(&line.text) {
            return true;
        }
        if line.idx != lines[idx].idx && line.text.contains('}') {
            break;
        }
    }
    false
}

fn line_returns_or_errors(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    line.contains("return")
        || line.contains("Err(")
        || line.contains("bail!(")
        || lower.contains("throw")
}

fn try_from_argument(line: &str) -> Option<String> {
    let start = line.find("::try_from(")? + "::try_from(".len();
    let mut depth = 1usize;
    for (offset, ch) in line[start..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(line[start..start + offset].trim().to_string());
                }
            }
            _ => {}
        }
    }
    None
}

fn simple_identifier_from_expression(expression: &str) -> Option<String> {
    is_simple_identifier(expression).then(|| expression.to_string())
}

fn let_binding_name(line: &str) -> Option<String> {
    let (before_assignment, _) = line.split_once('=')?;
    let mut binding = before_assignment.trim().strip_prefix("let ")?.trim();
    binding = binding.strip_prefix("mut ").unwrap_or(binding).trim();
    let binding = binding.split(':').next().unwrap_or(binding).trim();
    is_simple_identifier(binding).then(|| binding.to_string())
}

fn line_mentions_identifier(line: &str, identifier: &str) -> bool {
    let mut cursor = line;
    while let Some(pos) = cursor.find(identifier) {
        let before = cursor[..pos].chars().next_back();
        let after = &cursor[pos + identifier.len()..];
        let starts_on_boundary = before.is_none_or(|ch| !is_ident_continue(ch));
        let ends_on_boundary = after.chars().next().is_none_or(|ch| !is_ident_continue(ch));
        if starts_on_boundary && ends_on_boundary {
            return true;
        }
        cursor = &after[after.chars().next().map_or(after.len(), char::len_utf8)..];
    }
    false
}

fn is_simple_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn panic_from_safe_js_changed(
    diff: Option<&DiffIndex>,
    repo_mode: bool,
    rel: &PathBuf,
    sink: &JsSignedLine,
) -> bool {
    diff.is_none_or(|diff| repo_mode || diff.contains_near(rel, sink.line_no))
}

fn panic_from_safe_js_expression(sink: &JsSignedLine, source_binding: Option<&str>) -> String {
    let source = source_binding.map_or_else(
        || "inline JS-derived signed value".to_string(),
        |binding| format!("JS-derived signed binding `{binding}`"),
    );
    format!(
        "JS-derived signed value reaches panicking unsigned conversion without a visible sign/range guard; source: {}; sink: {}",
        source,
        one_line(&sink.text)
    )
}
