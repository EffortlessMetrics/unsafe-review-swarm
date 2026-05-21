use crate::api::AnalyzeOutput;
use serde::Serialize;

use super::model::CommentPlan;

pub(crate) fn render(output: &AnalyzeOutput) -> String {
    render_pretty(&CommentPlan::from(output))
}

fn render_pretty(value: &impl Serialize) -> String {
    match serde_json::to_string_pretty(value) {
        Ok(text) => text,
        Err(err) => format!("{{\n  \"error\": \"comment plan serialization failed: {err}\"\n}}"),
    }
}
