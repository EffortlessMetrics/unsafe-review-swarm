use serde_json::Value;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn cargo_subcommand_alias_runs_check_json() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");
    let output = checked_output(
        Command::new(env!("CARGO_BIN_EXE_cargo-unsafe-review"))
            .arg("unsafe-review")
            .arg("check")
            .arg("--root")
            .arg(&fixture)
            .arg("--diff")
            .arg(fixture.join("change.diff"))
            .arg("--format")
            .arg("json"),
    )?;
    let stdout = String::from_utf8(output.stdout)?;
    let value: Value = serde_json::from_str(&stdout)?;

    assert_eq!(value["schema_version"], "0.2");
    assert_eq!(value["tool"], "unsafe-review");
    assert_eq!(value["scope"], "diff");
    assert_eq!(value["summary"]["cards"], 1);
    assert_eq!(value["cards"][0]["class"], "guard_missing");
    assert_eq!(value["cards"][0]["operation_family"], "raw_pointer_read");

    Ok(())
}

#[test]
fn cargo_subcommand_alias_writes_pr_summary_artifact() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");
    let temp = TempDir::new("unsafe-review-cargo-alias-e2e")?;
    let summary_path = temp.path().join("nested").join("pr-summary.md");

    let output = checked_output(
        Command::new(env!("CARGO_BIN_EXE_cargo-unsafe-review"))
            .arg("unsafe-review")
            .arg("check")
            .arg("--root")
            .arg(&fixture)
            .arg("--diff")
            .arg(fixture.join("change.diff"))
            .arg("--format")
            .arg("pr-summary")
            .arg("--out")
            .arg(&summary_path),
    )?;

    assert_eq!(String::from_utf8(output.stdout)?.trim(), "");
    let summary = fs::read_to_string(summary_path)?;
    assert!(summary.contains("# unsafe-review PR summary"));
    assert!(summary.contains("## Card table"));
    assert!(summary.contains("`guard_missing`"));
    assert!(summary.contains("`raw_pointer_read`"));
    assert!(summary.contains("## Trust boundary"));

    Ok(())
}

#[test]
fn first_pr_stdout_points_to_top_card_handoff() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");
    let temp = TempDir::new("unsafe-review-first-pr-stdout-e2e")?;
    let out_dir = temp.path().join("review-kit");

    let output = checked_output(
        Command::new(env!("CARGO_BIN_EXE_cargo-unsafe-review"))
            .arg("unsafe-review")
            .arg("first-pr")
            .arg("--root")
            .arg(&fixture)
            .arg("--diff")
            .arg(fixture.join("change.diff"))
            .arg("--out-dir")
            .arg(&out_dir),
    )?;
    let stdout = String::from_utf8(output.stdout)?;

    assert!(
        stdout.starts_with("unsafe-review first-pr\n"),
        "first-pr stdout must start with the invoked command label: {stdout}"
    );
    assert_contains(&stdout, "unsafe-review wrote an advisory PR bundle.");
    // Artifact paths in console output are normalised to forward slashes on all
    // platforms; compare against the normalised form.
    assert_contains(
        &stdout,
        &format!("- Artifact directory: {}", path_display_fwd(&out_dir)),
    );
    assert_contains(&stdout, "Open:");
    assert_contains(&stdout, &path_display_fwd(&out_dir.join("pr-summary.md")));
    assert_contains(&stdout, "Agent repair queue:");
    assert_contains(
        &stdout,
        &format!(
            "{} (copy-only; unsafe-review did not run an agent)",
            path_display_fwd(&out_dir.join("repair-queue.json"))
        ),
    );
    assert_contains(&stdout, "Top card:");
    assert_contains(&stdout, "src/lib.rs:8 `raw_pointer_read`");
    assert_contains(&stdout, "Class: `guard_missing`");
    assert_contains(&stdout, "Missing: guard, witness");
    assert_contains(&stdout, "Explain top card:");
    assert_contains(
        &stdout,
        &format!("unsafe-review explain --root {}", fixture.display()),
    );
    assert_contains(&stdout, "Agent packet:");
    assert_contains(
        &stdout,
        &format!("unsafe-review context --root {}", fixture.display()),
    );
    assert_contains(&stdout, "--json");
    assert_contains(&stdout, "Audit saved receipts:");
    assert_contains(
        &stdout,
        "saved receipt metadata only; unsafe-review did not run a witness",
    );
    assert_contains(&stdout, "Brownfield baseline (optional):");
    assert_contains(
        &stdout,
        "run only from a clean base/default branch before feature changes",
    );
    assert_contains(&stdout, "do not run it from the PR branch being reviewed");
    assert_contains(
        &stdout,
        &format!("unsafe-review baseline init --root {}", fixture.display()),
    );
    assert_contains(
        &stdout,
        "records current open actionable gaps as pre-existing debt",
    );
    assert_contains(
        &stdout,
        "not a safety record, not UB-free status, and not a witness result",
    );
    assert_contains(&stdout, "Manual candidates:");
    assert_contains(
        &stdout,
        &format!(
            "{} (0; manual/advisory sidecar, not analyzer ReviewCards)",
            path_display_fwd(&out_dir.join("manual-candidates.json"))
        ),
    );
    assert!(
        !stdout.contains("Manual candidate queue preview:"),
        "zero manual candidates should not print a queue preview:\n{stdout}"
    );
    assert_order(&stdout, "Top card:", "Audit saved receipts:");
    assert_order(&stdout, "Audit saved receipts:", "Policy report:");
    assert_order(&stdout, "Policy report:", "Brownfield baseline (optional):");
    assert_order(
        &stdout,
        "Brownfield baseline (optional):",
        "Manual candidates:",
    );
    assert_order(&stdout, "Manual candidates:", "Artifacts:");
    assert_contains(&stdout, "Artifacts:");
    assert_contains(&stdout, &path_display_fwd(&out_dir.join("review-kit.json")));
    assert_contains(
        &stdout,
        &path_display_fwd(&out_dir.join("github-summary.md")),
    );
    assert_contains(
        &stdout,
        &path_display_fwd(&out_dir.join("comment-plan.json")),
    );
    assert_contains(
        &stdout,
        &path_display_fwd(&out_dir.join("receipt-audit.md")),
    );
    assert_contains(&stdout, "Trust boundary:");
    assert_contains(&stdout, "static unsafe contract review only");
    assert_contains(&stdout, "not memory-safety proof");
    assert_contains(
        &stdout,
        "unsafe-review did not run witnesses, post comments, edit source, or enforce blocking policy.",
    );

    Ok(())
}

#[test]
fn pr_alias_with_explicit_flags_produces_same_bundle_as_first_pr() -> Result<(), Box<dyn Error>> {
    // `pr` with explicit --root/--diff behaves identically to `first-pr` with
    // those flags (no auto-detection path because explicit flags are supplied).
    let fixture = fixture_root("raw_pointer_alignment");
    let temp = TempDir::new("unsafe-review-pr-alias-e2e")?;
    let out_dir = temp.path().join("pr-alias-review-kit");

    let output = checked_output(
        Command::new(env!("CARGO_BIN_EXE_cargo-unsafe-review"))
            .arg("unsafe-review")
            .arg("pr")
            .arg("--root")
            .arg(&fixture)
            .arg("--diff")
            .arg(fixture.join("change.diff"))
            .arg("--out-dir")
            .arg(&out_dir),
    )?;
    let stdout = String::from_utf8(output.stdout)?;

    // `pr` produces the same advisory bundle while presenting the command the
    // user typed in the terminal handoff.
    assert!(
        stdout.starts_with("unsafe-review pr\n"),
        "pr stdout must start with the invoked command label: {stdout}"
    );
    assert_contains(&stdout, "unsafe-review wrote an advisory PR bundle.");
    assert_contains(&stdout, "Top card:");
    assert_contains(&stdout, "Class: `guard_missing`");
    // The advisory bundle files must be on disk.
    assert!(
        out_dir.join("pr-summary.md").exists(),
        "pr-summary.md must be written by `pr` alias"
    );
    assert!(
        out_dir.join("cards.json").exists(),
        "cards.json must be written by `pr` alias"
    );
    assert!(
        out_dir.join("review-kit.json").exists(),
        "review-kit.json must be written by `pr` alias"
    );

    Ok(())
}

#[test]
fn pr_alias_accepts_exact_base_and_head_sha_inputs() -> Result<(), Box<dyn Error>> {
    let repo = exact_pr_fixture_repo("unsafe-review-exact-pr-e2e")?;
    let out_dir = repo.temp.path().join("review-kit");

    let output = checked_output(
        Command::new(env!("CARGO_BIN_EXE_cargo-unsafe-review"))
            .arg("unsafe-review")
            .arg("pr")
            .arg("--root")
            .arg(&repo.root)
            .arg("--base-sha")
            .arg(&repo.base_sha)
            .arg("--head-sha")
            .arg(&repo.head_sha)
            .arg("--out-dir")
            .arg(&out_dir),
    )?;
    let stdout = String::from_utf8(output.stdout)?;

    assert!(
        stdout.starts_with("unsafe-review pr\n"),
        "exact SHA PR path must preserve the pr entrypoint label: {stdout}"
    );
    let cards_text = fs::read_to_string(out_dir.join("cards.json"))?;
    let cards: Value = serde_json::from_str(&cards_text)?;
    assert_eq!(cards["scope"], "diff");
    assert_eq!(cards["provenance"]["base_sha"], repo.base_sha);
    assert_eq!(cards["provenance"]["head_sha"], repo.head_sha);
    let families = cards["cards"]
        .as_array()
        .ok_or_else(|| "cards.json cards field must be an array".to_string())?
        .iter()
        .filter_map(|card| card["operation_family"].as_str())
        .collect::<Vec<_>>();
    assert!(
        families.contains(&"raw_pointer_deref"),
        "exact SHA path must preserve normal analyzer output; families={families:?}"
    );

    Ok(())
}

#[test]
fn pr_alias_rejects_exact_head_sha_mismatch() -> Result<(), Box<dyn Error>> {
    let repo = exact_pr_fixture_repo("unsafe-review-exact-pr-mismatch-e2e")?;
    let out_dir = repo.temp.path().join("review-kit");
    let stale_head_sha = "1111111111111111111111111111111111111111";

    let output = Command::new(env!("CARGO_BIN_EXE_cargo-unsafe-review"))
        .arg("unsafe-review")
        .arg("pr")
        .arg("--root")
        .arg(&repo.root)
        .arg("--base-sha")
        .arg(&repo.base_sha)
        .arg("--head-sha")
        .arg(stale_head_sha)
        .arg("--out-dir")
        .arg(&out_dir)
        .output()?;

    assert!(
        !output.status.success(),
        "stale exact-head input must fail before analysis"
    );
    assert_eq!(
        output.status.code(),
        Some(2),
        "exact-head mismatch must be a tool/input error"
    );
    let stderr = String::from_utf8(output.stderr)?;
    assert!(
        stderr.contains("--head-sha expected"),
        "stderr must name the expected head SHA mismatch: {stderr}"
    );
    assert!(
        stderr.contains("before running pr"),
        "stderr must name the command the user ran: {stderr}"
    );
    assert!(
        stderr.contains("git -C"),
        "stderr must include a copyable git command: {stderr}"
    );
    assert!(
        stderr.contains("fetch origin"),
        "stderr must show the exact fetch step: {stderr}"
    );
    assert!(
        stderr.contains("baseRefName,baseRefOid,headRefOid"),
        "stderr must tell users to capture the base branch name plus exact SHAs: {stderr}"
    );
    assert!(
        stderr.contains("pull/<number>/head") && stderr.contains("<base-ref-name>"),
        "stderr must use the public PR pull ref and base branch for checkout recovery: {stderr}"
    );
    assert!(
        stderr.contains(&repo.base_sha) && stderr.contains(stale_head_sha),
        "stderr must keep exact base/head SHAs visible: {stderr}"
    );
    assert!(
        stderr.contains("checkout --detach"),
        "stderr must show the detach checkout step: {stderr}"
    );
    assert!(
        !out_dir.join("cards.json").exists(),
        "stale exact-head input must not write PR artifacts"
    );

    Ok(())
}

#[test]
fn pr_alias_rejects_dirty_worktree_with_exact_head_sha() -> Result<(), Box<dyn Error>> {
    let repo = exact_pr_fixture_repo("unsafe-review-exact-pr-dirty-e2e")?;
    let out_dir = repo.temp.path().join("review-kit");
    fs::write(
        repo.root.join("src/lib.rs"),
        "pub unsafe fn read_byte(ptr: *const u8) -> u8 {\n    unsafe { *ptr.add(1) }\n}\n",
    )?;

    let output = Command::new(env!("CARGO_BIN_EXE_cargo-unsafe-review"))
        .arg("unsafe-review")
        .arg("pr")
        .arg("--root")
        .arg(&repo.root)
        .arg("--base-sha")
        .arg(&repo.base_sha)
        .arg("--head-sha")
        .arg(&repo.head_sha)
        .arg("--out-dir")
        .arg(&out_dir)
        .output()?;

    assert!(
        !output.status.success(),
        "dirty exact-head worktree must fail before analysis"
    );
    assert_eq!(
        output.status.code(),
        Some(2),
        "dirty exact-head worktree must be a tool/input error"
    );
    let stderr = String::from_utf8(output.stderr)?;
    assert!(
        stderr.contains("dirty worktree"),
        "stderr must explain that exact-head mode requires a clean worktree: {stderr}"
    );
    assert!(
        !out_dir.join("cards.json").exists(),
        "dirty exact-head input must not write PR artifacts"
    );

    Ok(())
}

#[test]
fn pr_alias_auto_detect_unresolved_base_prints_actionable_error() -> Result<(), Box<dyn Error>> {
    // When `pr` is run without explicit flags from a directory that is not
    // inside a git repository, the error must name the exact command to run
    // and must exit with a tool-error code (2), not a policy-violation code (1).
    let temp = TempDir::new("unsafe-review-pr-no-git")?;

    let output = Command::new(env!("CARGO_BIN_EXE_cargo-unsafe-review"))
        .arg("unsafe-review")
        .arg("pr")
        .current_dir(temp.path())
        .output()?;

    assert!(
        !output.status.success(),
        "pr in a non-git directory must exit non-zero"
    );
    assert_eq!(
        output.status.code(),
        Some(2),
        "pr detection failure must use exit code 2 (tool error), not 1 (policy)"
    );
    let stderr = String::from_utf8(output.stderr)?;
    let combined = format!("{stderr}{}", String::from_utf8(output.stdout)?);
    assert!(
        combined.contains("--base") || combined.contains("--root"),
        "error must name the explicit flag to use: {combined}"
    );
    assert!(
        combined.contains("unsafe-review first-pr") || combined.contains("unsafe-review pr"),
        "error must name the command to run: {combined}"
    );

    Ok(())
}

#[test]
fn help_output_mentions_pr_alias() -> Result<(), Box<dyn Error>> {
    // The top-level help must include a one-line hint about `unsafe-review pr`.
    let output = checked_output(
        Command::new(env!("CARGO_BIN_EXE_cargo-unsafe-review"))
            .arg("unsafe-review")
            .arg("--help"),
    )?;
    let stdout = String::from_utf8(output.stdout)?;

    assert!(
        stdout.contains("  pr      first-run PR review bundle"),
        "help must mention the `pr` first-run entry point: {stdout}"
    );
    assert!(
        stdout.contains("auto-detects root and base ref"),
        "help must say pr auto-detects first-run inputs: {stdout}"
    );
    assert!(
        stdout
            .contains("pr-setup print read-only external GitHub PR checkout and raw-diff commands"),
        "help must mention the read-only external PR setup helper: {stdout}"
    );

    Ok(())
}

#[test]
fn first_pr_help_lists_current_bundle_artifacts() -> Result<(), Box<dyn Error>> {
    let output = checked_output(
        Command::new(env!("CARGO_BIN_EXE_cargo-unsafe-review"))
            .arg("unsafe-review")
            .arg("first-pr")
            .arg("--help"),
    )?;
    let stdout = String::from_utf8(output.stdout)?;

    for artifact in [
        "review-kit.json",
        "unsafe-review-gate.json",
        "cards.json",
        "pr-summary.md",
        "github-summary.md",
        "cards.sarif",
        "comment-plan.json",
        "witness-plan.md",
        "receipt-audit.md",
        "receipt-audit.json",
        "policy-report.json",
        "policy-report.md",
        "manual-candidates.json",
        "manual-repair-queue.json",
        "tokmd-packets.json",
        "usefulness-telemetry.json",
        "lsp.json",
        "repair-queue.json",
    ] {
        assert!(
            stdout.contains(artifact),
            "first-pr help must list bundle artifact `{artifact}`\nstdout:\n{stdout}"
        );
    }

    Ok(())
}

#[test]
fn first_pr_help_shows_exact_external_pr_setup_cue() -> Result<(), Box<dyn Error>> {
    let output = checked_output(
        Command::new(env!("CARGO_BIN_EXE_cargo-unsafe-review"))
            .arg("unsafe-review")
            .arg("first-pr")
            .arg("--help"),
    )?;
    let stdout = String::from_utf8(output.stdout)?;

    for expected in [
        "External PR setup:",
        "gh pr view <number> --repo <owner>/<repo> --json baseRefName,baseRefOid,headRefOid",
        "unsafe-review pr-setup --repo <owner>/<repo> --number <number> --base-ref <base-ref-name> --base-sha <base-sha> --head-sha <head-sha> --root /path/to/repo --diff-out /path/to/change.diff",
        "git -C /path/to/repo fetch origin <base-ref-name> pull/<number>/head",
        "git -C /path/to/repo checkout --detach <head-sha>",
        "unsafe-review pr --root /path/to/repo --base-sha <base-sha> --head-sha <head-sha>",
        "Raw diff capture for receipts or --diff:",
        "git -C /path/to/repo diff --binary --full-index --output=/path/to/change.diff <base-sha>...<head-sha>",
        "unsafe-review pr --root /path/to/repo --diff /path/to/change.diff",
    ] {
        assert!(
            stdout.contains(expected),
            "first-pr help must include exact external PR setup cue `{expected}`\nstdout:\n{stdout}"
        );
    }

    Ok(())
}

#[test]
fn pr_setup_prints_read_only_external_pr_commands() -> Result<(), Box<dyn Error>> {
    let output = checked_output(
        Command::new(env!("CARGO_BIN_EXE_cargo-unsafe-review"))
            .arg("unsafe-review")
            .arg("pr-setup")
            .arg("--repo")
            .arg("tokio-rs/bytes")
            .arg("--number")
            .arg("827")
            .arg("--base-ref")
            .arg("main")
            .arg("--base-sha")
            .arg("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .arg("--head-sha")
            .arg("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
            .arg("--root")
            .arg("/tmp/bytes checkout")
            .arg("--diff-out")
            .arg("target/external-pilots/bytes-pr827.diff"),
    )?;
    let stdout = String::from_utf8(output.stdout)?;

    for expected in [
        "unsafe-review pr-setup",
        "Read-only setup commands for external GitHub PR tokio-rs/bytes#827.",
        "This command did not fetch, checkout, run unsafe-review, execute witnesses, post comments, or edit source.",
        "gh pr view 827 --repo tokio-rs/bytes --json baseRefName,baseRefOid,headRefOid",
        "git -C \"/tmp/bytes checkout\" fetch origin main pull/827/head",
        "git -C \"/tmp/bytes checkout\" checkout --detach bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "unsafe-review pr --root \"/tmp/bytes checkout\" --base-sha aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa --head-sha bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "git -C \"/tmp/bytes checkout\" diff --binary --full-index --output=\"target/external-pilots/bytes-pr827.diff\" aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa...bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "unsafe-review pr --root \"/tmp/bytes checkout\" --diff \"target/external-pilots/bytes-pr827.diff\"",
        "baseRefName: main",
        "baseRefOid: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "headRefOid: bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "Trust boundary: always advisory;",
    ] {
        assert_contains(&stdout, expected);
    }

    Ok(())
}

#[test]
fn pr_setup_rejects_injected_repo_token() -> Result<(), Box<dyn Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-unsafe-review"))
        .arg("unsafe-review")
        .arg("pr-setup")
        .arg("--repo")
        .arg("tokio-rs/bytes;rm")
        .arg("--number")
        .arg("827")
        .arg("--base-ref")
        .arg("main")
        .arg("--base-sha")
        .arg("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        .arg("--head-sha")
        .arg("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
        .output()?;

    assert!(
        !output.status.success(),
        "malformed repo token must be rejected"
    );
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr)?;
    assert_contains(&stderr, "invalid --repo");
    let stdout = String::from_utf8(output.stdout)?;
    assert!(
        !stdout.contains("git -C"),
        "rejected input must not print a command plan: {stdout}"
    );

    Ok(())
}

#[test]
fn pr_setup_rejects_injected_base_ref() -> Result<(), Box<dyn Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-unsafe-review"))
        .arg("unsafe-review")
        .arg("pr-setup")
        .arg("--repo")
        .arg("tokio-rs/bytes")
        .arg("--number")
        .arg("827")
        .arg("--base-ref")
        .arg("main;echo")
        .arg("--base-sha")
        .arg("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        .arg("--head-sha")
        .arg("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
        .output()?;

    assert!(
        !output.status.success(),
        "malformed base ref must be rejected"
    );
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr)?;
    assert_contains(&stderr, "invalid --base-ref");
    let stdout = String::from_utf8(output.stdout)?;
    assert!(
        !stdout.contains("git -C"),
        "rejected input must not print a command plan: {stdout}"
    );

    Ok(())
}

#[test]
fn pr_setup_rejects_shell_expanding_diff_path() -> Result<(), Box<dyn Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-unsafe-review"))
        .arg("unsafe-review")
        .arg("pr-setup")
        .arg("--repo")
        .arg("tokio-rs/bytes")
        .arg("--number")
        .arg("827")
        .arg("--base-ref")
        .arg("main")
        .arg("--base-sha")
        .arg("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        .arg("--head-sha")
        .arg("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
        .arg("--diff-out")
        .arg("target/$(whoami).diff")
        .output()?;

    assert!(
        !output.status.success(),
        "shell-expanding diff path must be rejected"
    );
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr)?;
    assert_contains(&stderr, "invalid --diff-out");
    let stdout = String::from_utf8(output.stdout)?;
    assert!(
        !stdout.contains("git -C"),
        "rejected input must not print a command plan: {stdout}"
    );

    Ok(())
}

#[test]
fn candidate_help_is_command_specific() -> Result<(), Box<dyn Error>> {
    let output = checked_output(
        Command::new(env!("CARGO_BIN_EXE_cargo-unsafe-review"))
            .arg("unsafe-review")
            .arg("candidate")
            .arg("--help"),
    )?;
    let stdout = String::from_utf8(output.stdout)?;

    assert!(
        stdout.contains("unsafe-review candidate: import and project manual advisory candidates")
    );
    assert!(stdout.contains("unsafe-review candidate new --class <stable-byte-class>"));
    assert!(stdout.contains("unsafe-review candidate import <manual-candidate.json>"));
    assert!(stdout.contains("unsafe-review candidate lint <manual-candidate.json>"));
    assert!(stdout.contains("unsafe-review candidate list"));
    assert!(stdout.contains("unsafe-review candidate witness-plan"));
    assert!(stdout.contains("stable-byte-source-getter-reentry"));
    assert!(stdout.contains("reports the first schema error plus all TODO markers"));
    assert!(stdout.contains("candidate new and candidate lint are authoring aids only"));
    assert!(stdout.contains("manual_candidate `true`"));
    assert!(stdout.contains("analyzer_discovered `false`"));
    assert!(stdout.contains("not analyzer-discovered findings"));
    assert!(!stdout.contains("Commands:\n  check"));

    Ok(())
}

#[test]
fn subcommand_help_is_command_specific() -> Result<(), Box<dyn Error>> {
    // Table of (subcommand args, expected keyword unique to that subcommand's help).
    let cases: &[(&[&str], &str)] = &[
        (&["check", "--help"], "unsafe-review check:"),
        (&["first-pr", "--help"], "unsafe-review first-pr:"),
        (&["review", "--help"], "unsafe-review first-pr:"),
        (&["pr", "--help"], "unsafe-review first-pr:"),
        (&["pilot", "--help"], "unsafe-review pilot:"),
        (&["explain", "--help"], "unsafe-review explain:"),
        (&["context", "--help"], "unsafe-review context:"),
        (&["confirm", "--help"], "unsafe-review confirm:"),
        (&["receipt", "--help"], "unsafe-review receipt:"),
        (&["receipt", "audit", "-h"], "unsafe-review receipt:"),
        (&["outcome", "--help"], "unsafe-review outcome:"),
        (&["policy", "--help"], "unsafe-review policy:"),
        (&["pr-setup", "--help"], "unsafe-review pr-setup:"),
        (&["doctor", "--help"], "unsafe-review doctor:"),
        (&["badges", "--help"], "unsafe-review badges:"),
        (&["lsp", "--help"], "unsafe-review lsp:"),
    ];

    for (subargs, expected) in cases {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_cargo-unsafe-review"));
        cmd.arg("unsafe-review");
        for arg in *subargs {
            cmd.arg(arg);
        }
        let output = checked_output(&mut cmd)?;
        let stdout = String::from_utf8(output.stdout)?;

        assert!(
            stdout.contains(expected),
            "subcommand {:?}: expected stdout to contain `{expected}`\nstdout:\n{stdout}",
            subargs
        );
        // Must NOT fall back to the top-level command list header.
        assert!(
            !stdout.contains("Commands:\n  check"),
            "subcommand {:?}: fell back to top-level help\nstdout:\n{stdout}",
            subargs
        );
        // Each help must contain "Usage:".
        assert!(
            stdout.contains("Usage:"),
            "subcommand {:?}: missing 'Usage:'\nstdout:\n{stdout}",
            subargs
        );
    }

    Ok(())
}

#[test]
fn cargo_bin_policy_violation_exits_1_not_2() -> Result<(), Box<dyn Error>> {
    // Exit-code contract: cargo-unsafe-review must exit 1 for policy violations
    // (no-new-debt) and exit 2 only for tool errors. Before the fix, the binary
    // mapped every RunFailure to exit 2, making policy failures indistinguishable
    // from crashes in CI scripts.
    let fixture = fixture_root("raw_pointer_alignment");

    let output = Command::new(env!("CARGO_BIN_EXE_cargo-unsafe-review"))
        .arg("unsafe-review")
        .arg("check")
        .arg("--root")
        .arg(&fixture)
        .arg("--diff")
        .arg(fixture.join("change.diff"))
        .arg("--format")
        .arg("json")
        .arg("--policy")
        .arg("no-new-debt")
        .output()?;

    assert!(
        !output.status.success(),
        "no-new-debt violation must exit non-zero"
    );
    assert_eq!(
        output.status.code(),
        Some(1),
        "no-new-debt violation must exit 1 (policy), not 2 (tool error)"
    );
    let stderr = String::from_utf8(output.stderr)?;
    assert!(
        stderr.contains("policy:"),
        "stderr must carry the 'policy:' category prefix: {stderr}"
    );

    Ok(())
}

/// Bug A regression: a capped repo scan must report stop_reason=max_cards and exit 0
/// even when `--timeout-seconds` is supplied.  Before the fix, the timed_out()
/// guard could fire on the terminal capped event (stop_reason=MaxCards) if the
/// timeout clock elapsed by the time that event arrived, causing the scan to be
/// mislabelled as stop_reason=timeout and exit 2.
#[test]
fn repo_capped_scan_reports_max_cards_not_timeout() -> Result<(), Box<dyn Error>> {
    // Build a temp fixture with two unsafe files so --max-cards=1 stops early.
    let temp = TempDir::new("unsafe-review-cli-capped-timeout-e2e")?;
    let scan_root = temp.path().join("fixture");
    fs::create_dir_all(scan_root.join("src"))?;
    fs::write(
        scan_root.join("Cargo.toml"),
        "[package]\nname = \"capped-timeout-fixture\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
    )?;
    fs::write(
        scan_root.join("src/lib.rs"),
        "pub unsafe fn alpha(ptr: *const u8) -> u8 { unsafe { *ptr } }\n",
    )?;
    fs::write(
        scan_root.join("src/beta.rs"),
        "pub unsafe fn beta(ptr: *const u8) -> u8 { unsafe { *ptr } }\n",
    )?;

    let report_path = temp.path().join("repo.json");
    let status_path = temp.path().join("repo.json.status.json");

    // A capped scan must exit 0 (not 2), even with --timeout-seconds supplied.
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-unsafe-review"))
        .arg("unsafe-review")
        .arg("repo")
        .arg("--root")
        .arg(&scan_root)
        .arg("--format")
        .arg("json")
        .arg("--out")
        .arg(&report_path)
        .arg("--max-cards")
        .arg("1")
        .arg("--timeout-seconds")
        .arg("300")
        .output()?;

    assert!(
        output.status.success(),
        "capped scan must exit 0 (not tool-error 2): status={:?}\nstderr:\n{}\nstdout:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "capped scan must exit 0, not 1 or 2"
    );

    // Status sidecar must carry stop_reason=max_cards, not timeout.
    assert!(
        status_path.exists(),
        "capped scan must write a status sidecar"
    );
    let status_json = fs::read_to_string(&status_path)?;
    let status: Value = serde_json::from_str(&status_json)?;
    assert_eq!(
        status["stop_reason"], "max_cards",
        "capped scan stop_reason must be max_cards, not timeout or error: {}",
        status_json
    );
    assert_eq!(
        status["phase"], "complete",
        "capped scan phase must be complete: {status_json}"
    );
    assert_eq!(
        status["operator"]["state"], "capped",
        "capped scan operator state must be capped: {status_json}"
    );
    assert_eq!(
        status["operator"]["downstream_consumable"], true,
        "capped scan must be downstream-consumable: {status_json}"
    );

    Ok(())
}

/// Bug B regression: the capped arm of the repo-scan operator guidance must
/// describe card-level truncation (all files scanned, card list capped), not
/// file-level truncation (which only applies to genuinely file-truncated paths).
/// Under `--max-cards`, all files ARE scanned — only the card list is trimmed.
#[test]
fn repo_capped_scan_operator_json_uses_card_level_wording() -> Result<(), Box<dyn Error>> {
    // Build a temp fixture with two unsafe files; cap at 1 card.
    let temp = TempDir::new("unsafe-review-cli-capped-wording-e2e")?;
    let scan_root = temp.path().join("fixture");
    fs::create_dir_all(scan_root.join("src"))?;
    fs::write(
        scan_root.join("Cargo.toml"),
        "[package]\nname = \"capped-wording-fixture\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
    )?;
    fs::write(
        scan_root.join("src/lib.rs"),
        "pub unsafe fn alpha(ptr: *const u8) -> u8 { unsafe { *ptr } }\n",
    )?;
    fs::write(
        scan_root.join("src/beta.rs"),
        "pub unsafe fn beta(ptr: *const u8) -> u8 { unsafe { *ptr } }\n",
    )?;

    let report_path = temp.path().join("repo.json");
    let status_path = temp.path().join("repo.json.status.json");

    let output = Command::new(env!("CARGO_BIN_EXE_cargo-unsafe-review"))
        .arg("unsafe-review")
        .arg("repo")
        .arg("--root")
        .arg(&scan_root)
        .arg("--format")
        .arg("json")
        .arg("--out")
        .arg(&report_path)
        .arg("--max-cards")
        .arg("1")
        .output()?;

    assert!(
        output.status.success(),
        "capped scan must exit 0: status={:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr),
    );

    assert!(
        status_path.exists(),
        "capped scan must write a status sidecar"
    );
    let status_json = fs::read_to_string(&status_path)?;
    let status: Value = serde_json::from_str(&status_json)?;

    let limitation = status["operator"]["partial_report_limitation"]
        .as_str()
        .unwrap_or("");
    // Card-level wording: all files were scanned; the cap applies to the card list.
    assert!(
        limitation.contains("All files scanned"),
        "capped operator limitation must say all files were scanned (card-level, not file-level): {limitation}"
    );
    assert!(
        limitation.contains("card list truncated") || limitation.contains("--max-cards"),
        "capped operator limitation must describe card list truncation: {limitation}"
    );
    assert!(
        limitation.contains("cap=1"),
        "capped operator limitation must embed the configured cap value: {limitation}"
    );
    // Must NOT use the old file-level snapshot wording.
    assert!(
        !limitation.contains("Completed-file snapshot only"),
        "capped operator limitation must not use file-level snapshot wording: {limitation}"
    );

    Ok(())
}

fn assert_contains(haystack: &str, needle: &str) {
    assert!(
        haystack.contains(needle),
        "expected stdout to contain `{needle}`\nstdout:\n{haystack}"
    );
}

fn assert_order(haystack: &str, before: &str, after: &str) {
    let before_idx = haystack.find(before);
    let after_idx = haystack.find(after);
    assert!(
        matches!((before_idx, after_idx), (Some(left), Some(right)) if left < right),
        "expected `{before}` to appear before `{after}`\nstdout:\n{haystack}"
    );
}

/// Return a forward-slash-normalised display string for a path, matching the
/// normalisation applied by `artifact_path_display` in the console output.
fn path_display_fwd(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn checked_output(command: &mut Command) -> Result<Output, Box<dyn Error>> {
    let output = command.output()?;
    if output.status.success() {
        return Ok(output);
    }
    Err(format!(
        "command failed with status {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
    .into())
}

fn fixture_root(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
}

struct ExactPrFixtureRepo {
    temp: TempDir,
    root: PathBuf,
    base_sha: String,
    head_sha: String,
}

fn exact_pr_fixture_repo(prefix: &str) -> Result<ExactPrFixtureRepo, Box<dyn Error>> {
    let temp = TempDir::new(prefix)?;
    let root = temp.path().join("repo");
    fs::create_dir_all(root.join("src"))?;
    run_git(&root, &["init"])?;
    run_git(
        &root,
        &["config", "user.email", "unsafe-review@example.test"],
    )?;
    run_git(&root, &["config", "user.name", "unsafe-review test"])?;
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"exact-pr-fixture\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
    )?;
    fs::write(root.join("src/lib.rs"), "pub fn read_byte() -> u8 { 0 }\n")?;
    run_git(&root, &["add", "."])?;
    run_git(&root, &["commit", "-m", "base"])?;
    let base_sha = run_git(&root, &["rev-parse", "HEAD"])?;

    fs::write(
        root.join("src/lib.rs"),
        "pub unsafe fn read_byte(ptr: *const u8) -> u8 {\n    unsafe { *ptr }\n}\n",
    )?;
    run_git(&root, &["add", "."])?;
    run_git(&root, &["commit", "-m", "head"])?;
    let head_sha = run_git(&root, &["rev-parse", "HEAD"])?;

    Ok(ExactPrFixtureRepo {
        temp,
        root,
        base_sha,
        head_sha,
    })
}

fn run_git(repo: &Path, args: &[&str]) -> Result<String, Box<dyn Error>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "git {:?} failed with status {:?}\nstdout:\n{}\nstderr:\n{}",
            args,
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Result<Self, Box<dyn Error>> {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
