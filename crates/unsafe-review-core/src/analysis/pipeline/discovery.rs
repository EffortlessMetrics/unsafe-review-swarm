use super::progress::{RepoScanReporter, owned_path};
use crate::api::{AnalysisMode, AnalyzeInput, DiscoveryOptions, Scope};
use crate::input::{diff::DiffIndex, workspace};
use std::path::{Path, PathBuf};

pub(super) fn default_discovery_for(input: &AnalyzeInput) -> DiscoveryOptions {
    if matches!(input.scope, Scope::Repo) || matches!(input.mode, AnalysisMode::Repo) {
        DiscoveryOptions::repo_defaults()
    } else {
        DiscoveryOptions::default()
    }
}

pub(super) fn is_repo_mode(input: &AnalyzeInput) -> bool {
    matches!(input.scope, Scope::Repo) || matches!(input.mode, AnalysisMode::Repo)
}

pub(super) fn discover_rust_files(
    root: &Path,
    discovery: &DiscoveryOptions,
    reporter: &mut RepoScanReporter<'_>,
) -> Result<Vec<PathBuf>, String> {
    reporter.emit_discovering(0, None)?;
    let mut discovered_files = 0usize;
    let files = {
        let mut discovery_progress = |count: usize, path: &Path| {
            discovered_files = count;
            reporter.emit_discovering(discovered_files, Some(owned_path(path)))
        };
        workspace::discover_rust_files_with_progress(
            root,
            discovery,
            Some(&mut discovery_progress),
        )?
    };
    Ok(files)
}

pub(super) fn candidate_files(
    all_rust_files: &[PathBuf],
    diff_index: &DiffIndex,
    repo_mode: bool,
) -> Vec<PathBuf> {
    if repo_mode || diff_index.is_empty() {
        return all_rust_files.to_vec();
    }
    all_rust_files
        .iter()
        .filter(|path| diff_index.contains_file(path))
        .cloned()
        .collect()
}
