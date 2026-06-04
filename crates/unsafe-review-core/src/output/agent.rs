use crate::domain::ReviewCard;
use crate::output::REVIEWCARD_TRUST_BOUNDARY as TRUST_BOUNDARY;
use serde::Serialize;

pub(crate) use queue::{AgentQueueProjection, AgentReadiness};

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

pub(crate) use queue::repair_queue_projection;

mod context;
mod evidence;
mod packet;
mod queue;
mod readiness;
mod repairs;

#[cfg(test)]
mod tests;

pub(crate) fn render(card: &ReviewCard) -> String {
    render_pretty(&packet::AgentPacket::from(card))
}

fn render_pretty(value: &impl Serialize) -> String {
    match serde_json::to_string_pretty(value) {
        Ok(text) => text,
        Err(err) => format!("{{\n  \"error\": \"agent packet serialization failed: {err}\"\n}}"),
    }
}
