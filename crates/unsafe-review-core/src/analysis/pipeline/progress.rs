use crate::api::{RepoScanPhase, RepoScanStatus};
use std::path::PathBuf;
use std::time::Instant;

pub(super) type RepoProgressFn<'a> = &'a mut dyn FnMut(&RepoScanStatus) -> Result<(), String>;

pub(super) fn emit_repo_status(
    progress: &mut Option<RepoProgressFn<'_>>,
    status: RepoScanStatus,
) -> Result<(), String> {
    if let Some(progress) = progress.as_deref_mut() {
        progress(&status)?;
    }
    Ok(())
}

pub(super) fn repo_status(
    phase: RepoScanPhase,
    started: &Instant,
    files_discovered: usize,
    files_scanned: usize,
    cards_found: usize,
    last_path: Option<PathBuf>,
    completed: bool,
) -> RepoScanStatus {
    RepoScanStatus {
        schema_version: "repo-scan-status/v1".to_string(),
        phase,
        elapsed_ms: started.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
        files_discovered,
        files_scanned,
        cards_found,
        last_path,
        completed,
    }
}
