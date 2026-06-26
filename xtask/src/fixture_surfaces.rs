//! Fixture surface-golden gates (`check-fixtures`, `check-fixture-surface-parity`,
//! `check-surface-determinism`).
//!
//! Extracted from `main.rs` as part of #1806 (xtask modularization). Verifies the
//! fixture corpus and that committed surface goldens (`expected.lsp.json`,
//! `expected.repair-queue.json`, `expected.comment-plan.json`) are byte-identical
//! to a fresh rendering of each calibration fixture that has `surface_goldens`
//! set, and that rendering is deterministic across repeated generation.
//!
//! The surface-golden gates are deterministic: both surfaces contain no
//! `tool_version`, `generated_at`, or wall-clock timestamp, so re-rendering
//! produces the same bytes on every run.

use crate::{
    FIXTURE_EXPECTED_CARDS_EXCEPTIONS, FIXTURE_PACKAGE_PREFIX_EXCEPTIONS, calibration_manifest,
    check_fixture, fixture_dir_name, fixture_dirs, read_to_string, workspace_path,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub(crate) fn check_fixtures() -> Result<(), String> {
    let dirs = fixture_dirs(Path::new("fixtures"))?;
    if dirs.is_empty() {
        return Err("fixtures directory has no fixture cases".to_string());
    }
    check_fixture_exception_ledgers(&dirs)?;
    for dir in &dirs {
        check_fixture(dir)?;
    }
    println!("check-fixtures: ok ({} fixtures)", dirs.len());
    Ok(())
}

/// Verify that committed surface goldens (`expected.lsp.json`,
/// `expected.repair-queue.json`) are byte-identical to a fresh rendering of
/// each calibration fixture that has `surface_goldens` set.
///
/// The gate is deterministic: both surfaces contain no `tool_version`,
/// `generated_at`, or wall-clock timestamp, so re-rendering produces the same
/// bytes on every run.
pub(crate) fn check_fixture_surface_parity() -> Result<(), String> {
    let manifest = calibration_manifest::validate()?;
    let workspace_root = workspace_path("");
    let mut checked = 0usize;
    let mut mismatches: Vec<String> = Vec::new();

    for (fixture, case) in &manifest.fixture_cases {
        if case.surface_goldens.is_empty() {
            continue;
        }
        for surface in &case.surface_goldens {
            let filename = surface_golden_filename(surface);
            let committed_path = workspace_path(&format!("fixtures/{fixture}/{filename}"));
            let committed = read_to_string(&committed_path).map_err(|err| {
                format!(
                    "check-fixture-surface-parity: fixture `{fixture}` surface `{surface}`: \
                     committed golden `{filename}` missing or unreadable: {err}. \
                     Run `cargo run -p xtask -- bless-goldens` to generate it."
                )
            })?;

            let rendered = unsafe_review_core::render_fixture_surface_from_workspace(
                &workspace_root,
                fixture,
                surface,
            )
            .map_err(|err| {
                format!(
                    "check-fixture-surface-parity: fixture `{fixture}` surface `{surface}`: \
                     render failed: {err}"
                )
            })?;

            if committed != rendered {
                let first_diff = first_differing_line(&committed, &rendered);
                mismatches.push(format!(
                    "  fixture `{fixture}` surface `{surface}` ({filename}): {first_diff}"
                ));
            }
            checked += 1;
        }
    }

    if !mismatches.is_empty() {
        return Err(format!(
            "check-fixture-surface-parity: {} surface golden(s) do not match rendered output \
             (run `cargo run -p xtask -- bless-goldens` to regenerate):\n{}",
            mismatches.len(),
            mismatches.join("\n")
        ));
    }

    println!("check-fixture-surface-parity: ok ({checked} surface goldens verified)");
    Ok(())
}

const SURFACE_DETERMINISM_RUNS: usize = 3;

/// Verify that canonical fixture surfaces render to byte-identical output across
/// repeated generation in one process.
pub(crate) fn check_surface_determinism() -> Result<(), String> {
    let manifest = calibration_manifest::validate()?;
    let workspace_root = workspace_path("");
    let mut checked = 0usize;
    let mut mismatches: Vec<String> = Vec::new();

    for (fixture, case) in &manifest.fixture_cases {
        if case.surface_goldens.is_empty() {
            continue;
        }
        for surface in &case.surface_goldens {
            let baseline = unsafe_review_core::render_fixture_surface_from_workspace(
                &workspace_root,
                fixture,
                surface,
            )
            .map_err(|err| {
                format!(
                    "check-surface-determinism: fixture `{fixture}` surface `{surface}`: \
                     initial render failed: {err}"
                )
            })?;

            for run_idx in 2..=SURFACE_DETERMINISM_RUNS {
                let candidate = unsafe_review_core::render_fixture_surface_from_workspace(
                    &workspace_root,
                    fixture,
                    surface,
                )
                .map_err(|err| {
                    format!(
                        "check-surface-determinism: fixture `{fixture}` surface `{surface}`: \
                             render {run_idx} failed: {err}"
                    )
                })?;
                if baseline != candidate {
                    let first_diff = first_differing_line(&baseline, &candidate);
                    mismatches.push(format!(
                        "  fixture `{fixture}` surface `{surface}` render {run_idx}: {first_diff}"
                    ));
                    break;
                }
            }
            checked += 1;
        }
    }

    if !mismatches.is_empty() {
        return Err(format!(
            "check-surface-determinism: {} surface render(s) drifted across repeated generation:\n{}",
            mismatches.len(),
            mismatches.join("\n")
        ));
    }

    println!(
        "check-surface-determinism: ok ({checked} surface render(s), {SURFACE_DETERMINISM_RUNS} passes each)"
    );
    Ok(())
}

/// Return the committed golden filename for a surface name.
fn surface_golden_filename(surface: &str) -> &'static str {
    match surface {
        "lsp" => "expected.lsp.json",
        "repair-queue" => "expected.repair-queue.json",
        "comment-plan" => "expected.comment-plan.json",
        _ => "expected.unknown.json",
    }
}

/// Describe the first line where two multi-line strings differ.
fn first_differing_line(committed: &str, rendered: &str) -> String {
    let committed_lines: Vec<&str> = committed.lines().collect();
    let rendered_lines: Vec<&str> = rendered.lines().collect();
    let len = committed_lines.len().max(rendered_lines.len());
    for i in 0..len {
        let a = committed_lines.get(i).copied().unwrap_or("<missing>");
        let b = rendered_lines.get(i).copied().unwrap_or("<missing>");
        if a != b {
            return format!(
                "first diff at line {}: committed={a:?} rendered={b:?}",
                i + 1
            );
        }
    }
    "content differs but all lines appear equal (trailing newline difference?)".to_string()
}

pub(crate) fn check_fixture_exception_ledgers(dirs: &[PathBuf]) -> Result<(), String> {
    let mut fixture_paths = BTreeMap::new();
    for dir in dirs {
        let name = fixture_dir_name(dir)?.to_string();
        fixture_paths.insert(name, dir);
    }

    for fixture in FIXTURE_EXPECTED_CARDS_EXCEPTIONS {
        let Some(dir) = fixture_paths.get(*fixture) else {
            return Err(format!(
                "expected-card exception fixture `{fixture}` does not exist"
            ));
        };
        if dir.join("expected.cards.json").is_file() {
            return Err(format!(
                "expected-card exception fixture `{fixture}` has expected.cards.json"
            ));
        }
    }

    for (fixture, _prefix) in FIXTURE_PACKAGE_PREFIX_EXCEPTIONS {
        if !fixture_paths.contains_key(*fixture) {
            return Err(format!(
                "package-prefix exception fixture `{fixture}` does not exist"
            ));
        }
    }

    Ok(())
}
