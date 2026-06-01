use crate::domain::{EvidenceState, ObligationEvidence, ReviewCard};
use serde::Serialize;

#[derive(Serialize)]
pub(super) struct AgentSafetyContract<'a> {
    required_conditions: Vec<&'a str>,
    contract_evidence: &'a str,
    discharge_evidence: &'a str,
    reach_evidence: &'a str,
    witness_evidence: &'a str,
    reach_limitation: &'static str,
}

impl<'a> From<&'a ReviewCard> for AgentSafetyContract<'a> {
    fn from(card: &'a ReviewCard) -> Self {
        Self {
            required_conditions: card
                .obligations
                .iter()
                .map(|obligation| obligation.description.as_str())
                .collect(),
            contract_evidence: &card.contract.summary,
            discharge_evidence: &card.discharge.summary,
            reach_evidence: &card.reach.summary,
            witness_evidence: &card.witness.summary,
            reach_limitation: "static reach evidence is not proof that the unsafe site executed",
        }
    }
}

#[derive(Serialize)]
pub(super) struct AgentObligationEvidence<'a> {
    key: &'a str,
    description: &'a str,
    contract: AgentEvidenceState<'a>,
    discharge: AgentEvidenceState<'a>,
    reach: AgentEvidenceState<'a>,
    witness: AgentEvidenceState<'a>,
}

impl<'a> From<&'a ObligationEvidence> for AgentObligationEvidence<'a> {
    fn from(evidence: &'a ObligationEvidence) -> Self {
        Self {
            key: &evidence.obligation.key,
            description: &evidence.obligation.description,
            contract: AgentEvidenceState::from(&evidence.contract),
            discharge: AgentEvidenceState::from(&evidence.discharge),
            reach: AgentEvidenceState::from(&evidence.reach),
            witness: AgentEvidenceState::from(&evidence.witness),
        }
    }
}

#[derive(Serialize)]
struct AgentEvidenceState<'a> {
    present: bool,
    state: &'a str,
    summary: &'a str,
}

impl<'a> From<&'a EvidenceState> for AgentEvidenceState<'a> {
    fn from(state: &'a EvidenceState) -> Self {
        Self {
            present: state.present,
            state: &state.state,
            summary: &state.summary,
        }
    }
}

#[derive(Serialize)]
pub(super) struct AgentMissingEvidence<'a> {
    pub(super) kind: &'a str,
    pub(super) message: &'a str,
}
