#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReviewClass {
    GuardedAndWitnessed,
    GuardedUnwitnessed,
    ContractMissing,
    GuardMissing,
    ReachableUnwitnessed,
    UnsafeUnreached,
    WitnessMismatch,
    RequiresLoom,
    RequiresSanitizer,
    RequiresKaniOrCrux,
    MiriUnsupported,
    StaticUnknown,
    BaselineKnown,
    Suppressed,
}

impl ReviewClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::GuardedAndWitnessed => "guarded_and_witnessed",
            Self::GuardedUnwitnessed => "guarded_unwitnessed",
            Self::ContractMissing => "contract_missing",
            Self::GuardMissing => "guard_missing",
            Self::ReachableUnwitnessed => "reachable_unwitnessed",
            Self::UnsafeUnreached => "unsafe_unreached",
            Self::WitnessMismatch => "witness_mismatch",
            Self::RequiresLoom => "requires_loom",
            Self::RequiresSanitizer => "requires_sanitizer",
            Self::RequiresKaniOrCrux => "requires_kani_or_crux",
            Self::MiriUnsupported => "miri_unsupported",
            Self::StaticUnknown => "static_unknown",
            Self::BaselineKnown => "baseline_known",
            Self::Suppressed => "suppressed",
        }
    }

    pub fn is_actionable(&self) -> bool {
        matches!(
            self,
            Self::GuardedUnwitnessed
                | Self::ContractMissing
                | Self::GuardMissing
                | Self::ReachableUnwitnessed
                | Self::UnsafeUnreached
                | Self::RequiresLoom
                | Self::RequiresSanitizer
                | Self::RequiresKaniOrCrux
                | Self::MiriUnsupported
                | Self::StaticUnknown
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProofPath {
    ObservableRedGreen,
    MutationMiriModel,
    SourceRouteOnly,
    HelperGated,
    HumanReviewOnly,
}

impl ProofPath {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ObservableRedGreen => "observable_red_green",
            Self::MutationMiriModel => "mutation_miri_model",
            Self::SourceRouteOnly => "source_route_only",
            Self::HelperGated => "helper_gated",
            Self::HumanReviewOnly => "human_review_only",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Priority {
    High,
    Medium,
    Low,
}

impl Priority {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Confidence {
    High,
    Medium,
    Low,
    Unknown,
}

impl Confidence {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
            Self::Unknown => "unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_class_strings_cover_every_variant() {
        let cases = [
            (ReviewClass::GuardedAndWitnessed, "guarded_and_witnessed"),
            (ReviewClass::GuardedUnwitnessed, "guarded_unwitnessed"),
            (ReviewClass::ContractMissing, "contract_missing"),
            (ReviewClass::GuardMissing, "guard_missing"),
            (ReviewClass::ReachableUnwitnessed, "reachable_unwitnessed"),
            (ReviewClass::UnsafeUnreached, "unsafe_unreached"),
            (ReviewClass::WitnessMismatch, "witness_mismatch"),
            (ReviewClass::RequiresLoom, "requires_loom"),
            (ReviewClass::RequiresSanitizer, "requires_sanitizer"),
            (ReviewClass::RequiresKaniOrCrux, "requires_kani_or_crux"),
            (ReviewClass::MiriUnsupported, "miri_unsupported"),
            (ReviewClass::StaticUnknown, "static_unknown"),
            (ReviewClass::BaselineKnown, "baseline_known"),
            (ReviewClass::Suppressed, "suppressed"),
        ];

        for (class, expected) in cases {
            assert_eq!(class.as_str(), expected);
        }
    }

    #[test]
    fn review_class_actionability_keeps_closed_outcomes_non_actionable() {
        let actionable = [
            ReviewClass::GuardedUnwitnessed,
            ReviewClass::ContractMissing,
            ReviewClass::GuardMissing,
            ReviewClass::ReachableUnwitnessed,
            ReviewClass::UnsafeUnreached,
            ReviewClass::RequiresLoom,
            ReviewClass::RequiresSanitizer,
            ReviewClass::RequiresKaniOrCrux,
            ReviewClass::MiriUnsupported,
            ReviewClass::StaticUnknown,
        ];
        let non_actionable = [
            ReviewClass::GuardedAndWitnessed,
            ReviewClass::WitnessMismatch,
            ReviewClass::BaselineKnown,
            ReviewClass::Suppressed,
        ];

        for class in actionable {
            assert!(
                class.is_actionable(),
                "{} should be actionable",
                class.as_str()
            );
        }
        for class in non_actionable {
            assert!(
                !class.is_actionable(),
                "{} should not be actionable",
                class.as_str()
            );
        }
    }

    #[test]
    fn priority_and_confidence_strings_cover_every_variant() {
        assert_eq!(Priority::High.as_str(), "high");
        assert_eq!(Priority::Medium.as_str(), "medium");
        assert_eq!(Priority::Low.as_str(), "low");

        assert_eq!(Confidence::High.as_str(), "high");
        assert_eq!(Confidence::Medium.as_str(), "medium");
        assert_eq!(Confidence::Low.as_str(), "low");
        assert_eq!(Confidence::Unknown.as_str(), "unknown");
    }

    #[test]
    fn proof_path_strings_cover_every_variant() {
        let cases = [
            (ProofPath::ObservableRedGreen, "observable_red_green"),
            (ProofPath::MutationMiriModel, "mutation_miri_model"),
            (ProofPath::SourceRouteOnly, "source_route_only"),
            (ProofPath::HelperGated, "helper_gated"),
            (ProofPath::HumanReviewOnly, "human_review_only"),
        ];

        for (path, expected) in cases {
            assert_eq!(path.as_str(), expected);
        }
    }
}
