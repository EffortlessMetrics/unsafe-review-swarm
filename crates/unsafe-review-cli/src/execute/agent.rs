//! Machine agent protocol, first slice: the compact task index
//! (`agent tasks`, #2311 PR1).
//!
//! The index is assembled over the same local-scope analysis as `work`
//! (same change set, same bytes) and projects one bounded row per actionable
//! card with canonical identities and closed-vocabulary values. Read queries
//! never execute commands and never write: selection, filtering, and the cap
//! only omit rows. The selected packet behind each row's `packet_command`
//! and the result/recheck envelope arrive in PR2/PR3.

use crate::command::{AgentCommand, AgentTasksOptions, Format, ScopeSelect};
use unsafe_review_core::{
    AnalysisMode, AnalyzeInput, AnalyzeOutput, DiffSource, DiscoveryOptions, PolicyMode, Scope,
    TaskIndexOptions, TaskReadiness, TaskRole, analyze_with_discovery, assemble_task_index,
    render_task_index_human,
};

use super::work::local_scope_diff;

fn scope_name(scope: &ScopeSelect) -> &'static str {
    match scope {
        ScopeSelect::Staged => "staged",
        ScopeSelect::Unstaged => "unstaged",
        ScopeSelect::Worktree | ScopeSelect::CommitRange => "worktree",
    }
}

fn analyze_scope(
    root: &std::path::Path,
    scope: &ScopeSelect,
) -> Result<(AnalyzeOutput, unsafe_review_core::ChangeSet), String> {
    let (set, diff_text, _) = local_scope_diff(root, scope)?;
    let output = analyze_with_discovery(
        AnalyzeInput {
            root: root.to_path_buf(),
            scope: Scope::Diff,
            diff: DiffSource::Text(diff_text),
            mode: AnalysisMode::Draft,
            policy: PolicyMode::Advisory,
            include_unchanged_tests: true,
            max_cards: None,
        },
        DiscoveryOptions::default(),
    )?;
    Ok((output, set))
}

/// Rebuild the exact command that reproduces this index, so a consumer can
/// recheck the same scope, filters, and cap after editing.
fn recheck_command(options: &AgentTasksOptions) -> String {
    let mut command = format!(
        "unsafe-review agent tasks --scope {} --root {}",
        scope_name(&options.scope),
        options.root.display()
    );
    if let Some(role) = options.role.as_deref() {
        command.push_str(&format!(" --role {role}"));
    }
    if let Some(readiness) = options.readiness.as_deref() {
        command.push_str(&format!(" --readiness {readiness}"));
    }
    if options.human_only {
        command.push_str(" --human-only");
    }
    if options.changed_only {
        command.push_str(" --changed-only");
    }
    if options.max_tasks != unsafe_review_core::DEFAULT_MAX_TASKS {
        command.push_str(&format!(" --max-tasks {}", options.max_tasks));
    }
    if matches!(options.format, Format::Human) {
        command.push_str(" --format human");
    }
    command
}

fn index_options(options: &AgentTasksOptions) -> Result<TaskIndexOptions, String> {
    Ok(TaskIndexOptions {
        role: options.role.as_deref().map(TaskRole::parse).transpose()?,
        readiness: options
            .readiness
            .as_deref()
            .map(TaskReadiness::parse)
            .transpose()?,
        human_only: options.human_only,
        changed_only: options.changed_only,
        max_tasks: options.max_tasks,
    })
}

fn run_tasks(options: &AgentTasksOptions) -> Result<(), String> {
    let (output, set) = analyze_scope(&options.root, &options.scope)?;
    let packet_prefix = format!("unsafe-review context --root {}", options.root.display());
    let index = assemble_task_index(
        &output,
        &set.digest,
        &recheck_command(options),
        &packet_prefix,
        &index_options(options)?,
    );
    match options.format {
        Format::Json => {
            let rendered = serde_json::to_string_pretty(&index)
                .map_err(|err| format!("serialize task index failed: {err}"))?;
            println!("{rendered}");
        }
        Format::Human => {
            println!("{}", render_task_index_human(&index));
        }
        _ => {
            return Err(
                "unsupported agent format; agent tasks projects `human` and `json` only"
                    .to_string(),
            );
        }
    }
    Ok(())
}

pub(crate) fn run(command: &AgentCommand) -> Result<(), crate::RunFailure> {
    match command {
        AgentCommand::Tasks(options) => run_tasks(options).map_err(crate::RunFailure::Tool),
        AgentCommand::Help => {
            print_agent_help();
            Ok(())
        }
    }
}

pub(crate) fn print_agent_help() {
    println!("unsafe-review agent: stable machine task protocol for LLM consumers");
    println!();
    println!("Usage:");
    println!(
        "  unsafe-review agent tasks [--scope staged|unstaged|worktree] [--root .] \\\n         [--role production|test|generated|unknown] [--readiness ready|needs_human|requires_witness_receipt|unsupported] \\\n         [--human-only] [--changed-only] [--max-tasks <N>] [--format human|json]"
    );
    println!();
    println!("Read-only: one bounded row per actionable card with exact analysis,");
    println!("change-set, and subject identities plus the packet and recheck commands.");
    println!("Filters omit rows without reclassifying; the cap truncates visibly with");
    println!("an expansion path. No source excerpts, prompts, secrets, or execution.");
}
