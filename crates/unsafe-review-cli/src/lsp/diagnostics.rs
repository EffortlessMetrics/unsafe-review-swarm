use std::collections::BTreeMap;
use std::path::Path;

use tower_lsp_server::ls_types::{
    Diagnostic, DiagnosticSeverity, NumberOrString, Position, Range, Uri,
};
use unsafe_review_core::{AnalyzeOutput, Priority, ReviewCard};

use super::uri::uri_from_path;

mod data;

pub(super) fn diagnostics_by_uri(
    root: &Path,
    output: &AnalyzeOutput,
) -> BTreeMap<Uri, Vec<Diagnostic>> {
    let mut map = BTreeMap::new();
    for card in &output.cards {
        if !card.class.is_actionable() {
            continue;
        }
        let path = root.join(&card.site.location.file);
        let Some(uri) = uri_from_path(path) else {
            continue;
        };
        map.entry(uri)
            .or_insert_with(Vec::new)
            .push(diagnostic_from_card(card));
    }
    map
}

fn diagnostic_from_card(card: &ReviewCard) -> Diagnostic {
    let line = card.site.location.line.saturating_sub(1) as u32;
    let start = Position::new(line, card.site.location.column.saturating_sub(1) as u32);
    let end = Position::new(line, start.character + lsp_width(&card.site.snippet));
    Diagnostic {
        range: Range::new(start, end),
        severity: Some(diagnostic_severity(card)),
        code: Some(NumberOrString::String(card.class.as_str().to_string())),
        source: Some("unsafe-review".into()),
        message: diagnostic_message(card),
        data: Some(data::build_diagnostic_data(card)),
        ..Default::default()
    }
}

fn diagnostic_severity(card: &ReviewCard) -> DiagnosticSeverity {
    if matches!(card.priority, Priority::High) {
        DiagnosticSeverity::WARNING
    } else {
        DiagnosticSeverity::INFORMATION
    }
}

fn diagnostic_message(card: &ReviewCard) -> String {
    format!(
        "{}: {}",
        card.operation.family.as_str(),
        card.next_action.summary
    )
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
