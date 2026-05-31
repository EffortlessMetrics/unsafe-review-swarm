use super::atomic_pointer_state::is_atomic_pointer_state_transition;
use super::copy_operation::copy_operation_family;
use super::ffi_boundary::ffi_boundary_applicability;
use super::maybeuninit_operation::maybeuninit_operation_family;
use super::nonnull_operation::nonnull_operation_family;
use super::slice_operation::slice_operation_family;
use super::static_mut::{is_static_mut_item, parse_static_mut_name};
use super::syntax::{ParsedSource, SyntaxNodeFact};
use super::target_feature::is_target_feature_attribute;
use super::transmute_operation::{
    is_incomplete_multiline_transmute_copy, transmute_operation_family,
};
use super::unsafe_impl::{parse_impl_owner, parse_impl_trait_name};
use super::unwrap_operation::unwrap_operation_family;
use super::utf8_operation::utf8_operation_family;
use super::vec_operation::vec_operation_family;
use super::zeroed_operation::zeroed_operation_family;
use crate::domain::{OperationFamily, SourceLocation, UnsafeOperation, UnsafeSite, UnsafeSiteKind};
use crate::input::diff::DiffIndex;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

mod item_names;
mod owner_context;
mod text_detection;

use self::item_names::{parse_fn_name, parse_mod_name, parse_trait_name};
#[cfg(test)]
use self::owner_context::find_owner_declaration_index;
use self::owner_context::{
    context_before_site, find_extern_block_owner, find_following_fn_owner, find_owner,
};
use self::text_detection::{LineCommentState, line_for_text_detection};

#[derive(Clone, Debug)]
pub(crate) struct ScannedSite {
    pub(crate) site: UnsafeSite,
    pub(crate) operation: UnsafeOperation,
    pub(crate) context_before: Vec<String>,
    pub(crate) context_after: Vec<String>,
}

pub(crate) fn scan_file(
    root: &Path,
    rel: &PathBuf,
    diff: Option<&DiffIndex>,
    repo_mode: bool,
) -> Result<Vec<ScannedSite>, String> {
    let abs = root.join(rel);
    let text =
        fs::read_to_string(&abs).map_err(|err| format!("read {} failed: {err}", abs.display()))?;
    let lines: Vec<&str> = text.lines().collect();
    let parsed = super::syntax::parse_source(text.as_str());
    let extern_names = extern_fn_names(&lines);
    let local_modules = local_module_names(&lines);
    let syntax_sites = detect_syntax_sites(&parsed, &extern_names, &local_modules);
    let syntax_operation_lines = syntax_sites
        .iter()
        .filter(|site| site.kind == UnsafeSiteKind::Operation)
        .map(|site| site.line)
        .collect::<BTreeSet<_>>();
    let syntax_operation_block_lines = operation_block_start_lines(&parsed);
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    let mut line_comment_state = LineCommentState::default();
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
        if syntax_site_covers_fallback(&syntax_sites, line_no, &kind, &family) {
            continue;
        }
        if kind == UnsafeSiteKind::Operation
            && family == OperationFamily::Transmute
            && is_incomplete_multiline_transmute_copy(detection_trimmed)
            && syntax_operation_covers_fallback(&syntax_sites, line_no, &family)
        {
            continue;
        }
        if kind == UnsafeSiteKind::UnsafeBlock
            && family == OperationFamily::Unknown
            && (syntax_operation_lines.contains(&line_no)
                || syntax_operation_block_lines.contains(&line_no)
                || fallback_unsafe_block_contains_specific_operation(&lines, idx))
        {
            continue;
        }
        seen.insert(site_key(line_no, &kind, &family));
        let changed = diff.is_none_or(|d| {
            repo_mode
                || if syntax_site_uses_exact_range(&kind) {
                    d.contains_in_range(rel, line_no, line_no)
                } else {
                    d.contains_near(rel, line_no)
                }
        });
        if !changed && !repo_mode {
            continue;
        }
        let owner = match (&kind, &family) {
            (UnsafeSiteKind::ExternBlock, OperationFamily::Ffi) => {
                find_extern_block_owner(&lines, idx)
            }
            (UnsafeSiteKind::Operation, OperationFamily::TargetFeature) => {
                find_following_fn_owner(&lines, idx)
            }
            (UnsafeSiteKind::StaticMut, OperationFamily::StaticMut) => {
                parse_static_mut_name(detection_trimmed)
            }
            _ => None,
        }
        .or_else(|| find_owner(&lines, idx));
        let visibility = visibility_for_snippet(trimmed).to_string();
        let public_api_surface = is_public_api_surface(&kind, trimmed);
        let context_before = context_before_site(&lines, idx);
        let context_after = context_slice(&lines, idx + 1, (idx + 8).min(lines.len()));
        out.push(ScannedSite {
            site: UnsafeSite {
                location: SourceLocation::new(rel.clone(), line_no, first_non_ws_column(raw)),
                kind,
                owner,
                visibility,
                public_api_surface,
                changed,
                snippet: trimmed.to_string(),
            },
            operation: UnsafeOperation {
                family,
                expression: trimmed.to_string(),
            },
            context_before,
            context_after,
        });
    }

    for detected in syntax_sites {
        if detected.kind == UnsafeSiteKind::UnsafeBlock
            && detected.family == OperationFamily::Unknown
            && syntax_operation_lines.contains(&detected.line)
        {
            continue;
        }
        if !seen.insert(site_key(detected.line, &detected.kind, &detected.family)) {
            continue;
        }
        let changed = diff.is_none_or(|d| {
            repo_mode
                || if syntax_site_uses_exact_range(&detected.kind) {
                    d.contains_in_range(rel, detected.line, detected.end_line)
                } else {
                    d.contains_near(rel, detected.line)
                }
        });
        if !changed && !repo_mode {
            continue;
        }
        let idx = detected.line.saturating_sub(1);
        let owner = syntax_owner(&detected, &lines, idx);
        let visibility = visibility_for_snippet(&detected.source_snippet).to_string();
        let public_api_surface = is_public_api_surface(&detected.kind, &detected.source_snippet);
        let context_before = context_before_site(&lines, idx);
        let context_after = context_slice(
            &lines,
            (idx + 1).min(lines.len()),
            (idx + 8).min(lines.len()),
        );
        out.push(ScannedSite {
            site: UnsafeSite {
                location: SourceLocation::new(rel.clone(), detected.line, detected.column),
                kind: detected.kind,
                owner,
                visibility,
                public_api_surface,
                changed,
                snippet: detected.card_snippet.clone(),
            },
            operation: UnsafeOperation {
                family: detected.family,
                expression: detected.card_snippet,
            },
            context_before,
            context_after,
        });
    }
    out.extend(detect_js_buffer_reentry_sites(rel, diff, repo_mode, &lines));
    out.sort_by(|left, right| {
        left.site
            .location
            .line
            .cmp(&right.site.location.line)
            .then(left.site.location.column.cmp(&right.site.location.column))
    });
    Ok(out)
}

#[derive(Clone, Debug)]
struct JsBufferLine {
    idx: usize,
    line_no: usize,
    text: String,
    owner: String,
}

fn detect_js_buffer_reentry_sites(
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

fn contains_any(line: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| line.contains(needle))
}

fn one_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn first_non_ws_column(line: &str) -> usize {
    line.chars()
        .position(|ch| !ch.is_whitespace())
        .map_or(1, |pos| pos + 1)
}

fn context_slice(lines: &[&str], start: usize, end: usize) -> Vec<String> {
    lines[start..end]
        .iter()
        .map(|line| line.trim().to_string())
        .collect()
}

fn detect_site(line: &str) -> Option<(UnsafeSiteKind, OperationFamily)> {
    if line.contains("unsafe impl") {
        return Some(match parse_impl_trait_name(line).as_deref() {
            Some("Send") => (
                UnsafeSiteKind::UnsafeImplSend,
                OperationFamily::UnsafeImplSendSync,
            ),
            Some("Sync") => (
                UnsafeSiteKind::UnsafeImplSync,
                OperationFamily::UnsafeImplSendSync,
            ),
            _ => (UnsafeSiteKind::UnsafeImpl, OperationFamily::Unknown),
        });
    }
    if line.contains("unsafe fn") {
        return Some((UnsafeSiteKind::UnsafeFn, OperationFamily::Unknown));
    }
    if line.contains("unsafe trait") {
        return Some((UnsafeSiteKind::UnsafeTrait, OperationFamily::Unknown));
    }
    if is_extern_boundary(line) {
        return Some((UnsafeSiteKind::ExternBlock, OperationFamily::Ffi));
    }
    if is_static_mut_item(line) {
        return Some((UnsafeSiteKind::StaticMut, OperationFamily::StaticMut));
    }
    if is_import_item(line) {
        return None;
    }
    if let Some(family) = detect_operation_family(line) {
        return Some((UnsafeSiteKind::Operation, family));
    }
    if let Some(family) = utf8_operation_family(line) {
        return Some((UnsafeSiteKind::Operation, family));
    }
    if let Some(family) = maybeuninit_operation_family(line) {
        return Some((UnsafeSiteKind::Operation, family));
    }
    if let Some(family) = transmute_operation_family(line) {
        return Some((UnsafeSiteKind::Operation, family));
    }
    if let Some(family) = zeroed_operation_family(line) {
        return Some((UnsafeSiteKind::Operation, family));
    }
    if contains_call_name(line, "drop_in_place") {
        return Some((UnsafeSiteKind::Operation, OperationFamily::DropInPlace));
    }
    if is_atomic_pointer_state_transition(line) {
        return Some((
            UnsafeSiteKind::Operation,
            OperationFamily::AtomicPointerState,
        ));
    }
    if contains_call_name(line, "unreachable_unchecked") {
        return Some((
            UnsafeSiteKind::Operation,
            OperationFamily::UnreachableUnchecked,
        ));
    }
    if let Some(family) = unwrap_operation_family(line) {
        return Some((UnsafeSiteKind::Operation, family));
    }
    if contains_call_name(line, "from_raw") {
        return Some((UnsafeSiteKind::Operation, OperationFamily::BoxFromRaw));
    }
    if contains_call_name(line, "new_unchecked") && line.contains("Pin") {
        return Some((UnsafeSiteKind::Operation, OperationFamily::PinUnchecked));
    }
    if contains_call_name(line, "get_unchecked") || contains_call_name(line, "get_unchecked_mut") {
        return Some((UnsafeSiteKind::Operation, OperationFamily::GetUnchecked));
    }
    if let Some(family) = nonnull_operation_family(line) {
        return Some((UnsafeSiteKind::Operation, family));
    }
    if contains_call_name(line, "new_unchecked") {
        return Some((UnsafeSiteKind::Operation, OperationFamily::UnsafeFnCall));
    }
    if line.contains(".read_unaligned()") || line.contains("ptr::read_unaligned") {
        return Some((
            UnsafeSiteKind::Operation,
            OperationFamily::RawPointerReadUnaligned,
        ));
    }
    if is_raw_pointer_write_unaligned(line) {
        return Some((
            UnsafeSiteKind::Operation,
            OperationFamily::RawPointerWriteUnaligned,
        ));
    }
    if line.contains(".read()") || line.contains(".read_volatile(") || line.contains("ptr::read") {
        return Some((UnsafeSiteKind::Operation, OperationFamily::RawPointerRead));
    }
    if is_raw_pointer_write(line) {
        return Some((UnsafeSiteKind::Operation, OperationFamily::RawPointerWrite));
    }
    if line.contains(".add(") || line.contains(".offset(") {
        return Some((
            UnsafeSiteKind::Operation,
            OperationFamily::PointerArithmetic,
        ));
    }
    if line.contains("asm!") {
        return Some((UnsafeSiteKind::Operation, OperationFamily::InlineAsm));
    }
    if unsafe_block_contains_call(line) {
        return Some((UnsafeSiteKind::Operation, OperationFamily::UnsafeFnCall));
    }
    if is_target_feature_attribute(line) {
        return Some((UnsafeSiteKind::Operation, OperationFamily::TargetFeature));
    }
    if line.contains("unsafe {") || line == "unsafe" {
        return Some((UnsafeSiteKind::UnsafeBlock, OperationFamily::Unknown));
    }
    None
}

fn is_extern_boundary(line: &str) -> bool {
    let trimmed = line.trim_start();
    if trimmed.starts_with("extern crate") || trimmed.starts_with("pub extern crate") {
        return false;
    }
    trimmed.contains("extern \"")
        || trimmed.starts_with("unsafe extern {")
        || trimmed.starts_with("extern {")
}

fn extern_fn_names(lines: &[&str]) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut in_extern = false;
    let mut module_stack: Vec<(String, usize)> = Vec::new();
    let mut brace_depth = 0usize;
    let mut state = LineCommentState::default();
    for raw in lines {
        let detection_line = line_for_text_detection(raw, &mut state);
        let trimmed = detection_line.trim();
        if trimmed.is_empty() {
            continue;
        }
        while module_stack
            .last()
            .is_some_and(|(_name, depth)| brace_depth < *depth)
        {
            module_stack.pop();
        }
        if trimmed.contains('{')
            && let Some(module_name) = parse_mod_name(trimmed)
        {
            module_stack.push((module_name, brace_depth + 1));
        }
        if is_extern_boundary(trimmed) {
            in_extern = true;
        }
        if in_extern {
            if let Some(name) = parse_fn_name(trimmed) {
                insert_extern_call_paths(&mut names, &module_stack, &name);
            }
            if trimmed.contains('}') {
                in_extern = false;
            }
        }
        brace_depth = update_brace_depth(trimmed, brace_depth);
    }
    names
}

fn local_module_names(lines: &[&str]) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut state = LineCommentState::default();
    for raw in lines {
        let detection_line = line_for_text_detection(raw, &mut state);
        let trimmed = detection_line.trim();
        if let Some(name) = parse_mod_name(trimmed) {
            names.insert(name);
        }
    }
    names
}

fn insert_extern_call_paths(
    names: &mut BTreeSet<String>,
    module_stack: &[(String, usize)],
    name: &str,
) {
    if module_stack.is_empty() {
        names.insert(name.to_string());
        return;
    }
    let module_path = module_stack
        .iter()
        .map(|(module, _depth)| module.as_str())
        .collect::<Vec<_>>()
        .join("::");
    let qualified = format!("{module_path}::{name}");
    names.insert(qualified.clone());
    names.insert(format!("crate::{qualified}"));
    names.insert(format!("self::{qualified}"));
}

fn update_brace_depth(line: &str, mut depth: usize) -> usize {
    for ch in line.chars() {
        match ch {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    depth
}

fn is_import_item(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("use ")
        || trimmed.starts_with("pub use ")
        || (trimmed.starts_with("pub(") && trimmed.contains(" use "))
}

fn detect_operation_family(line: &str) -> Option<OperationFamily> {
    if let Some(family) = copy_operation_family(line) {
        return Some(family);
    }
    if let Some(family) = vec_operation_family(line) {
        return Some(family);
    }
    if let Some(family) = slice_operation_family(line) {
        return Some(family);
    }
    None
}

fn contains_call_name(line: &str, name: &str) -> bool {
    let mut cursor = line;
    while let Some(pos) = cursor.find(name) {
        let before = cursor[..pos].chars().next_back();
        let after = &cursor[pos + name.len()..];
        let starts_on_boundary = before.is_none_or(|ch| !is_ident_continue(ch));
        if starts_on_boundary && call_suffix(after) {
            return true;
        }
        cursor = &after[after
            .char_indices()
            .next()
            .map_or(after.len(), |(idx, ch)| idx + ch.len_utf8())..];
    }
    false
}

fn unsafe_block_contains_call(line: &str) -> bool {
    let Some(after_unsafe) = unsafe_keyword_tail(line) else {
        return false;
    };
    let Some((_before_block, after_open)) = after_unsafe.split_once('{') else {
        return false;
    };
    after_open.contains('(') && after_open.contains(')')
}

fn unsafe_keyword_tail(line: &str) -> Option<&str> {
    let mut cursor = line;
    while let Some(pos) = cursor.find("unsafe") {
        let before = cursor[..pos].chars().next_back();
        let after = &cursor[pos + "unsafe".len()..];
        let starts_on_boundary = before.is_none_or(|ch| !is_ident_continue(ch));
        let ends_on_boundary = after.chars().next().is_none_or(|ch| !is_ident_continue(ch));
        if starts_on_boundary && ends_on_boundary {
            return Some(after);
        }
        cursor = &after[after
            .char_indices()
            .next()
            .map_or(after.len(), |(idx, ch)| idx + ch.len_utf8())..];
    }
    None
}

fn call_suffix(after_name: &str) -> bool {
    let rest = after_name.trim_start();
    if rest.starts_with('(') {
        return true;
    }
    rest.strip_prefix("::")
        .is_some_and(|after_colons| after_colons.trim_start().starts_with('<'))
}

fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

#[derive(Clone, Debug)]
struct DetectedSyntaxSite {
    line: usize,
    end_line: usize,
    column: usize,
    start: usize,
    end: usize,
    kind: UnsafeSiteKind,
    family: OperationFamily,
    card_snippet: String,
    source_snippet: String,
}

fn detect_syntax_sites(
    parsed: &ParsedSource,
    extern_names: &BTreeSet<String>,
    local_modules: &BTreeSet<String>,
) -> Vec<DetectedSyntaxSite> {
    let mut sites = Vec::new();
    let unsafe_block_ranges = unsafe_block_ranges(parsed);
    let operation_block_ranges = operation_block_ranges(parsed, &unsafe_block_ranges);
    for fact in &parsed.nodes {
        let Some((kind, family)) = detect_syntax_site(
            fact,
            &parsed.text,
            &unsafe_block_ranges,
            &operation_block_ranges,
            extern_names,
            local_modules,
        ) else {
            continue;
        };
        let _span_len = fact.end.saturating_sub(fact.start);
        let card_snippet = card_snippet_for(fact, &kind, &family, &parsed.text);
        sites.push(DetectedSyntaxSite {
            line: fact.line,
            end_line: fact.line + fact.snippet.lines().count().saturating_sub(1),
            column: fact.column,
            start: fact.start,
            end: fact.end,
            kind,
            family,
            card_snippet,
            source_snippet: fact.snippet.clone(),
        });
    }
    sites = without_parent_duplicate_operations(sites);
    sites.sort_by(|left, right| {
        left.line
            .cmp(&right.line)
            .then(left.column.cmp(&right.column))
    });
    sites
}

fn without_parent_duplicate_operations(sites: Vec<DetectedSyntaxSite>) -> Vec<DetectedSyntaxSite> {
    let mut operation_indices: Vec<usize> = sites
        .iter()
        .enumerate()
        .filter_map(|(index, site)| (site.kind == UnsafeSiteKind::Operation).then_some(index))
        .collect();
    operation_indices.sort_by(|left, right| {
        sites[*left]
            .family
            .as_str()
            .cmp(sites[*right].family.as_str())
            .then(sites[*left].start.cmp(&sites[*right].start))
            .then(sites[*right].end.cmp(&sites[*left].end))
    });

    let mut parent_duplicate = vec![false; sites.len()];
    let mut active_ranges: Vec<(usize, OperationFamily, usize, usize)> = Vec::new();

    for index in operation_indices {
        let site = &sites[index];
        while let Some((_index, family, _start, end)) = active_ranges.last() {
            if *family != site.family || *end <= site.start {
                active_ranges.pop();
                continue;
            }
            break;
        }

        for (parent_index, family, start, end) in &active_ranges {
            if *family == site.family && *start < site.start && site.end < *end {
                parent_duplicate[*parent_index] = true;
            }
        }

        active_ranges.push((index, site.family.clone(), site.start, site.end));
    }

    sites
        .into_iter()
        .enumerate()
        .filter_map(|(index, site)| (!parent_duplicate[index]).then_some(site))
        .collect()
}

fn syntax_owner(site: &DetectedSyntaxSite, lines: &[&str], idx: usize) -> Option<String> {
    match site.kind {
        UnsafeSiteKind::UnsafeFn => parse_fn_name(&site.source_snippet),
        UnsafeSiteKind::UnsafeTrait => parse_trait_name(&site.source_snippet),
        UnsafeSiteKind::UnsafeImpl
        | UnsafeSiteKind::UnsafeImplSend
        | UnsafeSiteKind::UnsafeImplSync => parse_impl_owner(&site.source_snippet),
        UnsafeSiteKind::ExternBlock => {
            parse_fn_name(&site.source_snippet).or_else(|| find_extern_block_owner(lines, idx))
        }
        UnsafeSiteKind::StaticMut => parse_static_mut_name(&site.source_snippet),
        UnsafeSiteKind::Operation if site.family == OperationFamily::TargetFeature => {
            find_following_fn_owner(lines, idx)
        }
        _ => None,
    }
    .or_else(|| find_owner(lines, idx))
}

fn syntax_site_covers_fallback(
    syntax_sites: &[DetectedSyntaxSite],
    line: usize,
    kind: &UnsafeSiteKind,
    family: &OperationFamily,
) -> bool {
    if *kind == UnsafeSiteKind::Operation && *family == OperationFamily::UnsafeFnCall {
        return syntax_sites.iter().any(|site| {
            site.kind == UnsafeSiteKind::FfiCall
                && site.family == OperationFamily::Ffi
                && site.line <= line
                && line <= site.end_line
        });
    }
    if !matches!(
        kind,
        UnsafeSiteKind::UnsafeFn
            | UnsafeSiteKind::UnsafeTrait
            | UnsafeSiteKind::UnsafeImpl
            | UnsafeSiteKind::UnsafeImplSend
            | UnsafeSiteKind::UnsafeImplSync
            | UnsafeSiteKind::ExternBlock
            | UnsafeSiteKind::StaticMut
    ) {
        return false;
    }
    syntax_sites.iter().any(|site| {
        site.kind == *kind && site.family == *family && site.line <= line && line <= site.end_line
    })
}

fn syntax_operation_covers_fallback(
    syntax_sites: &[DetectedSyntaxSite],
    line: usize,
    family: &OperationFamily,
) -> bool {
    syntax_sites.iter().any(|site| {
        site.kind == UnsafeSiteKind::Operation
            && site.family == *family
            && site.line <= line
            && line <= site.end_line
    })
}

fn fallback_unsafe_block_contains_specific_operation(lines: &[&str], start_idx: usize) -> bool {
    let mut line_comment_state = LineCommentState::default();
    let mut entered = false;
    let mut saw_open = false;
    let mut depth = 0usize;
    let mut block_text = String::new();

    for (idx, raw) in lines.iter().enumerate().skip(start_idx).take(80) {
        let detection_line = line_for_text_detection(raw, &mut line_comment_state);
        let trimmed = detection_line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if !entered {
            if trimmed.contains("unsafe {") || trimmed == "unsafe" {
                entered = true;
            } else {
                continue;
            }
        } else if fallback_line_is_specific_operation(trimmed) {
            return true;
        } else {
            if !block_text.is_empty() {
                block_text.push(' ');
            }
            block_text.push_str(trimmed);
            if fallback_line_is_specific_operation(&block_text) {
                return true;
            }
        }

        for ch in trimmed.chars() {
            match ch {
                '{' => {
                    saw_open = true;
                    depth += 1;
                }
                '}' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
        if idx > start_idx && saw_open && depth == 0 {
            return false;
        }
    }
    false
}

fn fallback_line_is_specific_operation(trimmed: &str) -> bool {
    matches!(
        detect_site(trimmed),
        Some((UnsafeSiteKind::Operation, family))
            if !matches!(family, OperationFamily::Unknown | OperationFamily::UnsafeFnCall)
    )
}

fn syntax_site_uses_exact_range(kind: &UnsafeSiteKind) -> bool {
    matches!(
        kind,
        UnsafeSiteKind::UnsafeFn
            | UnsafeSiteKind::UnsafeTrait
            | UnsafeSiteKind::UnsafeImpl
            | UnsafeSiteKind::UnsafeImplSend
            | UnsafeSiteKind::UnsafeImplSync
            | UnsafeSiteKind::ExternBlock
            | UnsafeSiteKind::StaticMut
    )
}

fn detect_syntax_site(
    fact: &SyntaxNodeFact,
    source: &str,
    unsafe_block_ranges: &[(usize, usize)],
    operation_block_ranges: &BTreeSet<(usize, usize)>,
    extern_names: &BTreeSet<String>,
    local_modules: &BTreeSet<String>,
) -> Option<(UnsafeSiteKind, OperationFamily)> {
    if !syntax_kind_can_be_unsafe_site(&fact.kind) {
        return None;
    }
    let compact = compact_whitespace(&fact.snippet);
    if compact.starts_with("//") {
        return None;
    }
    let declaration = declaration_prefix(&compact);
    match fact.kind.as_str() {
        "FN" if declaration.contains("unsafe fn") => {
            Some((UnsafeSiteKind::UnsafeFn, OperationFamily::Unknown))
        }
        "TRAIT" if declaration.contains("unsafe trait") => {
            Some((UnsafeSiteKind::UnsafeTrait, OperationFamily::Unknown))
        }
        "IMPL"
            if declaration.contains("unsafe impl")
                && parse_impl_trait_name(declaration).as_deref() == Some("Send") =>
        {
            Some((
                UnsafeSiteKind::UnsafeImplSend,
                OperationFamily::UnsafeImplSendSync,
            ))
        }
        "IMPL"
            if declaration.contains("unsafe impl")
                && parse_impl_trait_name(declaration).as_deref() == Some("Sync") =>
        {
            Some((
                UnsafeSiteKind::UnsafeImplSync,
                OperationFamily::UnsafeImplSendSync,
            ))
        }
        "IMPL" if declaration.contains("unsafe impl") => {
            Some((UnsafeSiteKind::UnsafeImpl, OperationFamily::Unknown))
        }
        "EXTERN_BLOCK" if compact.contains("extern") => {
            Some((UnsafeSiteKind::ExternBlock, OperationFamily::Ffi))
        }
        "STATIC" if compact.contains("static mut") => {
            Some((UnsafeSiteKind::StaticMut, OperationFamily::StaticMut))
        }
        "BLOCK_EXPR"
            if compact.starts_with("unsafe {")
                && !operation_block_ranges.contains(&(fact.start, fact.end))
                && ffi_boundary_applicability(&compact, extern_names, local_modules) =>
        {
            Some((UnsafeSiteKind::FfiCall, OperationFamily::Ffi))
        }
        "BLOCK_EXPR"
            if compact.starts_with("unsafe {")
                && !operation_block_ranges.contains(&(fact.start, fact.end))
                && unsafe_block_contains_call(&compact) =>
        {
            Some((UnsafeSiteKind::Operation, OperationFamily::UnsafeFnCall))
        }
        "BLOCK_EXPR"
            if is_unknown_unsafe_block(&compact)
                && !operation_block_ranges.contains(&(fact.start, fact.end)) =>
        {
            Some((UnsafeSiteKind::UnsafeBlock, OperationFamily::Unknown))
        }
        "PREFIX_EXPR"
            if is_raw_pointer_deref(&compact) && is_inside_range(fact, unsafe_block_ranges) =>
        {
            let family = if prefix_deref_is_assignment_target(fact, source) {
                OperationFamily::RawPointerWrite
            } else {
                OperationFamily::RawPointerDeref
            };
            Some((UnsafeSiteKind::Operation, family))
        }
        "CALL_EXPR" | "METHOD_CALL_EXPR" | "MACRO_EXPR" => {
            detect_site(&syntax_detection_text(&compact))
        }
        _ => None,
    }
}

fn syntax_kind_can_be_unsafe_site(kind: &str) -> bool {
    matches!(
        kind,
        "FN" | "TRAIT"
            | "IMPL"
            | "EXTERN_BLOCK"
            | "STATIC"
            | "BLOCK_EXPR"
            | "PREFIX_EXPR"
            | "CALL_EXPR"
            | "METHOD_CALL_EXPR"
            | "MACRO_EXPR"
    )
}

fn declaration_prefix(compact: &str) -> &str {
    compact
        .split_once('{')
        .map_or(compact, |(declaration, _body)| declaration.trim())
}

fn card_snippet_for(
    fact: &SyntaxNodeFact,
    kind: &UnsafeSiteKind,
    family: &OperationFamily,
    source: &str,
) -> String {
    let compact = compact_whitespace(&fact.snippet);
    match kind {
        UnsafeSiteKind::UnsafeBlock => "unsafe {".to_string(),
        UnsafeSiteKind::UnsafeFn
        | UnsafeSiteKind::UnsafeTrait
        | UnsafeSiteKind::UnsafeImpl
        | UnsafeSiteKind::UnsafeImplSend
        | UnsafeSiteKind::UnsafeImplSync
        | UnsafeSiteKind::ExternBlock => compact
            .split_once('{')
            .map_or(compact.clone(), |(head, _tail)| {
                format!("{} {{", head.trim())
            }),
        UnsafeSiteKind::Operation if family == &OperationFamily::RawPointerWrite => {
            source_line_at(source, fact.start)
                .map(|line| compact_whitespace(line.trim()))
                .filter(|line| !line.is_empty())
                .unwrap_or_else(|| normalize_call_spacing(&compact))
        }
        UnsafeSiteKind::FfiCall => normalize_call_spacing(&compact),
        UnsafeSiteKind::Operation => normalize_call_spacing(&compact),
        _ => compact,
    }
}

fn is_unknown_unsafe_block(compact: &str) -> bool {
    compact.starts_with("unsafe {")
        && !matches!(
            detect_site(compact),
            Some((UnsafeSiteKind::Operation, _family))
        )
}

fn is_raw_pointer_deref(compact: &str) -> bool {
    compact.starts_with('*') && !compact.starts_with("**")
}

fn prefix_deref_is_assignment_target(fact: &SyntaxNodeFact, source: &str) -> bool {
    let Some(rest) = source.get(fact.end..) else {
        return false;
    };
    let mut rest = rest.trim_start();
    while let Some(after_paren) = rest.strip_prefix(')') {
        rest = after_paren.trim_start();
    }
    starts_with_assignment_operator(rest)
}

fn source_line_at(source: &str, offset: usize) -> Option<&str> {
    let offset = offset.min(source.len());
    let start = source[..offset].rfind('\n').map_or(0, |idx| idx + 1);
    let end = source[offset..]
        .find('\n')
        .map_or(source.len(), |idx| offset + idx);
    source.get(start..end)
}

fn is_raw_pointer_write(line: &str) -> bool {
    line.contains("ptr::write")
        || line.contains("ptr::write_volatile")
        || line.contains("ptr::write_bytes")
        || line.contains(".write_volatile(")
        || line.contains("ptr.write(")
        || line.contains("ptr.write_volatile(")
        || line.contains("ptr.write_bytes(")
        || line.contains(".as_mut_ptr().write(")
        || line.contains(".as_mut_ptr().write_volatile(")
        || line.contains(".as_mut_ptr().write_bytes")
        || line.contains(".cast_mut().write(")
        || line.contains(".cast_mut().write_volatile(")
        || line.contains(".cast_mut().write_bytes")
        || (line.contains(".cast::<") && line.contains(".write("))
        || (line.contains(".cast::<") && line.contains(".write_volatile("))
        || (line.contains(".cast::<") && line.contains(".write_bytes"))
}

fn is_raw_pointer_write_unaligned(line: &str) -> bool {
    line.contains("ptr::write_unaligned") || line.contains(".write_unaligned(")
}

fn assignment_operator_start(text: &str) -> Option<usize> {
    const COMPOUND_ASSIGNMENTS: &[&str] =
        &["+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "<<=", ">>="];
    for operator in COMPOUND_ASSIGNMENTS {
        if let Some(idx) = text.find(operator) {
            return Some(idx);
        }
    }
    for (idx, ch) in text.char_indices() {
        if ch != '=' {
            continue;
        }
        let previous = text[..idx].chars().next_back();
        let next = text[idx + ch.len_utf8()..].chars().next();
        if !matches!(previous, Some('=' | '!' | '<' | '>')) && !matches!(next, Some('=' | '>')) {
            return Some(idx);
        }
    }
    None
}

fn starts_with_assignment_operator(text: &str) -> bool {
    assignment_operator_start(text).is_some_and(|idx| idx == 0)
}

fn unsafe_block_ranges(parsed: &ParsedSource) -> Vec<(usize, usize)> {
    parsed
        .nodes
        .iter()
        .filter(|fact| {
            fact.kind == "BLOCK_EXPR" && compact_whitespace(&fact.snippet).starts_with("unsafe {")
        })
        .map(|fact| (fact.start, fact.end))
        .collect()
}

fn operation_block_ranges(
    parsed: &ParsedSource,
    unsafe_block_ranges: &[(usize, usize)],
) -> BTreeSet<(usize, usize)> {
    parsed
        .nodes
        .iter()
        .filter(|fact| syntax_operation_in_unsafe_block(fact, unsafe_block_ranges))
        .filter_map(|fact| containing_range(fact, unsafe_block_ranges))
        .collect()
}

fn operation_block_start_lines(parsed: &ParsedSource) -> BTreeSet<usize> {
    let unsafe_block_ranges = unsafe_block_ranges(parsed);
    let operation_block_ranges = operation_block_ranges(parsed, &unsafe_block_ranges);
    parsed
        .nodes
        .iter()
        .filter(|fact| {
            fact.kind == "BLOCK_EXPR" && operation_block_ranges.contains(&(fact.start, fact.end))
        })
        .map(|fact| fact.line)
        .collect()
}

fn syntax_operation_in_unsafe_block(
    fact: &SyntaxNodeFact,
    unsafe_block_ranges: &[(usize, usize)],
) -> bool {
    if !is_inside_range(fact, unsafe_block_ranges) {
        return false;
    }
    let compact = compact_whitespace(&fact.snippet);
    match fact.kind.as_str() {
        "PREFIX_EXPR" => is_raw_pointer_deref(&compact),
        "CALL_EXPR" | "METHOD_CALL_EXPR" | "MACRO_EXPR" => {
            matches!(
                detect_site(&syntax_detection_text(&compact)),
                Some((UnsafeSiteKind::Operation, _family))
            )
        }
        _ => false,
    }
}

fn containing_range(fact: &SyntaxNodeFact, ranges: &[(usize, usize)]) -> Option<(usize, usize)> {
    ranges
        .iter()
        .copied()
        .find(|(start, end)| fact.start >= *start && fact.end <= *end)
}

fn is_inside_range(fact: &SyntaxNodeFact, ranges: &[(usize, usize)]) -> bool {
    ranges
        .iter()
        .any(|(start, end)| fact.start >= *start && fact.end <= *end)
}

fn compact_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_call_spacing(text: &str) -> String {
    text.replace(" (", "(")
}

fn syntax_detection_text(compact: &str) -> String {
    let mut state = LineCommentState::default();
    normalize_call_spacing(line_for_text_detection(compact, &mut state).trim())
}

fn site_key(
    line: usize,
    kind: &UnsafeSiteKind,
    family: &OperationFamily,
) -> (usize, String, String) {
    (line, kind.as_str().to_string(), family.as_str().to_string())
}

fn visibility_for_snippet(snippet: &str) -> &'static str {
    if is_public_surface(snippet) {
        "public"
    } else {
        "private"
    }
}

fn is_public_surface(snippet: &str) -> bool {
    let compact = compact_whitespace(snippet);
    starts_with_pub_visibility(&compact) || compact.contains(" pub ") || compact.contains(" pub(")
}

fn starts_with_pub_visibility(compact: &str) -> bool {
    compact.starts_with("pub ") || compact.starts_with("pub(")
}

fn is_public_api_surface(kind: &UnsafeSiteKind, snippet: &str) -> bool {
    if !matches!(
        kind,
        UnsafeSiteKind::UnsafeFn
            | UnsafeSiteKind::UnsafeTrait
            | UnsafeSiteKind::UnsafeImpl
            | UnsafeSiteKind::UnsafeImplSend
            | UnsafeSiteKind::UnsafeImplSync
    ) {
        return false;
    }
    is_public_surface(snippet)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn duplicate_operation_pruning_removes_containing_parent_operation() {
        let parent = DetectedSyntaxSite {
            line: 1,
            end_line: 1,
            column: 1,
            start: 10,
            end: 100,
            kind: UnsafeSiteKind::Operation,
            family: OperationFamily::RawPointerRead,
            card_snippet: "unsafe { ptr.read() }".to_string(),
            source_snippet: "unsafe { ptr.read() }".to_string(),
        };
        let child = DetectedSyntaxSite {
            line: 1,
            end_line: 1,
            column: 10,
            start: 25,
            end: 45,
            kind: UnsafeSiteKind::Operation,
            family: OperationFamily::RawPointerRead,
            card_snippet: "ptr.read()".to_string(),
            source_snippet: "ptr.read()".to_string(),
        };

        let sites = without_parent_duplicate_operations(vec![parent, child]);

        assert_eq!(sites.len(), 1);
        assert_eq!(sites[0].card_snippet, "ptr.read()");
    }

    #[test]
    fn text_detection_ignores_comment_only_unsafe_tokens() {
        let mut state = LineCommentState::default();

        let line = line_for_text_detection("// unsafe { ptr::read(ptr) }", &mut state);

        assert!(line.trim().is_empty());
        assert_eq!(detect_site(line.trim()), None);
    }

    #[test]
    fn text_detection_ignores_block_comment_unsafe_tokens() {
        let mut state = LineCommentState::default();

        let first = line_for_text_detection("/* unsafe {", &mut state);
        let second = line_for_text_detection("ptr::read(ptr); */ pub fn safe() {}", &mut state);

        assert!(first.trim().is_empty());
        assert_eq!(second.trim(), "pub fn safe() {}");
        assert_eq!(detect_site(second.trim()), None);
    }

    #[test]
    fn text_detection_ignores_string_literal_unsafe_tokens_but_preserves_extern_abi() {
        let mut state = LineCommentState::default();

        let string_line = line_for_text_detection(
            "let text = \"unsafe { core::ptr::read(ptr) }\";",
            &mut state,
        );
        let extern_line = line_for_text_detection("unsafe extern \"C\" {", &mut state);

        assert_eq!(detect_site(string_line.trim()), None);
        assert_eq!(
            detect_site(extern_line.trim()),
            Some((UnsafeSiteKind::ExternBlock, OperationFamily::Ffi))
        );
    }

    #[test]
    fn text_detection_ignores_unsafe_substrings_in_safe_identifiers() {
        assert_eq!(
            detect_site("let output = unsafe_review_core::analyze(AnalyzeInput { root, scope })?;"),
            None
        );
        assert_eq!(
            detect_site("after_unsafe.split_once('{').map(|(_open, after)| after)"),
            None
        );
        assert_eq!(
            detect_site("unsafe { ffi::call(ptr) }"),
            Some((UnsafeSiteKind::Operation, OperationFamily::UnsafeFnCall))
        );
    }

    #[test]
    fn text_detection_does_not_classify_extern_crate_as_ffi() {
        assert_eq!(detect_site("extern crate std;"), None);
        assert_eq!(detect_site("pub extern crate alloc;"), None);
        assert_eq!(
            detect_site("unsafe extern \"C\" {"),
            Some((UnsafeSiteKind::ExternBlock, OperationFamily::Ffi))
        );
    }

    #[test]
    fn syntax_detection_classifies_known_extern_calls_as_ffi_calls() -> Result<(), String> {
        let root = unique_temp_dir()?;
        fs::create_dir_all(root.join("src"))
            .map_err(|err| format!("create temp src failed: {err}"))?;
        fs::write(
            root.join("src/lib.rs"),
            "unsafe extern \"C\" {\n    fn strlen(ptr: *const u8) -> usize;\n}\n\npub fn len(ptr: *const u8) -> usize {\n    // SAFETY: caller provides a C string pointer.\n    unsafe { strlen(ptr) }\n}\n",
        )
        .map_err(|err| format!("write temp source failed: {err}"))?;
        let diff = crate::input::diff::parse_unified_diff(
            "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -7,1 +7,1 @@\n+    unsafe { strlen(ptr) }\n",
        );

        let rel = PathBuf::from("src/lib.rs");
        let sites = scan_file(&root, &rel, Some(&diff), false)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert_eq!(sites.len(), 1, "unexpected sites: {sites:#?}");
        assert_eq!(sites[0].site.kind, UnsafeSiteKind::FfiCall);
        assert_eq!(sites[0].operation.family, OperationFamily::Ffi);
        assert_eq!(sites[0].site.owner, Some("len".to_string()));
        assert_eq!(sites[0].site.snippet, "unsafe { strlen(ptr) }");
        Ok(())
    }

    #[test]
    fn syntax_detection_classifies_qualified_same_file_extern_calls_as_ffi_calls()
    -> Result<(), String> {
        let root = unique_temp_dir()?;
        fs::create_dir_all(root.join("src"))
            .map_err(|err| format!("create temp src failed: {err}"))?;
        fs::write(
            root.join("src/lib.rs"),
            "mod ffi {\n    unsafe extern \"C\" {\n        pub(super) fn strlen(ptr: *const u8) -> usize;\n    }\n}\n\npub fn len(ptr: *const u8) -> usize {\n    // SAFETY: caller provides a C string pointer.\n    unsafe { ffi::strlen(ptr) }\n}\n",
        )
        .map_err(|err| format!("write temp source failed: {err}"))?;
        let diff = crate::input::diff::parse_unified_diff(
            "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -9,1 +9,1 @@\n+    unsafe { ffi::strlen(ptr) }\n",
        );

        let rel = PathBuf::from("src/lib.rs");
        let sites = scan_file(&root, &rel, Some(&diff), false)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert_eq!(sites.len(), 1, "unexpected sites: {sites:#?}");
        assert_eq!(sites[0].site.kind, UnsafeSiteKind::FfiCall);
        assert_eq!(sites[0].operation.family, OperationFamily::Ffi);
        assert_eq!(sites[0].site.owner, Some("len".to_string()));
        assert_eq!(sites[0].site.snippet, "unsafe { ffi::strlen(ptr) }");
        Ok(())
    }

    #[test]
    fn syntax_detection_classifies_libc_path_calls_as_ffi_calls() -> Result<(), String> {
        let root = unique_temp_dir()?;
        fs::create_dir_all(root.join("src"))
            .map_err(|err| format!("create temp src failed: {err}"))?;
        fs::write(
            root.join("src/lib.rs"),
            "pub fn len(ptr: *const i8) -> usize {\n    // SAFETY: caller provides a C string pointer.\n    unsafe { libc::strlen(ptr) }\n}\n",
        )
        .map_err(|err| format!("write temp source failed: {err}"))?;
        let diff = crate::input::diff::parse_unified_diff(
            "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -3,1 +3,1 @@\n+    unsafe { libc::strlen(ptr) }\n",
        );

        let rel = PathBuf::from("src/lib.rs");
        let sites = scan_file(&root, &rel, Some(&diff), false)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert_eq!(sites.len(), 1, "unexpected sites: {sites:#?}");
        assert_eq!(sites[0].site.kind, UnsafeSiteKind::FfiCall);
        assert_eq!(sites[0].operation.family, OperationFamily::Ffi);
        assert_eq!(sites[0].site.owner, Some("len".to_string()));
        assert_eq!(sites[0].site.snippet, "unsafe { libc::strlen(ptr) }");
        Ok(())
    }

    #[test]
    fn syntax_detection_keeps_local_libc_module_calls_generic() -> Result<(), String> {
        let root = unique_temp_dir()?;
        fs::create_dir_all(root.join("src"))
            .map_err(|err| format!("create temp src failed: {err}"))?;
        fs::write(
            root.join("src/lib.rs"),
            "mod libc {\n    pub unsafe fn strlen(_ptr: *const i8) -> usize { 0 }\n}\n\npub fn len(ptr: *const i8) -> usize {\n    // SAFETY: fixture exercises a local module named libc, not a foreign call.\n    unsafe { libc::strlen(ptr) }\n}\n",
        )
        .map_err(|err| format!("write temp source failed: {err}"))?;
        let diff = crate::input::diff::parse_unified_diff(
            "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -7,1 +7,1 @@\n+    unsafe { libc::strlen(ptr) }\n",
        );

        let rel = PathBuf::from("src/lib.rs");
        let sites = scan_file(&root, &rel, Some(&diff), false)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert_eq!(sites.len(), 1, "unexpected sites: {sites:#?}");
        assert_eq!(sites[0].site.kind, UnsafeSiteKind::Operation);
        assert_eq!(sites[0].operation.family, OperationFamily::UnsafeFnCall);
        assert_eq!(sites[0].site.owner, Some("len".to_string()));
        assert_eq!(sites[0].site.snippet, "unsafe { libc::strlen(ptr) }");
        Ok(())
    }

    #[test]
    fn syntax_detection_keeps_non_boundary_libc_text_as_generic_unsafe_call() -> Result<(), String>
    {
        let root = unique_temp_dir()?;
        fs::create_dir_all(root.join("src"))
            .map_err(|err| format!("create temp src failed: {err}"))?;
        fs::write(
            root.join("src/lib.rs"),
            "unsafe extern \"C\" {\n    fn strlen(ptr: *const i8) -> usize;\n}\n\nmod mylibc {\n    pub unsafe fn strlen(_ptr: *const i8) -> usize { 0 }\n}\n\npub fn len(ptr: *const i8) -> usize {\n    // SAFETY: fixture exercises a local module whose name merely contains libc.\n    unsafe { mylibc::strlen(ptr) }\n}\n",
        )
        .map_err(|err| format!("write temp source failed: {err}"))?;
        let diff = crate::input::diff::parse_unified_diff(
            "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -11,1 +11,1 @@\n+    unsafe { mylibc::strlen(ptr) }\n",
        );

        let rel = PathBuf::from("src/lib.rs");
        let sites = scan_file(&root, &rel, Some(&diff), false)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert_eq!(sites.len(), 1, "unexpected sites: {sites:#?}");
        assert_eq!(sites[0].site.kind, UnsafeSiteKind::Operation);
        assert_eq!(sites[0].operation.family, OperationFamily::UnsafeFnCall);
        assert_eq!(sites[0].site.owner, Some("len".to_string()));
        Ok(())
    }

    #[test]
    fn text_detection_does_not_classify_imported_operation_paths() {
        assert_eq!(detect_site("use core::ptr::copy_nonoverlapping;"), None);
        assert_eq!(
            detect_site("pub use core::mem::transmute as cast_value;"),
            None
        );
        assert_eq!(
            detect_site("pub use core::mem::transmute_copy as copy_bits;"),
            None
        );
        assert_eq!(
            detect_site("core::ptr::copy_nonoverlapping(src, dst, len);"),
            Some((
                UnsafeSiteKind::Operation,
                OperationFamily::CopyNonOverlapping
            ))
        );
        assert_eq!(
            detect_site("core::ptr::copy(src, dst, len);"),
            Some((UnsafeSiteKind::Operation, OperationFamily::PtrCopy))
        );
        assert_eq!(
            detect_site("core::ptr::replace(dst, value);"),
            Some((UnsafeSiteKind::Operation, OperationFamily::PtrReplace))
        );
        assert_eq!(
            detect_site("core::mem::transmute_copy::<u8, bool>(&value);"),
            Some((UnsafeSiteKind::Operation, OperationFamily::Transmute))
        );
        assert_eq!(
            detect_site("unsafe { slot.assume_init_read() }"),
            Some((
                UnsafeSiteKind::Operation,
                OperationFamily::MaybeUninitAssumeInit
            ))
        );
    }

    #[test]
    fn parses_generic_unsafe_impl_owner() {
        assert_eq!(
            parse_impl_owner("unsafe impl<T: Send> Send for Sender<T> {}").as_deref(),
            Some("Sender")
        );
        assert_eq!(
            parse_impl_owner("unsafe impl<'a, T: Sync> Sync for Receiver<'a, T> {}").as_deref(),
            Some("Receiver")
        );
        assert_eq!(
            parse_impl_owner("impl<T> Buffer<T> {").as_deref(),
            Some("Buffer")
        );
    }

    #[test]
    fn parses_generic_unsafe_impl_trait_name() {
        assert_eq!(
            parse_impl_trait_name("unsafe impl<T: Send> Sync for Sender<T> {}").as_deref(),
            Some("Sync")
        );
        assert_eq!(
            parse_impl_trait_name("unsafe impl<T: Sync> core::marker::Send for Sender<T> {}")
                .as_deref(),
            Some("Send")
        );
        assert_eq!(parse_impl_trait_name("impl<T> Buffer<T> {"), None);
    }

    #[test]
    fn text_detection_uses_implemented_send_sync_trait_not_bounds() {
        assert_eq!(
            detect_site("unsafe impl<T: Send> Sync for Sender<T> {}"),
            Some((
                UnsafeSiteKind::UnsafeImplSync,
                OperationFamily::UnsafeImplSendSync
            ))
        );
        assert_eq!(
            detect_site("unsafe impl<T: Sync> Send for Sender<T> {}"),
            Some((
                UnsafeSiteKind::UnsafeImplSend,
                OperationFamily::UnsafeImplSendSync
            ))
        );
    }

    #[test]
    fn text_detection_distinguishes_target_feature_attributes_from_cfg_checks() {
        assert_eq!(detect_site("#[cfg(target_feature = \"neon\")]"), None);
        assert_eq!(detect_site("#[cfg(not(target_feature = \"neon\"))]"), None);
        assert_eq!(
            detect_site("#[target_feature(enable = \"neon\")]"),
            Some((UnsafeSiteKind::Operation, OperationFamily::TargetFeature))
        );
        assert_eq!(
            detect_site(
                "#[cfg_attr(target_arch = \"aarch64\", target_feature(enable = \"neon\"))]"
            ),
            Some((UnsafeSiteKind::Operation, OperationFamily::TargetFeature))
        );
    }

    #[test]
    fn target_feature_owner_inference_uses_following_function() {
        let site = DetectedSyntaxSite {
            line: 5,
            end_line: 5,
            column: 1,
            kind: UnsafeSiteKind::Operation,
            family: OperationFamily::TargetFeature,
            source_snippet: "#[target_feature(enable = \"sse2\")]".to_string(),
            card_snippet: "#[target_feature(enable = \"sse2\")]".to_string(),
            start: 0,
            end: 0,
        };
        let lines = [
            "/// Runs a target-feature-specific path.",
            "///",
            "/// # Safety",
            "/// Callers must check SSE2.",
            "#[target_feature(enable = \"sse2\")]",
            "#[inline]",
            "pub unsafe fn find_raw(data: &[u8]) -> usize {",
            "    data.len()",
            "}",
        ];

        assert_eq!(syntax_owner(&site, &lines, 4), Some("find_raw".to_string()));
    }

    #[test]
    fn owner_inference_ignores_comment_text_when_scanning_backwards() {
        let lines = [
            "fn keep_rest(&mut self) {",
            "    unsafe {",
            "        // Normally `Drop` impl would drop [tail].",
            "        let src = ptr.add(this.idx);",
            "    }",
            "}",
        ];

        assert_eq!(find_owner(&lines, 3), Some("keep_rest".to_string()));
    }

    #[test]
    fn owner_inference_prefers_fn_over_impl_trait_parameters() {
        let lines = [
            "pub fn with_byte(ptr: *mut u8, f: impl FnOnce(&mut u8)) {",
            "    f(unsafe { &mut *ptr });",
            "}",
        ];

        assert_eq!(find_owner(&lines, 1), Some("with_byte".to_string()));
        assert_eq!(find_owner_declaration_index(&lines, 1), Some(0));
    }

    #[test]
    fn owner_inference_ignores_multiline_impl_trait_bounds() {
        let lines = [
            "pub fn try_reserve(",
            "    &mut self,",
            "    hasher: impl Fn(&u8) -> u64,",
            ") {",
            "    unsafe { self.reserve_rehash(hasher) }",
            "}",
        ];

        assert_eq!(find_owner(&lines, 4), Some("try_reserve".to_string()));
        assert_eq!(find_owner_declaration_index(&lines, 4), Some(0));
    }

    #[test]
    fn owner_inference_handles_long_function_bodies() {
        let mut lines = vec!["pub unsafe fn run(ptr: *mut u8) {".to_string()];
        lines.extend((0..120).map(|idx| format!("    let _pad_{idx} = ptr;")));
        lines.push("    unsafe { ptr.drop_in_place() };".to_string());
        lines.push("}".to_string());
        let borrowed = lines.iter().map(String::as_str).collect::<Vec<_>>();

        assert_eq!(find_owner(&borrowed, 121), Some("run".to_string()));
        assert_eq!(find_owner_declaration_index(&borrowed, 121), Some(0));
    }

    #[test]
    fn owner_inference_does_not_cross_closed_function() {
        let mut lines = vec![
            "pub unsafe fn previous(ptr: *mut u8) {".to_string(),
            "    unsafe { ptr.drop_in_place() };".to_string(),
            "}".to_string(),
        ];
        lines.extend((0..12).map(|idx| format!("// gap {idx}")));
        lines.push("unsafe { core::ptr::read(0 as *const u8) };".to_string());
        let borrowed = lines.iter().map(String::as_str).collect::<Vec<_>>();

        assert_eq!(find_owner(&borrowed, 15), None);
        assert_eq!(find_owner_declaration_index(&borrowed, 15), None);
    }

    #[test]
    fn owner_inference_uses_macro_rules_owner() {
        let lines = [
            "macro_rules! spawn_unchecked {",
            "    ($ptr:ident) => {{",
            "        let runnable = unsafe { Runnable::from_raw($ptr) };",
            "        runnable",
            "    }};",
            "}",
        ];

        assert_eq!(find_owner(&lines, 2), Some("spawn_unchecked".to_string()));
        assert_eq!(find_owner_declaration_index(&lines, 2), Some(0));
    }

    #[test]
    fn text_detection_does_not_classify_deref_assignments_as_writes() {
        assert_eq!(detect_site("*ptr = value;"), None);
        assert_eq!(detect_site("*ptr += 1;"), None);
        assert_eq!(detect_site("*next += 1;"), None);
        assert_eq!(detect_site("*ptr == value;"), None);
    }

    #[test]
    fn text_detection_classifies_raw_pointer_write_bytes_as_write() {
        for line in [
            "unsafe { ptr.write_bytes(byte, len) }",
            "unsafe { self.as_mut_ptr().write_bytes(tag.0, self.len()) }",
            "unsafe { core::ptr::write_bytes(ptr, byte, len) }",
        ] {
            assert_eq!(
                detect_site(line),
                Some((UnsafeSiteKind::Operation, OperationFamily::RawPointerWrite)),
                "{line} should be classified as a raw pointer write"
            );
        }
    }

    #[test]
    fn text_detection_classifies_raw_pointer_write_method_forms() {
        for line in [
            "unsafe { ptr.write(value) }",
            "unsafe { buf.as_mut_ptr().write(value) }",
            "unsafe { ptr.cast_mut().write(value) }",
            "unsafe { ptr.cast::<u8>().write(value) }",
        ] {
            assert_eq!(
                detect_site(line),
                Some((UnsafeSiteKind::Operation, OperationFamily::RawPointerWrite)),
                "{line} should be classified as a raw pointer write"
            );
        }
        assert_eq!(detect_site("writer.write_all(bytes)"), None);
    }

    #[test]
    fn text_detection_classifies_unaligned_pointer_write_separately() {
        for line in [
            "unsafe { ptr.write_unaligned(value) }",
            "unsafe { core::ptr::write_unaligned(ptr, value) }",
        ] {
            assert_eq!(
                detect_site(line),
                Some((
                    UnsafeSiteKind::Operation,
                    OperationFamily::RawPointerWriteUnaligned
                )),
                "{line} should be classified as an unaligned raw pointer write"
            );
        }
    }

    #[test]
    fn text_detection_classifies_volatile_pointer_write_as_raw_write() {
        for line in [
            "unsafe { register.write_volatile(value) }",
            "unsafe { core::ptr::write_volatile(register, value) }",
        ] {
            assert_eq!(
                detect_site(line),
                Some((UnsafeSiteKind::Operation, OperationFamily::RawPointerWrite)),
                "{line} should be classified as a raw pointer write"
            );
        }
    }

    #[test]
    fn text_detection_classifies_volatile_pointer_read_as_raw_read() {
        assert_eq!(
            detect_site("unsafe { register.read_volatile() }"),
            Some((UnsafeSiteKind::Operation, OperationFamily::RawPointerRead))
        );
    }

    #[test]
    fn text_detection_classifies_atomic_pointer_null_swap_as_state_transition() {
        assert_eq!(
            detect_site("block = self.head.block.swap(ptr::null_mut(), Ordering::AcqRel);"),
            Some((
                UnsafeSiteKind::Operation,
                OperationFamily::AtomicPointerState
            ))
        );
        assert_eq!(
            detect_site("block = self.head.block.load(Ordering::Acquire);"),
            None
        );
    }

    #[test]
    fn text_detection_classifies_atomic_pointer_fetch_state_transitions() {
        for line in [
            "Shared::from_ptr(self.data.fetch_and(val, order))",
            "Shared::from_ptr(self.data.fetch_or(val, order))",
            "Shared::from_ptr(self.data.fetch_xor(val, order) as *mut ())",
        ] {
            assert_eq!(
                detect_site(line),
                Some((
                    UnsafeSiteKind::Operation,
                    OperationFamily::AtomicPointerState
                )),
                "{line} should be classified as an atomic pointer state transition"
            );
        }
        assert_eq!(detect_site("bits.fetch_and(mask, Ordering::AcqRel);"), None);
    }

    #[test]
    fn text_detection_prefers_inline_asm_over_generic_unsafe_call_wrapper() {
        assert_eq!(
            detect_site("unsafe { core::arch::asm!(\"nop\") }"),
            Some((UnsafeSiteKind::Operation, OperationFamily::InlineAsm))
        );
    }

    #[test]
    fn text_detection_only_classifies_nonnull_new_unchecked_as_nonnull() {
        assert_eq!(
            detect_site("unsafe { NonNull::new_unchecked(ptr) }"),
            Some((UnsafeSiteKind::Operation, OperationFamily::NonNullUnchecked))
        );
        assert_eq!(
            detect_site("unsafe { Pin::new_unchecked(value) }"),
            Some((UnsafeSiteKind::Operation, OperationFamily::PinUnchecked))
        );
        assert_eq!(
            detect_site("unsafe { Some(One::new_unchecked(needle)) }"),
            Some((UnsafeSiteKind::Operation, OperationFamily::UnsafeFnCall))
        );
    }

    #[test]
    fn syntax_detection_classifies_unsafe_raw_pointer_assignments_as_writes() -> Result<(), String>
    {
        let root = unique_temp_dir()?;
        fs::create_dir_all(root.join("src"))
            .map_err(|err| format!("create temp src failed: {err}"))?;
        fs::write(
            root.join("src/lib.rs"),
            "pub fn write_byte(ptr: *mut u8, value: u8) {\n    unsafe {\n        *ptr = value;\n    }\n}\n",
        )
        .map_err(|err| format!("write temp source failed: {err}"))?;

        let sites = scan_file(&root, &PathBuf::from("src/lib.rs"), None, true)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let operations = sites
            .iter()
            .filter(|site| site.site.kind == UnsafeSiteKind::Operation)
            .collect::<Vec<_>>();
        assert_eq!(operations.len(), 1, "unexpected sites: {sites:#?}");
        assert_eq!(
            operations[0].operation.family,
            OperationFamily::RawPointerWrite
        );
        assert_eq!(operations[0].site.snippet, "*ptr = value;");
        Ok(())
    }

    #[test]
    fn scan_file_does_not_emit_cards_for_extern_crate_or_unsafe_import_paths() -> Result<(), String>
    {
        let root = unique_temp_dir()?;
        fs::create_dir_all(root.join("src"))
            .map_err(|err| format!("create temp src failed: {err}"))?;
        fs::write(
            root.join("src/lib.rs"),
            "extern crate std;\n\nuse core::ptr::copy_nonoverlapping;\npub use core::mem::transmute as cast_value;\n\npub fn len(bytes: &[u8]) -> usize { bytes.len() }\n",
        )
        .map_err(|err| format!("write temp source failed: {err}"))?;

        let sites = scan_file(&root, &PathBuf::from("src/lib.rs"), None, true)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(sites.is_empty(), "unexpected sites: {sites:#?}");
        Ok(())
    }

    #[test]
    fn scan_file_does_not_emit_cards_for_unsafe_words_in_comments_or_strings() -> Result<(), String>
    {
        let root = unique_temp_dir()?;
        fs::create_dir_all(root.join("src"))
            .map_err(|err| format!("create temp src failed: {err}"))?;
        fs::write(
            root.join("src/lib.rs"),
            "pub fn safe_text() -> &'static str {\n    /* unsafe { core::ptr::read(ptr) } */\n    \"unsafe { core::ptr::read(ptr) }\"\n}\n",
        )
        .map_err(|err| format!("write temp source failed: {err}"))?;

        let sites = scan_file(&root, &PathBuf::from("src/lib.rs"), None, true)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(sites.is_empty(), "unexpected sites: {sites:#?}");
        Ok(())
    }

    #[test]
    fn scan_file_ignores_nested_block_comments_and_raw_strings() -> Result<(), String> {
        let root = unique_temp_dir()?;
        fs::create_dir_all(root.join("src"))
            .map_err(|err| format!("create temp src failed: {err}"))?;
        fs::write(
            root.join("src/lib.rs"),
            "pub fn safe_text() -> &'static str {\n    /* outer /* unsafe impl Send for Nope {} */ unsafe { core::ptr::read(ptr) } */\n    r#\"unsafe { core::ptr::read(ptr) }\"#\n}\n",
        )
        .map_err(|err| format!("write temp source failed: {err}"))?;

        let sites = scan_file(&root, &PathBuf::from("src/lib.rs"), None, true)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            sites.is_empty(),
            "nested comments or raw strings should not emit cards: {sites:#?}"
        );
        Ok(())
    }

    #[test]
    fn text_detection_keeps_code_before_trailing_comment() {
        let mut state = LineCommentState::default();
        let detection_line = line_for_text_detection(
            "    unsafe { core::ptr::read(ptr) } // mention transmute::<u8, bool>",
            &mut state,
        );

        assert_eq!(detection_line.trim(), "unsafe { core::ptr::read(ptr) }");
        assert_eq!(state.block_depth, 0);
        assert_eq!(
            detect_site(detection_line.trim()),
            Some((UnsafeSiteKind::Operation, OperationFamily::RawPointerRead))
        );
    }

    #[test]
    fn scan_file_infers_extern_block_owner_from_declared_function() -> Result<(), String> {
        let root = unique_temp_dir()?;
        fs::create_dir_all(root.join("src"))
            .map_err(|err| format!("create temp src failed: {err}"))?;
        fs::write(
            root.join("src/lib.rs"),
            "unsafe extern \"C\" {\n    fn strlen(ptr: *const u8) -> usize;\n}\n",
        )
        .map_err(|err| format!("write temp source failed: {err}"))?;

        let sites = scan_file(&root, &PathBuf::from("src/lib.rs"), None, true)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let extern_block = sites
            .iter()
            .find(|site| site.site.kind == UnsafeSiteKind::ExternBlock)
            .ok_or_else(|| format!("expected extern block site: {sites:#?}"))?;
        assert_eq!(extern_block.operation.family, OperationFamily::Ffi);
        assert_eq!(extern_block.site.owner.as_deref(), Some("strlen"));
        Ok(())
    }

    #[test]
    fn scan_file_does_not_classify_static_lifetime_mut_reference_as_static_mut()
    -> Result<(), String> {
        let root = unique_temp_dir()?;
        fs::create_dir_all(root.join("src"))
            .map_err(|err| format!("create temp src failed: {err}"))?;
        fs::write(
            root.join("src/lib.rs"),
            "pub fn expose_mut(ptr: *mut u8, len: usize) -> &'static mut [u8] {\n    unsafe { core::slice::from_raw_parts_mut(ptr, len) }\n}\n",
        )
        .map_err(|err| format!("write temp source failed: {err}"))?;

        let sites = scan_file(&root, &PathBuf::from("src/lib.rs"), None, true)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            sites
                .iter()
                .all(|site| site.site.kind != UnsafeSiteKind::StaticMut),
            "static lifetime in mutable reference should not be a static mut item: {sites:#?}"
        );
        assert_eq!(
            sites
                .iter()
                .filter(|site| site.operation.family == OperationFamily::SliceFromRawParts)
                .count(),
            1,
            "slice operation should still be detected: {sites:#?}"
        );
        Ok(())
    }

    #[test]
    fn scan_file_classifies_static_mut_items() -> Result<(), String> {
        let root = unique_temp_dir()?;
        fs::create_dir_all(root.join("src"))
            .map_err(|err| format!("create temp src failed: {err}"))?;
        fs::write(
            root.join("src/lib.rs"),
            "static mut ROOT: usize = 0;\npub static mut PUBLIC: usize = 0;\npub(crate) static mut RESTRICTED: usize = 0;\n",
        )
        .map_err(|err| format!("write temp source failed: {err}"))?;

        let sites = scan_file(&root, &PathBuf::from("src/lib.rs"), None, true)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let static_mut_sites = sites
            .iter()
            .filter(|site| site.site.kind == UnsafeSiteKind::StaticMut)
            .collect::<Vec<_>>();
        assert_eq!(
            static_mut_sites.len(),
            3,
            "expected each static mut item to be detected: {sites:#?}"
        );
        assert!(
            static_mut_sites
                .iter()
                .all(|site| site.operation.family == OperationFamily::StaticMut),
            "static mut sites should keep the static_mut operation family: {sites:#?}"
        );
        assert_eq!(static_mut_sites[0].site.owner.as_deref(), Some("ROOT"));
        assert_eq!(static_mut_sites[1].site.owner.as_deref(), Some("PUBLIC"));
        assert_eq!(
            static_mut_sites[2].site.owner.as_deref(),
            Some("RESTRICTED")
        );
        Ok(())
    }

    #[test]
    fn syntax_detection_ignores_unsafe_words_inside_call_string_literals() -> Result<(), String> {
        let root = unique_temp_dir()?;
        fs::create_dir_all(root.join("src"))
            .map_err(|err| format!("create temp src failed: {err}"))?;
        fs::write(
            root.join("src/lib.rs"),
            "pub fn detector_text(line: &str) -> bool {\n    line.contains(\"get_unchecked\") || line.contains(\"ptr::read_unaligned\")\n}\n",
        )
        .map_err(|err| format!("write temp source failed: {err}"))?;

        let sites = scan_file(&root, &PathBuf::from("src/lib.rs"), None, true)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(sites.is_empty(), "unexpected sites: {sites:#?}");
        Ok(())
    }

    #[test]
    fn syntax_detection_survives_unrelated_parse_errors() -> Result<(), String> {
        let root = unique_temp_dir()?;
        fs::create_dir_all(root.join("src"))
            .map_err(|err| format!("create temp src failed: {err}"))?;
        fs::write(
            root.join("src/lib.rs"),
            "pub fn read_byte(ptr: *const u8) -> u8 {\n    // SAFETY: caller provides a valid pointer.\n    unsafe {\n        ptr\n            .read ()\n    }\n}\n\npub fn broken( {\n",
        )
        .map_err(|err| format!("write temp source failed: {err}"))?;

        let sites = scan_file(&root, &PathBuf::from("src/lib.rs"), None, true)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let operations = sites
            .iter()
            .filter(|site| site.site.kind == UnsafeSiteKind::Operation)
            .collect::<Vec<_>>();
        assert_eq!(operations.len(), 1, "unexpected sites: {sites:#?}");
        assert_eq!(
            operations[0].operation.family,
            OperationFamily::RawPointerRead
        );
        assert!(
            sites.iter().all(|site| {
                !(site.site.kind == UnsafeSiteKind::UnsafeBlock
                    && site.operation.family == OperationFamily::Unknown)
            }),
            "concrete syntax operation should suppress wrapper unknown unsafe block"
        );
        Ok(())
    }

    #[test]
    fn scan_file_suppresses_unknown_wrapper_when_concrete_operation_exists() -> Result<(), String> {
        let root = unique_temp_dir()?;
        fs::create_dir_all(root.join("src"))
            .map_err(|err| format!("create temp src failed: {err}"))?;
        fs::write(
            root.join("src/lib.rs"),
            "pub fn read_byte(ptr: *const u8) -> u8 {\n    unsafe { *ptr }\n}\n",
        )
        .map_err(|err| format!("write temp source failed: {err}"))?;

        let sites = scan_file(&root, &PathBuf::from("src/lib.rs"), None, true)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert_eq!(sites.len(), 1, "unexpected sites: {sites:#?}");
        assert_eq!(sites[0].site.kind, UnsafeSiteKind::Operation);
        assert_eq!(sites[0].operation.family, OperationFamily::RawPointerDeref);
        assert_eq!(sites[0].site.owner, Some("read_byte".to_string()));
        Ok(())
    }

    #[test]
    fn scan_file_suppresses_multiline_unknown_wrapper_when_specific_operation_exists()
    -> Result<(), String> {
        let root = unique_temp_dir()?;
        fs::create_dir_all(root.join("src"))
            .map_err(|err| format!("create temp src failed: {err}"))?;
        fs::write(
            root.join("src/lib.rs"),
            "use core::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};\n\
pub struct Shared<T>(*mut T);\n\
impl<T> Shared<T> { fn from_ptr(ptr: *mut T) -> Self { Self(ptr) } }\n\
pub struct Tagged<T> { data: AtomicPtr<T> }\n\
impl<T> Tagged<T> {\n\
    pub fn fetch_and_tag(&self, val: usize, order: Ordering) -> Shared<T> {\n\
        unsafe {\n\
            Shared::from_ptr(\n\
                (*(&self.data as *const AtomicPtr<_> as *const AtomicUsize))\n\
                    .fetch_and(val, order) as *mut T,\n\
            )\n\
        }\n\
    }\n\
}\n",
        )
        .map_err(|err| format!("write temp source failed: {err}"))?;

        let sites = scan_file(&root, &PathBuf::from("src/lib.rs"), None, true)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let atomic_sites = sites
            .iter()
            .filter(|site| site.operation.family == OperationFamily::AtomicPointerState)
            .collect::<Vec<_>>();
        assert_eq!(atomic_sites.len(), 1, "unexpected sites: {sites:#?}");
        assert!(
            sites.iter().all(|site| {
                !(site.site.kind == UnsafeSiteKind::UnsafeBlock
                    && site.operation.family == OperationFamily::Unknown)
            }),
            "specific operation should suppress wrapper unknown unsafe block"
        );
        Ok(())
    }

    #[test]
    fn scan_file_keeps_public_surface_on_unsafe_api_not_operations() -> Result<(), String> {
        let root = unique_temp_dir()?;
        fs::create_dir_all(root.join("src"))
            .map_err(|err| format!("create temp src failed: {err}"))?;
        fs::write(
            root.join("src/lib.rs"),
            "pub(crate) unsafe fn expose(ptr: *const u8) -> u8 {\n    unsafe { *ptr }\n}\n\nunsafe impl Send for LocalType {}\n\nstruct LocalType;\n",
        )
        .map_err(|err| format!("write temp source failed: {err}"))?;

        let sites = scan_file(&root, &PathBuf::from("src/lib.rs"), None, true)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let public_fn = sites
            .iter()
            .find(|site| site.site.kind == UnsafeSiteKind::UnsafeFn)
            .ok_or_else(|| format!("expected unsafe function site: {sites:#?}"))?;
        assert_eq!(public_fn.site.owner.as_deref(), Some("expose"));
        assert_eq!(public_fn.site.visibility, "public");
        assert!(public_fn.site.public_api_surface);

        let deref = sites
            .iter()
            .find(|site| site.operation.family == OperationFamily::RawPointerDeref)
            .ok_or_else(|| format!("expected raw pointer deref site: {sites:#?}"))?;
        assert_eq!(deref.site.owner.as_deref(), Some("expose"));
        assert!(!deref.site.public_api_surface);

        let unsafe_impl = sites
            .iter()
            .find(|site| site.site.kind == UnsafeSiteKind::UnsafeImplSend)
            .ok_or_else(|| format!("expected unsafe impl Send site: {sites:#?}"))?;
        assert_eq!(
            unsafe_impl.operation.family,
            OperationFamily::UnsafeImplSendSync
        );
        assert_eq!(unsafe_impl.site.visibility, "private");
        assert!(!unsafe_impl.site.public_api_surface);
        Ok(())
    }

    #[test]
    fn scan_file_keeps_declaration_and_concrete_operations_without_comment_noise()
    -> Result<(), String> {
        let root = unique_temp_dir()?;
        fs::create_dir_all(root.join("src"))
            .map_err(|err| format!("create temp src failed: {err}"))?;
        fs::write(
            root.join("src/lib.rs"),
            "pub unsafe fn expose(ptr: *const u8) -> u8 {\n    // core::mem::transmute in a comment must not be reported.\n    unsafe {\n        *ptr\n    }\n}\n\npub fn read_byte(ptr: *const u8) -> u8 {\n    unsafe { core::ptr::read(ptr) }\n}\n",
        )
        .map_err(|err| format!("write temp source failed: {err}"))?;

        let sites = scan_file(&root, &PathBuf::from("src/lib.rs"), None, true)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let families = sites
            .iter()
            .map(|site| site.operation.family.clone())
            .collect::<Vec<_>>();
        assert!(
            families.contains(&OperationFamily::Unknown),
            "unsafe function declaration should remain visible: {sites:#?}"
        );
        assert!(
            families.contains(&OperationFamily::RawPointerDeref),
            "raw pointer deref operation should remain visible: {sites:#?}"
        );
        assert!(
            families.contains(&OperationFamily::RawPointerRead),
            "raw pointer read operation should remain visible: {sites:#?}"
        );
        assert!(
            sites
                .iter()
                .all(|site| site.site.kind != UnsafeSiteKind::UnsafeBlock),
            "concrete operations should suppress wrapper unsafe-block cards: {sites:#?}"
        );
        assert!(
            sites
                .iter()
                .all(|site| !site.operation.expression.contains("comment")),
            "comment text should not be reported as an operation: {sites:#?}"
        );
        Ok(())
    }

    #[test]
    fn scan_file_filters_to_diff_neighborhood_unless_repo_mode() -> Result<(), String> {
        let root = unique_temp_dir()?;
        fs::create_dir_all(root.join("src"))
            .map_err(|err| format!("create temp src failed: {err}"))?;
        fs::write(
            root.join("src/lib.rs"),
            "pub fn first(ptr: *const u8) -> u8 {\n    unsafe { *ptr }\n}\n\n\n\n\n\n\n\n\npub fn second(ptr: *const u8) -> u8 {\n    unsafe { *ptr }\n}\n",
        )
        .map_err(|err| format!("write temp source failed: {err}"))?;
        let diff = crate::input::diff::parse_unified_diff(
            "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,2 +1,2 @@\n+pub fn first(ptr: *const u8) -> u8 {\n",
        );

        let rel = PathBuf::from("src/lib.rs");
        let diff_sites = scan_file(&root, &rel, Some(&diff), false)?;
        let repo_sites = scan_file(&root, &rel, Some(&diff), true)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert_eq!(
            diff_sites.len(),
            1,
            "unexpected diff sites: {diff_sites:#?}"
        );
        assert_eq!(diff_sites[0].site.owner, Some("first".to_string()));
        assert_eq!(
            repo_sites.len(),
            2,
            "unexpected repo sites: {repo_sites:#?}"
        );
        assert_eq!(repo_sites[1].site.owner, Some("second".to_string()));
        Ok(())
    }

    #[test]
    fn syntax_detection_ignores_unsafe_declarations_inside_function_bodies() -> Result<(), String> {
        let root = unique_temp_dir()?;
        fs::create_dir_all(root.join("src"))
            .map_err(|err| format!("create temp src failed: {err}"))?;
        fs::write(
            root.join("src/lib.rs"),
            "pub fn safe_text() -> &'static str {\n    r#\"pub unsafe fn fake() {}\"#\n}\n",
        )
        .map_err(|err| format!("write temp source failed: {err}"))?;

        let sites = scan_file(&root, &PathBuf::from("src/lib.rs"), None, true)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(sites.is_empty(), "unexpected sites: {sites:#?}");
        Ok(())
    }

    #[test]
    fn scan_file_ignores_multiline_string_literal_unsafe_text() -> Result<(), String> {
        let root = unique_temp_dir()?;
        fs::create_dir_all(root.join("src"))
            .map_err(|err| format!("create temp src failed: {err}"))?;
        fs::write(
            root.join("src/lib.rs"),
            "pub fn fixture_text() -> (&'static str, &'static str) {\n    let raw = r#\"\n        pub unsafe fn fake_api(ptr: *const u8) -> u8 {\n            unsafe { core::ptr::read(ptr) }\n        }\n    \"#;\n    let cooked = \"\n        unsafe { core::mem::transmute::<u32, i32>(value) }\n    \";\n    (raw, cooked)\n}\n",
        )
        .map_err(|err| format!("write temp source failed: {err}"))?;

        let sites = scan_file(&root, &PathBuf::from("src/lib.rs"), None, true)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(sites.is_empty(), "unexpected sites: {sites:#?}");
        Ok(())
    }

    #[test]
    fn scan_file_does_not_report_detector_literal_matchers() -> Result<(), String> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let sites = scan_file(&root, &PathBuf::from("src/analysis/scanner.rs"), None, true)?;

        assert!(
            sites.iter().all(|site| {
                !(site.site.owner.as_deref() == Some("detect_site")
                    && site.site.snippet.starts_with("if line.contains("))
            }),
            "detector literal matchers should not be reported: {sites:#?}"
        );
        Ok(())
    }

    #[test]
    fn scan_file_ignores_unsafe_words_inside_contains_string_literals() -> Result<(), String> {
        let root = unique_temp_dir()?;
        fs::create_dir_all(root.join("src"))
            .map_err(|err| format!("create temp src failed: {err}"))?;
        fs::write(
            root.join("src/lib.rs"),
            "pub fn detector(line: &str) -> bool {\n    if line.contains(\"unsafe impl\") { return true; }\n    line.contains(\"ptr::read\")\n}\n",
        )
        .map_err(|err| format!("write temp source failed: {err}"))?;

        let sites = scan_file(&root, &PathBuf::from("src/lib.rs"), None, true)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(sites.is_empty(), "unexpected sites: {sites:#?}");
        Ok(())
    }

    #[test]
    fn text_detection_strips_inline_string_literals() {
        let mut state = LineCommentState::default();

        assert_eq!(
            line_for_text_detection("if line.contains(\"unsafe impl\") {", &mut state),
            "if line.contains(\"\") {"
        );
        assert_eq!(
            syntax_detection_text("line.contains ( \"ptr::read\" )"),
            "line.contains( \"\" )"
        );
    }

    #[test]
    fn declaration_prefix_limits_declaration_detection_to_header() {
        assert_eq!(
            declaration_prefix("pub fn safe() { let text = \"pub unsafe fn fake() {}\"; }"),
            "pub fn safe()"
        );
        assert_eq!(
            declaration_prefix("pub unsafe fn real() { }"),
            "pub unsafe fn real()"
        );
    }

    #[test]
    fn restricted_visibility_counts_as_public_surface() {
        for snippet in [
            "pub(crate) unsafe fn expose() {}",
            "pub(super) unsafe trait Token {}",
            "pub(in crate::ffi) unsafe fn expose() {}",
        ] {
            assert_eq!(visibility_for_snippet(snippet), "public");
            assert!(is_public_surface(snippet));
        }
    }

    #[test]
    fn unsafe_api_surface_includes_restricted_pub_items() {
        assert!(is_public_api_surface(
            &UnsafeSiteKind::UnsafeFn,
            "pub(crate) unsafe fn expose() {}"
        ));
        assert!(is_public_api_surface(
            &UnsafeSiteKind::UnsafeTrait,
            "pub(super) unsafe trait Token {}"
        ));
        assert!(!is_public_api_surface(
            &UnsafeSiteKind::Operation,
            "pub(crate) unsafe { *ptr }"
        ));
    }

    fn unique_temp_dir() -> Result<PathBuf, String> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system clock before UNIX_EPOCH: {err}"))?
            .as_nanos();
        Ok(std::env::temp_dir().join(format!("unsafe-review-scanner-test-{nanos}")))
    }
}
