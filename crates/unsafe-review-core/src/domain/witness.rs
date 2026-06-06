#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WitnessKind {
    Miri,
    CargoCareful,
    AddressSanitizer,
    MemorySanitizer,
    ThreadSanitizer,
    LeakSanitizer,
    Loom,
    Shuttle,
    Kani,
    Crux,
    HumanDeepReview,
    Unsupported,
}

impl WitnessKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Miri => "miri",
            Self::CargoCareful => "cargo-careful",
            Self::AddressSanitizer => "asan",
            Self::MemorySanitizer => "msan",
            Self::ThreadSanitizer => "tsan",
            Self::LeakSanitizer => "lsan",
            Self::Loom => "loom",
            Self::Shuttle => "shuttle",
            Self::Kani => "kani",
            Self::Crux => "crux",
            Self::HumanDeepReview => "human-deep-review",
            Self::Unsupported => "unsupported",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WitnessRoute {
    pub kind: WitnessKind,
    pub reason: String,
    pub command: Option<String>,
    pub required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WitnessEvidence {
    pub present: bool,
    pub summary: String,
    /// True only when an imported receipt records that a runtime witness tool
    /// actually executed (`ran`, `test_targeted`, or `site_reached`).
    /// Human review receipts and missing evidence stay `false`. This reflects
    /// the imported receipt's claim, not execution by unsafe-review itself.
    pub runtime_executed: bool,
    /// Structured verdict carried from the imported witness receipt, when the
    /// receipt recorded one. "confirmed" means the UB-risk hypothesis
    /// reproduced; "not_reproduced" means this single run did not reproduce
    /// it — it is NOT a safety claim. "inconclusive" marks an ambiguous or
    /// partial run. `None` when no receipt verdict was recorded.
    pub verdict: Option<String>,
}

impl WitnessEvidence {
    pub fn missing() -> Self {
        Self {
            present: false,
            summary: "No imported witness receipt was found".to_string(),
            runtime_executed: false,
            verdict: None,
        }
    }

    pub fn missing_with(summary: impl Into<String>) -> Self {
        Self {
            present: false,
            summary: summary.into(),
            runtime_executed: false,
            verdict: None,
        }
    }

    pub fn present(summary: impl Into<String>) -> Self {
        Self {
            present: true,
            summary: summary.into(),
            runtime_executed: false,
            verdict: None,
        }
    }

    pub fn with_runtime_executed(mut self, runtime_executed: bool) -> Self {
        self.runtime_executed = runtime_executed;
        self
    }

    pub fn with_verdict(mut self, verdict: Option<String>) -> Self {
        self.verdict = verdict;
        self
    }

    /// Single source of truth for the per-card confirmation state projection.
    ///
    /// Closed vocabulary:
    /// - `"confirmed"` / `"not_reproduced"` / `"inconclusive"`: an imported
    ///   runtime receipt carries that verdict. "confirmed" means the UB-risk
    ///   hypothesis reproduced; "not_reproduced" means this single run did
    ///   not reproduce it — it is NOT a safety claim.
    /// - `"executed"`: a runtime witness receipt was imported
    ///   (`runtime_executed` true) without a structured verdict.
    /// - `"receipt_imported"`: witness evidence is present but not
    ///   runtime-executed (for example a `human-deep-review` receipt).
    /// - `"pending"`: no witness evidence was imported.
    pub fn confirmation_state(&self) -> &'static str {
        if !self.present {
            return "pending";
        }
        if !self.runtime_executed {
            return "receipt_imported";
        }
        match self.verdict.as_deref() {
            Some("confirmed") => "confirmed",
            Some("not_reproduced") => "not_reproduced",
            Some("inconclusive") => "inconclusive",
            // Receipt validation rejects unknown verdict values, so only an
            // absent verdict reaches here.
            _ => "executed",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn witness_kind_strings_cover_every_variant() {
        let cases = [
            (WitnessKind::Miri, "miri"),
            (WitnessKind::CargoCareful, "cargo-careful"),
            (WitnessKind::AddressSanitizer, "asan"),
            (WitnessKind::MemorySanitizer, "msan"),
            (WitnessKind::ThreadSanitizer, "tsan"),
            (WitnessKind::LeakSanitizer, "lsan"),
            (WitnessKind::Loom, "loom"),
            (WitnessKind::Shuttle, "shuttle"),
            (WitnessKind::Kani, "kani"),
            (WitnessKind::Crux, "crux"),
            (WitnessKind::HumanDeepReview, "human-deep-review"),
            (WitnessKind::Unsupported, "unsupported"),
        ];

        for (kind, expected) in cases {
            assert_eq!(kind.as_str(), expected);
        }
    }

    #[test]
    fn witness_evidence_constructors_preserve_presence_and_summary() {
        let missing = WitnessEvidence::missing();
        assert!(!missing.present);
        assert_eq!(missing.summary, "No imported witness receipt was found");

        let missing_with = WitnessEvidence::missing_with("receipt expired");
        assert!(!missing_with.present);
        assert_eq!(missing_with.summary, "receipt expired");

        let present = WitnessEvidence::present("miri receipt imported");
        assert!(present.present);
        assert_eq!(present.summary, "miri receipt imported");
    }

    #[test]
    fn confirmation_state_is_pending_without_witness_evidence() {
        assert_eq!(WitnessEvidence::missing().confirmation_state(), "pending");
        assert_eq!(
            WitnessEvidence::missing_with("receipt expired").confirmation_state(),
            "pending"
        );
    }

    #[test]
    fn confirmation_state_is_receipt_imported_for_non_runtime_evidence() {
        let evidence = WitnessEvidence::present("human-deep-review receipt imported");
        assert_eq!(evidence.confirmation_state(), "receipt_imported");
        // A verdict never upgrades evidence that did not record runtime
        // execution.
        let evidence = evidence.with_verdict(Some("confirmed".to_string()));
        assert_eq!(evidence.confirmation_state(), "receipt_imported");
    }

    #[test]
    fn confirmation_state_is_executed_for_runtime_receipt_without_verdict() {
        let evidence =
            WitnessEvidence::present("miri receipt imported").with_runtime_executed(true);
        assert_eq!(evidence.confirmation_state(), "executed");
    }

    #[test]
    fn confirmation_state_projects_runtime_receipt_verdicts() {
        for verdict in ["confirmed", "not_reproduced", "inconclusive"] {
            let evidence = WitnessEvidence::present("miri receipt imported")
                .with_runtime_executed(true)
                .with_verdict(Some(verdict.to_string()));
            assert_eq!(evidence.confirmation_state(), verdict);
        }
    }
}
