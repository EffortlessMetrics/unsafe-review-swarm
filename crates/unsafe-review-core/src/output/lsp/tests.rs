use super::*;
use crate::api::{AnalysisMode, AnalyzeInput, DiffSource, PolicyMode, Scope, analyze};
use crate::domain::ReviewClass;
use std::path::PathBuf;

#[test]
fn lsp_projection_is_parseable_and_read_only() -> Result<(), String> {
    let output = fixture_output("raw_pointer_alignment")?;
    let value = parse_json(&render(&output))?;

    assert_eq!(value["schema_version"], "0.1");
    assert_eq!(value["tool"], "unsafe-review");
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
    let hover_contents = value["hovers"][0]["contents"].as_str().unwrap_or("");
    assert!(hover_contents.contains("Handoff commands"));
    assert!(hover_contents.contains(&format!("unsafe-review explain {card_id}")));
    assert!(hover_contents.contains(&format!("unsafe-review context {card_id} --json")));
    assert_eq!(
        value["code_actions"][0]["command"],
        "unsafe-review.copyAgentPacket"
    );
    assert_eq!(
        value["code_actions"][0]["payload"]["kind"],
        "unsafe-review.agent_packet"
    );
    assert_eq!(
        value["code_actions"][0]["payload"]["card_id"],
        value["diagnostics"][0]["card_id"]
    );
    assert!(value["code_actions"][0]["arguments"].is_array());
    assert!(value["code_actions"].as_array().is_some_and(|actions| {
        actions
            .iter()
            .any(|action| action["command"] == "unsafe-review.openRelatedTest")
    }));
    assert!(value["code_actions"].as_array().is_some_and(|actions| {
        actions.iter().any(|action| {
            action["command"] == "unsafe-review.openRelatedTest"
                && action["payload"]["kind"] == "unsafe-review.related_test"
                && action["payload"]["card_id"] == value["diagnostics"][0]["card_id"]
                && action["payload"]["file"] == "src/lib.rs"
                && action["payload"]["line"] == 3
                && action["payload"]["name"] == "read_header"
        })
    }));
    assert!(value["code_actions"].as_array().is_some_and(|actions| {
        actions.iter().any(|action| {
            action["command"] == "unsafe-review.copyWitnessCommand"
                && action["title"] == "Copy witness command (does not run)"
                && action["payload"]["kind"] == "unsafe-review.witness_command"
                && action["payload"]["card_id"] == value["diagnostics"][0]["card_id"]
                && action["payload"]["command"]
                    .as_str()
                    .unwrap_or("")
                    .contains("cargo +nightly miri test read_header")
                && action["payload"]["trust_boundary"]
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
