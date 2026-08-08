use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde_json::{Value, json};
use tokio::sync::Mutex;
use tower_lsp_server::jsonrpc::Result as LspResult;
use tower_lsp_server::ls_types::{
    CodeActionOrCommand, CodeActionParams, Diagnostic, DidChangeTextDocumentParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
    ExecuteCommandParams, Hover, HoverParams, InitializeParams, InitializeResult,
    InitializedParams, MessageType, TextDocumentContentChangeEvent, Uri,
};
use tower_lsp_server::{Client, LanguageServer};
use unsafe_review_core::AnalyzeOutput;

use super::TRUST_BOUNDARY;
use super::actions::{
    ActionClientSupport, code_actions_for, execute_card_command, validate_command_arguments,
};
use super::capabilities::{root_from_initialize_params, server_capabilities};
use super::config::{LspConfig, parse_config, should_refresh_on_change};
use super::diagnostics::canonical_request_diagnostics;
use super::hover::hover_for;
use super::state::DocumentStore;
use super::{CMD_OPEN_TEST, CMD_PACKET, CMD_REFRESH, CMD_WITNESS_COMMAND, CMD_WITNESS_ROUTE};

mod refresh;

pub(super) struct Backend {
    client: Client,
    root: Mutex<PathBuf>,
    config: Mutex<LspConfig>,
    documents: Mutex<DocumentStore>,
    live_snapshot: Mutex<LiveSnapshot>,
    last_diagnostic_uris: Mutex<BTreeSet<Uri>>,
    refresh_generation: Mutex<u64>,
    refresh_in_flight: Mutex<()>,
    refresh_pending: Mutex<bool>,
    action_support: Mutex<ActionClientSupport>,
}

#[derive(Default)]
struct LiveSnapshot {
    analysis: Option<AnalyzeOutput>,
    diagnostics: BTreeMap<Uri, Vec<Diagnostic>>,
    current: bool,
}

impl Backend {
    pub(super) fn new(client: Client) -> Self {
        Self {
            client,
            root: Mutex::new(PathBuf::from(".")),
            config: Mutex::new(LspConfig::default()),
            documents: Mutex::new(DocumentStore::default()),
            live_snapshot: Mutex::new(LiveSnapshot::default()),
            last_diagnostic_uris: Mutex::new(BTreeSet::new()),
            refresh_generation: Mutex::new(0),
            refresh_in_flight: Mutex::new(()),
            refresh_pending: Mutex::new(false),
            action_support: Mutex::new(ActionClientSupport::default()),
        }
    }
}

impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> LspResult<InitializeResult> {
        let action_capabilities = params
            .capabilities
            .text_document
            .as_ref()
            .and_then(|text| text.code_action.as_ref());
        *self.action_support.lock().await = ActionClientSupport {
            literals: action_capabilities
                .and_then(|caps| caps.code_action_literal_support.as_ref())
                .is_some(),
            disabled: action_capabilities
                .and_then(|caps| caps.disabled_support)
                .unwrap_or(false),
            data: action_capabilities
                .and_then(|caps| caps.data_support)
                .unwrap_or(false),
            preferred: action_capabilities
                .and_then(|caps| caps.is_preferred_support)
                .unwrap_or(false),
        };
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
        self.documents.lock().await.upsert(
            params.text_document.uri,
            params.text_document.text,
            params.text_document.version,
        );
        if self.config.lock().await.refresh_on_open {
            self.refresh().await;
        }
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = params.text_document.version;
        if let Some(TextDocumentContentChangeEvent { text, .. }) =
            params.content_changes.into_iter().next()
        {
            let mut documents = self.documents.lock().await;
            if let Some(document) = documents.docs.get_mut(&uri) {
                document.text = text;
                document.version = version;
            } else {
                documents.upsert(uri.clone(), text, version);
            }
        } else {
            self.documents.lock().await.update_version(&uri, version);
        }
        self.mark_diagnostics_stale().await;
        let refresh_on_change = {
            let config = self.config.lock().await;
            should_refresh_on_change(&config)
        };
        if refresh_on_change {
            self.refresh().await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.documents
            .lock()
            .await
            .remove(&params.text_document.uri);
        self.mark_diagnostics_stale().await;
    }

    async fn did_save(&self, _params: DidSaveTextDocumentParams) {
        if self.config.lock().await.refresh_on_save {
            self.refresh().await;
        }
    }

    async fn hover(&self, params: HoverParams) -> LspResult<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let snapshot = self.live_snapshot.lock().await;
        let output = snapshot.analysis.clone();
        let diagnostics = snapshot.diagnostics.get(&uri).cloned().unwrap_or_default();
        Ok(hover_for(output.as_ref(), &diagnostics, position))
    }

    async fn code_action(
        &self,
        params: CodeActionParams,
    ) -> LspResult<Option<Vec<CodeActionOrCommand>>> {
        let snapshot = self.live_snapshot.lock().await;
        let output = snapshot
            .current
            .then(|| snapshot.analysis.clone())
            .flatten();
        let cached_diagnostics = snapshot
            .diagnostics
            .get(&params.text_document.uri)
            .cloned()
            .unwrap_or_default();
        let diagnostics =
            canonical_request_diagnostics(cached_diagnostics, &params.context.diagnostics);
        Ok(Some(code_actions_for(
            output.as_ref(),
            &diagnostics,
            params.range.start,
            *self.action_support.lock().await,
        )))
    }

    async fn execute_command(&self, params: ExecuteCommandParams) -> LspResult<Option<Value>> {
        match params.command.as_str() {
            CMD_REFRESH => {
                self.refresh().await;
                Ok(Some(json!({"ok":true})))
            }
            CMD_PACKET | CMD_WITNESS_ROUTE | CMD_WITNESS_COMMAND | CMD_OPEN_TEST => {
                let snapshot = self.live_snapshot.lock().await;
                let Some(output) = snapshot
                    .current
                    .then(|| snapshot.analysis.clone())
                    .flatten()
                else {
                    return Ok(Some(json!({
                        "ok": false,
                        "error": "analysis_not_current",
                        "retry": "unsafe-review.refresh",
                    })));
                };
                if let Err(code) =
                    validate_command_arguments(params.command.as_str(), &params.arguments, &output)
                {
                    return Ok(Some(json!({
                        "ok": false,
                        "error": code,
                        "retry": "unsafe-review.refresh",
                    })));
                }
                let result =
                    execute_card_command(params.command.as_str(), &params.arguments, &output);
                Ok(result.or_else(|| {
                    Some(json!({
                        "ok": false,
                        "error": "action_unavailable",
                        "retry": "request a fresh unsafe-review code action",
                    }))
                }))
            }
            _ => Ok(None),
        }
    }
}
