//! Change-set scope discovery against disposable real Git repositories
//! (issues #2308 PR1, #2318 PR1 foundations).
//!
//! Every test builds its own repository under the system temp dir, so no
//! test touches the user's index or worktree bytes. Tests return
//! `Result<(), String>` and never `expect`/`panic`, per the workspace
//! clippy gates.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use unsafe_review_core::{
    ChangeScopeKind, ChangedFile, DiscoverOptions, FileChangeKind, FileProvenance, OmissionReason,
    OmittedFile, RepoFacts, ScopeCompleteness, changeset_from_external_diff,
    changeset_from_overlay, changeset_from_snapshot, discover_commit_range, discover_repo,
    discover_staged, discover_unstaged, discover_worktree, render_changeset_human,
    render_changeset_json,
};

fn unique_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()))
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|err| format!("git {args:?} failed to spawn: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn write(dir: &Path, rel: &str, content: &str) -> Result<(), String> {
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("create parent dirs failed: {err}"))?;
    }
    fs::write(&path, content).map_err(|err| format!("write fixture file failed: {err}"))?;
    Ok(())
}

fn has_path(files: &[ChangedFile], want: &str) -> bool {
    files
        .iter()
        .any(|file| file.path.as_path() == Path::new(want))
}

fn omitted_as(files: &[OmittedFile], want: &str) -> Option<OmissionReason> {
    files
        .iter()
        .find(|file| file.path.as_path() == Path::new(want))
        .map(|file| file.reason.clone())
}

/// A fresh repo with one committed Rust file.
fn fresh_repo(prefix: &str) -> Result<PathBuf, String> {
    let dir = unique_dir(prefix);
    fs::create_dir_all(&dir).map_err(|err| format!("create temp repo failed: {err}"))?;
    git(&dir, &["init", "-q"])?;
    git(&dir, &["config", "user.email", "test@example.com"])?;
    git(&dir, &["config", "user.name", "changeset-test"])?;
    write(&dir, "src/lib.rs", "pub fn base() {}\n")?;
    git(&dir, &["add", "src/lib.rs"])?;
    git(&dir, &["commit", "-qm", "base"])?;
    Ok(dir)
}

fn repo_facts(dir: &Path) -> Result<RepoFacts, String> {
    discover_repo(dir)
}

fn require_partial_with(
    completeness: &ScopeCompleteness,
    needle: &str,
    what: &str,
) -> Result<(), String> {
    match completeness {
        ScopeCompleteness::Partial { limitations } => {
            if limitations
                .iter()
                .any(|limitation| limitation.detail.contains(needle))
            {
                Ok(())
            } else {
                Err(format!("{what} must be reported: {limitations:?}"))
            }
        }
        ScopeCompleteness::Complete => Err(format!("{what} must not be complete")),
    }
}

#[test]
fn staged_scope_reads_index_not_later_worktree_edits() -> Result<(), String> {
    let dir = fresh_repo("changeset-staged")?;
    write(&dir, "src/lib.rs", "pub fn staged_version() {}\n")?;
    git(&dir, &["add", "src/lib.rs"])?;
    // A later worktree edit must not leak into the staged result.
    write(&dir, "src/lib.rs", "pub fn worktree_version() {}\n")?;

    let set = discover_staged(&dir, &repo_facts(&dir)?, &DiscoverOptions::default())?;
    if set.scope != ChangeScopeKind::Staged {
        return Err(format!("expected staged scope, got {:?}", set.scope));
    }
    if set.included_files.len() != 1 {
        return Err(format!(
            "expected exactly the staged file, got {:?}",
            set.included_files
        ));
    }
    if !has_path(&set.included_files, "src/lib.rs") {
        return Err("staged scope must name the staged file".to_string());
    }
    match &set.identities.index_state {
        Some(state) if state.starts_with("index-sha256:") => {}
        other => {
            return Err(format!(
                "staged scope must pin the non-mutating index state, got {other:?}"
            ));
        }
    }
    // The extra worktree edit is reported, not included.
    require_partial_with(
        &set.completeness,
        "unstaged",
        "staged scope with later worktree edits",
    )
}

#[test]
fn staged_scope_includes_new_files_and_lists_untracked_as_omitted() -> Result<(), String> {
    let dir = fresh_repo("changeset-staged-new")?;
    write(&dir, "src/new.rs", "pub fn fresh() {}\n")?;
    git(&dir, &["add", "src/new.rs"])?;
    write(&dir, "src/untracked.rs", "pub fn ghost() {}\n")?;

    let set = discover_staged(&dir, &repo_facts(&dir)?, &DiscoverOptions::default())?;
    if !has_path(&set.included_files, "src/new.rs") {
        return Err("staged new files must be included".to_string());
    }
    match omitted_as(&set.omitted_files, "src/untracked.rs") {
        Some(OmissionReason::UntrackedExcludedByScope) => Ok(()),
        other => Err(format!(
            "untracked file must be listed with an explicit scope reason, got {other:?}"
        )),
    }
}

#[test]
fn unstaged_scope_excludes_already_staged_hunks() -> Result<(), String> {
    let dir = fresh_repo("changeset-unstaged")?;
    // Stage a new file, then edit a tracked file without staging it.
    // (`git diff` never shows untracked files, so the unstaged edit must
    // touch a tracked file to be visible at all.)
    write(&dir, "src/staged.rs", "pub fn s() {}\n")?;
    git(&dir, &["add", "src/staged.rs"])?;
    write(&dir, "src/lib.rs", "pub fn base_edited() {}\n")?;

    let set = discover_unstaged(&dir, &repo_facts(&dir)?, &DiscoverOptions::default())?;
    if set.scope != ChangeScopeKind::Unstaged {
        return Err(format!("expected unstaged scope, got {:?}", set.scope));
    }
    if has_path(&set.included_files, "src/staged.rs") {
        return Err("already-staged hunks are not new unstaged work".to_string());
    }
    if !has_path(&set.included_files, "src/lib.rs") {
        return Err(format!(
            "tracked worktree edits must be included: {:?}",
            set.included_files
        ));
    }
    require_partial_with(
        &set.completeness,
        "staged",
        "unstaged scope with staged remainder",
    )
}

#[test]
fn worktree_scope_unions_staged_and_unstaged_without_duplicates() -> Result<(), String> {
    let dir = fresh_repo("changeset-worktree")?;
    // Stage a change, then edit the same file again: mixed scope.
    write(&dir, "src/lib.rs", "pub fn staged_edit() {}\n")?;
    git(&dir, &["add", "src/lib.rs"])?;
    write(&dir, "src/lib.rs", "pub fn staged_then_edited() {}\n")?;
    write(&dir, "src/extra.rs", "pub fn extra() {}\n")?;

    let set = discover_worktree(&dir, &repo_facts(&dir)?, &DiscoverOptions::default())?;
    if set.scope != ChangeScopeKind::Worktree {
        return Err(format!("expected worktree scope, got {:?}", set.scope));
    }
    let lib_entries: Vec<&ChangedFile> = set
        .included_files
        .iter()
        .filter(|file| file.path.as_path() == Path::new("src/lib.rs"))
        .collect();
    if lib_entries.len() != 1 {
        return Err(format!(
            "one final operation must not become duplicate entries: {:?}",
            set.included_files
        ));
    }
    if lib_entries[0].provenance != FileProvenance::StagedAndUnstaged {
        return Err(format!(
            "a staged-then-edited file reports both states, got {:?}",
            lib_entries[0].provenance
        ));
    }
    if !has_path(&set.included_files, "src/extra.rs") {
        return Err("untracked files are included by the default worktree scope".to_string());
    }
    Ok(())
}

#[test]
fn worktree_staged_modified_then_deleted_is_deleted() -> Result<(), String> {
    // Net-kind rule: a file staged as modified and then deleted from the
    // worktree is finally deleted. It must appear as Deleted, never as a
    // Modified file with an Unreadable omission for bytes that no longer exist.
    let dir = fresh_repo("changeset-net-delete")?;
    write(&dir, "src/lib.rs", "pub fn v1() {}\n")?;
    git(&dir, &["add", "src/lib.rs"])?;
    fs::remove_file(dir.join("src/lib.rs"))
        .map_err(|err| format!("delete worktree file failed: {err}"))?;

    let set = discover_worktree(&dir, &repo_facts(&dir)?, &DiscoverOptions::default())?;
    let entry = set
        .included_files
        .iter()
        .find(|file| file.path.as_path() == Path::new("src/lib.rs"))
        .ok_or_else(|| "deleted path must stay in the worktree scope".to_string())?;
    if entry.kind != FileChangeKind::Deleted {
        return Err(format!("net kind must be deleted, got {:?}", entry.kind));
    }
    if omitted_as(&set.omitted_files, "src/lib.rs").is_some() {
        return Err("a net deletion is an included fact, not an omission".to_string());
    }
    Ok(())
}

#[test]
fn worktree_staged_deleted_then_recreated_hashes_content() -> Result<(), String> {
    // Net-kind rule: a file staged as deleted and then recreated carries its
    // recreated bytes. The digest must cover the content, not a silent deletion.
    let dir = fresh_repo("changeset-net-recreate")?;
    git(&dir, &["rm", "-q", "src/lib.rs"])?;
    write(&dir, "src/lib.rs", "pub fn recreated() {}\n")?;

    let repo = repo_facts(&dir)?;
    let options = DiscoverOptions::default();
    let first = discover_worktree(&dir, &repo, &options)?;
    let entries: Vec<&ChangedFile> = first
        .included_files
        .iter()
        .filter(|file| file.path.as_path() == Path::new("src/lib.rs"))
        .collect();
    if entries.len() != 1 {
        return Err(format!(
            "recreated path must appear exactly once, got {:?}",
            first.included_files
        ));
    }
    if entries[0].kind == FileChangeKind::Deleted {
        return Err("recreated content must not analyze as a deletion".to_string());
    }
    write(&dir, "src/lib.rs", "pub fn recreated_v2() {}\n")?;
    let second = discover_worktree(&dir, &repo, &options)?;
    if first.digest == second.digest {
        return Err("recreated bytes must enter the subject digest".to_string());
    }
    Ok(())
}

#[test]
fn worktree_scope_without_untracked_lists_them_as_omitted() -> Result<(), String> {
    let dir = fresh_repo("changeset-worktree-omit")?;
    write(&dir, "src/ghost.rs", "pub fn ghost() {}\n")?;

    let options = DiscoverOptions {
        include_untracked: false,
        ..DiscoverOptions::default()
    };
    let set = discover_worktree(&dir, &repo_facts(&dir)?, &options)?;
    if has_path(&set.included_files, "src/ghost.rs") {
        return Err("excluded untracked files must not be included".to_string());
    }
    match omitted_as(&set.omitted_files, "src/ghost.rs") {
        Some(OmissionReason::UntrackedExcludedByScope) => Ok(()),
        other => Err(format!(
            "excluded untracked files must be listed with a reason, got {other:?}"
        )),
    }
}

#[test]
fn commit_range_names_both_endpoints_and_rejects_missing_base() -> Result<(), String> {
    let dir = fresh_repo("changeset-range")?;
    write(&dir, "src/lib.rs", "pub fn v2() {}\n")?;
    git(&dir, &["commit", "-qam", "second"])?;
    let head = git(&dir, &["rev-parse", "HEAD"])?;
    let base = git(&dir, &["rev-parse", "HEAD~1"])?;

    let repo = repo_facts(&dir)?;
    let set = discover_commit_range(&dir, repo.shallow, &base, &head)?;
    if set.scope != ChangeScopeKind::CommitRange {
        return Err(format!("expected commit_range scope, got {:?}", set.scope));
    }
    if set.identities.base_commit.as_deref() != Some(base.as_str()) {
        return Err(format!(
            "range must pin the base commit, got {:?}",
            set.identities.base_commit
        ));
    }
    if set.identities.head_commit.as_deref() != Some(head.as_str()) {
        return Err(format!(
            "range must pin the head commit, got {:?}",
            set.identities.head_commit
        ));
    }
    if !has_path(&set.included_files, "src/lib.rs") {
        return Err("range must include the changed file".to_string());
    }

    // A missing base fails closed: no silent widening into a repo scan.
    match discover_commit_range(&dir, repo.shallow, "deadbeef", &head) {
        Err(err) if err.contains("resolve the range base") => Ok(()),
        Err(err) => Err(format!(
            "missing base error must name the failing step: {err}"
        )),
        Ok(_) => Err("missing base must fail".to_string()),
    }
}

#[test]
fn rename_keeps_from_to_identity() -> Result<(), String> {
    let dir = fresh_repo("changeset-rename")?;
    git(&dir, &["mv", "src/lib.rs", "src/renamed.rs"])?;
    write(&dir, "src/renamed.rs", "pub fn base() {}\n// touched\n")?;
    git(&dir, &["add", "-A"])?;

    let set = discover_staged(&dir, &repo_facts(&dir)?, &DiscoverOptions::default())?;
    if set.renames.len() != 1 {
        return Err(format!("rename must be carried: {set:?}"));
    }
    if set.renames[0].from.as_path() != Path::new("src/lib.rs") {
        return Err(format!(
            "rename source must be the old path: {:?}",
            set.renames[0]
        ));
    }
    if set.renames[0].to.as_path() != Path::new("src/renamed.rs") {
        return Err(format!(
            "rename target must be the new path: {:?}",
            set.renames[0]
        ));
    }
    if !has_path(&set.included_files, "src/renamed.rs") {
        return Err("renames analyze as their target path".to_string());
    }
    Ok(())
}

#[test]
fn digest_is_stable_for_same_state_and_changes_with_scope() -> Result<(), String> {
    let dir = fresh_repo("changeset-digest")?;
    write(&dir, "src/lib.rs", "pub fn v2() {}\n")?;
    git(&dir, &["add", "src/lib.rs"])?;

    let repo = repo_facts(&dir)?;
    let options = DiscoverOptions::default();
    let first = discover_staged(&dir, &repo, &options)?;
    let second = discover_staged(&dir, &repo, &options)?;
    if first.digest != second.digest {
        return Err("identical states must produce identical digests".to_string());
    }
    let worktree = discover_worktree(&dir, &repo, &options)?;
    if first.digest == worktree.digest {
        return Err("a scope change must alter the subject digest".to_string());
    }
    Ok(())
}

#[test]
fn projections_name_scope_identities_counts_and_completeness() -> Result<(), String> {
    let dir = fresh_repo("changeset-render")?;
    write(&dir, "src/lib.rs", "pub fn v2() {}\n")?;
    git(&dir, &["add", "src/lib.rs"])?;

    let set = discover_staged(&dir, &repo_facts(&dir)?, &DiscoverOptions::default())?;
    let human = render_changeset_human(&set);
    for needle in [
        "scope: staged",
        "digest: changeset-sha256:",
        "index: index-sha256:",
        "included: 1 file(s)",
        "src/lib.rs",
        "completeness: ",
    ] {
        if !human.contains(needle) {
            return Err(format!("human summary must contain `{needle}`:\n{human}"));
        }
    }
    let json = render_changeset_json(&set)?;
    let parsed: serde_json::Value =
        serde_json::from_str(&json).map_err(|err| format!("json must parse: {err}"))?;
    if parsed["scope"] != "staged" {
        return Err(format!("json scope must be staged: {json}"));
    }
    let digest = parsed["digest"].as_str().unwrap_or("");
    if !digest.starts_with("changeset-sha256:") {
        return Err(format!("json digest must carry the scheme: {json}"));
    }
    if parsed["included_files"].as_array().map(Vec::len) != Some(1) {
        return Err(format!("json must list one included file: {json}"));
    }
    Ok(())
}

#[test]
fn external_snapshot_and_overlay_constructors_never_share_digests() -> Result<(), String> {
    let root = PathBuf::from("repo");
    let external = changeset_from_external_diff(
        root.clone(),
        b"diff --git a/src/lib.rs b/src/lib.rs",
        vec![PathBuf::from("src/lib.rs")],
    );
    if external.scope != ChangeScopeKind::ExternalDiff {
        return Err(format!(
            "expected external_diff scope, got {:?}",
            external.scope
        ));
    }
    let diff_digest = external.identities.diff_digest.clone().unwrap_or_default();
    if !diff_digest.starts_with("diff-sha256:") {
        return Err("external scope binds the supplied bytes".to_string());
    }
    let snapshot = changeset_from_snapshot(root.clone(), "source-sha256:abc".to_string());
    if snapshot.scope != ChangeScopeKind::RepoSnapshot {
        return Err(format!(
            "expected repo_snapshot scope, got {:?}",
            snapshot.scope
        ));
    }
    let overlay = changeset_from_overlay(
        root,
        "saved-sha256:abc".to_string(),
        7,
        "overlay-sha256:def".to_string(),
    );
    if overlay.scope != ChangeScopeKind::DocumentOverlay {
        return Err(format!(
            "expected document_overlay scope, got {:?}",
            overlay.scope
        ));
    }
    if external.digest == snapshot.digest
        || snapshot.digest == overlay.digest
        || external.digest == overlay.digest
    {
        return Err("distinct scopes must never share a subject digest".to_string());
    }
    Ok(())
}

#[test]
fn hostile_diff_paths_are_listed_never_resolved() -> Result<(), String> {
    let root = PathBuf::from("repo");
    let set = changeset_from_external_diff(
        root,
        b"diff",
        vec![
            PathBuf::from("/etc/absolute.rs"),
            PathBuf::from("../traversal.rs"),
            PathBuf::from("src/ok.rs"),
        ],
    );
    if !has_path(&set.included_files, "src/ok.rs") {
        return Err("normal paths are still included".to_string());
    }
    for hostile in ["../traversal.rs", "/etc/absolute.rs"] {
        match omitted_as(&set.omitted_files, hostile) {
            Some(OmissionReason::PathRejected) => {}
            other => {
                return Err(format!(
                    "{hostile} must be listed as path-rejected, got {other:?}"
                ));
            }
        }
        if has_path(&set.included_files, hostile) {
            return Err(format!("{hostile} must never be included"));
        }
    }
    Ok(())
}

#[test]
fn non_repository_root_fails_closed() -> Result<(), String> {
    let dir = unique_dir("changeset-norepo");
    fs::create_dir_all(&dir).map_err(|err| format!("create plain dir failed: {err}"))?;
    match discover_repo(&dir) {
        Err(err) if err.contains("toplevel") => Ok(()),
        Err(err) => Err(format!("non-repo error must name the failing step: {err}")),
        Ok(_) => Err("non-repo discovery must fail".to_string()),
    }
}

#[test]
fn index_state_ignores_worktree_bytes_but_tracks_index() -> Result<(), String> {
    // The staged identity must follow index blobs, not later worktree bytes:
    // editing the worktree without staging leaves the index digest unchanged,
    // while staging the edit changes it.
    let dir = fresh_repo("changeset-index-state")?;
    write(&dir, "src/lib.rs", "pub fn v1() {}\n")?;
    git(&dir, &["add", "src/lib.rs"])?;
    let repo = repo_facts(&dir)?;
    let options = DiscoverOptions::default();
    let before = discover_staged(&dir, &repo, &options)?;
    let index_before = before.identities.index_state.clone().unwrap_or_default();

    write(&dir, "src/lib.rs", "pub fn v2_worktree_only() {}\n")?;
    let after_edit = discover_staged(&dir, &repo, &options)?;
    if after_edit.identities.index_state.as_deref() != Some(index_before.as_str()) {
        return Err("unstaged worktree bytes must not move the index identity".to_string());
    }

    git(&dir, &["add", "src/lib.rs"])?;
    let after_stage = discover_staged(&dir, &repo, &options)?;
    if after_stage.identities.index_state.as_deref() == Some(index_before.as_str()) {
        return Err("staging new content must move the index identity".to_string());
    }
    Ok(())
}

#[test]
fn same_file_list_changed_bytes_changes_digest() -> Result<(), String> {
    // File lists staying constant while bytes change must produce a
    // different subject identity (worktree bytes are hashed, not just listed).
    let dir = fresh_repo("changeset-bytes-digest")?;
    write(&dir, "src/lib.rs", "pub fn v1() {}\n")?;
    let repo = repo_facts(&dir)?;
    let options = DiscoverOptions::default();
    let first = discover_worktree(&dir, &repo, &options)?;
    write(&dir, "src/lib.rs", "pub fn v2() {}\n")?;
    let second = discover_worktree(&dir, &repo, &options)?;
    if first.digest == second.digest {
        return Err("changed bytes with a constant file list must change the digest".to_string());
    }
    Ok(())
}

#[test]
fn digest_has_no_cwd_dependent_absolute_path() -> Result<(), String> {
    // The same repository state copied to a different absolute path must
    // produce the same subject digest: portable digests never embed the
    // checkout location.
    let dir = fresh_repo("changeset-abspath")?;
    write(&dir, "src/lib.rs", "pub fn v1() {}\n")?;
    git(&dir, &["add", "src/lib.rs"])?;
    let repo = repo_facts(&dir)?;
    let options = DiscoverOptions::default();
    let first = discover_staged(&dir, &repo, &options)?;

    let copy = unique_dir("changeset-abspath-copy");
    copy_dir_all(&dir, &copy)?;
    let copy_repo = repo_facts(&copy)?;
    let second = discover_staged(&copy, &copy_repo, &options)?;
    if first.root == second.root {
        return Err("the copied checkout must live at a different path".to_string());
    }
    if first.digest != second.digest {
        return Err(format!(
            "identical state at different paths must digest identically:\n{}\n{}",
            first.digest, second.digest
        ));
    }
    Ok(())
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|err| format!("create copy dir failed: {err}"))?;
    for entry in fs::read_dir(src).map_err(|err| format!("read copy source failed: {err}"))? {
        let entry = entry.map_err(|err| format!("read copy entry failed: {err}"))?;
        let name = entry.file_name();
        if name == "target" {
            continue;
        }
        let from = entry.path();
        let to = dst.join(&name);
        let kind = entry
            .file_type()
            .map_err(|err| format!("stat copy entry failed: {err}"))?;
        if kind.is_dir() {
            copy_dir_all(&from, &to)?;
        } else if kind.is_file() {
            fs::copy(&from, &to).map_err(|err| format!("copy file failed: {err}"))?;
        }
    }
    Ok(())
}

#[test]
fn cap_truncation_names_dropped_files() -> Result<(), String> {
    let dir = fresh_repo("changeset-cap")?;
    for name in ["a.rs", "b.rs", "c.rs"] {
        write(&dir, &format!("src/{name}"), "pub fn f() {}\n")?;
    }
    git(&dir, &["add", "src/a.rs", "src/b.rs", "src/c.rs"])?;
    let options = DiscoverOptions {
        file_cap: Some(1),
        ..DiscoverOptions::default()
    };
    let set = discover_staged(&dir, &repo_facts(&dir)?, &options)?;
    if set.included_files.len() != 1 {
        return Err(format!(
            "cap of 1 must keep exactly one file, got {:?}",
            set.included_files
        ));
    }
    let truncated: Vec<&Path> = set
        .omitted_files
        .iter()
        .filter(|file| file.reason == OmissionReason::CapTruncated)
        .map(|file| file.path.as_path())
        .collect();
    if truncated.len() != 2 {
        return Err(format!(
            "both dropped files must be named as cap-truncated, got {truncated:?}"
        ));
    }
    require_partial_with(&set.completeness, "cap", "capped discovery")
}

#[test]
fn shallow_range_reports_history_limitation() -> Result<(), String> {
    // A shallow clone keeps only fetched history: the range stays explicit
    // about that instead of silently covering less.
    let src = fresh_repo("changeset-shallow-src")?;
    write(&src, "src/lib.rs", "pub fn v2() {}\n")?;
    git(&src, &["commit", "-qam", "second"])?;
    let dst = unique_dir("changeset-shallow");
    let status = std::process::Command::new("git")
        .args(["clone", "--depth", "2", "--no-local", "-q"])
        .arg(&src)
        .arg(&dst)
        .status()
        .map_err(|err| format!("git clone failed to spawn: {err}"))?;
    if !status.success() {
        return Err("git clone --depth 2 failed".to_string());
    }
    let repo = repo_facts(&dst)?;
    if !repo.shallow {
        return Err("depth-2 clone must report shallow".to_string());
    }
    let head = git(&dst, &["rev-parse", "HEAD"])?;
    let base = git(&dst, &["rev-parse", "HEAD~1"])?;
    let set = discover_commit_range(&dst, repo.shallow, &base, &head)?;
    require_partial_with(&set.completeness, "shallow", "shallow range")
}

#[test]
#[cfg(unix)]
fn non_utf8_path_fails_explicitly() -> Result<(), String> {
    // A non-UTF-8 index entry must fail the scope with a named step, never
    // corrupt the identity or silently drop the file.
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    let dir = fresh_repo("changeset-nonutf8")?;
    let raw = b"src/\xff\xfe.rs";
    let rel = OsStr::from_bytes(raw);
    fs::write(dir.join(rel), "pub fn f() {}\n")
        .map_err(|err| format!("write non-UTF-8 fixture failed: {err}"))?;
    git(&dir, &["add", "."])?;
    match discover_staged(&dir, &repo_facts(&dir)?, &DiscoverOptions::default()) {
        Err(err) if err.contains("UTF-8") => Ok(()),
        Err(err) => Err(format!("non-UTF-8 input must fail at a named step: {err}")),
        Ok(_) => Err("non-UTF-8 index entries must not produce an identity".to_string()),
    }
}

#[test]
#[cfg(unix)]
fn symlink_is_listed_not_followed() -> Result<(), String> {
    // Replacing a tracked file with a symlink to out-of-tree content must not
    // pull foreign bytes into the digest: the path is listed as non-regular.
    let dir = fresh_repo("changeset-symlink")?;
    let outside = unique_dir("changeset-symlink-outside");
    fs::create_dir_all(&outside).map_err(|err| format!("create outside dir failed: {err}"))?;
    let target = outside.join("real.rs");
    fs::write(&target, "pub fn foreign() {}\n")
        .map_err(|err| format!("write outside target failed: {err}"))?;
    fs::remove_file(dir.join("src/lib.rs"))
        .map_err(|err| format!("remove tracked file failed: {err}"))?;
    std::os::unix::fs::symlink(&target, dir.join("src/lib.rs"))
        .map_err(|err| format!("create symlink failed: {err}"))?;

    let set = discover_unstaged(&dir, &repo_facts(&dir)?, &DiscoverOptions::default())?;
    match omitted_as(&set.omitted_files, "src/lib.rs") {
        Some(OmissionReason::NonRegularFile) => Ok(()),
        other => Err(format!(
            "symlinked path must be listed as non-regular (never followed), got {other:?}"
        )),
    }
}
