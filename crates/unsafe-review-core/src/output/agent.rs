use crate::domain::ReviewCard;
use serde::Serialize;

const TRUST_BOUNDARY: &str = "Static unsafe contract review only; this is not a proof of memory safety, not UB-free status, and not a Miri result unless a witness receipt is attached.";
const MAX_CONTEXT_EVIDENCE: usize = 3;
const MAX_RELATED_TESTS: usize = 3;
pub(crate) const DO_NOT_DO: &[&str] = &[
    "do not widen unsafe code without reducing the missing evidence",
    "do not suppress this card instead of adding, exposing, or explicitly waiving evidence",
    "do not add a broad suppression",
    "do not replace executable guard or discharge evidence with comments or docs",
    "do not claim Miri proof unless the witness command is run and attached",
    "do not claim automatic safety repair from this packet",
    "do not claim unsafe-review ran an agent, ran witnesses, applied source edits, or posted comments",
    "do not change unrelated unsafe code or public API behavior",
    "do not treat a test mention as proof that the unsafe site executed",
];

pub(crate) fn render(card: &ReviewCard) -> String {
    render_pretty(&AgentPacket::from(card))
}

fn render_pretty(value: &impl Serialize) -> String {
    match serde_json::to_string_pretty(value) {
        Ok(text) => text,
        Err(err) => format!("{{\n  \"error\": \"agent packet serialization failed: {err}\"\n}}"),
    }
}

#[derive(Serialize)]
struct AgentPacket<'a> {
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
        let allowed_repairs = allowed_repairs(card);
        let agent_readiness = agent_readiness(card, allowed_repairs.has_card_scoped_repairs);
        let repair_queue = repair_queue(card, &agent_readiness);
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

fn agent_readiness(card: &ReviewCard, has_card_scoped_repairs: bool) -> AgentReadiness {
    readiness::build(card, has_card_scoped_repairs)
}

fn allowed_repairs(card: &ReviewCard) -> AllowedRepairs {
    repairs::build(card)
}

fn repair_queue(card: &ReviewCard, readiness: &AgentReadiness) -> AgentRepairQueue {
    let mut buckets = Vec::new();
    if has_missing_kind(card, "contract") {
        push_bucket(&mut buckets, "repairable_by_safety_docs");
    }
    if has_missing_kind(card, "guard") {
        push_bucket(&mut buckets, "repairable_by_guard");
    }
    if has_missing_kind(card, "reach") {
        push_bucket(&mut buckets, "repairable_by_test");
    }
    if has_missing_kind(card, "witness") {
        push_bucket(&mut buckets, "requires_witness_receipt");
    }
    if !readiness.ready {
        push_bucket(&mut buckets, "requires_human_review");
        push_bucket(&mut buckets, "do_not_auto_repair");
    }
    if buckets.is_empty() {
        push_bucket(&mut buckets, "review_only");
    }

    AgentRepairQueue {
        summary: repair_queue_summary(&buckets, readiness.ready),
        buckets,
    }
}

pub(crate) fn repair_queue_projection(card: &ReviewCard) -> AgentQueueProjection {
    let allowed_repairs = allowed_repairs(card);
    let agent_readiness = agent_readiness(card, allowed_repairs.has_card_scoped_repairs);
    let repair_queue = repair_queue(card, &agent_readiness);
    AgentQueueProjection {
        agent_readiness,
        repair_queue,
    }
}

fn repair_queue_summary(buckets: &[&'static str], ready: bool) -> String {
    if buckets == ["review_only"] {
        return "No repair bucket selected; inspect the ReviewCard before delegating work."
            .to_string();
    }
    let mut summary = format!("Queue this card as: {}.", buckets.join(", "));
    if !ready {
        summary.push_str(" Keep human review in the loop before delegating edits.");
    }
    summary
}

fn has_missing_kind(card: &ReviewCard, kind: &str) -> bool {
    card.missing.iter().any(|missing| missing.kind == kind)
}

fn push_bucket(buckets: &mut Vec<&'static str>, bucket: &'static str) {
    if !buckets.contains(&bucket) {
        buckets.push(bucket);
    }
}

struct AllowedRepairs {
    repairs: Vec<String>,
    has_card_scoped_repairs: bool,
}

mod context;
mod evidence;
mod readiness;
mod repairs;
mod witness;

use context::{AgentContext, AgentSafetyContract, AgentSourceContext};
use evidence::{AgentMissingEvidence, AgentObligationEvidence};
use witness::AgentWitnessRoute;

#[derive(Serialize)]
struct AgentCard<'a> {
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

#[derive(Clone, Serialize)]
pub(crate) struct AgentQueueProjection {
    pub(crate) agent_readiness: AgentReadiness,
    pub(crate) repair_queue: AgentRepairQueue,
}

#[derive(Clone, Serialize)]
pub(crate) struct AgentReadiness {
    pub(crate) ready: bool,
    pub(crate) state: &'static str,
    pub(crate) reasons: Vec<String>,
}

#[derive(Clone, Serialize)]
pub(crate) struct AgentRepairQueue {
    pub(crate) buckets: Vec<&'static str>,
    pub(crate) summary: String,
}

impl AgentReadiness {
    fn not_ready(state: &'static str, reasons: Vec<String>) -> Self {
        Self {
            ready: false,
            state,
            reasons,
        }
    }
}

#[cfg(test)]
mod tests;
