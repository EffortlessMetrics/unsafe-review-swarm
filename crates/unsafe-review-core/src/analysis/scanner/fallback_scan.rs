use super::{
    ScannedSite, detect_site, is_incomplete_multiline_copy, is_incomplete_multiline_transmute,
    is_incomplete_multiline_transmute_copy, line_for_text_detection, scan_site, site_key,
    syntax_operation_covers_fallback, syntax_scan::SyntaxSiteIndex, syntax_site_covers_fallback,
};
use crate::domain::{OperationFamily, UnsafeSiteKind};
use crate::input::diff::DiffIndex;
use std::collections::BTreeSet;
use std::path::PathBuf;

pub(super) fn sites(
    rel: &PathBuf,
    diff: Option<&DiffIndex>,
    repo_mode: bool,
    lines: &[&str],
    syntax_sites: &[super::DetectedSyntaxSite],
    syntax_index: &SyntaxSiteIndex,
    seen: &mut BTreeSet<(usize, String, String)>,
) -> Vec<ScannedSite> {
    let in_unsafe_block = unsafe_block_scope_per_line(lines);
    let mut out = Vec::new();
    let mut line_comment_state = super::LineCommentState::default();
    for (idx, raw) in lines.iter().enumerate() {
        let line_no = idx + 1;
        let trimmed = raw.trim();
        let detection_line = line_for_text_detection(raw, &mut line_comment_state);
        let detection_trimmed = detection_line.trim();
        if detection_trimmed.is_empty() {
            continue;
        }
        let Some((kind, family)) = detect_site(detection_trimmed) else {
            continue;
        };
        // Gate bare `.add`/`.offset` PointerArithmetic on syntactic unsafe scope.
        // Raw-pointer arithmetic is only legal inside `unsafe { }` blocks or `unsafe fn`
        // bodies; a match outside that scope is a false positive (e.g. safe bitflag `.add`).
        if kind == UnsafeSiteKind::Operation
            && family == OperationFamily::PointerArithmetic
            && !in_unsafe_block[idx]
            && !line_is_in_unsafe_fn(lines, idx)
        {
            continue;
        }
        if fallback_is_shadowed_by_syntax(FallbackShadowInput {
            lines,
            syntax_sites,
            syntax_index,
            idx,
            line_no,
            detection_trimmed,
            kind: &kind,
            family: &family,
        }) {
            continue;
        }
        seen.insert(site_key(line_no, &kind, &family));
        if let Some(site) = scan_site::fallback_site(scan_site::FallbackSiteInput {
            rel,
            diff,
            repo_mode,
            lines,
            idx,
            raw,
            trimmed,
            detection_trimmed,
            kind,
            family,
        }) {
            out.push(site);
        }
    }
    out
}

/// Returns a per-line boolean: `true` when line `idx` is inside an `unsafe { }` block.
///
/// Walks every detection line character-by-character, maintaining a stack of brace depths
/// at which `unsafe {` was opened.  A line is "in scope" for the purposes of this check
/// when either:
/// - the brace depth at the start of the line is already above a depth pushed by a
///   previous `unsafe {`, or
/// - `unsafe {` appears somewhere on the same line before the operation marker would
///   appear (conservatively: if any `unsafe {` is opened on this line we mark it true).
fn unsafe_block_scope_per_line(lines: &[&str]) -> Vec<bool> {
    let mut result = vec![false; lines.len()];
    // Stack of brace depths *before* the matching `unsafe {` was opened.
    // The line is in scope when current brace_depth > *stack.last().
    let mut unsafe_stack: Vec<usize> = Vec::new();
    let mut brace_depth: usize = 0;
    let mut state = super::LineCommentState::default();

    for (idx, raw) in lines.iter().enumerate() {
        let detection = line_for_text_detection(raw, &mut state);

        // Is this line already in scope from a previous `unsafe {`?
        let already_in_scope = unsafe_stack
            .last()
            .is_some_and(|&open_depth| brace_depth > open_depth);

        // Walk the detection text char-by-char to update brace state and detect
        // single-line `unsafe { ... }` patterns on the same line.
        let mut opened_unsafe_this_line = false;
        let bytes = detection.as_bytes();
        let mut byte_idx = 0usize;
        while byte_idx < bytes.len() {
            let ch = detection[byte_idx..].chars().next();
            let Some(ch) = ch else { break };
            match ch {
                '{' => {
                    // Look at what precedes this `{` on the current detection text.
                    let before = detection[..byte_idx].trim_end();
                    if before.ends_with("unsafe") {
                        unsafe_stack.push(brace_depth);
                        opened_unsafe_this_line = true;
                    }
                    brace_depth += 1;
                }
                '}' => {
                    brace_depth = brace_depth.saturating_sub(1);
                    // Pop any exhausted unsafe scopes.
                    while unsafe_stack.last().is_some_and(|&d| brace_depth <= d) {
                        unsafe_stack.pop();
                    }
                }
                _ => {}
            }
            byte_idx += ch.len_utf8();
        }

        result[idx] = already_in_scope || opened_unsafe_this_line;
    }
    result
}

/// Returns `true` when the line at `idx` is inside an `unsafe fn` body.
///
/// Scans backward to find the innermost enclosing function declaration.  If that
/// declaration contains `unsafe fn`, any line within its body is considered to be
/// in unsafe scope — raw-pointer arithmetic is legal there even without an inner
/// `unsafe { }` block (the `unsafe fn` contract covers the whole body).
fn line_is_in_unsafe_fn(lines: &[&str], idx: usize) -> bool {
    const OWNER_SCAN_LIMIT: usize = 160;
    let mut state = super::LineCommentState::default();
    for (line_idx, raw) in lines[..=idx]
        .iter()
        .enumerate()
        .rev()
        .take(OWNER_SCAN_LIMIT)
    {
        let detection = line_for_text_detection(raw, &mut state);
        let trimmed = detection.trim();
        if trimmed.is_empty() || is_comment_line(trimmed) {
            continue;
        }
        if trimmed.contains("fn ") && declaration_encloses(lines, line_idx, idx) {
            return trimmed.contains("unsafe fn ");
        }
    }
    false
}

/// Returns `true` if the declaration beginning at `decl_idx` syntactically encloses
/// the line at `idx` (i.e., its opening brace has not been closed before we reach `idx`).
fn declaration_encloses(lines: &[&str], decl_idx: usize, idx: usize) -> bool {
    if decl_idx == idx {
        return true;
    }
    let mut state = super::LineCommentState::default();
    let mut depth = 0isize;
    let mut opened = false;
    for (line_idx, raw) in lines
        .iter()
        .enumerate()
        .take(idx.saturating_add(1))
        .skip(decl_idx)
    {
        let code = line_for_text_detection(raw, &mut state);
        for ch in code.chars() {
            match ch {
                '{' => {
                    depth += 1;
                    opened = true;
                }
                '}' => {
                    depth -= 1;
                    if opened && depth <= 0 && line_idx < idx {
                        return false;
                    }
                }
                _ => {}
            }
        }
    }
    opened && depth > 0
}

fn is_comment_line(line: &str) -> bool {
    line.starts_with("//") || line.starts_with("/*") || line.starts_with('*')
}

struct FallbackShadowInput<'a> {
    lines: &'a [&'a str],
    syntax_sites: &'a [super::DetectedSyntaxSite],
    syntax_index: &'a SyntaxSiteIndex,
    idx: usize,
    line_no: usize,
    detection_trimmed: &'a str,
    kind: &'a UnsafeSiteKind,
    family: &'a OperationFamily,
}

fn fallback_is_shadowed_by_syntax(input: FallbackShadowInput<'_>) -> bool {
    syntax_site_covers_fallback(input.syntax_sites, input.line_no, input.kind, input.family)
        || (*input.kind == UnsafeSiteKind::Operation
            && *input.family == OperationFamily::Transmute
            && is_incomplete_multiline_transmute_copy(input.detection_trimmed)
            && syntax_operation_covers_fallback(input.syntax_sites, input.line_no, input.family))
        || (*input.kind == UnsafeSiteKind::Operation
            && *input.family == OperationFamily::Transmute
            && is_incomplete_multiline_transmute(input.detection_trimmed)
            && syntax_operation_covers_fallback(input.syntax_sites, input.line_no, input.family))
        || (*input.kind == UnsafeSiteKind::Operation
            && matches!(
                input.family,
                OperationFamily::PtrCopy
                    | OperationFamily::CopyNonOverlapping
                    | OperationFamily::PtrReplace
            )
            && is_incomplete_multiline_copy(input.detection_trimmed)
            && syntax_operation_covers_fallback(input.syntax_sites, input.line_no, input.family))
        || (*input.kind == UnsafeSiteKind::UnsafeBlock
            && *input.family == OperationFamily::Unknown
            && input
                .syntax_index
                .covers_specific_operation(input.line_no, input.lines, input.idx))
}
