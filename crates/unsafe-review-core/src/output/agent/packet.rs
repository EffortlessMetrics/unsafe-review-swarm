use super::context::{AgentContext, AgentSourceContext};
use super::evidence::{
    AgentMissingEvidence, AgentObligationEvidence, AgentSafetyContract, AgentWitnessRoute,
};
use super::queue::{
    AgentReadiness, AgentRepairQueue, READY_FOR_AGENT, REQUIRES_HUMAN_REVIEW,
    REQUIRES_WITNESS_RECEIPT, packet_repair_projection,
};
use super::{DO_NOT_DO, TRUST_BOUNDARY};
use crate::domain::{
    AgentLspReadiness, BaselineState, CommentPlanStatus, Coverage, CoverageBlock, ManualContext,
    OutcomeMovement, ReviewCard, WitnessReceiptCoverage,
};
use crate::output::confirmation::ConfirmationCue;
use crate::policy::SnapshotCoverage;
use serde::Serialize;

/// Map an `agent_readiness.state` string back to the domain [`AgentLspReadiness`]
/// enum so that `coverage.agent_lsp_readiness` in the agent packet is derived
/// from the SAME readiness computation as `agent_readiness.state` (output audit
/// #1687, findings 3+4 — guarantees identity when `has_card_scoped_repairs` is
/// available from the repair projection).
fn agent_state_to_lsp_readiness(state: &'static str) -> AgentLspReadiness {
    match state {
        READY_FOR_AGENT => AgentLspReadiness::Ready,
        REQUIRES_HUMAN_REVIEW => AgentLspReadiness::NeedsHuman,
        REQUIRES_WITNESS_RECEIPT => AgentLspReadiness::RequiresWitnessReceipt,
        _ => AgentLspReadiness::Unsupported,
    }
}

/// Machine-readable per-card coverage block (SPEC-0029) projected into the
/// agent context packet.
///
/// All values are advisory closed-vocabulary strings. This block is derived
/// exclusively from `CoverageBlock::derive(card)` — the same derivation used by
/// the JSON analyzer output — so no second truth surface is created.
#[derive(Serialize)]
pub(super) struct AgentCoverageBlock {
    /// Contract (safety doc) evidence level: `present`, `weak`, or `missing`.
    contract_coverage: &'static str,
    /// Guard (discharge) evidence level: `present`, `weak`, or `missing`.
    guard_coverage: &'static str,
    /// Test reachability evidence level: `present`, `weak`, or `missing`.
    test_reach_coverage: &'static str,
    /// Witness receipt import state: `present` or `missing`.
    witness_receipt_coverage: &'static str,
    /// Manual-candidate overlay: `present` or `absent`.
    manual_context: &'static str,
    /// Baseline posture (SPEC-0030): `new`, `worsened`, `inherited`, `resolved`, or `unknown`.
    baseline_state: &'static str,
    /// Outcome movement vs. saved snapshot (SPEC-0030): `improved`, `regressed`, `unchanged`, or `unknown`.
    outcome_movement: &'static str,
    /// Comment-plan selection status (SPEC-0032): `selected`, `not_selected`, or `not_eligible`.
    comment_plan_status: &'static str,
    /// Agent-LSP readiness: `ready`, `needs_human`, or `unsupported`.
    agent_lsp_readiness: &'static str,
}

impl From<CoverageBlock> for AgentCoverageBlock {
    fn from(block: CoverageBlock) -> Self {
        Self {
            contract_coverage: coverage_str(block.contract_coverage),
            guard_coverage: coverage_str(block.guard_coverage),
            test_reach_coverage: coverage_str(block.test_reach_coverage),
            witness_receipt_coverage: witness_receipt_str(block.witness_receipt_coverage),
            manual_context: manual_context_str(block.manual_context),
            baseline_state: baseline_state_str(block.baseline_state),
            outcome_movement: outcome_movement_str(block.outcome_movement),
            comment_plan_status: comment_plan_status_str(block.comment_plan_status),
            agent_lsp_readiness: agent_lsp_readiness_str(block.agent_lsp_readiness),
        }
    }
}

fn coverage_str(coverage: Coverage) -> &'static str {
    coverage.as_str()
}

fn witness_receipt_str(coverage: WitnessReceiptCoverage) -> &'static str {
    coverage.as_str()
}

fn manual_context_str(context: ManualContext) -> &'static str {
    context.as_str()
}

fn baseline_state_str(state: BaselineState) -> &'static str {
    state.as_str()
}

fn outcome_movement_str(movement: OutcomeMovement) -> &'static str {
    movement.as_str()
}

fn comment_plan_status_str(status: CommentPlanStatus) -> &'static str {
    status.as_str()
}

fn agent_lsp_readiness_str(readiness: AgentLspReadiness) -> &'static str {
    readiness.as_str()
}

#[derive(Serialize)]
pub(super) struct AgentPacket<'a> {
    schema_version: &'static str,
    tool: &'static str,
    mode: &'static str,
    source: &'static str,
    policy: &'static str,
    trust_boundary: &'static str,
    card_id: &'a str,
    card: AgentCard<'a>,
    proof_path: &'static str,
    task: &'a str,
    confirmation_cue: ConfirmationCue,
    context: AgentContext<'a>,
    source_context: AgentSourceContext<'a>,
    safety_contract: AgentSafetyContract<'a>,
    required_safety_conditions: Vec<&'a str>,
    obligation_evidence: Vec<AgentObligationEvidence<'a>>,
    missing: Vec<&'a str>,
    missing_evidence: Vec<AgentMissingEvidence<'a>>,
    allowed_repairs: Vec<String>,
    agent_readiness: AgentReadiness,
    repair_queue: AgentRepairQueue,
    repair_scope: &'static str,
    witness_routes: Vec<AgentWitnessRoute<'a>>,
    verify_commands: &'a [String],
    /// SPEC-0029 coverage block — same derivation as the JSON analyzer output.
    coverage: AgentCoverageBlock,
    do_not_do: &'static [&'static str],
    stop_conditions: &'static [&'static str],
}

impl<'a> AgentPacket<'a> {
    /// Build a packet for `card`, overriding `comment_plan_status` with the
    /// value computed by the comment-plan selection pass (SPEC-0032).
    ///
    /// Callers that have access to the full `AnalyzeOutput` must supply the
    /// status from [`comment_plan::card_statuses`] so that the agent packet
    /// projects the same `comment_plan_status` as `comment-plan.json` and
    /// `cards.json`.  Callers without output context (e.g. `From<&ReviewCard>`)
    /// pass `CommentPlanStatus::NotEligible` as the honest default.
    ///
    /// `snapshot` is the per-card coverage snapshot from `AnalyzeOutput.coverage_snapshot`,
    /// used to project `baseline_state`/`outcome_movement` from the same slot-level comparison
    /// the summary uses (SPEC-0030 §single-truth, output audit #1687).
    pub(super) fn from_with_status(
        card: &'a ReviewCard,
        comment_plan_status: CommentPlanStatus,
        snapshot: Option<&SnapshotCoverage>,
    ) -> Self {
        let repairs = packet_repair_projection(card);
        let mut coverage_block = card.coverage_block();
        coverage_block.comment_plan_status = comment_plan_status;
        // Apply snapshot-level movement so per-card baseline_state/outcome_movement
        // agree with summary.worsened_gaps / summary.improved_gaps (SPEC-0030 §single-truth).
        if let Some(snap) = snapshot {
            coverage_block.apply_snapshot_slots(
                &snap.contract_coverage,
                &snap.guard_coverage,
                &snap.test_reach_coverage,
                &snap.witness_receipt_coverage,
            );
        }
        // Guarantee: coverage.agent_lsp_readiness == agent_readiness.state
        // (output audit #1687, findings 3+4).  The repair projection already
        // computed the authoritative readiness state via `compute_agent_lsp_readiness`
        // with the exact `has_card_scoped_repairs` value.  Override the coverage
        // block slot so both fields in the agent packet agree unconditionally.
        coverage_block.agent_lsp_readiness =
            agent_state_to_lsp_readiness(repairs.agent_readiness.state);
        Self {
            schema_version: "0.1",
            tool: "unsafe-review",
            mode: "bounded_repair_packet",
            source: "review_card",
            policy: "advisory",
            trust_boundary: TRUST_BOUNDARY,
            card_id: &card.id.0,
            card: AgentCard::from(card),
            proof_path: card.proof_path.as_str(),
            task: &card.next_action.summary,
            confirmation_cue: ConfirmationCue::from(card),
            context: AgentContext::from(card),
            source_context: AgentSourceContext::from(card),
            safety_contract: AgentSafetyContract::from(card),
            required_safety_conditions: card
                .obligations
                .iter()
                .map(|obligation| obligation.description.as_str())
                .collect(),
            obligation_evidence: card
                .obligation_evidence
                .iter()
                .map(AgentObligationEvidence::from)
                .collect(),
            missing: card
                .missing
                .iter()
                .map(|missing| missing.message.as_str())
                .collect(),
            missing_evidence: card
                .missing
                .iter()
                .map(|missing| AgentMissingEvidence {
                    kind: &missing.kind,
                    message: &missing.message,
                })
                .collect(),
            allowed_repairs: repairs.allowed_repairs,
            agent_readiness: repairs.agent_readiness,
            repair_queue: repairs.repair_queue,
            repair_scope: "this card only",
            witness_routes: card.routes.iter().map(AgentWitnessRoute::from).collect(),
            verify_commands: &card.next_action.verify_commands,
            coverage: AgentCoverageBlock::from(coverage_block),
            do_not_do: DO_NOT_DO,
            stop_conditions: &[
                "the missing evidence is present or explicitly waived with owner and expiry",
                "the focused test or witness command has been run or marked unavailable",
                "no unrelated unsafe code was changed",
                "the ReviewCard identity still maps to the same unsafe seam",
            ],
        }
    }
}

impl<'a> From<&'a ReviewCard> for AgentPacket<'a> {
    /// Build a packet without output context.
    ///
    /// `comment_plan_status` defaults to `NotEligible` because the selection
    /// outcome is only computable from the full `AnalyzeOutput`.  `snapshot` is
    /// `None` because the coverage snapshot is only available through `AnalyzeOutput`.
    /// Callers with output context should use [`AgentPacket::from_with_status`] instead.
    fn from(card: &'a ReviewCard) -> Self {
        Self::from_with_status(card, CommentPlanStatus::NotEligible, None)
    }
}

#[derive(Serialize)]
struct AgentCard<'a> {
    id: &'a str,
    #[serde(rename = "class")]
    class_name: &'static str,
    priority: &'static str,
    confidence: &'static str,
    proof_path: &'static str,
}

impl<'a> From<&'a ReviewCard> for AgentCard<'a> {
    fn from(card: &'a ReviewCard) -> Self {
        Self {
            id: &card.id.0,
            class_name: card.class.as_str(),
            priority: card.priority.as_str(),
            confidence: card.confidence.as_str(),
            proof_path: card.proof_path.as_str(),
        }
    }
}
