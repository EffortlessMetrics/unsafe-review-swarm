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
    assert_contains(&stdout, "Selected reviewer actions (showing 1 of 1):");
    assert_contains(
        &stdout,
        "1. src/lib.rs:8 `raw_pointer_read` — Why: guard_coverage: missing — actionable high-priority card; selected as the top card above",
    );
    let cards: Value = serde_json::from_str(&fs::read_to_string(out_dir.join("cards.json"))?)?;
    let card_id = cards["cards"][0]["id"]
        .as_str()
        .ok_or("missing cards[0].id")?;
    let rendered_root = if cfg!(windows) {
        rendered_shell_path(&fixture)
    } else {
        fixture.display().to_string()
    };
    assert_contains(
        &stdout,
        &format!("Explain top card:\n  unsafe-review explain --root {rendered_root} {card_id}"),
    );
    assert_contains(
        &stdout,
        "Confirmation step: build/run `cargo +nightly miri test read_header` first",
    );
    let selected_actions = stdout
        .lines()
        .skip_while(|line| !line.starts_with("Selected reviewer actions ("))
        .skip(1)
        .take_while(|line| !line.starts_with("Audit saved receipts:"))
        .collect::<Vec<_>>();
    assert_eq!(selected_actions.len(), 1);
    assert!(!selected_actions[0].contains("\n"));
    assert!(!selected_actions[0].contains("; Explain:"));
    assert!(!selected_actions[0].contains("; Verify:"));
    assert_contains(&stdout, "Explain top card:");
    let top_card = stdout
        .lines()
        .skip_while(|line| *line != "Top card:")
        .take_while(|line| !line.starts_with("Selected reviewer actions:"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_contains(&top_card, "Route:");
    assert_contains(&top_card, "Next:");
    assert_contains(&top_card, "Hypothesis:");
    assert_contains(&top_card, "Build/run this first:");
    assert_contains(&top_card, "Minimal repro cue:");
    assert_contains(&top_card, "Confirmation step:");
    assert_contains(&top_card, "Limitation: Minimal repro cue only;");
    assert_not_contains(&top_card, "    - Confirm ReviewCard");
    assert_order(
        &stdout,
        "Top card:",
        "Selected reviewer actions (showing 1 of 1):",
    );
    assert_order(
        &stdout,
        "Selected reviewer actions (showing 1 of 1):",
        "Audit saved receipts:",
    );
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
    assert_contains(&stdout, "Artifacts: 18 files indexed by:");
    assert_contains(&stdout, &path_display_fwd(&out_dir.join("review-kit.json")));
    assert_contains(
        &stdout,
        &path_display_fwd(&out_dir.join("unsafe-review-gate.json")),
    );
    assert_contains(
        &stdout,
        "inspect review-kit.json for the complete bundle inventory",
    );
    assert_not_contains(&stdout, &path_display_fwd(&out_dir.join("cards.json")));
    assert_not_contains(
        &stdout,
        &path_display_fwd(&out_dir.join("github-summary.md")),
    );
    assert_not_contains(
        &stdout,
        &path_display_fwd(&out_dir.join("comment-plan.json")),
    );
    assert_not_contains(
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
fn pr_alias_missing_base_prints_fetch_remediation() -> Result<(), Box<dyn Error>> {
    let repo = exact_pr_fixture_repo("unsafe-review-missing-base-remediation-e2e")?;
    let out_dir = repo.temp.path().join("review-kit");
    let missing_base = "origin/missing-base";

    let output = Command::new(env!("CARGO_BIN_EXE_cargo-unsafe-review"))
        .arg("unsafe-review")
        .arg("pr")
        .arg("--root")
        .arg(&repo.root)
        .arg("--base")
        .arg(missing_base)
        .arg("--out-dir")
        .arg(&out_dir)
        .output()?;

    assert!(
        !output.status.success(),
        "missing base must fail before analysis"
    );
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr)?;
    assert!(stderr.contains("base ref 'origin/missing-base' could not be resolved"));
    assert!(stderr.contains("Recovery:"));
    assert!(stderr.contains("git fetch --no-tags origin"));
    assert!(stderr.contains("git fetch --unshallow origin"));
    assert!(stderr.contains(&format!(
        "unsafe-review pr --root \"{}\" --base {missing_base}",
        repo.root.display()
    )));
    assert!(!out_dir.join("cards.json").exists());

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
        stdout.contains("  pr        first-run PR review bundle"),
        "help must mention the `pr` first-run entry point: {stdout}"
    );
    assert!(
        stdout.contains("auto-detects root and base ref"),
        "help must say pr auto-detects first-run inputs: {stdout}"
    );
    assert!(
        stdout.contains(
            "pr-setup  print read-only external GitHub PR checkout and raw-diff commands"
        ),
        "help must mention the read-only external PR setup helper: {stdout}"
    );

    Ok(())
}

#[test]
fn help_output_routes_to_per_command_help() -> Result<(), Box<dyn Error>> {
    // The top-level help is an overview, so it must tell the reader where the
    // full flag list for a single command lives.
    let output = checked_output(
        Command::new(env!("CARGO_BIN_EXE_cargo-unsafe-review"))
            .arg("unsafe-review")
            .arg("--help"),
    )?;
    let stdout = String::from_utf8(output.stdout)?;

    assert!(
        stdout.contains("unsafe-review <command> --help"),
        "top-level help must point at per-command help: {stdout}"
    );
    assert!(
        stdout.contains("Start here:"),
        "top-level help must offer a first-run entry point: {stdout}"
    );
    assert!(
        stdout.contains(
            "unsafe-review finds unsafe Rust changes missing a safety contract, guard, test, or witness."
        ),
        "top-level help must state the product sentence: {stdout}"
    );
    assert!(
        stdout.contains(
            "unsafe-review does not run witnesses, post comments, edit source, or block by default."
        ),
        "top-level help must state the advisory default posture: {stdout}"
    );

    Ok(())
}

#[test]
fn help_output_groups_and_lists_every_routable_command() -> Result<(), Box<dyn Error>> {
    let output = checked_output(
        Command::new(env!("CARGO_BIN_EXE_cargo-unsafe-review"))
            .arg("unsafe-review")
            .arg("--help"),
    )?;
    let stdout = String::from_utf8(output.stdout)?;

    let groups = [
        "Review a change:",
        "Inspect a finding:",
        "Track and discharge coverage debt:",
        "Repository posture:",
    ];
    for group in groups {
        assert!(
            stdout.contains(group),
            "top-level help must group commands by task, missing `{group}`: {stdout}"
        );
    }

    // Collect the command entries listed inside the task groups. A group runs
    // from its header to the next blank line; an entry is a line indented by
    // exactly two spaces (continuation lines are indented further).
    let mut listed: Vec<&str> = Vec::new();
    let mut group_text = String::new();
    let mut in_group = false;
    for line in stdout.lines() {
        if groups.contains(&line) {
            in_group = true;
            continue;
        }
        if line.trim().is_empty() {
            in_group = false;
            continue;
        }
        if !in_group {
            continue;
        }
        group_text.push_str(line);
        group_text.push('\n');
        let Some(rest) = line.strip_prefix("  ") else {
            continue;
        };
        if rest.starts_with(' ') {
            continue;
        }
        let name = rest.split(' ').next().unwrap_or_default();
        listed.push(name);

        // Descriptions share one column so each group reads as a table.
        let description_column = line.len() - rest.trim_start_matches(name).trim_start().len();
        assert_eq!(
            description_column, HELP_DESCRIPTION_COLUMN,
            "command `{name}` description must start at column {HELP_DESCRIPTION_COLUMN}: {line:?}"
        );
    }

    // Every command the parser routes appears in exactly one task group,
    // including the editor entrypoint, and the groups list nothing else.
    // Compatibility aliases that route into another command's entry are named
    // in that entry's text rather than taking a line of their own.
    assert!(
        group_text.contains("`receipt-template` is a compatibility name"),
        "the routed `receipt-template` alias must be named in the receipt entry: {stdout}"
    );

    let mut expected = vec![
        "check",
        "repo",
        "pr",
        "pr-setup",
        "first-pr",
        "review",
        "pilot",
        "badges",
        "explain",
        "context",
        "lsp",
        "candidate",
        "baseline",
        "confirm",
        "support",
        "outcome",
        "policy",
        "receipt",
        "doctor",
        "init",
    ];
    expected.sort_unstable();
    let mut actual = listed.clone();
    actual.sort_unstable();
    assert_eq!(
        actual, expected,
        "task groups must list every routed command exactly once: {stdout}"
    );

    Ok(())
}

/// Column (0-indexed) where every top-level help command description starts.
const HELP_DESCRIPTION_COLUMN: usize = 12;

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
fn first_pr_artifact_write_failure_prints_recovery() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");
    let temp = TempDir::new("unsafe-review-pr-artifact-write-failure")?;
    let blocked = temp.path().join("blocked-output");
    fs::write(&blocked, "not a directory\n")?;

    let output = Command::new(env!("CARGO_BIN_EXE_cargo-unsafe-review"))
        .arg("unsafe-review")
        .arg("pr")
        .arg("--root")
        .arg(&fixture)
        .arg("--diff")
        .arg(fixture.join("change.diff"))
        .arg("--out-dir")
        .arg(&blocked)
        .output()?;

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr)?;
    assert!(stderr.contains("create"));
    assert!(stderr.contains("blocked-output"));
    assert!(stderr.contains("Recovery:"));
    assert!(stderr.contains("unsafe-review doctor --root ."));
    assert!(stderr.contains("choose a writable output directory or parent"));
    assert!(String::from_utf8(output.stdout)?.is_empty());

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
        "unsafe-review pr-setup --repo <owner>/<repo> --number <number> --base-ref <base-ref-name> --base-sha <base-sha> --head-sha <head-sha> --root /path/to/repo --out-dir /path/to/review-kit --diff-out /path/to/change.diff",
        "git -C /path/to/repo fetch origin <base-ref-name> pull/<number>/head",
        "git -C /path/to/repo checkout --detach <head-sha>",
        "unsafe-review pr --root /path/to/repo --base-sha <base-sha> --head-sha <head-sha> --out-dir /path/to/review-kit",
        "Raw diff capture for receipts or --diff:",
        "git -C /path/to/repo diff --binary --full-index --output=/path/to/change.diff <base-sha>...<head-sha>",
        "unsafe-review pr --root /path/to/repo --diff /path/to/change.diff --out-dir /path/to/review-kit",
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
    let diff_out = std::env::current_dir()?.join("target/external-pilots/bytes-pr827.diff");
    let out_dir = std::env::current_dir()?.join("target/external-pilots/bytes-pr827/first-pr");
    let diff_out_parent = diff_out
        .parent()
        .ok_or("expected diff output path to have a parent")?;
    let diff_out_arg = rendered_shell_path(&diff_out);
    let out_dir_arg = rendered_shell_path(&out_dir);
    let diff_out_parent_arg = rendered_shell_path(diff_out_parent);
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
            .arg("--out-dir")
            .arg("target/external-pilots/bytes-pr827/first-pr")
            .arg("--diff-out")
            .arg("target/external-pilots/bytes-pr827.diff"),
    )?;
    let stdout = String::from_utf8(output.stdout)?;

    for expected in [
        "unsafe-review pr-setup".to_string(),
        "Read-only setup commands for external GitHub PR tokio-rs/bytes#827.".to_string(),
        "This command did not fetch, checkout, run unsafe-review, execute witnesses, post comments, or edit source.".to_string(),
        "gh pr view 827 --repo tokio-rs/bytes --json baseRefName,baseRefOid,headRefOid".to_string(),
        "git -C \"/tmp/bytes checkout\" fetch origin main pull/827/head".to_string(),
        "git -C \"/tmp/bytes checkout\" checkout --detach bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        format!("unsafe-review pr --root \"/tmp/bytes checkout\" --base-sha aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa --head-sha bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb --out-dir {out_dir_arg}"),
        format!("mkdir -p {diff_out_parent_arg}"),
        format!("git -C \"/tmp/bytes checkout\" diff --binary --full-index --output={diff_out_arg} aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa...bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        format!("unsafe-review pr --root \"/tmp/bytes checkout\" --diff {diff_out_arg} --out-dir {out_dir_arg}"),
        "baseRefName: main".to_string(),
        "baseRefOid: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        "headRefOid: bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        format!("out_dir: {}", out_dir.display()),
        "Trust boundary: always advisory;".to_string(),
    ] {
        assert_contains(&stdout, &expected);
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
fn pr_setup_rejects_shell_expanding_out_dir() -> Result<(), Box<dyn Error>> {
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
        .arg("--out-dir")
        .arg("target/$(whoami)")
        .output()?;

    assert!(
        !output.status.success(),
        "shell-expanding out dir must be rejected"
    );
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr)?;
    assert_contains(&stderr, "invalid --out-dir");
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
fn init_is_preview_only_deterministic_and_conflict_visible() -> Result<(), Box<dyn Error>> {
    let temp = TempDir::new("unsafe-review-init-e2e")?;
    let root = temp.path().join("repo");
    let workflow = root.join(".github/workflows/unsafe-review-first-pr.yml");
    fs::create_dir_all(workflow.parent().ok_or("workflow path has no parent")?)?;
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"init-fixture\"\n",
    )?;
    fs::write(root.join(".gitignore"), "target/\n")?;
    fs::write(&workflow, "name: owner-managed-workflow\n")?;
    let proposal_dir = temp.path().join("proposal");

    let run_init = || {
        Command::new(env!("CARGO_BIN_EXE_cargo-unsafe-review"))
            .arg("unsafe-review")
            .arg("init")
            .arg("--root")
            .arg(&root)
            .arg("--format")
            .arg("json")
            .arg("--out")
            .arg(&proposal_dir)
            .output()
    };

    let first = run_init()?;
    assert!(first.status.success(), "init failed: {:?}", first.status);
    let first_stdout = String::from_utf8(first.stdout)?;
    let first_json: Value = serde_json::from_str(&first_stdout)?;
    assert_eq!(first_json["mode"], "preview_only");
    assert_eq!(first_json["writes_repository"], false);
    assert_eq!(first_json["proposed_files"][0]["status"], "conflict");
    assert!(first_json["proposed_files"][0]["diff"]
        .as_str()
        .is_some_and(|diff| diff.contains("--- a/.github/workflows/unsafe-review-first-pr.yml")));
    assert_eq!(first_json["warnings"][0]["code"], "missing_git");
    assert!(String::from_utf8(first.stderr)?.contains("Proposal written:"));
    assert_eq!(
        fs::read_to_string(&workflow)?,
        "name: owner-managed-workflow\n"
    );

    let second = run_init()?;
    assert_eq!(first_stdout, String::from_utf8(second.stdout)?);
    assert!(proposal_dir.join("unsafe-review-init.json").is_file());

    let default_preview = Command::new(env!("CARGO_BIN_EXE_cargo-unsafe-review"))
        .arg("unsafe-review")
        .arg("init")
        .arg("--root")
        .arg(&root)
        .output()?;
    assert!(default_preview.status.success());
    let human = String::from_utf8(default_preview.stdout)?;
    assert!(human.contains("Recommendations:"));
    assert!(human.contains("optional_snippet"));
    assert!(human.contains("target/unsafe-review/unsafe-review-gate.json"));
    assert!(human.contains("<verified-release-ref>"));
    assert!(!root.join("unsafe-review-init.json").exists());

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

/// #2006 regression: a `--max-cards` capped `first-pr` run must not be
/// indistinguishable from a complete one on the front door.
///
/// Before the fix the capped run printed a smaller "Review cards" / "Open
/// actionable gaps" pair and nothing else, so it was byte-shaped exactly like a
/// genuine smaller result. The same root is run twice — capped and complete —
/// and only the capped run may carry the disclosure.
///
/// Drift-lock: drop the `capped_scan_notice` print in `print_first_pr_overview` → RED.
#[test]
fn capped_first_pr_run_discloses_the_cap_on_the_terminal() -> Result<(), Box<dyn Error>> {
    let temp = TempDir::new("unsafe-review-cli-capped-disclosure-e2e")?;
    let scan_root = temp.path().join("fixture");
    fs::create_dir_all(scan_root.join("src"))?;
    fs::write(
        scan_root.join("Cargo.toml"),
        "[package]\nname = \"capped-disclosure-fixture\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
    )?;
    fs::write(
        scan_root.join("src/lib.rs"),
        "pub unsafe fn alpha(ptr: *const u8) -> u8 { unsafe { *ptr } }\n\
         pub unsafe fn bravo(ptr: *const u8) -> u8 { unsafe { *ptr } }\n",
    )?;
    // first-pr is diff-scoped; supply the change that introduced both sites so the
    // run does not fall back to `git diff` against a non-repository.
    let diff_path = scan_root.join("change.diff");
    fs::write(
        &diff_path,
        "diff --git a/src/lib.rs b/src/lib.rs\n\
         --- a/src/lib.rs\n\
         +++ b/src/lib.rs\n\
         @@ -1,0 +1,2 @@\n\
         +pub unsafe fn alpha(ptr: *const u8) -> u8 { unsafe { *ptr } }\n\
         +pub unsafe fn bravo(ptr: *const u8) -> u8 { unsafe { *ptr } }\n",
    )?;

    let run = |out_dir: &Path, cap: Option<&str>| -> Result<String, Box<dyn Error>> {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-unsafe-review"));
        command
            .arg("unsafe-review")
            .arg("first-pr")
            .arg("--root")
            .arg(&scan_root)
            .arg("--diff")
            .arg(&diff_path)
            .arg("--out-dir")
            .arg(out_dir);
        if let Some(cap) = cap {
            command.arg("--max-cards").arg(cap);
        }
        let output = command.output()?;
        assert!(
            output.status.success(),
            "first-pr run must exit 0: status={:?}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr),
        );
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    };

    let capped = run(&temp.path().join("capped"), Some("1"))?;
    let complete = run(&temp.path().join("complete"), None)?;

    assert!(
        capped.contains("Partial scan:"),
        "a capped run must disclose the cap on the terminal, got:\n{capped}"
    );
    assert!(
        capped.contains("--max-cards 1"),
        "the disclosure must name the cap that bound the run, got:\n{capped}"
    );
    assert!(
        capped.contains("Retry without cap: unsafe-review first-pr")
            && capped.contains("--root")
            && capped.contains("--diff")
            && capped.contains("--out-dir"),
        "a capped run must provide an exact retry command, got:\n{capped}"
    );
    assert!(
        !complete.contains("Partial scan:"),
        "a complete run must not claim to be partial, got:\n{complete}"
    );
    // The disclosure is the ONLY thing distinguishing the two headline blocks —
    // that is precisely the bug, so assert the counts really do differ.
    assert!(
        capped.contains("- Review cards: 1"),
        "capped run must report the reduced card count, got:\n{capped}"
    );
    assert!(
        !complete.contains("- Review cards: 1"),
        "fixture must yield more than one card uncapped, got:\n{complete}"
    );

    Ok(())
}

/// #2006 regression: the capped state must reach the structured artifacts too,
/// so a machine consumer cannot read a truncated bundle as a complete inventory.
///
/// Drift-lock: stop projecting `scan_capped` into `cards.json` or the gate
/// manifest → RED.
#[test]
fn capped_first_pr_run_marks_cards_json_and_gate_manifest() -> Result<(), Box<dyn Error>> {
    let temp = TempDir::new("unsafe-review-cli-capped-artifacts-e2e")?;
    let scan_root = temp.path().join("fixture");
    fs::create_dir_all(scan_root.join("src"))?;
    fs::write(
        scan_root.join("Cargo.toml"),
        "[package]\nname = \"capped-artifacts-fixture\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
    )?;
    fs::write(
        scan_root.join("src/lib.rs"),
        "pub unsafe fn alpha(ptr: *const u8) -> u8 { unsafe { *ptr } }\n\
         pub unsafe fn bravo(ptr: *const u8) -> u8 { unsafe { *ptr } }\n",
    )?;
    // first-pr is diff-scoped; supply the change that introduced both sites so the
    // run does not fall back to `git diff` against a non-repository.
    let diff_path = scan_root.join("change.diff");
    fs::write(
        &diff_path,
        "diff --git a/src/lib.rs b/src/lib.rs\n\
         --- a/src/lib.rs\n\
         +++ b/src/lib.rs\n\
         @@ -1,0 +1,2 @@\n\
         +pub unsafe fn alpha(ptr: *const u8) -> u8 { unsafe { *ptr } }\n\
         +pub unsafe fn bravo(ptr: *const u8) -> u8 { unsafe { *ptr } }\n",
    )?;

    let out_dir = temp.path().join("bundle");
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-unsafe-review"))
        .arg("unsafe-review")
        .arg("first-pr")
        .arg("--root")
        .arg(&scan_root)
        .arg("--diff")
        .arg(&diff_path)
        .arg("--out-dir")
        .arg(&out_dir)
        .arg("--max-cards")
        .arg("1")
        .output()?;
    assert!(
        output.status.success(),
        "capped first-pr must exit 0: {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr),
    );

    let cards: Value = serde_json::from_str(&fs::read_to_string(out_dir.join("cards.json"))?)?;
    assert_eq!(
        cards["summary"]["scan_capped"], true,
        "cards.json summary must mark the run capped: {}",
        cards["summary"]
    );
    assert_eq!(
        cards["summary"]["card_cap"], 1,
        "cards.json summary must carry the cap: {}",
        cards["summary"]
    );
    assert!(
        cards["summary"]["unsafe_sites"].as_u64() > cards["summary"]["cards"].as_u64(),
        "a capped run must show more discovered sites than emitted cards: {}",
        cards["summary"]
    );

    let gate: Value = serde_json::from_str(&fs::read_to_string(
        out_dir.join("unsafe-review-gate.json"),
    )?)?;
    assert_eq!(
        gate["scan_capped"], true,
        "the gate manifest must disclose the cap alongside its movement counts: {gate}"
    );
    assert_eq!(
        gate["card_cap"], 1,
        "the gate manifest must carry the cap value: {gate}"
    );
    // The manifest stays advisory — disclosure is not a verdict.
    assert_eq!(
        gate["status"], "advisory",
        "cap disclosure must not change the advisory posture: {gate}"
    );

    let pr_summary = fs::read_to_string(out_dir.join("pr-summary.md"))?;
    assert!(
        pr_summary.contains("Partial scan:"),
        "the PR summary must disclose the cap next to its counts:\n{pr_summary}"
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

/// Hostile source-shape regression: a large but valid Rust file must remain a
/// truthful repo scan input rather than panic, silently fall back, or emit an
/// empty success report.
#[test]
fn repo_huge_source_file_scans_without_panic_or_scope_fallback() -> Result<(), Box<dyn Error>> {
    let temp = TempDir::new("unsafe-review-cli-huge-source-e2e")?;
    let scan_root = temp.path().join("fixture");
    fs::create_dir_all(scan_root.join("src"))?;
    fs::write(
        scan_root.join("Cargo.toml"),
        "[package]\nname = \"huge-source-fixture\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
    )?;
    let padding = "// bounded hostile-source padding for scan coverage\n".repeat(24_000);
    let source = format!(
        "{padding}pub unsafe fn read_byte(ptr: *const u8) -> u8 {{\n    unsafe {{ *ptr }}\n}}\n"
    );
    assert!(
        source.len() > 1_000_000,
        "fixture must exercise a large source file"
    );
    fs::write(scan_root.join("src/lib.rs"), source)?;

    let report_path = temp.path().join("repo.json");
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-unsafe-review"))
        .arg("unsafe-review")
        .arg("repo")
        .arg("--root")
        .arg(&scan_root)
        .arg("--format")
        .arg("json")
        .arg("--out")
        .arg(&report_path)
        .output()?;

    assert!(
        output.status.success(),
        "large source scan must not panic or fail: status={:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr),
    );
    let report: Value = serde_json::from_str(&fs::read_to_string(&report_path)?)?;
    assert_eq!(report["scope"], "repo");
    assert_eq!(report["summary"]["rust_files"], 1);
    assert!(report["summary"]["cards"].as_u64().unwrap_or(0) >= 1);
    assert!(
        report["cards"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|card| card["operation_family"] == "raw_pointer_deref"),
        "large source scan must preserve the raw-pointer dereference card: {report}"
    );
    Ok(())
}

fn assert_contains(haystack: &str, needle: &str) {
    assert!(
        haystack.contains(needle),
        "expected stdout to contain `{needle}`\nstdout:\n{haystack}"
    );
}

fn assert_not_contains(haystack: &str, needle: &str) {
    assert!(
        !haystack.contains(needle),
        "expected stdout not to contain `{needle}`\nstdout:\n{haystack}"
    );
}

fn rendered_shell_path(path: &Path) -> String {
    let raw = path.display().to_string();
    if cfg!(windows) {
        format!("\"{}\"", raw.replace('\\', "/"))
    } else {
        format!("\"{}\"", raw)
    }
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
