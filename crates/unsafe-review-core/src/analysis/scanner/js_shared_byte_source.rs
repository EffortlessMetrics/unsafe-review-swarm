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
struct JsSharedByteLine {
    idx: usize,
    line_no: usize,
    text: String,
    owner: String,
}

pub(super) fn detect_js_shared_byte_sites(
    rel: &PathBuf,
    diff: Option<&DiffIndex>,
    repo_mode: bool,
    lines: &[&str],
) -> Vec<ScannedSite> {
    let mut by_owner = BTreeMap::<String, Vec<JsSharedByteLine>>::new();
    for signal in js_shared_byte_lines(lines) {
        by_owner
            .entry(signal.owner.clone())
            .or_default()
            .push(signal);
    }

    let mut sites = Vec::new();
    for (owner, mut owner_lines) in by_owner {
        owner_lines.sort_by_key(|line| line.line_no);
        let Some(shared_idx) = owner_lines
            .iter()
            .position(|line| is_shared_backing_signal(&line.text))
        else {
            continue;
        };
        let Some(materialize_idx) = shared_borrowed_materialization_after(&owner_lines, shared_idx)
        else {
            continue;
        };
        let shared = &owner_lines[shared_idx];
        let materialize = &owner_lines[materialize_idx];
        if !js_shared_byte_changed(diff, repo_mode, rel, shared, materialize) {
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
                family: OperationFamily::StableByteSourceSabRace,
                expression: js_shared_byte_expression(shared, materialize),
            },
            context_before,
            context_after,
        });
    }
    sites
}

fn js_shared_byte_lines(lines: &[&str]) -> Vec<JsSharedByteLine> {
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
        out.push(JsSharedByteLine {
            idx,
            line_no: idx + 1,
            text: text.to_string(),
            owner,
        });
    }
    out
}

fn shared_borrowed_materialization_after(
    lines: &[JsSharedByteLine],
    shared_idx: usize,
) -> Option<usize> {
    let mut saw_snapshot = false;
    for (idx, line) in lines.iter().enumerate().skip(shared_idx + 1) {
        if is_shared_byte_snapshot(&line.text) {
            saw_snapshot = true;
        }
        if is_shared_borrowed_materialization(&line.text) {
            if saw_snapshot {
                return None;
            }
            return Some(idx);
        }
    }
    None
}

fn js_shared_byte_changed(
    diff: Option<&DiffIndex>,
    repo_mode: bool,
    rel: &PathBuf,
    shared: &JsSharedByteLine,
    materialize: &JsSharedByteLine,
) -> bool {
    diff.is_none_or(|diff| {
        repo_mode
            || diff.contains_near(rel, shared.line_no)
            || diff.contains_near(rel, materialize.line_no)
    })
}

fn is_shared_backing_signal(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains("sharedarraybuffer")
        || lower.contains("shared_array_buffer")
        || lower.contains("shared_backing")
        || lower.contains("shared backing")
        || contains_call_name(line, "is_shared")
        || contains_call_name(line, "is_shared_array_buffer")
}

fn is_shared_borrowed_materialization(line: &str) -> bool {
    contains_call_name(line, "from_raw_parts")
        || contains_call_name(line, "from_raw_parts_mut")
        || (contains_call_name(line, "byte_slice")
            && contains_any(line, &["SharedArrayBuffer", "shared", "Shared"]))
}

fn is_shared_byte_snapshot(line: &str) -> bool {
    contains_any(
        line,
        &[
            ".to_vec()",
            ".to_owned()",
            "copy_from_slice",
            "snapshot",
            "copy_shared",
            "owned",
        ],
    )
}

fn js_shared_byte_expression(shared: &JsSharedByteLine, materialize: &JsSharedByteLine) -> String {
    format!(
        "stable-byte-source-sab-race candidate; proof required: mutation-plus-miri; shared JS backing reaches Rust/native borrowed-slice materialization before snapshot; source: {}; materialize: {}",
        one_line(&shared.text),
        one_line(&materialize.text)
    )
}
