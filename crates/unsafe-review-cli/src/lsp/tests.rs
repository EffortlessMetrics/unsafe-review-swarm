use std::collections::BTreeSet;
use std::error::Error;
use std::path::{Path, PathBuf};

use futures::StreamExt;
use serde_json::{Value, json};
use tower::Service as _;
use tower_lsp_server::jsonrpc::Request;
use tower_lsp_server::ls_types::{
    CodeActionOrCommand, CodeActionProviderCapability, DiagnosticSeverity,
    DidChangeTextDocumentParams, ExecuteCommandOptions, ExecuteCommandParams, HoverContents,
    HoverParams, HoverProviderCapability, InitializeParams, MessageType, Position,
    TextDocumentContentChangeEvent, TextDocumentIdentifier, TextDocumentPositionParams,
    VersionedTextDocumentIdentifier, WorkspaceFolder,
};
use tower_lsp_server::{ClientSocket, LanguageServer, LspService};
use unsafe_review_core::{
    AnalysisMode, AnalyzeInput, AnalyzeOutput, CardId, DiffSource, EditorActionArguments,
    PolicyMode, ReviewClass, Scope, analyze, project_editor, project_editor_diagnostics,
};

use super::actions::{
    ActionClientSupport, code_actions_for, execute_card_command, validate_command_arguments,
};
use super::backend::Backend;
use super::capabilities::server_capabilities;
use super::config::{LspConfig, parse_config, should_refresh_on_change};
use super::diagnostics::{canonical_request_diagnostics, diagnostic_card_id, diagnostics_by_uri};
use super::hover::hover_for;
use super::state::clear_uris_for_failure;
use super::uri::uri_from_path;
use super::{CMD_OPEN_TEST, CMD_PACKET, CMD_REFRESH, CMD_WITNESS_COMMAND, CMD_WITNESS_ROUTE};

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
    let Some(CodeActionProviderCapability::Options(action_options)) =
        capabilities.code_action_provider
    else {
        return Err("structured code action options should be advertised".into());
    };
    assert_eq!(action_options.resolve_provider, Some(false));
    assert_eq!(
        action_options
            .code_action_kinds
            .map_or(0, |kinds| kinds.len()),
        5
    );
    let Some(ExecuteCommandOptions { commands, .. }) = capabilities.execute_command_provider else {
        return Err("execute command provider should be present".into());
    };
    assert!(commands.contains(&CMD_REFRESH.to_string()));
    assert!(commands.contains(&CMD_PACKET.to_string()));
    assert!(commands.contains(&CMD_WITNESS_ROUTE.to_string()));
    assert!(commands.contains(&CMD_WITNESS_COMMAND.to_string()));
    assert!(commands.contains(&CMD_OPEN_TEST.to_string()));
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
fn parse_config_reads_supported_fields() {
    let config = parse_config(json!({
        "unsafeReview": {
            "mode": "diff",
            "base": "origin/main",
            "maxCards": 15,
            "refreshOnOpen": true,
            "refreshOnSave": false
        }
    }));

    assert_eq!(config.mode, "diff");
    assert_eq!(config.base.as_deref(), Some("origin/main"));
    assert_eq!(config.max_cards, Some(15));
    assert!(config.refresh_on_open);
    assert!(!config.refresh_on_save);
}

#[test]
fn oversized_max_cards_is_ignored() {
    let config = parse_config(json!({
        "unsafeReview": {
            "maxCards": u64::MAX
        }
    }));
    #[cfg(target_pointer_width = "32")]
    assert_eq!(config.max_cards, None);
    #[cfg(target_pointer_width = "64")]
    assert_eq!(config.max_cards, Some(usize::MAX));
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
    let trust_boundary = diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get("trust_boundary"))
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(trust_boundary.contains("not Miri-clean status"));
    assert!(trust_boundary.contains("not a site-execution claim"));
    assert!(trust_boundary.contains("matching witness receipt"));
    let data = diagnostic
        .data
        .as_ref()
        .ok_or("diagnostic data should be present")?;
    assert_eq!(data["operation_family"], "raw_pointer_read");
    assert_eq!(data["required_safety_conditions"][0]["key"], "pointer-live");
    assert!(
        data["required_safety_conditions"][0]["description"]
            .as_str()
            .unwrap_or("")
            .contains("pointer is live")
    );
    assert_eq!(data["evidence_summary"]["contract"]["state"], "present");
    assert_eq!(data["evidence_summary"]["discharge"]["state"], "missing");
    assert!(
        data["evidence_summary"]["reach_limitation"]
            .as_str()
            .unwrap_or("")
            .contains("not proof")
    );
    assert!(data["obligation_evidence"].as_array().is_some_and(|items| {
        items.iter().any(|item| {
            item["key"] == "alignment"
                && item["discharge"]["state"] == "missing"
                && item["witness"]["state"] == "missing"
        })
    }));
    assert!(
        data["witness_routes"][0]["command"]
            .as_str()
            .unwrap_or("")
            .contains("cargo +nightly miri test read_header")
    );
    Ok(())
}

#[test]
fn live_diagnostic_projects_the_canonical_editor_diagnostic() -> Result<(), Box<dyn Error>> {
    let (root, output) = fixture_output("raw_pointer_alignment")?;
    let canonical = project_editor_diagnostics(&output)
        .into_iter()
        .find(|diagnostic| diagnostic.card_id == output.cards[0].id.0)
        .ok_or("expected canonical diagnostic")?;
    let saved = project_editor(&output)
        .diagnostics
        .into_iter()
        .find(|diagnostic| diagnostic.card_id == canonical.card_id)
        .ok_or("expected saved canonical diagnostic")?;
    assert_eq!(
        serde_json::to_value(&canonical)?,
        serde_json::to_value(&saved)?,
        "diagnostics-only and saved projections must share the canonical DTO"
    );
    let diagnostics = diagnostics_by_uri(&root, &output);
    let live = diagnostics
        .values()
        .flatten()
        .find(|diagnostic| diagnostic_card_id(diagnostic).as_deref() == Some(&canonical.card_id))
        .ok_or("expected live diagnostic")?;
    let data = live
        .data
        .as_ref()
        .ok_or("live diagnostic should carry canonical data")?;

    assert_eq!(data, &serde_json::to_value(&canonical)?);
    assert_eq!(live.code, Some(canonical.code.clone().into()));
    assert_eq!(live.message, canonical.message);
    assert_eq!(live.source.as_deref(), Some(canonical.source.as_str()));
    let expected_severity = match canonical.severity {
        2 => Some(DiagnosticSeverity::WARNING),
        3 => Some(DiagnosticSeverity::INFORMATION),
        4 => Some(DiagnosticSeverity::HINT),
        _ => None,
    };
    assert_eq!(live.severity, expected_severity);
    assert_eq!(live.range.start.line as usize, canonical.range.start.line);
    assert_eq!(
        live.range.start.character as usize,
        canonical.range.start.character
    );
    assert_eq!(live.range.end.line as usize, canonical.range.end.line);
    assert_eq!(
        live.range.end.character as usize,
        canonical.range.end.character
    );
    Ok(())
}

#[test]
fn diagnostic_range_uses_utf16_width() -> Result<(), Box<dyn Error>> {
    let (root, mut output) = fixture_output("raw_pointer_alignment")?;
    output.cards[0].site.snippet = "a\u{1f980}".to_string();
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
    // Card identity and trust boundary (preserved from original).
    assert!(markup.value.contains(&output.cards[0].id.0));
    assert!(markup.value.contains("Trust boundary"));
    // Rich hover: obligations section must be present.
    assert!(
        markup.value.contains("Required safety conditions:"),
        "hover must contain obligations section (got: {:?})",
        &markup.value[..markup.value.len().min(200)]
    );
    // Rich hover: at least one concrete obligation description.
    assert!(
        markup.value.contains("pointer is live"),
        "hover must contain at least one obligation description (got: {:?})",
        &markup.value[..markup.value.len().min(200)]
    );
    // Rich hover: evidence sections must be present.
    assert!(
        markup.value.contains("Evidence found:"),
        "hover must contain evidence-found section"
    );
    assert!(
        markup.value.contains("Evidence missing:"),
        "hover must contain evidence-missing section"
    );
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
fn code_actions_are_structured_and_command_only() -> Result<(), Box<dyn Error>> {
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
        modern_action_support(),
    );
    assert_eq!(actions.len(), 5);
    assert!(
        matches!(&actions[0], CodeActionOrCommand::Command(command) if command.command == CMD_REFRESH)
    );
    for action in &actions[1..] {
        let CodeActionOrCommand::CodeAction(action) = action else {
            return Err("card-scoped actions must be structured CodeActions".into());
        };
        assert!(action.edit.is_none());
        assert_eq!(action.is_preferred, Some(false));
        assert_eq!(
            action.diagnostics.as_deref(),
            Some(std::slice::from_ref(diagnostic))
        );
        assert!(action.data.is_some());
    }
    Ok(())
}

#[test]
fn unavailable_card_actions_are_structured_and_disabled() -> Result<(), Box<dyn Error>> {
    let (root, mut output) = fixture_output("raw_pointer_alignment")?;
    output.cards[0].routes.clear();
    output.cards[0].related_tests.clear();
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
        modern_action_support(),
    );
    let disabled = actions[1..]
        .iter()
        .filter_map(|action| match action {
            CodeActionOrCommand::CodeAction(action) => action.disabled.as_ref(),
            CodeActionOrCommand::Command(_) => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(disabled.len(), 3);
    assert!(
        disabled
            .iter()
            .any(|item| item.reason.contains("No witness route"))
    );
    assert!(
        disabled
            .iter()
            .any(|item| item.reason.contains("No witness command"))
    );
    assert!(
        disabled
            .iter()
            .any(|item| item.reason.contains("No structured related test"))
    );
    for action in &actions[1..] {
        let CodeActionOrCommand::CodeAction(action) = action else {
            continue;
        };
        if action.disabled.is_some() {
            assert!(action.command.is_none());
        }
    }
    let reason_codes = actions[1..]
        .iter()
        .filter_map(|action| match action {
            CodeActionOrCommand::CodeAction(action) => action
                .data
                .as_ref()?
                .get("applicability")?
                .get("reason_code")?
                .as_str(),
            CodeActionOrCommand::Command(_) => None,
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        reason_codes,
        BTreeSet::from(["no_related_test", "no_witness_command", "no_witness_route"])
    );
    Ok(())
}

#[test]
fn client_capabilities_never_upgrade_unsupported_actions() -> Result<(), Box<dyn Error>> {
    let (root, mut output) = fixture_output("raw_pointer_alignment")?;
    output.cards[0].routes.clear();
    output.cards[0].related_tests.clear();
    let diagnostics = diagnostics_by_uri(&root, &output);
    let diagnostic = diagnostics
        .values()
        .flatten()
        .next()
        .ok_or("expected diagnostic")?;
    let legacy = code_actions_for(
        Some(&output),
        std::slice::from_ref(diagnostic),
        diagnostic.range.start,
        ActionClientSupport::default(),
    );
    assert_eq!(legacy.len(), 2);
    assert!(
        legacy
            .iter()
            .all(|action| matches!(action, CodeActionOrCommand::Command(_)))
    );
    let without_disabled = code_actions_for(
        Some(&output),
        std::slice::from_ref(diagnostic),
        diagnostic.range.start,
        ActionClientSupport {
            literals: true,
            ..Default::default()
        },
    );
    assert_eq!(without_disabled.len(), 2);
    Ok(())
}

#[test]
fn client_diagnostics_cannot_replace_canonical_association() -> Result<(), Box<dyn Error>> {
    let (root, output) = fixture_output("raw_pointer_alignment")?;
    let cached = diagnostics_by_uri(&root, &output)
        .into_values()
        .flatten()
        .collect::<Vec<_>>();
    let mut forged = cached[0].clone();
    forged.message = "forged client message".to_string();
    assert!(canonical_request_diagnostics(cached.clone(), &[forged]).is_empty());
    assert_eq!(
        canonical_request_diagnostics(cached.clone(), &cached),
        cached
    );
    Ok(())
}

#[test]
fn overlapping_diagnostics_emit_deterministic_actions_for_each_card() -> Result<(), Box<dyn Error>>
{
    let (root, mut output) = fixture_output("raw_pointer_alignment")?;
    let mut sibling = output.cards[0].clone();
    sibling.id = CardId(format!("{}-overlap", sibling.id.0));
    output.cards.push(sibling);
    let diagnostics = diagnostics_by_uri(&root, &output);
    let same_uri = diagnostics.values().next().ok_or("expected diagnostics")?;
    assert_eq!(same_uri.len(), 2);
    let actions = code_actions_for(
        Some(&output),
        same_uri,
        same_uri[0].range.start,
        modern_action_support(),
    );
    assert_eq!(actions.len(), 9);
    let mut card_ids = actions[1..]
        .iter()
        .filter_map(|action| match action {
            CodeActionOrCommand::CodeAction(action) => action
                .data
                .as_ref()?
                .get("payload")?
                .get("card_id")?
                .as_str(),
            CodeActionOrCommand::Command(_) => None,
        })
        .collect::<Vec<_>>();
    card_ids.dedup();
    assert_eq!(card_ids.len(), 2);
    Ok(())
}

#[test]
fn execute_collect_agent_packet_returns_packet_for_card() -> Result<(), Box<dyn Error>> {
    let (_root, output) = fixture_output("raw_pointer_alignment")?;
    let card_id = output.cards[0].id.0.clone();
    let packet = execute_card_command(
        CMD_PACKET,
        &[command_arguments(&output, &card_id)?],
        &output,
    )
    .ok_or("expected packet")?;
    let packet = packet
        .as_str()
        .ok_or("packet should be returned as a string")?;
    assert!(packet.contains(&output.cards[0].id.0));
    assert!(packet.contains("\"confirmation_cue\""));
    assert!(packet.contains("\"build_this_first\""));
    assert!(packet.contains("\"minimal_repro\""));
    assert!(packet.contains("attach a matching receipt"));
    assert!(packet.contains("unsafe-review did not run this command"));
    assert!(packet.contains("do_not_do"));
    Ok(())
}

#[test]
fn execute_rejects_action_from_a_different_analysis() -> Result<(), Box<dyn Error>> {
    let (_root, first) = fixture_output("raw_pointer_alignment")?;
    let (_root, second) = fixture_output("raw_pointer_alignment")?;
    assert_eq!(first.cards[0].id, second.cards[0].id);
    assert_ne!(first.analysis_identity, second.analysis_identity);
    let arguments = command_arguments(&first, &first.cards[0].id.0)?;
    assert!(execute_card_command(CMD_PACKET, &[arguments], &second).is_none());
    Ok(())
}

#[test]
fn command_validation_rejects_malformed_foreign_and_unavailable_actions()
-> Result<(), Box<dyn Error>> {
    let (_root, mut output) = fixture_output("raw_pointer_alignment")?;
    assert_eq!(
        validate_command_arguments(CMD_PACKET, &[], &output),
        Err("invalid_action_arguments")
    );
    let valid = command_arguments(&output, &output.cards[0].id.0)?;
    assert_eq!(
        validate_command_arguments(CMD_PACKET, &[valid.clone(), valid.clone()], &output),
        Err("invalid_action_arguments")
    );
    assert_eq!(
        validate_command_arguments(
            CMD_PACKET,
            &[json!({"card_id": output.cards[0].id.0})],
            &output
        ),
        Err("invalid_action_arguments")
    );
    let unknown = command_arguments(&output, "unknown-card")?;
    assert_eq!(
        validate_command_arguments(CMD_PACKET, &[unknown], &output),
        Err("unknown_card")
    );
    output.cards[0].routes.clear();
    assert_eq!(
        validate_command_arguments(CMD_WITNESS_ROUTE, &[valid], &output),
        Err("action_unavailable")
    );
    Ok(())
}

#[test]
fn human_only_live_actions_never_look_like_automatic_quickfixes() -> Result<(), Box<dyn Error>> {
    let (root, output) = fixture_output("ffi_missing_boundary_contract")?;
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
        modern_action_support(),
    );
    let packet = actions[1..]
        .iter()
        .find_map(|action| match action {
            CodeActionOrCommand::CodeAction(action) if action.title.contains("review context") => {
                Some(action)
            }
            _ => None,
        })
        .ok_or("expected human review context action")?;
    assert_eq!(
        packet.kind.as_ref().map(|kind| kind.as_str()),
        Some("source.unsafeReview.reviewContext")
    );
    assert!(!packet.title.to_ascii_lowercase().contains("automatic"));
    assert_eq!(packet.is_preferred, Some(false));
    assert!(packet.edit.is_none());
    Ok(())
}

#[test]
fn execute_unknown_command_returns_none() -> Result<(), Box<dyn Error>> {
    let (_root, output) = fixture_output("raw_pointer_alignment")?;
    assert!(
        execute_card_command(
            "unsafe-review.unknown",
            &[command_arguments(&output, &output.cards[0].id.0)?],
            &output
        )
        .is_none()
    );
    Ok(())
}

/// Drift-lock: `CMD_WITNESS_ROUTE` must return a JSON payload with the expected
/// shape and trust-boundary string.  If the command is removed from the
/// dispatcher or the route kind is renamed, this test turns red.
#[test]
fn execute_explain_witness_route_returns_route_for_card() -> Result<(), Box<dyn Error>> {
    let (_root, output) = fixture_output("raw_pointer_alignment")?;
    let card = output
        .cards
        .first()
        .ok_or("fixture must have at least one card")?;
    let route = card.routes.first().ok_or(
        "raw_pointer_alignment card must have at least one witness route; \
         if the fixture changed, update the fixture or pick one that has routes",
    )?;
    let result = execute_card_command(
        CMD_WITNESS_ROUTE,
        &[command_arguments(&output, &card.id.0)?],
        &output,
    )
    .ok_or("CMD_WITNESS_ROUTE must return Some for a card with routes")?;
    assert_eq!(result["kind"], "unsafe-review.witness_route");
    assert_eq!(result["card_id"], card.id.0.as_str());
    assert_eq!(result["route"], route.kind.as_str());
    assert!(
        result["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("not a site-execution claim"),
        "trust_boundary must contain advisory wording"
    );
    Ok(())
}

/// Drift-lock: `CMD_OPEN_TEST` must return a JSON payload with the expected
/// shape.  If the command is removed from the dispatcher or field names change,
/// this test turns red.
#[test]
fn execute_open_related_test_returns_test_metadata() -> Result<(), Box<dyn Error>> {
    let (_root, output) = fixture_output("raw_pointer_alignment")?;
    let card = output
        .cards
        .first()
        .ok_or("fixture must have at least one card")?;
    let test = card.related_tests.first().ok_or(
        "raw_pointer_alignment card must have at least one related test; \
         if the fixture changed, update the fixture or pick one that has related tests",
    )?;
    let result = execute_card_command(
        CMD_OPEN_TEST,
        &[command_arguments(&output, &card.id.0)?],
        &output,
    )
    .ok_or("CMD_OPEN_TEST must return Some for a card with related tests")?;
    assert_eq!(result["kind"], "unsafe-review.related_test");
    assert_eq!(result["card_id"], card.id.0.as_str());
    assert_eq!(result["name"], test.name.as_str());
    assert!(
        result["file"].as_str().is_some(),
        "file field must be a string"
    );
    assert!(
        result["line"].as_u64().is_some(),
        "line field must be a number"
    );
    Ok(())
}

fn command_arguments(output: &AnalyzeOutput, card_id: &str) -> Result<Value, Box<dyn Error>> {
    Ok(serde_json::to_value(EditorActionArguments {
        card_id: card_id.to_string(),
        analysis: output.analysis_identity.clone(),
        file: None,
        line: None,
        name: None,
    })?)
}

fn modern_action_support() -> ActionClientSupport {
    ActionClientSupport {
        literals: true,
        disabled: true,
        data: true,
        preferred: true,
    }
}

/// `clear_uris_for_failure` backs `mark_diagnostics_stale` (the didChange
/// path), which intentionally blanks diagnostics because the document is
/// known to have changed underneath them. This is distinct from a *failed*
/// refresh (analysis error, task-join error, or no diff source): see
/// `refresh_failure_*` tests below, which assert the opposite — a failed
/// refresh must NOT blank diagnostics via this helper.
#[test]
fn document_change_clears_previously_diagnosed_uris() -> Result<(), Box<dyn Error>> {
    let uri =
        uri_from_path(std::env::current_dir()?.join("fixtures/raw_pointer_alignment/src/lib.rs"))
            .ok_or("expected file uri")?;
    let mut previous = BTreeSet::from([uri.clone()]);
    let clear = clear_uris_for_failure(&mut previous);
    assert_eq!(clear, vec![uri]);
    assert!(previous.is_empty());
    Ok(())
}

#[test]
fn did_change_publishes_versioned_clear_and_invalidates_cached_diagnostics()
-> Result<(), Box<dyn Error>> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to build test runtime: {err}"))?;
    runtime.block_on(async {
        let (root, _) = fixture_output("raw_pointer_alignment")?;
        let root_uri = uri_from_path(&root).ok_or("expected root uri")?;
        let lib_uri =
            uri_from_path(root.join("src/lib.rs")).ok_or("expected file uri for src/lib.rs")?;
        let (mut service, socket) = LspService::new(Backend::new);
        initialize_over_the_wire(
            &mut service,
            &InitializeParams {
                workspace_folders: Some(vec![WorkspaceFolder {
                    uri: root_uri,
                    name: "fixture".to_string(),
                }]),
                ..Default::default()
            },
        )
        .await?;
        let backend = service.inner();
        let (socket, success_messages) =
            refresh_via_execute_command_collecting_messages(backend, socket, 1).await?;
        let publish = success_messages
            .first()
            .ok_or("expected initial diagnostics publication")?;
        assert_eq!(publish.method(), "textDocument/publishDiagnostics");
        let initial_params = publish
            .params()
            .cloned()
            .ok_or("initial diagnostics should carry params")?;
        let start = initial_params["diagnostics"][0]["range"]["start"]
            .as_object()
            .ok_or("initial diagnostic should carry a range start")?;
        let position = Position::new(
            start["line"]
                .as_u64()
                .ok_or("initial diagnostic line should be numeric")? as u32,
            start["character"]
                .as_u64()
                .ok_or("initial diagnostic character should be numeric")? as u32,
        );
        let before = backend
            .hover(HoverParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier {
                        uri: lib_uri.clone(),
                    },
                    position,
                },
                work_done_progress_params: Default::default(),
            })
            .await?;
        assert!(
            before.is_some(),
            "initial refresh should make the card inspectable"
        );

        let mut drain = tokio::spawn(async move {
            let mut socket = socket;
            let mut messages = Vec::new();
            while messages.len() < 2 {
                let Some(message) = socket.next().await else {
                    break;
                };
                messages.push(message);
            }
            (socket, messages)
        });
        backend
            .did_change(DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: lib_uri.clone(),
                    version: 2,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: "changed unsafe document".to_string(),
                }],
            })
            .await;
        let joined =
            match tokio::time::timeout(std::time::Duration::from_secs(30), &mut drain).await {
                Ok(joined) => joined,
                Err(_timeout) => {
                    drain.abort();
                    return Err("draining didChange notifications timed out".into());
                }
            };
        let (_, messages) =
            joined.map_err(|err| format!("drain task panicked or was cancelled: {err}"))?;
        let clear = messages
            .iter()
            .find(|message| message.method() == "textDocument/publishDiagnostics")
            .ok_or("didChange should publish a diagnostics clear")?;
        let params = clear
            .params()
            .cloned()
            .ok_or("diagnostics clear should carry params")?;
        assert_eq!(params["uri"], lib_uri.to_string());
        assert_eq!(params["version"], 2);
        assert_eq!(params["diagnostics"].as_array().map(Vec::len), Some(0));
        assert!(messages.iter().any(|message| {
            message.method() == "window/logMessage"
                && message
                    .params()
                    .is_some_and(|params| params.to_string().contains("marked stale"))
        }));
        assert!(
            backend
                .hover(HoverParams {
                    text_document_position_params: TextDocumentPositionParams {
                        text_document: TextDocumentIdentifier { uri: lib_uri },
                        position,
                    },
                    work_done_progress_params: Default::default(),
                })
                .await?
                .is_none(),
            "stale cached diagnostics must not remain inspectable after didChange"
        );
        Ok(())
    })
}

/// Drives `execute_command(CMD_REFRESH)` while concurrently draining `count`
/// server-to-client notifications off `socket` on a spawned task, then hands
/// `socket` back for the next call. `ClientSocket`'s channel has a capacity of
/// 1, so a refresh that sends more than one notification (e.g. a
/// `window/logMessage` from `diff_source` followed by the `window/showMessage`
/// from `mark_diagnostics_failed`) would deadlock if the socket were drained
/// only after `refresh` completed — draining must happen concurrently, and a
/// real spawned task (rather than a hand-rolled `select`/`join` over a
/// borrowed socket) sidesteps the channel's internal single-slot readiness
/// bookkeeping.
async fn refresh_via_execute_command_collecting_messages(
    backend: &Backend,
    socket: ClientSocket,
    count: usize,
) -> Result<(ClientSocket, Vec<Request>), Box<dyn Error>> {
    // Bound both awaits: if a regression emits fewer notifications than `count`,
    // the drain's `socket.next()` would otherwise block forever; if it emits
    // more, the capacity-1 channel could wedge `execute_command`. On any
    // timeout or refresh failure, abort the drain task before returning so a
    // hung test fails fast instead of hanging CI.
    let timeout = std::time::Duration::from_secs(30);
    let mut drain_handle = tokio::spawn(async move {
        let mut socket = socket;
        let mut collected = Vec::new();
        while collected.len() < count {
            match socket.next().await {
                Some(message) => collected.push(message),
                None => break,
            }
        }
        (socket, collected)
    });
    let refresh_result = match tokio::time::timeout(
        timeout,
        backend.execute_command(ExecuteCommandParams {
            command: CMD_REFRESH.to_string(),
            arguments: Vec::new(),
            work_done_progress_params: Default::default(),
        }),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => {
            drain_handle.abort();
            return Err("execute_command(refresh) timed out".into());
        }
    };
    if let Err(err) = refresh_result {
        drain_handle.abort();
        return Err(format!("execute_command(refresh) failed: {err:?}").into());
    }
    match tokio::time::timeout(timeout, &mut drain_handle).await {
        Ok(joined) => {
            Ok(joined.map_err(|err| format!("drain task panicked or was cancelled: {err}"))?)
        }
        Err(_) => {
            drain_handle.abort();
            Err("draining server-to-client notifications timed out".into())
        }
    }
}

/// Drives a real `initialize` request/response through `service` (rather than
/// calling `Backend::initialize` directly) so that `tower_lsp_server`'s
/// internal server-state gate flips from `Uninitialized` to `Initialized`.
/// That gate is otherwise invisible: `Client::show_message`/`log_message`/
/// `publish_diagnostics` silently no-op pre-initialize, which would make
/// every assertion in these tests about outbound messages vacuously pass.
/// The service only accepts one real `initialize` call (a second is rejected
/// as a duplicate), so later config changes in these tests instead call
/// `Backend::initialize` directly — that updates the backend's root/config
/// fields without touching server state, exactly like the state-gate-free
/// `did_change_configuration` path a real client would use.
async fn initialize_over_the_wire(
    service: &mut LspService<Backend>,
    params: &InitializeParams,
) -> Result<(), Box<dyn Error>> {
    let request = Request::build("initialize")
        .id(1_i64)
        .params(serde_json::to_value(params).map_err(|err| err.to_string())?)
        .finish();
    let response = service
        .call(request)
        .await
        .map_err(|err| format!("service call failed: {err}"))?
        .ok_or("expected a response to the initialize request")?;
    if !response.is_ok() {
        return Err(format!("initialize request was rejected: {response:?}").into());
    }
    Ok(())
}

fn fixture_root() -> Result<PathBuf, Box<dyn Error>> {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or("unsafe-review-cli should live under crates/")?
        .to_path_buf();
    Ok(workspace_root
        .join("fixtures")
        .join("raw_pointer_alignment"))
}

/// Headline regression test for the refresh-failure bug: a refresh that fails
/// (here, because the configured diff base does not resolve) must surface a
/// visible, distinct failure to the editor — not silently render the same as
/// a clean, zero-card file, and not blank out diagnostics from the last
/// successful analysis.
#[test]
fn refresh_failure_surfaces_a_visible_warning_and_preserves_last_diagnostics()
-> Result<(), Box<dyn Error>> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to build test runtime: {err}"))?;
    runtime.block_on(async {
        let root = fixture_root()?;
        let root_uri = uri_from_path(&root).ok_or("expected root uri")?;
        let lib_uri =
            uri_from_path(root.join("src/lib.rs")).ok_or("expected file uri for src/lib.rs")?;

        let (mut service, socket) = LspService::new(Backend::new);
        initialize_over_the_wire(
            &mut service,
            &InitializeParams {
                workspace_folders: Some(vec![WorkspaceFolder {
                    uri: root_uri.clone(),
                    name: "fixture".to_string(),
                }]),
                ..Default::default()
            },
        )
        .await?;
        let backend = service.inner();

        // Repo mode (the default): `diff_source` resolves immediately, so a
        // successful refresh over a known-hazard fixture sends exactly one
        // `textDocument/publishDiagnostics` notification.
        let (socket, success_messages) =
            refresh_via_execute_command_collecting_messages(backend, socket, 1).await?;
        let publish = success_messages
            .first()
            .ok_or("expected a publishDiagnostics notification from the successful refresh")?;
        assert_eq!(publish.method(), "textDocument/publishDiagnostics");
        let params = publish
            .params()
            .cloned()
            .ok_or("publishDiagnostics notification should carry params")?;
        let diagnostics = params
            .get("diagnostics")
            .and_then(Value::as_array)
            .cloned()
            .ok_or("publishDiagnostics params should carry a diagnostics array")?;
        assert!(
            !diagnostics.is_empty(),
            "the successful refresh over raw_pointer_alignment must publish at least one diagnostic"
        );
        let start = diagnostics
            .first()
            .and_then(|diagnostic| diagnostic.get("range"))
            .and_then(|range| range.get("start"))
            .ok_or("expected a diagnostic range start")?;
        let position = Position::new(
            start
                .get("line")
                .and_then(Value::as_u64)
                .ok_or("expected numeric line")? as u32,
            start
                .get("character")
                .and_then(Value::as_u64)
                .ok_or("expected numeric character")? as u32,
        );

        let hover_before = backend
            .hover(HoverParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier {
                        uri: lib_uri.clone(),
                    },
                    position,
                },
                work_done_progress_params: Default::default(),
            })
            .await
            .map_err(|err| format!("hover failed: {err:?}"))?;
        assert!(
            hover_before.is_some(),
            "expected a hover result over the published diagnostic's range"
        );

        // Reconfigure to a diff base that cannot resolve, so `diff_source`
        // returns `None` and the refresh fails before analysis even runs.
        // This calls `Backend::initialize` directly (not through `service`,
        // which would reject a second real `initialize` as a duplicate) —
        // there is no `did_change_configuration` handler in this backend, so
        // this is the only way to change the root/config fields in a test.
        // It intentionally does not touch server state (already Initialized
        // from `initialize_over_the_wire` above).
        backend
            .initialize(InitializeParams {
                workspace_folders: Some(vec![WorkspaceFolder {
                    uri: root_uri,
                    name: "fixture".to_string(),
                }]),
                initialization_options: Some(json!({
                    "unsafeReview": {
                        "mode": "diff",
                        "base": "definitely-not-a-real-ref-zzz",
                    }
                })),
                ..Default::default()
            })
            .await
            .map_err(|err| format!("re-initialize failed: {err:?}"))?;

        let (_socket, failure_messages) =
            refresh_via_execute_command_collecting_messages(backend, socket, 2).await?;

        // The bug: every failure path used to call `clear_stale_diagnostics`,
        // which published an EMPTY diagnostics set — rendering a failed
        // refresh identically to a clean, zero-card file. Assert that no
        // `publishDiagnostics` notification is sent on failure at all.
        assert!(
            failure_messages
                .iter()
                .all(|message| message.method() != "textDocument/publishDiagnostics"),
            "a failed refresh must not publish diagnostics (that would blank or fake a clean \
             result); got methods: {:?}",
            failure_messages
                .iter()
                .map(Request::method)
                .collect::<Vec<_>>()
        );

        // The fix: a `window/showMessage` (editor-visible, not just a log)
        // must appear, and its wording must be a freshness signal only — it
        // must not claim the file is safe, proven, or UB-free.
        let show_message = failure_messages
            .iter()
            .find(|message| message.method() == "window/showMessage")
            .ok_or("expected a window/showMessage notification on refresh failure")?;
        let show_params = show_message
            .params()
            .cloned()
            .ok_or("showMessage notification should carry params")?;
        let expected_type = serde_json::to_value(MessageType::WARNING)
            .map_err(|err| format!("failed to serialize MessageType::WARNING: {err}"))?;
        assert_eq!(
            show_params.get("type"),
            Some(&expected_type),
            "refresh failure must be surfaced at WARNING severity"
        );
        let show_text = show_params
            .get("message")
            .and_then(Value::as_str)
            .ok_or("showMessage params should carry a message string")?;
        assert!(
            !show_text.is_empty(),
            "the visible failure message must not be empty"
        );
        for forbidden in ["proof", "UB-free", "Miri-clean", "guarantee", "certif"] {
            assert!(
                !show_text.to_lowercase().contains(&forbidden.to_lowercase()),
                "visible failure message must not overclaim (found {forbidden:?} in {show_text:?})"
            );
        }
        assert!(
            show_text.to_lowercase().contains("not current")
                || show_text.to_lowercase().contains("stale"),
            "visible failure message must read as a freshness signal, got {show_text:?}"
        );
        assert!(
            show_text.to_lowercase().contains("does not mean")
                || show_text
                    .to_lowercase()
                    .contains("not mean this file is safe"),
            "visible failure message must make clear absence of diagnostics is not a safety \
             claim, got {show_text:?}"
        );

        // The last successful diagnostics must survive the failed refresh
        // untouched: hovering the same position returns the same result.
        let hover_after = backend
            .hover(HoverParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: lib_uri },
                    position,
                },
                work_done_progress_params: Default::default(),
            })
            .await
            .map_err(|err| format!("hover failed: {err:?}"))?;
        assert_eq!(
            hover_after, hover_before,
            "a failed refresh must preserve the last successful diagnostics, not blank them"
        );

        Ok(())
    })
}

#[test]
fn did_change_does_not_trigger_analysis_by_default() {
    assert!(!should_refresh_on_change(&LspConfig::default()));
}

/// Drift-lock: non-actionable cards must not appear in LSP diagnostics.
///
/// The non-actionable classes (GuardedAndWitnessed, Suppressed, BaselineKnown)
/// represent resolved or policy-suppressed states; surfacing them as IDE
/// diagnostics is noise with no required action. This test verifies the filter
/// added in `diagnostics_by_uri` (issue #1593).
///
/// WitnessMismatch was previously listed here but is NOW actionable (issue
/// #1602): a saved receipt whose tool does not match any routed witness tool is
/// a live "fix your receipt" condition, not a resolved state. See the positive
/// arm below for its drift-lock coverage.
///
/// Cards are constructed programmatically by cloning a real fixture card and
/// overriding the class field — the same pattern recommended by the verify pass
/// (mirrors `domain::coverage` tests) — so no new fixture or calibration entry
/// is needed for the non-actionable classes.
#[test]
fn non_actionable_cards_produce_no_lsp_diagnostic() -> Result<(), Box<dyn Error>> {
    let (root, base_output) = fixture_output("raw_pointer_alignment")?;
    let base_card = base_output
        .cards
        .first()
        .ok_or("fixture must have at least one card")?;
    let non_actionable_classes = [
        ReviewClass::GuardedAndWitnessed,
        ReviewClass::Suppressed,
        ReviewClass::BaselineKnown,
    ];
    for class in non_actionable_classes {
        let class_str = class.as_str();
        let mut card = base_card.clone();
        card.class = class;
        let output = AnalyzeOutput {
            cards: vec![card],
            ..base_output.clone()
        };
        let diagnostics = diagnostics_by_uri(&root, &output);
        assert!(
            diagnostics.is_empty(),
            "non-actionable class {class_str} produced an LSP diagnostic — it should be filtered out",
        );
    }
    Ok(())
}

/// Drift-lock (positive arm): actionable cards must still appear in LSP diagnostics.
///
/// Verifies that the filter in `diagnostics_by_uri` does not accidentally suppress
/// actionable cards (issue #1593).
///
/// WitnessMismatch is included here (issue #1602): a saved receipt whose tool
/// does not match any routed witness tool is a live "fix your receipt" condition
/// and must be visible as an IDE diagnostic. This arm would fail if
/// `is_actionable()` were reverted to exclude WitnessMismatch.
#[test]
fn actionable_cards_produce_lsp_diagnostic() -> Result<(), Box<dyn Error>> {
    let (root, base_output) = fixture_output("raw_pointer_alignment")?;
    let base_card = base_output
        .cards
        .first()
        .ok_or("fixture must have at least one card")?;
    let actionable_classes = [
        ReviewClass::ContractMissing,
        ReviewClass::GuardMissing,
        ReviewClass::GuardedUnwitnessed,
        ReviewClass::ReachableUnwitnessed,
        ReviewClass::UnsafeUnreached,
        ReviewClass::WitnessMismatch,
        ReviewClass::RequiresLoom,
        ReviewClass::RequiresSanitizer,
        ReviewClass::RequiresKaniOrCrux,
        ReviewClass::MiriUnsupported,
        ReviewClass::StaticUnknown,
    ];
    for class in actionable_classes {
        let class_str = class.as_str();
        let mut card = base_card.clone();
        card.class = class;
        let output = AnalyzeOutput {
            cards: vec![card],
            ..base_output.clone()
        };
        let diagnostics = diagnostics_by_uri(&root, &output);
        assert!(
            !diagnostics.is_empty(),
            "actionable class {class_str} produced no LSP diagnostic — it should be included",
        );
    }
    Ok(())
}
