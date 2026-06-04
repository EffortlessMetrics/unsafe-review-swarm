use super::owner_context::{context_before_site, find_owner};
use super::text_detection::{LineCommentState, line_for_text_detection};
use super::{
    ScannedSite, contains_any, contains_call_name, context_slice, first_non_ws_column, one_line,
    visibility_for_snippet,
};
use crate::domain::{OperationFamily, SourceLocation, UnsafeOperation, UnsafeSite, UnsafeSiteKind};
use crate::input::diff::DiffIndex;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Clone, Debug)]
struct JsNativeFfiLine {
    idx: usize,
    line_no: usize,
    text: String,
    owner: String,
}

pub(super) fn detect_js_native_ffi_byte_sites(
    rel: &PathBuf,
    diff: Option<&DiffIndex>,
    repo_mode: bool,
    lines: &[&str],
) -> Vec<ScannedSite> {
    let mut by_owner = BTreeMap::<String, Vec<JsNativeFfiLine>>::new();
    for signal in js_native_ffi_lines(lines) {
        by_owner
            .entry(signal.owner.clone())
            .or_default()
            .push(signal);
    }

    let mut sites = Vec::new();
    for (owner, mut owner_lines) in by_owner {
        owner_lines.sort_by_key(|line| line.line_no);
        let Some(capture_idx) = owner_lines
            .iter()
            .position(|line| is_js_backed_byte_capture(&line.text))
        else {
            continue;
        };
        let capture_binding = js_byte_capture_binding(&owner_lines[capture_idx].text);
        let Some(handoff_idx) =
            native_ffi_handoff_after(&owner_lines, capture_idx, capture_binding.as_deref())
        else {
            continue;
        };
        let capture = &owner_lines[capture_idx];
        let handoff = &owner_lines[handoff_idx];
        if !js_native_ffi_changed(diff, repo_mode, rel, capture, handoff) {
            continue;
        }

        let raw = lines[handoff.idx];
        let context_before = context_before_site(lines, handoff.idx);
        let context_after = context_slice(
            lines,
            (handoff.idx + 1).min(lines.len()),
            (handoff.idx + 8).min(lines.len()),
        );
        sites.push(ScannedSite {
            site: UnsafeSite {
                location: SourceLocation::new(
                    rel.clone(),
                    handoff.line_no,
                    first_non_ws_column(raw),
                ),
                kind: UnsafeSiteKind::Operation,
                owner: Some(owner),
                visibility: visibility_for_snippet(raw.trim()).to_string(),
                public_api_surface: false,
                changed: true,
                snippet: handoff.text.clone(),
            },
            operation: UnsafeOperation {
                family: OperationFamily::StableByteSourceNativeFfiRead,
                expression: js_native_ffi_expression(capture, handoff),
            },
            context_before,
            context_after,
        });
    }
    sites
}

fn js_native_ffi_lines(lines: &[&str]) -> Vec<JsNativeFfiLine> {
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
        out.push(JsNativeFfiLine {
            idx,
            line_no: idx + 1,
            text: text.to_string(),
            owner,
        });
    }
    out
}

fn native_ffi_handoff_after(
    lines: &[JsNativeFfiLine],
    capture_idx: usize,
    capture_binding: Option<&str>,
) -> Option<usize> {
    for (idx, line) in lines.iter().enumerate().skip(capture_idx + 1) {
        if capture_binding.is_some_and(|binding| {
            line_mentions_identifier(&line.text, binding) && is_owned_byte_snapshot(&line.text)
        }) {
            return None;
        }
        if is_zstd_native_ffi_handoff(&line.text)
            && capture_binding.is_none_or(|binding| line_mentions_identifier(&line.text, binding))
        {
            return Some(idx);
        }
    }
    None
}

fn js_native_ffi_changed(
    diff: Option<&DiffIndex>,
    repo_mode: bool,
    rel: &PathBuf,
    capture: &JsNativeFfiLine,
    handoff: &JsNativeFfiLine,
) -> bool {
    diff.is_none_or(|diff| {
        repo_mode
            || diff.contains_near(rel, capture.line_no)
            || diff.contains_near(rel, handoff.line_no)
    })
}

fn is_js_backed_byte_capture(line: &str) -> bool {
    line.contains("StringOrBuffer::from_js")
        || (contains_call_name(line, "from_js")
            && contains_any(
                line,
                &[
                    "ArrayBuffer",
                    "ArrayBufferView",
                    "TypedArray",
                    "JSArrayBuffer",
                    "JSArrayBufferView",
                    "StringOrBuffer",
                ],
            ))
}

fn js_byte_capture_binding(line: &str) -> Option<String> {
    let (before_assignment, _) = line.split_once('=')?;
    let mut binding = before_assignment.trim().strip_prefix("let ")?.trim();
    binding = binding.strip_prefix("mut ").unwrap_or(binding).trim();
    let binding = binding.split(':').next().unwrap_or(binding).trim();
    is_simple_identifier(binding).then(|| binding.to_string())
}

fn is_zstd_native_ffi_handoff(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains("zstd") && contains_call_name(line, "as_ptr") && contains_call_name(line, "len")
}

fn is_owned_byte_snapshot(line: &str) -> bool {
    contains_any(
        line,
        &[
            ".to_vec()",
            ".to_owned()",
            "copy_from_slice",
            "snapshot",
            "copy_bytes",
            "owned",
        ],
    )
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

fn js_native_ffi_expression(capture: &JsNativeFfiLine, handoff: &JsNativeFfiLine) -> String {
    format!(
        "stable-byte-source-native-ffi-read candidate; proof required: observable-red-green; JS-backed bytes reach native FFI pointer/length read before snapshot; source: {}; native read: {}",
        one_line(&capture.text),
        one_line(&handoff.text)
    )
}
