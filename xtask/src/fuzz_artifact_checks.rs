//! Manual fuzz harness and tracked-generated-artifact check gates.
//!
//! Extracted from main.rs as part of #1806 (xtask modularization).

use crate::{
    is_forbidden_generated_path, parse_toml_file, read_to_string, repo_path, require_repo_file,
};
use std::fs;
use std::process::Command;

const FUZZ_REQUIRED_FILES: &[&str] = &[
    "docs/FUZZING.md",
    "fuzz/.gitignore",
    "fuzz/Cargo.lock",
    "fuzz/Cargo.toml",
    "fuzz/corpus/analyze/basic",
    "fuzz/fuzz_targets/analyze.rs",
];

pub(crate) fn check_manual_fuzz_harness() -> Result<(), String> {
    for path in FUZZ_REQUIRED_FILES {
        require_repo_file(path)?;
    }

    let workspace = parse_toml_file(&repo_path("Cargo.toml"))?;
    let excludes = workspace
        .get("workspace")
        .and_then(|workspace| workspace.get("exclude"))
        .and_then(toml::Value::as_array)
        .ok_or_else(|| "Cargo.toml workspace.exclude must list fuzz".to_string())?;
    if !excludes
        .iter()
        .any(|entry| entry.as_str().is_some_and(|entry| entry == "fuzz"))
    {
        return Err("Cargo.toml workspace.exclude must list fuzz".to_string());
    }

    let fuzz_manifest = parse_toml_file(&repo_path("fuzz/Cargo.toml"))?;
    if fuzz_manifest
        .get("package")
        .and_then(|package| package.get("publish"))
        .and_then(toml::Value::as_bool)
        .unwrap_or(true)
    {
        return Err("fuzz/Cargo.toml package.publish must be false".to_string());
    }
    let cargo_fuzz = fuzz_manifest
        .get("package")
        .and_then(|package| package.get("metadata"))
        .and_then(|metadata| metadata.get("cargo-fuzz"))
        .and_then(toml::Value::as_bool)
        .unwrap_or(false);
    if !cargo_fuzz {
        return Err("fuzz/Cargo.toml package.metadata.cargo-fuzz must be true".to_string());
    }
    let bins = fuzz_manifest
        .get("bin")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| "fuzz/Cargo.toml must define an analyze fuzz target".to_string())?;
    let has_analyze_target = bins.iter().any(|bin| {
        bin.get("name").and_then(toml::Value::as_str) == Some("analyze")
            && bin.get("path").and_then(toml::Value::as_str) == Some("fuzz_targets/analyze.rs")
    });
    if !has_analyze_target {
        return Err("fuzz/Cargo.toml must define analyze at fuzz_targets/analyze.rs".to_string());
    }

    let fuzz_docs = read_to_string(&repo_path("docs/FUZZING.md"))?;
    for phrase in [
        "manual `cargo-fuzz` harness",
        "not part of the default PR gate",
        "does not prove soundness",
    ] {
        if !fuzz_docs.contains(phrase) {
            return Err(format!("docs/FUZZING.md must include `{phrase}`"));
        }
    }

    let target = read_to_string(&repo_path("fuzz/fuzz_targets/analyze.rs"))?;
    for phrase in [
        "fuzz_target!",
        "DiffSource::Text",
        "render_json",
        "MAX_SOURCE_BYTES",
        "MAX_DIFF_BYTES",
    ] {
        if !target.contains(phrase) {
            return Err(format!(
                "fuzz/fuzz_targets/analyze.rs must include `{phrase}`"
            ));
        }
    }

    let ignore = read_to_string(&repo_path("fuzz/.gitignore"))?;
    for ignored in ["artifacts/", "target/"] {
        if !ignore.lines().any(|line| line.trim() == ignored) {
            return Err(format!("fuzz/.gitignore must ignore `{ignored}`"));
        }
    }
    let corpus_dir = repo_path("fuzz/corpus/analyze");
    let corpus_entries = fs::read_dir(&corpus_dir)
        .map_err(|err| format!("failed to read {}: {err}", corpus_dir.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("failed to enumerate {}: {err}", corpus_dir.display()))?;
    if corpus_entries.is_empty() {
        return Err("fuzz/corpus/analyze must include at least one corpus seed".to_string());
    }
    let mut has_diff_marker_seed = false;
    for entry in corpus_entries {
        let seed_path = entry.path();
        if !seed_path.is_file() {
            continue;
        }
        let seed = fs::read_to_string(&seed_path)
            .map_err(|err| format!("failed to read {}: {err}", seed_path.display()))?;
        if seed.contains("---DIFF---") {
            has_diff_marker_seed = true;
            break;
        }
    }
    if !has_diff_marker_seed {
        return Err(
            "fuzz/corpus/analyze must include at least one seed containing `---DIFF---`"
                .to_string(),
        );
    }

    println!("check-fuzz: ok");
    Ok(())
}

pub(crate) fn check_tracked_generated_artifacts() -> Result<(), String> {
    let output = Command::new("git")
        .args(["ls-files"])
        .output()
        .map_err(|err| format!("failed to run git ls-files: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    for path in String::from_utf8_lossy(&output.stdout).lines() {
        if is_forbidden_generated_path(path) {
            return Err(format!("generated artifact is tracked: {path}"));
        }
    }
    Ok(())
}
