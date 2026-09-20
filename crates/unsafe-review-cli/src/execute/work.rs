//! Local authoring `work` command: review staged, unstaged, or combined
//! worktree changes without shell-created diff files (#2310 PR1).
//!
//! The command discovers the canonical [`ChangeSet`](unsafe_review_core::ChangeSet)
//! for the selected scope, derives the matching `git diff` text, and runs it
//! through the same analysis and renderers as `check` (same cards, same
//! movement, same quiet wording: a quiet result names the scope, never
//! "safe"). A scope header headlines every human run with the exact
//! base/index/worktree identity, included/omitted files, and copyable next
//! commands; JSON output is the same `check` projection so both formats
//! identify the same tasks and limitations.
//!
//! Read-only: no `git add`, commit, stash, reset, source edit, witness
//! execution, or hook installation. Staged review analyzes worktree bytes,
//! so files that are both staged and unstaged are named loudly instead of
//! passing as staged-only content.

use crate::command::{CheckOptions, EnvFeatureSelect, Format, ScopeSelect, WorkOptions};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use unsafe_review_core::{
    AnalysisMode, AnalyzeOutput, ChangeSet, DiffSource, DiscoverOptions, DiscoveryOptions,
    PolicyMode, Scope, discover_repo, discover_staged, discover_unstaged, discover_worktree,
};

use super::{CheckFrame, run_check_with_diff};

/// Maximum files named inline in the scope header before collapsing to a
/// count; the full lists stay in `scope --format json` and the analysis
/// output's unresolved/rejected sets.
const HEADER_PATH_LIMIT: usize = 10;

fn scope_flag(scope: &ScopeSelect) -> &'static str {
    match scope {
        ScopeSelect::Staged => "--staged",
        ScopeSelect::Unstaged => "--unstaged",
        ScopeSelect::Worktree => "--worktree",
        ScopeSelect::CommitRange => "--worktree",
    }
}

fn scope_label(scope: &ScopeSelect) -> &'static str {
    match scope {
        ScopeSelect::Staged => "staged changes (HEAD -> index)",
        ScopeSelect::Unstaged => "unstaged changes (index -> worktree)",
        ScopeSelect::Worktree => "all local changes (HEAD -> worktree)",
        ScopeSelect::CommitRange => "all local changes (HEAD -> worktree)",
    }
}

/// Extra `git diff` arguments selecting exactly the named scope's bytes.
fn scope_diff_args(scope: &ScopeSelect) -> Vec<&'static str> {
    match scope {
        ScopeSelect::Staged => vec!["--cached"],
        ScopeSelect::Unstaged => Vec::new(),
        ScopeSelect::Worktree | ScopeSelect::CommitRange => vec!["HEAD"],
    }
}

fn run_git(toplevel: &Path, args: &[&str]) -> Result<String, String> {
    let output = ProcessCommand::new("git")
        .arg("-C")
        .arg(toplevel)
        .arg("diff")
        .arg("--no-color")
        .arg("--no-ext-diff")
        .args(args)
        .output()
        .map_err(|err| format!("failed to run git diff: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "git diff failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn run_git_names(toplevel: &Path, args: &[&str]) -> Result<BTreeSet<String>, String> {
    let mut command_args = vec!["--name-only"];
    command_args.extend(args);
    Ok(run_git(toplevel, &command_args)?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.trim().to_string())
        .collect())
}

/// Short display for a digest identity: strip the `kind-sha256:` prefix and
/// keep 12 hex chars. Full values stay in `scope --format json`.
fn short_id(value: &str) -> String {
    let hex = value.split(':').next_back().unwrap_or(value);
    if hex.len() > 12 {
        format!("{}...", &hex[..12])
    } else {
        hex.to_string()
    }
}

fn short_sha(value: &str) -> &str {
    if value.len() > 12 {
        &value[..12]
    } else {
        value
    }
}

fn render_paths(label: &str, paths: &[PathBuf], out: &mut String) {
    if paths.is_empty() {
        return;
    }
    if paths.len() <= HEADER_PATH_LIMIT {
        let names: Vec<String> = paths
            .iter()
            .map(|path| path.display().to_string())
            .collect();
        out.push_str(&format!("{}: {}\n", label, names.join(", ")));
    } else {
        out.push_str(&format!("{}: {} files\n", label, paths.len()));
    }
}

fn build_header(options: &WorkOptions, set: &ChangeSet, mixed: &[String]) -> String {
    let identities = &set.identities;
    let mut out = String::new();
    out.push_str(&format!(
        "unsafe-review work: {}\n",
        scope_label(&options.scope)
    ));
    let mut identity = String::from("scope: ");
    identity.push_str(match options.scope {
        ScopeSelect::Staged => "staged",
        ScopeSelect::Unstaged => "unstaged",
        _ => "worktree",
    });
    if let Some(head) = identities.head_commit.as_deref() {
        identity.push_str(&format!(", HEAD {}", short_sha(head)));
    }
    if let Some(index) = identities.index_state.as_deref() {
        identity.push_str(&format!(", index {}", short_id(index)));
    }
    if let Some(worktree) = identities.worktree_digest.as_deref() {
        identity.push_str(&format!(", worktree {}", short_id(worktree)));
    }
    identity.push_str(&format!(", changeset {}", short_id(&set.digest)));
    out.push_str(&identity);
    out.push('\n');
    let included: Vec<PathBuf> = set
        .included_files
        .iter()
        .map(|file| file.path.clone())
        .collect();
    render_paths("included", &included, &mut out);
    let omitted: Vec<PathBuf> = set
        .omitted_files
        .iter()
        .map(|file| file.path.clone())
        .collect();
    render_paths("omitted (not analyzed)", &omitted, &mut out);
    if !mixed.is_empty() {
        let (subject, verb) = if mixed.len() == 1 {
            ("this file", "has")
        } else {
            ("these files", "have")
        };
        out.push_str(&format!(
            "warning: {subject} also {verb} unstaged edits; worktree bytes are analyzed, not staged-only content: {}\n",
            mixed.join(", ")
        ));
    }
    out.push('\n');
    out
}

fn build_footer(scope: &ScopeSelect, root_display: &str, output: &AnalyzeOutput) -> String {
    let mut out = String::new();
    out.push_str("Next:\n");
    out.push_str(&format!(
        "- recheck this scope: unsafe-review work {} --root {}\n",
        scope_flag(scope),
        root_display
    ));
    for card in output.cards.iter().take(3) {
        out.push_str(&format!(
            "- explain {}: unsafe-review explain --root {} {}\n",
            card.id, root_display, card.id
        ));
    }
    out.push_str(&format!(
        "- scope identity: unsafe-review scope --root {} {}\n",
        root_display,
        scope_flag(scope)
    ));
    out
}

/// Discover the canonical change set and matching `git diff` text for a
/// local scope. Shared by `work` (human front door) and `agent tasks`
/// (machine index) so both analyze exactly the same bytes.
pub(super) fn local_scope_diff(
    root: &Path,
    scope: &ScopeSelect,
) -> Result<(ChangeSet, String, PathBuf), String> {
    let repo = discover_repo(root)?;
    let toplevel = repo.toplevel.clone();
    let discover = DiscoverOptions::default();
    let set: ChangeSet = match scope {
        ScopeSelect::Staged => discover_staged(&toplevel, &repo, &discover)?,
        ScopeSelect::Unstaged => discover_unstaged(&toplevel, &repo, &discover)?,
        ScopeSelect::Worktree | ScopeSelect::CommitRange => {
            discover_worktree(&toplevel, &repo, &discover)?
        }
    };
    let diff_text = run_git(&toplevel, &scope_diff_args(scope))?;
    Ok((set, diff_text, toplevel))
}

pub(crate) fn run(options: &WorkOptions) -> Result<(), crate::RunFailure> {
    let tool = crate::RunFailure::Tool;
    let (set, diff_text, toplevel) =
        local_scope_diff(&options.root, &options.scope).map_err(tool)?;
    // Staged review analyzes worktree bytes: a file edited after `git add`
    // would otherwise pass as staged-only content. Name every mixed file
    // loudly in the scope header instead.
    let mixed: Vec<String> = if matches!(options.scope, ScopeSelect::Staged) {
        let staged = run_git_names(&toplevel, &["--cached"]).map_err(tool)?;
        let unstaged = run_git_names(&toplevel, &[]).map_err(tool)?;
        staged.intersection(&unstaged).cloned().collect()
    } else {
        Vec::new()
    };
    let check = CheckOptions {
        root: options.root.clone(),
        base: None,
        diff: None,
        format: options.format.clone(),
        policy: PolicyMode::Advisory,
        out: None,
        max_cards: options.max_cards,
        short: options.short,
        latency_out: None,
        env_features: EnvFeatureSelect::Default,
        target: None,
        aperture: false,
        impact: false,
        stages: false,
    };
    let root_display = options.root.display().to_string();
    let scope = options.scope.clone();
    let header = build_header(options, &set, &mixed);
    let footer = move |output: &AnalyzeOutput| build_footer(&scope, &root_display, output);
    let frame = CheckFrame {
        command_name: "work",
        header: matches!(options.format, Format::Human).then(|| header),
        footer: matches!(options.format, Format::Human).then(|| {
            let boxed: super::CheckFooter = Box::new(footer);
            boxed
        }),
    };
    run_check_with_diff(
        check,
        Scope::Diff,
        AnalysisMode::Draft,
        DiscoveryOptions::default(),
        DiffSource::Text(diff_text),
        frame,
    )
}
