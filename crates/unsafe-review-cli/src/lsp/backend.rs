use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

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
use unsafe_review_core::AnalyzeOutput;

use super::TRUST_BOUNDARY;
use super::actions::{code_actions_for, execute_card_command};
use super::capabilities::{root_from_initialize_params, server_capabilities};
use super::config::{LspConfig, parse_config, should_refresh_on_change};
use super::hover::hover_for;
use super::state::DocumentStore;
use super::{CMD_OPEN_TEST, CMD_PACKET, CMD_REFRESH, CMD_WITNESS_COMMAND, CMD_WITNESS_ROUTE};

mod refresh;

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
