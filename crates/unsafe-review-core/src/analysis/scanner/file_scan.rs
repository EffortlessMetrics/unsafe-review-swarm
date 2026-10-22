use super::{
    ScannedSite, detect_syntax_sites, disposition, extern_fn_names, fallback_scan,
    js_buffer_reentry, js_native_ffi_byte_source, js_shared_byte_source, local_module_names,
    panic_from_safe_js, syntax_scan,
};
use crate::input::diff::DiffIndex;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Result of scanning a single file: the detected sites and the wall-clock
/// milliseconds spent on parse + site detection.
#[derive(Debug)]
pub(crate) struct FileScanResult {
    pub(crate) sites: Vec<ScannedSite>,
    /// Wall-clock milliseconds for parse + all detection passes on this file.
    /// Diagnostic only — not a proof, coverage claim, or performance guarantee.
    pub(crate) scan_ms: u64,
    /// Bytes read for this file. Deterministic for identical inputs.
    pub(crate) bytes: u64,
    /// Lines parsed for this file. Deterministic for identical inputs.
    pub(crate) lines: u64,
    /// Per-line record of whether text fallback entered for the
    /// syntax-first `NonNullUnchecked` slice and why. Proves a structural
    /// clean miss was not resurrected by the text path. Read by focused
    /// tests in PR1; canonical card/coverage projection follows in PR3.
    #[allow(dead_code, reason = "read by focused tests in PR1; projected in PR3")]
    pub(crate) fallback_entries: Vec<disposition::FallbackEntry>,
}

pub(crate) fn scan_file(
    root: &Path,
    rel: &PathBuf,
    diff: Option<&DiffIndex>,
    repo_mode: bool,
) -> Result<FileScanResult, String> {
    let file_start = Instant::now();
    let abs = root.join(rel);
    let text =
        fs::read_to_string(&abs).map_err(|err| format!("read {} failed: {err}", abs.display()))?;
    let lines: Vec<&str> = text.lines().collect();
    let parsed = super::super::syntax::parse_source(text.as_str());
    let extern_names = extern_fn_names(&lines);
    let local_modules = local_module_names(&lines);
    let (syntax_sites, nonnull) = detect_syntax_sites(&parsed, &extern_names, &local_modules);
    let syntax_index = syntax_scan::SyntaxSiteIndex::new(&parsed, &syntax_sites);
    let mut seen = BTreeSet::new();

    let mut dispatch = disposition::FallbackDispatch {
        syntax_sites: &syntax_sites,
        syntax_index: &syntax_index,
        nonnull: &nonnull,
        entries: Vec::new(),
    };
    let mut out = fallback_scan::sites(rel, diff, repo_mode, &lines, &mut dispatch, &mut seen);
    let fallback_entries = dispatch.entries;
    out.extend(syntax_scan::backfill_sites(
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
    out.extend(js_shared_byte_source::detect_js_shared_byte_sites(
        rel, diff, repo_mode, &lines,
    ));
    out.extend(js_native_ffi_byte_source::detect_js_native_ffi_byte_sites(
        rel, diff, repo_mode, &lines,
    ));
    out.extend(panic_from_safe_js::detect_panic_from_safe_js_sites(
        rel, diff, repo_mode, &lines,
    ));
    out.sort_by(|left, right| {
        left.site
            .location
            .line
            .cmp(&right.site.location.line)
            .then(left.site.location.column.cmp(&right.site.location.column))
    });
    let scan_ms = file_start
        .elapsed()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX);
    Ok(FileScanResult {
        sites: out,
        scan_ms,
        bytes: text.len() as u64,
        lines: lines.len() as u64,
        fallback_entries,
    })
}
