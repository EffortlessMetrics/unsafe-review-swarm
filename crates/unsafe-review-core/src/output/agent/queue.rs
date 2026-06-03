use super::{readiness, repairs};
use crate::domain::ReviewCard;
use serde::Serialize;

pub(super) struct PacketRepairProjection {
    pub(super) allowed_repairs: Vec<String>,
    pub(super) agent_readiness: AgentReadiness,
    pub(super) repair_queue: AgentRepairQueue,
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

pub(crate) const READY_FOR_AGENT: &str = "ready_for_agent";
pub(crate) const REQUIRES_HUMAN_REVIEW: &str = "requires_human_review";
pub(crate) const REQUIRES_WITNESS_RECEIPT: &str = "requires_witness_receipt";
pub(crate) const UNSUPPORTED: &str = "unsupported";

#[derive(Clone, Serialize)]
pub(crate) struct AgentRepairQueue {
    pub(crate) buckets: Vec<&'static str>,
    pub(crate) summary: String,
}

impl AgentReadiness {
    pub(super) fn ready_for_agent(reasons: Vec<String>) -> Self {
        Self {
            ready: true,
            state: READY_FOR_AGENT,
            reasons,
        }
    }

    pub(super) fn not_ready(state: &'static str, reasons: Vec<String>) -> Self {
        Self {
            ready: false,
            state,
            reasons,
        }
    }
}

pub(super) struct AllowedRepairs {
    pub(super) repairs: Vec<String>,
    pub(super) has_card_scoped_repairs: bool,
}

pub(super) fn packet_repair_projection(card: &ReviewCard) -> PacketRepairProjection {
    let allowed_repairs = allowed_repairs(card);
    let agent_readiness = agent_readiness(card, allowed_repairs.has_card_scoped_repairs);
    let repair_queue = repair_queue(card, &agent_readiness);
    PacketRepairProjection {
        allowed_repairs: allowed_repairs.repairs,
        agent_readiness,
        repair_queue,
    }
}

pub(crate) fn repair_queue_projection(card: &ReviewCard) -> AgentQueueProjection {
    let projection = packet_repair_projection(card);
    AgentQueueProjection {
        agent_readiness: projection.agent_readiness,
        repair_queue: projection.repair_queue,
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
    if readiness.state == REQUIRES_HUMAN_REVIEW {
        push_bucket(&mut buckets, "requires_human_review");
    }
    if !readiness.ready {
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
