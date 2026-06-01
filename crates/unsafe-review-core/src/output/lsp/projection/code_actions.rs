use crate::domain::ReviewCard;
use crate::util::path_display;
use serde::Serialize;

use super::{LspPosition, LspRange, TRUST_BOUNDARY, range_for};

#[derive(Serialize)]
pub(super) struct LspCodeAction<'a> {
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

pub(super) fn code_actions(card: &ReviewCard) -> Vec<LspCodeAction<'_>> {
    let path = path_display(&card.site.location.file);
    let range = range_for(card);
    let mut actions = default_card_actions(card, &path, &range);
    if let Some(test) = card.related_tests.first() {
        actions.push(related_test_action(card, test));
    }
    if let Some(command) = card.next_action.verify_commands.first() {
        actions.push(witness_command_action(card, &path, &range, command));
    }
    actions
}

fn default_card_actions<'a>(
    card: &'a ReviewCard,
    path: &str,
    range: &LspRange,
) -> Vec<LspCodeAction<'a>> {
    vec![
        card_action(
            card,
            path.to_string(),
            range.clone(),
            format!("Copy unsafe-review packet for {}", card.id.0),
            "unsafe-review.copyAgentPacket",
            payload(card, "unsafe-review.agent_packet"),
            vec![card.id.0.clone()],
        ),
        card_action(
            card,
            path.to_string(),
            range.clone(),
            "Explain unsafe-review witness route".to_string(),
            "unsafe-review.explainWitnessRoute",
            payload(card, "unsafe-review.witness_route"),
            vec![card.id.0.clone()],
        ),
    ]
}

fn related_test_action<'a>(
    card: &'a ReviewCard,
    test: &'a crate::domain::RelatedTest,
) -> LspCodeAction<'a> {
    card_action(
        card,
        test.file.clone(),
        single_character_range(test.line),
        format!("Open related test {}", test.name),
        "unsafe-review.openRelatedTest",
        LspCodeActionPayload {
            kind: "unsafe-review.related_test",
            card_id: &card.id.0,
            file: Some(&test.file),
            line: Some(test.line),
            name: Some(&test.name),
            command: None,
            trust_boundary: TRUST_BOUNDARY,
        },
        vec![
            card.id.0.clone(),
            test.file.clone(),
            test.line.to_string(),
            test.name.clone(),
        ],
    )
}

fn witness_command_action<'a>(
    card: &'a ReviewCard,
    path: &str,
    range: &LspRange,
    command: &'a str,
) -> LspCodeAction<'a> {
    card_action(
        card,
        path.to_string(),
        range.clone(),
        "Copy witness command (does not run)".to_string(),
        "unsafe-review.copyWitnessCommand",
        LspCodeActionPayload {
            kind: "unsafe-review.witness_command",
            card_id: &card.id.0,
            file: None,
            line: None,
            name: None,
            command: Some(command),
            trust_boundary: TRUST_BOUNDARY,
        },
        vec![command.to_string()],
    )
}

fn card_action<'a>(
    card: &'a ReviewCard,
    path: String,
    range: LspRange,
    title: String,
    command: &'static str,
    payload: LspCodeActionPayload<'a>,
    arguments: Vec<String>,
) -> LspCodeAction<'a> {
    LspCodeAction {
        card_id: &card.id.0,
        path,
        range,
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

fn single_character_range(line: usize) -> LspRange {
    let lsp_line = line.saturating_sub(1);
    LspRange {
        start: LspPosition {
            line: lsp_line,
            character: 0,
        },
        end: LspPosition {
            line: lsp_line,
            character: 1,
        },
    }
}
