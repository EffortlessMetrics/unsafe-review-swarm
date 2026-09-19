//! Canonical change-set identity for shift-left review (issue #2308).
//!
//! A [`ChangeSet`] names exactly which source state was analyzed: the scope
//! kind (staged, unstaged, worktree, commit range, external diff, document
//! overlay, or repo snapshot), the Git identities that pin it (base/head
//! commits, index tree, worktree digest), the included and omitted files, and
//! the completeness of the discovery. It is an identity and provenance record
//! only: it carries no safety, coverage, or correctness claim about the
//! analyzed source, and a quiet result names its scope rather than implying
//! the source is clean.
//!
//! All Git access here is read-only with respect to user state. Discovery
//! runs `git rev-parse`, `git diff --name-status`, and `git ls-files`. The
//! index identity is a digest over `ls-files --stage` records, which observes
//! the index without writing objects, refs, or worktree bytes. No `add`,
//! `reset`, `stash`, `checkout`, `commit`, `write-tree`, or hook installation
//! is performed.

use crate::sha256_hex_of;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The authoring state a change set was derived from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeScopeKind {
    CommitRange,
    Staged,
    Unstaged,
    Worktree,
    ExternalDiff,
    DocumentOverlay,
    RepoSnapshot,
}

impl ChangeScopeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CommitRange => "commit_range",
            Self::Staged => "staged",
            Self::Unstaged => "unstaged",
            Self::Worktree => "worktree",
            Self::ExternalDiff => "external_diff",
            Self::DocumentOverlay => "document_overlay",
            Self::RepoSnapshot => "repo_snapshot",
        }
    }
}

/// How a single file participates in the analyzed state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed { from: PathBuf },
    TypeChanged,
}

impl FileChangeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Modified => "modified",
            Self::Deleted => "deleted",
            Self::Renamed { .. } => "renamed",
            Self::TypeChanged => "type_changed",
        }
    }
}

/// Which local state(s) a worktree-scope file was seen in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileProvenance {
    /// Present only in the index image (staged, worktree matches index).
    StagedOnly,
    /// Present only as an unstaged worktree edit.
    UnstagedOnly,
    /// Staged and then edited again: both states are reported.
    StagedAndUnstaged,
    /// Seen in a commit-range or external diff where staged/unstaged has no meaning.
    NotApplicable,
}

impl FileProvenance {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::StagedOnly => "staged_only",
            Self::UnstagedOnly => "unstaged_only",
            Self::StagedAndUnstaged => "staged_and_unstaged",
            Self::NotApplicable => "not_applicable",
        }
    }
}

/// A file included in the analyzed state, repo-relative.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangedFile {
    pub path: PathBuf,
    pub kind: FileChangeKind,
    pub provenance: FileProvenance,
}

/// Why a file the discovery saw is not part of the analyzed state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OmissionReason {
    /// An untracked file excluded because the selected scope does not include
    /// untracked files. Never silent: the file is listed here.
    UntrackedExcludedByScope,
    /// A diff path that cannot resolve under the root (traversal, absolute,
    /// or symlink-escaping): never resolved by design.
    PathRejected,
    /// The file could not be read (permissions, vanishing mid-scan, invalid UTF-8).
    Unreadable,
    /// A submodule, symlink, or non-regular file: content identity is out of scope.
    NonRegularFile,
    /// Discovery stopped listing files after a configured cap.
    CapTruncated,
}

impl OmissionReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UntrackedExcludedByScope => "untracked_excluded_by_scope",
            Self::PathRejected => "path_rejected",
            Self::Unreadable => "unreadable",
            Self::NonRegularFile => "non_regular_file",
            Self::CapTruncated => "cap_truncated",
        }
    }
}

/// A file seen by discovery but excluded from the analyzed state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OmittedFile {
    pub path: PathBuf,
    pub reason: OmissionReason,
}

/// A rename carried separately so movement keeps the old-to-new identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileRename {
    pub from: PathBuf,
    pub to: PathBuf,
}

/// Why a change set is partial rather than complete.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeLimitationKind {
    /// Additional unstaged changes exist alongside the staged scope (or vice
    /// versa): reported, not treated as part of the result.
    OtherLocalStatePresent,
    /// The repository is shallow or the base is missing: no silent widening
    /// into a repo scan happened.
    HistoryUnavailable,
    /// A diff path was rejected (traversal/absolute/symlink escape).
    PathRejection,
    /// An input file could not be read.
    UnreadableInput,
    /// File listing stopped at a configured cap.
    CapReached,
    /// HEAD is unborn (no commits yet): commit-pinned scopes are unavailable.
    UnbornHead,
}

impl ScopeLimitationKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OtherLocalStatePresent => "other_local_state_present",
            Self::HistoryUnavailable => "history_unavailable",
            Self::PathRejection => "path_rejection",
            Self::UnreadableInput => "unreadable_input",
            Self::CapReached => "cap_reached",
            Self::UnbornHead => "unborn_head",
        }
    }
}

/// One machine-readable completeness limitation with human detail.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeLimitation {
    pub kind: ScopeLimitationKind,
    pub detail: String,
}

/// Whether discovery covered the full requested scope.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeCompleteness {
    Complete,
    Partial { limitations: Vec<ScopeLimitation> },
}

impl ScopeCompleteness {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial { .. } => "partial",
        }
    }

    pub fn is_complete(&self) -> bool {
        matches!(self, Self::Complete)
    }
}

/// Git identities pinning the analyzed state. Fields that do not apply to a
/// scope are `None`; no identity is ever assumed or defaulted.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeIdentities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_commit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head_commit: Option<String>,
    /// Non-mutating identity of the index state (digest over `ls-files
    /// --stage` records), for staged-bearing scopes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index_state: Option<String>,
    /// Digest of worktree bytes that differ from the index, plus untracked
    /// content included by the scope. Files identical to the index are covered
    /// by `index_tree` and re-hashed only when they differ.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree_digest: Option<String>,
    /// Digest of externally supplied diff bytes, for `ExternalDiff`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saved_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_version: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overlay_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_digest: Option<String>,
    /// Whether the repository is shallow (affects history-bearing scopes).
    pub shallow: bool,
}

/// One canonical change-set identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeSet {
    pub scope: ChangeScopeKind,
    /// Repository toplevel the discovery ran in (absolute, local use only).
    /// Never serialized: portable JSON must not embed the checkout location.
    /// Relative file lists are interpreted against the invoking context, and
    /// the subject digest provably excludes this field. Deserializing
    /// portable JSON yields an empty root: re-anchor before local use.
    #[serde(skip_serializing, default)]
    pub root: PathBuf,
    pub identities: ScopeIdentities,
    pub included_files: Vec<ChangedFile>,
    pub omitted_files: Vec<OmittedFile>,
    pub renames: Vec<FileRename>,
    pub completeness: ScopeCompleteness,
    /// Content digest over the canonical encoding below. Two change sets with
    /// equal digests name the same analyzed state; a scope change alters it.
    pub digest: String,
}

impl ChangeSet {
    pub fn included_count(&self) -> usize {
        self.included_files.len()
    }

    pub fn omitted_count(&self) -> usize {
        self.omitted_files.len()
    }

    /// Canonical encoding covered by [`ChangeSet::digest`]. Sorted record
    /// lists keep the digest stable across discovery order.
    ///
    /// Every field is length-framed (`<len>:<bytes>\n`) and paths are encoded
    /// as raw bytes, so no two distinct change sets share an encoding: bare
    /// `:`/`\n` delimiters would let a hostile path mimic record boundaries,
    /// and `Path::display()` would lossily collapse non-UTF-8 paths.
    fn canonical_encoding(&self) -> Vec<u8> {
        fn push_field(out: &mut Vec<u8>, bytes: &[u8]) {
            out.extend_from_slice(bytes.len().to_string().as_bytes());
            out.push(b':');
            out.extend_from_slice(bytes);
            out.push(b'\n');
        }
        fn push_path(out: &mut Vec<u8>, path: &Path) {
            push_field(out, path.as_os_str().as_encoded_bytes());
        }
        let mut out = Vec::new();
        push_field(&mut out, self.scope.as_str().as_bytes());
        for field in [
            &self.identities.base_commit,
            &self.identities.head_commit,
            &self.identities.index_state,
            &self.identities.worktree_digest,
            &self.identities.diff_digest,
            &self.identities.saved_digest,
            &self.identities.overlay_digest,
            &self.identities.source_digest,
        ] {
            push_field(&mut out, field.as_deref().unwrap_or("-").as_bytes());
        }
        push_field(
            &mut out,
            self.identities
                .document_version
                .map(|version| version.to_string())
                .unwrap_or_default()
                .as_bytes(),
        );
        let mut included: Vec<&ChangedFile> = self.included_files.iter().collect();
        included.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then(left.kind.as_str().cmp(right.kind.as_str()))
                .then(left.provenance.as_str().cmp(right.provenance.as_str()))
        });
        push_field(&mut out, included.len().to_string().as_bytes());
        for file in included {
            push_path(&mut out, &file.path);
            push_field(&mut out, file.kind.as_str().as_bytes());
            push_field(&mut out, file.provenance.as_str().as_bytes());
        }
        let mut omitted: Vec<&OmittedFile> = self.omitted_files.iter().collect();
        omitted.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then(left.reason.as_str().cmp(right.reason.as_str()))
        });
        push_field(&mut out, omitted.len().to_string().as_bytes());
        for file in omitted {
            push_path(&mut out, &file.path);
            push_field(&mut out, file.reason.as_str().as_bytes());
        }
        let mut renames: Vec<&FileRename> = self.renames.iter().collect();
        renames.sort_by(|left, right| left.from.cmp(&right.from).then(left.to.cmp(&right.to)));
        push_field(&mut out, renames.len().to_string().as_bytes());
        for rename in renames {
            push_path(&mut out, &rename.from);
            push_path(&mut out, &rename.to);
        }
        push_field(&mut out, self.completeness.as_str().as_bytes());
        out
    }

    pub(crate) fn finish(
        scope: ChangeScopeKind,
        root: PathBuf,
        identities: ScopeIdentities,
        included_files: Vec<ChangedFile>,
        omitted_files: Vec<OmittedFile>,
        renames: Vec<FileRename>,
        completeness: ScopeCompleteness,
    ) -> Self {
        let mut set = Self {
            scope,
            root,
            identities,
            included_files,
            omitted_files,
            renames,
            completeness,
            digest: String::new(),
        };
        let encoding = set.canonical_encoding();
        set.digest = format!("changeset-sha256:{}", sha256_hex_of(&encoding));
        set
    }
}
