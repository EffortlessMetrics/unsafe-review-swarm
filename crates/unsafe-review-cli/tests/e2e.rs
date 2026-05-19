use serde_json::Value;
use std::error::Error;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
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
fn cargo_subcommand_alias_reads_diff_from_stdin() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");
    let diff = fs::read_to_string(fixture.join("change.diff"))?;
    let output = checked_output_with_stdin(
        cargo_unsafe_review()
            .arg("check")
            .arg("--root")
            .arg(&fixture)
            .arg("--diff")
            .arg("-")
            .arg("--format")
            .arg("json"),
        &diff,
    )?;
    let stdout = String::from_utf8(output.stdout)?;
    let value: Value = serde_json::from_str(&stdout)?;

    assert_eq!(value["schema_version"], "0.1");
    assert_eq!(value["summary"]["cards"], 1);
    assert_eq!(value["cards"][0]["operation_family"], "raw_pointer_read");
    assert!(
        value["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("not a proof of memory safety")
    );

    Ok(())
}

#[test]
fn cargo_subcommand_alias_covers_current_review_artifacts() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_root("raw_pointer_alignment");
    let temp = TempDir::new("unsafe-review-current-artifacts-e2e")?;
    let card_id = raw_pointer_card_id(&fixture)?;

    let sarif_path = temp.path().join("cards.sarif");
    let output = checked_output(
        cargo_unsafe_review()
            .arg("check")
            .arg("--root")
            .arg(&fixture)
            .arg("--diff")
            .arg(fixture.join("change.diff"))
            .arg("--format")
            .arg("sarif")
            .arg("--out")
            .arg(&sarif_path),
    )?;
    assert_eq!(String::from_utf8(output.stdout)?.trim(), "");
    let sarif: Value = serde_json::from_str(&fs::read_to_string(sarif_path)?)?;
    assert_eq!(sarif["version"], "2.1.0");
    assert_eq!(
        sarif["runs"][0]["results"][0]["properties"]["cardId"],
        card_id
    );
    assert!(
        sarif["runs"][0]["properties"]["trustBoundary"]
            .as_str()
            .unwrap_or("")
            .contains("not a proof of memory safety")
    );

    let comment_plan_path = temp.path().join("comment-plan.json");
    let output = checked_output(
        cargo_unsafe_review()
            .arg("check")
            .arg("--root")
            .arg(&fixture)
            .arg("--diff")
            .arg(fixture.join("change.diff"))
            .arg("--format")
            .arg("comment-plan")
            .arg("--out")
            .arg(&comment_plan_path),
    )?;
    assert_eq!(String::from_utf8(output.stdout)?.trim(), "");
    let comment_plan: Value = serde_json::from_str(&fs::read_to_string(comment_plan_path)?)?;
    assert_eq!(comment_plan["mode"], "plan_only");
    assert_eq!(comment_plan["comments"][0]["card_id"], card_id);
    assert!(
        comment_plan["comments"][0]["body"]
            .as_str()
            .unwrap_or("")
            .contains("not memory-safety proof")
    );

    let lsp_path = temp.path().join("lsp.json");
    let output = checked_output(
        cargo_unsafe_review()
            .arg("check")
            .arg("--root")
            .arg(&fixture)
            .arg("--diff")
            .arg(fixture.join("change.diff"))
            .arg("--format")
            .arg("lsp")
            .arg("--out")
            .arg(&lsp_path),
    )?;
    assert_eq!(String::from_utf8(output.stdout)?.trim(), "");
    let lsp: Value = serde_json::from_str(&fs::read_to_string(lsp_path)?)?;
    assert_eq!(lsp["mode"], "read_only_projection");
    assert_eq!(lsp["diagnostics"][0]["card_id"], card_id);
    assert_eq!(
        lsp["code_actions"][0]["command"],
        "unsafe-review.copyAgentPacket"
    );

    let context = json_output(
        cargo_unsafe_review()
            .arg("context")
            .arg("--root")
            .arg(&fixture)
            .arg("--json")
            .arg(&card_id),
    )?;
    assert_eq!(context["mode"], "bounded_repair_packet");
    assert_eq!(context["card_id"], card_id);
    assert!(
        context["do_not_do"]
            .as_array()
            .is_some_and(|rules| rules.iter().any(|rule| rule
                .as_str()
                .unwrap_or("")
                .contains("do not claim Miri proof")))
    );

    let explain = json_output(
        cargo_unsafe_review()
            .arg("explain")
            .arg("--root")
            .arg(&fixture)
            .arg("--format")
            .arg("json")
            .arg(&card_id),
    )?;
    assert_eq!(explain["source"], "review_card");
    assert_eq!(explain["card"]["class"], "guard_missing");

    Ok(())
}

fn cargo_unsafe_review() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-unsafe-review"));
    command.arg("unsafe-review");
    command
}

fn raw_pointer_card_id(fixture: &Path) -> Result<String, Box<dyn Error>> {
    let output = json_output(
        cargo_unsafe_review()
            .arg("check")
            .arg("--root")
            .arg(fixture)
            .arg("--diff")
            .arg(fixture.join("change.diff"))
            .arg("--format")
            .arg("json"),
    )?;
    Ok(output["cards"][0]["id"]
        .as_str()
        .ok_or("card id missing from JSON output")?
        .to_string())
}

fn json_output(command: &mut Command) -> Result<Value, Box<dyn Error>> {
    let output = checked_output(command)?;
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn checked_output_with_stdin(command: &mut Command, stdin: &str) -> Result<Output, Box<dyn Error>> {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .as_mut()
        .ok_or("stdin was not piped")?
        .write_all(stdin.as_bytes())?;
    let output = child.wait_with_output()?;
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
