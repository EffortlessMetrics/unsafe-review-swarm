//! Read-only `scope` command: name the source state a review would analyze,
//! without running the analysis.
//!
//! Output is an identity record only: scope kind, pinning Git identities,
//! included/omitted files, completeness, and the subject digest. It carries
//! no safety, coverage, or cleanliness claim. The configuration envelope
//! projection belongs to the environment-identity slice (#2318).

use crate::command::{Format, ScopeOptions, ScopeSelect};
use unsafe_review_core::{
    ChangeSet, DiscoverOptions, discover_commit_range, discover_repo, discover_staged,
    discover_unstaged, discover_worktree, render_changeset_human,
};

pub(crate) fn run(options: &ScopeOptions) -> Result<(), String> {
    let repo = discover_repo(&options.root)?;
    let toplevel = repo.toplevel.clone();
    let discover = DiscoverOptions::default();
    let set: ChangeSet = match options.scope {
        ScopeSelect::Staged => discover_staged(&toplevel, &repo, &discover)?,
        ScopeSelect::Unstaged => discover_unstaged(&toplevel, &repo, &discover)?,
        ScopeSelect::Worktree => discover_worktree(&toplevel, &repo, &discover)?,
        ScopeSelect::CommitRange => {
            let base = options
                .base
                .as_deref()
                .ok_or_else(|| "commit range scope requires a base commit/ref".to_string())?;
            let head = options.head.as_deref().unwrap_or("HEAD");
            discover_commit_range(&toplevel, repo.shallow, base, head)?
        }
    };
    match options.format {
        Format::Human => {
            println!("{}", render_changeset_human(&set));
        }
        Format::Json => {
            let changeset = serde_json::to_value(&set)
                .map_err(|err| format!("serialize change set failed: {err}"))?;
            let combined = serde_json::json!({ "changeset": changeset });
            println!(
                "{}",
                serde_json::to_string_pretty(&combined)
                    .map_err(|err| format!("serialize scope output failed: {err}"))?
            );
        }
        _ => {
            return Err(
                "unsupported scope format; scope projects `human` and `json` only".to_string(),
            );
        }
    }
    Ok(())
}
