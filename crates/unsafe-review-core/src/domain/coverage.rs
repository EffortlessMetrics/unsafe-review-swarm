use super::{ReviewCard, ReviewClass, WitnessKind};

/// Coverage level for contract, guard, and test-reach slots.
///
/// `present` — evidence found and discharges the obligation.
/// `weak`    — evidence found but does not fully discharge (SPEC-0029 §coverage slots).
/// `missing` — no evidence found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coverage {
    Present,
    Weak,
    Missing,
}

impl Coverage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Present => "present",
            Self::Weak => "weak",
            Self::Missing => "missing",
        }
    }
}

/// Coverage for witness-receipt slot: present only via an imported receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WitnessReceiptCoverage {
    Present,
    Missing,
}

impl WitnessReceiptCoverage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Present => "present",
            Self::Missing => "missing",
        }
    }
}

/// Whether a manual-candidate overlay is attached to this ReviewCard.
///
/// Derived from ReviewCard alone this is always `Absent`; a higher-level layer
/// that resolves manual candidates against cards may upgrade it to `Present`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManualContext {
    Present,
    Absent,
}

impl ManualContext {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Present => "present",
            Self::Absent => "absent",
        }
    }
}

/// Baseline posture relative to a saved coverage floor (SPEC-0030 populates this).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BaselineState {
    New,
    Worsened,
    Inherited,
    Resolved,
    Unknown,
}

impl BaselineState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Worsened => "worsened",
            Self::Inherited => "inherited",
            Self::Resolved => "resolved",
            Self::Unknown => "unknown",
        }
    }
}

/// Coverage movement relative to a saved snapshot (SPEC-0030 populates this).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutcomeMovement {
    Improved,
    Regressed,
    Unchanged,
    Unknown,
}

impl OutcomeMovement {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Improved => "improved",
            Self::Regressed => "regressed",
            Self::Unchanged => "unchanged",
            Self::Unknown => "unknown",
        }
    }
}

/// Whether this card is selected for the comment plan (SPEC-0032 populates this).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommentPlanStatus {
    Selected,
    NotSelected,
    NotEligible,
}

impl CommentPlanStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Selected => "selected",
            Self::NotSelected => "not_selected",
            Self::NotEligible => "not_eligible",
        }
    }
}

/// Whether the card is ready for agent-assisted LSP repair (from agent readiness).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentLspReadiness {
    Ready,
    RequiresWitnessReceipt,
    NeedsHuman,
    Unsupported,
}

impl AgentLspReadiness {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::RequiresWitnessReceipt => "requires_witness_receipt",
            Self::NeedsHuman => "needs_human",
            Self::Unsupported => "unsupported",
        }
    }
}

/// Machine-readable per-card coverage block (SPEC-0029).
///
/// Each slot has a closed-vocabulary state. Consumers read this block rather
/// than re-deriving coverage from raw card fields.
///
/// Slots defaulted to `Unknown` / `NotEligible` / `Absent` at derivation time
/// are populated by later pipeline stages (SPEC-0030 baseline movement, SPEC-0032
/// comment plan, manual-candidate overlay resolution).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoverageBlock {
    /// Contract (safety doc) evidence level.
    pub contract_coverage: Coverage,
    /// Guard (discharge) evidence level.
    pub guard_coverage: Coverage,
    /// Test reachability evidence level.
    pub test_reach_coverage: Coverage,
    /// Witness receipt import state.
    pub witness_receipt_coverage: WitnessReceiptCoverage,
    /// Manual-candidate overlay attachment state.
    pub manual_context: ManualContext,
    /// Baseline posture (supplied by SPEC-0030; defaults to `Unknown`).
    pub baseline_state: BaselineState,
    /// Outcome movement vs. saved snapshot (supplied by SPEC-0030; defaults to `Unknown`).
    pub outcome_movement: OutcomeMovement,
    /// Comment-plan selection status (supplied by SPEC-0032; defaults to `NotEligible`).
    pub comment_plan_status: CommentPlanStatus,
    /// Agent-LSP readiness derived from card's agent-readiness state.
    pub agent_lsp_readiness: AgentLspReadiness,
}

impl CoverageBlock {
    /// Derive the coverage block from a ReviewCard using existing evidence fields.
    ///
    /// # Derivation rules (SPEC-0029)
    ///
    /// **contract_coverage**: `present` when `card.contract.present`; `missing` otherwise.
    /// No `weak` state for contract — a SAFETY comment is either present or not.
    ///
    /// **guard_coverage**: `present` when `card.discharge.present` AND the card class is not
    /// `contract_missing` or `guard_missing` (i.e. all obligations are guarded); `weak` when
    /// `card.discharge.present` but some obligations lack guard evidence (reflected by the
    /// card class being `guard_missing`); `missing` when `card.discharge.present` is false.
    ///
    /// **test_reach_coverage**: `present` when `card.reach.state == "present"`; `weak` when
    /// the state string indicates partial reach (not produced by the current analyzer but
    /// reserved for future use); `missing` otherwise.
    ///
    /// **witness_receipt_coverage**: `present` when `card.witness.present`; `missing` otherwise.
    ///
    /// **manual_context**: always `Absent` from a bare ReviewCard — a higher-level layer that
    /// resolves manual candidates against cards may upgrade to `Present`.
    ///
    /// **baseline_state**: derived from `card.class` per SPEC-0030.
    ///
    /// - `BaselineKnown` class → `Inherited` (card matched a baseline ledger entry and is still
    ///   open unchanged).
    /// - Actionable class → `New` (open actionable gap not covered by the baseline ledger).
    /// - All other classes (non-actionable, `Suppressed`) → `Unknown` (no baseline relation or
    ///   suppression is a separate ledger concept).
    ///
    /// `Worsened` and `Resolved` cannot be derived from the card alone: `worsened` requires a
    /// saved coverage snapshot and `resolved` applies to baseline entries with no current card.
    /// Both default to `Unknown` at card level; they are surfaced in the `Summary` movement
    /// counts and the policy report, not on individual cards.
    ///
    /// **outcome_movement**: derived from `baseline_state` per SPEC-0030.
    ///
    /// - `Inherited` baseline → `Unchanged` (gap persists but was not introduced by this change).
    /// - `New` baseline → `Regressed` (the change introduced this gap).
    /// - All other states → `Unknown`.
    ///
    /// **comment_plan_status**: `NotEligible` — populated by SPEC-0032 comment plan.
    ///
    /// **agent_lsp_readiness**: derived from `card.class` and `card.routes` using the same
    /// logic as the agent readiness module.
    pub fn derive(card: &ReviewCard) -> Self {
        let baseline_state = derive_baseline_state(card);
        let outcome_movement = derive_outcome_movement(baseline_state);
        Self {
            contract_coverage: derive_contract_coverage(card),
            guard_coverage: derive_guard_coverage(card),
            test_reach_coverage: derive_test_reach_coverage(card),
            witness_receipt_coverage: derive_witness_receipt_coverage(card),
            manual_context: ManualContext::Absent,
            baseline_state,
            outcome_movement,
            comment_plan_status: CommentPlanStatus::NotEligible,
            agent_lsp_readiness: derive_agent_lsp_readiness(card),
        }
    }
}

fn derive_contract_coverage(card: &ReviewCard) -> Coverage {
    if card.contract.present {
        Coverage::Present
    } else {
        Coverage::Missing
    }
}

fn derive_guard_coverage(card: &ReviewCard) -> Coverage {
    // `card.discharge.present` means at least one guard was found, but the card
    // class tells us whether all obligations are satisfied.
    //
    // - `present`  when discharge is present AND the class is not guard_missing
    //   (i.e., all guarded obligations are satisfied).
    // - `weak`     when discharge is present but class is guard_missing (some
    //   obligations still lack guard evidence — the analyzer found partial evidence).
    // - `missing`  when discharge is not present at all.
    if card.discharge.present {
        match card.class {
            ReviewClass::GuardMissing => Coverage::Weak,
            _ => Coverage::Present,
        }
    } else {
        Coverage::Missing
    }
}

fn derive_test_reach_coverage(card: &ReviewCard) -> Coverage {
    // The analyzer sets `reach.state` to "present" when at least one related test
    // mentioning the owning function was found, and "missing" otherwise.  A "weak"
    // state is reserved for future partial-reach signals (e.g., reach via a helper
    // but not the exact function); currently the analyzer does not emit it.
    match card.reach.state.as_str() {
        "present" => Coverage::Present,
        "weak" => Coverage::Weak,
        _ => Coverage::Missing,
    }
}

fn derive_witness_receipt_coverage(card: &ReviewCard) -> WitnessReceiptCoverage {
    if card.witness.present {
        WitnessReceiptCoverage::Present
    } else {
        WitnessReceiptCoverage::Missing
    }
}

/// Derive `BaselineState` from a card's class (SPEC-0030).
///
/// `BaselineKnown` → `Inherited`; actionable class → `New`; all others → `Unknown`.
/// `Worsened` and `Resolved` are not derivable from a single card.
fn derive_baseline_state(card: &ReviewCard) -> BaselineState {
    use super::ReviewClass;
    match card.class {
        ReviewClass::BaselineKnown => BaselineState::Inherited,
        ref class if class.is_actionable() => BaselineState::New,
        _ => BaselineState::Unknown,
    }
}

/// Derive `OutcomeMovement` from a card's baseline state (SPEC-0030).
///
/// `Inherited` → `Unchanged`; `New` → `Regressed`; all others → `Unknown`.
fn derive_outcome_movement(baseline_state: BaselineState) -> OutcomeMovement {
    match baseline_state {
        BaselineState::Inherited => OutcomeMovement::Unchanged,
        BaselineState::New => OutcomeMovement::Regressed,
        _ => OutcomeMovement::Unknown,
    }
}

fn derive_agent_lsp_readiness(card: &ReviewCard) -> AgentLspReadiness {
    // Mirror the agent-readiness state logic from output/agent/readiness.rs
    // without importing that module (this is a domain-level derivation).
    //
    // Unsupported:            class not actionable, or witness route is Unsupported.
    // NeedsHuman:             operation family requires human review (Ffi, InlineAsm,
    //                         TargetFeature, Unknown) or a HumanDeepReview route exists
    //                         or class is StaticUnknown/MiriUnsupported.
    // RequiresWitnessReceipt: class is RequiresLoom/RequiresSanitizer/RequiresKaniOrCrux
    //                         — an external witness receipt is needed before repair
    //                         delegation is appropriate. Checked after NeedsHuman so
    //                         that a human-gating factor on those classes still wins.
    // Ready:                  actionable, no human-gating or receipt-blocking factors,
    //                         specific op family, at least one verify command.
    use super::ReviewClass;
    use super::operation::OperationFamily;

    if !card.class.is_actionable() {
        return AgentLspReadiness::Unsupported;
    }

    // An Unsupported witness route hard-gates.
    if card
        .routes
        .iter()
        .any(|route| matches!(route.kind, WitnessKind::Unsupported))
    {
        return AgentLspReadiness::Unsupported;
    }

    // Human-review-requiring classes/families/routes take priority over the
    // receipt-blocking check below.
    let requires_human = matches!(
        card.class,
        ReviewClass::StaticUnknown | ReviewClass::MiriUnsupported
    ) || matches!(
        card.operation.family,
        OperationFamily::Unknown
            | OperationFamily::Ffi
            | OperationFamily::InlineAsm
            | OperationFamily::TargetFeature
    ) || card
        .routes
        .iter()
        .any(|route| matches!(route.kind, WitnessKind::HumanDeepReview));

    if requires_human {
        return AgentLspReadiness::NeedsHuman;
    }

    // Receipt-blocking classes: an external concurrency/sanitizer/formal-methods
    // witness receipt is required before an agent can discharge the obligation.
    // This matches the logic in output/agent/readiness.rs that sets
    // `requires_witness_receipt` for the same class set.
    if matches!(
        card.class,
        ReviewClass::RequiresLoom
            | ReviewClass::RequiresSanitizer
            | ReviewClass::RequiresKaniOrCrux
    ) {
        return AgentLspReadiness::RequiresWitnessReceipt;
    }

    AgentLspReadiness::Ready
}

#[cfg(test)]
mod tests {
    use super::{
        AgentLspReadiness, BaselineState, CommentPlanStatus, Coverage, CoverageBlock,
        ManualContext, OutcomeMovement, WitnessReceiptCoverage,
    };
    use crate::domain::{
        CardId, Confidence, ContractEvidence, DischargeEvidence, HazardKind, NextAction,
        OperationFamily, Priority, ProofPath, ReachEvidence, ReviewCard, ReviewClass,
        SourceLocation, UnsafeOperation, UnsafeSite, UnsafeSiteKind, WitnessEvidence, WitnessKind,
        WitnessRoute,
    };

    fn minimal_card(class: ReviewClass) -> ReviewCard {
        ReviewCard {
            id: CardId("UR-test-c1".to_string()),
            class,
            priority: Priority::Medium,
            confidence: Confidence::Medium,
            proof_path: ProofPath::SourceRouteOnly,
            site: UnsafeSite {
                location: SourceLocation {
                    file: "src/lib.rs".into(),
                    line: 1,
                    column: 1,
                },
                kind: UnsafeSiteKind::Operation,
                owner: Some("owner".to_string()),
                visibility: "private".to_string(),
                public_api_surface: false,
                changed: true,
                snippet: "unsafe { *ptr }".to_string(),
            },
            operation: UnsafeOperation {
                expression: "unsafe { *ptr }".to_string(),
                family: OperationFamily::RawPointerDeref,
            },
            hazards: vec![HazardKind::PointerValidity],
            obligations: vec![],
            obligation_evidence: vec![],
            contract: ContractEvidence::missing(),
            discharge: DischargeEvidence::missing(),
            reach: ReachEvidence {
                state: "missing".to_string(),
                summary: "no tests".to_string(),
            },
            witness: WitnessEvidence::missing(),
            missing: vec![],
            routes: vec![WitnessRoute {
                kind: WitnessKind::Miri,
                reason: "test".to_string(),
                command: Some("cargo miri test".to_string()),
                required: false,
            }],
            next_action: NextAction {
                summary: "add guard".to_string(),
                verify_commands: vec!["cargo miri test".to_string()],
            },
            related_tests: vec![],
        }
    }

    #[test]
    fn contract_coverage_present_when_contract_evidence_present() {
        let mut card = minimal_card(ReviewClass::GuardMissing);
        card.contract = ContractEvidence::present("safety comment");
        let block = CoverageBlock::derive(&card);
        assert_eq!(block.contract_coverage, Coverage::Present);
        assert_eq!(block.contract_coverage.as_str(), "present");
    }

    #[test]
    fn contract_coverage_missing_when_no_contract_evidence() {
        let card = minimal_card(ReviewClass::ContractMissing);
        let block = CoverageBlock::derive(&card);
        assert_eq!(block.contract_coverage, Coverage::Missing);
        assert_eq!(block.contract_coverage.as_str(), "missing");
    }

    #[test]
    fn guard_coverage_present_when_discharge_present_and_class_not_guard_missing() {
        let mut card = minimal_card(ReviewClass::GuardedUnwitnessed);
        card.discharge = DischargeEvidence::present("bounds check");
        let block = CoverageBlock::derive(&card);
        assert_eq!(block.guard_coverage, Coverage::Present);
        assert_eq!(block.guard_coverage.as_str(), "present");
    }

    #[test]
    fn guard_coverage_weak_when_discharge_present_but_class_is_guard_missing() {
        let mut card = minimal_card(ReviewClass::GuardMissing);
        card.discharge = DischargeEvidence::present("partial guard");
        let block = CoverageBlock::derive(&card);
        assert_eq!(block.guard_coverage, Coverage::Weak);
        assert_eq!(block.guard_coverage.as_str(), "weak");
    }

    #[test]
    fn guard_coverage_missing_when_discharge_absent() {
        let card = minimal_card(ReviewClass::GuardMissing);
        let block = CoverageBlock::derive(&card);
        assert_eq!(block.guard_coverage, Coverage::Missing);
        assert_eq!(block.guard_coverage.as_str(), "missing");
    }

    #[test]
    fn test_reach_coverage_present_when_reach_state_present() {
        let mut card = minimal_card(ReviewClass::GuardMissing);
        card.reach = ReachEvidence {
            state: "present".to_string(),
            summary: "1 related test".to_string(),
        };
        let block = CoverageBlock::derive(&card);
        assert_eq!(block.test_reach_coverage, Coverage::Present);
        assert_eq!(block.test_reach_coverage.as_str(), "present");
    }

    #[test]
    fn test_reach_coverage_weak_when_reach_state_weak() {
        let mut card = minimal_card(ReviewClass::GuardMissing);
        card.reach = ReachEvidence {
            state: "weak".to_string(),
            summary: "indirect test reach".to_string(),
        };
        let block = CoverageBlock::derive(&card);
        assert_eq!(block.test_reach_coverage, Coverage::Weak);
    }

    #[test]
    fn test_reach_coverage_missing_when_reach_state_missing() {
        let card = minimal_card(ReviewClass::GuardMissing);
        let block = CoverageBlock::derive(&card);
        assert_eq!(block.test_reach_coverage, Coverage::Missing);
        assert_eq!(block.test_reach_coverage.as_str(), "missing");
    }

    #[test]
    fn witness_receipt_coverage_present_when_witness_present() {
        let mut card = minimal_card(ReviewClass::GuardedAndWitnessed);
        card.witness = WitnessEvidence::present("miri receipt imported");
        let block = CoverageBlock::derive(&card);
        assert_eq!(
            block.witness_receipt_coverage,
            WitnessReceiptCoverage::Present
        );
        assert_eq!(block.witness_receipt_coverage.as_str(), "present");
    }

    #[test]
    fn witness_receipt_coverage_missing_when_no_witness() {
        let card = minimal_card(ReviewClass::GuardMissing);
        let block = CoverageBlock::derive(&card);
        assert_eq!(
            block.witness_receipt_coverage,
            WitnessReceiptCoverage::Missing
        );
        assert_eq!(block.witness_receipt_coverage.as_str(), "missing");
    }

    #[test]
    fn manual_context_always_absent_from_bare_card() {
        let card = minimal_card(ReviewClass::GuardMissing);
        let block = CoverageBlock::derive(&card);
        assert_eq!(block.manual_context, ManualContext::Absent);
        assert_eq!(block.manual_context.as_str(), "absent");
    }

    #[test]
    fn baseline_state_new_for_actionable_class() {
        // An actionable card not in the baseline ledger is `new` (SPEC-0030).
        let card = minimal_card(ReviewClass::GuardMissing);
        let block = CoverageBlock::derive(&card);
        assert_eq!(block.baseline_state, BaselineState::New);
        assert_eq!(block.baseline_state.as_str(), "new");
    }

    #[test]
    fn baseline_state_inherited_for_baseline_known_class() {
        // A card with class `BaselineKnown` is `inherited` — it matched the baseline ledger.
        let card = minimal_card(ReviewClass::BaselineKnown);
        let block = CoverageBlock::derive(&card);
        assert_eq!(block.baseline_state, BaselineState::Inherited);
        assert_eq!(block.baseline_state.as_str(), "inherited");
    }

    #[test]
    fn baseline_state_unknown_for_non_actionable_non_baseline_class() {
        // Non-actionable cards that are not baseline-known have no movement posture.
        let card = minimal_card(ReviewClass::GuardedAndWitnessed);
        let block = CoverageBlock::derive(&card);
        assert_eq!(block.baseline_state, BaselineState::Unknown);
        assert_eq!(block.baseline_state.as_str(), "unknown");
    }

    #[test]
    fn baseline_state_unknown_for_suppressed_class() {
        // Suppressed cards have their own ledger; they do not carry baseline posture.
        let card = minimal_card(ReviewClass::Suppressed);
        let block = CoverageBlock::derive(&card);
        assert_eq!(block.baseline_state, BaselineState::Unknown);
        assert_eq!(block.baseline_state.as_str(), "unknown");
    }

    #[test]
    fn outcome_movement_regressed_for_new_actionable_card() {
        // An open actionable card not in baseline represents a regression.
        let card = minimal_card(ReviewClass::GuardMissing);
        let block = CoverageBlock::derive(&card);
        assert_eq!(block.outcome_movement, OutcomeMovement::Regressed);
        assert_eq!(block.outcome_movement.as_str(), "regressed");
    }

    #[test]
    fn outcome_movement_unchanged_for_baseline_known_card() {
        // A baseline-known card persists but was not introduced by this change.
        let card = minimal_card(ReviewClass::BaselineKnown);
        let block = CoverageBlock::derive(&card);
        assert_eq!(block.outcome_movement, OutcomeMovement::Unchanged);
        assert_eq!(block.outcome_movement.as_str(), "unchanged");
    }

    #[test]
    fn outcome_movement_unknown_for_non_actionable_non_baseline_class() {
        let card = minimal_card(ReviewClass::GuardedAndWitnessed);
        let block = CoverageBlock::derive(&card);
        assert_eq!(block.outcome_movement, OutcomeMovement::Unknown);
        assert_eq!(block.outcome_movement.as_str(), "unknown");
    }

    #[test]
    fn comment_plan_status_defaults_to_not_eligible() {
        let card = minimal_card(ReviewClass::GuardMissing);
        let block = CoverageBlock::derive(&card);
        assert_eq!(block.comment_plan_status, CommentPlanStatus::NotEligible);
        assert_eq!(block.comment_plan_status.as_str(), "not_eligible");
    }

    #[test]
    fn agent_lsp_readiness_unsupported_for_non_actionable_class() {
        let card = minimal_card(ReviewClass::GuardedAndWitnessed);
        let block = CoverageBlock::derive(&card);
        assert_eq!(block.agent_lsp_readiness, AgentLspReadiness::Unsupported);
        assert_eq!(block.agent_lsp_readiness.as_str(), "unsupported");
    }

    #[test]
    fn agent_lsp_readiness_unsupported_for_unsupported_witness_route() {
        let mut card = minimal_card(ReviewClass::GuardMissing);
        card.routes = vec![WitnessRoute {
            kind: WitnessKind::Unsupported,
            reason: "not supported".to_string(),
            command: None,
            required: false,
        }];
        let block = CoverageBlock::derive(&card);
        assert_eq!(block.agent_lsp_readiness, AgentLspReadiness::Unsupported);
    }

    #[test]
    fn agent_lsp_readiness_needs_human_for_human_deep_review_route() {
        let mut card = minimal_card(ReviewClass::GuardMissing);
        card.routes = vec![WitnessRoute {
            kind: WitnessKind::HumanDeepReview,
            reason: "manual review needed".to_string(),
            command: None,
            required: false,
        }];
        let block = CoverageBlock::derive(&card);
        assert_eq!(block.agent_lsp_readiness, AgentLspReadiness::NeedsHuman);
        assert_eq!(block.agent_lsp_readiness.as_str(), "needs_human");
    }

    #[test]
    fn agent_lsp_readiness_needs_human_for_static_unknown_class() {
        let card = minimal_card(ReviewClass::StaticUnknown);
        let block = CoverageBlock::derive(&card);
        assert_eq!(block.agent_lsp_readiness, AgentLspReadiness::NeedsHuman);
    }

    #[test]
    fn agent_lsp_readiness_needs_human_for_miri_unsupported_class() {
        let card = minimal_card(ReviewClass::MiriUnsupported);
        let block = CoverageBlock::derive(&card);
        assert_eq!(block.agent_lsp_readiness, AgentLspReadiness::NeedsHuman);
    }

    #[test]
    fn agent_lsp_readiness_needs_human_for_ffi_family() {
        let mut card = minimal_card(ReviewClass::GuardMissing);
        card.operation.family = OperationFamily::Ffi;
        let block = CoverageBlock::derive(&card);
        assert_eq!(block.agent_lsp_readiness, AgentLspReadiness::NeedsHuman);
    }

    #[test]
    fn agent_lsp_readiness_needs_human_for_inline_asm_family() {
        let mut card = minimal_card(ReviewClass::GuardMissing);
        card.operation.family = OperationFamily::InlineAsm;
        let block = CoverageBlock::derive(&card);
        assert_eq!(block.agent_lsp_readiness, AgentLspReadiness::NeedsHuman);
    }

    #[test]
    fn agent_lsp_readiness_needs_human_for_target_feature_family() {
        let mut card = minimal_card(ReviewClass::GuardMissing);
        card.operation.family = OperationFamily::TargetFeature;
        let block = CoverageBlock::derive(&card);
        assert_eq!(block.agent_lsp_readiness, AgentLspReadiness::NeedsHuman);
    }

    #[test]
    fn agent_lsp_readiness_ready_for_actionable_specific_family() {
        let card = minimal_card(ReviewClass::GuardMissing);
        // default card has Miri route and RawPointerDeref family — should be Ready
        let block = CoverageBlock::derive(&card);
        assert_eq!(block.agent_lsp_readiness, AgentLspReadiness::Ready);
        assert_eq!(block.agent_lsp_readiness.as_str(), "ready");
    }

    /// Drift-lock: RequiresLoom must map to RequiresWitnessReceipt (issue #1632).
    ///
    /// A card that requires an external loom/shuttle witness receipt before repair
    /// delegation is NOT immediately agent-ready. Reporting it as "ready" would
    /// over-count the telemetry `agent_readiness.ready` bucket and disagree with
    /// the comment-plan's per-card `agent_readiness.state = "requires_witness_receipt"`.
    #[test]
    fn agent_lsp_readiness_requires_witness_receipt_for_requires_loom_class() {
        let card = minimal_card(ReviewClass::RequiresLoom);
        let block = CoverageBlock::derive(&card);
        assert_eq!(
            block.agent_lsp_readiness,
            AgentLspReadiness::RequiresWitnessReceipt,
            "RequiresLoom must produce RequiresWitnessReceipt, not Ready — revert would re-introduce #1632"
        );
        assert_eq!(
            block.agent_lsp_readiness.as_str(),
            "requires_witness_receipt"
        );
    }

    /// Drift-lock: RequiresSanitizer must map to RequiresWitnessReceipt (issue #1632).
    #[test]
    fn agent_lsp_readiness_requires_witness_receipt_for_requires_sanitizer_class() {
        let card = minimal_card(ReviewClass::RequiresSanitizer);
        let block = CoverageBlock::derive(&card);
        assert_eq!(
            block.agent_lsp_readiness,
            AgentLspReadiness::RequiresWitnessReceipt,
            "RequiresSanitizer must produce RequiresWitnessReceipt, not Ready — revert would re-introduce #1632"
        );
    }

    /// Drift-lock: RequiresKaniOrCrux must map to RequiresWitnessReceipt (issue #1632).
    #[test]
    fn agent_lsp_readiness_requires_witness_receipt_for_requires_kani_or_crux_class() {
        let card = minimal_card(ReviewClass::RequiresKaniOrCrux);
        let block = CoverageBlock::derive(&card);
        assert_eq!(
            block.agent_lsp_readiness,
            AgentLspReadiness::RequiresWitnessReceipt,
            "RequiresKaniOrCrux must produce RequiresWitnessReceipt, not Ready — revert would re-introduce #1632"
        );
    }

    /// Drift-lock: WitnessMismatch must produce baseline_state=New (issue #1602).
    ///
    /// WitnessMismatch is actionable (a broken receipt is a live, surfaced
    /// condition), so it must feed the no-new-debt/exit-code policy gate like any
    /// other open actionable class. This test would FAIL if WitnessMismatch were
    /// reverted to non-actionable in `is_actionable()`.
    #[test]
    fn witness_mismatch_baseline_state_is_new() {
        let card = minimal_card(ReviewClass::WitnessMismatch);
        let block = CoverageBlock::derive(&card);
        assert_eq!(
            block.baseline_state,
            BaselineState::New,
            "WitnessMismatch must produce baseline_state=New (actionable) — revert is_actionable() to break this"
        );
    }

    /// Drift-lock: WitnessMismatch must produce outcome_movement=Regressed (issue #1602).
    ///
    /// Would FAIL if WitnessMismatch were reverted to non-actionable.
    #[test]
    fn witness_mismatch_outcome_movement_is_regressed() {
        let card = minimal_card(ReviewClass::WitnessMismatch);
        let block = CoverageBlock::derive(&card);
        assert_eq!(
            block.outcome_movement,
            OutcomeMovement::Regressed,
            "WitnessMismatch must produce outcome_movement=Regressed — revert is_actionable() to break this"
        );
    }

    #[test]
    fn coverage_block_as_str_methods_cover_all_variants() {
        assert_eq!(Coverage::Present.as_str(), "present");
        assert_eq!(Coverage::Weak.as_str(), "weak");
        assert_eq!(Coverage::Missing.as_str(), "missing");

        assert_eq!(WitnessReceiptCoverage::Present.as_str(), "present");
        assert_eq!(WitnessReceiptCoverage::Missing.as_str(), "missing");

        assert_eq!(ManualContext::Present.as_str(), "present");
        assert_eq!(ManualContext::Absent.as_str(), "absent");

        assert_eq!(BaselineState::New.as_str(), "new");
        assert_eq!(BaselineState::Worsened.as_str(), "worsened");
        assert_eq!(BaselineState::Inherited.as_str(), "inherited");
        assert_eq!(BaselineState::Resolved.as_str(), "resolved");
        assert_eq!(BaselineState::Unknown.as_str(), "unknown");

        assert_eq!(OutcomeMovement::Improved.as_str(), "improved");
        assert_eq!(OutcomeMovement::Regressed.as_str(), "regressed");
        assert_eq!(OutcomeMovement::Unchanged.as_str(), "unchanged");
        assert_eq!(OutcomeMovement::Unknown.as_str(), "unknown");

        assert_eq!(CommentPlanStatus::Selected.as_str(), "selected");
        assert_eq!(CommentPlanStatus::NotSelected.as_str(), "not_selected");
        assert_eq!(CommentPlanStatus::NotEligible.as_str(), "not_eligible");

        assert_eq!(AgentLspReadiness::Ready.as_str(), "ready");
        assert_eq!(
            AgentLspReadiness::RequiresWitnessReceipt.as_str(),
            "requires_witness_receipt"
        );
        assert_eq!(AgentLspReadiness::NeedsHuman.as_str(), "needs_human");
        assert_eq!(AgentLspReadiness::Unsupported.as_str(), "unsupported");
    }
}
