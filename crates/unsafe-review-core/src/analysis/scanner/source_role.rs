//! Source-role classification for review triage.
//!
//! Every scanned site gets a [`SourceRole`](crate::domain::SourceRole) from
//! path plus content signals, evaluated in fixed order: generated, test,
//! production, unknown. Unknown is the default; only positively-evidenced
//! roles classify. Classification labels and aggregates only: it never
//! suppresses a card, removes inventory, or enters card identity.

use std::path::Path;

use crate::domain::SourceRole;

/// Directory names that are explicitly neither test nor production signals.
/// They are stripped before path matching so a `fixtures`-named crate keeps
/// its `src/` production rule and `examples/` or `testdata` never
/// masquerade as either bucket.
const ROLE_NEUTRAL_COMPONENTS: &[&str] = &["fixtures", "fixture", "testdata", "examples"];

/// Directory names that evidence test code.
const TEST_DIR_COMPONENTS: &[&str] = &["tests"];

/// Maximum lines above the site to search for a `#[cfg(test)]` attribute.
const CFG_TEST_SCAN_BACK_LINES: usize = 40;

/// Maximum lines to walk forward when balancing a `#[cfg(test)]` region.
const CFG_TEST_REGION_LIMIT_LINES: usize = 2000;

pub(super) fn classify_source_role(rel: &Path, lines: &[&str], site_line: usize) -> SourceRole {
    if is_generated_path(rel) || has_generated_marker(lines) {
        return SourceRole::Generated;
    }
    if is_test_path(rel) || site_in_cfg_test_region(lines, site_line) {
        return SourceRole::Test;
    }
    if has_source_component(rel) {
        return SourceRole::Production;
    }
    SourceRole::Unknown
}

/// Path components with neutral names removed. The neutral list is load
/// bearing: without it a `fixtures`-named production crate or an
/// `examples/` demo could be misread by future signal rules; stripping
/// keeps every downstream rule honest.
fn signal_components(rel: &Path) -> Vec<String> {
    rel.components()
        .filter_map(|component| {
            let text = component.as_os_str().to_str()?;
            if ROLE_NEUTRAL_COMPONENTS.contains(&text) {
                return None;
            }
            Some(text.to_string())
        })
        .collect()
}

fn file_stem(rel: &Path) -> Option<String> {
    rel.file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::to_string)
}

fn is_generated_path(rel: &Path) -> bool {
    if signal_components(rel)
        .iter()
        .any(|component| component.contains("generated"))
    {
        return true;
    }
    file_stem(rel).is_some_and(|stem| stem.contains("generated"))
}

fn has_generated_marker(lines: &[&str]) -> bool {
    lines
        .iter()
        .any(|line| line.to_ascii_lowercase().contains("@generated"))
}

fn is_test_path(rel: &Path) -> bool {
    if signal_components(rel)
        .iter()
        .any(|component| TEST_DIR_COMPONENTS.contains(&component.as_str()))
    {
        return true;
    }
    match file_stem(rel) {
        Some(stem) => {
            stem == "test" || stem == "tests" || stem.ends_with("_test") || stem.ends_with("_tests")
        }
        None => false,
    }
}

fn has_source_component(rel: &Path) -> bool {
    signal_components(rel)
        .iter()
        .any(|component| component == "src")
}

/// Reports whether the 1-based `site_line` falls inside a `#[cfg(test)]`
/// item region (module or function). Production seams in the same file but
/// outside every test region stay production: scoping is per site, never
/// per file.
fn site_in_cfg_test_region(lines: &[&str], site_line: usize) -> bool {
    if site_line == 0 || site_line > lines.len() {
        return false;
    }
    if !lines.iter().any(|line| line.contains("cfg(test)")) {
        return false;
    }
    let site_idx = site_line - 1;
    let scan_from = site_idx.saturating_sub(CFG_TEST_SCAN_BACK_LINES);
    for attr_idx in (scan_from..site_idx).rev() {
        if !is_cfg_test_attribute(lines[attr_idx]) {
            continue;
        }
        if let Some((open_idx, close_idx)) = balanced_region(lines, attr_idx)
            && attr_idx <= site_idx
            && site_idx <= close_idx
            && open_idx <= site_idx
        {
            return true;
        }
    }
    false
}

fn is_cfg_test_attribute(line: &str) -> bool {
    let compact: String = line.chars().filter(|ch| !ch.is_whitespace()).collect();
    compact.contains("#[cfg(test)]")
}

/// Balances the first `{...}` region starting at or after `from`, capped at
/// [`CFG_TEST_REGION_LIMIT_LINES`] lines. Returns the opening and closing
/// line indexes. String literals are not tracked; test-region detection
/// only needs the common `mod`/`fn` shapes.
fn balanced_region(lines: &[&str], from: usize) -> Option<(usize, usize)> {
    let limit = (from + CFG_TEST_REGION_LIMIT_LINES).min(lines.len());
    let mut depth = 0i32;
    let mut open_idx = None;
    for (idx, line) in lines.iter().enumerate().take(limit).skip(from) {
        let code = line.split("//").next().unwrap_or(line);
        for ch in code.chars() {
            if ch == '{' {
                if open_idx.is_none() {
                    open_idx = Some(idx);
                }
                depth += 1;
            } else if ch == '}' {
                depth -= 1;
                if depth == 0 {
                    return open_idx.map(|open| (open, idx));
                }
                if depth < 0 {
                    return None;
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classify(rel: &str, body: &str, site_line: usize) -> SourceRole {
        let lines: Vec<&str> = body.lines().collect();
        classify_source_role(Path::new(rel), &lines, site_line)
    }

    const PRODUCTION_BODY: &str =
        "pub fn run() -> i32 {\n    let result = unsafe { checkable() };\n    result\n}\n";

    #[test]
    fn cfg_test_module_in_src_is_test() {
        let body = "#[cfg(test)]\npub mod checks {\n    pub fn helper(ptr: *const u8) -> u8 {\n        unsafe { *ptr }\n    }\n}\n";
        assert_eq!(classify("src/tested.rs", body, 4), SourceRole::Test);
    }

    #[test]
    fn production_seam_beside_test_module_stays_production() {
        let body = "pub fn prod(ptr: *const u8) -> u8 {\n    unsafe { *ptr }\n}\n\n#[cfg(test)]\nmod checks {\n    fn t() {}\n}\n";
        assert_eq!(classify("src/lib.rs", body, 2), SourceRole::Production);
    }

    #[test]
    fn fixtures_named_crate_file_is_production() {
        assert_eq!(
            classify("fixtures/checkout/src/lib.rs", PRODUCTION_BODY, 2),
            SourceRole::Production
        );
    }

    #[test]
    fn generated_marker_file_is_generated() {
        let body = "//! @generated by codegen. Do not edit.\n\npub fn get(buf: &[u8], idx: usize) -> u8 {\n    unsafe { *buf.get_unchecked(idx) }\n}\n";
        assert_eq!(classify("src/generated.rs", body, 4), SourceRole::Generated);
    }

    #[test]
    fn generated_path_is_generated_without_marker() {
        assert_eq!(
            classify("src/generated/pins.rs", PRODUCTION_BODY, 2),
            SourceRole::Generated
        );
    }

    #[test]
    fn tests_dir_is_test_despite_fixtures_component() {
        assert_eq!(
            classify("tests/fixtures/byte_input.rs", PRODUCTION_BODY, 2),
            SourceRole::Test
        );
    }

    #[test]
    fn shared_ambiguous_include_is_unknown() {
        assert_eq!(
            classify("shared/span.rs", PRODUCTION_BODY, 2),
            SourceRole::Unknown
        );
    }

    #[test]
    fn example_is_unknown_by_default() {
        assert_eq!(
            classify("examples/demo.rs", PRODUCTION_BODY, 2),
            SourceRole::Unknown
        );
    }

    #[test]
    fn neutral_names_alone_never_classify() {
        assert_eq!(
            classify("fixtures/helpers.rs", PRODUCTION_BODY, 2),
            SourceRole::Unknown
        );
        assert_eq!(
            classify("testdata/input.rs", PRODUCTION_BODY, 2),
            SourceRole::Unknown
        );
    }

    #[test]
    fn underscore_test_stem_is_test() {
        assert_eq!(
            classify("src/decoder_test.rs", PRODUCTION_BODY, 2),
            SourceRole::Test
        );
    }

    #[test]
    fn src_file_is_production() {
        assert_eq!(
            classify("src/lib.rs", PRODUCTION_BODY, 2),
            SourceRole::Production
        );
        assert_eq!(
            classify("crates/tool/src/main.rs", PRODUCTION_BODY, 2),
            SourceRole::Production
        );
    }

    #[test]
    fn root_level_file_is_unknown() {
        assert_eq!(
            classify("build.rs", PRODUCTION_BODY, 2),
            SourceRole::Unknown
        );
    }
}
