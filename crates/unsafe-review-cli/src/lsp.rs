use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use tokio::sync::Mutex;
use tower_lsp_server::jsonrpc::Result as LspResult;
use tower_lsp_server::ls_types::{
    CodeActionOrCommand, CodeActionParams, CodeActionProviderCapability, Command, Diagnostic,
    DiagnosticSeverity, DidChangeTextDocumentParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams, ExecuteCommandOptions, ExecuteCommandParams, Hover, HoverContents,
    HoverParams, InitializeParams, InitializeResult, InitializedParams, MarkupContent, MarkupKind,
    MessageType, Position, Range, ServerCapabilities, TextDocumentContentChangeEvent,
    TextDocumentSyncCapability, TextDocumentSyncKind, Uri,
};
use tower_lsp_server::{Client, LanguageServer, LspService, Server};
use unsafe_review_core::{
    AnalysisMode, AnalyzeInput, AnalyzeOutput, CardId, DiffSource, PolicyMode, Scope, analyze,
    collect_context,
};

const CMD_REFRESH: &str = "unsafe-review.refresh";
const CMD_PACKET: &str = "unsafe-review.collectAgentPacket";
const CMD_WITNESS_ROUTE: &str = "unsafe-review.explainWitnessRoute";
const CMD_WITNESS_COMMAND: &str = "unsafe-review.collectWitnessCommand";
const CMD_OPEN_TEST: &str = "unsafe-review.openRelatedTest";
const TRUST_BOUNDARY: &str = "Static unsafe-contract review only. This is not memory-safety proof, not UB-free status, and not a Miri result unless a matching witness receipt is attached.";

#[derive(Clone, Debug)]
struct LspConfig {
    mode: String,
    base: Option<String>,
    max_cards: Option<usize>,
    refresh_on_open: bool,
    refresh_on_save: bool,
}

impl Default for LspConfig {
    fn default() -> Self {
        Self {
            mode: "repo".to_string(),
            base: None,
            max_cards: None,
            refresh_on_open: false,
            refresh_on_save: true,
        }
    }
}

#[derive(Default)]
struct DocumentStore {
    docs: BTreeMap<Uri, String>,
}

struct Backend {
    client: Client,
    root: Mutex<PathBuf>,
    config: Mutex<LspConfig>,
    documents: Mutex<DocumentStore>,
    latest_analysis: Mutex<Option<AnalyzeOutput>>,
    last_diagnostic_uris: Mutex<BTreeSet<Uri>>,
    refresh_generation: Mutex<u64>,
    refresh_in_flight: Mutex<()>,
}

impl Backend {
    fn new(client: Client) -> Self {
        Self {
            client,
            root: Mutex::new(PathBuf::from(".")),
            config: Mutex::new(LspConfig::default()),
            documents: Mutex::new(DocumentStore::default()),
            latest_analysis: Mutex::new(None),
            last_diagnostic_uris: Mutex::new(BTreeSet::new()),
            refresh_generation: Mutex::new(0),
            refresh_in_flight: Mutex::new(()),
        }
    }

    async fn refresh(&self) {
        let _guard = self.refresh_in_flight.lock().await;
        let root = self.root.lock().await.clone();
        let cfg = self.config.lock().await.clone();
        let diff = if cfg.mode == "diff" {
            if let Some(base) = cfg.base.as_ref() {
                match std::process::Command::new("git")
                    .arg("diff")
                    .arg(format!("{base}...HEAD"))
                    .current_dir(&root)
                    .output()
                {
                    Ok(out) if out.status.success() => {
                        DiffSource::Text(String::from_utf8_lossy(&out.stdout).into_owned())
                    }
                    _ => DiffSource::NoneRepoScan,
                }
            } else {
                DiffSource::NoneRepoScan
            }
        } else {
            DiffSource::NoneRepoScan
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
        let Ok(Ok(output)) = analyzed else {
            return;
        };
        let by_uri = diagnostics_by_uri(&root, &output);
        let mut prev = self.last_diagnostic_uris.lock().await;
        let current: BTreeSet<_> = by_uri.keys().cloned().collect();
        for uri in prev.difference(&current) {
            self.client
                .publish_diagnostics(uri.clone(), vec![], None)
                .await;
        }
        for (uri, diagnostics) in &by_uri {
            self.client
                .publish_diagnostics(uri.clone(), diagnostics.clone(), None)
                .await;
        }
        *prev = current;
        *self.latest_analysis.lock().await = Some(output);
        *self.refresh_generation.lock().await += 1;
    }
}

impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> LspResult<InitializeResult> {
        if let Some(folder) = params
            .workspace_folders
            .and_then(|mut f| f.drain(..).next())
            && let Some(path) = folder.uri.to_file_path()
        {
            *self.root.lock().await = path.to_path_buf();
        } else if let Some(uri) = params.root_uri
            && let Some(path) = uri.to_file_path()
        {
            *self.root.lock().await = path.to_path_buf();
        }
        if let Some(opts) = params.initialization_options {
            *self.config.lock().await = parse_config(opts);
        }
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(tower_lsp_server::ls_types::HoverProviderCapability::Simple(
                    true,
                )),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec![
                        CMD_REFRESH.into(),
                        CMD_PACKET.into(),
                        CMD_WITNESS_ROUTE.into(),
                        CMD_WITNESS_COMMAND.into(),
                        CMD_OPEN_TEST.into(),
                    ],
                    work_done_progress_options: Default::default(),
                }),
                ..Default::default()
            },
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
    }
    async fn did_save(&self, _params: DidSaveTextDocumentParams) {
        if self.config.lock().await.refresh_on_save {
            self.refresh().await;
        }
    }
    async fn hover(&self, params: HoverParams) -> LspResult<Option<Hover>> {
        Ok(hover_for(
            self.latest_analysis.lock().await.as_ref(),
            params.text_document_position_params.position,
        ))
    }
    async fn code_action(
        &self,
        _params: CodeActionParams,
    ) -> LspResult<Option<Vec<CodeActionOrCommand>>> {
        Ok(Some(vec![CodeActionOrCommand::Command(Command {
            title: "Refresh unsafe-review diagnostics".into(),
            command: CMD_REFRESH.into(),
            arguments: None,
        })]))
    }
    async fn execute_command(&self, params: ExecuteCommandParams) -> LspResult<Option<Value>> {
        match params.command.as_str() {
            CMD_REFRESH => {
                self.refresh().await;
                Ok(Some(json!({"ok":true})))
            }
            CMD_PACKET => {
                let Some(card_id) = params
                    .arguments
                    .into_iter()
                    .next()
                    .and_then(|a| a.as_array().and_then(|arr| arr.first().cloned()))
                    .and_then(|v| v.as_str().map(ToOwned::to_owned))
                else {
                    return Ok(None);
                };
                let Some(output) = self.latest_analysis.lock().await.as_ref().cloned() else {
                    return Ok(None);
                };
                let id = CardId(card_id);
                Ok(collect_context(&output, &id).map(Value::String))
            }
            _ => Ok(None),
        }
    }
}

fn parse_config(v: Value) -> LspConfig {
    let mut cfg = LspConfig::default();
    if let Some(u) = v.get("unsafeReview") {
        if let Some(mode) = u.get("mode").and_then(Value::as_str)
            && matches!(mode, "repo" | "diff")
        {
            cfg.mode = mode.to_string();
        }
        if let Some(base) = u.get("base").and_then(Value::as_str) {
            cfg.base = Some(base.to_string());
        }
        if let Some(m) = u.get("maxCards").and_then(Value::as_u64) {
            cfg.max_cards = Some(m as usize);
        }
        if let Some(b) = u.get("refreshOnOpen").and_then(Value::as_bool) {
            cfg.refresh_on_open = b;
        }
        if let Some(b) = u.get("refreshOnSave").and_then(Value::as_bool) {
            cfg.refresh_on_save = b;
        }
    }
    cfg
}

fn diagnostics_by_uri(root: &Path, output: &AnalyzeOutput) -> BTreeMap<Uri, Vec<Diagnostic>> {
    let mut map = BTreeMap::new();
    for card in &output.cards {
        let path = root.join(&card.site.location.file);
        let Some(uri) = Uri::from_file_path(path) else {
            continue;
        };
        let line = card.site.location.line.saturating_sub(1) as u32;
        let start = Position::new(line, card.site.location.column.saturating_sub(1) as u32);
        let end = Position::new(line, start.character + 1);
        let d = Diagnostic {
            range: Range::new(start, end),
            severity: Some(
                if matches!(card.priority, unsafe_review_core::Priority::High) {
                    DiagnosticSeverity::WARNING
                } else {
                    DiagnosticSeverity::INFORMATION
                },
            ),
            code: Some(tower_lsp_server::ls_types::NumberOrString::String(
                card.class.as_str().to_string(),
            )),
            source: Some("unsafe-review".into()),
            message: format!(
                "{}: {}",
                card.operation.family.as_str(),
                card.next_action.summary
            ),
            data: Some(json!({"card_id": &card.id.0, "trust_boundary": TRUST_BOUNDARY})),
            ..Default::default()
        };
        map.entry(uri).or_insert_with(Vec::new).push(d);
    }
    map
}

fn hover_for(output: Option<&AnalyzeOutput>, _pos: Position) -> Option<Hover> {
    let card = output.and_then(|o| o.cards.first())?;
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: format!(
                "### unsafe-review: {}\n\nCard: `{}`\n\nOperation: `{}`\n\nSuggested next action:\n{}\n\nTrust boundary:\n{}",
                card.class.as_str(),
                &card.id.0,
                card.operation.family.as_str(),
                card.next_action.summary,
                TRUST_BOUNDARY
            ),
        }),
        range: None,
    })
}

pub(crate) fn serve() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("tokio runtime init failed: {e}"))?;
    runtime.block_on(async {
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        let (service, socket) = LspService::new(Backend::new);
        Server::new(stdin, stdout, socket).serve(service).await;
    });
    Ok(())
}
