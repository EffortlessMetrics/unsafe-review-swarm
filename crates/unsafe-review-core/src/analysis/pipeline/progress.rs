use crate::api::{RepoScanPhase, RepoScanStatus};
use std::path::{Path, PathBuf};
use std::time::Instant;

pub(super) type RepoProgressFn<'a> = &'a mut dyn FnMut(&RepoScanStatus) -> Result<(), String>;

pub(super) struct RepoScanReporter<'a> {
    started: Instant,
    progress: Option<RepoProgressFn<'a>>,
}

impl<'a> RepoScanReporter<'a> {
    pub(super) fn new(progress: Option<RepoProgressFn<'a>>) -> Self {
        Self {
            started: Instant::now(),
            progress,
        }
    }

    pub(super) fn emit_discovering(
        &mut self,
        files_discovered: usize,
        last_path: Option<PathBuf>,
    ) -> Result<(), String> {
        self.emit(
            RepoScanPhase::Discovering,
            files_discovered,
            0,
            0,
            last_path,
            false,
        )
    }

    pub(super) fn emit_scanning(
        &mut self,
        files_discovered: usize,
        files_scanned: usize,
        cards_found: usize,
        last_path: Option<PathBuf>,
    ) -> Result<(), String> {
        self.emit(
            RepoScanPhase::Scanning,
            files_discovered,
            files_scanned,
            cards_found,
            last_path,
            false,
        )
    }

    pub(super) fn emit_complete(
        &mut self,
        files_discovered: usize,
        files_scanned: usize,
        cards_found: usize,
        last_path: Option<PathBuf>,
    ) -> Result<(), String> {
        self.emit(
            RepoScanPhase::Complete,
            files_discovered,
            files_scanned,
            cards_found,
            last_path,
            true,
        )
    }

    fn emit(
        &mut self,
        phase: RepoScanPhase,
        files_discovered: usize,
        files_scanned: usize,
        cards_found: usize,
        last_path: Option<PathBuf>,
        completed: bool,
    ) -> Result<(), String> {
        let status = RepoScanStatus {
            schema_version: "repo-scan-status/v1".to_string(),
            phase,
            elapsed_ms: self
                .started
                .elapsed()
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX),
            files_discovered,
            files_scanned,
            cards_found,
            last_path,
            completed,
        };
        if let Some(progress) = self.progress.as_deref_mut() {
            progress(&status)?;
        }
        Ok(())
    }
}

pub(super) fn owned_path(path: &Path) -> PathBuf {
    path.to_path_buf()
}
