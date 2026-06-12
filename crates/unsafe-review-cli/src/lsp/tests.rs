use std::collections::BTreeSet;
use std::error::Error;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use tower_lsp_server::ls_types::{
    CodeActionOrCommand, CodeActionProviderCapability, ExecuteCommandOptions, HoverContents,
    HoverProviderCapability, Position,
};
use unsafe_review_core::{
    AnalysisMode, AnalyzeInput, AnalyzeOutput, DiffSource, PolicyMode, ReviewClass, Scope, analyze,
};

use super::actions::{code_actions_for, execute_card_command};
use super::capabilities::server_capabilities;
use super::config::{LspConfig, parse_config, should_refresh_on_change};
use super::diagnostics::{diagnostic_card_id, diagnostics_by_uri};
use super::hover::hover_for;
use super::state::clear_uris_for_failure;
use super::uri::uri_from_path;
use super::{CMD_PACKET, CMD_REFRESH, CMD_WITNESS_COMMAND};

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
    let Some(ExecuteCommandOptions { commands, .. }) = capabilities.execute_command_provider else {
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
    assert!(actions.iter().any(|action| {
        matches!(
            action,
            CodeActionOrCommand::Command(command)
                if command.command == CMD_WITNESS_COMMAND
                    && command.title.contains("does not run")
        )
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
    assert!(packet.contains("\"confirmation_cue\""));
    assert!(packet.contains("\"build_this_first\""));
    assert!(packet.contains("\"minimal_repro\""));
    assert!(packet.contains("attach a matching receipt"));
    assert!(packet.contains("unsafe-review did not run this command"));
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
