use super::context::{AgentContext, AgentSourceContext};
use super::evidence::{
    AgentMissingEvidence, AgentObligationEvidence, AgentSafetyContract, AgentWitnessRoute,
};
use super::queue::{AgentReadiness, AgentRepairQueue, packet_repair_projection};
use super::{DO_NOT_DO, TRUST_BOUNDARY};
use crate::domain::ReviewCard;
use crate::output::confirmation::ConfirmationCue;
use serde::Serialize;

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
    do_not_do: &'static [&'static str],
    stop_conditions: &'static [&'static str],
}

impl<'a> From<&'a ReviewCard> for AgentPacket<'a> {
    fn from(card: &'a ReviewCard) -> Self {
        let repairs = packet_repair_projection(card);
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
