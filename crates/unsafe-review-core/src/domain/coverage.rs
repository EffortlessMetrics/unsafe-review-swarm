use super::operation::OperationFamily;
use super::{Confidence, ReviewCard, ReviewClass, WitnessKind};

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
    /// `Resolved` defaults to `Unknown` at card level (it has no card).  `Worsened` and `Improved`
    /// are promoted after derivation via [`CoverageBlock::apply_snapshot_movement`], which output
    /// renderers call using the snapshot stored in `AnalyzeOutput.coverage_snapshot`.
    ///
    /// **outcome_movement**: derived from `baseline_state` per SPEC-0030.
    ///
    /// - `Inherited` baseline → `Unchanged` initially; upgraded to `Improved` or `Worsened` (with
    ///   `baseline_state → Worsened`) by `apply_snapshot_movement` if a slot comparison warrants it.
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

    /// Upgrade `baseline_state` and `outcome_movement` from snapshot-level slot comparison
    /// (SPEC-0030 §single-truth).
    ///
    /// Called by output renderers that have access to the coverage snapshot
    /// (`AnalyzeOutput.coverage_snapshot`).  The snapshot slot values are the four
    /// string-encoded coverage fields stored in `SnapshotCoverage`:
    /// `contract`, `guard`, `test_reach`, `witness_receipt` — each `"present"`, `"weak"`, or
    /// `"missing"`.
    ///
    /// This function replicates the slot-ordinal comparison from
    /// `SnapshotCoverage::is_worsened_by` / `is_improved_by` (`policy/mod.rs`) using the
    /// same ordinal: `present=2 > weak=1 > missing=0`.  Keeping the logic inline avoids a
    /// `domain → policy` circular dependency while guaranteeing a single derivation rule.
    ///
    /// Precedence: worsened > improved (mirrors `summarize` in `pipeline/summary.rs`).
    ///
    /// Only has effect when `baseline_state == Inherited` (the card is `BaselineKnown`).
    /// Cards that are `New`, `Unknown`, etc. are not affected.
    pub fn apply_snapshot_slots(
        &mut self,
        snap_contract: &str,
        snap_guard: &str,
        snap_test_reach: &str,
        snap_witness_receipt: &str,
    ) {
        if self.baseline_state != BaselineState::Inherited {
            return;
        }
        let ordinal = |s: &str| -> u8 {
            match s {
                "present" => 2,
                "weak" => 1,
                _ => 0,
            }
        };
        let cur_contract = ordinal(self.contract_coverage.as_str());
        let cur_guard = ordinal(self.guard_coverage.as_str());
        let cur_test_reach = ordinal(self.test_reach_coverage.as_str());
        let cur_witness = ordinal(self.witness_receipt_coverage.as_str());

        let snap_contract_ord = ordinal(snap_contract);
        let snap_guard_ord = ordinal(snap_guard);
        let snap_test_reach_ord = ordinal(snap_test_reach);
        let snap_witness_ord = ordinal(snap_witness_receipt);

        let is_worsened = cur_contract < snap_contract_ord
            || cur_guard < snap_guard_ord
            || cur_test_reach < snap_test_reach_ord
            || cur_witness < snap_witness_ord;

        let any_higher = cur_contract > snap_contract_ord
            || cur_guard > snap_guard_ord
            || cur_test_reach > snap_test_reach_ord
            || cur_witness > snap_witness_ord;
        let is_improved = any_higher && !is_worsened;

        if is_worsened {
            self.baseline_state = BaselineState::Worsened;
            self.outcome_movement = OutcomeMovement::Regressed;
        } else if is_improved {
            // baseline_state stays Inherited (card is still open, still present).
            self.outcome_movement = OutcomeMovement::Improved;
        }
        // else: unchanged — no update needed
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

/// Output of the single shared readiness computation, used by both the agent
/// packet (`output/agent/readiness.rs`) and the coverage block
/// (`derive_agent_lsp_readiness`).  Keeping both callers on this one function
/// is the enforcement point: the two surfaces CANNOT diverge because they
/// share exactly one code path.
pub(crate) struct AgentReadinessResult {
    /// Closed-vocabulary readiness state.
    pub(crate) state: AgentLspReadiness,
    /// Human-readable reasons explaining the state (used by the agent packet's
    /// `agent_readiness.reasons` field; ignored by the coverage block).
    pub(crate) reasons: Vec<String>,
}

/// Compute agent readiness for `card`, encoding ALL gates used by both
/// the agent-packet surface (`output/agent/readiness.rs`) and the
/// coverage block (`coverage.agent_lsp_readiness`).
///
/// `has_card_scoped_repairs` must be supplied by the caller:
/// - The agent packet computes it via `output/agent/repairs::build(card)`.
/// - The coverage block computes it via [`domain_has_card_scoped_repairs`].
///
/// Both callers pass their respective value into this function so that the
/// derived state is identical, resolving the single-truth drift described in
/// output audit #1687 (findings 3+4).
///
/// Gate order matches `output/agent/readiness.rs` exactly:
/// 1. Class not actionable → `Unsupported`
/// 2. No missing evidence → `Unsupported`
/// 3. All missing is witness → `RequiresWitnessReceipt`
/// 4. RequiresSanitizer / RequiresKaniOrCrux / RequiresLoom class → sets witness-receipt flag
/// 5. StaticUnknown / MiriUnsupported class → sets human-review flag
/// 6. Unknown / Ffi / InlineAsm / TargetFeature family → sets human-review flag
/// 7. HumanDeepReview route → sets human-review flag
/// 8. Unsupported route → `Unsupported`
/// 9. Low/Unknown confidence → accumulates unsupported reason
/// 10. No card-scoped repair → accumulates unsupported reason
/// 11. No verify command → accumulates unsupported reason
/// 12. Resolve: empty reasons → `Ready`; human flag → `NeedsHuman`;
///     receipt flag → `RequiresWitnessReceipt`; else → `Unsupported`
pub(crate) fn compute_agent_lsp_readiness(
    card: &ReviewCard,
    has_card_scoped_repairs: bool,
) -> AgentReadinessResult {
    let mut reasons: Vec<String> = Vec::new();

    // Gate 1: class must be actionable.
    if !card.class.is_actionable() {
        reasons.push(format!(
            "card class `{}` is not an open actionable repair target",
            card.class.as_str()
        ));
        return AgentReadinessResult {
            state: AgentLspReadiness::Unsupported,
            reasons,
        };
    }

    // Gate 2: card must have at least one missing evidence entry.
    if card.missing.is_empty() {
        reasons.push("card has no missing evidence to repair".to_string());
        return AgentReadinessResult {
            state: AgentLspReadiness::Unsupported,
            reasons,
        };
    }

    // Gate 3: if every missing entry is a witness receipt, the remaining work
    // is an external receipt — not an automatic source repair.
    if card.missing.iter().all(|m| m.kind == "witness") {
        reasons.push(
            "remaining work is an external witness receipt, not an automatic source repair"
                .to_string(),
        );
        return AgentReadinessResult {
            state: AgentLspReadiness::RequiresWitnessReceipt,
            reasons,
        };
    }

    // Flags accumulated by gates 4-7; resolved at gate 12.
    let mut requires_human_review = false;
    let mut requires_witness_receipt = false;

    // Gate 4: receipt-blocking classes.
    if matches!(
        card.class,
        ReviewClass::RequiresSanitizer
            | ReviewClass::RequiresKaniOrCrux
            | ReviewClass::RequiresLoom
    ) {
        reasons.push(format!(
            "card class `{}` requires an external witness receipt before repair delegation",
            card.class.as_str()
        ));
        requires_witness_receipt = true;
    }

    // Gate 5: human-review-requiring classes.
    if matches!(
        card.class,
        ReviewClass::StaticUnknown | ReviewClass::MiriUnsupported
    ) {
        reasons.push(format!(
            "card class `{}` requires human review before repair delegation",
            card.class.as_str()
        ));
        requires_human_review = true;
    }

    // Gate 6: human-review-requiring operation families.
    if matches!(
        card.operation.family,
        OperationFamily::UnsafeDeclaration
            | OperationFamily::Unknown
            | OperationFamily::Ffi
            | OperationFamily::InlineAsm
            | OperationFamily::TargetFeature
    ) {
        reasons.push(format!(
            "operation family `{}` is not safe for automatic repair delegation",
            card.operation.family.as_str()
        ));
        requires_human_review = true;
    }

    // Gate 7: HumanDeepReview witness route.
    if card
        .routes
        .iter()
        .any(|route| matches!(route.kind, WitnessKind::HumanDeepReview))
    {
        reasons.push("witness route requires human deep review".to_string());
        requires_human_review = true;
    }

    // Gate 8: Unsupported witness route hard-gates immediately.
    if card
        .routes
        .iter()
        .any(|route| matches!(route.kind, WitnessKind::Unsupported))
    {
        reasons.push("witness route is unsupported for bounded agent repair".to_string());
        return AgentReadinessResult {
            state: AgentLspReadiness::Unsupported,
            reasons,
        };
    }

    // Gate 9: confidence must be Medium or High.
    if !matches!(card.confidence, Confidence::High | Confidence::Medium) {
        reasons.push(format!(
            "card confidence `{}` is too weak for bounded repair delegation",
            card.confidence.as_str()
        ));
    }

    // Gate 10: at least one card-scoped repair must be available.
    if !has_card_scoped_repairs {
        reasons.push("no card-scoped allowed repair is available".to_string());
    }

    // Gate 11: at least one verify command must be available.
    if card.next_action.verify_commands.is_empty() {
        reasons.push("no verify command is available for this card".to_string());
    }

    // Gate 12: resolve to a readiness state.
    if reasons.is_empty() {
        AgentReadinessResult {
            state: AgentLspReadiness::Ready,
            reasons: vec![
                "specific operation family".to_string(),
                "card-scoped allowed repairs".to_string(),
                "verify commands available".to_string(),
                "medium-or-high confidence".to_string(),
            ],
        }
    } else if requires_human_review {
        AgentReadinessResult {
            state: AgentLspReadiness::NeedsHuman,
            reasons,
        }
    } else if requires_witness_receipt {
        AgentReadinessResult {
            state: AgentLspReadiness::RequiresWitnessReceipt,
            reasons,
        }
    } else {
        AgentReadinessResult {
            state: AgentLspReadiness::Unsupported,
            reasons,
        }
    }
}

/// Approximate whether `card` has card-scoped allowed repairs, using only
/// domain-level card fields (no output-layer imports).
///
/// Mirrors the computation in `output/agent/repairs::build` that sets
/// `has_card_scoped_repairs`.  Two categories of repairs are considered:
///
/// 1. **Card-missing repairs** (`output/agent/repairs/card_missing.rs`): added
///    when `card.missing` contains any entry with kind `"contract"`, `"reach"`,
///    `"test"`, or `"witness"`.
///
/// 2. **Operation-level repairs** (`output/agent/repairs/operation.rs`): added
///    when the operation family has specific repair actions AND at least one
///    `obligation_evidence` entry has `discharge.present = false`.
///
/// This function is intentionally conservative: it prefers `true` over `false`
/// to avoid falsely marking cards as unsupported in domain-only contexts where
/// the full repair-build pass is not available.  The agent-packet path supplies
/// the exact value from `repairs::build`, so domain-derived `agent_lsp_readiness`
/// stays as correct as the available information allows.
fn domain_has_card_scoped_repairs(card: &ReviewCard) -> bool {
    // Category 1: card_missing repairs (kinds that card_missing.rs handles).
    let card_missing_repairs = card
        .missing
        .iter()
        .any(|m| matches!(m.kind.as_str(), "contract" | "reach" | "test" | "witness"));

    // Category 2: operation-level repairs (family has a specific path AND at
    // least one obligation is undischarged).
    let operation_repairs = !matches!(
        card.operation.family,
        OperationFamily::UnsafeDeclaration
            | OperationFamily::Unknown
            | OperationFamily::Ffi
            | OperationFamily::InlineAsm
            | OperationFamily::TargetFeature
    ) && card
        .obligation_evidence
        .iter()
        .any(|e| !e.discharge.present);

    card_missing_repairs || operation_repairs
}

fn derive_agent_lsp_readiness(card: &ReviewCard) -> AgentLspReadiness {
    // Delegate to the single shared function, computing has_card_scoped_repairs
    // from domain-level card fields.  The agent-packet path uses the exact value
    // from `output/agent/repairs::build`; coverage-only callers use this
    // domain approximation.
    compute_agent_lsp_readiness(card, domain_has_card_scoped_repairs(card)).state
}

#[cfg(test)]
mod tests {
    use super::{
        AgentLspReadiness, BaselineState, CommentPlanStatus, Coverage, CoverageBlock,
        ManualContext, OutcomeMovement, WitnessReceiptCoverage,
    };
    use crate::domain::{
        CardId, Confidence, ContractEvidence, DischargeEvidence, HazardKind, MissingEvidence,
        NextAction, OperationFamily, Priority, ProofPath, ReachEvidence, ReviewCard, ReviewClass,
        SourceLocation, UnsafeOperation, UnsafeSite, UnsafeSiteKind, WitnessEvidence, WitnessKind,
        WitnessRoute,
    };

    /// Minimal card for coverage-block tests.
    ///
    /// `missing` contains one `"contract"`-kind entry so that:
    ///   - Gate 2 of `compute_agent_lsp_readiness` (empty-missing → Unsupported) is satisfied.
    ///   - Gate 3 (all-missing-is-witness → RequiresWitnessReceipt) does not fire.
    ///   - Gate 10 (has_card_scoped_repairs) is satisfied via the `"contract"` kind, which
    ///     `domain_has_card_scoped_repairs` matches through `card_missing_repairs`.
    ///
    /// Tests that need a specific `missing` state must override `card.missing` explicitly.
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
            missing: vec![MissingEvidence {
                kind: "contract".to_string(),
                message: "no safety contract was found".to_string(),
            }],
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

    /// Drift-lock: a guarded_unwitnessed card whose only missing entries are witness
    /// receipts must produce RequiresWitnessReceipt, not Ready (single-truth parity
    /// with output/agent/readiness.rs lines 17-23).
    ///
    /// Without this check the coverage block / LSP / telemetry histogram disagrees
    /// with the agent packet for the same card — the same class of drift that was
    /// fixed in #1632 for the Loom/Sanitizer/KaniOrCrux classes.
    #[test]
    fn agent_lsp_readiness_requires_witness_receipt_when_only_missing_is_witness() {
        use crate::domain::MissingEvidence;
        let mut card = minimal_card(ReviewClass::GuardedUnwitnessed);
        card.discharge = DischargeEvidence::present("bounds check");
        card.missing = vec![MissingEvidence {
            kind: "witness".to_string(),
            message: "no imported witness receipt was found".to_string(),
        }];
        let block = CoverageBlock::derive(&card);
        assert_eq!(
            block.agent_lsp_readiness,
            AgentLspReadiness::RequiresWitnessReceipt,
            "guarded_unwitnessed card with only witness-kind missing entries must produce \
             RequiresWitnessReceipt — revert to break parity with agent packet builder"
        );
        assert_eq!(
            block.agent_lsp_readiness.as_str(),
            "requires_witness_receipt"
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
