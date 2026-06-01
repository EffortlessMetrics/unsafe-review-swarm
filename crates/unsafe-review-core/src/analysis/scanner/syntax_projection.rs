use super::{
    DetectedSyntaxSite, ScannedSite, context_before_site, context_slice, is_public_api_surface,
    site_key, syntax_owner, visibility_for_snippet,
};
use crate::domain::{OperationFamily, SourceLocation, UnsafeOperation, UnsafeSite, UnsafeSiteKind};
use crate::input::diff::DiffIndex;
use std::collections::BTreeSet;
use std::path::PathBuf;

pub(super) fn syntax_operation_lines(syntax_sites: &[DetectedSyntaxSite]) -> BTreeSet<usize> {
    syntax_sites
        .iter()
        .filter(|site| site.kind == UnsafeSiteKind::Operation)
        .map(|site| site.line)
        .collect()
}

pub(super) fn scan_syntax_sites(
    rel: &PathBuf,
    diff: Option<&DiffIndex>,
    repo_mode: bool,
    lines: &[&str],
    syntax_sites: Vec<DetectedSyntaxSite>,
    syntax_operation_lines: &BTreeSet<usize>,
    seen: &mut BTreeSet<(usize, String, String)>,
) -> Vec<ScannedSite> {
    let mut sites = Vec::new();
    for detected in syntax_sites {
        if syntax_unknown_block_is_shadowed(&detected, syntax_operation_lines) {
            continue;
        }
        if !seen.insert(site_key(detected.line, &detected.kind, &detected.family)) {
            continue;
        }
        let changed = super::change_filter::site_changed(
            diff,
            repo_mode,
            rel,
            detected.line,
            detected.end_line,
            &detected.kind,
        );
        if !changed && !repo_mode {
            continue;
        }
        let idx = detected.line.saturating_sub(1);
        let owner = syntax_owner(&detected, lines, idx);
        let public_api_surface = is_public_api_surface(&detected.kind, &detected.source_snippet);
        sites.push(ScannedSite {
            site: UnsafeSite {
                location: SourceLocation::new(rel.clone(), detected.line, detected.column),
                kind: detected.kind,
                owner,
                visibility: visibility_for_snippet(&detected.source_snippet).to_string(),
                public_api_surface,
                changed,
                snippet: detected.card_snippet.clone(),
            },
            operation: UnsafeOperation {
                family: detected.family,
                expression: detected.card_snippet,
            },
            context_before: context_before_site(lines, idx),
            context_after: context_slice(
                lines,
                (idx + 1).min(lines.len()),
                (idx + 8).min(lines.len()),
            ),
        });
    }
    sites
}

fn syntax_unknown_block_is_shadowed(
    detected: &DetectedSyntaxSite,
    syntax_operation_lines: &BTreeSet<usize>,
) -> bool {
    detected.kind == UnsafeSiteKind::UnsafeBlock
        && detected.family == OperationFamily::Unknown
        && syntax_operation_lines.contains(&detected.line)
}
