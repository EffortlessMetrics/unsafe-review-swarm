use super::{ScannedSite, fallback_unsafe_block_contains_specific_operation};
use super::{operation_block_start_lines, scan_site, site_key};
use crate::domain::{OperationFamily, UnsafeSiteKind};
use crate::input::diff::DiffIndex;
use std::collections::BTreeSet;
use std::path::PathBuf;

pub(super) struct SyntaxSiteIndex {
    operation_lines: BTreeSet<usize>,
    operation_block_lines: BTreeSet<usize>,
}

impl SyntaxSiteIndex {
    pub(super) fn new(
        parsed: &super::ParsedSource,
        syntax_sites: &[super::DetectedSyntaxSite],
    ) -> Self {
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

    pub(super) fn covers_specific_operation(
        &self,
        line_no: usize,
        lines: &[&str],
        idx: usize,
    ) -> bool {
        self.operation_lines.contains(&line_no)
            || self.operation_block_lines.contains(&line_no)
            || fallback_unsafe_block_contains_specific_operation(lines, idx)
    }
}

pub(super) fn backfill_sites(
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
