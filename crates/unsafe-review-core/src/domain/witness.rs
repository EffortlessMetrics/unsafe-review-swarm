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
}

impl WitnessEvidence {
    pub fn missing() -> Self {
        Self {
            present: false,
            summary: "No imported witness receipt was found".to_string(),
            runtime_executed: false,
        }
    }

    pub fn missing_with(summary: impl Into<String>) -> Self {
        Self {
            present: false,
            summary: summary.into(),
            runtime_executed: false,
        }
    }

    pub fn present(summary: impl Into<String>) -> Self {
        Self {
            present: true,
            summary: summary.into(),
            runtime_executed: false,
        }
    }

    pub fn with_runtime_executed(mut self, runtime_executed: bool) -> Self {
        self.runtime_executed = runtime_executed;
        self
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
}
