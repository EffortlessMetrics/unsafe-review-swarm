use tower_lsp_server::ls_types::{
    Diagnostic, Hover, HoverContents, MarkupContent, MarkupKind, Position,
};
use unsafe_review_core::{AnalyzeOutput, render_lsp_hover};

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
            value: render_lsp_hover(card),
        }),
        range: None,
    })
}
