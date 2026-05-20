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
    latest_diagnostics: Mutex<BTreeMap<Uri, Vec<Diagnostic>>>,
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

fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
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
    }
}

fn root_from_initialize_params(params: &InitializeParams) -> Option<PathBuf> {
    if let Some(folder) = params
        .workspace_folders
        .as_ref()
        .and_then(|folders| folders.first())
        && let Some(path) = folder.uri.to_file_path()
    {
        return Some(path.to_path_buf());
    }
    deprecated_root_uri(params)
}

#[expect(
    deprecated,
    reason = "root_uri remains the fallback for clients without workspaceFolders"
)]
fn deprecated_root_uri(params: &InitializeParams) -> Option<PathBuf> {
    params
        .root_uri
        .as_ref()
        .and_then(Uri::to_file_path)
        .map(|path| path.to_path_buf())
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
        let end = Position::new(line, start.character + lsp_width(&card.site.snippet));
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

fn lsp_width(text: &str) -> u32 {
    text.lines()
        .next()
        .unwrap_or(text)
        .chars()
        .map(|c| c.len_utf16() as u32)
        .sum::<u32>()
        .max(1)
}

fn hover_for(
    output: Option<&AnalyzeOutput>,
    diagnostics: &[Diagnostic],
    pos: Position,
) -> Option<Hover> {
    let card = find_card_at_position(output?, diagnostics, pos)?;
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

fn code_actions_for(
    output: Option<&AnalyzeOutput>,
    diagnostics: &[Diagnostic],
    pos: Position,
) -> Vec<CodeActionOrCommand> {
    let mut actions = vec![CodeActionOrCommand::Command(Command {
        title: "Refresh unsafe-review diagnostics".into(),
        command: CMD_REFRESH.into(),
        arguments: None,
    })];
    let Some(output) = output else {
        return actions;
    };
    let Some(card) = find_card_at_position(output, diagnostics, pos) else {
        return actions;
    };
    actions.extend(card_code_actions(card));
    actions
}

fn card_code_actions(card: &unsafe_review_core::ReviewCard) -> Vec<CodeActionOrCommand> {
    let card_id = card.id.0.clone();
    let mut actions = vec![
        command_action(
            format!("Copy unsafe-review packet for {card_id}"),
            CMD_PACKET,
            json!({"card_id": card_id}),
        ),
        command_action(
            format!("Explain unsafe-review witness route for {}", card.id.0),
            CMD_WITNESS_ROUTE,
            json!({"card_id": card.id.0}),
        ),
    ];
    if card.routes.iter().any(|route| route.command.is_some()) {
        actions.push(command_action(
            format!("Copy recommended witness command for {}", card.id.0),
            CMD_WITNESS_COMMAND,
            json!({"card_id": card.id.0}),
        ));
    }
    if let Some(test) = card.related_tests.first() {
        actions.push(command_action(
            format!("Open related test `{}`", test.name),
            CMD_OPEN_TEST,
            json!({
                "card_id": card.id.0,
                "file": test.file,
                "line": test.line,
                "name": test.name
            }),
        ));
    }
    actions
}

fn command_action(title: impl Into<String>, command: &str, argument: Value) -> CodeActionOrCommand {
    CodeActionOrCommand::Command(Command {
        title: title.into(),
        command: command.into(),
        arguments: Some(vec![argument]),
    })
}

fn find_card_at_position<'a>(
    output: &'a AnalyzeOutput,
    diagnostics: &[Diagnostic],
    pos: Position,
) -> Option<&'a unsafe_review_core::ReviewCard> {
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| range_contains(diagnostic.range, pos))?;
    let card_id = diagnostic_card_id(diagnostic)?;
    output.cards.iter().find(|card| card.id.0 == card_id)
}

fn diagnostic_card_id(diagnostic: &Diagnostic) -> Option<String> {
    diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get("card_id"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn range_contains(range: Range, pos: Position) -> bool {
    pos.line == range.start.line
        && pos.character >= range.start.character
        && pos.character <= range.end.character
}

fn execute_card_command(
    command: &str,
    arguments: &[Value],
    output: &AnalyzeOutput,
) -> Option<Value> {
    let card_id = command_card_id(arguments)?;
    let card = output.cards.iter().find(|card| card.id.0 == card_id)?;
    match command {
        CMD_PACKET => collect_context(output, &CardId(card_id)).map(Value::String),
        CMD_WITNESS_ROUTE => card.routes.first().map(|route| {
            json!({
                "kind": "unsafe-review.witness_route",
                "card_id": card.id.0,
                "route": route.kind.as_str(),
                "reason": route.reason,
                "trust_boundary": TRUST_BOUNDARY
            })
        }),
        CMD_WITNESS_COMMAND => card.routes.iter().find_map(|route| {
            route.command.as_ref().map(|command| {
                json!({
                    "kind": "unsafe-review.witness_command",
                    "card_id": card.id.0,
                    "route": route.kind.as_str(),
                    "command": command,
                    "trust_boundary": TRUST_BOUNDARY
                })
            })
        }),
        CMD_OPEN_TEST => card.related_tests.first().map(|test| {
            json!({
                "kind": "unsafe-review.related_test",
                "card_id": card.id.0,
                "file": test.file,
                "line": test.line,
                "name": test.name
            })
        }),
        _ => None,
    }
}

fn command_card_id(arguments: &[Value]) -> Option<String> {
    arguments
        .first()
        .and_then(|argument| argument.get("card_id"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn clear_uris_for_failure(previous: &mut BTreeSet<Uri>) -> Vec<Uri> {
    let clear_uris = previous.iter().cloned().collect::<Vec<_>>();
    previous.clear();
    clear_uris
}

fn should_refresh_on_change(_cfg: &LspConfig) -> bool {
    false
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use tower_lsp_server::ls_types::{
        CodeActionProviderCapability, ExecuteCommandOptions, HoverProviderCapability,
    };
    use unsafe_review_core::{AnalysisMode, AnalyzeInput, DiffSource, PolicyMode, Scope};

    fn fixture_output(name: &str) -> Result<(PathBuf, AnalyzeOutput), Box<dyn Error>> {
        let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .ok_or("unsafe-review-cli should live under crates/")?
            .to_path_buf();
        let root = workspace_root.join("fixtures").join(name);
        let output = analyze(AnalyzeInput {
            root: root.clone(),
            scope: Scope::Repo,
            diff: DiffSource::NoneRepoScan,
            mode: AnalysisMode::Repo,
            policy: PolicyMode::Advisory,
            include_unchanged_tests: true,
            max_cards: None,
        })?;
        Ok((root, output))
    }

    #[test]
    fn initialize_returns_read_only_capabilities() -> Result<(), Box<dyn Error>> {
        let capabilities = server_capabilities();
        assert!(matches!(
            capabilities.hover_provider,
            Some(HoverProviderCapability::Simple(true))
        ));
        assert!(matches!(
            capabilities.code_action_provider,
            Some(CodeActionProviderCapability::Simple(true))
        ));
        let Some(ExecuteCommandOptions { commands, .. }) = capabilities.execute_command_provider
        else {
            return Err("execute command provider should be present".into());
        };
        assert!(commands.contains(&CMD_REFRESH.to_string()));
        assert!(commands.contains(&CMD_PACKET.to_string()));
        assert!(commands.contains(&CMD_WITNESS_COMMAND.to_string()));
        Ok(())
    }

    #[test]
    fn parse_config_defaults_to_repo_advisory() {
        let config = parse_config(json!({}));
        assert_eq!(config.mode, "repo");
        assert_eq!(config.base, None);
        assert_eq!(config.max_cards, None);
        assert!(!config.refresh_on_open);
        assert!(config.refresh_on_save);
    }

    #[test]
    fn invalid_config_falls_back_to_safe_defaults() {
        let config = parse_config(json!({
            "unsafeReview": {
                "mode": "unsafe-edits",
                "maxCards": "many",
                "refreshOnOpen": true,
                "refreshOnSave": false
            }
        }));
        assert_eq!(config.mode, "repo");
        assert_eq!(config.max_cards, None);
        assert!(config.refresh_on_open);
        assert!(!config.refresh_on_save);
    }

    #[test]
    fn diagnostic_for_card_carries_card_id_and_trust_boundary() -> Result<(), Box<dyn Error>> {
        let (root, output) = fixture_output("raw_pointer_alignment")?;
        let diagnostics = diagnostics_by_uri(&root, &output);
        let diagnostic = diagnostics
            .values()
            .flatten()
            .next()
            .ok_or("expected diagnostic")?;
        assert_eq!(
            diagnostic_card_id(diagnostic),
            Some(output.cards[0].id.0.clone())
        );
        assert!(
            diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("trust_boundary"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .contains("not UB-free status")
        );
        Ok(())
    }

    #[test]
    fn diagnostic_range_uses_utf16_width() -> Result<(), Box<dyn Error>> {
        let (root, mut output) = fixture_output("raw_pointer_alignment")?;
        output.cards[0].site.snippet = "a🦀".to_string();
        let diagnostics = diagnostics_by_uri(&root, &output);
        let diagnostic = diagnostics
            .values()
            .flatten()
            .next()
            .ok_or("expected diagnostic")?;
        assert_eq!(
            diagnostic.range.end.character - diagnostic.range.start.character,
            3
        );
        Ok(())
    }

    #[test]
    fn hover_selects_card_at_cursor() -> Result<(), Box<dyn Error>> {
        let (root, output) = fixture_output("raw_pointer_alignment")?;
        let diagnostics = diagnostics_by_uri(&root, &output);
        let diagnostic = diagnostics
            .values()
            .flatten()
            .next()
            .ok_or("expected diagnostic")?;
        let hover = hover_for(
            Some(&output),
            std::slice::from_ref(diagnostic),
            diagnostic.range.start,
        )
        .ok_or("expected hover")?;
        let HoverContents::Markup(markup) = hover.contents else {
            return Err("expected markdown hover".into());
        };
        assert!(markup.value.contains(&output.cards[0].id.0));
        assert!(markup.value.contains("Trust boundary"));
        Ok(())
    }

    #[test]
    fn hover_outside_card_returns_none_or_neutral_status() -> Result<(), Box<dyn Error>> {
        let (root, output) = fixture_output("raw_pointer_alignment")?;
        let diagnostics = diagnostics_by_uri(&root, &output);
        let diagnostic = diagnostics
            .values()
            .flatten()
            .next()
            .ok_or("expected diagnostic")?;
        let outside = Position::new(
            diagnostic.range.end.line,
            diagnostic.range.end.character + 10,
        );
        assert!(hover_for(Some(&output), std::slice::from_ref(diagnostic), outside).is_none());
        Ok(())
    }

    #[test]
    fn code_actions_are_command_only() -> Result<(), Box<dyn Error>> {
        let (root, output) = fixture_output("raw_pointer_alignment")?;
        let diagnostics = diagnostics_by_uri(&root, &output);
        let diagnostic = diagnostics
            .values()
            .flatten()
            .next()
            .ok_or("expected diagnostic")?;
        let actions = code_actions_for(
            Some(&output),
            std::slice::from_ref(diagnostic),
            diagnostic.range.start,
        );
        assert!(actions.len() >= 3);
        assert!(
            actions
                .iter()
                .all(|action| matches!(action, CodeActionOrCommand::Command(_)))
        );
        assert!(actions.iter().any(|action| {
            matches!(action, CodeActionOrCommand::Command(command) if command.command == CMD_PACKET)
        }));
        Ok(())
    }

    #[test]
    fn execute_collect_agent_packet_returns_packet_for_card() -> Result<(), Box<dyn Error>> {
        let (_root, output) = fixture_output("raw_pointer_alignment")?;
        let card_id = output.cards[0].id.0.clone();
        let packet = execute_card_command(CMD_PACKET, &[json!({"card_id": card_id})], &output)
            .ok_or("expected packet")?;
        let packet = packet
            .as_str()
            .ok_or("packet should be returned as a string")?;
        assert!(packet.contains(&output.cards[0].id.0));
        assert!(packet.contains("do_not_do"));
        Ok(())
    }

    #[test]
    fn execute_unknown_command_returns_none() -> Result<(), Box<dyn Error>> {
        let (_root, output) = fixture_output("raw_pointer_alignment")?;
        assert!(
            execute_card_command(
                "unsafe-review.unknown",
                &[json!({"card_id": output.cards[0].id.0})],
                &output
            )
            .is_none()
        );
        Ok(())
    }

    #[test]
    fn refresh_failure_clears_stale_diagnostics() -> Result<(), Box<dyn Error>> {
        let uri = Uri::from_file_path(
            std::env::current_dir()?.join("fixtures/raw_pointer_alignment/src/lib.rs"),
        )
        .ok_or("expected file uri")?;
        let mut previous = BTreeSet::from([uri.clone()]);
        let clear = clear_uris_for_failure(&mut previous);
        assert_eq!(clear, vec![uri]);
        assert!(previous.is_empty());
        Ok(())
    }

    #[test]
    fn did_change_does_not_trigger_analysis_by_default() {
        assert!(!should_refresh_on_change(&LspConfig::default()));
    }
}
