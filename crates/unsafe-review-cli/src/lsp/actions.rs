use serde_json::{Value, json};
use tower_lsp_server::ls_types::{
    CodeAction, CodeActionDisabled, CodeActionKind, CodeActionOrCommand, Command, Diagnostic,
    Position,
};
use unsafe_review_core::{
    AnalyzeOutput, CardId, EditorActionApplicability, EditorActionArguments, actions_for_card,
    collect_context,
};

use super::TRUST_BOUNDARY;
use super::diagnostics::{diagnostic_card_id, range_contains};
use super::{CMD_OPEN_TEST, CMD_PACKET, CMD_REFRESH, CMD_WITNESS_COMMAND, CMD_WITNESS_ROUTE};

#[derive(Clone, Copy, Default)]
pub(super) struct ActionClientSupport {
    pub(super) literals: bool,
    pub(super) disabled: bool,
    pub(super) data: bool,
    pub(super) preferred: bool,
}

pub(super) fn code_actions_for(
    output: Option<&AnalyzeOutput>,
    diagnostics: &[Diagnostic],
    pos: Position,
    support: ActionClientSupport,
) -> Vec<CodeActionOrCommand> {
    let mut actions = vec![CodeActionOrCommand::Command(Command {
        title: "Refresh unsafe-review diagnostics".into(),
        command: CMD_REFRESH.into(),
        arguments: None,
    })];
    let Some(output) = output else {
        return actions;
    };
    let mut matched = diagnostics
        .iter()
        .filter(|diagnostic| range_contains(diagnostic.range, pos))
        .filter_map(|diagnostic| diagnostic_card_id(diagnostic).map(|id| (id, diagnostic)))
        .collect::<Vec<_>>();
    matched.sort_by(|(left, _), (right, _)| left.cmp(right));
    matched.dedup_by(|(left, _), (right, _)| left == right);
    for (card_id, diagnostic) in matched {
        let Ok(contracts) = actions_for_card(output, &card_id) else {
            continue;
        };
        if !support.literals {
            actions.extend(contracts.into_iter().filter_map(|contract| {
                contract.applicability.is_available().then(|| {
                    CodeActionOrCommand::Command(Command {
                        title: contract.title,
                        command: contract.command.command,
                        arguments: serde_json::to_value(contract.command.arguments)
                            .ok()
                            .map(|argument| vec![argument]),
                    })
                })
            }));
            continue;
        }
        actions.extend(contracts.into_iter().filter_map(|contract| {
            let arguments = serde_json::to_value(&contract.command.arguments).ok()?;
            let data = serde_json::to_value(&contract).ok()?;
            let disabled = match &contract.applicability {
                EditorActionApplicability::Available => None,
                EditorActionApplicability::Disabled { reason, .. } => Some(CodeActionDisabled {
                    reason: reason.clone(),
                }),
            };
            if disabled.is_some() && !support.disabled {
                return None;
            }
            let command = disabled.is_none().then(|| Command {
                title: contract.title.clone(),
                command: contract.command.command.clone(),
                arguments: Some(vec![arguments]),
            });
            Some(CodeActionOrCommand::CodeAction(CodeAction {
                title: contract.title,
                kind: Some(CodeActionKind::from(contract.kind)),
                diagnostics: Some(vec![diagnostic.clone()]),
                edit: None,
                command,
                is_preferred: support.preferred.then_some(contract.is_preferred),
                disabled,
                data: support.data.then_some(data),
            }))
        }));
    }
    actions
}

pub(super) fn execute_card_command(
    command: &str,
    arguments: &[Value],
    output: &AnalyzeOutput,
) -> Option<Value> {
    let typed: EditorActionArguments = serde_json::from_value(arguments.first()?.clone()).ok()?;
    if typed.analysis != output.analysis_identity {
        return None;
    }
    let card_id = typed.card_id;
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

pub(super) fn validate_command_arguments(
    command: &str,
    arguments: &[Value],
    output: &AnalyzeOutput,
) -> Result<(), &'static str> {
    if arguments.len() != 1 {
        return Err("invalid_action_arguments");
    }
    let typed: EditorActionArguments =
        serde_json::from_value(arguments[0].clone()).map_err(|_err| "invalid_action_arguments")?;
    if typed.analysis != output.analysis_identity {
        return Err("stale_analysis");
    }
    if !output.cards.iter().any(|card| card.id.0 == typed.card_id) {
        return Err("unknown_card");
    }
    let actions = actions_for_card(output, &typed.card_id).map_err(|_err| "unknown_card")?;
    if !actions
        .iter()
        .any(|action| action.command.command == command && action.applicability.is_available())
    {
        return Err("action_unavailable");
    }
    Ok(())
}
