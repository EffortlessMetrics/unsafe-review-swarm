use super::context::{AgentCard, AgentContext, AgentSourceContext};
use super::evidence::{AgentMissingEvidence, AgentObligationEvidence, AgentSafetyContract};
use super::queue::{AgentReadiness, AgentRepairQueue};
use super::{DO_NOT_DO, TRUST_BOUNDARY, queue, readiness, repairs};
use crate::domain::{ReviewCard, WitnessRoute};
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
    task: &'a str,
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
        let allowed_repairs = repairs::build(card);
        let agent_readiness = readiness::build(card, allowed_repairs.has_card_scoped_repairs);
        let repair_queue = queue::build(card, &agent_readiness);
        Self {
            schema_version: "0.1",
            tool: "unsafe-review",
            mode: "bounded_repair_packet",
            source: "review_card",
            policy: "advisory",
            trust_boundary: TRUST_BOUNDARY,
            card_id: &card.id.0,
            card: AgentCard::from(card),
            task: &card.next_action.summary,
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
            allowed_repairs: allowed_repairs.repairs,
            agent_readiness,
            repair_queue,
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
struct AgentWitnessRoute<'a> {
    kind: &'static str,
    reason: &'a str,
    command: Option<&'a str>,
    required: bool,
}

impl<'a> From<&'a WitnessRoute> for AgentWitnessRoute<'a> {
    fn from(route: &'a WitnessRoute) -> Self {
        Self {
            kind: route.kind.as_str(),
            reason: &route.reason,
            command: route.command.as_deref(),
            required: route.required,
        }
    }
}
