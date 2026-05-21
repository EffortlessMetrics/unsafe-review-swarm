use crate::api::AnalyzeOutput;
use crate::domain::{Confidence, Priority, ReviewCard, WitnessRoute};
use crate::output::{NO_CHANGED_GAPS_LIMITATION, NO_CHANGED_GAPS_MESSAGE};
use crate::util::path_display;
use serde::Serialize;

const TRUST_BOUNDARY: &str = "Static unsafe contract review only; this is not a proof of memory safety, not UB-free status, and not a Miri result unless a witness receipt is attached.";
const PLAN_BOUNDARY: &str = "Plan boundary: artifact-only inline comment candidate; unsafe-review did not post this comment, run witnesses, or make a policy decision.";
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
    #[serde(skip_serializing_if = "Option::is_none")]
    no_changed_gaps: Option<NoChangedGaps>,
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
            no_changed_gaps: (output.summary.open_actionable_gaps == 0).then_some(NoChangedGaps {
                message: NO_CHANGED_GAPS_MESSAGE,
                limitation: NO_CHANGED_GAPS_LIMITATION,
            }),
            trust_boundary: TRUST_BOUNDARY,
        }
    }
}

#[derive(Serialize)]
struct NoChangedGaps {
    message: &'static str,
    limitation: &'static str,
}

#[derive(Serialize)]
struct PlannedComment {
    card_id: String,
    path: String,
    line: usize,
    class: &'static str,
    priority: &'static str,
    confidence: &'static str,
    operation: String,
    operation_family: &'static str,
    witness_routes: Vec<PlannedWitnessRoute>,
    verify_commands: Vec<String>,
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
            operation: card.operation.expression.clone(),
            operation_family: card.operation.family.as_str(),
            witness_routes: card.routes.iter().map(PlannedWitnessRoute::from).collect(),
            verify_commands: card.next_action.verify_commands.clone(),
            selection_reason: selection_reason(card),
            body: comment_body(card),
        }
    }
}

#[derive(Serialize)]
struct PlannedWitnessRoute {
    kind: &'static str,
    reason: String,
    command: Option<String>,
    required: bool,
}

impl From<&WitnessRoute> for PlannedWitnessRoute {
    fn from(route: &WitnessRoute) -> Self {
        Self {
            kind: route.kind.as_str(),
            reason: route.reason.clone(),
            command: route.command.clone(),
            required: route.required,
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
        "`unsafe-review` found `{}` for `{}` (`{}`).\n\n",
        card.class.as_str(),
        one_line(&card.operation.expression),
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
    if let Some(command) = card.next_action.verify_commands.first() {
        body.push_str(&format!("Verify command: `{command}`\n\n"));
    }
    body.push_str(PLAN_BOUNDARY);
    body.push_str("\n\n");
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

fn one_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{AnalysisMode, AnalyzeInput, DiffSource, PolicyMode, Scope, analyze};
    use std::path::PathBuf;

    #[test]
    fn comment_plan_projects_high_signal_actionable_cards() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let value = parse_json(&render(&output))?;

        assert_eq!(value["mode"], "plan_only");
        assert_eq!(value["comments"].as_array().map_or(0, Vec::len), 1);
        assert_eq!(value["comments"][0]["class"], "guard_missing");
        assert_eq!(value["comments"][0]["path"], "src/lib.rs");
        assert_eq!(
            value["comments"][0]["operation"],
            "unsafe { ptr.cast::<Header>().read() }"
        );
        assert_eq!(value["comments"][0]["operation_family"], "raw_pointer_read");
        assert_eq!(value["comments"][0]["witness_routes"][0]["kind"], "miri");
        assert!(
            value["comments"][0]["verify_commands"][0]
                .as_str()
                .unwrap_or("")
                .contains("cargo +nightly miri test read_header")
        );
        assert!(
            value["comments"][0]["body"]
                .as_str()
                .unwrap_or("")
                .contains("unsafe { ptr.cast::<Header>().read() }")
        );
        assert!(
            value["comments"][0]["body"]
                .as_str()
                .unwrap_or("")
                .contains("Verify command: `cargo +nightly miri test read_header`")
        );
        assert!(
            value["comments"][0]["body"]
                .as_str()
                .unwrap_or("")
                .contains("unsafe-review did not post this comment")
        );
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
        assert_eq!(value["no_changed_gaps"]["message"], NO_CHANGED_GAPS_MESSAGE);
        assert_eq!(
            value["no_changed_gaps"]["limitation"],
            NO_CHANGED_GAPS_LIMITATION
        );
        assert!(
            value["trust_boundary"]
                .as_str()
                .unwrap_or("")
                .contains("not UB-free status")
        );
        Ok(())
    }

    #[test]
    fn comment_plan_caps_planned_comments() -> Result<(), String> {
        let mut output = fixture_output("raw_pointer_alignment")?;
        let template = output
            .cards
            .first()
            .cloned()
            .ok_or_else(|| "fixture should emit a card".to_string())?;
        output.cards = (0..5)
            .map(|idx| {
                let mut card = template.clone();
                card.id.0 = format!("card-{idx}");
                card
            })
            .collect();
        output.summary.cards = output.cards.len();
        output.summary.open_actionable_gaps = output.cards.len();

        let value = parse_json(&render(&output))?;

        assert_eq!(
            value["comments"]
                .as_array()
                .ok_or_else(|| "comments should be an array".to_string())?
                .len(),
            MAX_PLANNED_COMMENTS
        );
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
