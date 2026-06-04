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

#[derive(Clone, Debug)]
struct MaterializedSpan {
    line_idx: usize,
    binding: Option<String>,
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
        let Some((input_span, output_span, handoff_idx)) =
            native_ffi_byte_handoff(&owner, &owner_lines)
        else {
            continue;
        };
        let input = &owner_lines[input_span.line_idx];
        let output = &owner_lines[output_span.line_idx];
        let handoff = &owner_lines[handoff_idx];
        if !js_native_ffi_changed(diff, repo_mode, rel, input, output, handoff) {
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
                expression: js_native_ffi_expression(input, output, handoff),
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

fn native_ffi_byte_handoff(
    owner: &str,
    lines: &[JsNativeFfiLine],
) -> Option<(MaterializedSpan, MaterializedSpan, usize)> {
    for (handoff_idx, handoff) in lines.iter().enumerate() {
        if !is_native_ffi_handoff(owner, &handoff.text) {
            continue;
        }
        for input_idx in (0..handoff_idx).rev() {
            if !is_native_input_materialization(&lines[input_idx].text) {
                continue;
            }
            let input = MaterializedSpan {
                line_idx: input_idx,
                binding: binding_name(&lines[input_idx].text),
            };
            if !handoff_mentions_binding(&handoff.text, input.binding.as_deref()) {
                continue;
            }
            for output_idx in (0..handoff_idx).rev() {
                if output_idx == input_idx
                    || !is_native_output_materialization(&lines[output_idx].text)
                {
                    continue;
                }
                let output = MaterializedSpan {
                    line_idx: output_idx,
                    binding: binding_name(&lines[output_idx].text),
                };
                if !handoff_mentions_binding(&handoff.text, output.binding.as_deref()) {
                    continue;
                }
                if has_disjointness_or_copy_boundary(lines, input_idx.min(output_idx), handoff_idx)
                {
                    continue;
                }
                return Some((input, output, handoff_idx));
            }
        }
    }
    None
}

fn is_native_input_materialization(line: &str) -> bool {
    contains_call_name(line, "byte_slice") && !contains_call_name(line, "byte_slice_mut")
}

fn is_native_output_materialization(line: &str) -> bool {
    contains_call_name(line, "byte_slice_mut")
}

fn is_native_ffi_handoff(owner: &str, line: &str) -> bool {
    let lower_owner = owner.to_ascii_lowercase();
    let lower_line = line.to_ascii_lowercase();
    let native_context = contains_any(
        &lower_owner,
        &["zstd", "native", "ffi", "compress", "decompress"],
    ) || contains_any(
        &lower_line,
        &["zstd", "native", "ffi", "compress", "decompress"],
    );
    native_context
        && (contains_call_name(line, "set_buffers")
            || contains_call_name(line, "compress_into")
            || lower_line.contains("zstd_compress(")
            || lower_line.contains("zstd_decompress("))
}

fn has_disjointness_or_copy_boundary(
    lines: &[JsNativeFfiLine],
    materialize_idx: usize,
    handoff_idx: usize,
) -> bool {
    lines
        .iter()
        .take(handoff_idx)
        .skip(materialize_idx + 1)
        .any(|line| is_disjointness_or_copy_boundary(&line.text))
}

fn is_disjointness_or_copy_boundary(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    contains_any(
        &lower,
        &[
            "reject_overlap",
            "ensure_disjoint",
            "assert_disjoint",
            "check_disjoint",
            "copy_to_owned",
            "copy_to_stable",
            "owned_copy",
            ".to_vec()",
            ".to_owned()",
            "copy_from_slice",
        ],
    )
}

fn binding_name(line: &str) -> Option<String> {
    let (before_assignment, _) = line.split_once('=')?;
    let mut binding = before_assignment.trim().strip_prefix("let ")?.trim();
    binding = binding.strip_prefix("mut ").unwrap_or(binding).trim();
    let binding = binding.split(':').next().unwrap_or(binding).trim();
    is_simple_identifier(binding).then(|| binding.to_string())
}

fn handoff_mentions_binding(line: &str, binding: Option<&str>) -> bool {
    binding.is_none_or(|binding| line_mentions_identifier(line, binding))
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

fn js_native_ffi_changed(
    diff: Option<&DiffIndex>,
    repo_mode: bool,
    rel: &PathBuf,
    input: &JsNativeFfiLine,
    output: &JsNativeFfiLine,
    handoff: &JsNativeFfiLine,
) -> bool {
    diff.is_none_or(|diff| {
        repo_mode
            || diff.contains_near(rel, input.line_no)
            || diff.contains_near(rel, output.line_no)
            || diff.contains_near(rel, handoff.line_no)
    })
}

fn js_native_ffi_expression(
    input: &JsNativeFfiLine,
    output: &JsNativeFfiLine,
    handoff: &JsNativeFfiLine,
) -> String {
    format!(
        "stable-byte-source-native-ffi-read candidate; proof required: observable-red-green; JS-backed input and mutable output byte spans reach native FFI handoff before disjointness or copy boundary; input: {}; output: {}; handoff: {}",
        one_line(&input.text),
        one_line(&output.text),
        one_line(&handoff.text)
    )
}
