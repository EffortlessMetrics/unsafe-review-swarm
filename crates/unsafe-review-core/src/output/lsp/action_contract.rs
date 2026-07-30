//! Canonical, transport-neutral editor action contract (issue #1909 PR1).
//!
//! Live LSP, saved LSP, and editor adapters consume this vocabulary rather
//! than deriving action identity, readiness wording, or applicability.

use super::EditorRange;
use crate::api::AnalyzeOutput;
use crate::domain::ReviewCard;
use crate::freshness::AnalysisIdentity;
use crate::output::REVIEWCARD_TRUST_BOUNDARY;
use serde::{Deserialize, Serialize};

pub const ACTION_AGENT_PACKET: &str = "agent-packet";
pub const ACTION_WITNESS_ROUTE: &str = "witness-route";
pub const ACTION_WITNESS_COMMAND: &str = "witness-command";
pub const ACTION_RELATED_TEST: &str = "related-test";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorActionContract {
    pub action_id: String,
    pub title: String,
    pub kind: String,
    pub diagnostic: EditorActionDiagnostic,
    pub payload: EditorActionPayload,
    pub command: EditorActionCommand,
    pub applicability: EditorActionApplicability,
    pub is_preferred: bool,
    pub command_only: bool,
    pub trust_boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorActionDiagnostic {
    pub card_id: String,
    pub path: String,
    pub range: EditorRange,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorActionPayload {
    pub action_id: String,
    pub card_id: String,
    pub analysis: AnalysisIdentity,
    pub agent_readiness: EditorActionReadiness,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_packet: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditorActionReadiness {
    ReadyForAgent,
    RequiresHumanReview,
    RequiresWitnessReceipt,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorActionCommand {
    pub command: String,
    pub arguments: EditorActionArguments,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorActionArguments {
    pub card_id: String,
    pub analysis: AnalysisIdentity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum EditorActionApplicability {
    Available,
    Disabled { reason_code: String, reason: String },
}

impl EditorActionApplicability {
    pub fn is_available(&self) -> bool {
        matches!(self, Self::Available)
    }
}

pub fn actions_for_card(
    output: &AnalyzeOutput,
    card_id: &str,
) -> Result<Vec<EditorActionContract>, String> {
    let card = output
        .cards
        .iter()
        .find(|card| card.id.0 == card_id)
        .ok_or_else(|| format!("card `{card_id}` is not part of this analysis"))?;
    let diagnostic = super::project_editor_diagnostics(output)
        .into_iter()
        .find(|diagnostic| diagnostic.card_id == card_id)
        .ok_or_else(|| format!("card `{card_id}` has no canonical editor diagnostic"))?;
    let readiness = action_readiness(&diagnostic.coverage.agent_lsp_readiness)?;
    let diagnostic_identity = EditorActionDiagnostic {
        card_id: diagnostic.card_id.clone(),
        path: diagnostic.path.clone(),
        range: diagnostic.range.clone(),
    };

    Ok(vec![
        action(
            output,
            card,
            diagnostic_identity.clone(),
            ACTION_AGENT_PACKET,
            packet_kind(readiness),
            packet_title(readiness),
            "unsafe-review.collectAgentPacket",
            arguments(output, card, None),
            readiness,
            EditorActionApplicability::Available,
        ),
        action(
            output,
            card,
            diagnostic_identity.clone(),
            ACTION_WITNESS_ROUTE,
            "source.unsafeReview.witnessRoute",
            "Explain unsafe-review witness route".to_string(),
            "unsafe-review.explainWitnessRoute",
            arguments(output, card, None),
            readiness,
            available_if(
                !card.routes.is_empty(),
                "no_witness_route",
                "No witness route is available for this card.",
            ),
        ),
        action(
            output,
            card,
            diagnostic_identity.clone(),
            ACTION_WITNESS_COMMAND,
            "source.unsafeReview.witnessCommand",
            "Copy witness command (does not run)".to_string(),
            "unsafe-review.collectWitnessCommand",
            arguments(output, card, None),
            readiness,
            available_if(
                card.routes.iter().any(|route| route.command.is_some()),
                "no_witness_command",
                "No witness command is available for this card.",
            ),
        ),
        action(
            output,
            card,
            diagnostic_identity,
            ACTION_RELATED_TEST,
            "source.unsafeReview.relatedTest",
            related_test_title(card),
            "unsafe-review.openRelatedTest",
            arguments(output, card, card.related_tests.first()),
            readiness,
            available_if(
                !card.related_tests.is_empty(),
                "no_related_test",
                "No structured related test is available for this card.",
            ),
        ),
    ])
}

#[allow(
    clippy::too_many_arguments,
    reason = "the constructor keeps the closed action vocabulary visible at each call site"
)]
fn action(
    output: &AnalyzeOutput,
    card: &ReviewCard,
    diagnostic: EditorActionDiagnostic,
    action_id: &str,
    kind: &str,
    title: String,
    command: &str,
    arguments: EditorActionArguments,
    readiness: EditorActionReadiness,
    applicability: EditorActionApplicability,
) -> EditorActionContract {
    EditorActionContract {
        action_id: action_id.to_string(),
        title,
        kind: kind.to_string(),
        diagnostic,
        payload: EditorActionPayload {
            action_id: action_id.to_string(),
            card_id: card.id.0.clone(),
            analysis: output.analysis_identity.clone(),
            agent_readiness: readiness,
            agent_packet: (action_id == ACTION_AGENT_PACKET)
                .then(|| crate::output::agent::render_with_output(output, card)),
        },
        command: EditorActionCommand {
            command: command.to_string(),
            arguments,
        },
        applicability,
        is_preferred: false,
        command_only: true,
        trust_boundary: REVIEWCARD_TRUST_BOUNDARY.to_string(),
    }
}

fn available_if(available: bool, reason_code: &str, reason: &str) -> EditorActionApplicability {
    if available {
        EditorActionApplicability::Available
    } else {
        EditorActionApplicability::Disabled {
            reason_code: reason_code.to_string(),
            reason: reason.to_string(),
        }
    }
}

fn action_readiness(readiness: &str) -> Result<EditorActionReadiness, String> {
    match readiness {
        "ready" => Ok(EditorActionReadiness::ReadyForAgent),
        "needs_human" => Ok(EditorActionReadiness::RequiresHumanReview),
        "requires_witness_receipt" => Ok(EditorActionReadiness::RequiresWitnessReceipt),
        "unsupported" => Ok(EditorActionReadiness::Unsupported),
        other => Err(format!("unknown canonical agent readiness `{other}`")),
    }
}

fn packet_title(readiness: EditorActionReadiness) -> String {
    match readiness {
        EditorActionReadiness::ReadyForAgent => {
            "Copy bounded unsafe-review agent packet".to_string()
        }
        _ => "Copy bounded unsafe-review review context (human review required)".to_string(),
    }
}

fn packet_kind(readiness: EditorActionReadiness) -> &'static str {
    match readiness {
        EditorActionReadiness::ReadyForAgent => "quickfix.unsafeReview.agentPacket",
        _ => "source.unsafeReview.reviewContext",
    }
}

fn related_test_title(card: &ReviewCard) -> String {
    card.related_tests.first().map_or_else(
        || "Open related test".to_string(),
        |test| format!("Open related test `{}`", test.name),
    )
}

fn arguments(
    output: &AnalyzeOutput,
    card: &ReviewCard,
    test: Option<&crate::domain::RelatedTest>,
) -> EditorActionArguments {
    EditorActionArguments {
        card_id: card.id.0.clone(),
        analysis: output.analysis_identity.clone(),
        file: test.map(|test| test.file.clone()),
        line: test.map(|test| test.line),
        name: test.map(|test| test.name.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{AnalysisMode, AnalyzeInput, DiffSource, PolicyMode, Scope, analyze};
    use crate::output::lsp::project_editor_diagnostics;
    use std::path::PathBuf;

    #[test]
    fn action_contract_has_stable_vocabulary_and_no_edits() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let diagnostics = project_editor_diagnostics(&output);
        let actions = actions_for_card(&output, &output.cards[0].id.0)?;
        assert_eq!(
            actions
                .iter()
                .map(|action| (
                    action.action_id.as_str(),
                    action.kind.as_str(),
                    action.command.command.as_str(),
                ))
                .collect::<Vec<_>>(),
            vec![
                (
                    ACTION_AGENT_PACKET,
                    "quickfix.unsafeReview.agentPacket",
                    "unsafe-review.collectAgentPacket"
                ),
                (
                    ACTION_WITNESS_ROUTE,
                    "source.unsafeReview.witnessRoute",
                    "unsafe-review.explainWitnessRoute"
                ),
                (
                    ACTION_WITNESS_COMMAND,
                    "source.unsafeReview.witnessCommand",
                    "unsafe-review.collectWitnessCommand"
                ),
                (
                    ACTION_RELATED_TEST,
                    "source.unsafeReview.relatedTest",
                    "unsafe-review.openRelatedTest"
                ),
            ]
        );
        assert!(
            actions
                .iter()
                .all(|action| action.command_only && !action.is_preferred)
        );
        let json = serde_json::to_string(&actions).map_err(|err| err.to_string())?;
        assert!(!json.contains("workspace_edit"));
        assert!(!json.contains("\"edit\""));
        assert!(actions.iter().all(|action| {
            action.payload.card_id == diagnostics[0].card_id
                && action.payload.analysis == output.analysis_identity
                && action.command.arguments.analysis == output.analysis_identity
                && action.command.arguments.card_id == diagnostics[0].card_id
                && action.diagnostic.path == diagnostics[0].path
                && action.diagnostic.range == diagnostics[0].range
        }));
        let packet = actions[0]
            .payload
            .agent_packet
            .as_deref()
            .ok_or("agent-packet action must carry the bounded packet")?;
        let packet: serde_json::Value =
            serde_json::from_str(packet).map_err(|err| err.to_string())?;
        assert_eq!(packet["card_id"], diagnostics[0].card_id);
        let analysis =
            serde_json::to_value(&output.analysis_identity).map_err(|err| err.to_string())?;
        assert_eq!(packet["analysis"], analysis);
        assert!(
            actions[1..]
                .iter()
                .all(|action| action.payload.agent_packet.is_none())
        );
        assert_eq!(actions, actions_for_card(&output, &output.cards[0].id.0)?);
        Ok(())
    }

    #[test]
    fn unavailable_actions_are_precisely_disabled() -> Result<(), String> {
        let mut output = fixture_output("raw_pointer_alignment")?;
        output.cards[0].routes.clear();
        output.cards[0].next_action.verify_commands.clear();
        output.cards[0].related_tests.clear();
        let actions = actions_for_card(&output, &output.cards[0].id.0)?;
        assert!(actions[0].applicability.is_available());
        assert_eq!(disabled_code(&actions[1]), Some("no_witness_route"));
        assert_eq!(disabled_code(&actions[2]), Some("no_witness_command"));
        assert_eq!(disabled_code(&actions[3]), Some("no_related_test"));
        Ok(())
    }

    #[test]
    fn verify_command_without_route_command_is_not_falsely_available() -> Result<(), String> {
        let mut output = fixture_output("raw_pointer_alignment")?;
        output.cards[0].routes.clear();
        assert!(!output.cards[0].next_action.verify_commands.is_empty());
        let actions = actions_for_card(&output, &output.cards[0].id.0)?;
        assert_eq!(disabled_code(&actions[1]), Some("no_witness_route"));
        assert_eq!(disabled_code(&actions[2]), Some("no_witness_command"));
        Ok(())
    }

    #[test]
    fn unknown_card_cannot_construct_split_identity() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let err = match actions_for_card(&output, "foreign-card") {
            Ok(_) => return Err("foreign card must fail".to_string()),
            Err(err) => err,
        };
        assert!(err.contains("not part of this analysis"));
        Ok(())
    }

    #[test]
    fn human_only_cards_never_look_like_automatic_repairs() -> Result<(), String> {
        let output = fixture_output("ffi_missing_boundary_contract")?;
        let actions = actions_for_card(&output, &output.cards[0].id.0)?;
        assert_eq!(
            actions[0].payload.agent_readiness,
            EditorActionReadiness::RequiresHumanReview
        );
        assert!(actions[0].title.contains("human review required"));
        assert!(!actions[0].kind.starts_with("quickfix"));
        assert!(actions.iter().all(|action| !action.is_preferred));
        Ok(())
    }

    fn disabled_code(action: &EditorActionContract) -> Option<&str> {
        match &action.applicability {
            EditorActionApplicability::Available => None,
            EditorActionApplicability::Disabled { reason_code, .. } => Some(reason_code),
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
}
