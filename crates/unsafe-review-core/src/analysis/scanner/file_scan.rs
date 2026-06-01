use super::{
    ScannedSite, context_before_site, context_slice, detect_js_buffer_reentry_sites, detect_site,
    detect_syntax_sites, extern_fn_names, fallback_unsafe_block_contains_specific_operation,
    find_extern_block_owner, find_following_fn_owner, find_owner, first_non_ws_column,
    is_incomplete_multiline_transmute_copy, is_public_api_surface, line_for_text_detection,
    local_module_names, operation_block_start_lines, parse_static_mut_name, site_key,
    syntax_operation_covers_fallback, syntax_owner, syntax_site_covers_fallback,
    syntax_site_uses_exact_range, visibility_for_snippet,
};
use crate::domain::{OperationFamily, SourceLocation, UnsafeOperation, UnsafeSite, UnsafeSiteKind};
use crate::input::diff::DiffIndex;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

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
    let parsed = super::super::syntax::parse_source(text.as_str());
    let extern_names = extern_fn_names(&lines);
    let local_modules = local_module_names(&lines);
    let syntax_sites = detect_syntax_sites(&parsed, &extern_names, &local_modules);
    let syntax_index = SyntaxSiteIndex::new(&parsed, &syntax_sites);
    let mut seen = BTreeSet::new();

    let mut out = fallback_sites(
        rel,
        diff,
        repo_mode,
        &lines,
        &syntax_sites,
        &syntax_index,
        &mut seen,
    );
    out.extend(syntax_backfill_sites(
        rel,
        diff,
        repo_mode,
        &lines,
        syntax_sites,
        &syntax_index,
        &mut seen,
    ));
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

struct SyntaxSiteIndex {
    operation_lines: BTreeSet<usize>,
    operation_block_lines: BTreeSet<usize>,
}

impl SyntaxSiteIndex {
    fn new(parsed: &super::ParsedSource, syntax_sites: &[super::DetectedSyntaxSite]) -> Self {
        let operation_lines = syntax_sites
            .iter()
            .filter(|site| site.kind == UnsafeSiteKind::Operation)
            .map(|site| site.line)
            .collect::<BTreeSet<_>>();
        let operation_block_lines = operation_block_start_lines(parsed);
        Self {
            operation_lines,
            operation_block_lines,
        }
    }

    fn covers_specific_operation(&self, line_no: usize, lines: &[&str], idx: usize) -> bool {
        self.operation_lines.contains(&line_no)
            || self.operation_block_lines.contains(&line_no)
            || fallback_unsafe_block_contains_specific_operation(lines, idx)
    }
}

fn fallback_sites(
    rel: &PathBuf,
    diff: Option<&DiffIndex>,
    repo_mode: bool,
    lines: &[&str],
    syntax_sites: &[super::DetectedSyntaxSite],
    syntax_index: &SyntaxSiteIndex,
    seen: &mut BTreeSet<(usize, String, String)>,
) -> Vec<ScannedSite> {
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
        if fallback_is_shadowed_by_syntax(
            lines,
            syntax_sites,
            syntax_index,
            idx,
            line_no,
            detection_trimmed,
            &kind,
            &family,
        ) {
            continue;
        }
        seen.insert(site_key(line_no, &kind, &family));
        if !line_is_changed(rel, diff, repo_mode, line_no, line_no, &kind) {
            continue;
        }
        out.push(fallback_scanned_site(
            rel,
            lines,
            idx,
            raw,
            trimmed,
            detection_trimmed,
            kind,
            family,
        ));
    }
    out
}

fn fallback_is_shadowed_by_syntax(
    lines: &[&str],
    syntax_sites: &[super::DetectedSyntaxSite],
    syntax_index: &SyntaxSiteIndex,
    idx: usize,
    line_no: usize,
    detection_trimmed: &str,
    kind: &UnsafeSiteKind,
    family: &OperationFamily,
) -> bool {
    syntax_site_covers_fallback(syntax_sites, line_no, kind, family)
        || (*kind == UnsafeSiteKind::Operation
            && *family == OperationFamily::Transmute
            && is_incomplete_multiline_transmute_copy(detection_trimmed)
            && syntax_operation_covers_fallback(syntax_sites, line_no, family))
        || (*kind == UnsafeSiteKind::UnsafeBlock
            && *family == OperationFamily::Unknown
            && syntax_index.covers_specific_operation(line_no, lines, idx))
}

fn fallback_scanned_site(
    rel: &PathBuf,
    lines: &[&str],
    idx: usize,
    raw: &str,
    trimmed: &str,
    detection_trimmed: &str,
    kind: UnsafeSiteKind,
    family: OperationFamily,
) -> ScannedSite {
    let line_no = idx + 1;
    let owner = match (&kind, &family) {
        (UnsafeSiteKind::ExternBlock, OperationFamily::Ffi) => find_extern_block_owner(lines, idx),
        (UnsafeSiteKind::Operation, OperationFamily::TargetFeature) => {
            find_following_fn_owner(lines, idx)
        }
        (UnsafeSiteKind::StaticMut, OperationFamily::StaticMut) => {
            parse_static_mut_name(detection_trimmed)
        }
        _ => None,
    }
    .or_else(|| find_owner(lines, idx));

    scanned_site(
        rel,
        line_no,
        first_non_ws_column(raw),
        kind,
        owner,
        family,
        trimmed,
        trimmed,
        context_before_site(lines, idx),
        context_slice(lines, idx + 1, (idx + 8).min(lines.len())),
    )
}

fn syntax_backfill_sites(
    rel: &PathBuf,
    diff: Option<&DiffIndex>,
    repo_mode: bool,
    lines: &[&str],
    syntax_sites: Vec<super::DetectedSyntaxSite>,
    syntax_index: &SyntaxSiteIndex,
    seen: &mut BTreeSet<(usize, String, String)>,
) -> Vec<ScannedSite> {
    let mut out = Vec::new();
    for detected in syntax_sites {
        if detected.kind == UnsafeSiteKind::UnsafeBlock
            && detected.family == OperationFamily::Unknown
            && syntax_index.operation_lines.contains(&detected.line)
        {
            continue;
        }
        if !seen.insert(site_key(detected.line, &detected.kind, &detected.family)) {
            continue;
        }
        if !line_is_changed(
            rel,
            diff,
            repo_mode,
            detected.line,
            detected.end_line,
            &detected.kind,
        ) {
            continue;
        }
        out.push(syntax_scanned_site(rel, lines, detected));
    }
    out
}

fn syntax_scanned_site(
    rel: &PathBuf,
    lines: &[&str],
    detected: super::DetectedSyntaxSite,
) -> ScannedSite {
    let idx = detected.line.saturating_sub(1);
    let owner = syntax_owner(&detected, lines, idx);
    let context_after = context_slice(
        lines,
        (idx + 1).min(lines.len()),
        (idx + 8).min(lines.len()),
    );
    scanned_site(
        rel,
        detected.line,
        detected.column,
        detected.kind,
        owner,
        detected.family,
        &detected.source_snippet,
        &detected.card_snippet,
        context_before_site(lines, idx),
        context_after,
    )
}

fn line_is_changed(
    rel: &PathBuf,
    diff: Option<&DiffIndex>,
    repo_mode: bool,
    start_line: usize,
    end_line: usize,
    kind: &UnsafeSiteKind,
) -> bool {
    diff.is_none_or(|d| {
        repo_mode
            || if syntax_site_uses_exact_range(kind) {
                d.contains_in_range(rel, start_line, end_line)
            } else {
                d.contains_near(rel, start_line)
            }
    })
}

fn scanned_site(
    rel: &PathBuf,
    line: usize,
    column: usize,
    kind: UnsafeSiteKind,
    owner: Option<String>,
    family: OperationFamily,
    source_snippet: &str,
    card_snippet: &str,
    context_before: Vec<String>,
    context_after: Vec<String>,
) -> ScannedSite {
    let visibility = visibility_for_snippet(source_snippet).to_string();
    let public_api_surface = is_public_api_surface(&kind, source_snippet);
    ScannedSite {
        site: UnsafeSite {
            location: SourceLocation::new(rel.clone(), line, column),
            kind,
            owner,
            visibility,
            public_api_surface,
            changed: true,
            snippet: card_snippet.to_string(),
        },
        operation: UnsafeOperation {
            family,
            expression: card_snippet.to_string(),
        },
        context_before,
        context_after,
    }
}
