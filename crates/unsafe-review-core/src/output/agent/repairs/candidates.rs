use crate::domain::coverage::{AgentLspReadiness, compute_agent_lsp_readiness};
use crate::domain::{OperationFamily, ReviewCard};
use crate::util::path_display;
use serde::Serialize;

const CLAIM_BOUNDARY: &str = "advisory repair candidate only; not a patch, execution result, witness receipt, proof, or safety claim";

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RepairCandidateKind {
    SafetyDocs,
    Guard,
    Test,
    WitnessRoute,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RepairCandidateApplicability {
    Candidate,
    HumanOnly,
    RequiresWitness,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct RepairCandidate {
    pub repair_id: String,
    pub kind: RepairCandidateKind,
    pub target: RepairCandidateTarget,
    pub preconditions: Vec<String>,
    pub allowed_change: String,
    pub forbidden_substitutes: Vec<String>,
    pub verification: Vec<String>,
    pub expected_evidence_movement: Vec<RepairEvidenceMovement>,
    pub applicability: RepairCandidateApplicability,
    pub claim_boundary: &'static str,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct RepairCandidateTarget {
    pub file: String,
    pub range: RepairCandidateRange,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct RepairCandidateRange {
    pub start: RepairCandidatePosition,
    pub end: RepairCandidatePosition,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct RepairCandidatePosition {
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct RepairEvidenceMovement {
    pub slot: &'static str,
    pub from: &'static str,
    pub to: &'static str,
}

pub(super) fn build(card: &ReviewCard) -> Vec<RepairCandidate> {
    if !supports_bounded_candidates(&card.operation.family)
        && !supports_unsafe_declaration_candidates(card)
    {
        return Vec::new();
    }

    let mut candidates = Vec::new();
    if card
        .missing
        .iter()
        .any(|missing| missing.kind == "contract")
    {
        push_candidate(
            &mut candidates,
            card,
            CandidateSpec {
                kind: RepairCandidateKind::SafetyDocs,
                repair_id: "add-safety-contract".to_string(),
                preconditions: vec!["the contract obligation remains undischarged".to_string()],
                allowed_change:
                    "add or expose the local safety contract for this card's obligations"
                        .to_string(),
                forbidden_substitutes: vec![
                    "SAFETY comment alone".to_string(),
                    "broad suppression".to_string(),
                    "claiming a contract is proof".to_string(),
                ],
                evidence_slot: "contract_coverage",
            },
        );
    }

    for evidence in &card.obligation_evidence {
        if evidence.discharge.present
            || !supports_guard_candidate(&card.operation.family, &evidence.obligation.key)
        {
            continue;
        }
        let repair_id = format!(
            "add-{}-{}-guard",
            card.operation.family.as_str(),
            evidence.obligation.key.replace('_', "-")
        );
        push_candidate(
            &mut candidates,
            card,
            CandidateSpec {
                kind: RepairCandidateKind::Guard,
                repair_id,
                preconditions: vec![evidence.obligation.description.clone()],
                allowed_change: format!(
                    "add a same-origin executable guard for the `{}` obligation at this card's unsafe site",
                    evidence.obligation.key
                ),
                forbidden_substitutes: vec![
                    "SAFETY comment alone".to_string(),
                    "debug_assert only".to_string(),
                    "broad suppression".to_string(),
                ],
                evidence_slot: "guard_coverage",
            },
        );
    }

    if card
        .missing
        .iter()
        .any(|missing| missing.kind == "reach" || missing.kind == "test")
    {
        push_candidate(
            &mut candidates,
            card,
            CandidateSpec {
                kind: RepairCandidateKind::Test,
                repair_id: "add-focused-test".to_string(),
                preconditions: vec![
                    "the owner or unsafe seam lacks focused test reach evidence".to_string(),
                ],
                allowed_change: "add or point to a focused test that exercises this owner or seam"
                    .to_string(),
                forbidden_substitutes: vec![
                    "test mention without exercising the unsafe owner".to_string(),
                    "broad suppression".to_string(),
                ],
                evidence_slot: "test_reach_coverage",
            },
        );
    }

    if card.missing.iter().any(|missing| missing.kind == "witness") {
        push_candidate(
            &mut candidates,
            card,
            CandidateSpec {
                kind: RepairCandidateKind::WitnessRoute,
                repair_id: "attach-witness-receipt".to_string(),
                preconditions: vec!["the selected witness route remains unconfirmed".to_string()],
                allowed_change: "attach a scoped witness receipt after running the suggested command outside unsafe-review".to_string(),
                forbidden_substitutes: vec![
                    "treating a suggested command as an executed witness".to_string(),
                    "using an unrelated receipt as proof".to_string(),
                ],
                evidence_slot: "witness_receipt_coverage",
            },
        );
    }

    candidates
}

struct CandidateSpec {
    kind: RepairCandidateKind,
    repair_id: String,
    preconditions: Vec<String>,
    allowed_change: String,
    forbidden_substitutes: Vec<String>,
    evidence_slot: &'static str,
}

fn push_candidate(candidates: &mut Vec<RepairCandidate>, card: &ReviewCard, spec: CandidateSpec) {
    if candidates
        .iter()
        .any(|candidate| candidate.repair_id == spec.repair_id)
    {
        return;
    }
    let applicability = applicability(card, &spec.kind);
    candidates.push(RepairCandidate {
        repair_id: spec.repair_id,
        kind: spec.kind,
        target: target(card),
        preconditions: spec.preconditions,
        allowed_change: spec.allowed_change,
        forbidden_substitutes: spec.forbidden_substitutes,
        verification: card.next_action.verify_commands.clone(),
        expected_evidence_movement: vec![RepairEvidenceMovement {
            slot: spec.evidence_slot,
            from: "missing",
            to: "present",
        }],
        applicability,
        claim_boundary: CLAIM_BOUNDARY,
    });
}

fn target(card: &ReviewCard) -> RepairCandidateTarget {
    let position = RepairCandidatePosition {
        line: card.site.location.line,
        column: card.site.location.column,
    };
    RepairCandidateTarget {
        file: path_display(&card.site.location.file),
        range: RepairCandidateRange {
            start: position.clone(),
            end: position,
        },
    }
}

fn applicability(card: &ReviewCard, kind: &RepairCandidateKind) -> RepairCandidateApplicability {
    if matches!(kind, RepairCandidateKind::WitnessRoute) {
        return RepairCandidateApplicability::RequiresWitness;
    }
    match compute_agent_lsp_readiness(card, true).state {
        AgentLspReadiness::Ready => RepairCandidateApplicability::Candidate,
        AgentLspReadiness::RequiresWitnessReceipt => RepairCandidateApplicability::RequiresWitness,
        AgentLspReadiness::NeedsHuman | AgentLspReadiness::Unsupported => {
            RepairCandidateApplicability::HumanOnly
        }
    }
}

fn supports_bounded_candidates(family: &OperationFamily) -> bool {
    matches!(
        family,
        OperationFamily::RawPointerDeref
            | OperationFamily::RawPointerRead
            | OperationFamily::RawPointerReadUnaligned
            | OperationFamily::RawPointerWrite
            | OperationFamily::RawPointerWriteUnaligned
            | OperationFamily::PtrCopy
            | OperationFamily::CopyNonOverlapping
            | OperationFamily::StrFromUtf8Unchecked
            | OperationFamily::MaybeUninitAssumeInit
            | OperationFamily::VecSetLen
            | OperationFamily::UnwrapUnchecked
            | OperationFamily::NonNullUnchecked
            | OperationFamily::GetUnchecked
    )
}

fn supports_unsafe_declaration_candidates(card: &ReviewCard) -> bool {
    card.site.public_api_surface
        && matches!(card.operation.family, OperationFamily::UnsafeDeclaration)
}

fn supports_guard_candidate(family: &OperationFamily, key: &str) -> bool {
    match family {
        OperationFamily::RawPointerDeref
        | OperationFamily::RawPointerRead
        | OperationFamily::RawPointerWrite => matches!(
            key,
            "pointer-live" | "bounds" | "alignment" | "initialized" | "allocation"
        ),
        OperationFamily::RawPointerReadUnaligned | OperationFamily::RawPointerWriteUnaligned => {
            matches!(
                key,
                "pointer-live" | "bounds" | "initialized" | "allocation"
            )
        }
        OperationFamily::PtrCopy => matches!(key, "valid-range" | "initialized"),
        OperationFamily::CopyNonOverlapping => matches!(key, "valid-range" | "non-overlap"),
        OperationFamily::StrFromUtf8Unchecked => key == "utf8",
        OperationFamily::MaybeUninitAssumeInit => key == "initialized",
        OperationFamily::VecSetLen => matches!(key, "capacity" | "initialized"),
        OperationFamily::UnwrapUnchecked => key == "valid-value",
        OperationFamily::NonNullUnchecked => key == "non-null",
        OperationFamily::GetUnchecked => key == "bounds",
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        CardId, Confidence, ContractEvidence, DischargeEvidence, EvidenceState, HazardKind,
        MissingEvidence, NextAction, OperationFamily, Priority, ProofPath, ReachEvidence,
        ReviewClass, SafetyObligation, SourceLocation, UnsafeOperation, UnsafeSite, UnsafeSiteKind,
        WitnessEvidence, WitnessKind, WitnessRoute,
    };

    fn candidate_card() -> ReviewCard {
        ReviewCard {
            id: CardId("UR-candidate-test".to_string()),
            class: ReviewClass::GuardMissing,
            priority: Priority::Medium,
            confidence: Confidence::Medium,
            proof_path: ProofPath::SourceRouteOnly,
            site: UnsafeSite {
                location: SourceLocation {
                    file: "src/lib.rs".into(),
                    line: 12,
                    column: 4,
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
            obligation_evidence: vec![crate::domain::ObligationEvidence {
                obligation: SafetyObligation::new("alignment", "pointer must be aligned"),
                contract: EvidenceState::missing("no contract"),
                discharge: EvidenceState::missing("no alignment guard"),
                reach: EvidenceState::missing("no test"),
                witness: EvidenceState::missing("no witness"),
            }],
            contract: ContractEvidence::missing(),
            discharge: DischargeEvidence::missing(),
            reach: ReachEvidence {
                state: "missing".to_string(),
                summary: "no tests".to_string(),
            },
            witness: WitnessEvidence::missing(),
            missing: vec![MissingEvidence::new("guard", "missing guard")],
            routes: vec![WitnessRoute {
                kind: WitnessKind::Miri,
                reason: "test".to_string(),
                command: Some("cargo miri test".to_string()),
                required: false,
            }],
            next_action: NextAction {
                summary: "add guard".to_string(),
                verify_commands: vec!["cargo test focused".to_string()],
            },
            related_tests: vec![],
        }
    }

    #[test]
    fn typed_guard_candidate_has_exact_target_and_machine_movement() -> Result<(), String> {
        let card = candidate_card();
        let candidates = build(&card);
        let guard = candidates
            .iter()
            .find(|candidate| candidate.kind == RepairCandidateKind::Guard)
            .ok_or_else(|| "expected a typed guard candidate".to_string())?;

        assert_eq!(guard.target.file, "src/lib.rs");
        assert_eq!(guard.target.range.start, guard.target.range.end);
        assert_eq!(guard.expected_evidence_movement[0].slot, "guard_coverage");
        assert_eq!(guard.applicability, RepairCandidateApplicability::Candidate);
        assert!(
            guard
                .verification
                .iter()
                .any(|command| command.contains("test"))
        );
        assert!(guard.claim_boundary.contains("not a patch"));
        Ok(())
    }

    #[test]
    fn ambiguous_operation_families_receive_no_typed_auto_candidate() {
        assert!(!supports_bounded_candidates(&OperationFamily::Ffi));
        assert!(!supports_bounded_candidates(&OperationFamily::InlineAsm));
        assert!(!supports_bounded_candidates(
            &OperationFamily::UnsafeImplSendSync
        ));
        assert!(!supports_bounded_candidates(
            &OperationFamily::AtomicPointerState
        ));
    }

    #[test]
    fn witness_route_candidate_is_receipt_gated() -> Result<(), String> {
        let mut card = candidate_card();
        card.missing
            .push(MissingEvidence::new("witness", "no receipt"));
        let witness = build(&card)
            .into_iter()
            .find(|candidate| candidate.kind == RepairCandidateKind::WitnessRoute)
            .ok_or_else(|| "witness gap should produce a typed route candidate".to_string())?;

        assert_eq!(
            witness.applicability,
            RepairCandidateApplicability::RequiresWitness
        );
        assert_eq!(
            witness.expected_evidence_movement[0].slot,
            "witness_receipt_coverage"
        );
        Ok(())
    }

    #[test]
    fn weak_confidence_candidate_is_human_only() -> Result<(), String> {
        let mut card = candidate_card();
        card.confidence = crate::domain::Confidence::Low;
        let guard = build(&card)
            .into_iter()
            .find(|candidate| candidate.kind == RepairCandidateKind::Guard)
            .ok_or_else(|| "guard gap should produce a typed candidate".to_string())?;

        assert_eq!(guard.applicability, RepairCandidateApplicability::HumanOnly);
        Ok(())
    }

    #[test]
    fn public_unsafe_declaration_contract_candidate_is_human_only() -> Result<(), String> {
        let mut card = candidate_card();
        card.operation.family = OperationFamily::UnsafeDeclaration;
        card.site.public_api_surface = true;
        card.missing = vec![MissingEvidence::new(
            "contract",
            "public unsafe declaration is missing a safety contract",
        )];

        let contract = build(&card)
            .into_iter()
            .find(|candidate| candidate.kind == RepairCandidateKind::SafetyDocs)
            .ok_or_else(|| "unsafe declaration should expose a contract candidate".to_string())?;

        assert_eq!(contract.repair_id, "add-safety-contract");
        assert_eq!(
            contract.applicability,
            RepairCandidateApplicability::HumanOnly
        );
        assert_eq!(
            contract.expected_evidence_movement[0].slot,
            "contract_coverage"
        );
        assert!(contract.allowed_change.contains("safety contract"));
        Ok(())
    }

    #[test]
    fn private_unsafe_declaration_has_no_typed_contract_candidate() {
        let mut card = candidate_card();
        card.operation.family = OperationFamily::UnsafeDeclaration;
        card.missing = vec![MissingEvidence::new(
            "contract",
            "private unsafe declaration is missing a safety contract",
        )];

        assert!(build(&card).is_empty());
    }

    #[test]
    fn focused_test_candidate_tracks_reach_evidence() -> Result<(), String> {
        let mut card = candidate_card();
        card.missing.push(MissingEvidence::new(
            "reach",
            "no focused test reaches the unsafe owner",
        ));

        let test_candidate = build(&card)
            .into_iter()
            .find(|candidate| candidate.kind == RepairCandidateKind::Test)
            .ok_or_else(|| "reach gap should produce a typed test candidate".to_string())?;

        assert_eq!(test_candidate.repair_id, "add-focused-test");
        assert_eq!(
            test_candidate.expected_evidence_movement[0].slot,
            "test_reach_coverage"
        );
        assert!(
            test_candidate
                .forbidden_substitutes
                .iter()
                .any(|item| { item.contains("without exercising the unsafe owner") })
        );
        Ok(())
    }
}
