use serde_json::{Value, json};
use tower_lsp_server::ls_types::{CodeActionOrCommand, Command, Diagnostic, Position};
use unsafe_review_core::{AnalyzeOutput, CardId, ReviewCard, collect_context};

use super::TRUST_BOUNDARY;
use super::diagnostics::find_card_at_position;
use super::{CMD_OPEN_TEST, CMD_PACKET, CMD_REFRESH, CMD_WITNESS_COMMAND, CMD_WITNESS_ROUTE};

pub(super) fn code_actions_for(
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

fn card_code_actions(card: &ReviewCard) -> Vec<CodeActionOrCommand> {
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

pub(super) fn execute_card_command(
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
