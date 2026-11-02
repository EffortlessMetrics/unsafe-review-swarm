#![forbid(unsafe_code)]

use std::path::Path;

use crate::read_to_string;

// Current-release truth checker (standalone; deliberately not wired into
// `check-pr` or any CI gate in this slice).
//
// The authority file `RELEASE_TRUTH_AUTHORITY` records the current release
// (0.5.0 registry/tag/Release/smoke state) separately from the workspace/dev
// version (0.3.8 workbench). This checker parses that authority offline and
// scans the four front-panel surfaces for release claims, failing on:
//
// - stale prior-release versions named as current,
// - wrong tag targets,
// - claimed-but-absent tags or GitHub Releases,
// - missing required fields (authority keys or per-surface release claims),
// - cross-panel disagreement about the current release.
//
// No network access: only checked-in text is read.

pub(crate) const RELEASE_TRUTH_AUTHORITY: &str = "policy/release-truth.toml";

/// Front-panel surfaces scanned for release claims, with the field label each
/// surface's release claim is reported under.
pub(crate) const RELEASE_TRUTH_SURFACES: &[(&str, &str)] = &[
    ("README.md", "release-row"),
    ("docs/status/SUPPORT_SUMMARY.md", "public-release-claim"),
    (
        "docs/status/RELEASE_COMPATIBILITY.md",
        "distribution-posture",
    ),
    ("docs/status/RELEASE_DOCS_MATRIX.md", "availability-matrix"),
];

/// Line markers (case-insensitive) that identify a currency claim: the line
/// asserts what is current/latest/publicly available, so every `X.Y.Z`
/// version on the line must equal the authority's current release.
const CURRENCY_MARKERS: &[&str] = &[
    "current release",
    "latest",
    "last public",
    "last published",
    "ships in public",
    "ship in public",
    "published packages are",
    "cargo install",
    "--version",
    "install the public",
    "installation path",
    "public v",
    "public `v",
];

#[allow(
    dead_code,
    reason = "authority contract data is parsed and validated here; PR3 runbook mapping reads the remaining fields"
)]
pub(crate) struct ReleaseTruth {
    pub(crate) current: String,
    pub(crate) workspace_version: String,
    pub(crate) public_source_commit: String,
    pub(crate) tag_exists: bool,
    pub(crate) tag_target: String,
    pub(crate) release_exists: bool,
    pub(crate) release_published_at: String,
}

pub(crate) fn check_release_truth() -> Result<(), String> {
    let authority_text = read_to_string(Path::new(RELEASE_TRUTH_AUTHORITY))?;
    let truth = parse_release_truth_text(RELEASE_TRUTH_AUTHORITY, &authority_text)?;
    let mut panels = Vec::new();
    for (surface, field) in RELEASE_TRUTH_SURFACES {
        let text = read_to_string(Path::new(surface))?;
        panels.push((*surface, *field, text));
    }
    let borrowed: Vec<(&str, &str, &str)> = panels
        .iter()
        .map(|(surface, field, text)| (*surface, *field, text.as_str()))
        .collect();
    check_release_truth_texts(&truth, &borrowed)?;
    println!("check-release-truth: ok");
    Ok(())
}

pub(crate) fn parse_release_truth_text(path: &str, text: &str) -> Result<ReleaseTruth, String> {
    let table: toml::Table = text
        .parse()
        .map_err(|err| format!("{path} is not valid TOML: {err}"))?;
    let value = toml::Value::Table(table);
    let workspace_version = required_top_string(&value, path, "workspace_version")?;
    let public_source_commit = required_top_string(&value, path, "public_source_commit")?;
    let installed_smoke_receipt = required_top_string(&value, path, "installed_smoke_receipt")?;
    if installed_smoke_receipt.trim().is_empty() {
        return Err(format!(
            "{path} string key `installed_smoke_receipt` is empty"
        ));
    }
    let workbench = required_table(&value, path, "workbench")?;
    let workbench_version = required_table_string(workbench, path, "workbench", "version")?;
    let workbench_availability =
        required_table_string(workbench, path, "workbench", "availability")?;
    if workbench_version.trim().is_empty() || workbench_availability.trim().is_empty() {
        return Err(format!(
            "{path} [workbench] `version`/`availability` must not be empty"
        ));
    }
    let crates_table = required_table(&value, path, "crates_io")?;
    for crate_name in ["unsafe_review_core", "unsafe_review_cli", "unsafe_review"] {
        let state = required_table_string(crates_table, path, "crates_io", crate_name)?;
        if state != "published" {
            return Err(format!(
                "{path} [crates_io] `{crate_name}` is `{state}`, expected `published`"
            ));
        }
    }
    let tag = required_table(&value, path, "tag")?;
    let tag_exists = required_table_bool(tag, path, "tag", "exists")?;
    let tag_target = if tag_exists {
        required_table_string(tag, path, "tag", "target")?.to_string()
    } else {
        String::new()
    };
    let release = required_table(&value, path, "github_release")?;
    let release_exists = required_table_bool(release, path, "github_release", "exists")?;
    let release_published_at = if release_exists {
        required_table_string(release, path, "github_release", "published_at")?.to_string()
    } else {
        String::new()
    };
    let crates_version = required_table_string(crates_table, path, "crates_io", "version")?;
    Ok(ReleaseTruth {
        current: crates_version.to_string(),
        workspace_version: workspace_version.to_string(),
        public_source_commit: public_source_commit.to_string(),
        tag_exists,
        tag_target,
        release_exists,
        release_published_at,
    })
}

fn required_top_string<'a>(
    value: &'a toml::Value,
    path: &str,
    key: &str,
) -> Result<&'a str, String> {
    match value.get(key).and_then(toml::Value::as_str) {
        Some(text) if !text.trim().is_empty() => Ok(text),
        _ => Err(format!("{path} is missing required field `{key}`")),
    }
}

fn required_table<'a>(
    value: &'a toml::Value,
    path: &str,
    table: &str,
) -> Result<&'a toml::map::Map<String, toml::Value>, String> {
    value
        .get(table)
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{path} is missing required field `[{table}]`"))
}

fn required_table_string<'a>(
    table: &'a toml::map::Map<String, toml::Value>,
    path: &str,
    table_name: &str,
    key: &str,
) -> Result<&'a str, String> {
    match table.get(key).and_then(toml::Value::as_str) {
        Some(text) if !text.trim().is_empty() => Ok(text),
        _ => Err(format!(
            "{path} is missing required field `[{table_name}] {key}`"
        )),
    }
}

fn required_table_bool(
    table: &toml::map::Map<String, toml::Value>,
    path: &str,
    table_name: &str,
    key: &str,
) -> Result<bool, String> {
    table
        .get(key)
        .and_then(toml::Value::as_bool)
        .ok_or_else(|| format!("{path} is missing required field `[{table_name}] {key}`"))
}

/// Check in-memory panel texts against parsed authority truth.
///
/// `panels` entries are `(surface path, field label, panel text)`. Every
/// error names the surface and field.
pub(crate) fn check_release_truth_texts(
    truth: &ReleaseTruth,
    panels: &[(&str, &str, &str)],
) -> Result<(), String> {
    let mut claims: Vec<(String, String, usize, String)> = Vec::new();
    for (surface, field, text) in panels {
        let mut surface_claims = 0usize;
        for (idx, line) in text.lines().enumerate() {
            let line_no = idx + 1;
            let lowered = line.to_ascii_lowercase();
            if CURRENCY_MARKERS
                .iter()
                .any(|marker| lowered.contains(marker))
            {
                for version in release_versions_in_line(line) {
                    claims.push((
                        (*surface).to_string(),
                        (*field).to_string(),
                        line_no,
                        version,
                    ));
                    surface_claims += 1;
                }
            }
            check_tag_claim(truth, surface, field, line, &lowered, line_no)?;
            check_release_claim(truth, surface, field, line, &lowered, line_no)?;
        }
        if surface_claims == 0 {
            return Err(format!(
                "{surface} is missing required `{field}` release claim"
            ));
        }
    }
    let mut distinct: Vec<String> = Vec::new();
    for (_, _, _, version) in &claims {
        if !distinct.iter().any(|seen| seen == version) {
            distinct.push(version.clone());
        }
    }
    if distinct.len() > 1 {
        let mut detail = Vec::new();
        for version in &distinct {
            let first = claims.iter().find(|claim| &claim.3 == version);
            match first {
                Some((surface, field, line_no, _)) => detail.push(format!(
                    "`{version}` claimed by {surface} `{field}` line {line_no}"
                )),
                None => detail.push(format!("`{version}` claimed by an unknown surface")),
            }
        }
        return Err(format!(
            "release panels disagree on the current release: {}; authority records `{}`",
            detail.join(", "),
            truth.current
        ));
    }
    match distinct.first() {
        Some(version) if *version != truth.current => {
            let mut location = "an unknown surface `unknown-field`".to_string();
            for (surface, field, line_no, claimed) in &claims {
                if claimed == version {
                    location = format!("{surface}:{line_no} `{field}`");
                    break;
                }
            }
            Err(format!(
                "{location} claims stale prior-release version `{version}`; current release is `{}`",
                truth.current
            ))
        }
        _ => Ok(()),
    }
}

fn check_tag_claim(
    truth: &ReleaseTruth,
    surface: &str,
    field: &str,
    line: &str,
    lowered: &str,
    line_no: usize,
) -> Result<(), String> {
    if !lowered.contains("tag") {
        return Ok(());
    }
    let versions = release_versions_in_line(line);
    let claimed_current = versions.contains(&truth.current);
    if !claimed_current {
        return Ok(());
    }
    if !truth.tag_exists {
        if lowered.contains("pending") || lowered.contains("unavailable") {
            return Ok(());
        }
        return Err(format!(
            "{surface}:{line_no} `{field}` claims tag `v{}` but the authority records the tag as pending",
            truth.current
        ));
    }
    for sha in hex_tokens_in_line(line) {
        if sha_targets_match(&sha, &truth.tag_target) {
            continue;
        }
        return Err(format!(
            "{surface}:{line_no} `{field}` claims tag target `{sha}` but the authority records `{}`",
            truth.tag_target
        ));
    }
    Ok(())
}

fn check_release_claim(
    truth: &ReleaseTruth,
    surface: &str,
    field: &str,
    line: &str,
    lowered: &str,
    line_no: usize,
) -> Result<(), String> {
    if !lowered.contains("github release") {
        return Ok(());
    }
    let versions = release_versions_in_line(line);
    let claimed_current = versions.contains(&truth.current);
    if !claimed_current {
        return Ok(());
    }
    if truth.release_exists {
        return Ok(());
    }
    if lowered.contains("pending") || lowered.contains("unavailable") {
        return Ok(());
    }
    Err(format!(
        "{surface}:{line_no} `{field}` claims GitHub Release `v{}` but the authority records no published release",
        truth.current
    ))
}

fn sha_targets_match(panel_sha: &str, authority_target: &str) -> bool {
    let panel = panel_sha.to_ascii_lowercase();
    let target = authority_target.to_ascii_lowercase();
    if panel.is_empty() || target.is_empty() {
        return true;
    }
    panel.starts_with(&target) || target.starts_with(&panel)
}

/// Extract `X.Y.Z` version tokens (with optional leading `v`) from a line.
pub(crate) fn release_versions_in_line(line: &str) -> Vec<String> {
    let bytes = line.as_bytes();
    let mut versions = Vec::new();
    let mut idx = 0;
    while idx < bytes.len() {
        let byte = bytes[idx];
        let starts_version = byte.is_ascii_digit()
            || (byte == b'v' && idx + 1 < bytes.len() && bytes[idx + 1].is_ascii_digit());
        if !starts_version {
            idx += 1;
            continue;
        }
        if idx > 0 {
            let prev = bytes[idx - 1];
            if prev.is_ascii_digit() || prev == b'.' {
                idx += 1;
                continue;
            }
        }
        let mut cursor = if byte == b'v' { idx + 1 } else { idx };
        let mut parts = 0;
        while parts < 3 {
            let start = cursor;
            while cursor < bytes.len() && bytes[cursor].is_ascii_digit() {
                cursor += 1;
            }
            if cursor == start {
                break;
            }
            parts += 1;
            if parts == 3 {
                break;
            }
            if cursor < bytes.len() && bytes[cursor] == b'.' {
                cursor += 1;
            } else {
                break;
            }
        }
        if parts == 3
            && (cursor >= bytes.len() || (!bytes[cursor].is_ascii_digit() && bytes[cursor] != b'.'))
        {
            let token_start = if byte == b'v' { idx + 1 } else { idx };
            versions.push(line[token_start..cursor].to_string());
            idx = cursor;
        } else {
            idx += 1;
        }
    }
    versions
}

/// Extract hex tokens of length 7..=40 (candidate SHAs) from a line.
fn hex_tokens_in_line(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in line.chars() {
        if ch.is_ascii_hexdigit() {
            current.push(ch);
        } else {
            if (7..=40).contains(&current.len()) {
                tokens.push(current.clone());
            }
            current.clear();
        }
    }
    if (7..=40).contains(&current.len()) {
        tokens.push(current);
    }
    tokens
}
