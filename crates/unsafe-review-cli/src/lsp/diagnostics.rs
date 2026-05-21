use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Value, json};
use tower_lsp_server::ls_types::{
    Diagnostic, DiagnosticSeverity, NumberOrString, Position, Range, Uri,
};
use unsafe_review_core::{AnalyzeOutput, ReviewCard};

use super::TRUST_BOUNDARY;
use super::uri::uri_from_path;

pub(super) fn diagnostics_by_uri(
    root: &Path,
    output: &AnalyzeOutput,
) -> BTreeMap<Uri, Vec<Diagnostic>> {
    let mut map = BTreeMap::new();
    for card in &output.cards {
        let path = root.join(&card.site.location.file);
        let Some(uri) = uri_from_path(path) else {
            continue;
        };
        let line = card.site.location.line.saturating_sub(1) as u32;
        let start = Position::new(line, card.site.location.column.saturating_sub(1) as u32);
        let end = Position::new(line, start.character + lsp_width(&card.site.snippet));
        let diagnostic = Diagnostic {
            range: Range::new(start, end),
            severity: Some(
                if matches!(card.priority, unsafe_review_core::Priority::High) {
                    DiagnosticSeverity::WARNING
                } else {
                    DiagnosticSeverity::INFORMATION
                },
            ),
            code: Some(NumberOrString::String(card.class.as_str().to_string())),
            source: Some("unsafe-review".into()),
            message: format!(
                "{}: {}",
                card.operation.family.as_str(),
                card.next_action.summary
            ),
            data: Some(diagnostic_data(card)),
            ..Default::default()
        };
        map.entry(uri).or_insert_with(Vec::new).push(diagnostic);
    }
    map
}

fn diagnostic_data(card: &ReviewCard) -> Value {
    json!({
        "card_id": &card.id.0,
        "operation": &card.operation.expression,
        "operation_family": card.operation.family.as_str(),
        "hazards": card.hazards.iter().map(|hazard| hazard.as_str()).collect::<Vec<_>>(),
        "required_safety_conditions": card.obligations.iter().map(|obligation| {
            json!({
                "key": &obligation.key,
                "description": &obligation.description
            })
        }).collect::<Vec<_>>(),
        "evidence_summary": {
            "contract": {
                "present": card.contract.present,
                "state": present_label(card.contract.present),
                "summary": &card.contract.summary
            },
            "discharge": {
                "present": card.discharge.present,
                "state": present_label(card.discharge.present),
                "summary": &card.discharge.summary
            },
            "reach": {
                "state": &card.reach.state,
                "summary": &card.reach.summary
            },
            "witness": {
                "present": card.witness.present,
                "state": present_label(card.witness.present),
                "summary": &card.witness.summary
            },
            "reach_limitation": "static reach evidence is not proof that the unsafe site executed"
        },
        "obligation_evidence": card.obligation_evidence.iter().map(|evidence| {
            json!({
                "key": &evidence.obligation.key,
                "description": &evidence.obligation.description,
                "contract": evidence_state(
                    evidence.contract.present,
                    &evidence.contract.state,
                    &evidence.contract.summary
                ),
                "discharge": evidence_state(
                    evidence.discharge.present,
                    &evidence.discharge.state,
                    &evidence.discharge.summary
                ),
                "reach": evidence_state(
                    evidence.reach.present,
                    &evidence.reach.state,
                    &evidence.reach.summary
                ),
                "witness": evidence_state(
                    evidence.witness.present,
                    &evidence.witness.state,
                    &evidence.witness.summary
                )
            })
        }).collect::<Vec<_>>(),
        "missing_evidence": card.missing.iter().map(|missing| missing.message.as_str()).collect::<Vec<_>>(),
        "next_action": &card.next_action.summary,
        "witness_routes": card.routes.iter().map(|route| {
            json!({
                "kind": route.kind.as_str(),
                "reason": &route.reason,
                "command": route.command.as_deref(),
                "required": route.required
            })
        }).collect::<Vec<_>>(),
        "verify_commands": &card.next_action.verify_commands,
        "trust_boundary": TRUST_BOUNDARY
    })
}

fn evidence_state(present: bool, state: &str, summary: &str) -> Value {
    json!({
        "present": present,
        "state": state,
        "summary": summary
    })
}

fn present_label(present: bool) -> &'static str {
    if present { "present" } else { "missing" }
}

pub(super) fn lsp_width(text: &str) -> u32 {
    text.lines()
        .next()
        .unwrap_or(text)
        .chars()
        .map(|c| c.len_utf16() as u32)
        .sum::<u32>()
        .max(1)
}

pub(super) fn find_card_at_position<'a>(
    output: &'a AnalyzeOutput,
    diagnostics: &[Diagnostic],
    pos: Position,
) -> Option<&'a ReviewCard> {
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| range_contains(diagnostic.range, pos))?;
    let card_id = diagnostic_card_id(diagnostic)?;
    output.cards.iter().find(|card| card.id.0 == card_id)
}

pub(super) fn diagnostic_card_id(diagnostic: &Diagnostic) -> Option<String> {
    diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get("card_id"))
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
}

pub(super) fn range_contains(range: Range, pos: Position) -> bool {
    pos.line == range.start.line
        && pos.character >= range.start.character
        && pos.character <= range.end.character
}
