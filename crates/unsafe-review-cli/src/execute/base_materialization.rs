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

/// Resolve a base ref to a full commit SHA, or report [`BaseMaterializationError::UnknownSha`].
fn resolve_base_sha(anchor: &Path, base_ref: &str) -> Result<String, BaseMaterializationError> {
    let output = ProcessCommand::new("git")
        .arg("-C")
        .arg(anchor)
        .arg("rev-parse")
        .arg("--verify")
        .arg(format!("{base_ref}^{{commit}}"))
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
            detail: "git rev-parse returned empty output".to_string(),
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
    let sha = resolve_base_sha(root, base_ref)?;
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
        anchor: root.to_path_buf(),
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
