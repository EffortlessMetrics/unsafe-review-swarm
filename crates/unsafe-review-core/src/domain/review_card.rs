use super::coverage::CoverageBlock;
use super::{
    CardId, Confidence, ContractEvidence, DischargeEvidence, HazardKind, MissingEvidence,
    ObligationEvidence, Priority, ProofPath, ReachEvidence, RelatedTest, ReviewClass,
    SafetyObligation, UnsafeOperation, UnsafeSite, WitnessEvidence, WitnessRoute,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NextAction {
    pub summary: String,
    pub verify_commands: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewCard {
    pub id: CardId,
    pub class: ReviewClass,
    pub priority: Priority,
    pub confidence: Confidence,
    pub proof_path: ProofPath,
    pub site: UnsafeSite,
    pub operation: UnsafeOperation,
    pub hazards: Vec<HazardKind>,
    pub obligations: Vec<SafetyObligation>,
    pub obligation_evidence: Vec<ObligationEvidence>,
    pub contract: ContractEvidence,
    pub discharge: DischargeEvidence,
    pub reach: ReachEvidence,
    pub witness: WitnessEvidence,
    pub missing: Vec<MissingEvidence>,
    pub routes: Vec<WitnessRoute>,
    pub next_action: NextAction,
    pub related_tests: Vec<RelatedTest>,
}

impl ReviewCard {
    /// Compute the machine-readable coverage block for this card (SPEC-0029).
    ///
    /// Slots that depend on not-yet-built pipeline stages default honestly:
    /// - `baseline_state = Unknown` (populated by SPEC-0030 movement)
    /// - `outcome_movement = Unknown` (populated by SPEC-0030 movement)
    /// - `comment_plan_status = NotEligible` (populated when comment plan runs)
    /// - `manual_context = Absent` (upgraded by manual-candidate overlay resolution)
    pub fn coverage_block(&self) -> CoverageBlock {
        CoverageBlock::derive(self)
    }
}
