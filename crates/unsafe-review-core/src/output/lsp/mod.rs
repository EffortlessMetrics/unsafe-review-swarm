mod projection;
#[cfg(test)]
mod tests;

use crate::api::AnalyzeOutput;
use serde::Serialize;

pub use projection::EditorProjection;
pub(crate) use projection::project_editor;

pub(crate) fn render(output: &AnalyzeOutput) -> String {
    render_pretty(&project_editor(output))
}

fn render_pretty(value: &impl Serialize) -> String {
    match serde_json::to_string_pretty(value) {
        Ok(text) => text,
        Err(err) => format!("{{\n  \"error\": \"lsp projection serialization failed: {err}\"\n}}"),
    }
}
