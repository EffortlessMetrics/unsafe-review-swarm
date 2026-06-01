use super::super::static_mut::parse_static_mut_name;
use super::owner_context::{
    context_before_site, find_extern_block_owner, find_following_fn_owner, find_owner,
};
use super::text_detection::{LineCommentState, line_for_text_detection};
use super::{
    DetectedSyntaxSite, ScannedSite, context_slice, detect_js_buffer_reentry_sites, detect_site,
    detect_syntax_sites, fallback_unsafe_block_contains_specific_operation, first_non_ws_column,
    is_incomplete_multiline_transmute_copy, is_public_api_surface, operation_block_start_lines,
    site_key, syntax_operation_covers_fallback, syntax_owner, syntax_site_covers_fallback,
    syntax_site_uses_exact_range, visibility_for_snippet,
};
use crate::domain::{OperationFamily, SourceLocation, UnsafeOperation, UnsafeSite, UnsafeSiteKind};
use crate::input::diff::DiffIndex;
use std::collections::BTreeSet;
use std::path::PathBuf;

pub(super) fn scan_text(
    rel: &PathBuf,
    diff: Option<&DiffIndex>,
    repo_mode: bool,
    text: &str,
) -> Vec<ScannedSite> {
    let lines: Vec<&str> = text.lines().collect();
    let syntax = SyntaxScan::new(text, &lines);
    let ctx = ScanContext {
        rel,
        diff,
        repo_mode,
        lines: &lines,
    };
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();

    out.extend(collect_fallback_sites(&ctx, &syntax, &mut seen));
    out.extend(collect_syntax_sites(&ctx, &syntax, &mut seen));
    out.extend(detect_js_buffer_reentry_sites(rel, diff, repo_mode, &lines));
    out.sort_by(|left, right| {
        left.site
            .location
            .line
            .cmp(&right.site.location.line)
            .then(left.site.location.column.cmp(&right.site.location.column))
    });
    out
}

struct ScanContext<'a> {
    rel: &'a PathBuf,
    diff: Option<&'a DiffIndex>,
    repo_mode: bool,
    lines: &'a [&'a str],
}

struct SyntaxScan {
    sites: Vec<DetectedSyntaxSite>,
    operation_lines: BTreeSet<usize>,
    operation_block_lines: BTreeSet<usize>,
}

impl SyntaxScan {
    fn new(text: &str, lines: &[&str]) -> Self {
        let parsed = super::super::syntax::parse_source(text);
        let extern_names = super::extern_fn_names(lines);
        let local_modules = super::local_module_names(lines);
        let sites = detect_syntax_sites(&parsed, &extern_names, &local_modules);
        let operation_lines = sites
            .iter()
            .filter(|site| site.kind == UnsafeSiteKind::Operation)
            .map(|site| site.line)
            .collect::<BTreeSet<_>>();
        let operation_block_lines = operation_block_start_lines(&parsed);
        Self {
            sites,
            operation_lines,
            operation_block_lines,
        }
    }
}

fn collect_fallback_sites(
    ctx: &ScanContext<'_>,
    syntax: &SyntaxScan,
    seen: &mut BTreeSet<(usize, String, String)>,
) -> Vec<ScannedSite> {
    let mut sites = Vec::new();
    let mut line_comment_state = LineCommentState::default();
    for (idx, raw) in ctx.lines.iter().enumerate() {
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
            ctx,
            syntax,
            idx,
            line_no,
            detection_trimmed,
            &kind,
            &family,
        ) {
            continue;
        }
        if !seen.insert(site_key(line_no, &kind, &family)) {
            continue;
        }
        if !line_changed(ctx, line_no, line_no, &kind) {
            continue;
        }
        sites.push(build_fallback_site(
            ctx,
            idx,
            line_no,
            trimmed,
            detection_trimmed,
            kind,
            family,
        ));
    }
    sites
}

fn fallback_is_shadowed_by_syntax(
    ctx: &ScanContext<'_>,
    syntax: &SyntaxScan,
    idx: usize,
    line_no: usize,
    detection_trimmed: &str,
    kind: &UnsafeSiteKind,
    family: &OperationFamily,
) -> bool {
    syntax_site_covers_fallback(&syntax.sites, line_no, kind, family)
        || (*kind == UnsafeSiteKind::Operation
            && *family == OperationFamily::Transmute
            && is_incomplete_multiline_transmute_copy(detection_trimmed)
            && syntax_operation_covers_fallback(&syntax.sites, line_no, family))
        || (*kind == UnsafeSiteKind::UnsafeBlock
            && *family == OperationFamily::Unknown
            && (syntax.operation_lines.contains(&line_no)
                || syntax.operation_block_lines.contains(&line_no)
                || fallback_unsafe_block_contains_specific_operation(ctx.lines, idx)))
}

fn build_fallback_site(
    ctx: &ScanContext<'_>,
    idx: usize,
    line_no: usize,
    trimmed: &str,
    detection_trimmed: &str,
    kind: UnsafeSiteKind,
    family: OperationFamily,
) -> ScannedSite {
    let raw = ctx.lines[idx];
    let owner = fallback_owner(ctx.lines, idx, detection_trimmed, &kind, &family)
        .or_else(|| find_owner(ctx.lines, idx));
    ScannedSite {
        site: UnsafeSite {
            location: SourceLocation::new(ctx.rel.clone(), line_no, first_non_ws_column(raw)),
            kind: kind.clone(),
            owner,
            visibility: visibility_for_snippet(trimmed).to_string(),
            public_api_surface: is_public_api_surface(&kind, trimmed),
            changed: true,
            snippet: trimmed.to_string(),
        },
        operation: UnsafeOperation {
            family,
            expression: trimmed.to_string(),
        },
        context_before: context_before_site(ctx.lines, idx),
        context_after: context_slice(ctx.lines, idx + 1, (idx + 8).min(ctx.lines.len())),
    }
}

fn fallback_owner(
    lines: &[&str],
    idx: usize,
    detection_trimmed: &str,
    kind: &UnsafeSiteKind,
    family: &OperationFamily,
) -> Option<String> {
    match (kind, family) {
        (UnsafeSiteKind::ExternBlock, OperationFamily::Ffi) => find_extern_block_owner(lines, idx),
        (UnsafeSiteKind::Operation, OperationFamily::TargetFeature) => {
            find_following_fn_owner(lines, idx)
        }
        (UnsafeSiteKind::StaticMut, OperationFamily::StaticMut) => {
            parse_static_mut_name(detection_trimmed)
        }
        _ => None,
    }
}

fn collect_syntax_sites(
    ctx: &ScanContext<'_>,
    syntax: &SyntaxScan,
    seen: &mut BTreeSet<(usize, String, String)>,
) -> Vec<ScannedSite> {
    syntax
        .sites
        .iter()
        .filter(|detected| syntax_site_should_emit(detected, syntax, seen))
        .filter(|detected| line_changed(ctx, detected.line, detected.end_line, &detected.kind))
        .map(|detected| build_syntax_site(ctx, detected))
        .collect()
}

fn syntax_site_should_emit(
    detected: &DetectedSyntaxSite,
    syntax: &SyntaxScan,
    seen: &mut BTreeSet<(usize, String, String)>,
) -> bool {
    if detected.kind == UnsafeSiteKind::UnsafeBlock
        && detected.family == OperationFamily::Unknown
        && syntax.operation_lines.contains(&detected.line)
    {
        return false;
    }
    seen.insert(site_key(detected.line, &detected.kind, &detected.family))
}

fn build_syntax_site(ctx: &ScanContext<'_>, detected: &DetectedSyntaxSite) -> ScannedSite {
    let idx = detected.line.saturating_sub(1);
    ScannedSite {
        site: UnsafeSite {
            location: SourceLocation::new(ctx.rel.clone(), detected.line, detected.column),
            kind: detected.kind.clone(),
            owner: syntax_owner(detected, ctx.lines, idx),
            visibility: visibility_for_snippet(&detected.source_snippet).to_string(),
            public_api_surface: is_public_api_surface(&detected.kind, &detected.source_snippet),
            changed: true,
            snippet: detected.card_snippet.clone(),
        },
        operation: UnsafeOperation {
            family: detected.family.clone(),
            expression: detected.card_snippet.clone(),
        },
        context_before: context_before_site(ctx.lines, idx),
        context_after: context_slice(
            ctx.lines,
            (idx + 1).min(ctx.lines.len()),
            (idx + 8).min(ctx.lines.len()),
        ),
    }
}

fn line_changed(
    ctx: &ScanContext<'_>,
    line: usize,
    end_line: usize,
    kind: &UnsafeSiteKind,
) -> bool {
    ctx.diff.is_none_or(|diff| {
        ctx.repo_mode
            || if syntax_site_uses_exact_range(kind) {
                diff.contains_in_range(ctx.rel, line, end_line)
            } else {
                diff.contains_near(ctx.rel, line)
            }
    })
}
