use crate::api::AnalyzeOutput;
use crate::domain::{ReviewCard, WitnessRoute};
use crate::output::{
    NO_CHANGED_GAPS_LIMITATION, NO_CHANGED_GAPS_MESSAGE,
    REVIEWCARD_TRUST_BOUNDARY as TRUST_BOUNDARY,
};
use crate::util::path_display;
use serde::Serialize;
use std::collections::BTreeSet;

use super::selection::{
    MAX_COMMENT_BUDGET_REASON, OPERATION_FAMILY_BUDGET_REASON, ReviewBudgetReason, actionability,
    build_this_first, comment_body, confirmation_step, hypothesis_to_confirm, non_selection_reason,
    relevance, selection_reason, should_plan_comment,
};

const MAX_PLANNED_COMMENTS: usize = 3;
const REVIEW_BUDGET_REASON: ReviewBudgetReason = ReviewBudgetReason {
    code: "bounded_reviewer_noise",
    message: "bounded reviewer noise",
};
#[derive(Serialize)]
pub(super) struct CommentPlan {
    pub(super) schema_version: String,
    pub(super) tool: String,
    pub(super) mode: &'static str,
    pub(super) policy: &'static str,
    pub(super) summary: CommentPlanSummary,
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
        let mut selected_budget_keys = BTreeSet::new();

        for card in &output.cards {
            if should_plan_comment(card) {
                let budget_key = comment_budget_key(card);
                if selected_budget_keys.contains(&budget_key) {
                    not_selected.push(NotSelectedCard::from_reason(
                        card,
                        OPERATION_FAMILY_BUDGET_REASON,
                    ));
                } else if comments.len() < MAX_PLANNED_COMMENTS {
                    selected_budget_keys.insert(budget_key);
                    comments.push(PlannedComment::from(card));
                } else {
                    not_selected.push(NotSelectedCard::from_reason(
                        card,
                        MAX_COMMENT_BUDGET_REASON,
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
            summary: CommentPlanSummary {
                selected_count: comments.len(),
                not_selected_count: not_selected.len(),
                budget: MAX_PLANNED_COMMENTS,
                reason: REVIEW_BUDGET_REASON.message,
                reason_code: REVIEW_BUDGET_REASON.code,
            },
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

fn comment_budget_key(card: &ReviewCard) -> String {
    let mut obligations = card
        .obligation_evidence
        .iter()
        .filter(|evidence| {
            !evidence.contract.present
                || !evidence.discharge.present
                || !evidence.reach.present
                || !evidence.witness.present
        })
        .map(|evidence| evidence.obligation.key.as_str())
        .collect::<Vec<_>>();
    obligations.sort_unstable();
    obligations.dedup();

    if obligations.is_empty() {
        obligations.push("review");
    }

    format!(
        "{}:{}",
        card.operation.family.as_str(),
        obligations.join("|")
    )
}

#[derive(Serialize)]
pub(super) struct CommentPlanSummary {
    selected_count: usize,
    not_selected_count: usize,
    budget: usize,
    reason: &'static str,
    reason_code: &'static str,
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
    changed_line: bool,
    class: &'static str,
    priority: &'static str,
    confidence: &'static str,
    proof_path: &'static str,
    hypothesis_to_confirm: String,
    operation: String,
    operation_family: &'static str,
    witness_routes: Vec<PlannedWitnessRoute>,
    next_action: String,
    verify_commands: Vec<String>,
    build_this_first: PlannedBuildFirst,
    confirmation_step: String,
    selection_reason: &'static str,
    selection_reason_code: &'static str,
    actionability: &'static str,
    relevance: &'static str,
    trust_boundary: &'static str,
    body: String,
}

impl From<&ReviewCard> for PlannedComment {
    fn from(card: &ReviewCard) -> Self {
        let selection_reason = selection_reason(card);
        Self {
            card_id: card.id.0.clone(),
            path: path_display(&card.site.location.file),
            line: card.site.location.line,
            changed_line: card.site.changed,
            class: card.class.as_str(),
            priority: card.priority.as_str(),
            confidence: card.confidence.as_str(),
            proof_path: card.proof_path.as_str(),
            hypothesis_to_confirm: hypothesis_to_confirm(card),
            operation: card.operation.expression.clone(),
            operation_family: card.operation.family.as_str(),
            witness_routes: card.routes.iter().map(PlannedWitnessRoute::from).collect(),
            next_action: card.next_action.summary.clone(),
            verify_commands: card.next_action.verify_commands.clone(),
            build_this_first: build_this_first(card),
            confirmation_step: confirmation_step(card),
            selection_reason: selection_reason.message,
            selection_reason_code: selection_reason.code,
            actionability: actionability(card),
            relevance: relevance(card),
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
    changed_line: bool,
    class: &'static str,
    priority: &'static str,
    confidence: &'static str,
    proof_path: &'static str,
    operation: String,
    operation_family: &'static str,
    next_action: String,
    actionability: &'static str,
    relevance: &'static str,
    reason: &'static str,
    reason_code: &'static str,
}

impl NotSelectedCard {
    fn from_reason(card: &ReviewCard, reason: ReviewBudgetReason) -> Self {
        Self {
            card_id: card.id.0.clone(),
            path: path_display(&card.site.location.file),
            line: card.site.location.line,
            changed_line: card.site.changed,
            class: card.class.as_str(),
            priority: card.priority.as_str(),
            confidence: card.confidence.as_str(),
            proof_path: card.proof_path.as_str(),
            operation: card.operation.expression.clone(),
            operation_family: card.operation.family.as_str(),
            next_action: card.next_action.summary.clone(),
            actionability: actionability(card),
            relevance: relevance(card),
            reason: reason.message,
            reason_code: reason.code,
        }
    }
}

#[derive(Serialize)]
pub(super) struct PlannedBuildFirst {
    kind: &'static str,
    command: Option<String>,
    route_kind: Option<&'static str>,
    pub(super) summary: String,
}

impl PlannedBuildFirst {
    pub(super) fn new(
        kind: &'static str,
        command: Option<String>,
        route_kind: Option<&'static str>,
        summary: String,
    ) -> Self {
        Self {
            kind,
            command,
            route_kind,
            summary,
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
