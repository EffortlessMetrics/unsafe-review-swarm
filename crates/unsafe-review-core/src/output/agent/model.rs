use crate::domain::{EvidenceState, ObligationEvidence, RelatedTest, ReviewCard, WitnessRoute};
use crate::util::path_display;
use serde::Serialize;

const MAX_CONTEXT_EVIDENCE: usize = 3;
const MAX_RELATED_TESTS: usize = 3;

#[derive(Serialize)]
pub(super) struct AgentCard<'a> {
    id: &'a str,
    #[serde(rename = "class")]
    class_name: &'static str,
    priority: &'static str,
    confidence: &'static str,
}

impl<'a> From<&'a ReviewCard> for AgentCard<'a> {
    fn from(card: &'a ReviewCard) -> Self {
        Self {
            id: &card.id.0,
            class_name: card.class.as_str(),
            priority: card.priority.as_str(),
            confidence: card.confidence.as_str(),
        }
    }
}

#[derive(Serialize)]
pub(super) struct AgentContext<'a> {
    file: String,
    line: usize,
    column: usize,
    owner: &'a str,
    site_kind: &'static str,
    operation_family: &'static str,
    operation: &'a str,
    snippet: &'a str,
    hazards: Vec<&'static str>,
}

impl<'a> From<&'a ReviewCard> for AgentContext<'a> {
    fn from(card: &'a ReviewCard) -> Self {
        Self {
            file: path_display(&card.site.location.file),
            line: card.site.location.line,
            column: card.site.location.column,
            owner: card.site.owner.as_deref().unwrap_or(""),
            site_kind: card.site.kind.as_str(),
            operation_family: card.operation.family.as_str(),
            operation: &card.operation.expression,
            snippet: &card.site.snippet,
            hazards: card.hazards.iter().map(|hazard| hazard.as_str()).collect(),
        }
    }
}

#[derive(Serialize)]
pub(super) struct AgentSourceContext<'a> {
    unsafe_site: AgentSourceSite<'a>,
    nearby_safety_contract: Option<AgentContextEvidence<'a>>,
    nearby_guard_evidence: Vec<AgentContextEvidence<'a>>,
    related_tests: Vec<AgentRelatedTest<'a>>,
    limits: &'static [&'static str],
}

impl<'a> From<&'a ReviewCard> for AgentSourceContext<'a> {
    fn from(card: &'a ReviewCard) -> Self {
        let nearby_safety_contract = card.contract.present.then_some(AgentContextEvidence {
            kind: "safety_contract",
            key: None,
            summary: &card.contract.summary,
        });
        let nearby_guard_evidence = card
            .obligation_evidence
            .iter()
            .filter(|evidence| evidence.discharge.present)
            .take(MAX_CONTEXT_EVIDENCE)
            .map(|evidence| AgentContextEvidence {
                kind: "guard_evidence",
                key: Some(evidence.obligation.key.as_str()),
                summary: &evidence.discharge.summary,
            })
            .collect();
        let related_tests = card
            .related_tests
            .iter()
            .take(MAX_RELATED_TESTS)
            .map(AgentRelatedTest::from)
            .collect();

        Self {
            unsafe_site: AgentSourceSite::from(card),
            nearby_safety_contract,
            nearby_guard_evidence,
            related_tests,
            limits: &[
                "bounded source context only; this packet does not include whole files",
                "related test mentions do not prove the unsafe site executed",
                "evidence summaries are ReviewCard projections, not independent analyzer truth",
            ],
        }
    }
}

#[derive(Serialize)]
struct AgentSourceSite<'a> {
    file: String,
    line: usize,
    column: usize,
    owner: &'a str,
    snippet: &'a str,
}

impl<'a> From<&'a ReviewCard> for AgentSourceSite<'a> {
    fn from(card: &'a ReviewCard) -> Self {
        Self {
            file: path_display(&card.site.location.file),
            line: card.site.location.line,
            column: card.site.location.column,
            owner: card.site.owner.as_deref().unwrap_or(""),
            snippet: &card.site.snippet,
        }
    }
}

#[derive(Serialize)]
struct AgentContextEvidence<'a> {
    kind: &'static str,
    key: Option<&'a str>,
    summary: &'a str,
}

#[derive(Serialize)]
struct AgentRelatedTest<'a> {
    name: &'a str,
    file: &'a str,
    line: usize,
}

impl<'a> From<&'a RelatedTest> for AgentRelatedTest<'a> {
    fn from(test: &'a RelatedTest) -> Self {
        Self {
            name: &test.name,
            file: &test.file,
            line: test.line,
        }
    }
}

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

#[derive(Serialize)]
pub(super) struct AgentWitnessRoute<'a> {
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
