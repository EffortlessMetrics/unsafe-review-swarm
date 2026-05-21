use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use tokio::sync::Mutex;
use tower_lsp_server::jsonrpc::Result as LspResult;
use tower_lsp_server::ls_types::{
    CodeActionOrCommand, CodeActionParams, Diagnostic, DidChangeTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, ExecuteCommandParams, Hover, HoverParams,
    InitializeParams, InitializeResult, InitializedParams, MessageType,
    TextDocumentContentChangeEvent, Uri,
};
use tower_lsp_server::{Client, LanguageServer};
use unsafe_review_core::{
    AnalysisMode, AnalyzeInput, AnalyzeOutput, DiffSource, PolicyMode, Scope, analyze,
};

use super::TRUST_BOUNDARY;
use super::actions::{code_actions_for, execute_card_command};
use super::capabilities::{root_from_initialize_params, server_capabilities};
use super::config::{LspConfig, parse_config, should_refresh_on_change};
use super::diagnostics::diagnostics_by_uri;
use super::hover::hover_for;
use super::state::{DocumentStore, clear_uris_for_failure};
use super::{CMD_OPEN_TEST, CMD_PACKET, CMD_REFRESH, CMD_WITNESS_COMMAND, CMD_WITNESS_ROUTE};

pub(super) struct Backend {
    client: Client,
    root: Mutex<PathBuf>,
    config: Mutex<LspConfig>,
    documents: Mutex<DocumentStore>,
    latest_analysis: Mutex<Option<AnalyzeOutput>>,
    latest_diagnostics: Mutex<BTreeMap<Uri, Vec<Diagnostic>>>,
    last_diagnostic_uris: Mutex<BTreeSet<Uri>>,
    refresh_generation: Mutex<u64>,
    refresh_in_flight: Mutex<()>,
}

impl Backend {
    pub(super) fn new(client: Client) -> Self {
        Self {
            client,
            root: Mutex::new(PathBuf::from(".")),
            config: Mutex::new(LspConfig::default()),
            documents: Mutex::new(DocumentStore::default()),
            latest_analysis: Mutex::new(None),
            latest_diagnostics: Mutex::new(BTreeMap::new()),
            last_diagnostic_uris: Mutex::new(BTreeSet::new()),
            refresh_generation: Mutex::new(0),
            refresh_in_flight: Mutex::new(()),
        }
    }

    async fn refresh(&self) {
        let generation = self.next_refresh_generation().await;
        let _guard = self.refresh_in_flight.lock().await;
        let root = self.root.lock().await.clone();
        let cfg = self.config.lock().await.clone();
        let Some(diff) = self.diff_source(&root, &cfg).await else {
            self.clear_stale_diagnostics().await;
            return;
        };
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
                self.client
                    .log_message(
                        MessageType::ERROR,
                        format!("unsafe-review analysis failed: {err}"),
                    )
                    .await;
                self.clear_stale_diagnostics().await;
                return;
            }
            Err(err) => {
                self.client
                    .log_message(
                        MessageType::ERROR,
                        format!("unsafe-review analysis task failed: {err}"),
                    )
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
        let (clear_uris, publish_batches) = self.install_refresh_result(output, by_uri).await;
        for uri in clear_uris {
            self.client.publish_diagnostics(uri, vec![], None).await;
        }
        for (uri, diagnostics) in publish_batches {
            self.client
                .publish_diagnostics(uri, diagnostics, None)
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
    ) -> (Vec<Uri>, Vec<(Uri, Vec<Diagnostic>)>) {
        let current: BTreeSet<_> = by_uri.keys().cloned().collect();
        let clear_uris = {
            let mut previous = self.last_diagnostic_uris.lock().await;
            let clear_uris = previous.difference(&current).cloned().collect::<Vec<_>>();
            *previous = current;
            clear_uris
        };
        let publish_batches = by_uri
            .iter()
            .map(|(uri, diagnostics)| (uri.clone(), diagnostics.clone()))
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
            self.client.publish_diagnostics(uri, vec![], None).await;
        }
    }
}

impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> LspResult<InitializeResult> {
        if let Some(path) = root_from_initialize_params(&params) {
            *self.root.lock().await = path;
        }
        if let Some(opts) = params.initialization_options {
            *self.config.lock().await = parse_config(opts);
        }
        Ok(InitializeResult {
            capabilities: server_capabilities(),
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, TRUST_BOUNDARY)
            .await;
        self.refresh().await;
    }

    async fn shutdown(&self) -> LspResult<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.documents
            .lock()
            .await
            .docs
            .insert(params.text_document.uri, params.text_document.text);
        if self.config.lock().await.refresh_on_open {
            self.refresh().await;
        }
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if let Some(TextDocumentContentChangeEvent { text, .. }) =
            params.content_changes.into_iter().next()
        {
            self.documents
                .lock()
                .await
                .docs
                .insert(params.text_document.uri, text);
        }
        let refresh_on_change = {
            let config = self.config.lock().await;
            should_refresh_on_change(&config)
        };
        if refresh_on_change {
            self.refresh().await;
        }
    }

    async fn did_save(&self, _params: DidSaveTextDocumentParams) {
        if self.config.lock().await.refresh_on_save {
            self.refresh().await;
        }
    }

    async fn hover(&self, params: HoverParams) -> LspResult<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let output = self.latest_analysis.lock().await.clone();
        let diagnostics = self
            .latest_diagnostics
            .lock()
            .await
            .get(&uri)
            .cloned()
            .unwrap_or_default();
        Ok(hover_for(output.as_ref(), &diagnostics, position))
    }

    async fn code_action(
        &self,
        params: CodeActionParams,
    ) -> LspResult<Option<Vec<CodeActionOrCommand>>> {
        let output = self.latest_analysis.lock().await.clone();
        let diagnostics = self
            .latest_diagnostics
            .lock()
            .await
            .get(&params.text_document.uri)
            .cloned()
            .unwrap_or_default();
        Ok(Some(code_actions_for(
            output.as_ref(),
            &diagnostics,
            params.range.start,
        )))
    }

    async fn execute_command(&self, params: ExecuteCommandParams) -> LspResult<Option<Value>> {
        match params.command.as_str() {
            CMD_REFRESH => {
                self.refresh().await;
                Ok(Some(json!({"ok":true})))
            }
            CMD_PACKET | CMD_WITNESS_ROUTE | CMD_WITNESS_COMMAND | CMD_OPEN_TEST => {
                let Some(output) = self.latest_analysis.lock().await.as_ref().cloned() else {
                    return Ok(None);
                };
                Ok(execute_card_command(
                    params.command.as_str(),
                    &params.arguments,
                    &output,
                ))
            }
            _ => Ok(None),
        }
    }
}
