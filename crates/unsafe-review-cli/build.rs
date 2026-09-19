//! Bake build-time identities for latency receipts.
//!
//! The receipt needs the exact tool source commit and toolchain, but the CLI
//! must not shell out to git or rustc on every run. This script captures both
//! at build time into `UNSAFE_REVIEW_BUILD_COMMIT` and
//! `UNSAFE_REVIEW_BUILD_RUSTC`. Either falls back to `"unknown"` when the
//! information is unavailable (source distribution without `.git`, or an
//! unusual toolchain layout); the receipt records the fallback honestly
//! rather than failing the run.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let git_dir = PathBuf::from(&manifest_dir)
        .join("..")
        .join("..")
        .join(".git");
    let commit = Command::new("git")
        .arg("--git-dir")
        .arg(&git_dir)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .ok()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|sha| sha.len() == 40 && sha.chars().all(|c| c.is_ascii_hexdigit()))
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=UNSAFE_REVIEW_BUILD_COMMIT={commit}");
    if git_dir.join("HEAD").is_file() {
        println!("cargo:rerun-if-changed=../../.git/HEAD");
    }
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
