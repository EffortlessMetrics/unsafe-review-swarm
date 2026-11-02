//! Base-commit materialization for the first-pr base/head movement section.
//!
//! The movement evaluator needs full-fidelity [`AnalyzeOutput`]s on both
//! sides, so the base commit is materialized into an isolated temp worktree
//! (never the caller's checkout) and analyzed at repo scope. The worktree is
//! Drop-cleaned: removing the registration first, then the temp directory, so
//! no temp tree escapes the command run. Failures are fail-soft taxonomy, not
//! panics: the caller renders them as an unknown movement section while the
//! head output stays intact.

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::sync::atomic::{AtomicU64, Ordering};

/// Fail-soft taxonomy for base materialization. Every leg names the condition
/// so the movement section can report an honest unknown with the cause.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum BaseMaterializationError {
    /// `root` is not inside a git work tree, so no base ref can resolve.
    NotARepo { root: PathBuf, detail: String },
    /// The base ref does not resolve to a commit in this repository.
    UnknownSha { base_ref: String, detail: String },
    /// The ref resolved but `git worktree add` failed (locked worktree,
    /// unusable temp path, or another git-level refusal).
    WorktreeFailure { sha: String, detail: String },
}

impl fmt::Display for BaseMaterializationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotARepo { root, detail } => write!(
                f,
                "base cannot be materialized: {} is not inside a git repository ({detail})",
                root.display()
            ),
            Self::UnknownSha { base_ref, detail } => write!(
                f,
                "base ref '{base_ref}' could not be resolved to a commit ({detail})"
            ),
            Self::WorktreeFailure { sha, detail } => {
                write!(f, "base worktree creation failed for {sha} ({detail})")
            }
        }
    }
}

/// An isolated temp checkout of one base commit, registered as a git
/// worktree. Drop removes the worktree registration first and then the temp
/// directory, best effort: cleanup must never fail the advisory run it
/// serves.
pub(crate) struct MaterializedBase {
    worktree: PathBuf,
    anchor: PathBuf,
    owner_tmp: PathBuf,
}

impl MaterializedBase {
    /// The materialized base tree root: pass this as the analysis `--root`.
    pub(crate) fn path(&self) -> &Path {
        &self.worktree
    }

    /// The analysis root corresponding to the requested `--root`: the
    /// requested root relative to the work-tree toplevel, resolved under
    /// this worktree. A requested root at the toplevel maps to the worktree
    /// itself; a nested `--root` maps to the matching subtree so the base
    /// census covers exactly the tree the head side analyzed. A requested
    /// root outside the repository tree cannot be mapped and reports
    /// [`BaseMaterializationError::NotARepo`] with the cause, so the caller
    /// degrades to an honest unknown section instead of analyzing an
    /// unrelated directory.
    pub(crate) fn analysis_root_for(
        &self,
        root: &Path,
    ) -> Result<PathBuf, BaseMaterializationError> {
        let rel = relative_to_toplevel(&self.anchor, root)?;
        Ok(self.worktree.join(rel))
    }
}

/// Express `root` relative to the work-tree `toplevel`. Both sides are
/// canonicalized first so verbatim-prefix (`\\?\`) and separator
/// differences on Windows cannot fake an outside-the-tree mismatch.
fn relative_to_toplevel(toplevel: &Path, root: &Path) -> Result<PathBuf, BaseMaterializationError> {
    let canonical_toplevel =
        std::fs::canonicalize(toplevel).unwrap_or_else(|_| toplevel.to_path_buf());
    let canonical_root =
        std::fs::canonicalize(root).map_err(|err| BaseMaterializationError::NotARepo {
            root: root.to_path_buf(),
            detail: format!("could not resolve the requested root: {err}"),
        })?;
    canonical_root
        .strip_prefix(&canonical_toplevel)
        .map(Path::to_path_buf)
        .map_err(|err| BaseMaterializationError::NotARepo {
            root: root.to_path_buf(),
            detail: format!(
                "requested root is outside the repository tree rooted at {} ({err})",
                toplevel.display()
            ),
        })
}

impl Drop for MaterializedBase {
    fn drop(&mut self) {
        let _ = ProcessCommand::new("git")
            .arg("-C")
            .arg(&self.anchor)
            .arg("worktree")
            .arg("remove")
            .arg("--force")
            .arg(&self.worktree)
            .output();
        let _ = std::fs::remove_dir_all(&self.owner_tmp);
    }
}

/// Resolve `root` to its work-tree toplevel, or report [`BaseMaterializationError::NotARepo`].
fn repo_toplevel(root: &Path) -> Result<PathBuf, BaseMaterializationError> {
    let output = ProcessCommand::new("git")
        .arg("-C")
        .arg(root)
        .arg("rev-parse")
        .arg("--show-toplevel")
        .output()
        .map_err(|err| BaseMaterializationError::NotARepo {
            root: root.to_path_buf(),
            detail: format!("could not run git: {err}"),
        })?;
    if !output.status.success() {
        return Err(BaseMaterializationError::NotARepo {
            root: root.to_path_buf(),
            detail: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    let toplevel = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if toplevel.is_empty() {
        return Err(BaseMaterializationError::NotARepo {
            root: root.to_path_buf(),
            detail: "git rev-parse --show-toplevel returned empty output".to_string(),
        });
    }
    Ok(PathBuf::from(toplevel))
}

/// Resolve a base ref to the merge-base commit SHA with HEAD, or report
/// [`BaseMaterializationError::UnknownSha`].
///
/// The head side of this surface is diff-scoped through `git diff
/// <base>...HEAD`, whose three-dot range is anchored at the merge base, not
/// the base tip. Materializing the tip on a diverged base would compare the
/// head against a tree the diff never covered; resolving the merge base
/// keeps both sides anchored at the same commit. The original ref string is
/// kept in the error taxonomy for provenance; it is never silently replaced.
fn resolve_base_sha(anchor: &Path, base_ref: &str) -> Result<String, BaseMaterializationError> {
    let output = ProcessCommand::new("git")
        .arg("-C")
        .arg(anchor)
        .arg("merge-base")
        .arg(base_ref)
        .arg("HEAD")
        .output()
        .map_err(|err| BaseMaterializationError::UnknownSha {
            base_ref: base_ref.to_string(),
            detail: format!("could not run git: {err}"),
        })?;
    if !output.status.success() {
        return Err(BaseMaterializationError::UnknownSha {
            base_ref: base_ref.to_string(),
            detail: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if sha.is_empty() {
        return Err(BaseMaterializationError::UnknownSha {
            base_ref: base_ref.to_string(),
            detail: "git merge-base returned empty output".to_string(),
        });
    }
    Ok(sha)
}

static MATERIALIZATION_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Register `sha` as a detached worktree at `worktree_path`.
fn add_worktree(
    anchor: &Path,
    sha: &str,
    worktree_path: &Path,
) -> Result<(), BaseMaterializationError> {
    let output = ProcessCommand::new("git")
        .arg("-C")
        .arg(anchor)
        .arg("worktree")
        .arg("add")
        .arg("--detach")
        .arg(worktree_path)
        .arg(sha)
        .output()
        .map_err(|err| BaseMaterializationError::WorktreeFailure {
            sha: sha.to_string(),
            detail: format!("could not run git: {err}"),
        })?;
    if !output.status.success() {
        return Err(BaseMaterializationError::WorktreeFailure {
            sha: sha.to_string(),
            detail: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(())
}

/// Materialize `base_ref` into an isolated temp worktree.
///
/// The returned [`MaterializedBase`] owns the temp tree; dropping it removes
/// the worktree registration and the temp directory.
pub(crate) fn materialize_base(
    root: &Path,
    base_ref: &str,
) -> Result<MaterializedBase, BaseMaterializationError> {
    let toplevel = repo_toplevel(root)?;
    let sha = resolve_base_sha(&toplevel, base_ref)?;
    let owner_tmp = std::env::temp_dir().join(format!(
        "unsafe-review-base-{}-{}",
        std::process::id(),
        MATERIALIZATION_COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    if let Err(err) = std::fs::create_dir_all(&owner_tmp) {
        return Err(BaseMaterializationError::WorktreeFailure {
            sha,
            detail: format!("could not create temp dir {}: {err}", owner_tmp.display()),
        });
    }
    let worktree = owner_tmp.join("worktree");
    if let Err(err) = add_worktree(&toplevel, &sha, &worktree) {
        let _ = std::fs::remove_dir_all(&owner_tmp);
        return Err(err);
    }
    Ok(MaterializedBase {
        worktree,
        anchor: toplevel,
        owner_tmp,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestTemp {
        path: PathBuf,
    }

    impl TestTemp {
        fn new(name: &str) -> Result<Self, String> {
            let path = std::env::temp_dir().join(format!(
                "unsafe-review-base-mat-test-{}-{}-{}",
                std::process::id(),
                MATERIALIZATION_COUNTER.fetch_add(1, Ordering::SeqCst),
                name
            ));
            if path.exists() {
                std::fs::remove_dir_all(&path)
                    .map_err(|err| format!("remove stale {} failed: {err}", path.display()))?;
            }
            std::fs::create_dir_all(&path)
                .map_err(|err| format!("create {} failed: {err}", path.display()))?;
            Ok(Self { path })
        }
    }

    impl Drop for TestTemp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    fn git(repo: &Path, args: &[&str]) -> Result<String, String> {
        let output = ProcessCommand::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .map_err(|err| format!("could not run git: {err}"))?;
        if !output.status.success() {
            return Err(format!(
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn init_repo_with_two_commits(dir: &Path) -> Result<(String, String), String> {
        git(dir, &["init", "-q"])?;
        git(dir, &["config", "user.name", "base mat test"])?;
        git(dir, &["config", "user.email", "base-mat@example.test"])?;
        git(dir, &["config", "commit.gpgsign", "false"])?;
        git(dir, &["config", "core.autocrlf", "false"])?;
        std::fs::write(dir.join("lib.rs"), "pub fn base_only() {}\n")
            .map_err(|err| format!("write base lib.rs failed: {err}"))?;
        git(dir, &["add", "-A"])?;
        git(
            dir,
            &[
                "-c",
                "user.name=base mat test",
                "-c",
                "user.email=base-mat@example.test",
                "commit",
                "-qm",
                "base commit",
            ],
        )?;
        let base = git(dir, &["rev-parse", "HEAD"])?;
        std::fs::write(
            dir.join("lib.rs"),
            "pub fn base_only() {}\npub fn head_only() {}\n",
        )
        .map_err(|err| format!("write head lib.rs failed: {err}"))?;
        git(dir, &["add", "-A"])?;
        git(
            dir,
            &[
                "-c",
                "user.name=base mat test",
                "-c",
                "user.email=base-mat@example.test",
                "commit",
                "-qm",
                "head commit",
            ],
        )?;
        let head = git(dir, &["rev-parse", "HEAD"])?;
        Ok((base, head))
    }

    #[test]
    fn materialized_tree_matches_requested_commit() -> Result<(), String> {
        let temp = TestTemp::new("match")?;
        let (base, head) = init_repo_with_two_commits(&temp.path)?;
        if base == head {
            return Err("fixture setup produced identical base and head".to_string());
        }
        let materialized = materialize_base(&temp.path, &base)
            .map_err(|err| format!("materialize base failed: {err}"))?;
        let content = std::fs::read_to_string(materialized.path().join("lib.rs"))
            .map_err(|err| format!("read materialized lib.rs failed: {err}"))?;
        if content != "pub fn base_only() {}\n" {
            return Err(format!(
                "materialized tree must match the base commit, got: {content:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn diverged_branch_materializes_merge_base_not_tip() -> Result<(), String> {
        let temp = TestTemp::new("diverged")?;
        let (base, head) = init_repo_with_two_commits(&temp.path)?;
        if base == head {
            return Err("fixture setup produced identical base and head".to_string());
        }
        // Fork a side branch off the base commit and advance it with a
        // contract change the three-dot head diff never covers.
        let previous = git(&temp.path, &["branch", "--show-current"])?;
        let previous = previous.trim().to_string();
        if previous.is_empty() {
            return Err("fixture repo must be on a branch".to_string());
        }
        git(&temp.path, &["checkout", "-qb", "side", &base])?;
        std::fs::write(
            temp.path.join("lib.rs"),
            "pub fn base_only() {}\n// Safety: base-side contract note.\n",
        )
        .map_err(|err| format!("write side lib.rs failed: {err}"))?;
        git(&temp.path, &["add", "-A"])?;
        git(&temp.path, &["commit", "-qm", "side contract change"])?;
        let tip = git(&temp.path, &["rev-parse", "HEAD"])?;
        git(&temp.path, &["checkout", "-q", &previous])?;
        let at = git(&temp.path, &["rev-parse", "HEAD"])?;
        if at.trim() != head {
            return Err("fixture must return to the head commit".to_string());
        }
        if tip.trim() == base {
            return Err("side branch must advance past the base".to_string());
        }
        let merge_base = git(&temp.path, &["merge-base", "side", "HEAD"])?;
        if merge_base.trim() != base {
            return Err("fixture merge base must be the base commit".to_string());
        }
        let materialized = materialize_base(&temp.path, "side")
            .map_err(|err| format!("materialize diverged ref failed: {err}"))?;
        let content = std::fs::read_to_string(materialized.path().join("lib.rs"))
            .map_err(|err| format!("read materialized lib.rs failed: {err}"))?;
        if content != "pub fn base_only() {}\n" {
            return Err(format!(
                "materialized tree must match the merge base, not the diverged tip; got: {content:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn nested_root_maps_to_matching_subtree() -> Result<(), String> {
        let temp = TestTemp::new("nested-root")?;
        let (base, _) = init_repo_with_two_commits(&temp.path)?;
        let materialized = materialize_base(&temp.path, &base)
            .map_err(|err| format!("materialize base failed: {err}"))?;
        let top = materialized
            .analysis_root_for(&temp.path)
            .map_err(|err| format!("toplevel mapping failed: {err}"))?;
        if top != materialized.path() {
            return Err(format!(
                "the toplevel must map to the worktree itself, got {}",
                top.display()
            ));
        }
        let nested = temp.path.join("nested");
        std::fs::create_dir_all(&nested)
            .map_err(|err| format!("create nested dir failed: {err}"))?;
        let mapped = materialized
            .analysis_root_for(&nested)
            .map_err(|err| format!("nested mapping failed: {err}"))?;
        if mapped != materialized.path().join("nested") {
            return Err(format!(
                "a nested root must map to the matching subtree, got {}",
                mapped.display()
            ));
        }
        Ok(())
    }

    #[test]
    fn root_outside_tree_is_not_mapped() -> Result<(), String> {
        let temp = TestTemp::new("outside-root")?;
        let (base, _) = init_repo_with_two_commits(&temp.path)?;
        let materialized = materialize_base(&temp.path, &base)
            .map_err(|err| format!("materialize base failed: {err}"))?;
        let outside = TestTemp::new("outside-tree")?;
        match materialized.analysis_root_for(&outside.path) {
            Err(BaseMaterializationError::NotARepo { root, detail }) => {
                if root != outside.path {
                    return Err(format!(
                        "outside-tree error must name the requested root, got {}",
                        root.display()
                    ));
                }
                if detail.is_empty() {
                    return Err("outside-tree error must carry a cause".to_string());
                }
                Ok(())
            }
            Err(other) => Err(format!("expected NotARepo, got {other}")),
            Ok(mapped) => Err(format!(
                "a root outside the tree must not map, got {}",
                mapped.display()
            )),
        }
    }

    #[test]
    fn drop_removes_temp_tree_and_worktree_registration() -> Result<(), String> {
        let temp = TestTemp::new("cleanup")?;
        let (base, _) = init_repo_with_two_commits(&temp.path)?;
        let worktree_path: PathBuf;
        let owner_tmp: PathBuf;
        {
            let materialized = materialize_base(&temp.path, &base)
                .map_err(|err| format!("materialize base failed: {err}"))?;
            worktree_path = materialized.path().to_path_buf();
            owner_tmp = worktree_path
                .parent()
                .ok_or_else(|| "materialized path must have a temp parent".to_string())?
                .to_path_buf();
            if !worktree_path.join("lib.rs").is_file() {
                return Err("worktree must contain the checkout before drop".to_string());
            }
            let listed = git(&temp.path, &["worktree", "list", "--porcelain"])?;
            let listed_normalized = listed.replace('\\', "/");
            let wanted = worktree_path.to_string_lossy().replace('\\', "/");
            if !listed_normalized.contains(wanted.as_str()) {
                return Err(format!(
                    "worktree must be registered before drop: {listed:?}"
                ));
            }
        }
        if owner_tmp.exists() {
            return Err(format!(
                "drop must remove the temp tree at {}",
                owner_tmp.display()
            ));
        }
        let listed = git(&temp.path, &["worktree", "list", "--porcelain"])?;
        let listed_normalized = listed.replace('\\', "/");
        let wanted = owner_tmp.to_string_lossy().replace('\\', "/");
        if listed_normalized.contains(wanted.as_str()) {
            return Err(format!("drop must unregister the worktree: {listed:?}"));
        }
        Ok(())
    }

    #[test]
    fn outside_a_repo_reports_not_a_repo() -> Result<(), String> {
        let temp = TestTemp::new("not-a-repo")?;
        match materialize_base(&temp.path, "HEAD") {
            Err(BaseMaterializationError::NotARepo { root, detail }) => {
                if root != temp.path {
                    return Err(format!(
                        "NotARepo must name the requested root, got {}",
                        root.display()
                    ));
                }
                if detail.is_empty() {
                    return Err("NotARepo must carry git detail".to_string());
                }
                Ok(())
            }
            Err(other) => Err(format!("expected NotARepo, got {other}")),
            Ok(_) => Err("materialization outside a repo must fail".to_string()),
        }
    }

    #[test]
    fn unknown_ref_reports_unknown_sha() -> Result<(), String> {
        let temp = TestTemp::new("unknown-sha")?;
        init_repo_with_two_commits(&temp.path)?;
        match materialize_base(&temp.path, "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef") {
            Err(BaseMaterializationError::UnknownSha { base_ref, detail }) => {
                if !base_ref.contains("deadbeef") {
                    return Err(format!(
                        "UnknownSha must name the requested ref, got {base_ref:?}"
                    ));
                }
                if detail.is_empty() {
                    return Err("UnknownSha must carry git detail".to_string());
                }
                Ok(())
            }
            Err(other) => Err(format!("expected UnknownSha, got {other}")),
            Ok(_) => Err("materialization of an unknown sha must fail".to_string()),
        }
    }

    #[test]
    fn blocked_worktree_path_reports_worktree_failure() -> Result<(), String> {
        let temp = TestTemp::new("worktree-failure")?;
        let (base, _) = init_repo_with_two_commits(&temp.path)?;
        let blocked = temp.path.join("blocked");
        std::fs::write(&blocked, "a regular file blocks worktree creation")
            .map_err(|err| format!("write blocker file failed: {err}"))?;
        match add_worktree(&temp.path, &base, &blocked) {
            Err(BaseMaterializationError::WorktreeFailure { sha, detail }) => {
                if sha != base {
                    return Err("WorktreeFailure must name the requested sha".to_string());
                }
                if detail.is_empty() {
                    return Err("WorktreeFailure must carry git detail".to_string());
                }
                Ok(())
            }
            Err(other) => Err(format!("expected WorktreeFailure, got {other}")),
            Ok(()) => Err("worktree add onto a blocked path must fail".to_string()),
        }
    }
}
