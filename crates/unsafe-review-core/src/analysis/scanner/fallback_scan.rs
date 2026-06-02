use super::{
    ScannedSite, detect_site, is_incomplete_multiline_transmute_copy, line_for_text_detection,
    scan_site, site_key, syntax_operation_covers_fallback, syntax_scan::SyntaxSiteIndex,
    syntax_site_covers_fallback,
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
        || (*input.kind == UnsafeSiteKind::UnsafeBlock
            && *input.family == OperationFamily::Unknown
            && input
                .syntax_index
                .covers_specific_operation(input.line_no, input.lines, input.idx))
}
