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
}

impl WitnessEvidence {
    pub fn missing() -> Self {
        Self {
            present: false,
            summary: "No imported witness receipt was found".to_string(),
        }
    }

    pub fn missing_with(summary: impl Into<String>) -> Self {
        Self {
            present: false,
            summary: summary.into(),
        }
    }

    pub fn present(summary: impl Into<String>) -> Self {
        Self {
            present: true,
            summary: summary.into(),
        }
    }
}
