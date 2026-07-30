use crate::domain::coverage::{AgentLspReadiness, compute_agent_lsp_readiness};
use crate::domain::{OperationFamily, ReviewCard};
use crate::util::path_display;
use serde::Serialize;

const CLAIM_BOUNDARY: &str = "advisory repair candidate only; not a patch, execution result, witness receipt, proof, or safety claim";

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RepairCandidateKind {
    SafetyDocs,
    Guard,
    Test,
    WitnessRoute,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RepairCandidateApplicability {
    Candidate,
    HumanOnly,
    RequiresWitness,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(crate) struct RepairCandidate {
    pub(crate) repair_id: String,
    pub(crate) kind: RepairCandidateKind,
    pub(crate) target: RepairCandidateTarget,
    pub(crate) preconditions: Vec<String>,
    pub(crate) allowed_change: String,
    pub(crate) forbidden_substitutes: Vec<String>,
    pub(crate) verification: Vec<String>,
    pub(crate) expected_evidence_movement: Vec<RepairEvidenceMovement>,
    pub(crate) applicability: RepairCandidateApplicability,
    pub(crate) claim_boundary: &'static str,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(crate) struct RepairCandidateTarget {
    pub(crate) file: String,
    pub(crate) range: RepairCandidateRange,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(crate) struct RepairCandidateRange {
    pub(crate) start: RepairCandidatePosition,
    pub(crate) end: RepairCandidatePosition,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(crate) struct RepairCandidatePosition {
    pub(crate) line: usize,
    pub(crate) column: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(crate) struct RepairEvidenceMovement {
    pub(crate) slot: &'static str,
    pub(crate) from: &'static str,
    pub(crate) to: &'static str,
}

pub(super) fn build(card: &ReviewCard) -> Vec<RepairCandidate> {
    if !supports_bounded_candidates(&card.operation.family) {
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
            RepairCandidateKind::SafetyDocs,
            "add-safety-contract",
            vec!["the contract obligation remains undischarged".to_string()],
            "add or expose the local safety contract for this card's obligations",
            vec![
                "SAFETY comment alone".to_string(),
                "broad suppression".to_string(),
                "claiming a contract is proof".to_string(),
            ],
            "contract_coverage",
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
            RepairCandidateKind::Guard,
            &repair_id,
            vec![evidence.obligation.description.clone()],
            &format!(
                "add a same-origin executable guard for the `{}` obligation at this card's unsafe site",
                evidence.obligation.key
            ),
            vec![
                "SAFETY comment alone".to_string(),
                "debug_assert only".to_string(),
                "broad suppression".to_string(),
            ],
            "guard_coverage",
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
            RepairCandidateKind::Test,
            "add-focused-test",
            vec!["the owner or unsafe seam lacks focused test reach evidence".to_string()],
            "add or point to a focused test that exercises this owner or seam",
            vec![
                "test mention without exercising the unsafe owner".to_string(),
                "broad suppression".to_string(),
            ],
            "test_reach_coverage",
        );
    }

    if card.missing.iter().any(|missing| missing.kind == "witness") {
        push_candidate(
            &mut candidates,
            card,
            RepairCandidateKind::WitnessRoute,
            "attach-witness-receipt",
            vec!["the selected witness route remains unconfirmed".to_string()],
            "attach a scoped witness receipt after running the suggested command outside unsafe-review",
            vec![
                "treating a suggested command as an executed witness".to_string(),
                "using an unrelated receipt as proof".to_string(),
            ],
            "witness_receipt_coverage",
        );
    }

    candidates
}

fn push_candidate(
    candidates: &mut Vec<RepairCandidate>,
    card: &ReviewCard,
    kind: RepairCandidateKind,
    repair_id: &str,
    preconditions: Vec<String>,
    allowed_change: &str,
    forbidden_substitutes: Vec<String>,
    evidence_slot: &'static str,
) {
    if candidates
        .iter()
        .any(|candidate| candidate.repair_id == repair_id)
    {
        return;
    }
    let applicability = applicability(card, &kind);
    candidates.push(RepairCandidate {
        repair_id: repair_id.to_string(),
        kind: kind.clone(),
        target: target(card),
        preconditions,
        allowed_change: allowed_change.to_string(),
        forbidden_substitutes,
        verification: card.next_action.verify_commands.clone(),
        expected_evidence_movement: vec![RepairEvidenceMovement {
            slot: evidence_slot,
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
    fn witness_route_candidate_is_receipt_gated() {
        let mut card = candidate_card();
        card.missing
            .push(MissingEvidence::new("witness", "no receipt"));
        let witness = build(&card)
            .into_iter()
            .find(|candidate| candidate.kind == RepairCandidateKind::WitnessRoute)
            .expect("witness gap should produce a typed route candidate");

        assert_eq!(
            witness.applicability,
            RepairCandidateApplicability::RequiresWitness
        );
        assert_eq!(
            witness.expected_evidence_movement[0].slot,
            "witness_receipt_coverage"
        );
    }

    #[test]
    fn weak_confidence_candidate_is_human_only() {
        let mut card = candidate_card();
        card.confidence = crate::domain::Confidence::Low;
        let guard = build(&card)
            .into_iter()
            .find(|candidate| candidate.kind == RepairCandidateKind::Guard)
            .expect("guard gap should produce a typed candidate");

        assert_eq!(guard.applicability, RepairCandidateApplicability::HumanOnly);
    }
}
