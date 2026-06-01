use crate::domain::ReviewCard;

mod context;
mod packet;
mod queue;
mod readiness;
mod repairs;
#[cfg(test)]
mod tests;

pub(crate) use packet::render;
pub(crate) use queue::{AgentQueueProjection, AgentReadiness, repair_queue_projection};

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

struct AllowedRepairs {
    repairs: Vec<String>,
    has_card_scoped_repairs: bool,
}

fn agent_readiness(card: &ReviewCard, has_card_scoped_repairs: bool) -> AgentReadiness {
    readiness::build(card, has_card_scoped_repairs)
}

fn allowed_repairs(card: &ReviewCard) -> AllowedRepairs {
    repairs::build(card)
}
