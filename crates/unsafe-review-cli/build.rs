//! Bake build-time identities for latency receipts.
//!
//! The receipt needs the exact tool source commit and toolchain, but the CLI
//! must not shell out to git or rustc on every run. This script captures both
//! at build time into `UNSAFE_REVIEW_BUILD_COMMIT` and
//! `UNSAFE_REVIEW_BUILD_RUSTC`. Either falls back to `"unknown"` when the
//! information is unavailable (source distribution without `.git`, or an
//! unusual toolchain layout); the receipt records the fallback honestly
//! rather than failing the run.
//!
//! The git directory is resolved through `git rev-parse --absolute-git-dir`
//! rather than assumed at `<workspace>/.git`, so linked worktrees (whose
//! git dir lives under the main checkout) rebuild when their HEAD moves.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let git_dir = Command::new("git")
        .arg("rev-parse")
        .arg("--absolute-git-dir")
        .current_dir(&manifest_dir)
        .output()
        .ok()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .filter(|dir| dir.is_dir());
    if let Some(dir) = &git_dir {
        let head = dir.join("HEAD");
        if head.is_file() {
            println!("cargo:rerun-if-changed={}", head.display());
        }
    }
    let commit = git_dir
        .as_ref()
        .and_then(|dir| {
            Command::new("git")
                .arg("--git-dir")
                .arg(dir)
                .arg("rev-parse")
                .arg("HEAD")
                .output()
                .ok()
        })
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|sha| sha.len() == 40 && sha.chars().all(|c| c.is_ascii_hexdigit()))
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=UNSAFE_REVIEW_BUILD_COMMIT={commit}");
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let version = Command::new(&rustc)
        .arg("--version")
        .output()
        .ok()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|line| line.starts_with("rustc "))
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=UNSAFE_REVIEW_BUILD_RUSTC={version}");
}
