//! Read-only Git discovery primitives behind the canonical change sets.
//!
//! Every helper here observes user state without mutating it: `rev-parse`,
//! `diff --name-status`, and `ls-files` variants that read the index,
//! worktree, and object names but never write objects, refs, index entries,
//! or worktree bytes. In particular the index identity is a digest over
//! `ls-files --stage` records, never a `write-tree` materialization.

use super::changeset::{FileChangeKind, FileRename, OmissionReason, OmittedFile};
use crate::sha256_hex_of;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Facts about the repository containing the discovery root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoFacts {
    /// Repository toplevel (absolute, local use only; never embedded in
    /// portable artifacts).
    pub toplevel: PathBuf,
    /// HEAD commit, or `None` when HEAD is unborn.
    pub head_commit: Option<String>,
    /// Whether the repository is shallow.
    pub shallow: bool,
}

/// Locate the repository toplevel for `start` and read its HEAD identity.
///
/// Fails closed when `start` is not inside a Git work tree. An unborn HEAD
/// (no commits yet) is not an error: `head_commit` is `None` and scopes that
/// need a commit report an unborn-head limitation instead.
pub fn discover_repo(start: &Path) -> Result<RepoFacts, String> {
    let toplevel = git_output(
        start,
        &["rev-parse", "--show-toplevel"],
        "locate the repository toplevel",
    )?;
    let toplevel = PathBuf::from(toplevel);
    let head = Command::new("git")
        .arg("-C")
        .arg(&toplevel)
        .args(["rev-parse", "--verify", "HEAD"])
        .output()
        .map_err(|err| format!("read HEAD commit failed: {err}"))?;
    let head_commit = if head.status.success() {
        Some(String::from_utf8_lossy(&head.stdout).trim().to_string())
    } else {
        None
    };
    let shallow = git_output(
        start,
        &["rev-parse", "--is-shallow-repository"],
        "check for a shallow repository",
    )?
    .trim()
        == "true";
    Ok(RepoFacts {
        toplevel,
        head_commit,
        shallow,
    })
}

/// Options bounding change-set discovery.
#[derive(Clone, Debug)]
pub struct DiscoverOptions {
    /// Include untracked Rust files in worktree-bearing scopes. Staged and
    /// unstaged scopes always list them as omitted instead.
    pub include_untracked: bool,
    /// Cap on listed files; further files become `CapTruncated` omissions
    /// with a `CapReached` limitation. `None` means unbounded.
    pub file_cap: Option<usize>,
}

impl Default for DiscoverOptions {
    fn default() -> Self {
        Self {
            include_untracked: true,
            file_cap: None,
        }
    }
}

/// Non-mutating identity of the current index state.
///
/// Canonical digest over `git ls-files --stage` records (mode, blob id,
/// index stage, path), sorted by path. Unlike `git write-tree` this never
/// writes objects into the repository object database: it observes the index
/// without changing the index, the worktree, refs, or stored objects.
/// Unmerged index entries are covered honestly (their nonzero stage is part
/// of the digest); unparseable output fails closed.
pub fn discover_index_state(toplevel: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(toplevel)
        .args(["ls-files", "--stage", "-z"])
        .output()
        .map_err(|err| format!("read the index state failed: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "read the index state failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let text = std::str::from_utf8(&output.stdout)
        .map_err(|err| format!("git ls-files output is not valid UTF-8: {err}"))?;
    let mut records = Vec::new();
    for record in text.split('\0') {
        if record.is_empty() {
            continue;
        }
        let (meta, path) = record.split_once('\t').ok_or_else(|| {
            "git ls-files produced a stage record without a path separator".to_string()
        })?;
        let mut fields = meta.split(' ');
        let mode = fields
            .next()
            .ok_or_else(|| "git ls-files produced a stage record without a mode".to_string())?;
        let blob = fields
            .next()
            .ok_or_else(|| "git ls-files produced a stage record without a blob id".to_string())?;
        let stage = fields.next().ok_or_else(|| {
            "git ls-files produced a stage record without an index stage".to_string()
        })?;
        if mode.is_empty() || blob.is_empty() || stage.is_empty() || path.is_empty() {
            return Err("git ls-files produced a stage record with an empty field".to_string());
        }
        records.push(format!("{mode}:{blob}:{stage}:{path}"));
    }
    records.sort();
    let mut encoding = String::new();
    for record in records {
        encoding.push_str(&record);
        encoding.push('\n');
    }
    Ok(format!(
        "index-sha256:{}",
        sha256_hex_of(encoding.as_bytes())
    ))
}

/// Re-verify that the repository did not move under a local-scope discovery.
///
/// Compares the current HEAD and index state against the values read before
/// discovery. A mismatch means a concurrent mutation (commit, amend, or
/// index update) published a mixed source generation: the change set is
/// rejected with an explicit error rather than presented as current.
/// Commit ranges compare immutable endpoints and need no such check.
pub(crate) fn verify_stable(
    toplevel: &Path,
    before_head: Option<&str>,
    before_index: &str,
) -> Result<(), String> {
    let head = Command::new("git")
        .arg("-C")
        .arg(toplevel)
        .args(["rev-parse", "--verify", "HEAD"])
        .output()
        .map_err(|err| format!("re-verify HEAD failed: {err}"))?;
    let after_head = if head.status.success() {
        Some(String::from_utf8_lossy(&head.stdout).trim().to_string())
    } else {
        None
    };
    if after_head.as_deref() != before_head {
        return Err(
            "repository changed during discovery (HEAD moved); the change set would mix source \
             generations. Re-run the scope command."
                .to_string(),
        );
    }
    let after_index = discover_index_state(toplevel)?;
    if after_index != before_index {
        return Err(
            "repository changed during discovery (index updated); the change set would mix source \
             generations. Re-run the scope command."
                .to_string(),
        );
    }
    Ok(())
}

/// One row of `git diff --name-status -M -z` output.
///
/// For renames, `path` is the source (old) path and `renamed_to` is the
/// target (new) path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NameStatusRow {
    pub(crate) status: char,
    pub(crate) path: PathBuf,
    pub(crate) renamed_to: Option<PathBuf>,
}

/// Run a read-only `git diff --name-status`.
///
/// `cached` selects index-vs-`HEAD` (`--cached`) instead of worktree-vs-index.
/// With explicit endpoints the diff runs between two commits. Rename
/// detection (`-M`) is always on so renames keep their from/to identity, and
/// output is NUL-delimited (`-z`) so unusual paths survive the round trip.
pub(crate) fn diff_name_status(
    toplevel: &Path,
    cached: bool,
    extra: &[&str],
) -> Result<Vec<NameStatusRow>, String> {
    let mut args = vec![
        "diff",
        "--name-status",
        "-M",
        "-z",
        "--no-color",
        "--no-ext-diff",
    ];
    if cached {
        args.push("--cached");
    }
    args.extend(extra);
    let output = Command::new("git")
        .arg("-C")
        .arg(toplevel)
        // Pin renames-only detection regardless of the user's `diff.renames`
        // configuration so identities stay comparable across machines.
        .arg("-c")
        .arg("diff.renames=true")
        .args(&args)
        .output()
        .map_err(|err| format!("git diff --name-status failed: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "git diff --name-status failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    parse_name_status_nul(&output.stdout)
}

/// Parse NUL-delimited `--name-status` rows. Rename rows carry the source
/// path in the path field and the target path as the next NUL field
/// (`R100\0<from>\0<to>\0`, the same order as the tab-delimited layout).
/// Copy rows (`C100\0<from>\0<to>\0`), should one ever appear despite
/// renames-only detection, consume both fields the same way so the parser
/// can never desynchronize; the target is treated as added content.
fn parse_name_status_nul(raw: &[u8]) -> Result<Vec<NameStatusRow>, String> {
    let text = std::str::from_utf8(raw)
        .map_err(|err| format!("git diff output is not valid UTF-8: {err}"))?;
    let mut fields = text.split('\0');
    let mut rows = Vec::new();
    while let Some(status_field) = fields.next() {
        if status_field.is_empty() {
            continue;
        }
        let status = status_field
            .chars()
            .next()
            .ok_or_else(|| "git diff produced an empty status field".to_string())?;
        let path_field = fields.next().ok_or_else(|| {
            format!("git diff status `{status_field}` has no path field; output truncated?")
        })?;
        // NUL-delimited rename/copy layout is `<status>\0<from>\0<to>\0`:
        // the path field names the source and the next field names the target.
        let (path, renamed_to) = if status == 'R' || status == 'C' {
            let to_field = fields.next().ok_or_else(|| {
                "git diff rename/copy row has no target path field; output truncated?".to_string()
            })?;
            (PathBuf::from(path_field), Some(PathBuf::from(to_field)))
        } else {
            (PathBuf::from(path_field), None)
        };
        rows.push(NameStatusRow {
            status,
            path,
            renamed_to,
        });
    }
    Ok(rows)
}

/// True for paths that must never resolve to a file: absolute paths and `..`
/// traversals escaping the root. Mirrors the hostile-input contract in
/// `input::diff` without duplicating its resolver.
pub(crate) fn is_rejected_path(path: &Path) -> bool {
    if path.is_absolute() {
        return true;
    }
    let mut depth = 0i32;
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                depth -= 1;
                if depth < 0 {
                    return true;
                }
            }
            std::path::Component::CurDir => {}
            std::path::Component::RootDir | std::path::Component::Prefix(_) => return true,
            std::path::Component::Normal(_) => depth += 1,
        }
    }
    false
}

pub(crate) fn file_kind_from_status(status: char) -> FileChangeKind {
    match status {
        'A' => FileChangeKind::Added,
        'D' => FileChangeKind::Deleted,
        'T' => FileChangeKind::TypeChanged,
        'U' => FileChangeKind::Modified,
        _ => FileChangeKind::Modified,
    }
}

/// Untracked files under the toplevel, repo-relative, honoring excludes.
pub(crate) fn untracked_files(toplevel: &Path) -> Result<Vec<PathBuf>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(toplevel)
        .args(["ls-files", "--others", "--exclude-standard", "-z"])
        .output()
        .map_err(|err| format!("git ls-files failed: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let text = std::str::from_utf8(&output.stdout)
        .map_err(|err| format!("git ls-files output is not valid UTF-8: {err}"))?;
    Ok(text
        .split('\0')
        .filter(|field| !field.is_empty())
        .map(PathBuf::from)
        .collect())
}

pub(crate) fn is_rust_path(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "rs")
}

/// Digest of worktree bytes that differ from the index for `paths`, plus the
/// full bytes of `untracked` files included by the scope. Files are read from
/// the worktree only; nothing is written. An unreadable file is reported
/// through `unreadable_out` and skipped, never silently dropped. Symlinks and
/// other non-regular files (fifos, sockets, directories standing in for a
/// listed file) are never followed or hashed: reading them could block
/// forever or pull out-of-tree bytes into the identity, so they are reported
/// through `non_regular_out` instead.
pub(crate) fn digest_worktree_state(
    toplevel: &Path,
    paths: &BTreeSet<PathBuf>,
    untracked: &[PathBuf],
    unreadable_out: &mut Vec<PathBuf>,
    non_regular_out: &mut Vec<PathBuf>,
) -> Result<String, String> {
    fn hash_one(
        toplevel: &Path,
        label: &str,
        path: &Path,
        hasher_input: &mut String,
        unreadable_out: &mut Vec<PathBuf>,
        non_regular_out: &mut Vec<PathBuf>,
    ) -> Result<(), String> {
        let abs = toplevel.join(path);
        let before = match std::fs::symlink_metadata(&abs) {
            Ok(meta) if meta.file_type().is_file() => meta,
            Ok(_) => {
                non_regular_out.push(path.to_path_buf());
                return Ok(());
            }
            Err(_) => {
                unreadable_out.push(path.to_path_buf());
                return Ok(());
            }
        };
        let bytes = match std::fs::read(&abs) {
            Ok(bytes) => bytes,
            Err(_) => {
                unreadable_out.push(path.to_path_buf());
                return Ok(());
            }
        };
        // The worktree may change under discovery: re-stat after the read and
        // reject the digest when the file moved mid-read, instead of
        // publishing a mixed source generation as current. A same-size,
        // same-mtime rewrite inside one read window stays undetectable; that
        // residual is documented, not denied.
        let after = std::fs::symlink_metadata(&abs).map_err(|err| {
            instability_error(path, &format!("vanished mid-read ({err})"))
        })?;
        if after.file_type() != before.file_type()
            || after.len() != before.len()
            || after.modified().ok() != before.modified().ok()
        {
            return Err(instability_error(path, "changed mid-read"));
        }
        hasher_input.push_str(label);
        hasher_input.push_str(&path.display().to_string());
        hasher_input.push('\n');
        hasher_input.push_str(&sha256_hex_of(&bytes));
        hasher_input.push('\n');
        Ok(())
    }
    fn instability_error(path: &Path, how: &str) -> String {
        format!(
            "worktree file {} {how}; the digest would mix source generations. Re-run the scope command.",
            path.display()
        )
    }
    let mut hasher_input = String::new();
    let mut sorted: Vec<&PathBuf> = paths.iter().collect();
    sorted.sort();
    for path in sorted {
        hash_one(
            toplevel,
            "",
            path,
            &mut hasher_input,
            unreadable_out,
            non_regular_out,
        )?;
    }
    let mut untracked_sorted: Vec<&PathBuf> = untracked.iter().collect();
    untracked_sorted.sort();
    for path in untracked_sorted {
        hash_one(
            toplevel,
            "untracked:",
            path,
            &mut hasher_input,
            unreadable_out,
            non_regular_out,
        )?;
    }
    Ok(format!(
        "worktree-sha256:{}",
        sha256_hex_of(hasher_input.as_bytes())
    ))
}

/// Map a status row to its effective (post-image) path, kind, and optional
/// rename record. Renames analyze as their target path with the source
/// carried as `FileChangeKind::Renamed { from }`. Copies analyze as added
/// content at the target path with no rename record: a copy is new content,
/// not a moved identity.
pub(crate) fn rename_mapping(row: &NameStatusRow) -> (PathBuf, FileChangeKind, Option<FileRename>) {
    match &row.renamed_to {
        Some(to) if row.status == 'R' => {
            let kind = FileChangeKind::Renamed {
                from: row.path.clone(),
            };
            let rename = FileRename {
                from: row.path.clone(),
                to: to.clone(),
            };
            (to.clone(), kind, Some(rename))
        }
        Some(to) => (to.clone(), FileChangeKind::Added, None),
        None => (row.path.clone(), file_kind_from_status(row.status), None),
    }
}

/// Effective (post-image) path of a row: the rename target, else the path.
pub(crate) fn effective_path(row: &NameStatusRow) -> &PathBuf {
    row.renamed_to.as_ref().unwrap_or(&row.path)
}

/// Collect rejected-path omissions for `rows`, naming the effective path.
pub(crate) fn rejected_omissions(rows: &[NameStatusRow]) -> Vec<OmittedFile> {
    rows.iter()
        .filter(|row| is_rejected_path(effective_path(row)))
        .map(|row| OmittedFile {
            path: effective_path(row).clone(),
            reason: OmissionReason::PathRejected,
        })
        .collect()
}

/// Split `items` into kept and dropped halves at `cap`. Callers name the
/// dropped files as `CapTruncated` omissions with a `CapReached` limitation
/// so the bound is never silent.
pub(crate) fn apply_cap_note<T>(mut items: Vec<T>, cap: Option<usize>) -> (Vec<T>, Vec<T>) {
    match cap {
        Some(limit) if items.len() > limit => {
            let dropped = items.split_off(limit);
            (items, dropped)
        }
        _ => (items, Vec::new()),
    }
}

pub(crate) fn git_output(dir: &Path, args: &[&str], step: &str) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|err| format!("{step} failed: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "{step} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
