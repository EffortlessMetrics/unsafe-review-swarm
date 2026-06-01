use super::{
    ScannedSite, detect_site, detect_syntax_sites, extern_fn_names,
    fallback_unsafe_block_contains_specific_operation, is_incomplete_multiline_transmute_copy,
    js_buffer_reentry, line_for_text_detection, local_module_names, operation_block_start_lines,
    scan_site, site_key, syntax_operation_covers_fallback, syntax_site_covers_fallback,
};
use crate::domain::{OperationFamily, UnsafeSiteKind};
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
    out.extend(js_buffer_reentry::detect_js_buffer_reentry_sites(
        rel, diff, repo_mode, &lines,
    ));
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
        if let Some(site) = scan_site::syntax_site(rel, diff, repo_mode, lines, detected) {
            out.push(site);
        }
    }
    out
}
