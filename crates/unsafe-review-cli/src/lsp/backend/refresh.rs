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
        let generation = self.next_refresh_generation().await;
        let _guard = self.refresh_in_flight.lock().await;
        let root = self.root.lock().await.clone();
        let cfg = self.config.lock().await.clone();
        let Some(diff) = self.diff_source(&root, &cfg).await else {
            self.clear_stale_diagnostics().await;
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
                self.clear_stale_diagnostics().await;
                return;
            }
            Err(err) => {
                self.log_refresh_error("unsafe-review analysis task failed", &err.to_string())
                    .await;
                self.clear_stale_diagnostics().await;
                return;
            }
        };
        if !self.is_current_generation(generation).await {
            self.client
                .log_message(
                    MessageType::INFO,
                    format!("discarded stale unsafe-review refresh generation {generation}"),
                )
                .await;
            return;
        }
        let by_uri = diagnostics_by_uri(&root, &output);
        let (clear_uris, publish_batches) = self
            .install_refresh_result(output, by_uri, document_versions)
            .await;
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

    async fn next_refresh_generation(&self) -> u64 {
        let mut generation = self.refresh_generation.lock().await;
        *generation += 1;
        *generation
    }

    async fn is_current_generation(&self, generation: u64) -> bool {
        *self.refresh_generation.lock().await == generation
    }

    async fn install_refresh_result(
        &self,
        output: AnalyzeOutput,
        by_uri: BTreeMap<Uri, Vec<Diagnostic>>,
        versions: BTreeMap<Uri, i32>,
    ) -> (
        Vec<(Uri, Option<i32>)>,
        Vec<(Uri, Vec<Diagnostic>, Option<i32>)>,
    ) {
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
        *self.latest_analysis.lock().await = Some(output);
        *self.latest_diagnostics.lock().await = by_uri;
        (clear_uris, publish_batches)
    }

    async fn clear_stale_diagnostics(&self) {
        let clear_uris = {
            let mut previous = self.last_diagnostic_uris.lock().await;
            clear_uris_for_failure(&mut previous)
        };
        *self.latest_analysis.lock().await = None;
        self.latest_diagnostics.lock().await.clear();
        for uri in clear_uris {
            let version = self.document_version(&uri).await;
            self.client.publish_diagnostics(uri, vec![], version).await;
        }
    }

    pub(super) async fn mark_diagnostics_stale(&self) {
        self.next_refresh_generation().await;
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
