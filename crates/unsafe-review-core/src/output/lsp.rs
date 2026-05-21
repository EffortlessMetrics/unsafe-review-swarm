use crate::api::{AnalyzeOutput, Scope};
use crate::domain::{Priority, ReviewCard, WitnessRoute};
use crate::util::path_display;
use serde::Serialize;

const TRUST_BOUNDARY: &str = "Static unsafe contract review only; this is not a proof of memory safety, not UB-free status, and not a Miri result unless a witness receipt is attached.";

pub(crate) fn render(output: &AnalyzeOutput) -> String {
    render_pretty(&LspProjection::from(output))
}

fn render_pretty(value: &impl Serialize) -> String {
    match serde_json::to_string_pretty(value) {
        Ok(text) => text,
        Err(err) => format!("{{\n  \"error\": \"lsp projection serialization failed: {err}\"\n}}"),
    }
}

#[derive(Serialize)]
struct LspProjection<'a> {
    schema_version: &'a str,
    tool: &'a str,
    mode: &'static str,
    policy: &'static str,
    scope: &'static str,
    status: LspStatus,
    diagnostics: Vec<LspDiagnostic<'a>>,
    hovers: Vec<LspHover<'a>>,
    code_actions: Vec<LspCodeAction<'a>>,
    trust_boundary: &'static str,
}

impl<'a> From<&'a AnalyzeOutput> for LspProjection<'a> {
    fn from(output: &'a AnalyzeOutput) -> Self {
        Self {
            schema_version: &output.schema_version,
            tool: &output.tool,
            mode: "read_only_projection",
            policy: output.policy.as_str(),
            scope: scope_label(output),
            status: status_for(output),
            diagnostics: output.cards.iter().map(LspDiagnostic::from).collect(),
            hovers: output.cards.iter().map(LspHover::from).collect(),
            code_actions: output.cards.iter().flat_map(code_actions).collect(),
            trust_boundary: TRUST_BOUNDARY,
        }
    }
}

#[derive(Serialize)]
struct LspDiagnostic<'a> {
    card_id: &'a str,
    path: String,
    range: LspRange,
    severity: usize,
    source: &'static str,
    code: &'static str,
    message: String,
    operation: &'a str,
    operation_family: &'static str,
    hazards: Vec<&'static str>,
    missing_evidence: Vec<&'a str>,
    next_action: &'a str,
    witness_routes: Vec<LspWitnessRoute<'a>>,
    verify_commands: &'a [String],
    trust_boundary: &'static str,
}

impl<'a> From<&'a ReviewCard> for LspDiagnostic<'a> {
    fn from(card: &'a ReviewCard) -> Self {
        Self {
            card_id: &card.id.0,
            path: path_display(&card.site.location.file),
            range: range_for(card),
            severity: severity_for(card),
            source: "unsafe-review",
            code: card.class.as_str(),
            message: format!(
                "{}: {}",
                card.operation.family.as_str(),
                card.next_action.summary
            ),
            operation: &card.operation.expression,
            operation_family: card.operation.family.as_str(),
            hazards: card.hazards.iter().map(|hazard| hazard.as_str()).collect(),
            missing_evidence: card
                .missing
                .iter()
                .map(|missing| missing.message.as_str())
                .collect(),
            next_action: &card.next_action.summary,
            witness_routes: card.routes.iter().map(LspWitnessRoute::from).collect(),
            verify_commands: &card.next_action.verify_commands,
            trust_boundary: TRUST_BOUNDARY,
        }
    }
}

#[derive(Serialize)]
struct LspWitnessRoute<'a> {
    kind: &'static str,
    reason: &'a str,
    command: Option<&'a str>,
    required: bool,
}

impl<'a> From<&'a WitnessRoute> for LspWitnessRoute<'a> {
    fn from(route: &'a WitnessRoute) -> Self {
        Self {
            kind: route.kind.as_str(),
            reason: &route.reason,
            command: route.command.as_deref(),
            required: route.required,
        }
    }
}

#[derive(Serialize)]
struct LspHover<'a> {
    card_id: &'a str,
    path: String,
    position: LspPosition,
    contents: String,
    trust_boundary: &'static str,
}

impl<'a> From<&'a ReviewCard> for LspHover<'a> {
    fn from(card: &'a ReviewCard) -> Self {
        Self {
            card_id: &card.id.0,
            path: path_display(&card.site.location.file),
            position: position_for(card),
            contents: hover_contents(card),
            trust_boundary: TRUST_BOUNDARY,
        }
    }
}

#[derive(Serialize)]
struct LspCodeAction<'a> {
    card_id: &'a str,
    path: String,
    range: LspRange,
    title: String,
    kind: &'static str,
    command: &'static str,
    payload: LspCodeActionPayload<'a>,
    arguments: Vec<String>,
}

#[derive(Serialize)]
struct LspCodeActionPayload<'a> {
    kind: &'static str,
    card_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<&'a str>,
    trust_boundary: &'static str,
}

#[derive(Serialize)]
struct LspStatus {
    state: &'static str,
    cards: usize,
    open_actionable_gaps: usize,
    high_priority_cards: usize,
    message: String,
    trust_boundary: &'static str,
}

#[derive(Serialize, Clone)]
struct LspRange {
    start: LspPosition,
    end: LspPosition,
}

#[derive(Serialize, Clone)]
struct LspPosition {
    line: usize,
    character: usize,
}

fn code_actions(card: &ReviewCard) -> Vec<LspCodeAction<'_>> {
    let path = path_display(&card.site.location.file);
    let range = range_for(card);
    let mut actions = vec![
        LspCodeAction {
            card_id: &card.id.0,
            path: path.clone(),
            range: range.clone(),
            title: format!("Copy unsafe-review packet for {}", card.id.0),
            kind: "quickfix",
            command: "unsafe-review.copyAgentPacket",
            payload: LspCodeActionPayload {
                kind: "unsafe-review.agent_packet",
                card_id: &card.id.0,
                file: None,
                line: None,
                name: None,
                command: None,
                trust_boundary: TRUST_BOUNDARY,
            },
            arguments: vec![card.id.0.clone()],
        },
        LspCodeAction {
            card_id: &card.id.0,
            path: path.clone(),
            range: range.clone(),
            title: "Explain unsafe-review witness route".to_string(),
            kind: "quickfix",
            command: "unsafe-review.explainWitnessRoute",
            payload: LspCodeActionPayload {
                kind: "unsafe-review.witness_route",
                card_id: &card.id.0,
                file: None,
                line: None,
                name: None,
                command: None,
                trust_boundary: TRUST_BOUNDARY,
            },
            arguments: vec![card.id.0.clone()],
        },
    ];
    if let Some(test) = card.related_tests.first() {
        actions.push(LspCodeAction {
            card_id: &card.id.0,
            path: test.file.clone(),
            range: LspRange {
                start: LspPosition {
                    line: test.line.saturating_sub(1),
                    character: 0,
                },
                end: LspPosition {
                    line: test.line.saturating_sub(1),
                    character: 1,
                },
            },
            title: format!("Open related test {}", test.name),
            kind: "quickfix",
            command: "unsafe-review.openRelatedTest",
            payload: LspCodeActionPayload {
                kind: "unsafe-review.related_test",
                card_id: &card.id.0,
                file: Some(&test.file),
                line: Some(test.line),
                name: Some(&test.name),
                command: None,
                trust_boundary: TRUST_BOUNDARY,
            },
            arguments: vec![
                card.id.0.clone(),
                test.file.clone(),
                test.line.to_string(),
                test.name.clone(),
            ],
        });
    }
    if let Some(command) = card.next_action.verify_commands.first() {
        actions.push(LspCodeAction {
            card_id: &card.id.0,
            path,
            range,
            title: "Copy recommended witness command".to_string(),
            kind: "quickfix",
            command: "unsafe-review.copyWitnessCommand",
            payload: LspCodeActionPayload {
                kind: "unsafe-review.witness_command",
                card_id: &card.id.0,
                file: None,
                line: None,
                name: None,
                command: Some(command),
                trust_boundary: TRUST_BOUNDARY,
            },
            arguments: vec![command.clone()],
        });
    }
    actions
}

fn hover_contents(card: &ReviewCard) -> String {
    let mut text = String::new();
    text.push_str(&format!(
        "unsafe-review `{}` for `{}` operation `{}`\n\n",
        card.class.as_str(),
        card.operation.family.as_str(),
        card.operation.expression
    ));
    text.push_str(&format!(
        "Card: `{}`; priority `{}`; confidence `{}`\n\n",
        card.id,
        card.priority.as_str(),
        card.confidence.as_str()
    ));
    text.push_str("Required safety conditions:\n");
    for obligation in &card.obligations {
        text.push_str(&format!("- {}\n", obligation.description));
    }
    text.push_str("\nEvidence summary:\n");
    text.push_str(&format!(
        "- Contract [{}]: {}\n",
        present_label(card.contract.present),
        card.contract.summary
    ));
    text.push_str(&format!(
        "- Guard/discharge [{}]: {}\n",
        present_label(card.discharge.present),
        card.discharge.summary
    ));
    text.push_str(&format!(
        "- Reach [{}]: {}\n",
        card.reach.state, card.reach.summary
    ));
    text.push_str(&format!(
        "- Witness [{}]: {}\n",
        present_label(card.witness.present),
        card.witness.summary
    ));
    text.push_str("\nMissing evidence:\n");
    if card.missing.is_empty() {
        text.push_str("- none recorded\n");
    } else {
        for missing in &card.missing {
            text.push_str(&format!("- {}\n", missing.message));
        }
    }
    text.push_str("\nNext action:\n");
    text.push_str(&format!("- {}\n", card.next_action.summary));
    if !card.next_action.verify_commands.is_empty() {
        text.push_str("\nVerify commands:\n");
        for command in &card.next_action.verify_commands {
            text.push_str(&format!("- `{command}`\n"));
        }
    }
    if let Some(route) = card.routes.first() {
        text.push_str(&format!(
            "\nWitness route: `{}` because {}.\n",
            route.kind.as_str(),
            route.reason
        ));
    }
    text.push_str(
        "\nReach note: static related-test evidence does not prove the unsafe site executed.\n",
    );
    text.push_str("\nTrust boundary: ");
    text.push_str(TRUST_BOUNDARY);
    text
}

fn present_label(present: bool) -> &'static str {
    if present { "present" } else { "missing" }
}

fn range_for(card: &ReviewCard) -> LspRange {
    let start = position_for(card);
    let end = LspPosition {
        line: start.line,
        character: start
            .character
            .saturating_add(card.site.snippet.chars().count().max(1)),
    };
    LspRange { start, end }
}

fn position_for(card: &ReviewCard) -> LspPosition {
    LspPosition {
        line: card.site.location.line.saturating_sub(1),
        character: card.site.location.column.saturating_sub(1),
    }
}

fn severity_for(card: &ReviewCard) -> usize {
    if matches!(card.priority, Priority::High) {
        2
    } else {
        3
    }
}

fn status_for(output: &AnalyzeOutput) -> LspStatus {
    let high_priority_cards = output
        .cards
        .iter()
        .filter(|card| matches!(card.priority, Priority::High))
        .count();
    let state = if output.cards.is_empty() {
        "quiet"
    } else if output.summary.open_actionable_gaps > 0 {
        "actionable"
    } else {
        "informational"
    };
    let message = match state {
        "quiet" => "No unsafe-review cards for this scope".to_string(),
        _ => format!(
            "{} unsafe-review card(s), {} open actionable gap(s)",
            output.summary.cards, output.summary.open_actionable_gaps
        ),
    };
    LspStatus {
        state,
        cards: output.summary.cards,
        open_actionable_gaps: output.summary.open_actionable_gaps,
        high_priority_cards,
        message,
        trust_boundary: TRUST_BOUNDARY,
    }
}

fn scope_label(output: &AnalyzeOutput) -> &'static str {
    match output.scope {
        Scope::Diff => "diff",
        Scope::Repo => "repo",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{AnalysisMode, AnalyzeInput, DiffSource, PolicyMode, analyze};
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
        assert_eq!(value["diagnostics"][0]["severity"], 2);
        assert!(
            value["diagnostics"][0]["next_action"]
                .as_str()
                .unwrap_or("")
                .contains("Add or expose the local guard")
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
                .contains("Required safety conditions")
        );
        assert!(
            value["hovers"][0]["contents"]
                .as_str()
                .unwrap_or("")
                .contains("operation `unsafe { ptr.cast::<Header>().read() }`")
        );
        assert!(
            value["hovers"][0]["contents"]
                .as_str()
                .unwrap_or("")
                .contains("Evidence summary")
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
                .contains("Next action")
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
                .contains("not a Miri result")
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
}
