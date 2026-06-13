use crate::api::AnalyzeOutput;
use crate::domain::{ReviewCard, WitnessRoute};
use crate::output::confirmation::{
    BuildThisFirstCue, MinimalReproCue, build_this_first, confirmation_step, hypothesis_to_confirm,
    minimal_repro,
};
use crate::output::{
    NO_CHANGED_GAPS_LIMITATION, NO_CHANGED_GAPS_MESSAGE,
    REVIEWCARD_TRUST_BOUNDARY as TRUST_BOUNDARY, agent, repair_queue,
};
use crate::util::path_display;
use serde::Serialize;
use std::collections::BTreeSet;

use super::selection::{
    MAX_COMMENT_BUDGET_REASON, OPERATION_FAMILY_BUDGET_REASON, ReviewBudgetReason, actionability,
    comment_body, coverage_gap, importance_rank, non_selection_reason, relevance, selection_reason,
    should_plan_comment,
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

        // Partition cards into eligible and ineligible.
        // Eligible candidates are sorted by importance before the family/obligation
        // dedup and budget cap are applied, so the highest-importance unique card
        // per family fills each budget slot rather than the first one in file order.
        // The global card order (output.cards = file/line) is preserved for all
        // other output surfaces; only the comment-plan candidate selection re-ranks.
        let (mut eligible, ineligible): (Vec<&ReviewCard>, Vec<&ReviewCard>) = output
            .cards
            .iter()
            .partition(|card| should_plan_comment(card));
        eligible.sort_by(|a, b| importance_rank(a).cmp(&importance_rank(b)));

        for card in eligible {
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
        }
        for card in ineligible {
            not_selected.push(NotSelectedCard::from_reason(
                card,
                non_selection_reason(card),
            ));
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

pub(super) fn comment_budget_key(card: &ReviewCard) -> String {
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
    /// The primary weak or missing SPEC-0029 coverage slot that makes this
    /// card worth surfacing (SPEC-0032). Format: `"<slot>: <state>"`.
    coverage_gap: String,
    /// Per-card confirmation state derived from the imported witness receipt,
    /// if any (SPEC-0032 / SPEC-0030). Closed vocabulary: `pending`,
    /// `receipt_imported`, `executed`, `confirmed`, `not_reproduced`,
    /// `inconclusive`.
    confirmation_state: &'static str,
    hypothesis_to_confirm: String,
    operation: String,
    operation_family: &'static str,
    witness_routes: Vec<PlannedWitnessRoute>,
    next_action: String,
    verify_commands: Vec<String>,
    build_this_first: BuildThisFirstCue,
    minimal_repro: MinimalReproCue,
    confirmation_step: String,
    selection_reason: &'static str,
    selection_reason_code: &'static str,
    actionability: &'static str,
    relevance: &'static str,
    agent_readiness: CommentPlanAgentReadiness,
    repair_queue_buckets: Vec<&'static str>,
    repair_queue_bucket_reasons: Vec<&'static str>,
    context_command: String,
    trust_boundary: &'static str,
    body: String,
}

impl From<&ReviewCard> for PlannedComment {
    fn from(card: &ReviewCard) -> Self {
        let selection_reason = selection_reason(card);
        let repair = CommentPlanRepairMetadata::from(card);
        Self {
            card_id: card.id.0.clone(),
            path: path_display(&card.site.location.file),
            line: card.site.location.line,
            changed_line: card.site.changed,
            class: card.class.as_str(),
            priority: card.priority.as_str(),
            confidence: card.confidence.as_str(),
            proof_path: card.proof_path.as_str(),
            coverage_gap: coverage_gap(card),
            confirmation_state: card.witness.confirmation_state(),
            hypothesis_to_confirm: hypothesis_to_confirm(card),
            operation: card.operation.expression.clone(),
            operation_family: card.operation.family.as_str(),
            witness_routes: card.routes.iter().map(PlannedWitnessRoute::from).collect(),
            next_action: card.next_action.summary.clone(),
            verify_commands: card.next_action.verify_commands.clone(),
            build_this_first: build_this_first(card),
            minimal_repro: minimal_repro(card),
            confirmation_step: confirmation_step(card),
            selection_reason: selection_reason.message,
            selection_reason_code: selection_reason.code,
            actionability: actionability(card),
            relevance: relevance(card),
            agent_readiness: repair.agent_readiness,
            repair_queue_buckets: repair.repair_queue_buckets,
            repair_queue_bucket_reasons: repair.repair_queue_bucket_reasons,
            context_command: repair.context_command,
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
    /// The primary weak or missing SPEC-0029 coverage slot for this card
    /// (SPEC-0032). Format: `"<slot>: <state>"`. Present for all cards so
    /// the posting wrapper can explain why the card is worth reviewing even
    /// when it was not selected for inline comment.
    #[serde(skip_serializing_if = "Option::is_none")]
    coverage_gap: Option<String>,
    operation: String,
    operation_family: &'static str,
    next_action: String,
    actionability: &'static str,
    relevance: &'static str,
    agent_readiness: CommentPlanAgentReadiness,
    repair_queue_buckets: Vec<&'static str>,
    repair_queue_bucket_reasons: Vec<&'static str>,
    context_command: String,
    reason: &'static str,
    reason_code: &'static str,
}

impl NotSelectedCard {
    fn from_reason(card: &ReviewCard, reason: ReviewBudgetReason) -> Self {
        let repair = CommentPlanRepairMetadata::from(card);
        // Surface coverage_gap for actionable cards so the posting wrapper
        // can explain why the card is worth reviewing (SPEC-0032).
        let gap = if card.class.is_actionable() {
            Some(coverage_gap(card))
        } else {
            None
        };
        Self {
            card_id: card.id.0.clone(),
            path: path_display(&card.site.location.file),
            line: card.site.location.line,
            changed_line: card.site.changed,
            class: card.class.as_str(),
            priority: card.priority.as_str(),
            confidence: card.confidence.as_str(),
            proof_path: card.proof_path.as_str(),
            coverage_gap: gap,
            operation: card.operation.expression.clone(),
            operation_family: card.operation.family.as_str(),
            next_action: card.next_action.summary.clone(),
            actionability: actionability(card),
            relevance: relevance(card),
            agent_readiness: repair.agent_readiness,
            repair_queue_buckets: repair.repair_queue_buckets,
            repair_queue_bucket_reasons: repair.repair_queue_bucket_reasons,
            context_command: repair.context_command,
            reason: reason.message,
            reason_code: reason.code,
        }
    }
}

struct CommentPlanRepairMetadata {
    agent_readiness: CommentPlanAgentReadiness,
    repair_queue_buckets: Vec<&'static str>,
    repair_queue_bucket_reasons: Vec<&'static str>,
    context_command: String,
}

impl From<&ReviewCard> for CommentPlanRepairMetadata {
    fn from(card: &ReviewCard) -> Self {
        let projection = agent::repair_queue_projection(card);
        let repair_queue_buckets = repair_queue::aggregate_buckets(&projection);
        let repair_queue_bucket_reasons = repair_queue_buckets
            .iter()
            .map(|bucket| repair_queue::bucket_reason(bucket))
            .collect();
        Self {
            agent_readiness: CommentPlanAgentReadiness::from(&projection.agent_readiness),
            repair_queue_buckets,
            repair_queue_bucket_reasons,
            context_command: format!("unsafe-review context {} --json", card.id),
        }
    }
}

#[derive(Serialize)]
struct CommentPlanAgentReadiness {
    ready: bool,
    state: &'static str,
    reasons: Vec<String>,
}

impl From<&agent::AgentReadiness> for CommentPlanAgentReadiness {
    fn from(readiness: &agent::AgentReadiness) -> Self {
        Self {
            ready: readiness.ready,
            state: readiness.state,
            reasons: readiness.reasons.clone(),
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
