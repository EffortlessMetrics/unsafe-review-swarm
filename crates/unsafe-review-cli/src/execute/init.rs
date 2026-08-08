use crate::command::{Format, InitOptions};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const INIT_SCHEMA_VERSION: &str = "unsafe-review/init/v1";
const WORKFLOW_PATH: &str = ".github/workflows/unsafe-review-first-pr.yml";
const PROPOSAL_FILE: &str = "unsafe-review-init.json";

pub(super) fn run(options: InitOptions) -> Result<(), String> {
    let proposal = build_proposal(&options.root, options.out.as_deref())?;
    let json_text = serde_json::to_string_pretty(&proposal)
        .map_err(|err| format!("failed to render init proposal: {err}"))?;

    if let Some(out_dir) = options.out {
        fs::create_dir_all(&out_dir).map_err(|err| {
            format!(
                "failed to create init proposal directory {}: {err}",
                out_dir.display()
            )
        })?;
        let output = out_dir.join(PROPOSAL_FILE);
        fs::write(&output, format!("{json_text}\n"))
            .map_err(|err| format!("failed to write init proposal {}: {err}", output.display()))?;
        if matches!(&options.format, Format::Json) {
            eprintln!("Proposal written: {}", output.display());
        } else {
            println!("Proposal written: {}", output.display());
        }
    }

    match options.format {
        Format::Json => println!("{json_text}"),
        Format::Human => print_human(&proposal),
        other => {
            return Err(format!(
                "unsafe-review init does not support output format {other:?}"
            ));
        }
    }
    Ok(())
}

fn build_proposal(root: &Path, out_dir: Option<&Path>) -> Result<Value, String> {
    if !root.is_dir() {
        return Err(format!("init root is not a directory: {}", root.display()));
    }

    let git_root = git_output(root, &["rev-parse", "--show-toplevel"]);
    let base_ref = detect_base_ref(root);
    let shallow =
        git_output(root, &["rev-parse", "--is-shallow-repository"]).as_deref() == Some("true");
    let cargo_manifest = root.join("Cargo.toml").is_file();
    let gitignore = root.join(".gitignore").is_file();
    let workflow_path = root.join(WORKFLOW_PATH);
    let workflow = file_proposal(
        root,
        WORKFLOW_PATH,
        &workflow_content(),
        "A minimal read-only pull-request workflow proposal; replace its action reference only after a verified public release exists.",
    );
    let existing_unsafe_workflows = existing_unsafe_workflows(root);
    let root_writable = writable_directory(root);
    let proposal_output_writable = out_dir.map(writable_parent).unwrap_or(true);

    let mut warnings = Vec::new();
    if git_root.is_none() {
        warnings.push(json!({
            "code": "missing_git",
            "severity": "warning",
            "message": "No Git checkout was detected; the proposed PR workflow needs a repository checkout and a resolvable base ref."
        }));
    }
    if shallow {
        warnings.push(json!({
            "code": "shallow_checkout",
            "severity": "warning",
            "message": "This checkout is shallow. The generated workflow is read-only, but local unsafe-review pr may need an explicit base or deeper history."
        }));
    }
    if git_root.is_some() && base_ref.is_none() {
        warnings.push(json!({
            "code": "missing_base",
            "severity": "warning",
            "message": "No origin/HEAD, origin/main, or origin/master base ref is currently resolvable; use an explicit base for local review."
        }));
    }
    if !cargo_manifest {
        warnings.push(json!({
            "code": "unsupported_layout",
            "severity": "warning",
            "message": "Cargo.toml was not found at the proposal root; confirm this is the Rust workspace root before applying the workflow."
        }));
    }
    if !root_writable {
        warnings.push(json!({
            "code": "root_not_writable",
            "severity": "warning",
            "message": "The proposal root appears unwritable; applying the workflow will require a writable checkout."
        }));
    }
    if out_dir.is_some() && !proposal_output_writable {
        warnings.push(json!({
            "code": "proposal_output_not_writable",
            "severity": "warning",
            "message": "The requested proposal output parent appears unwritable; no proposal file was written."
        }));
    }
    if !existing_unsafe_workflows.is_empty() && !workflow_path.is_file() {
        warnings.push(json!({
            "code": "existing_workflow",
            "severity": "warning",
            "message": "An unsafe-review-like workflow already exists under .github/workflows; review it before adding the proposed workflow.",
            "paths": existing_unsafe_workflows,
        }));
    }
    if workflow["status"] == "conflict" {
        warnings.push(json!({
            "code": "workflow_conflict",
            "severity": "warning",
            "message": "The proposed workflow path already contains different content; init never overwrites it.",
            "path": WORKFLOW_PATH,
        }));
    }

    let policy_dir = root.join("policy");
    let ub_review_files = existing_ub_review_files(root);
    let config_files = existing_config_files(root);
    let ignore_recommendation = if gitignore {
        json!({
            "kind": "ignore",
            "status": "already_covered",
            "path": ".gitignore",
            "recommended_entries": ["target/"],
            "reason": "unsafe-review already respects .gitignore by default; no ignore edit is proposed."
        })
    } else {
        json!({
            "kind": "ignore",
            "status": "review_required",
            "path": ".gitignore",
            "recommended_entries": ["target/"],
            "reason": "The repository has no .gitignore. Review whether generated target artifacts should be ignored; init does not create it."
        })
    };

    Ok(json!({
        "schema_version": INIT_SCHEMA_VERSION,
        "tool": "unsafe-review",
        "mode": "preview_only",
        "writes_repository": false,
        "root": root.display().to_string(),
        "repository": {
            "git_root": git_root,
            "cargo_manifest": cargo_manifest,
            "base_ref": base_ref,
            "shallow": shallow,
            "gitignore": gitignore,
            "policy_directory": policy_dir.is_dir(),
            "existing_config_files": config_files,
            "existing_ub_review_files": ub_review_files,
        },
        "proposed_files": [workflow],
        "recommendations": [
            ignore_recommendation,
            {
                "kind": "baseline",
                "status": "explicit_command_required",
                "ledger_path": "policy/unsafe-review-baseline.toml",
                "snapshot_path": "policy/unsafe-review-baseline-snapshot.toml",
                "command": "unsafe-review baseline init --root .",
                "reason": "Baseline creation is separate and must be run from a clean base/default branch after reviewing visible debt; it never labels debt as safe."
            },
            {
                "kind": "badge",
                "status": "optional_snippet",
                "command": "unsafe-review badges --root . --out badges/",
                "snippet": "[![unsafe-review](https://img.shields.io/endpoint?url=https%3A%2F%2Fraw.githubusercontent.com%2FOWNER%2FREPO%2Fmain%2Fbadges%2Funsafe-review.json)](docs/BADGE_POLICY.md)",
                "reason": "Badge output is a numeric projection of review evidence, not a safety or UB-free claim; replace OWNER/REPO only after choosing a checked-in refresh path."
            },
            {
                "kind": "ub_review",
                "status": "optional_pointer_only",
                "artifact": "target/unsafe-review/unsafe-review-gate.json",
                "existing_files": ub_review_files,
                "reason": "Pass the canonical gate-manifest artifact to ub-review if that integration is already adopted; init does not duplicate ub-review policy or make it mandatory."
            },
            {
                "kind": "config",
                "status": "no_file_proposed",
                "existing_files": config_files,
                "reason": "This release has no repository config-file contract for these adoption defaults; init keeps workflow inputs and policy ledgers as the visible authorities."
            }
        ],
        "commands": {
            "doctor": "unsafe-review doctor --root .",
            "first_pr": "unsafe-review pr --root .",
            "first_pr_artifacts": "target/unsafe-review",
            "review_baseline_separately": "unsafe-review baseline init --root ."
        },
        "external_dependencies": [
            {
                "name": "public unsafe-review Action or pinned CLI release",
                "status": "required_before_apply",
                "reason": "The public Action/publish lane is not assumed ready. Reviewers must replace the placeholder with a separately verified release or pinned CLI invocation before applying the workflow."
            }
        ],
        "warnings": warnings,
        "trust_boundary": "static unsafe contract review only; not memory-safety proof, not UB-free status, not Miri-clean status, and not a site-execution claim unless a matching witness receipt says so.",
        "non_claims": [
            "init does not edit source or apply a workflow",
            "init does not create, accept, or label baseline debt as safe",
            "init does not post comments, run witnesses, or enforce blocking policy",
            "init does not prove the generated public Action is currently published"
        ]
    }))
}

fn file_proposal(root: &Path, relative: &str, content: &str, reason: &str) -> Value {
    let path = root.join(relative);
    let exists = path.exists();
    let existing = fs::read_to_string(&path).ok();
    let status = if !exists {
        "create"
    } else if fs::read_to_string(&path)
        .map(|existing| existing == content)
        .unwrap_or(false)
    {
        "unchanged"
    } else {
        "conflict"
    };
    let rollback = if status == "create" {
        format!("Remove {relative} only if this proposal created it.")
    } else {
        format!("Restore the pre-existing {relative} content if an explicit edit is not wanted.")
    };
    let diff = proposal_diff(relative, existing.as_deref(), content);
    json!({
        "path": relative,
        "absolute_path": path.display().to_string(),
        "exists": exists,
        "status": status,
        "operation": if status == "create" { "create" } else { "review_before_update" },
        "reason": reason,
        "diff": diff,
        "content": content,
        "rollback": rollback,
    })
}

fn proposal_diff(relative: &str, existing: Option<&str>, proposed: &str) -> String {
    if existing == Some(proposed) {
        return String::new();
    }
    let old_path = if existing.is_some() {
        format!("a/{relative}")
    } else {
        "/dev/null".to_string()
    };
    let mut diff = format!("--- {old_path}\n+++ b/{relative}\n");
    if let Some(existing) = existing {
        for line in existing.lines() {
            diff.push('-');
            diff.push_str(line);
            diff.push('\n');
        }
    }
    for line in proposed.lines() {
        diff.push('+');
        diff.push_str(line);
        diff.push('\n');
    }
    diff
}

fn workflow_content() -> String {
    r#"# Generated by unsafe-review init as a preview. Review before applying.

name: unsafe-review-first-pr

on:
  pull_request:
    types: [opened, reopened, synchronize, ready_for_review]
  workflow_dispatch:

permissions:
  contents: read

jobs:
  first_pr_bundle:
    name: unsafe-review advisory packet
    runs-on: ubuntu-latest
    timeout-minutes: 30
    steps:
      - uses: actions/checkout@v6
        with:
          fetch-depth: 100
          persist-credentials: false

      - name: Run unsafe-review
        # Preview placeholder: replace with a verified public release or a
        # pinned CLI invocation before applying this workflow.
        # uses: EffortlessMetrics/unsafe-review@<verified-release-ref>
        with:
          out_dir: target/unsafe-review

      - name: Upload unsafe-review advisory bundle
        if: always()
        uses: actions/upload-artifact@v7
        with:
          name: unsafe-review-first-pr
          path: target/unsafe-review
          if-no-files-found: warn

# Advisory only: no comments, witnesses, source edits, or blocking policy.
"#
    .to_string()
}

fn existing_unsafe_workflows(root: &Path) -> Vec<String> {
    let dir = root.join(".github/workflows");
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_string_lossy().to_string();
            let is_workflow = matches!(
                path.extension().and_then(|ext| ext.to_str()),
                Some("yml" | "yaml")
            );
            (is_workflow && name.to_ascii_lowercase().contains("unsafe-review")).then_some(name)
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn existing_ub_review_files(root: &Path) -> Vec<String> {
    [
        "ub-review.toml",
        ".ub-review.toml",
        ".github/workflows/ub-review.yml",
        ".github/workflows/ub-review.yaml",
    ]
    .into_iter()
    .filter(|relative| root.join(relative).is_file())
    .map(str::to_string)
    .collect()
}

fn existing_config_files(root: &Path) -> Vec<String> {
    [
        "unsafe-review.toml",
        ".unsafe-review.toml",
        ".unsafe-review/config.toml",
    ]
    .into_iter()
    .filter(|relative| root.join(relative).is_file())
    .map(str::to_string)
    .collect()
}

fn detect_base_ref(root: &Path) -> Option<String> {
    let symbolic = git_output(root, &["symbolic-ref", "refs/remotes/origin/HEAD"])
        .and_then(|value| value.strip_prefix("refs/remotes/").map(str::to_string));
    symbolic
        .filter(|reference| git_output(root, &["rev-parse", "--verify", reference]).is_some())
        .or_else(|| {
            ["origin/main", "origin/master"]
                .into_iter()
                .find(|reference| git_output(root, &["rev-parse", "--verify", reference]).is_some())
                .map(str::to_string)
        })
}

fn git_output(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn writable_directory(path: &Path) -> bool {
    let Some(existing) = nearest_existing(path) else {
        return false;
    };
    fs::metadata(existing)
        .map(|metadata| !metadata.permissions().readonly())
        .unwrap_or(false)
}

fn writable_parent(path: &Path) -> bool {
    writable_directory(path)
}

fn nearest_existing(path: &Path) -> Option<PathBuf> {
    let mut current = path;
    loop {
        if current.exists() {
            return Some(current.to_path_buf());
        }
        current = current.parent()?;
    }
}

fn print_human(proposal: &Value) {
    println!("unsafe-review init");
    println!("Mode: preview-only; repository changes: none");
    println!("Root: {}", proposal["root"].as_str().unwrap_or("."));
    println!();
    println!("Proposed files:");
    if let Some(files) = proposal["proposed_files"].as_array() {
        for file in files {
            println!(
                "- {} [{}]",
                file["path"].as_str().unwrap_or("unknown"),
                file["status"].as_str().unwrap_or("unknown")
            );
            println!("  reason: {}", file["reason"].as_str().unwrap_or(""));
            println!("  rollback: {}", file["rollback"].as_str().unwrap_or(""));
            println!("  diff:");
            if let Some(diff) = file["diff"].as_str() {
                if diff.is_empty() {
                    println!("    (no change)");
                } else {
                    for line in diff.lines() {
                        println!("    {line}");
                    }
                }
            }
            println!("  proposed content:");
            if let Some(content) = file["content"].as_str() {
                for line in content.lines() {
                    println!("    {line}");
                }
            }
        }
    }
    println!();
    println!("Commands:");
    println!("- Doctor: {}", proposal["commands"]["doctor"]);
    println!("- First PR: {}", proposal["commands"]["first_pr"]);
    println!(
        "- Baseline (separate, explicit): {}",
        proposal["commands"]["review_baseline_separately"]
    );
    println!();
    println!("Recommendations:");
    if let Some(recommendations) = proposal["recommendations"].as_array() {
        for recommendation in recommendations {
            println!(
                "- {} [{}]: {}",
                recommendation["kind"].as_str().unwrap_or("unknown"),
                recommendation["status"].as_str().unwrap_or("unknown"),
                recommendation["reason"].as_str().unwrap_or("")
            );
            if let Some(command) = recommendation["command"].as_str() {
                println!("  command: {command}");
            }
            if let Some(snippet) = recommendation["snippet"].as_str() {
                println!("  snippet: {snippet}");
            }
            if let Some(artifact) = recommendation["artifact"].as_str() {
                println!("  artifact: {artifact}");
            }
        }
    }
    println!();
    println!("External dependencies:");
    if let Some(dependencies) = proposal["external_dependencies"].as_array() {
        for dependency in dependencies {
            println!(
                "- {} [{}]: {}",
                dependency["name"].as_str().unwrap_or("unknown"),
                dependency["status"].as_str().unwrap_or("unknown"),
                dependency["reason"].as_str().unwrap_or("")
            );
        }
    }
    println!();
    println!("Warnings:");
    if let Some(warnings) = proposal["warnings"].as_array() {
        if warnings.is_empty() {
            println!("- none detected");
        } else {
            for warning in warnings {
                println!("- [{}] {}", warning["code"], warning["message"]);
            }
        }
    }
    println!();
    println!("Trust boundary: {}", proposal["trust_boundary"]);
    println!(
        "Review the JSON proposal for complete fields, recommendations, and non-claims before applying anything."
    );
}

#[cfg(test)]
mod tests {
    use super::{WORKFLOW_PATH, build_proposal, workflow_content};
    use std::path::Path;

    #[test]
    fn workflow_is_read_only_and_requires_release_review() {
        let workflow = workflow_content();
        assert!(workflow.contains("permissions:\n  contents: read"));
        assert!(workflow.contains("<verified-release-ref>"));
        assert!(workflow.contains("if: always()"));
        assert!(!workflow.contains("contents: write"));
        assert!(!workflow.contains("cargo install"));
    }

    #[test]
    fn proposal_does_not_write_repository_files() -> Result<(), String> {
        let proposal = build_proposal(Path::new("."), None)?;
        assert_eq!(proposal["mode"], "preview_only");
        assert_eq!(proposal["writes_repository"], false);
        assert_eq!(proposal["proposed_files"][0]["path"], WORKFLOW_PATH);
        Ok(())
    }
}
