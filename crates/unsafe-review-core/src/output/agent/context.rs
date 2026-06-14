use crate::domain::{RelatedTest, ReviewCard};
use crate::output::UNKNOWN_OWNER;
use crate::util::path_display;
use serde::Serialize;

const MAX_CONTEXT_EVIDENCE: usize = 3;
const MAX_RELATED_TESTS: usize = 3;

#[derive(Serialize)]
pub(super) struct AgentContext<'a> {
    file: String,
    line: usize,
    column: usize,
    owner: &'a str,
    site_kind: &'static str,
    operation_family: &'static str,
    proof_path: &'static str,
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
            owner: card.site.owner.as_deref().unwrap_or(UNKNOWN_OWNER),
            site_kind: card.site.kind.as_str(),
            operation_family: card.operation.family.as_str(),
            proof_path: card.proof_path.as_str(),
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
            owner: card.site.owner.as_deref().unwrap_or(UNKNOWN_OWNER),
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
