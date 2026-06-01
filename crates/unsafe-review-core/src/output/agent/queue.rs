use super::model::{AgentReadiness, AgentRepairQueue};
use crate::domain::ReviewCard;

pub(super) fn build(card: &ReviewCard, readiness: &AgentReadiness) -> AgentRepairQueue {
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
        summary: summary(&buckets, readiness.ready),
        buckets,
    }
}

fn summary(buckets: &[&'static str], ready: bool) -> String {
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
