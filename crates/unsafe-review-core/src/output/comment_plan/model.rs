use crate::api::AnalyzeOutput;
use crate::domain::{ReviewCard, WitnessRoute};
use crate::output::{NO_CHANGED_GAPS_LIMITATION, NO_CHANGED_GAPS_MESSAGE};
use crate::util::path_display;
use serde::Serialize;

use super::selection::{
    actionability, comment_body, non_selection_reason, selection_reason, should_plan_comment,
};

const MAX_PLANNED_COMMENTS: usize = 3;
pub(super) const TRUST_BOUNDARY: &str = "Static unsafe contract review only; this is not a proof of memory safety, not UB-free status, and not a Miri result unless a witness receipt is attached.";

#[derive(Serialize)]
pub(super) struct CommentPlan {
    pub(super) schema_version: String,
    pub(super) tool: String,
    pub(super) mode: &'static str,
    pub(super) policy: &'static str,
    pub(super) comments: Vec<PlannedComment>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) not_selected: Vec<NotSelectedCard>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) no_changed_gaps: Option<NoChangedGaps>,
    pub(super) trust_boundary: &'static str,
}

impl From<&AnalyzeOutput> for CommentPlan {
    fn from(output: &AnalyzeOutput) -> Self {
        let mut comments = Vec::new();
        let mut not_selected = Vec::new();

        for card in &output.cards {
            if should_plan_comment(card) {
                if comments.len() < MAX_PLANNED_COMMENTS {
                    comments.push(PlannedComment::from(card));
                } else {
                    not_selected.push(NotSelectedCard::from_reason(
                        card,
                        "comment-plan max of three candidates reached",
                    ));
                }
            } else {
                not_selected.push(NotSelectedCard::from_reason(
                    card,
                    non_selection_reason(card),
                ));
            }
        }

        Self {
            schema_version: output.schema_version.clone(),
            tool: output.tool.clone(),
            mode: "plan_only",
            policy: output.policy.as_str(),
            comments,
            not_selected,
            no_changed_gaps: (output.summary.open_actionable_gaps == 0).then_some(NoChangedGaps {
                message: NO_CHANGED_GAPS_MESSAGE,
                limitation: NO_CHANGED_GAPS_LIMITATION,
            }),
            trust_boundary: TRUST_BOUNDARY,
        }
    }
}

#[derive(Serialize)]
pub(super) struct NoChangedGaps {
    message: &'static str,
    limitation: &'static str,
}

#[derive(Serialize)]
pub(super) struct PlannedComment {
    card_id: String,
    path: String,
    line: usize,
    class: &'static str,
    priority: &'static str,
    confidence: &'static str,
    operation: String,
    operation_family: &'static str,
    witness_routes: Vec<PlannedWitnessRoute>,
    next_action: String,
    verify_commands: Vec<String>,
    selection_reason: &'static str,
    actionability: &'static str,
    trust_boundary: &'static str,
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
            next_action: card.next_action.summary.clone(),
            verify_commands: card.next_action.verify_commands.clone(),
            selection_reason: selection_reason(card),
            actionability: actionability(card),
            trust_boundary: TRUST_BOUNDARY,
            body: comment_body(card),
        }
    }
}

#[derive(Serialize)]
pub(super) struct NotSelectedCard {
    card_id: String,
    path: String,
    line: usize,
    class: &'static str,
    priority: &'static str,
    confidence: &'static str,
    operation_family: &'static str,
    actionability: &'static str,
    reason: &'static str,
}

impl NotSelectedCard {
    fn from_reason(card: &ReviewCard, reason: &'static str) -> Self {
        Self {
            card_id: card.id.0.clone(),
            path: path_display(&card.site.location.file),
            line: card.site.location.line,
            class: card.class.as_str(),
            priority: card.priority.as_str(),
            confidence: card.confidence.as_str(),
            operation_family: card.operation.family.as_str(),
            actionability: actionability(card),
            reason,
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
