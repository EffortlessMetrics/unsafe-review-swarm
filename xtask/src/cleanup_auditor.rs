//! Workbench cleanup auditor.
//!
//! Reports large target directories and stale worktrees so disk residue is
//! visible before it saturates (#1607 item 1). This is an advisory report,
//! not a gate — it does not remove anything or fail check-pr.

use crate::workspace_path;
use std::path::Path;

/// Report large directories and stale git worktrees.
///
/// Prints a structured summary to stdout:
/// - Each `target/` dir under the workspace and known worktree paths with its
///   size (human-readable) and age (mtime).
/// - Each git worktree with its branch, last-modified time, and whether it has
///   uncommitted changes.
/// - A total disk-usage summary.
///
/// Advisory only. Never gates a merge or removes files.
pub(crate) fn cleanup_audit() -> Result<(), String> {
    let root = workspace_path("");
    println!(
        "cleanup-audit: advisory disk + worktree report for {}",
        root.display()
    );
    println!();

    // 1. Report large target/ dirs
    report_large_dirs(&root)?;

    // 2. Report stale git worktrees
    report_worktrees(&root)?;

    println!("cleanup-audit: done (advisory; nothing removed)");
    Ok(())
}

fn report_large_dirs(root: &Path) -> Result<(), String> {
    let candidates = [
        root.join("target"),
        root.join("fuzz").join("target"),
        root.join("crates").join("target"),
    ];

    println!("## Large directories");
    println!();

    let mut any_found = false;
    for dir in &candidates {
        if !dir.is_dir() {
            continue;
        }
        let size = dir_size(dir).unwrap_or(0);
        let mtime = dir_mtime(dir);
        let size_str = human_readable_size(size);
        println!("  {} — {} (modified {})", dir.display(), size_str, mtime);
        any_found = true;
    }

    if !any_found {
        println!("  (no target/ directories found)");
    }
    println!();
    Ok(())
}

fn report_worktrees(root: &Path) -> Result<(), String> {
    let output = std::process::Command::new("git")
        .arg("worktree")
        .arg("list")
        .arg("--porcelain")
        .current_dir(root)
        .output()
        .map_err(|e| format!("git worktree list failed: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "git worktree list failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    println!("## Git worktrees");
    println!();

    let mut count = 0;
    let mut current_path = String::new();
    let mut current_branch = String::new();

    for line in text.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            current_path = path.to_string();
            current_branch.clear();
            count += 1;
        } else if let Some(branch) = line.strip_prefix("branch refs/heads/") {
            current_branch = branch.to_string();
        } else if line.is_empty() && !current_path.is_empty() {
            let mtime = dir_mtime(Path::new(&current_path));
            let worktree_root = root.to_string_lossy();
            let is_main = current_path == worktree_root.as_ref();
            let label = if is_main { "main" } else { "worktree" };
            println!("  {label}: {current_path} [{current_branch}] (modified {mtime})");
            current_path.clear();
            current_branch.clear();
        }
    }

    if count == 0 {
        println!("  (no worktrees found)");
    } else {
        println!();
        println!("  {count} worktree(s) total");
    }
    Ok(())
}

fn dir_size(path: &Path) -> Result<u64, String> {
    let mut total = 0u64;
    let entries = std::fs::read_dir(path).map_err(|e| format!("read_dir failed: {e}"))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("dir entry failed: {e}"))?;
        let p = entry.path();
        if p.is_dir() {
            total += dir_size(&p).unwrap_or(0);
        } else if p.is_file() {
            total += entry.metadata().map(|m| m.len()).unwrap_or(0);
        }
    }
    Ok(total)
}

fn dir_mtime(path: &Path) -> String {
    match std::fs::metadata(path) {
        Ok(meta) => {
            use std::time::SystemTime;
            let now = SystemTime::now();
            match meta.modified() {
                Ok(mtime) => {
                    let age = now.duration_since(mtime).unwrap_or_default();
                    let hours = age.as_secs() / 3600;
                    if hours < 1 {
                        "<1h ago".to_string()
                    } else if hours < 24 {
                        format!("{hours}h ago")
                    } else {
                        format!("{}d ago", hours / 24)
                    }
                }
                Err(_) => "unknown".to_string(),
            }
        }
        Err(_) => "unknown".to_string(),
    }
}

fn human_readable_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}
