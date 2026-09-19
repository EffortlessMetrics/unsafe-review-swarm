//! Non-local change-set constructors: commit ranges, external diffs, repo
//! snapshots, and document overlays, plus the human/JSON projections shared
//! by every scope.

use super::changeset::{
    ChangeScopeKind, ChangeSet, ChangedFile, FileChangeKind, FileProvenance, OmissionReason,
    OmittedFile, ScopeCompleteness, ScopeIdentities, ScopeLimitation, ScopeLimitationKind,
};
use super::scope_git::{diff_name_status, git_output, is_rejected_path, rename_mapping};
use crate::sha256_hex_of;
use std::path::{Path, PathBuf};

/// Discover a commit-range scope between `base` and `head`.
///
/// Both endpoints must resolve (`git rev-parse --verify`); a shallow or
/// missing history fails closed rather than widening into a repo scan.
pub fn discover_commit_range(
    toplevel: &Path,
    shallow: bool,
    base: &str,
    head: &str,
) -> Result<ChangeSet, String> {
    let base_commit = git_output(
        toplevel,
        &["rev-parse", "--verify", base],
        &format!("resolve the range base `{base}`"),
    )?;
    let head_commit = git_output(
        toplevel,
        &["rev-parse", "--verify", head],
        &format!("resolve the range head `{head}`"),
    )?;
    let rows = diff_name_status(toplevel, false, &[&base_commit, &head_commit])?;
    let mut included = Vec::new();
    let mut renames = Vec::new();
    let mut omitted = Vec::new();
    for row in &rows {
        let (path, kind, rename) = rename_mapping(row);
        if is_rejected_path(&path) {
            omitted.push(OmittedFile {
                path,
                reason: OmissionReason::PathRejected,
            });
            continue;
        }
        if let Some(rename) = rename {
            renames.push(rename);
        }
        included.push(ChangedFile {
            path,
            kind,
            provenance: FileProvenance::NotApplicable,
        });
    }

    let mut limitations = Vec::new();
    if shallow {
        limitations.push(ScopeLimitation {
            kind: ScopeLimitationKind::HistoryUnavailable,
            detail: "the repository is shallow; the range covers only fetched history".to_string(),
        });
    }
    let completeness = if limitations.is_empty() {
        ScopeCompleteness::Complete
    } else {
        ScopeCompleteness::Partial { limitations }
    };

    let identities = ScopeIdentities {
        base_commit: Some(base_commit),
        head_commit: Some(head_commit),
        shallow,
        ..ScopeIdentities::default()
    };
    Ok(ChangeSet::finish(
        ChangeScopeKind::CommitRange,
        toplevel.to_path_buf(),
        identities,
        included,
        omitted,
        renames,
        completeness,
    ))
}

/// Build an external-diff change set from already-supplied diff bytes.
///
/// The caller parsed the bytes; this constructor binds the content digest and
/// carries the paths through the same inclusion/omission vocabulary. Rejected
/// paths are listed, never resolved.
pub fn changeset_from_external_diff(
    root: PathBuf,
    diff_bytes: &[u8],
    paths: Vec<PathBuf>,
) -> ChangeSet {
    let mut included = Vec::new();
    let mut omitted = Vec::new();
    for path in paths {
        if is_rejected_path(&path) {
            omitted.push(OmittedFile {
                path,
                reason: OmissionReason::PathRejected,
            });
            continue;
        }
        included.push(ChangedFile {
            path,
            kind: FileChangeKind::Modified,
            provenance: FileProvenance::NotApplicable,
        });
    }
    included.sort_by(|left, right| left.path.cmp(&right.path));
    let identities = ScopeIdentities {
        diff_digest: Some(format!("diff-sha256:{}", sha256_hex_of(diff_bytes))),
        ..ScopeIdentities::default()
    };
    ChangeSet::finish(
        ChangeScopeKind::ExternalDiff,
        root,
        identities,
        included,
        omitted,
        Vec::new(),
        ScopeCompleteness::Complete,
    )
}

/// Build a repo-snapshot change set for an explicitly supplied source digest.
///
/// The digest computation belongs to the caller (a later slice binds the
/// repo-scan path); this constructor only carries it under the snapshot scope
/// so snapshots and local scopes can never share a subject digest.
pub fn changeset_from_snapshot(root: PathBuf, source_digest: String) -> ChangeSet {
    let identities = ScopeIdentities {
        source_digest: Some(source_digest),
        ..ScopeIdentities::default()
    };
    ChangeSet::finish(
        ChangeScopeKind::RepoSnapshot,
        root,
        identities,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        ScopeCompleteness::Complete,
    )
}

/// Reserve the document-overlay identity contract for #2313.
///
/// An unsaved overlay is not a Git tree and cannot create durable
/// receipts/history without a saved/current source equivalence step, so this
/// scope carries the saved digest, document version, and overlay digest as
/// given, with no Git discovery.
pub fn changeset_from_overlay(
    root: PathBuf,
    saved_digest: String,
    document_version: i64,
    overlay_digest: String,
) -> ChangeSet {
    let identities = ScopeIdentities {
        saved_digest: Some(saved_digest),
        document_version: Some(document_version),
        overlay_digest: Some(overlay_digest),
        ..ScopeIdentities::default()
    };
    ChangeSet::finish(
        ChangeScopeKind::DocumentOverlay,
        root,
        identities,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        ScopeCompleteness::Complete,
    )
}

/// One-screen human summary: scope, identities, counts, omissions, and
/// completeness. Quiet scopes name what was covered, never imply cleanliness.
pub fn render_changeset_human(set: &ChangeSet) -> String {
    let mut out = String::new();
    out.push_str(&format!("scope: {}\n", set.scope.as_str()));
    out.push_str(&format!("digest: {}\n", set.digest));
    if let Some(base) = &set.identities.base_commit {
        out.push_str(&format!("base: {base}\n"));
    }
    if let Some(head) = &set.identities.head_commit {
        out.push_str(&format!("head: {head}\n"));
    }
    if let Some(state) = &set.identities.index_state {
        out.push_str(&format!("index: {state}\n"));
    }
    if let Some(digest) = &set.identities.worktree_digest {
        out.push_str(&format!("worktree: {digest}\n"));
    }
    if let Some(digest) = &set.identities.diff_digest {
        out.push_str(&format!("diff: {digest}\n"));
    }
    if set.identities.shallow {
        out.push_str("shallow: true\n");
    }
    out.push_str(&format!("included: {} file(s)\n", set.included_files.len()));
    for file in set.included_files.iter().take(20) {
        out.push_str(&format!(
            "  {} [{}:{}]\n",
            file.path.display(),
            file.kind.as_str(),
            file.provenance.as_str()
        ));
    }
    if set.included_files.len() > 20 {
        out.push_str(&format!("  ... ({} more)\n", set.included_files.len() - 20));
    }
    out.push_str(&format!("omitted: {} file(s)\n", set.omitted_files.len()));
    for file in set.omitted_files.iter().take(20) {
        out.push_str(&format!(
            "  {} [{}]\n",
            file.path.display(),
            file.reason.as_str()
        ));
    }
    if set.omitted_files.len() > 20 {
        out.push_str(&format!("  ... ({} more)\n", set.omitted_files.len() - 20));
    }
    if !set.renames.is_empty() {
        out.push_str(&format!("renames: {}\n", set.renames.len()));
        for rename in set.renames.iter().take(10) {
            out.push_str(&format!(
                "  {} -> {}\n",
                rename.from.display(),
                rename.to.display()
            ));
        }
        if set.renames.len() > 10 {
            out.push_str(&format!("  ... ({} more)\n", set.renames.len() - 10));
        }
    }
    out.push_str(&format!("completeness: {}\n", set.completeness.as_str()));
    if let ScopeCompleteness::Partial { limitations } = &set.completeness {
        for limitation in limitations {
            out.push_str(&format!(
                "  limitation [{}]: {}\n",
                limitation.kind.as_str(),
                limitation.detail
            ));
        }
    }
    out
}

/// Canonical JSON projection of the same [`ChangeSet`].
pub fn render_changeset_json(set: &ChangeSet) -> Result<String, String> {
    serde_json::to_string_pretty(set).map_err(|err| format!("serialize change set failed: {err}"))
}
