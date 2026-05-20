use tower_lsp_server::ls_types::{
    Diagnostic, Hover, HoverContents, MarkupContent, MarkupKind, Position,
};
use unsafe_review_core::AnalyzeOutput;

use super::TRUST_BOUNDARY;
use super::diagnostics::find_card_at_position;

pub(super) fn hover_for(
    output: Option<&AnalyzeOutput>,
    diagnostics: &[Diagnostic],
    pos: Position,
) -> Option<Hover> {
    let card = find_card_at_position(output?, diagnostics, pos)?;
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: format!(
                "### unsafe-review: {}\n\nCard: `{}`\n\nOperation: `{}`\n\nSuggested next action:\n{}\n\nTrust boundary:\n{}",
                card.class.as_str(),
                &card.id.0,
                card.operation.family.as_str(),
                card.next_action.summary,
                TRUST_BOUNDARY
            ),
        }),
        range: None,
    })
}
