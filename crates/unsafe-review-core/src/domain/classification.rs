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
                | Self::WitnessMismatch
                | Self::RequiresLoom
                | Self::RequiresSanitizer
                | Self::RequiresKaniOrCrux
                | Self::MiriUnsupported
                | Self::StaticUnknown
        )
    }

    /// Return the SARIF result level for this class.
    ///
    /// The mapping is the single severity language for all surfaces — SARIF
    /// `level` and LSP diagnostic `severity` both derive from this table via
    /// [`Self::lsp_severity`].  Priority is a ranking/budget signal only and
    /// must NOT drive severity.
    ///
    /// Advisory boundary: "warning" means "needs attention" for this advisory
    /// tool, not a merge-blocking verdict.
    pub fn sarif_level(&self) -> &'static str {
        match self {
            // Non-actionable — evidence present or administratively closed.
            Self::GuardedAndWitnessed | Self::BaselineKnown | Self::Suppressed => "none",
            // Partially-resolved — has some evidence but not a full witness chain.
            Self::GuardedUnwitnessed | Self::UnsafeUnreached | Self::StaticUnknown => "note",
            // Actionable — evidence missing or receipt mismatched.
            Self::ContractMissing
            | Self::GuardMissing
            | Self::ReachableUnwitnessed
            | Self::WitnessMismatch
            | Self::RequiresLoom
            | Self::RequiresSanitizer
            | Self::RequiresKaniOrCrux
            | Self::MiriUnsupported => "warning",
        }
    }

    /// Return the LSP `DiagnosticSeverity` integer for this class.
    ///
    /// The LSP protocol defines: 1=Error, 2=Warning, 3=Information, 4=Hint.
    /// unsafe-review is advisory and never emits Error (1); the mapping is:
    ///
    /// | SARIF level | LSP severity | Meaning                          |
    /// |-------------|--------------|----------------------------------|
    /// | `warning`   | 2 (Warning)  | actionable — evidence missing    |
    /// | `note`      | 3 (Information) | partial — some evidence present |
    /// | `none`      | 4 (Hint)     | non-actionable — closed/suppressed |
    ///
    /// Priority is a ranking/ordering signal only and must NOT affect this
    /// value.  Both this method and [`Self::sarif_level`] derive from the same
    /// class table to keep SARIF and LSP in agreement.
    pub fn lsp_severity(&self) -> usize {
        match self.sarif_level() {
            "warning" => 2,
            "note" => 3,
            // "none" and anything unexpected — informational hint, lowest severity.
            _ => 4,
        }
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
        // WitnessMismatch is actionable: a saved receipt whose tool does not
        // match any routed witness tool is a live "fix your receipt" condition,
        // not a resolved state. Consistent with sarif.rs (level=warning),
        // comment_plan/selection.rs (specific_receipt_missing bucket),
        // action_summary.rs (next_action asks for matching receipt), and
        // outcome/mod.rs (counted in violation list).
        let actionable = [
            ReviewClass::GuardedUnwitnessed,
            ReviewClass::ContractMissing,
            ReviewClass::GuardMissing,
            ReviewClass::ReachableUnwitnessed,
            ReviewClass::UnsafeUnreached,
            ReviewClass::WitnessMismatch,
            ReviewClass::RequiresLoom,
            ReviewClass::RequiresSanitizer,
            ReviewClass::RequiresKaniOrCrux,
            ReviewClass::MiriUnsupported,
            ReviewClass::StaticUnknown,
        ];
        let non_actionable = [
            ReviewClass::GuardedAndWitnessed,
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

    /// Drift-lock: pin the complete class→(sarif_level, lsp_severity) table.
    ///
    /// If a class changes bucket (e.g. from "warning" to "note"), this test
    /// breaks intentionally — the owner must decide the new severity mapping
    /// and update both this table and the doc comment on `sarif_level`.
    ///
    /// Advisory boundary: "warning" means the card needs attention, NOT that it
    /// blocks a merge.  unsafe-review never emits LSP severity 1 (Error).
    #[test]
    fn severity_table_is_pinned_for_every_class() {
        // (class, sarif_level, lsp_severity)
        // lsp: 2=Warning, 3=Information, 4=Hint
        let cases: &[(ReviewClass, &str, usize)] = &[
            // Non-actionable — closed / administratively resolved.
            (ReviewClass::GuardedAndWitnessed, "none", 4),
            (ReviewClass::BaselineKnown, "none", 4),
            (ReviewClass::Suppressed, "none", 4),
            // Partially-resolved — some evidence present but not a full chain.
            (ReviewClass::GuardedUnwitnessed, "note", 3),
            (ReviewClass::UnsafeUnreached, "note", 3),
            (ReviewClass::StaticUnknown, "note", 3),
            // Actionable — evidence missing or receipt mismatched.
            (ReviewClass::ContractMissing, "warning", 2),
            (ReviewClass::GuardMissing, "warning", 2),
            (ReviewClass::ReachableUnwitnessed, "warning", 2),
            (ReviewClass::WitnessMismatch, "warning", 2),
            (ReviewClass::RequiresLoom, "warning", 2),
            (ReviewClass::RequiresSanitizer, "warning", 2),
            (ReviewClass::RequiresKaniOrCrux, "warning", 2),
            (ReviewClass::MiriUnsupported, "warning", 2),
        ];

        for (class, expected_sarif, expected_lsp) in cases {
            assert_eq!(
                class.sarif_level(),
                *expected_sarif,
                "sarif_level mismatch for {}",
                class.as_str()
            );
            assert_eq!(
                class.lsp_severity(),
                *expected_lsp,
                "lsp_severity mismatch for {}",
                class.as_str()
            );
        }
    }

    /// Drift-lock: SARIF level and LSP severity must be consistent signals for
    /// every class.  Specifically:
    /// - "warning" → lsp 2 (Warning),
    /// - "note"    → lsp 3 (Information),
    /// - "none"    → lsp 4 (Hint).
    ///
    /// If these diverge, `severity_table_is_pinned_for_every_class` also fails,
    /// but this test gives a targeted error message naming the broken mapping.
    #[test]
    fn sarif_level_and_lsp_severity_are_consistent_for_every_class() {
        let all_classes = [
            ReviewClass::GuardedAndWitnessed,
            ReviewClass::GuardedUnwitnessed,
            ReviewClass::ContractMissing,
            ReviewClass::GuardMissing,
            ReviewClass::ReachableUnwitnessed,
            ReviewClass::UnsafeUnreached,
            ReviewClass::WitnessMismatch,
            ReviewClass::RequiresLoom,
            ReviewClass::RequiresSanitizer,
            ReviewClass::RequiresKaniOrCrux,
            ReviewClass::MiriUnsupported,
            ReviewClass::StaticUnknown,
            ReviewClass::BaselineKnown,
            ReviewClass::Suppressed,
        ];

        for class in &all_classes {
            let sarif = class.sarif_level();
            let lsp = class.lsp_severity();
            // Advisory: lsp 1 (Error) must never appear — no blocking verdict.
            assert!(
                lsp >= 2,
                "class {} produced lsp_severity {lsp} — unsafe-review must never emit Error (1)",
                class.as_str()
            );
            // Consistency: the lsp bucket must match the sarif bucket.
            let expected_lsp = match sarif {
                "warning" => 2,
                "note" => 3,
                _ => 4,
            };
            assert_eq!(
                lsp,
                expected_lsp,
                "class {} has sarif_level={sarif} but lsp_severity={lsp} (expected {expected_lsp})",
                class.as_str()
            );
        }
    }
}
