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

    assert_eq!(value["schema_version"], "0.1");
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

    assert_contains(&stdout, "unsafe-review first-pr");
    assert_contains(&stdout, "unsafe-review wrote an advisory PR bundle.");
    assert_contains(
        &stdout,
        &format!("- Artifact directory: {}", out_dir.display()),
    );
    assert_contains(&stdout, "Open:");
    assert_contains(
        &stdout,
        &out_dir.join("pr-summary.md").display().to_string(),
    );
    assert_contains(&stdout, "Agent repair queue:");
    assert_contains(
        &stdout,
        &format!(
            "{} (copy-only; unsafe-review did not run an agent)",
            out_dir.join("repair-queue.json").display()
        ),
    );
    assert_contains(&stdout, "Audit saved receipts:");
    assert_contains(
        &stdout,
        "saved receipt metadata only; unsafe-review did not run a witness",
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
    assert_contains(&stdout, "Artifacts:");
    assert_contains(
        &stdout,
        &out_dir.join("review-kit.json").display().to_string(),
    );
    assert_contains(
        &stdout,
        &out_dir.join("github-summary.md").display().to_string(),
    );
    assert_contains(
        &stdout,
        &out_dir.join("comment-plan.json").display().to_string(),
    );
    assert_contains(
        &stdout,
        &out_dir.join("receipt-audit.md").display().to_string(),
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
    assert!(stdout.contains("unsafe-review candidate import <manual-candidate.json>"));
    assert!(stdout.contains("unsafe-review candidate witness-plan"));
    assert!(stdout.contains("manual_candidate `true`"));
    assert!(stdout.contains("analyzer_discovered `false`"));
    assert!(stdout.contains("not analyzer-discovered findings"));
    assert!(!stdout.contains("Commands:\n  check"));

    Ok(())
}

fn assert_contains(haystack: &str, needle: &str) {
    assert!(
        haystack.contains(needle),
        "expected stdout to contain `{needle}`\nstdout:\n{haystack}"
    );
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
