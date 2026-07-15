mod projection;
#[cfg(test)]
mod tests;

use crate::api::AnalyzeOutput;
use crate::domain::ReviewCard;
use serde::Serialize;

pub use projection::{
    EditorCoverageBlock, EditorDiagnostic, EditorEvidenceState, EditorEvidenceSummary,
    EditorObligationEvidence, EditorPosition, EditorProjection, EditorRange, EditorReachEvidence,
    EditorSafetyCondition, EditorSimpleEvidence, EditorWitnessRoute,
};
pub(crate) use projection::{
    project_actionable_editor_diagnostics, project_editor, project_editor_diagnostics,
};

pub(crate) fn render(output: &AnalyzeOutput) -> String {
    render_pretty(&project_editor(output))
}

pub(crate) fn render_hover(card: &ReviewCard) -> String {
    projection::render_hover(card)
}

fn render_pretty(value: &impl Serialize) -> String {
    match serde_json::to_string_pretty(value) {
        Ok(text) => text,
        Err(err) => format!("{{\n  \"error\": \"lsp projection serialization failed: {err}\"\n}}"),
    }
}
