use crate::domain::ReviewCard;
use crate::util::path_display;

use super::{
    LspCodeAction, LspCodeActionPayload, LspPosition, LspRange, TRUST_BOUNDARY, range_for,
};

pub(super) fn for_card(card: &ReviewCard) -> Vec<LspCodeAction<'_>> {
    let path = path_display(&card.site.location.file);
    let range = range_for(card);
    let mut actions = vec![
        agent_packet_action(card, &path, &range),
        witness_route_action(card, &path, &range),
    ];
    actions.extend(
        card.related_tests
            .first()
            .map(|test| related_test_action(card, test)),
    );
    actions.extend(
        card.next_action
            .verify_commands
            .first()
            .map(|command| witness_command_action(card, command, path, range)),
    );
    actions
}

fn agent_packet_action<'a>(
    card: &'a ReviewCard,
    path: &str,
    range: &LspRange,
) -> LspCodeAction<'a> {
    LspCodeAction {
        card_id: &card.id.0,
        path: path.to_string(),
        range: range.clone(),
        title: format!("Copy unsafe-review packet for {}", card.id.0),
        kind: "quickfix",
        command: "unsafe-review.copyAgentPacket",
        payload: LspCodeActionPayload {
            kind: "unsafe-review.agent_packet",
            card_id: &card.id.0,
            proof_path: card.proof_path.as_str(),
            file: None,
            line: None,
            name: None,
            command: None,
            trust_boundary: TRUST_BOUNDARY,
        },
        arguments: vec![card.id.0.clone()],
    }
}

fn witness_route_action<'a>(
    card: &'a ReviewCard,
    path: &str,
    range: &LspRange,
) -> LspCodeAction<'a> {
    LspCodeAction {
        card_id: &card.id.0,
        path: path.to_string(),
        range: range.clone(),
        title: "Explain unsafe-review witness route".to_string(),
        kind: "quickfix",
        command: "unsafe-review.explainWitnessRoute",
        payload: LspCodeActionPayload {
            kind: "unsafe-review.witness_route",
            card_id: &card.id.0,
            proof_path: card.proof_path.as_str(),
            file: None,
            line: None,
            name: None,
            command: None,
            trust_boundary: TRUST_BOUNDARY,
        },
        arguments: vec![card.id.0.clone()],
    }
}

fn related_test_action<'a>(
    card: &'a ReviewCard,
    test: &'a crate::domain::RelatedTest,
) -> LspCodeAction<'a> {
    LspCodeAction {
        card_id: &card.id.0,
        path: test.file.clone(),
        range: single_character_range(test.line),
        title: format!("Open related test {}", test.name),
        kind: "quickfix",
        command: "unsafe-review.openRelatedTest",
        payload: LspCodeActionPayload {
            kind: "unsafe-review.related_test",
            card_id: &card.id.0,
            proof_path: card.proof_path.as_str(),
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
    }
}

fn witness_command_action<'a>(
    card: &'a ReviewCard,
    command: &'a str,
    path: String,
    range: LspRange,
) -> LspCodeAction<'a> {
    LspCodeAction {
        card_id: &card.id.0,
        path,
        range,
        title: "Copy witness command (does not run)".to_string(),
        kind: "quickfix",
        command: "unsafe-review.copyWitnessCommand",
        payload: LspCodeActionPayload {
            kind: "unsafe-review.witness_command",
            card_id: &card.id.0,
            proof_path: card.proof_path.as_str(),
            file: None,
            line: None,
            name: None,
            command: Some(command),
            trust_boundary: TRUST_BOUNDARY,
        },
        arguments: vec![command.to_string()],
    }
}

fn single_character_range(line: usize) -> LspRange {
    let line = line.saturating_sub(1);
    LspRange {
        start: LspPosition { line, character: 0 },
        end: LspPosition { line, character: 1 },
    }
}
