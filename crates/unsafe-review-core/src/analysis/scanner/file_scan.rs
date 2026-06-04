use super::{
    ScannedSite, detect_syntax_sites, extern_fn_names, fallback_scan, js_buffer_reentry,
    js_shared_byte_source, local_module_names, panic_from_safe_js, syntax_scan,
};
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
    let syntax_index = syntax_scan::SyntaxSiteIndex::new(&parsed, &syntax_sites);
    let mut seen = BTreeSet::new();

    let mut out = fallback_scan::sites(
        rel,
        diff,
        repo_mode,
        &lines,
        &syntax_sites,
        &syntax_index,
        &mut seen,
    );
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
    Ok(out)
}
