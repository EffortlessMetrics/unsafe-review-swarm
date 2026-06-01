use super::owner_context::{context_before_site, find_owner};
use super::text_detection::{LineCommentState, line_for_text_detection};
use super::{
    ScannedSite, contains_any, contains_call_name, context_slice, first_non_ws_column, one_line,
    visibility_for_snippet,
};
use crate::domain::{OperationFamily, SourceLocation, UnsafeOperation, UnsafeSite, UnsafeSiteKind};
use crate::input::diff::DiffIndex;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

#[derive(Clone, Debug)]
struct JsBufferLine {
    idx: usize,
    line_no: usize,
    text: String,
    owner: String,
}

pub(super) fn detect_js_buffer_reentry_sites(
    rel: &PathBuf,
    diff: Option<&DiffIndex>,
    repo_mode: bool,
    lines: &[&str],
) -> Vec<ScannedSite> {
    let signals = js_buffer_reentry_lines(lines);
    let helper_materializers = js_buffer_materializer_owners(&signals);
    let mut by_owner = BTreeMap::<String, Vec<JsBufferLine>>::new();
    for signal in signals {
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
            .position(|line| is_js_buffer_descriptor_capture(&line.text))
        else {
            continue;
        };
        let Some(reentry_idx) = owner_lines
            .iter()
            .enumerate()
            .skip(capture_idx + 1)
            .find_map(|(idx, line)| is_possible_js_reentry(&line.text).then_some(idx))
        else {
            continue;
        };
        let Some(materialize_idx) = js_buffer_materialization_after_reentry(
            &owner,
            &owner_lines,
            reentry_idx,
            &helper_materializers,
        ) else {
            continue;
        };
        let capture = &owner_lines[capture_idx];
        let reentry = &owner_lines[reentry_idx];
        let materialize = &owner_lines[materialize_idx];
        if !js_buffer_reentry_changed(diff, repo_mode, rel, capture, reentry, materialize) {
            continue;
        }

        let raw = lines[materialize.idx];
        let context_before = context_before_site(lines, materialize.idx);
        let context_after = context_slice(
            lines,
            (materialize.idx + 1).min(lines.len()),
            (materialize.idx + 8).min(lines.len()),
        );
        sites.push(ScannedSite {
            site: UnsafeSite {
                location: SourceLocation::new(
                    rel.clone(),
                    materialize.line_no,
                    first_non_ws_column(raw),
                ),
                kind: UnsafeSiteKind::Operation,
                owner: Some(owner),
                visibility: visibility_for_snippet(raw.trim()).to_string(),
                public_api_surface: false,
                changed: true,
                snippet: materialize.text.clone(),
            },
            operation: UnsafeOperation {
                family: OperationFamily::UnsafeFnCall,
                expression: js_buffer_reentry_expression(capture, reentry, materialize),
            },
            context_before,
            context_after,
        });
    }
    sites
}

fn js_buffer_reentry_lines(lines: &[&str]) -> Vec<JsBufferLine> {
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
        out.push(JsBufferLine {
            idx,
            line_no: idx + 1,
            text: text.to_string(),
            owner,
        });
    }
    out
}

fn js_buffer_materializer_owners(lines: &[JsBufferLine]) -> BTreeSet<String> {
    lines
        .iter()
        .filter(|line| is_js_buffer_materialization(&line.text))
        .map(|line| line.owner.clone())
        .collect()
}

fn js_buffer_materialization_after_reentry(
    owner: &str,
    lines: &[JsBufferLine],
    reentry_idx: usize,
    helper_materializers: &BTreeSet<String>,
) -> Option<usize> {
    lines
        .iter()
        .enumerate()
        .skip(reentry_idx + 1)
        .find_map(|(idx, line)| {
            (is_js_buffer_materialization(&line.text)
                || calls_js_buffer_materializer_helper(owner, &line.text, helper_materializers))
            .then_some(idx)
        })
}

fn calls_js_buffer_materializer_helper(
    owner: &str,
    line: &str,
    helper_materializers: &BTreeSet<String>,
) -> bool {
    helper_materializers
        .iter()
        .filter(|helper| helper.as_str() != owner)
        .any(|helper| contains_call_name(line, helper))
}

fn js_buffer_reentry_changed(
    diff: Option<&DiffIndex>,
    repo_mode: bool,
    rel: &PathBuf,
    capture: &JsBufferLine,
    reentry: &JsBufferLine,
    materialize: &JsBufferLine,
) -> bool {
    diff.is_none_or(|diff| {
        repo_mode
            || diff.contains_near(rel, capture.line_no)
            || diff.contains_near(rel, reentry.line_no)
            || diff.contains_near(rel, materialize.line_no)
    })
}

fn is_js_buffer_descriptor_capture(line: &str) -> bool {
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

fn is_possible_js_reentry(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains("getter")
        || lower.contains("get_by_id")
        || lower.contains("getbyid")
        || lower.contains("get_property")
        || lower.contains("getownproperty")
        || lower.contains("get_own_property")
        || lower.contains("parse_options")
        || lower.contains("callback")
        || lower.contains("call_function")
        || lower.contains(".call(")
        || ((lower.contains("options") || lower.contains("opts"))
            && (contains_call_name(line, "get") || lower.contains(".get(")))
        || ((lower.contains("globalobject") || lower.contains("global_object"))
            && contains_call_name(line, "get"))
}

fn is_js_buffer_materialization(line: &str) -> bool {
    contains_call_name(line, "byte_slice")
        || contains_call_name(line, "byte_slice_mut")
        || contains_call_name(line, "from_raw_parts")
        || contains_call_name(line, "from_raw_parts_mut")
}

fn js_buffer_reentry_expression(
    capture: &JsBufferLine,
    reentry: &JsBufferLine,
    materialize: &JsBufferLine,
) -> String {
    format!(
        "JS-backed buffer descriptor captured before possible JS reentry and materialized afterward; capture: {}; reentry: {}; materialize: {}",
        one_line(&capture.text),
        one_line(&reentry.text),
        one_line(&materialize.text)
    )
}
