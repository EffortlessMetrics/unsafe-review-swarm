//! Local-scope change-set constructors: staged, unstaged, and worktree.
//!
//! Inclusion follows `git diff --cached` (staged) and `git diff` (worktree vs
//! index, unstaged) exactly, so unstaged edits never leak into a staged
//! result and already-staged hunks never reappear as new unstaged work. The
//! worktree scope unions both with per-file provenance instead of duplicate
//! entries.

use super::changeset::{
    ChangeScopeKind, ChangeSet, ChangedFile, FileChangeKind, FileProvenance, OmissionReason,
    OmittedFile, ScopeCompleteness, ScopeIdentities, ScopeLimitation, ScopeLimitationKind,
};
use super::scope_git::{
    DiscoverOptions, RepoFacts, apply_cap_note, diff_name_status, digest_worktree_state,
    discover_index_state, effective_path, is_rejected_path, is_rust_path, rejected_omissions,
    rename_mapping, untracked_files, verify_stable,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Discover the staged scope: exactly what would be committed from the index.
///
/// Unstaged edits to a staged file do not leak in: inclusion follows
/// `git diff --cached`. Remaining unstaged changes and untracked files are
/// reported as limitations and omissions, not treated as part of the result.
pub fn discover_staged(
    toplevel: &Path,
    repo: &RepoFacts,
    options: &DiscoverOptions,
) -> Result<ChangeSet, String> {
    let index_state = discover_index_state(toplevel)?;
    let rows = diff_name_status(toplevel, true, &[])?;
    let unstaged_rows = diff_name_status(toplevel, false, &[])?;
    let untracked = untracked_files(toplevel)?;

    let mut included = Vec::new();
    let mut renames = Vec::new();
    for row in &rows {
        let (path, kind, rename) = rename_mapping(row);
        if is_rejected_path(&path) {
            continue;
        }
        if let Some(rename) = rename {
            renames.push(rename);
        }
        included.push(ChangedFile {
            path,
            kind,
            provenance: FileProvenance::StagedOnly,
        });
    }
    let (included, dropped) = apply_cap_note(included, options.file_cap);

    let mut omitted = rejected_omissions(&rows);
    for path in untracked.iter().filter(|path| is_rust_path(path)) {
        omitted.push(OmittedFile {
            path: path.clone(),
            reason: OmissionReason::UntrackedExcludedByScope,
        });
    }
    let dropped_count = dropped.len();
    for file in dropped {
        omitted.push(OmittedFile {
            path: file.path,
            reason: OmissionReason::CapTruncated,
        });
    }

    let mut limitations = Vec::new();
    if !unstaged_rows.is_empty() {
        limitations.push(ScopeLimitation {
            kind: ScopeLimitationKind::OtherLocalStatePresent,
            detail: format!(
                "{} file(s) have additional unstaged changes not part of this staged result",
                unstaged_rows.len()
            ),
        });
    }
    if repo.head_commit.is_none() {
        limitations.push(ScopeLimitation {
            kind: ScopeLimitationKind::UnbornHead,
            detail: "HEAD is unborn; the staged scope is pinned by index tree only".to_string(),
        });
    }
    if dropped_count > 0 {
        limitations.push(ScopeLimitation {
            kind: ScopeLimitationKind::CapReached,
            detail: format!(
                "file listing stopped at the configured cap ({dropped_count} file(s) omitted)"
            ),
        });
    }
    let completeness = if limitations.is_empty() {
        ScopeCompleteness::Complete
    } else {
        ScopeCompleteness::Partial { limitations }
    };

    let identities = ScopeIdentities {
        head_commit: repo.head_commit.clone(),
        index_state: Some(index_state.clone()),
        shallow: repo.shallow,
        ..ScopeIdentities::default()
    };
    // The repository must not have moved under the discovery: re-verify HEAD
    // and index state before publishing the identity.
    verify_stable(toplevel, repo.head_commit.as_deref(), &index_state)?;
    Ok(ChangeSet::finish(
        ChangeScopeKind::Staged,
        toplevel.to_path_buf(),
        identities,
        included,
        omitted,
        renames,
        completeness,
    ))
}

/// Discover the unstaged scope: worktree changes relative to the index.
///
/// Already-staged hunks are not new unstaged work: inclusion follows
/// `git diff` (worktree vs index), which excludes them by construction.
/// Staged changes and untracked files are reported, not included.
pub fn discover_unstaged(
    toplevel: &Path,
    repo: &RepoFacts,
    options: &DiscoverOptions,
) -> Result<ChangeSet, String> {
    let index_state = discover_index_state(toplevel)?;
    let rows = diff_name_status(toplevel, false, &[])?;
    let staged_rows = diff_name_status(toplevel, true, &[])?;
    let untracked = untracked_files(toplevel)?;

    let mut unreadable = Vec::new();
    let mut paths = BTreeSet::new();
    let mut included = Vec::new();
    let mut renames = Vec::new();
    for row in &rows {
        let (path, kind, rename) = rename_mapping(row);
        if is_rejected_path(&path) {
            continue;
        }
        if let Some(rename) = rename {
            renames.push(rename);
        }
        if row.status != 'D' {
            paths.insert(path.clone());
        }
        included.push(ChangedFile {
            path,
            kind,
            provenance: FileProvenance::UnstagedOnly,
        });
    }
    let (included, dropped) = apply_cap_note(included, options.file_cap);
    let dropped_count = dropped.len();
    let mut non_regular = Vec::new();
    let worktree_digest =
        digest_worktree_state(toplevel, &paths, &[], &mut unreadable, &mut non_regular)?;

    let mut omitted = rejected_omissions(&rows);
    for path in untracked.iter().filter(|path| is_rust_path(path)) {
        omitted.push(OmittedFile {
            path: path.clone(),
            reason: OmissionReason::UntrackedExcludedByScope,
        });
    }
    for path in unreadable {
        omitted.push(OmittedFile {
            path,
            reason: OmissionReason::Unreadable,
        });
    }
    for path in non_regular {
        omitted.push(OmittedFile {
            path,
            reason: OmissionReason::NonRegularFile,
        });
    }
    for file in dropped {
        omitted.push(OmittedFile {
            path: file.path,
            reason: OmissionReason::CapTruncated,
        });
    }

    let mut limitations = Vec::new();
    if !staged_rows.is_empty() {
        limitations.push(ScopeLimitation {
            kind: ScopeLimitationKind::OtherLocalStatePresent,
            detail: format!(
                "{} staged file(s) exist alongside this unstaged result and are not included",
                staged_rows.len()
            ),
        });
    }
    if dropped_count > 0 {
        limitations.push(ScopeLimitation {
            kind: ScopeLimitationKind::CapReached,
            detail: format!(
                "file listing stopped at the configured cap ({dropped_count} file(s) omitted)"
            ),
        });
    }
    let completeness = if limitations.is_empty() {
        ScopeCompleteness::Complete
    } else {
        ScopeCompleteness::Partial { limitations }
    };

    let identities = ScopeIdentities {
        index_state: Some(index_state.clone()),
        worktree_digest: Some(worktree_digest),
        shallow: repo.shallow,
        ..ScopeIdentities::default()
    };
    verify_stable(toplevel, repo.head_commit.as_deref(), &index_state)?;
    Ok(ChangeSet::finish(
        ChangeScopeKind::Unstaged,
        toplevel.to_path_buf(),
        identities,
        included,
        omitted,
        renames,
        completeness,
    ))
}

/// Discover the combined worktree scope: staged plus unstaged relative to HEAD.
///
/// Per-file provenance retains which state(s) each file was seen in, so one
/// final operation never becomes duplicate entries. Untracked Rust files are
/// included when `options.include_untracked` is set, otherwise listed as
/// omitted with an explicit reason.
pub fn discover_worktree(
    toplevel: &Path,
    repo: &RepoFacts,
    options: &DiscoverOptions,
) -> Result<ChangeSet, String> {
    let index_state = discover_index_state(toplevel)?;
    let staged_rows = diff_name_status(toplevel, true, &[])?;
    let unstaged_rows = diff_name_status(toplevel, false, &[])?;
    // Net change kinds come from a direct worktree-vs-HEAD comparison: a file
    // staged as modified and then deleted is finally deleted (not modified),
    // and a file staged as deleted and then recreated carries its recreated
    // content (not a silent deletion). Staged/unstaged membership only sets
    // provenance, never the kind.
    let net_rows = diff_name_status(toplevel, false, &["HEAD"])?;
    let untracked = untracked_files(toplevel)?;

    let staged_paths: BTreeSet<PathBuf> = staged_rows
        .iter()
        .map(|row| effective_path(row).clone())
        .filter(|path| !is_rejected_path(path))
        .collect();
    let unstaged_paths: BTreeSet<PathBuf> = unstaged_rows
        .iter()
        .map(|row| effective_path(row).clone())
        .filter(|path| !is_rejected_path(path))
        .collect();

    let mut combined: BTreeMap<PathBuf, (FileChangeKind, FileProvenance)> = BTreeMap::new();
    let mut renames = Vec::new();
    for row in &net_rows {
        let (path, kind, rename) = rename_mapping(row);
        if is_rejected_path(&path) {
            continue;
        }
        if let Some(rename) = rename {
            renames.push(rename);
        }
        let provenance = match (staged_paths.contains(&path), unstaged_paths.contains(&path)) {
            (true, true) => FileProvenance::StagedAndUnstaged,
            (true, false) => FileProvenance::StagedOnly,
            (false, _) => FileProvenance::UnstagedOnly,
        };
        combined.insert(path, (kind, provenance));
    }
    let mut included: Vec<ChangedFile> = combined
        .into_iter()
        .map(|(path, (kind, provenance))| ChangedFile {
            path,
            kind,
            provenance,
        })
        .collect();

    let mut omitted: Vec<OmittedFile> = staged_rows
        .iter()
        .chain(unstaged_rows.iter())
        .chain(net_rows.iter())
        .filter(|row| is_rejected_path(effective_path(row)))
        .map(|row| OmittedFile {
            path: effective_path(row).clone(),
            reason: OmissionReason::PathRejected,
        })
        .collect();
    // Deduplicate: a hostile path can appear in several row sets.
    omitted.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.reason.as_str().cmp(right.reason.as_str()))
    });
    omitted.dedup_by(|next, current| next.path == current.path && next.reason == current.reason);
    let mut untracked_included: Vec<PathBuf> = Vec::new();
    for path in untracked.iter().filter(|path| is_rust_path(path)) {
        if options.include_untracked {
            // A path the net diff already covers must not become a duplicate
            // entry, with one exception: delete+recreate. The net diff
            // reports Deleted for a path deleted from the index even when the
            // worktree holds new bytes there (the bytes surface as untracked).
            // The bytes win: upgrade to Modified so the digest covers the
            // actual final state instead of a silent deletion.
            if let Some(existing) = included.iter_mut().find(|file| file.path == *path) {
                if existing.kind == FileChangeKind::Deleted {
                    existing.kind = FileChangeKind::Modified;
                }
                continue;
            }
            untracked_included.push(path.clone());
            included.push(ChangedFile {
                path: path.clone(),
                kind: FileChangeKind::Added,
                provenance: FileProvenance::UnstagedOnly,
            });
        } else {
            omitted.push(OmittedFile {
                path: path.clone(),
                reason: OmissionReason::UntrackedExcludedByScope,
            });
        }
    }
    included.sort_by(|left, right| left.path.cmp(&right.path));
    let (included, dropped) = apply_cap_note(included, options.file_cap);
    let dropped_count = dropped.len();

    // Deleted files have no worktree bytes: filter them out instead of
    // reporting deletions as unreadable.
    let content_paths: BTreeSet<PathBuf> = included
        .iter()
        .filter(|file| file.kind != FileChangeKind::Deleted)
        .map(|file| file.path.clone())
        .collect();
    let mut unreadable = Vec::new();
    let mut non_regular = Vec::new();
    let worktree_digest = digest_worktree_state(
        toplevel,
        &content_paths,
        &untracked_included,
        &mut unreadable,
        &mut non_regular,
    )?;
    for path in unreadable {
        omitted.push(OmittedFile {
            path,
            reason: OmissionReason::Unreadable,
        });
    }
    for path in non_regular {
        omitted.push(OmittedFile {
            path,
            reason: OmissionReason::NonRegularFile,
        });
    }
    for file in dropped {
        omitted.push(OmittedFile {
            path: file.path,
            reason: OmissionReason::CapTruncated,
        });
    }

    let mut limitations = Vec::new();
    if repo.head_commit.is_none() {
        limitations.push(ScopeLimitation {
            kind: ScopeLimitationKind::UnbornHead,
            detail:
                "HEAD is unborn; the worktree scope is pinned by index and worktree digests only"
                    .to_string(),
        });
    }
    if dropped_count > 0 {
        limitations.push(ScopeLimitation {
            kind: ScopeLimitationKind::CapReached,
            detail: format!(
                "file listing stopped at the configured cap ({dropped_count} file(s) omitted)"
            ),
        });
    }
    let completeness = if limitations.is_empty() {
        ScopeCompleteness::Complete
    } else {
        ScopeCompleteness::Partial { limitations }
    };

    let identities = ScopeIdentities {
        head_commit: repo.head_commit.clone(),
        index_state: Some(index_state.clone()),
        worktree_digest: Some(worktree_digest),
        shallow: repo.shallow,
        ..ScopeIdentities::default()
    };
    verify_stable(toplevel, repo.head_commit.as_deref(), &index_state)?;
    Ok(ChangeSet::finish(
        ChangeScopeKind::Worktree,
        toplevel.to_path_buf(),
        identities,
        included,
        omitted,
        renames,
        completeness,
    ))
}
