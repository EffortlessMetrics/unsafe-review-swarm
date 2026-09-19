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

/// Policy outcome of the run that produced the receipt, as decided by the
/// emit layer after policy evaluation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PolicyOutcome {
    Pass,
    Fail,
    /// The command never evaluates a policy (advisory-only paths).
    NotEvaluated,
}

impl PolicyOutcome {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            PolicyOutcome::Pass => "pass",
            PolicyOutcome::Fail => "fail",
            PolicyOutcome::NotEvaluated => "not_evaluated",
        }
    }
}

/// Terminal completeness state of the run. The receipt is written only on
/// the completed emit path, so `analysis` is always `"complete"` here; the
/// struct exists so a partial, capped, cancelled, or policy-failed run can
/// never be mistaken for a clean completed analysis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LatencyOutcome {
    pub policy: PolicyOutcome,
    /// True when the scan emitted fewer cards than it discovered because the
    /// `max_cards` cap was exceeded. Every count in the receipt is
    /// understated while this is true.
    pub scan_capped: bool,
    pub card_cap: Option<usize>,
    /// Files named by a diff scope that were not scanned at all. Non-empty
    /// means the run reviewed less than the diff names.
    pub unresolved_diff_files: usize,
    pub rejected_diff_files: usize,
}

/// Machine-readable latency receipt for one completed review command.
pub(crate) struct LatencyReceipt<'a> {
    pub tool_version: &'a str,
    /// Exact tool source commit baked at build time (`build.rs`), or
    /// `"unknown"` when built without git metadata.
    pub tool_commit: &'a str,
    /// Digest of the running binary, so a receipt can be tied to the exact
    /// bytes that produced it. `"unknown"` when the binary is unreadable.
    pub tool_binary_digest: &'a str,
    /// Toolchain that built the binary, baked at build time.
    pub build_rustc: &'a str,
    pub command: &'a str,
    pub scope: &'a str,
    /// Caller-supplied input identity (diff digest, base/head pair, or a
    /// machine-local root fingerprint). Never source contents, never an
    /// absolute path.
    pub input_identity: &'a str,
    /// Digest of the command options that affect the analysis
    /// (policy, caps, format). Paths and machine-local values are excluded.
    pub options_digest: &'a str,
    /// Whether the analyzed worktree had uncommitted changes (`None` when
    /// git was unavailable).
    pub dirty_worktree: Option<bool>,
    pub outcome: LatencyOutcome,
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
            "tool_commit": self.tool_commit,
            "tool_binary_digest": self.tool_binary_digest,
            "build_rustc": self.build_rustc,
            "command": self.command,
            "scope": self.scope,
            "input_identity": self.input_identity,
            "options_digest": self.options_digest,
            "dirty_worktree": self.dirty_worktree,
            "outcome": {
                "analysis": "complete",
                "policy": self.outcome.policy.as_str(),
                "scan_capped": self.outcome.scan_capped,
                "card_cap": self.outcome.card_cap,
                "unresolved_diff_files": self.outcome.unresolved_diff_files,
                "rejected_diff_files": self.outcome.rejected_diff_files,
            },
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
/// provenance block offers. A diff digest identifies the exact input; a
/// base/head pair identifies the exact range (a base alone does not, since
/// one base admits many heads); otherwise a machine-local fingerprint of
/// the root path, which is reproducible on the same machine but portable
/// nowhere. Never source contents, never a raw absolute path.
pub(crate) fn input_identity(provenance: &unsafe_review_core::Provenance) -> String {
    if let Some(digest) = &provenance.diff_sha256 {
        return format!("diff-sha256:{digest}");
    }
    if let (Some(base), Some(head)) = (&provenance.base_sha, &provenance.head_sha) {
        return format!("base:{base}:head:{head}");
    }
    if let Some(base) = &provenance.base_sha {
        return format!("base:{base}");
    }
    if let Some(root) = &provenance.root_abs {
        let fingerprint = unsafe_review_core::sha256_hex_of(root.as_bytes());
        let short = fingerprint.get(..16).unwrap_or(&fingerprint);
        return format!("root-path-sha256:{short}");
    }
    "root:unspecified".to_string()
}

/// Canonical digest of the command options that affect the analysis.
/// Each pair is `name=value` in the caller's order; paths and other
/// machine-local values must not be passed in.
pub(crate) fn digest_options(pairs: &[(&str, String)]) -> String {
    let canonical = pairs
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join("\n");
    let digest = unsafe_review_core::sha256_hex_of(canonical.as_bytes());
    format!("sha256:{}", digest.get(..16).unwrap_or(&digest))
}

/// Digest of the currently running binary, or `"unknown"` when the binary
/// cannot be read back (deleted or unreadable executable).
///
/// FNV-1a 64, not SHA-256: a debug binary exceeds 100MB and a cryptographic
/// hash costs seconds on every receipt write, which would dominate the very
/// phases being measured. The algorithm rides in the value
/// (`"fnv1a64:..."`), so the binding is explicit: build identity, not a
/// tamper proof.
pub(crate) fn running_binary_digest() -> String {
    let bytes = std::env::current_exe()
        .ok()
        .and_then(|path| std::fs::read(path).ok());
    match bytes {
        Some(bytes) => {
            let mut hash: u64 = 0xcbf29ce484222325;
            for byte in &bytes {
                hash ^= u64::from(*byte);
                hash = hash.wrapping_mul(0x100000001b3);
            }
            format!("fnv1a64:{hash:016x}")
        }
        None => "unknown".to_string(),
    }
}

/// Lexically normalize `path` for collision comparison: absolutize relative
/// paths against the current directory and resolve `.`/`..` components
/// without touching the filesystem (targets may not exist yet).
fn normalize_for_collision(path: &std::path::Path) -> std::path::PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join(path)
    };
    let mut normalized = std::path::PathBuf::new();
    for component in absolute.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

/// Reject a `--latency-out` destination that collides with a command output.
/// Each protected entry is `(flag name, path)`; on collision the error names
/// both flags. Without this a receipt write silently replaces the rendered
/// report (or a bundle artifact), destroying the run's primary output.
pub(crate) fn reject_latency_collision(
    latency_out: &std::path::Path,
    protected: &[(&str, &std::path::Path)],
) -> Result<(), String> {
    let candidate = normalize_for_collision(latency_out);
    for (flag, path) in protected {
        if candidate == normalize_for_collision(path) {
            return Err(format!(
                "--latency-out must not overwrite the {flag} output (both resolve to {})",
                candidate.display()
            ));
        }
    }
    Ok(())
}

/// Inputs for one receipt write, bundled so the emit call sites stay
/// readable.
pub(crate) struct ReceiptParams<'a> {
    pub path: Option<&'a std::path::Path>,
    pub command: &'static str,
    pub scope: &'a str,
    pub provenance: &'a unsafe_review_core::Provenance,
    pub options_digest: &'a str,
    pub outcome: &'a LatencyOutcome,
    pub clock: &'a PhaseClock,
    pub cards: usize,
    pub output_bytes_total: u64,
}

/// Build and write the receipt when `path` is `Some`; a no-op returning
/// `Ok(())` when the caller did not request one. Write failures are errors:
/// an explicitly requested output must not fail silently.
///
/// The receipt is written only for runs that reach the emit path, after
/// policy evaluation, so `outcome` always describes a completed analysis.
pub(crate) fn write_receipt_if_requested(params: ReceiptParams<'_>) -> Result<(), String> {
    let Some(path) = params.path else {
        return Ok(());
    };
    let binary_digest = running_binary_digest();
    let receipt = LatencyReceipt {
        tool_version: env!("CARGO_PKG_VERSION"),
        tool_commit: env!("UNSAFE_REVIEW_BUILD_COMMIT"),
        tool_binary_digest: &binary_digest,
        build_rustc: env!("UNSAFE_REVIEW_BUILD_RUSTC"),
        command: params.command,
        scope: params.scope,
        input_identity: &input_identity(params.provenance),
        options_digest: params.options_digest,
        dirty_worktree: params.provenance.dirty_worktree,
        outcome: params.outcome.clone(),
        phases: params.clock.phases().to_vec(),
        cards: params.cards,
        output_bytes_total: params.output_bytes_total,
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

    fn test_outcome() -> LatencyOutcome {
        LatencyOutcome {
            policy: PolicyOutcome::Pass,
            scan_capped: false,
            card_cap: None,
            unresolved_diff_files: 0,
            rejected_diff_files: 0,
        }
    }

    #[test]
    fn receipt_carries_identities_and_no_source_contents() {
        let mut clock = PhaseClock::start();
        clock.tick(PHASE_ANALYZE);
        let receipt = LatencyReceipt {
            tool_version: "0.0.0-test",
            tool_commit: "abc123",
            tool_binary_digest: "fnv1a64:def456",
            build_rustc: "rustc 1.0.0-test",
            command: "check",
            scope: "diff",
            input_identity: "sha256:abc",
            options_digest: "sha256:opts",
            dirty_worktree: Some(false),
            outcome: test_outcome(),
            phases: clock.phases().to_vec(),
            cards: 3,
            output_bytes_total: 100,
        };
        let json = receipt.to_json();
        assert_eq!(json["schema_version"], "1.0");
        assert_eq!(json["tool"], "unsafe-review");
        assert_eq!(json["command"], "check");
        assert_eq!(json["tool_commit"], "abc123");
        assert_eq!(json["outcome"]["analysis"], "complete");
        assert_eq!(json["outcome"]["policy"], "pass");
        assert_eq!(json["phases"][0]["name"], PHASE_ANALYZE);
        assert_eq!(json["cards"], 3);
        let serialized = json.to_string();
        assert!(
            !serialized.contains("unsafe {"),
            "receipt must never embed source contents"
        );
    }

    #[test]
    fn input_identity_prefers_diff_then_base_head_pair() {
        use unsafe_review_core::Provenance;
        let mut provenance = Provenance::new_now();
        provenance.diff_sha256 = Some("digest".to_string());
        provenance.base_sha = Some("base".to_string());
        provenance.head_sha = Some("head".to_string());
        assert_eq!(input_identity(&provenance), "diff-sha256:digest");
        provenance.diff_sha256 = None;
        assert_eq!(input_identity(&provenance), "base:base:head:head");
        provenance.head_sha = None;
        assert_eq!(input_identity(&provenance), "base:base");
    }

    #[test]
    fn input_identity_never_embeds_absolute_paths() {
        use unsafe_review_core::Provenance;
        let mut provenance = Provenance::new_now();
        provenance.root_abs = Some("/home/someone/secret/checkout".to_string());
        let identity = input_identity(&provenance);
        assert!(
            identity.starts_with("root-path-sha256:"),
            "root fallback must be a fingerprint, got: {identity}"
        );
        assert!(
            !identity.contains("someone"),
            "identity must not leak path contents: {identity}"
        );
        let again = input_identity(&provenance);
        assert_eq!(identity, again, "fingerprint must be stable");
    }

    #[test]
    fn options_digest_is_stable_and_option_sensitive() {
        let pairs = [
            ("policy", "advisory".to_string()),
            ("max_cards", "50".to_string()),
        ];
        let first = digest_options(&pairs);
        assert_eq!(first, digest_options(&pairs));
        let changed = [
            ("policy", "no-new-debt".to_string()),
            ("max_cards", "50".to_string()),
        ];
        assert_ne!(first, digest_options(&changed));
    }

    #[test]
    fn latency_out_collision_with_report_is_rejected() -> Result<(), String> {
        use std::path::Path;
        let collision = reject_latency_collision(
            Path::new("target/report.json"),
            &[("--out", Path::new("target/report.json"))],
        );
        assert!(collision.is_err());
        let dotted = reject_latency_collision(
            Path::new("target/sub/../report.json"),
            &[("--out", Path::new("target/./report.json"))],
        );
        assert!(dotted.is_err(), "normalization must catch ./ and ../");
        reject_latency_collision(
            Path::new("target/latency.json"),
            &[("--out", Path::new("target/report.json"))],
        )?;
        Ok(())
    }
}
