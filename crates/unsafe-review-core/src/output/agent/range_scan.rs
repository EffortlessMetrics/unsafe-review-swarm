//! `file_range_scan` envelope — SPEC-0033.
//!
//! Wraps zero or more per-card [`packet::AgentPacket`]s whose unsafe site
//! overlaps the requested `file:Y-Z` range.  The list is ordered by site line.
//! An empty list means no reviewable seam overlaps those lines, never "safe".

use super::packet::AgentPacket;
use super::{DO_NOT_DO, TRUST_BOUNDARY};
use crate::domain::ReviewCard;
use crate::domain::coverage::BaselineState;
use serde::Serialize;

/// The `mode` string stamped on the envelope (SPEC-0033).
const MODE: &str = "file_range_scan";

/// A monotonic counter used as the `staleness_marker.refresh_generation`.
///
/// In the current implementation this is derived from the analysis-output
/// `schema_version`.  A future build-time or runtime generation counter could
/// replace this; the contract is: two reads with different generation values
/// mean different analysis runs, so stale diagnostics can be detected.
const SCHEMA_VERSION: &str = "0.1";

/// The `staleness_marker` field (SPEC-0033).
///
/// Carries a `refresh_generation` id so an agent comparing two reads can tell
/// whether the diagnostics changed.  This is a freshness *signal*, never a
/// freshness *guarantee*.
#[derive(Serialize)]
pub(super) struct StalenessMaker<'a> {
    /// Monotonic generation id — increments with each new analysis.
    refresh_generation: &'static str,
    /// The analyzed base that the generation covers (schema version tag).
    analyzed_base: &'a str,
}

/// The top-level `file_range_scan` envelope (SPEC-0033).
///
/// Mode is always `file_range_scan`.  The `packets` list contains zero or more
/// per-card packets whose unsafe site overlaps the requested range, ordered
/// deterministically by site line.  The `staleness_marker` lets an agent
/// detect stale diagnostics across reads.
#[derive(Serialize)]
pub(super) struct FileRangeScanEnvelope<'a> {
    schema_version: &'static str,
    tool: &'static str,
    mode: &'static str,
    policy: &'static str,
    trust_boundary: &'static str,
    /// Requested file path (normalized to forward slashes).
    queried_file: String,
    queried_line_start: u32,
    queried_line_end: u32,
    changed_only: bool,
    /// Zero or more per-card packets whose site overlaps the range.
    packets: Vec<AgentPacket<'a>>,
    /// Advisory signal: never a safety guarantee.
    empty_means: &'static str,
    staleness_marker: StalenessMaker<'a>,
    do_not_do: &'static [&'static str],
}

impl<'a> FileRangeScanEnvelope<'a> {
    pub(super) fn build(
        queried_file: String,
        queried_line_start: u32,
        queried_line_end: u32,
        changed_only: bool,
        cards: Vec<&'a ReviewCard>,
        analyzed_base: &'a str,
    ) -> Self {
        let packets = cards.into_iter().map(AgentPacket::from).collect();
        Self {
            schema_version: SCHEMA_VERSION,
            tool: "unsafe-review",
            mode: MODE,
            policy: "advisory",
            trust_boundary: TRUST_BOUNDARY,
            queried_file,
            queried_line_start,
            queried_line_end,
            changed_only,
            packets,
            empty_means: "no reviewable seam overlaps those lines — never that those lines are safe",
            staleness_marker: StalenessMaker {
                refresh_generation: SCHEMA_VERSION,
                analyzed_base,
            },
            do_not_do: DO_NOT_DO,
        }
    }
}

/// Return `true` when the card's unsafe site overlaps the half-open range
/// `[line_start, line_end]` (both endpoints inclusive, 1-based).
///
/// A card whose site is at line `L` overlaps iff `line_start <= L <= line_end`.
pub(crate) fn site_overlaps_range(card: &ReviewCard, line_start: u32, line_end: u32) -> bool {
    let site_line = card.site.location.line as u32;
    site_line >= line_start && site_line <= line_end
}

/// Return `true` when the card is in a baseline state that counts as
/// "new or worsened" — the two states SPEC-0030 uses to flag changed lines.
pub(crate) fn is_new_or_worsened(card: &ReviewCard) -> bool {
    let state = card.coverage_block().baseline_state;
    matches!(state, BaselineState::New | BaselineState::Worsened)
}
