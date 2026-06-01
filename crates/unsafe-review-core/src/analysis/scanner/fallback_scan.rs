use super::{
    DetectedSyntaxSite, LineCommentState, ScannedSite,
    fallback_unsafe_block_contains_specific_operation, find_extern_block_owner,
    find_following_fn_owner, find_owner, first_non_ws_column, is_public_api_surface,
    line_for_text_detection, parse_static_mut_name, site_key, syntax_operation_covers_fallback,
    syntax_site_covers_fallback, visibility_for_snippet,
};
use crate::domain::{OperationFamily, SourceLocation, UnsafeOperation, UnsafeSite, UnsafeSiteKind};
use crate::input::diff::DiffIndex;
use std::collections::BTreeSet;
use std::path::PathBuf;

pub(super) fn scan_text_fallback_sites(
    rel: &PathBuf,
    diff: Option<&DiffIndex>,
    repo_mode: bool,
    lines: &[&str],
    syntax_sites: &[DetectedSyntaxSite],
    syntax_operation_lines: &BTreeSet<usize>,
    syntax_operation_block_lines: &BTreeSet<usize>,
    seen: &mut BTreeSet<(usize, String, String)>,
) -> Vec<ScannedSite> {
    let mut sites = Vec::new();
    let mut line_comment_state = LineCommentState::default();
    for (idx, raw) in lines.iter().enumerate() {
        let line_no = idx + 1;
        let trimmed = raw.trim();
        let detection_line = line_for_text_detection(raw, &mut line_comment_state);
        let detection_trimmed = detection_line.trim();
        if detection_trimmed.is_empty() {
            continue;
        }
        let Some((kind, family)) = super::detect_site(detection_trimmed) else {
            continue;
        };
        if fallback_is_covered_by_syntax(
            lines,
            idx,
            line_no,
            detection_trimmed,
            &kind,
            &family,
            syntax_sites,
            syntax_operation_lines,
            syntax_operation_block_lines,
        ) {
            continue;
        }
        seen.insert(site_key(line_no, &kind, &family));
        let changed =
            super::change_filter::site_changed(diff, repo_mode, rel, line_no, line_no, &kind);
        if !changed && !repo_mode {
            continue;
        }
        let owner = fallback_owner(lines, idx, detection_trimmed, &kind, &family);
        let public_api_surface = is_public_api_surface(&kind, trimmed);
        sites.push(ScannedSite {
            site: UnsafeSite {
                location: SourceLocation::new(rel.clone(), line_no, first_non_ws_column(raw)),
                kind,
                owner,
                visibility: visibility_for_snippet(trimmed).to_string(),
                public_api_surface,
                changed,
                snippet: trimmed.to_string(),
            },
            operation: UnsafeOperation {
                family,
                expression: trimmed.to_string(),
            },
            context_before: super::context_before_site(lines, idx),
            context_after: super::context_slice(lines, idx + 1, (idx + 8).min(lines.len())),
        });
    }
    sites
}

fn fallback_is_covered_by_syntax(
    lines: &[&str],
    idx: usize,
    line_no: usize,
    detection_trimmed: &str,
    kind: &UnsafeSiteKind,
    family: &OperationFamily,
    syntax_sites: &[DetectedSyntaxSite],
    syntax_operation_lines: &BTreeSet<usize>,
    syntax_operation_block_lines: &BTreeSet<usize>,
) -> bool {
    syntax_site_covers_fallback(syntax_sites, line_no, kind, family)
        || (*kind == UnsafeSiteKind::Operation
            && *family == OperationFamily::Transmute
            && super::is_incomplete_multiline_transmute_copy(detection_trimmed)
            && syntax_operation_covers_fallback(syntax_sites, line_no, family))
        || (*kind == UnsafeSiteKind::UnsafeBlock
            && *family == OperationFamily::Unknown
            && (syntax_operation_lines.contains(&line_no)
                || syntax_operation_block_lines.contains(&line_no)
                || fallback_unsafe_block_contains_specific_operation(lines, idx)))
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
    .or_else(|| find_owner(lines, idx))
}
