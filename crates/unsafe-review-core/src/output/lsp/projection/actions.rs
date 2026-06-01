use crate::domain::ReviewCard;
use crate::util::path_display;

use super::{
    LspCodeAction, LspCodeActionPayload, LspPosition, LspRange, TRUST_BOUNDARY, range_for,
};

pub(super) fn for_card(card: &ReviewCard) -> Vec<LspCodeAction<'_>> {
    let card_action_scope = CardActionScope::new(card);
    let mut actions = vec![
        copy_agent_packet_action(card, &card_action_scope),
        explain_route_action(card, &card_action_scope),
    ];
    actions.extend(related_test_action(card));
    actions.extend(copy_witness_command_action(card, &card_action_scope));
    actions
}

struct CardActionScope {
    path: String,
    range: LspRange,
}

impl CardActionScope {
    fn new(card: &ReviewCard) -> Self {
        Self {
            path: path_display(&card.site.location.file),
            range: range_for(card),
        }
    }
}

fn copy_agent_packet_action<'a>(
    card: &'a ReviewCard,
    scope: &CardActionScope,
) -> LspCodeAction<'a> {
    card_action(
        card,
        scope,
        format!("Copy unsafe-review packet for {}", card.id.0),
        "unsafe-review.copyAgentPacket",
        payload(card, "unsafe-review.agent_packet"),
        vec![card.id.0.clone()],
    )
}

fn explain_route_action<'a>(card: &'a ReviewCard, scope: &CardActionScope) -> LspCodeAction<'a> {
    card_action(
        card,
        scope,
        "Explain unsafe-review witness route".to_string(),
        "unsafe-review.explainWitnessRoute",
        payload(card, "unsafe-review.witness_route"),
        vec![card.id.0.clone()],
    )
}

fn related_test_action(card: &ReviewCard) -> Option<LspCodeAction<'_>> {
    let test = card.related_tests.first()?;
    Some(LspCodeAction {
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
    })
}

fn copy_witness_command_action<'a>(
    card: &'a ReviewCard,
    scope: &CardActionScope,
) -> Option<LspCodeAction<'a>> {
    let command = card.next_action.verify_commands.first()?;
    Some(card_action(
        card,
        scope,
        "Copy witness command (does not run)".to_string(),
        "unsafe-review.copyWitnessCommand",
        LspCodeActionPayload {
            command: Some(command),
            ..payload(card, "unsafe-review.witness_command")
        },
        vec![command.clone()],
    ))
}

fn card_action<'a>(
    card: &'a ReviewCard,
    scope: &CardActionScope,
    title: String,
    command: &'static str,
    payload: LspCodeActionPayload<'a>,
    arguments: Vec<String>,
) -> LspCodeAction<'a> {
    LspCodeAction {
        card_id: &card.id.0,
        path: scope.path.clone(),
        range: scope.range.clone(),
        title,
        kind: "quickfix",
        command,
        payload,
        arguments,
    }
}

fn payload<'a>(card: &'a ReviewCard, kind: &'static str) -> LspCodeActionPayload<'a> {
    LspCodeActionPayload {
        kind,
        card_id: &card.id.0,
        file: None,
        line: None,
        name: None,
        command: None,
        trust_boundary: TRUST_BOUNDARY,
    }
}
