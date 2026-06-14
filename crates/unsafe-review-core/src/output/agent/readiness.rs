use super::queue::{AgentReadiness, REQUIRES_HUMAN_REVIEW, REQUIRES_WITNESS_RECEIPT, UNSUPPORTED};
use crate::domain::ReviewCard;
use crate::domain::coverage::{AgentLspReadiness, compute_agent_lsp_readiness};

/// Build an [`AgentReadiness`] for `card` by delegating to the single shared
/// readiness function [`compute_agent_lsp_readiness`].
///
/// This is the only call-site that converts [`AgentLspReadiness`] (the domain
/// state enum) into [`AgentReadiness`] (the serialisable agent-packet struct).
/// Because both this function and [`crate::domain::coverage::derive_agent_lsp_readiness`]
/// call [`compute_agent_lsp_readiness`], the agent packet's `agent_readiness.state`
/// and `coverage.agent_lsp_readiness` are guaranteed identical for the same card
/// (resolves output audit #1687 findings 3+4).
pub(super) fn build(card: &ReviewCard, has_card_scoped_repairs: bool) -> AgentReadiness {
    let result = compute_agent_lsp_readiness(card, has_card_scoped_repairs);
    match result.state {
        AgentLspReadiness::Ready => AgentReadiness::ready_for_agent(result.reasons),
        AgentLspReadiness::NeedsHuman => {
            AgentReadiness::not_ready(REQUIRES_HUMAN_REVIEW, result.reasons)
        }
        AgentLspReadiness::RequiresWitnessReceipt => {
            AgentReadiness::not_ready(REQUIRES_WITNESS_RECEIPT, result.reasons)
        }
        AgentLspReadiness::Unsupported => AgentReadiness::not_ready(UNSUPPORTED, result.reasons),
    }
}

#[cfg(test)]
mod tests {
    use super::super::queue::READY_FOR_AGENT;
    use super::*;
    use crate::domain::{
        CardId, Confidence, ContractEvidence, DischargeEvidence, HazardKind, NextAction,
        OperationFamily, Priority, ProofPath, ReachEvidence, ReviewCard, ReviewClass,
        SourceLocation, UnsafeOperation, UnsafeSite, UnsafeSiteKind, WitnessEvidence, WitnessKind,
        WitnessRoute,
    };

    fn minimal_ready_card() -> ReviewCard {
        use crate::domain::MissingEvidence;
        ReviewCard {
            id: CardId("UR-test-r1".to_string()),
            class: ReviewClass::GuardMissing,
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
                kind: "guard".to_string(),
                message: "missing guard".to_string(),
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
    fn build_returns_ready_state_for_ready_card() {
        let card = minimal_ready_card();
        // has_card_scoped_repairs=true because RawPointerDeref + no discharge
        let readiness = build(&card, true);
        assert!(readiness.ready);
        assert_eq!(readiness.state, READY_FOR_AGENT);
    }

    #[test]
    fn build_returns_unsupported_for_non_actionable_class() {
        let mut card = minimal_ready_card();
        card.class = ReviewClass::GuardedAndWitnessed;
        let readiness = build(&card, true);
        assert!(!readiness.ready);
        assert_eq!(readiness.state, UNSUPPORTED);
    }

    #[test]
    fn build_returns_unsupported_for_empty_missing() {
        let mut card = minimal_ready_card();
        card.missing.clear();
        let readiness = build(&card, true);
        assert!(!readiness.ready);
        assert_eq!(readiness.state, UNSUPPORTED);
        assert!(
            readiness
                .reasons
                .iter()
                .any(|r| r.contains("no missing evidence to repair")),
            "reason must mention 'no missing evidence to repair'"
        );
    }

    #[test]
    fn build_returns_requires_witness_receipt_when_all_missing_is_witness() {
        use crate::domain::MissingEvidence;
        let mut card = minimal_ready_card();
        card.missing = vec![MissingEvidence {
            kind: "witness".to_string(),
            message: "no receipt".to_string(),
        }];
        let readiness = build(&card, true);
        assert!(!readiness.ready);
        assert_eq!(readiness.state, REQUIRES_WITNESS_RECEIPT);
    }

    #[test]
    fn build_returns_requires_human_review_for_ffi_family() {
        let mut card = minimal_ready_card();
        card.operation.family = OperationFamily::Ffi;
        let readiness = build(&card, true);
        assert!(!readiness.ready);
        assert_eq!(readiness.state, REQUIRES_HUMAN_REVIEW);
    }

    #[test]
    fn build_returns_unsupported_for_low_confidence() {
        let mut card = minimal_ready_card();
        card.confidence = Confidence::Low;
        let readiness = build(&card, true);
        assert!(!readiness.ready);
        assert_eq!(readiness.state, UNSUPPORTED);
        assert!(
            readiness
                .reasons
                .iter()
                .any(|r| r.contains("too weak for bounded repair")),
            "reason must mention confidence weakness"
        );
    }

    #[test]
    fn build_returns_unsupported_when_no_scoped_repairs() {
        let card = minimal_ready_card();
        let readiness = build(&card, false);
        assert!(!readiness.ready);
        assert_eq!(readiness.state, UNSUPPORTED);
        assert!(
            readiness
                .reasons
                .iter()
                .any(|r| r.contains("no card-scoped allowed repair")),
            "reason must mention missing scoped repair"
        );
    }

    #[test]
    fn build_returns_unsupported_when_no_verify_commands() {
        let mut card = minimal_ready_card();
        card.next_action.verify_commands.clear();
        let readiness = build(&card, true);
        assert!(!readiness.ready);
        assert_eq!(readiness.state, UNSUPPORTED);
        assert!(
            readiness
                .reasons
                .iter()
                .any(|r| r.contains("no verify command")),
            "reason must mention missing verify command"
        );
    }
}
