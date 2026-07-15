use std::collections::BTreeMap;
use std::path::Path;

use tower_lsp_server::ls_types::{
    Diagnostic, DiagnosticSeverity, NumberOrString, Position, Range, Uri,
};
use unsafe_review_core::{
    AnalyzeOutput, EditorDiagnostic, ReviewCard, project_actionable_editor_diagnostics,
};

use super::uri::uri_from_path;

pub(super) fn diagnostics_by_uri(
    root: &Path,
    output: &AnalyzeOutput,
) -> BTreeMap<Uri, Vec<Diagnostic>> {
    let mut map = BTreeMap::new();
    for diagnostic in project_actionable_editor_diagnostics(output) {
        let path = root.join(&diagnostic.path);
        let Some(uri) = uri_from_path(path) else {
            continue;
        };
        map.entry(uri)
            .or_insert_with(Vec::new)
            .push(diagnostic_from_editor_diagnostic(&diagnostic));
    }
    map
}

fn diagnostic_from_editor_diagnostic(diagnostic: &EditorDiagnostic) -> Diagnostic {
    Diagnostic {
        range: range_from_editor_diagnostic(diagnostic),
        severity: lsp_severity(diagnostic.severity),
        code: Some(NumberOrString::String(diagnostic.code.clone())),
        source: Some(diagnostic.source.clone()),
        message: diagnostic.message.clone(),
        data: serde_json::to_value(diagnostic).ok(),
        ..Default::default()
    }
}

fn range_from_editor_diagnostic(diagnostic: &EditorDiagnostic) -> Range {
    Range::new(
        Position::new(
            diagnostic.range.start.line as u32,
            diagnostic.range.start.character as u32,
        ),
        Position::new(
            diagnostic.range.end.line as u32,
            diagnostic.range.end.character as u32,
        ),
    )
}

fn lsp_severity(severity: usize) -> Option<DiagnosticSeverity> {
    match severity {
        2 => Some(DiagnosticSeverity::WARNING),
        3 => Some(DiagnosticSeverity::INFORMATION),
        4 => Some(DiagnosticSeverity::HINT),
        _ => None,
    }
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
