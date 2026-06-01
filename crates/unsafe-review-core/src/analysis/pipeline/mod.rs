mod card_builder;
mod identity;
mod next_action;
mod progress;
mod sources;
mod summary;

#[cfg(test)]
mod tests;

use super::{receipts, scanner};
use crate::api::{
    AnalysisMode, AnalyzeInput, AnalyzeOutput, DiscoveryOptions, RepoScanPhase, RepoScanStatus,
    Scope,
};
use crate::input::workspace;
use crate::policy::PolicyState;
use progress::{RepoProgressFn, emit_repo_status, repo_status};
use sources::{load_diff_index, package_name};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;
use summary::summarize;

pub(crate) fn analyze(input: AnalyzeInput) -> Result<AnalyzeOutput, String> {
    let discovery = default_discovery_for(&input);
    analyze_with_receipts(input, true, discovery, None)
}

pub(crate) fn analyze_with_discovery(
    input: AnalyzeInput,
    discovery: DiscoveryOptions,
) -> Result<AnalyzeOutput, String> {
    analyze_with_receipts(input, true, discovery, None)
}

pub(crate) fn analyze_with_discovery_and_progress<F>(
    input: AnalyzeInput,
    discovery: DiscoveryOptions,
    mut progress: F,
) -> Result<AnalyzeOutput, String>
where
    F: FnMut(&RepoScanStatus) -> Result<(), String>,
{
    analyze_with_receipts(input, true, discovery, Some(&mut progress))
}

pub(crate) fn analyze_without_receipts(input: AnalyzeInput) -> Result<AnalyzeOutput, String> {
    let discovery = default_discovery_for(&input);
    analyze_with_receipts(input, false, discovery, None)
}

fn default_discovery_for(input: &AnalyzeInput) -> DiscoveryOptions {
    if matches!(input.scope, Scope::Repo) || matches!(input.mode, AnalysisMode::Repo) {
        DiscoveryOptions::repo_defaults()
    } else {
        DiscoveryOptions::default()
    }
}

fn analyze_with_receipts(
    input: AnalyzeInput,
    import_receipts: bool,
    discovery: DiscoveryOptions,
    mut progress: Option<RepoProgressFn<'_>>,
) -> Result<AnalyzeOutput, String> {
    let started = Instant::now();
    let repo_mode = matches!(input.scope, Scope::Repo) || matches!(input.mode, AnalysisMode::Repo);
    let diff_index = load_diff_index(&input.diff)?;
    emit_repo_status(
        &mut progress,
        repo_status(RepoScanPhase::Discovering, &started, 0, 0, 0, None, false),
    )?;
    let mut discovered_files = 0usize;
    let all_rust_files = {
        let mut discovery_progress = |count: usize, path: &Path| {
            discovered_files = count;
            emit_repo_status(
                &mut progress,
                repo_status(
                    RepoScanPhase::Discovering,
                    &started,
                    discovered_files,
                    0,
                    0,
                    Some(path.to_path_buf()),
                    false,
                ),
            )
        };
        workspace::discover_rust_files_with_progress(
            &input.root,
            &discovery,
            Some(&mut discovery_progress),
        )?
    };
    discovered_files = all_rust_files.len();
    let package = package_name(&input.root);
    let policy_state = PolicyState::load(&input.root)?;
    let receipt_index = if import_receipts {
        receipts::ReceiptIndex::load(&input.root)?
    } else {
        receipts::ReceiptIndex::default()
    };
    let candidate_files = if repo_mode || diff_index.is_empty() {
        all_rust_files.clone()
    } else {
        all_rust_files
            .iter()
            .filter(|path| diff_index.contains_file(path))
            .cloned()
            .collect::<Vec<_>>()
    };

    let mut cards = Vec::new();
    let mut identity_counts = BTreeMap::new();
    let max_cards = input.max_cards.unwrap_or(usize::MAX);
    let mut files_scanned = 0usize;
    let mut last_scanned_path = None;
    emit_repo_status(
        &mut progress,
        repo_status(
            RepoScanPhase::Scanning,
            &started,
            discovered_files,
            files_scanned,
            cards.len(),
            None,
            false,
        ),
    )?;
    'files: for rel in &candidate_files {
        if cards.len() >= max_cards {
            break;
        }
        emit_repo_status(
            &mut progress,
            repo_status(
                RepoScanPhase::Scanning,
                &started,
                discovered_files,
                files_scanned,
                cards.len(),
                Some(rel.clone()),
                false,
            ),
        )?;
        let scanned = scanner::scan_file(&input.root, rel, Some(&diff_index), repo_mode)?;
        files_scanned += 1;
        let mut build_ctx = card_builder::CardBuildContext {
            root: &input.root,
            package: &package,
            receipt_index: &receipt_index,
            policy_state: &policy_state,
            identity_counts: &mut identity_counts,
        };
        let mut reached_max_cards = false;
        for scanned_site in scanned {
            cards.push(card_builder::build_card(&mut build_ctx, scanned_site));
            if cards.len() >= max_cards {
                reached_max_cards = true;
                break;
            }
        }
        emit_repo_status(
            &mut progress,
            repo_status(
                RepoScanPhase::Scanning,
                &started,
                discovered_files,
                files_scanned,
                cards.len(),
                Some(rel.clone()),
                false,
            ),
        )?;
        last_scanned_path = Some(rel.clone());
        if reached_max_cards {
            break 'files;
        }
    }
    cards.sort_by(|left, right| {
        left.site
            .location
            .file
            .cmp(&right.site.location.file)
            .then(left.site.location.line.cmp(&right.site.location.line))
    });
    let summary = summarize(all_rust_files.len(), candidate_files.len(), &cards);
    emit_repo_status(
        &mut progress,
        repo_status(
            RepoScanPhase::Complete,
            &started,
            discovered_files,
            files_scanned,
            cards.len(),
            last_scanned_path,
            true,
        ),
    )?;
    Ok(AnalyzeOutput {
        schema_version: "0.1".to_string(),
        tool: "unsafe-review".to_string(),
        root: input.root,
        scope: input.scope,
        mode: input.mode,
        policy: input.policy,
        summary,
        cards,
    })
}
