#![cfg(unix)]

use serde_json::Value;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn staged_facade_discovers_init_and_repeats_foreign_root_handoff() -> Result<(), Box<dyn Error>> {
    let temp = TempDir::new()?;
    let bin = temp.0.join("prefix/bin");
    fs::create_dir_all(&bin)?;
    // Stage the exact source-built facade without changing any existing install.
    let executable = bin.join("unsafe-review");
    fs::copy(env!("CARGO_BIN_EXE_unsafe-review"), &executable)?;
    let mut path_entries = vec![bin];
    path_entries.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let staged_path = std::env::join_paths(path_entries)?;
    let caller = temp.0.join("caller");
    let target = temp.0.join("target repo's $literal;name");
    init_repo(&caller)?;
    init_repo(&target)?;
    git(&target, &["update-ref", "refs/remotes/origin/main", "HEAD"])?;
    fs::write(
        target.join("src/lib.rs"),
        "pub unsafe fn changed_byte(ptr: *const u8) -> u8 { unsafe { *ptr } }\n",
    )?;
    git(&target, &["add", "src/lib.rs"])?;
    git(&target, &["commit", "-qm", "changed seam"])?;
    fs::write(
        caller.join("src/caller_only.rs"),
        "pub unsafe fn caller_only(p: *const u8) -> u8 { unsafe { *p } }\n",
    )?;
    let owner_workflow = target.join(".github/workflows/owner.yml");
    fs::create_dir_all(owner_workflow.parent().ok_or("workflow parent")?)?;
    fs::write(&owner_workflow, "name: owner-managed\n")?;
    let caller_before = git(&caller, &["status", "--porcelain"])?;
    let target_before = git(&target, &["status", "--porcelain"])?;

    let invoke = |args: &[&str]| -> Result<Output, Box<dyn Error>> {
        checked(
            Command::new("unsafe-review")
                .args(args)
                .env("PATH", &staged_path)
                .current_dir(&caller),
        )
    };
    let version = invoke(&["--version"])?;
    assert_eq!(
        String::from_utf8(version.stdout)?.trim(),
        format!("unsafe-review {}", env!("CARGO_PKG_VERSION"))
    );
    let help = String::from_utf8(invoke(&["init", "--help"])?.stdout)?;
    assert!(help.contains("unsafe-review init [--root .]"));
    assert!(help.contains("--out <directory>"));
    let preview = checked(
        Command::new("unsafe-review")
            .args(["init", "--root"])
            .arg(&target)
            .args(["--format", "json"])
            .env("PATH", &staged_path)
            .current_dir(&caller),
    )?;
    let proposal: Value = serde_json::from_slice(&preview.stdout)?;
    assert_eq!(proposal["mode"], "preview_only");
    assert_eq!(proposal["writes_repository"], false);
    assert!(!target.join("target").exists());
    assert_eq!(git(&target, &["status", "--porcelain"])?, target_before);
    let generated = proposal["commands"]["first_pr"]
        .as_str()
        .ok_or("first PR command")?;
    let destination = PathBuf::from(
        proposal["commands"]["first_pr_artifacts"]
            .as_str()
            .ok_or("artifact destination")?,
    );
    assert_eq!(
        destination,
        fs::canonicalize(&target)?.join("target/unsafe-review")
    );

    // Execute the actual generated command using PATH, rather than rewriting it.
    let follow = |cwd: &Path| -> Result<(), Box<dyn Error>> {
        checked(
            Command::new("sh")
                .args(["-c", generated])
                .env("PATH", &staged_path)
                .current_dir(cwd),
        )?;
        Ok(())
    };
    follow(&caller)?;
    let first: Value = serde_json::from_slice(&fs::read(destination.join("cards.json"))?)?;
    let cards = first["cards"].as_array().ok_or("cards")?;
    assert!(!cards.is_empty());
    for card in cards {
        assert_eq!(card["site"]["file"], "src/lib.rs");
        assert_eq!(card["site"]["owner"], "changed_byte");
    }
    follow(&temp.0)?;
    let repeated: Value = serde_json::from_slice(&fs::read(destination.join("cards.json"))?)?;
    assert_eq!(repeated["cards"], first["cards"]);
    assert_eq!(fs::read_to_string(owner_workflow)?, "name: owner-managed\n");
    assert_eq!(git(&caller, &["status", "--porcelain"])?, caller_before);
    assert_eq!(git(&target, &["status", "--porcelain"])?, target_before);
    for cwd in [&caller, &temp.0] {
        for destination in ["target", "policy", "badges"] {
            assert!(!cwd.join(destination).exists());
        }
    }
    Ok(())
}

fn init_repo(root: &Path) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(root.join("src"))?;
    git(root, &["init", "-q"])?;
    git(root, &["config", "user.name", "init consumer test"])?;
    git(root, &["config", "user.email", "init@example.test"])?;
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"init-consumer\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
    )?;
    fs::write(root.join(".gitignore"), "target/\n")?;
    fs::write(root.join("src/lib.rs"), "pub fn base() {}\n")?;
    fs::write(
        root.join("src/inherited.rs"),
        "pub unsafe fn inherited(p: *const u8) -> u8 { unsafe { *p } }\n",
    )?;
    git(root, &["add", "."])?;
    git(root, &["commit", "-qm", "base"])?;
    Ok(())
}

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>, Box<dyn Error>> {
    Ok(checked(Command::new("git").arg("-C").arg(root).args(args))?.stdout)
}

fn checked(command: &mut Command) -> Result<Output, Box<dyn Error>> {
    let output = command.output()?;
    if !output.status.success() {
        return Err(format!(
            "command failed: {:?}\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(output)
}

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Result<Self, Box<dyn Error>> {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let path = std::env::temp_dir().join(format!(
            "unsafe-review-init-consumer-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&path)?;
        Ok(Self(path))
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
