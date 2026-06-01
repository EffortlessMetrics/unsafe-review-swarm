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

mod model;
mod queue;
mod readiness;
mod repairs;

use model::{AgentPacket, AllowedRepairs};
pub(crate) use model::{AgentQueueProjection, AgentReadiness};
use queue::repair_queue;
pub(crate) use queue::repair_queue_projection;

fn agent_readiness(card: &ReviewCard, has_card_scoped_repairs: bool) -> AgentReadiness {
    readiness::build(card, has_card_scoped_repairs)
}

fn allowed_repairs(card: &ReviewCard) -> AllowedRepairs {
    repairs::build(card)
}

#[cfg(test)]
mod tests;
