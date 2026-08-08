use super::*;
use crate::api::{AnalysisMode, AnalyzeInput, DiffSource, PolicyMode, Scope, analyze};
use crate::domain::ReviewClass;
use std::path::PathBuf;

#[test]
fn lsp_projection_is_parseable_and_read_only() -> Result<(), String> {
    let output = fixture_output("raw_pointer_alignment")?;
    let value = parse_json(&render(&output))?;

    assert_eq!(value["schema_version"], "0.2");
    assert_eq!(value["tool"], "unsafe-review");
    assert_eq!(
        value["analysis"]["analysis_id"],
        output.analysis_identity.analysis_id
    );
    assert_eq!(
        value["analysis"]["generation"],
        output.analysis_identity.generation
    );
    assert_eq!(value["analysis"]["scope"], "diff");
    assert_eq!(value["analysis"]["state"], "current");
    assert!(value["analysis"].get("base_commit").is_none());
    assert!(value["analysis"].get("document_version").is_none());
    assert_eq!(value["mode"], "read_only_projection");
    assert_eq!(value["policy"], "advisory");
    assert_eq!(value["status"]["state"], "actionable");
    assert_eq!(value["status"]["cards"], 1);
    assert_eq!(value["status"]["open_actionable_gaps"], 1);
    assert_eq!(value["status"]["high_priority_cards"], 1);
    assert!(
        value["status"]["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("not UB-free status")
    );
    assert_eq!(value["diagnostics"][0]["source"], "unsafe-review");
    assert_eq!(value["diagnostics"][0]["path"], "src/lib.rs");
    assert_eq!(
        value["diagnostics"][0]["operation"],
        "unsafe { ptr.cast::<Header>().read() }"
    );
    assert_eq!(
        value["diagnostics"][0]["operation_family"],
        "raw_pointer_read"
    );
    assert_eq!(
        value["diagnostics"][0]["required_safety_conditions"][0]["key"],
        "pointer-live"
    );
    assert!(
        value["diagnostics"][0]["required_safety_conditions"][0]["description"]
            .as_str()
            .unwrap_or("")
            .contains("pointer is live")
    );
    assert_eq!(
        value["diagnostics"][0]["evidence_summary"]["contract"]["state"],
        "present"
    );
    assert!(
        value["diagnostics"][0]["evidence_summary"]["contract"]["summary"]
            .as_str()
            .unwrap_or("")
            .contains("SAFETY")
    );
    assert_eq!(
        value["diagnostics"][0]["evidence_summary"]["discharge"]["state"],
        "missing"
    );
    assert_eq!(
        value["diagnostics"][0]["coverage"]["contract_coverage"],
        "present"
    );
    assert_eq!(
        value["diagnostics"][0]["coverage"]["guard_coverage"],
        "missing"
    );
    assert_eq!(
        value["diagnostics"][0]["coverage"]["test_reach_coverage"],
        "missing"
    );
    assert_eq!(
        value["diagnostics"][0]["coverage"]["witness_receipt_coverage"],
        "missing"
    );
    assert_eq!(
        value["diagnostics"][0]["coverage"]["manual_context"],
        "absent"
    );
    assert_eq!(value["diagnostics"][0]["coverage"]["baseline_state"], "new");
    assert_eq!(
        value["diagnostics"][0]["coverage"]["outcome_movement"],
        "regressed"
    );
    assert_eq!(
        value["diagnostics"][0]["coverage"]["comment_plan_status"],
        "selected"
    );
    assert_eq!(
        value["diagnostics"][0]["coverage"]["agent_lsp_readiness"],
        "ready"
    );
    assert!(
        value["diagnostics"][0]["evidence_summary"]["reach_limitation"]
            .as_str()
            .unwrap_or("")
            .contains("not proof")
    );
    assert_eq!(
        value["diagnostics"][0]["obligation_evidence"][0]["key"],
        "pointer-live"
    );
    assert!(
        value["diagnostics"][0]["obligation_evidence"]
            .as_array()
            .is_some_and(|items| {
                items.iter().any(|item| {
                    item["key"] == "alignment"
                        && item["discharge"]["state"] == "missing"
                        && item["witness"]["state"] == "missing"
                })
            })
    );
    assert_eq!(value["diagnostics"][0]["severity"], 2);
    assert!(
        value["diagnostics"][0]["next_action"]
            .as_str()
            .unwrap_or("")
            .contains("Add or expose local guards")
    );
    assert_eq!(value["diagnostics"][0]["witness_routes"][0]["kind"], "miri");
    assert!(
        value["diagnostics"][0]["verify_commands"][0]
            .as_str()
            .unwrap_or("")
            .contains("cargo +nightly miri test read_header")
    );
    assert!(
        value["hovers"][0]["contents"]
            .as_str()
            .unwrap_or("")
            .contains("Card: `UR-")
    );
    assert!(
        value["hovers"][0]["contents"]
            .as_str()
            .unwrap_or("")
            .contains("Location: src/lib.rs:8")
    );
    assert!(
        value["hovers"][0]["contents"]
            .as_str()
            .unwrap_or("")
            .contains("Required safety conditions")
    );
    assert!(
        value["hovers"][0]["contents"]
            .as_str()
            .unwrap_or("")
            .contains("Why this card exists")
    );
    assert!(
        value["hovers"][0]["contents"]
            .as_str()
            .unwrap_or("")
            .contains("Operation: `unsafe { ptr.cast::<Header>().read() }`")
    );
    assert!(
        value["hovers"][0]["contents"]
            .as_str()
            .unwrap_or("")
            .contains("Relevant hazard families")
    );
    assert!(
        value["hovers"][0]["contents"]
            .as_str()
            .unwrap_or("")
            .contains("`alignment`")
    );
    assert!(
        value["hovers"][0]["contents"]
            .as_str()
            .unwrap_or("")
            .contains("Evidence found")
    );
    assert!(
        value["hovers"][0]["contents"]
            .as_str()
            .unwrap_or("")
            .contains("Contract [present]")
    );
    assert!(
        value["hovers"][0]["contents"]
            .as_str()
            .unwrap_or("")
            .contains("Guard/discharge [missing]")
    );
    assert!(
        value["hovers"][0]["contents"]
            .as_str()
            .unwrap_or("")
            .contains("Witness [missing]")
    );
    assert!(
        value["hovers"][0]["contents"]
            .as_str()
            .unwrap_or("")
            .contains("Evidence missing")
    );
    assert!(
        value["hovers"][0]["contents"]
            .as_str()
            .unwrap_or("")
            .contains("What would resolve this")
    );
    assert!(
        value["hovers"][0]["contents"]
            .as_str()
            .unwrap_or("")
            .contains("What would not resolve this")
    );
    assert!(
        value["hovers"][0]["contents"]
            .as_str()
            .unwrap_or("")
            .contains("SAFETY:` comment alone does not discharge missing guard evidence")
    );
    assert!(
        value["hovers"][0]["contents"]
            .as_str()
            .unwrap_or("")
            .contains("Verify commands")
    );
    assert!(
        value["hovers"][0]["contents"]
            .as_str()
            .unwrap_or("")
            .contains("does not prove the unsafe site executed")
    );
    assert!(
        value["hovers"][0]["contents"]
            .as_str()
            .unwrap_or("")
            .contains(
                "Do not widen unsafe scope, suppress the card, or change unrelated unsafe code"
            )
    );
    let card_id = value["diagnostics"][0]["card_id"]
        .as_str()
        .ok_or("diagnostic card_id should be a string")?;
    assert_eq!(
        value["hovers"][0]["card_id"],
        value["diagnostics"][0]["card_id"]
    );
    assert_eq!(value["hovers"][0]["path"], value["diagnostics"][0]["path"]);
    assert_eq!(
        value["hovers"][0]["range"],
        value["diagnostics"][0]["range"]
    );
    assert_eq!(value["hovers"][0]["analysis"], value["analysis"]);
    let hover_contents = value["hovers"][0]["contents"].as_str().unwrap_or("");
    assert!(hover_contents.contains("Handoff commands"));
    assert!(hover_contents.contains(&format!("unsafe-review explain {card_id}")));
    assert!(hover_contents.contains(&format!("unsafe-review context {card_id} --json")));
    assert_eq!(value["code_actions"][0]["action_id"], "agent-packet");
    assert_eq!(
        value["code_actions"][0]["kind"],
        "quickfix.unsafeReview.agentPacket"
    );
    assert_eq!(
        value["code_actions"][0]["payload"]["card_id"],
        value["diagnostics"][0]["card_id"]
    );
    assert_eq!(
        value["code_actions"][0]["payload"]["analysis"],
        value["analysis"]
    );
    assert_eq!(
        value["code_actions"][0]["command"]["arguments"]["analysis"],
        value["analysis"]
    );
    assert!(value["code_actions"].as_array().is_some_and(|actions| {
        actions
            .iter()
            .any(|action| action["action_id"] == "related-test")
    }));
    assert!(value["code_actions"].as_array().is_some_and(|actions| {
        actions.iter().any(|action| {
            action["action_id"] == "related-test"
                && action["payload"]["card_id"] == value["diagnostics"][0]["card_id"]
                && action["command"]["arguments"]["file"] == "src/lib.rs"
                && action["command"]["arguments"]["line"] == 16
                && action["command"]["arguments"]["name"] == "reads_header"
        })
    }));
    assert!(value["code_actions"].as_array().is_some_and(|actions| {
        actions.iter().any(|action| {
            action["action_id"] == "witness-command"
                && action["title"] == "Copy witness command (does not run)"
                && action["payload"]["card_id"] == value["diagnostics"][0]["card_id"]
                && action["command"]["command"] == "unsafe-review.collectWitnessCommand"
                && action["trust_boundary"]
                    .as_str()
                    .unwrap_or("")
                    .contains("not UB-free status")
        })
    }));
    assert!(
        !serde_json::to_string(&value["code_actions"])
            .map_err(|err| format!("render code actions failed: {err}"))?
            .contains("\"edit\"")
    );
    assert!(
        value["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("not a site-execution claim")
    );
    Ok(())
}

#[test]
fn saved_lsp_and_agent_packet_share_analysis_identity() -> Result<(), String> {
    let output = fixture_output("raw_pointer_alignment")?;
    let lsp = parse_json(&render(&output))?;
    let card = output.cards.first().ok_or("fixture should emit a card")?;
    let packet = parse_json(&crate::output::agent::render_with_output(&output, card))?;
    assert_eq!(lsp["analysis"], packet["analysis"]);
    assert_eq!(
        packet["analysis"]["analysis_id"],
        output.analysis_identity.analysis_id
    );
    assert_eq!(
        packet["analysis"]["generation"],
        output.analysis_identity.generation
    );
    Ok(())
}

#[test]
fn editor_agent_and_review_surfaces_preserve_card_guidance() -> Result<(), String> {
    let output = fixture_output("raw_pointer_alignment")?;
    let card = output.cards.first().ok_or("fixture should emit one card")?;
    let lsp = parse_json(&render(&output))?;
    let agent = parse_json(&crate::output::agent::render_with_output(&output, card))?;
    let sarif = parse_json(&crate::api::render_sarif(&output))?;
    let pr = crate::api::render_pr_summary(&output);

    let diagnostic = &lsp["diagnostics"][0];
    let sarif_properties = &sarif["runs"][0]["results"][0]["properties"];
    let verify_command = card
        .next_action
        .verify_commands
        .first()
        .ok_or("fixture card should expose a verification command")?;

    for surface_card_id in [
        diagnostic["card_id"].as_str(),
        agent["card_id"].as_str(),
        sarif_properties["cardId"].as_str(),
    ] {
        assert_eq!(surface_card_id, Some(card.id.0.as_str()));
    }
    for surface_class in [
        diagnostic["code"].as_str(),
        agent["card"]["class"].as_str(),
        sarif_properties["class"].as_str(),
    ] {
        assert_eq!(surface_class, Some(card.class.as_str()));
    }
    for surface_family in [
        diagnostic["operation_family"].as_str(),
        agent["context"]["operation_family"].as_str(),
        sarif_properties["operationFamily"].as_str(),
    ] {
        assert_eq!(surface_family, Some(card.operation.family.as_str()));
    }

    assert_eq!(diagnostic["path"], "src/lib.rs");
    assert_eq!(
        diagnostic["range"]["start"]["line"],
        card.site.location.line - 1
    );
    assert_eq!(
        sarif["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
        "src/lib.rs"
    );
    assert_eq!(
        sarif["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["region"]["startLine"],
        card.site.location.line
    );

    assert_eq!(diagnostic["next_action"], card.next_action.summary);
    assert_eq!(agent["task"], card.next_action.summary);
    assert_eq!(sarif_properties["nextAction"], card.next_action.summary);
    assert!(pr.contains(&card.id.0));
    assert!(pr.contains(card.class.as_str()));
    assert!(pr.contains(&format!(
        "- Operation family: `{}`",
        card.operation.family.as_str()
    )));
    assert!(pr.contains(&card.next_action.summary));

    assert!(
        diagnostic["verify_commands"]
            .as_array()
            .is_some_and(|commands| commands.iter().any(|command| command == verify_command))
    );
    assert!(agent["confirmation_cue"]["build_this_first"]["command"] == *verify_command);
    assert!(
        sarif_properties["verifyCommands"]
            .as_array()
            .is_some_and(|commands| commands.iter().any(|command| command == verify_command))
    );

    let trust_boundary = lsp["trust_boundary"].as_str().unwrap_or("");
    assert!(!trust_boundary.is_empty());
    assert_eq!(agent["trust_boundary"], trust_boundary);
    assert_eq!(sarif_properties["trustBoundary"], trust_boundary);
    assert!(pr.contains(trust_boundary));
    Ok(())
}

#[test]
fn lsp_projection_empty_output_has_no_editor_items() -> Result<(), String> {
    let output = fixture_output("safe_code_no_cards")?;
    let value = parse_json(&render(&output))?;

    assert_eq!(value["status"]["state"], "quiet");
    assert_eq!(value["status"]["cards"], 0);
    assert_eq!(value["status"]["open_actionable_gaps"], 0);
    assert!(
        value["status"]["message"]
            .as_str()
            .unwrap_or("")
            .contains("No unsafe-review cards")
    );
    assert_eq!(value["diagnostics"].as_array().map_or(1, Vec::len), 0);
    assert_eq!(value["hovers"].as_array().map_or(1, Vec::len), 0);
    assert_eq!(value["code_actions"].as_array().map_or(1, Vec::len), 0);
    assert!(
        value["trust_boundary"]
            .as_str()
            .unwrap_or("")
            .contains("not UB-free status")
    );
    Ok(())
}

/// Regression guard: LSP severity must derive from card CLASS, not priority.
///
/// `guard_missing` → LSP 2 (Warning) regardless of whether the card has
/// High, Medium, or Low priority.  The fixture produces a `guard_missing` card
/// which also happens to have High priority, but the assertion documents that
/// the class is the deterministic source — not the priority.
#[test]
fn lsp_diagnostic_severity_derives_from_class_not_priority() -> Result<(), String> {
    let output = fixture_output("raw_pointer_alignment")?;
    // Confirm the fixture yields a guard_missing card (class is the driver).
    assert!(
        output
            .cards
            .iter()
            .any(|c| c.class == ReviewClass::GuardMissing),
        "fixture must produce a guard_missing card for this regression test"
    );
    let value = parse_json(&render(&output))?;
    // guard_missing → sarif_level "warning" → lsp_severity 2 (Warning).
    assert_eq!(
        value["diagnostics"][0]["severity"], 2,
        "guard_missing class must produce LSP severity 2 (Warning)"
    );
    Ok(())
}

/// Drift-lock: for classes that are non-actionable, the LSP severity must be
/// 4 (Hint) — the lowest non-error severity — confirming they do not mislead
/// the editor into showing Warning-level decorations.
#[test]
fn lsp_non_actionable_classes_produce_hint_severity() {
    let non_actionable = [
        ReviewClass::GuardedAndWitnessed,
        ReviewClass::BaselineKnown,
        ReviewClass::Suppressed,
    ];
    for class in non_actionable {
        assert_eq!(
            class.lsp_severity(),
            4,
            "non-actionable class {} must produce LSP severity 4 (Hint)",
            class.as_str()
        );
    }
}

#[test]
fn canonical_editor_diagnostic_exposes_card_semantics() -> Result<(), String> {
    let output = fixture_output("raw_pointer_alignment")?;
    let card = &output.cards[0];
    let statuses = crate::output::comment_plan::card_statuses(&output);
    let status = statuses
        .get(&card.id)
        .copied()
        .ok_or("fixture card should have a comment-plan status")?;
    let diagnostic = super::projection::EditorDiagnostic::from_with_status(
        card,
        status,
        output.coverage_snapshot.get(&card.id.0),
    );

    assert_eq!(diagnostic.card_id, card.id.0);
    assert_eq!(diagnostic.code, card.class.as_str());
    assert_eq!(diagnostic.severity, card.class.lsp_severity());
    assert_eq!(diagnostic.operation, card.operation.expression);
    assert_eq!(diagnostic.operation_family, card.operation.family.as_str());
    assert_eq!(diagnostic.coverage.baseline_state, "new");
    assert_eq!(diagnostic.coverage.outcome_movement, "regressed");
    assert_eq!(diagnostic.coverage.comment_plan_status, "selected");
    assert_eq!(diagnostic.coverage.agent_lsp_readiness, "ready");
    assert!(diagnostic.trust_boundary.contains("not UB-free status"));
    Ok(())
}

#[test]
fn saved_projection_serializes_the_canonical_diagnostic_without_rederivation() -> Result<(), String>
{
    let output = fixture_output("raw_pointer_alignment")?;
    let projection = project_editor(&output);
    let rendered = parse_json(&render(&output))?;
    let serialized = serde_json::to_value(&projection.diagnostics[0])
        .map_err(|err| format!("canonical diagnostic serialization failed: {err}"))?;

    assert_eq!(serialized, rendered["diagnostics"][0]);
    assert_eq!(projection.diagnostics[0].card_id, output.cards[0].id.0);
    assert_eq!(
        projection.diagnostics[0].code,
        output.cards[0].class.as_str()
    );
    assert_eq!(
        projection.diagnostics[0].severity,
        output.cards[0].class.lsp_severity()
    );

    let mut changed = projection.diagnostics[0].clone();
    changed.next_action = "test-only saved DTO override".to_string();
    assert_eq!(changed.next_action, "test-only saved DTO override");
    assert_ne!(changed.next_action, output.cards[0].next_action.summary);
    Ok(())
}

#[test]
fn editor_range_width_uses_utf16_code_units() {
    assert_eq!(super::projection::utf16_width("ascii"), 5);
    assert_eq!(super::projection::utf16_width("a😀b"), 4);
}

fn fixture_output(name: &str) -> Result<AnalyzeOutput, String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name);
    analyze(AnalyzeInput {
        root: root.clone(),
        scope: Scope::Diff,
        diff: DiffSource::File(root.join("change.diff")),
        mode: AnalysisMode::Draft,
        policy: PolicyMode::Advisory,
        include_unchanged_tests: true,
        max_cards: None,
    })
}

fn parse_json(text: &str) -> Result<serde_json::Value, String> {
    serde_json::from_str(text).map_err(|err| format!("JSON parse failed: {err}"))
}
