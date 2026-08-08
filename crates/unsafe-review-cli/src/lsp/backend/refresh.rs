use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use tower_lsp_server::ls_types::{Diagnostic, MessageType, Uri};
use unsafe_review_core::{
    AnalysisMode, AnalyzeInput, AnalyzeOutput, DiffSource, PolicyMode, Scope, analyze,
};

use crate::lsp::config::LspConfig;
use crate::lsp::diagnostics::diagnostics_by_uri;
use crate::lsp::state::clear_uris_for_failure;

use super::Backend;

impl Backend {
    pub(super) async fn refresh(&self) {
        let Ok(_guard) = self.refresh_in_flight.try_lock() else {
            *self.refresh_pending.lock().await = true;
            return;
        };

        loop {
            self.refresh_once().await;
            let rerun = {
                let mut pending = self.refresh_pending.lock().await;
                let rerun = *pending;
                *pending = false;
                rerun
            };
            if !rerun {
                break;
            }
        }
    }

    async fn refresh_once(&self) {
        let generation = self.begin_refresh().await;
        let root = self.root.lock().await.clone();
        let cfg = self.config.lock().await.clone();
        let Some(diff) = self.diff_source(&root, &cfg).await else {
            self.mark_diagnostics_failed("unsafe-review could not determine a diff source")
                .await;
            return;
        };
        let document_versions = self.document_versions().await;
        let input = AnalyzeInput {
            root: root.clone(),
            scope: if cfg.mode == "diff" {
                Scope::Diff
            } else {
                Scope::Repo
            },
            diff,
            mode: if cfg.mode == "diff" {
                AnalysisMode::Draft
            } else {
                AnalysisMode::Repo
            },
            policy: PolicyMode::Advisory,
            include_unchanged_tests: true,
            max_cards: cfg.max_cards,
        };
        let analyzed = tokio::task::spawn_blocking(move || analyze(input)).await;
        let output = match analyzed {
            Ok(Ok(output)) => output,
            Ok(Err(err)) => {
                self.log_refresh_error("unsafe-review analysis failed", &err.to_string())
                    .await;
                self.mark_diagnostics_failed("unsafe-review analysis failed")
                    .await;
                return;
            }
            Err(err) => {
                self.log_refresh_error("unsafe-review analysis task failed", &err.to_string())
                    .await;
                self.mark_diagnostics_failed("unsafe-review analysis task failed")
                    .await;
                return;
            }
        };
        let partial_notice = output.summary.capped_scan_notice();
        let by_uri = diagnostics_by_uri(&root, &output);
        let Some((clear_uris, publish_batches)) = self
            .install_refresh_result(output, by_uri, document_versions, generation)
            .await
        else {
            self.client
                .log_message(
                    MessageType::INFO,
                    format!("discarded stale unsafe-review refresh generation {generation}"),
                )
                .await;
            return;
        };
        if let Some(notice) = partial_notice {
            self.client
                .log_message(
                    MessageType::WARNING,
                    format!("unsafe-review: {notice} Live diagnostics are partial."),
                )
                .await;
        }
        for (uri, version) in clear_uris {
            self.client.publish_diagnostics(uri, vec![], version).await;
        }
        for (uri, diagnostics, version) in publish_batches {
            self.client
                .publish_diagnostics(uri, diagnostics, version)
                .await;
        }
    }

    async fn diff_source(&self, root: &Path, cfg: &LspConfig) -> Option<DiffSource> {
        if cfg.mode != "diff" {
            return Some(DiffSource::NoneRepoScan);
        }
        let Some(base) = cfg.base.as_ref() else {
            return Some(DiffSource::NoneRepoScan);
        };
        match std::process::Command::new("git")
            .arg("diff")
            .arg(format!("{base}...HEAD"))
            .current_dir(root)
            .output()
        {
            Ok(out) if out.status.success() => Some(DiffSource::Text(
                String::from_utf8_lossy(&out.stdout).into_owned(),
            )),
            Ok(out) => {
                self.client
                    .log_message(
                        MessageType::WARNING,
                        format!(
                            "unsafe-review git diff failed for base `{base}`: {}",
                            String::from_utf8_lossy(&out.stderr).trim()
                        ),
                    )
                    .await;
                None
            }
            Err(err) => {
                self.client
                    .log_message(
                        MessageType::WARNING,
                        format!("unsafe-review could not run git diff for base `{base}`: {err}"),
                    )
                    .await;
                None
            }
        }
    }

    async fn begin_refresh(&self) -> u64 {
        let mut generation = self.refresh_generation.lock().await;
        *generation += 1;
        self.live_snapshot.lock().await.current = false;
        *generation
    }

    async fn install_refresh_result(
        &self,
        output: AnalyzeOutput,
        by_uri: BTreeMap<Uri, Vec<Diagnostic>>,
        versions: BTreeMap<Uri, i32>,
        generation: u64,
    ) -> Option<(
        Vec<(Uri, Option<i32>)>,
        Vec<(Uri, Vec<Diagnostic>, Option<i32>)>,
    )> {
        let current_generation = self.refresh_generation.lock().await;
        if *current_generation != generation {
            return None;
        }
        let current: BTreeSet<_> = by_uri.keys().cloned().collect();
        let clear_uris = {
            let mut previous = self.last_diagnostic_uris.lock().await;
            let clear_uris = previous
                .difference(&current)
                .cloned()
                .map(|uri| {
                    let version = versions.get(&uri).copied();
                    (uri, version)
                })
                .collect::<Vec<_>>();
            *previous = current;
            clear_uris
        };
        let publish_batches = by_uri
            .iter()
            .map(|(uri, diagnostics)| {
                (uri.clone(), diagnostics.clone(), versions.get(uri).copied())
            })
            .collect::<Vec<_>>();
        let mut snapshot = self.live_snapshot.lock().await;
        snapshot.analysis = Some(output);
        snapshot.diagnostics = by_uri;
        snapshot.current = true;
        Some((clear_uris, publish_batches))
    }

    async fn clear_stale_diagnostics(&self) {
        let clear_uris = {
            let mut previous = self.last_diagnostic_uris.lock().await;
            clear_uris_for_failure(&mut previous)
        };
        {
            let mut snapshot = self.live_snapshot.lock().await;
            snapshot.analysis = None;
            snapshot.diagnostics.clear();
            snapshot.current = false;
        }
        for uri in clear_uris {
            let version = self.document_version(&uri).await;
            self.client.publish_diagnostics(uri, vec![], version).await;
        }
    }

    /// Surface a failed refresh to the editor without pretending the file is
    /// clean. A failed analysis must never look identical to a successful
    /// analysis that found zero cards, so this deliberately does NOT touch
    /// `latest_analysis`, `latest_diagnostics`, or any published diagnostics —
    /// the last successful result (if any) stays visible. `context` is a
    /// freshness signal only: it must never claim the file is safe, proven, or
    /// UB-free, and it must never claim the (possibly absent) diagnostics are
    /// current.
    pub(super) async fn mark_diagnostics_failed(&self, context: &str) {
        self.live_snapshot.lock().await.current = false;
        self.client
            .show_message(
                MessageType::WARNING,
                format!(
                    "unsafe-review: {context}. Diagnostics shown (if any) are from the \
                     last successful analysis and are not current; an empty or unchanged \
                     result does not mean this file is safe or clean."
                ),
            )
            .await;
    }

    pub(super) async fn mark_diagnostics_stale(&self) {
        self.begin_refresh().await;
        self.clear_stale_diagnostics().await;
        self.client
            .log_message(
                MessageType::INFO,
                "unsafe-review diagnostics marked stale after document change",
            )
            .await;
    }

    async fn document_versions(&self) -> BTreeMap<Uri, i32> {
        self.documents
            .lock()
            .await
            .docs
            .iter()
            .map(|(uri, document)| (uri.clone(), document.version))
            .collect()
    }

    async fn document_version(&self, uri: &Uri) -> Option<i32> {
        self.documents.lock().await.version(uri)
    }

    async fn log_refresh_error(&self, context: &str, detail: &str) {
        self.client
            .log_message(MessageType::ERROR, format!("{context}: {detail}"))
            .await;
    }
}
