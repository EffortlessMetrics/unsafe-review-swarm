use super::SafetyObligation;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractEvidence {
    pub present: bool,
    pub summary: String,
}

impl ContractEvidence {
    pub fn missing() -> Self {
        Self {
            present: false,
            summary: "No nearby `# Safety` docs or `SAFETY:` / `Safety:` comment detected"
                .to_string(),
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DischargeEvidence {
    pub present: bool,
    pub summary: String,
}

impl DischargeEvidence {
    pub fn missing() -> Self {
        Self {
            present: false,
            summary: "No visible local guard detected".to_string(),
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvidenceState {
    pub present: bool,
    pub state: String,
    pub summary: String,
}

impl EvidenceState {
    pub fn missing(summary: impl Into<String>) -> Self {
        Self {
            present: false,
            state: "missing".to_string(),
            summary: summary.into(),
        }
    }

    pub fn present(summary: impl Into<String>) -> Self {
        Self {
            present: true,
            state: "present".to_string(),
            summary: summary.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObligationEvidence {
    pub obligation: SafetyObligation,
    pub contract: EvidenceState,
    pub discharge: EvidenceState,
    pub reach: EvidenceState,
    pub witness: EvidenceState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReachEvidence {
    pub state: String,
    pub summary: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelatedTest {
    pub name: String,
    pub file: String,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissingEvidence {
    pub kind: String,
    pub message: String,
}

impl MissingEvidence {
    pub fn new(kind: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_evidence_constructors_set_expected_defaults() {
        let missing = ContractEvidence::missing();
        assert!(!missing.present);
        assert_eq!(
            missing.summary,
            "No nearby `# Safety` docs or `SAFETY:` / `Safety:` comment detected"
        );

        let missing_with = ContractEvidence::missing_with("custom missing");
        assert!(!missing_with.present);
        assert_eq!(missing_with.summary, "custom missing");

        let present = ContractEvidence::present("documented contract");
        assert!(present.present);
        assert_eq!(present.summary, "documented contract");
    }

    #[test]
    fn discharge_and_evidence_state_constructors_preserve_state_and_summary() {
        let discharge_missing = DischargeEvidence::missing();
        assert!(!discharge_missing.present);
        assert_eq!(discharge_missing.summary, "No visible local guard detected");

        let discharge_present = DischargeEvidence::present("checked bounds");
        assert!(discharge_present.present);
        assert_eq!(discharge_present.summary, "checked bounds");

        let state_missing = EvidenceState::missing("guard absent");
        assert!(!state_missing.present);
        assert_eq!(state_missing.state, "missing");
        assert_eq!(state_missing.summary, "guard absent");

        let state_present = EvidenceState::present("guard proven");
        assert!(state_present.present);
        assert_eq!(state_present.state, "present");
        assert_eq!(state_present.summary, "guard proven");
    }

    #[test]
    fn missing_evidence_new_sets_kind_and_message() {
        let missing = MissingEvidence::new("witness", "no receipt found");
        assert_eq!(missing.kind, "witness");
        assert_eq!(missing.message, "no receipt found");
    }
}
