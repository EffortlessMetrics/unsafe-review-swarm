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
mod range_scan;
mod readiness;
mod repairs;

#[cfg(test)]
mod tests;

pub(crate) fn render(card: &ReviewCard) -> String {
    render_pretty(&packet::AgentPacket::from(card))
}

/// Render a `file_range_scan` envelope for SPEC-0033.
///
/// Accepts a pre-file-filtered slice of cards (already restricted to the
/// requested file).  Applies line-range and optional `changed_only` filters,
/// sorts the result deterministically by site line then card id, and wraps the
/// matching packets in the `file_range_scan` envelope.
pub(crate) fn render_range_scan<'a>(
    queried_file: String,
    queried_line_start: u32,
    queried_line_end: u32,
    changed_only: bool,
    file_cards: &[&'a ReviewCard],
    analyzed_base: &'a str,
) -> String {
    let mut matching: Vec<&'a ReviewCard> = file_cards
        .iter()
        .copied()
        .filter(|card| range_scan::site_overlaps_range(card, queried_line_start, queried_line_end))
        .filter(|card| !changed_only || range_scan::is_new_or_worsened(card))
        .collect();
    // Deterministic order: ascending site line then card id.
    matching.sort_by(|a, b| {
        a.site
            .location
            .line
            .cmp(&b.site.location.line)
            .then_with(|| a.id.0.cmp(&b.id.0))
    });
    let envelope = range_scan::FileRangeScanEnvelope::build(
        queried_file,
        queried_line_start,
        queried_line_end,
        changed_only,
        matching,
        analyzed_base,
    );
    render_pretty(&envelope)
}

fn render_pretty(value: &impl Serialize) -> String {
    match serde_json::to_string_pretty(value) {
        Ok(text) => text,
        Err(err) => format!("{{\n  \"error\": \"agent packet serialization failed: {err}\"\n}}"),
    }
}
