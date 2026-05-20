use crate::api::AnalyzeOutput;
use crate::domain::{Confidence, Priority, ReviewCard};
use crate::util::path_display;
use serde::Serialize;

const TRUST_BOUNDARY: &str = "Static unsafe contract review only; this is not a proof of memory safety, not UB-free status, and not a Miri result unless a witness receipt is attached.";
const MAX_PLANNED_COMMENTS: usize = 3;

pub(crate) fn render(output: &AnalyzeOutput) -> String {
    render_pretty(&CommentPlan::from(output))
}

fn render_pretty(value: &impl Serialize) -> String {
    match serde_json::to_string_pretty(value) {
        Ok(text) => text,
        Err(err) => format!("{{\n  \"error\": \"comment plan serialization failed: {err}\"\n}}"),
    }
}

#[derive(Serialize)]
struct CommentPlan {
    schema_version: String,
    tool: String,
    mode: &'static str,
    policy: &'static str,
    comments: Vec<PlannedComment>,
    trust_boundary: &'static str,
}

impl From<&AnalyzeOutput> for CommentPlan {
    fn from(output: &AnalyzeOutput) -> Self {
        Self {
            schema_version: output.schema_version.clone(),
            tool: output.tool.clone(),
            mode: "plan_only",
            policy: output.policy.as_str(),
            comments: output
                .cards
                .iter()
                .filter(|card| should_plan_comment(card))
                .take(MAX_PLANNED_COMMENTS)
                .map(PlannedComment::from)
                .collect(),
            trust_boundary: TRUST_BOUNDARY,
        }
    }
}

#[derive(Serialize)]
struct PlannedComment {
    card_id: String,
    path: String,
    line: usize,
    class: &'static str,
    priority: &'static str,
    confidence: &'static str,
    operation_family: &'static str,
    selection_reason: &'static str,
    body: String,
}

impl From<&ReviewCard> for PlannedComment {
    fn from(card: &ReviewCard) -> Self {
        Self {
            card_id: card.id.0.clone(),
            path: path_display(&card.site.location.file),
            line: card.site.location.line,
            class: card.class.as_str(),
            priority: card.priority.as_str(),
            confidence: card.confidence.as_str(),
            operation_family: card.operation.family.as_str(),
            selection_reason: selection_reason(card),
            body: comment_body(card),
        }
    }
}

fn should_plan_comment(card: &ReviewCard) -> bool {
    card.class.is_actionable()
        && (matches!(card.priority, Priority::High) || matches!(card.confidence, Confidence::High))
        && !matches!(card.confidence, Confidence::Low | Confidence::Unknown)
}

fn selection_reason(card: &ReviewCard) -> &'static str {
    if matches!(card.confidence, Confidence::High) {
        "actionable high-confidence review card"
    } else {
        "actionable high-priority review card"
    }
}

fn comment_body(card: &ReviewCard) -> String {
    let mut body = String::new();
    body.push_str(&format!(
        "`unsafe-review` found `{}` for `{}`.\n\n",
        card.class.as_str(),
        card.operation.family.as_str()
    ));
    body.push_str(&format!("Missing evidence: {}\n\n", missing_summary(card)));
    body.push_str(&format!("Next action: {}\n\n", card.next_action.summary));
    if let Some(route) = card.routes.first() {
        body.push_str(&format!(
            "Witness route: `{}` because {}.\n\n",
            route.kind.as_str(),
            route.reason
        ));
    }
    body.push_str("Trust boundary: static unsafe contract review only; not memory-safety proof, not UB-free status, and not a Miri result unless a witness receipt is attached.");
    body
}

fn missing_summary(card: &ReviewCard) -> String {
    if card.missing.is_empty() {
        return "No missing evidence recorded".to_string();
    }
    card.missing
        .iter()
        .map(|missing| missing.message.as_str())
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{AnalysisMode, AnalyzeInput, DiffSource, PolicyMode, Scope, analyze};
    use crate::domain::CardId;
    use std::path::PathBuf;

    #[test]
    fn comment_plan_projects_high_signal_actionable_cards() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let value = parse_json(&render(&output))?;

        assert_eq!(value["mode"], "plan_only");
        assert_eq!(value["comments"].as_array().map_or(0, Vec::len), 1);
        assert_eq!(value["comments"][0]["class"], "guard_missing");
        assert_eq!(value["comments"][0]["path"], "src/lib.rs");
        assert_eq!(value["comments"][0]["operation_family"], "raw_pointer_read");
        assert!(
            value["comments"][0]["body"]
                .as_str()
                .unwrap_or("")
                .contains("not a Miri result")
        );
        Ok(())
    }

    #[test]
    fn comment_plan_empty_output_has_no_comments() -> Result<(), String> {
        let output = fixture_output("safe_code_no_cards")?;
        let value = parse_json(&render(&output))?;

        assert_eq!(value["comments"].as_array().map_or(1, Vec::len), 0);
        assert!(
            value["trust_boundary"]
                .as_str()
                .unwrap_or("")
                .contains("not UB-free status")
        );
        Ok(())
    }

    #[test]
    fn comment_plan_caps_inline_candidates() -> Result<(), String> {
        let mut output = fixture_output("raw_pointer_alignment")?;
        let template = output
            .cards
            .first()
            .ok_or_else(|| "raw pointer fixture should emit a card".to_string())?
            .clone();
        output.cards = (0..(MAX_PLANNED_COMMENTS + 2))
            .map(|index| {
                let mut card = template.clone();
                card.id = CardId(format!("UR-comment-plan-cap-{index}-c1"));
                card
            })
            .collect();

        let value = parse_json(&render(&output))?;
        let comments = value["comments"]
            .as_array()
            .ok_or_else(|| "comments should be an array".to_string())?;

        assert_eq!(comments.len(), MAX_PLANNED_COMMENTS);
        assert_eq!(comments[0]["card_id"], "UR-comment-plan-cap-0-c1");
        assert_eq!(
            comments[MAX_PLANNED_COMMENTS - 1]["card_id"],
            "UR-comment-plan-cap-2-c1"
        );
        assert!(comments.iter().all(|comment| {
            comment["body"]
                .as_str()
                .unwrap_or("")
                .contains("not memory-safety proof")
        }));
        Ok(())
    }

    fn fixture_output(name: &str) -> Result<AnalyzeOutput, String> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name);
        analyze(AnalyzeInput {
            root: root.clone(),
            scope: Scope::Diff,
            diff: DiffSource::File(root.join("change.diff")),
            mode: AnalysisMode::Draft,
            policy: PolicyMode::Advisory,
            include_unchanged_tests: true,
            max_cards: None,
        })
    }

    fn parse_json(text: &str) -> Result<serde_json::Value, String> {
        serde_json::from_str(text).map_err(|err| format!("JSON parse failed: {err}"))
    }
}
