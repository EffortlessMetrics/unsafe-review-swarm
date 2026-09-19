//! Phase latency clocks for review commands (`check`, `repo`, `first-pr`).
//!
//! The CLI emit layer owns every wall-clock measurement: core exposes
//! deterministic counts (files, bytes, lines, sites, cards) and per-file
//! `scan_ms` diagnostics, but only the process that runs the command can
//! observe how long each phase took. A [`PhaseClock`] records named spans
//! between ticks; [`LatencyReceipt`] serializes them with the identities a
//! later baseline comparison needs (tool, command, scope, input digest) and
//! nothing it must not retain (no source contents, no card contents).
//!
//! Receipts are written only when the caller passes `--latency-out <path>`,
//! and only for runs that complete the emit path. A failed or interrupted
//! run emits no receipt rather than a partial one.
//!
//! Diagnostic only — not a coverage claim, proof, UB-free, Miri-clean,
//! site-execution, or performance guarantee.

use std::time::Instant;

/// Phase names emitted in fixed order by the review commands. New phases
/// append at the end; existing names are never renamed so baseline receipts
/// stay comparable across tool versions.
pub(crate) const PHASE_INPUT: &str = "input_resolution";
pub(crate) const PHASE_ANALYZE: &str = "analyze";
pub(crate) const PHASE_RECEIPT_AUDIT: &str = "receipt_audit";
pub(crate) const PHASE_POLICY_EVAL: &str = "policy_eval";
pub(crate) const PHASE_PROJECTIONS: &str = "projections";
pub(crate) const PHASE_ARTIFACT_WRITES: &str = "artifact_writes";

/// Schema version of the serialized latency receipt.
pub(crate) const LATENCY_RECEIPT_SCHEMA_VERSION: &str = "1.0";

/// One measured phase: wall-clock milliseconds between the previous tick
/// (or clock start) and this tick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LatencyPhase {
    pub name: &'static str,
    pub elapsed_ms: u64,
}

/// Wall-clock phase recorder. Exactly one tick per phase, in emit order.
pub(crate) struct PhaseClock {
    last: Instant,
    phases: Vec<LatencyPhase>,
}

impl PhaseClock {
    pub(crate) fn start() -> Self {
        Self {
            last: Instant::now(),
            phases: Vec::new(),
        }
    }

    /// Close the current span under `name` and start the next one.
    pub(crate) fn tick(&mut self, name: &'static str) {
        let now = Instant::now();
        let elapsed_ms = now
            .duration_since(self.last)
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX);
        self.last = now;
        self.phases.push(LatencyPhase { name, elapsed_ms });
    }

    pub(crate) fn phases(&self) -> &[LatencyPhase] {
        &self.phases
    }
}

/// Machine-readable latency receipt for one completed review command.
pub(crate) struct LatencyReceipt<'a> {
    pub tool_version: &'a str,
    pub command: &'a str,
    pub scope: &'a str,
    /// Caller-supplied input identity (diff digest, base ref, or repo root
    /// marker). Never source contents.
    pub input_identity: &'a str,
    pub phases: Vec<LatencyPhase>,
    pub cards: usize,
    pub output_bytes_total: u64,
}

impl LatencyReceipt<'_> {
    pub(crate) fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "schema_version": LATENCY_RECEIPT_SCHEMA_VERSION,
            "tool": "unsafe-review",
            "tool_version": self.tool_version,
            "command": self.command,
            "scope": self.scope,
            "input_identity": self.input_identity,
            "phases": self.phases.iter().map(|phase| serde_json::json!({
                "name": phase.name,
                "elapsed_ms": phase.elapsed_ms,
            })).collect::<Vec<_>>(),
            "total_ms": self.phases.iter().map(|phase| phase.elapsed_ms).sum::<u64>(),
            "cards": self.cards,
            "output_bytes_total": self.output_bytes_total,
        })
    }
}

/// Input identity for a receipt: the strongest stable identifier the
/// provenance block offers, preferring content digests over refs over the
/// root marker. Never source contents.
pub(crate) fn input_identity(provenance: &unsafe_review_core::Provenance) -> String {
    if let Some(digest) = &provenance.diff_sha256 {
        return format!("diff-sha256:{digest}");
    }
    if let Some(base) = &provenance.base_sha {
        return format!("base:{base}");
    }
    if let Some(root) = &provenance.root_abs {
        return format!("root:{root}");
    }
    "root:unspecified".to_string()
}

/// Build and write the receipt when `path` is `Some`; a no-op returning
/// `Ok(())` when the caller did not request one. Write failures are errors:
/// an explicitly requested output must not fail silently.
pub(crate) fn write_receipt_if_requested(
    path: Option<&std::path::Path>,
    command: &'static str,
    scope: &'static str,
    provenance: &unsafe_review_core::Provenance,
    clock: &PhaseClock,
    cards: usize,
    output_bytes_total: u64,
) -> Result<(), String> {
    let Some(path) = path else {
        return Ok(());
    };
    let receipt = LatencyReceipt {
        tool_version: env!("CARGO_PKG_VERSION"),
        command,
        scope,
        input_identity: &input_identity(provenance),
        phases: clock.phases().to_vec(),
        cards,
        output_bytes_total,
    };
    write_latency_receipt(path, &receipt).map(|_| ())
}

/// Serialize `receipt` as JSON and write it to `path`, creating parent
/// directories. Returns the bytes written.
pub(crate) fn write_latency_receipt(
    path: &std::path::Path,
    receipt: &LatencyReceipt<'_>,
) -> Result<u64, String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("create {} failed: {err}", parent.display()))?;
    }
    let rendered = serde_json::to_string_pretty(&receipt.to_json())
        .map_err(|err| format!("serialize latency receipt failed: {err}"))?;
    std::fs::write(path, rendered.as_bytes())
        .map_err(|err| format!("write {} failed: {err}", path.display()))?;
    Ok(rendered.len() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_record_phases_in_emit_order() {
        let mut clock = PhaseClock::start();
        clock.tick(PHASE_INPUT);
        clock.tick(PHASE_ANALYZE);
        let names: Vec<&str> = clock.phases().iter().map(|phase| phase.name).collect();
        assert_eq!(names, vec![PHASE_INPUT, PHASE_ANALYZE]);
    }

    #[test]
    fn receipt_carries_identities_and_no_source_contents() {
        let mut clock = PhaseClock::start();
        clock.tick(PHASE_ANALYZE);
        let receipt = LatencyReceipt {
            tool_version: "0.0.0-test",
            command: "check",
            scope: "diff",
            input_identity: "sha256:abc",
            phases: clock.phases().to_vec(),
            cards: 3,
            output_bytes_total: 100,
        };
        let json = receipt.to_json();
        assert_eq!(json["schema_version"], "1.0");
        assert_eq!(json["tool"], "unsafe-review");
        assert_eq!(json["command"], "check");
        assert_eq!(json["phases"][0]["name"], PHASE_ANALYZE);
        assert_eq!(json["cards"], 3);
        let serialized = json.to_string();
        assert!(
            !serialized.contains("unsafe {"),
            "receipt must never embed source contents"
        );
    }
}
