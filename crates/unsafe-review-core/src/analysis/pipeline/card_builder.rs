use crate::analysis::{classify, evidence, obligations, receipts, witness};
use crate::domain::{
    ContractEvidence, MissingEvidence, NextAction, OperationFamily, Priority, ProofPath,
    ReviewCard, ReviewClass, WitnessKind, WitnessRoute,
};

pub(super) struct CardBuildContext<'a> {
    pub root: &'a std::path::Path,
    pub package: &'a str,
    pub receipt_index: &'a receipts::ReceiptIndex,
    pub policy_state: &'a crate::policy::PolicyState,
    pub identity_counts: &'a mut std::collections::BTreeMap<String, usize>,
}

pub(super) fn build_card(
    ctx: &mut CardBuildContext<'_>,
    scanned_site: crate::analysis::scanner::ScannedSite,
) -> ReviewCard {
    let hazards = obligations::hazards_for(&scanned_site.operation.family);
    let obligations = obligations::obligations_for(&scanned_site.operation.family);
    let contract = evidence::contract_evidence(&scanned_site);
    let contract_for_classification = operation_contract_override(&scanned_site.operation.family)
        .unwrap_or_else(|| contract.clone());
    let (reach, related_tests) =
        evidence::reach_evidence(ctx.root, scanned_site.site.owner.as_ref());
    let id =
        super::card_identity::card_id(ctx.package, &scanned_site, &hazards, ctx.identity_counts);
    let reach = ctx.receipt_index.reach_evidence_for(&id, reach);
    let mut obligation_evidence = evidence::obligation_evidence(
        &scanned_site,
        &obligations,
        &contract_for_classification,
        &reach,
    );
    let discharge = evidence::summarize_discharge(&obligation_evidence);
    let routes = witness::routes_for(&hazards, scanned_site.site.owner.as_ref());
    let (mut class, mut priority, confidence) =
        classify::classify(&hazards, &contract_for_classification, &discharge, &reach);
    let mut missing = Vec::new();

    if !operation_skips_safety_contract(&scanned_site.operation.family) && !contract.present {
        let contract_missing_message = if scanned_site.site.public_api_surface {
            "Missing public `# Safety` documentation for unsafe API"
        } else {
            "Missing `# Safety` documentation or `SAFETY:` / `Safety:` comment"
        };
        missing.push(MissingEvidence::new("contract", contract_missing_message));
    }
    if !discharge.present {
        missing.push(MissingEvidence::new(
            "guard",
            "Missing visible local guard for inferred safety obligations",
        ));
    }
    if reach.state == "unreached" {
        missing.push(MissingEvidence::new(
            "reach",
            "No related test path was found by static search",
        ));
    }

    let verify_commands = routes
        .iter()
        .filter_map(|route| route.command.clone())
        .collect::<Vec<_>>();

    let witness_evidence = ctx.receipt_index.witness_evidence_for(&id, &routes);

    if witness_evidence.present {
        for evidence in &mut obligation_evidence {
            evidence.witness = crate::domain::EvidenceState::present(&witness_evidence.summary);
        }
        if class == ReviewClass::GuardedUnwitnessed {
            class = ReviewClass::GuardedAndWitnessed;
            priority = Priority::Low;
        }
    }

    if ctx.policy_state.is_suppressed(&id) {
        class = ReviewClass::Suppressed;
        priority = Priority::Low;
    } else if ctx.policy_state.is_baseline_known(&id) {
        class = ReviewClass::BaselineKnown;
        priority = Priority::Low;
    }

    let next_action_summary = if scanned_site.operation.family
        == OperationFamily::StableByteSourceGetterReentry
    {
        "Review this stable-byte-source-getter-reentry card through an observable-red-green proof path: add or expose byte-stability guard evidence, confirm the safe JS caller route, then parse options before capture or re-fetch/copy bytes after reentry, and keep the PR inside that getter-reentry aperture.".to_string()
    } else if scanned_site.operation.family == OperationFamily::StableByteSourceRabAsync {
        "Review this stable-byte-source-rab-async card through an observable-red-green proof path: add or expose byte-stability guard evidence, confirm the safe JS caller route, snapshot before async scheduling or helper materialization, and keep the PR inside that RAB async aperture.".to_string()
    } else if scanned_site.operation.family == OperationFamily::StableByteSourceSabRace {
        "Review this stable-byte-source-sab-race card through a mutation-plus-Miri/model proof path: add or expose byte-stability guard evidence, confirm the safe JS caller route, snapshot shared bytes before Rust/native borrowed-slice materialization, and keep the PR inside that SAB race aperture.".to_string()
    } else if scanned_site.operation.family == OperationFamily::StableByteSourceNativeFfiRead {
        "Review this stable-byte-source-native-ffi-read card through an observable-red-green proof path: add or expose byte-stability guard evidence, confirm the safe JS caller route, snapshot or otherwise stabilize JS-backed bytes before native FFI pointer/length reads, and keep the PR inside that native FFI aperture.".to_string()
    } else if scanned_site.operation.family == OperationFamily::PanicFromSafeJs {
        "Add an explicit sign/range guard or fallible error return before converting the JS-derived signed value to an unsigned type, then attach a focused Bun runtime receipt showing safe JS throws/returns instead of aborting.".to_string()
    } else {
        super::next_action_summary(
            &class,
            scanned_site.operation.family.as_str(),
            scanned_site.site.public_api_surface,
            &routes,
        )
    };
    let next_action = NextAction {
        summary: next_action_summary,
        verify_commands,
    };
    let proof_path = if scanned_site.operation.family
        == OperationFamily::StableByteSourceGetterReentry
        || scanned_site.operation.family == OperationFamily::StableByteSourceRabAsync
        || scanned_site.operation.family == OperationFamily::StableByteSourceNativeFfiRead
    {
        ProofPath::ObservableRedGreen
    } else if scanned_site.operation.family == OperationFamily::StableByteSourceSabRace {
        ProofPath::MutationMiriModel
    } else {
        proof_path_for(&class, &routes)
    };

    if !witness_evidence.present {
        missing.push(MissingEvidence::new(
            "witness",
            "No witness receipt imported for this card",
        ));
    }

    ReviewCard {
        id,
        class,
        priority,
        confidence,
        proof_path,
        site: scanned_site.site,
        operation: scanned_site.operation,
        hazards,
        obligations,
        obligation_evidence,
        contract: contract_for_classification,
        discharge,
        reach,
        witness: witness_evidence,
        missing,
        routes,
        next_action,
        related_tests,
    }
}

fn proof_path_for(class: &ReviewClass, routes: &[WitnessRoute]) -> ProofPath {
    if routes.is_empty()
        || routes.iter().all(|route| {
            matches!(
                route.kind,
                WitnessKind::HumanDeepReview | WitnessKind::Unsupported
            )
        })
    {
        return ProofPath::HumanReviewOnly;
    }

    match class {
        ReviewClass::UnsafeUnreached => ProofPath::HelperGated,
        ReviewClass::RequiresLoom | ReviewClass::RequiresKaniOrCrux => ProofPath::MutationMiriModel,
        ReviewClass::GuardedAndWitnessed
        | ReviewClass::GuardedUnwitnessed
        | ReviewClass::ReachableUnwitnessed
        | ReviewClass::WitnessMismatch
        | ReviewClass::RequiresSanitizer
        | ReviewClass::MiriUnsupported => ProofPath::ObservableRedGreen,
        ReviewClass::ContractMissing
        | ReviewClass::GuardMissing
        | ReviewClass::StaticUnknown
        | ReviewClass::BaselineKnown
        | ReviewClass::Suppressed => ProofPath::SourceRouteOnly,
    }
}

fn operation_skips_safety_contract(family: &OperationFamily) -> bool {
    operation_contract_override(family).is_some()
}

fn operation_contract_override(family: &OperationFamily) -> Option<ContractEvidence> {
    match family {
        OperationFamily::PanicFromSafeJs => Some(ContractEvidence::present(
            "Panic-safety heuristic uses local guard evidence, not unsafe API safety docs",
        )),
        OperationFamily::StableByteSourceGetterReentry => Some(ContractEvidence::present(
            "Stable-byte-source heuristic uses byte-stability evidence, not unsafe API safety docs",
        )),
        OperationFamily::StableByteSourceRabAsync => Some(ContractEvidence::present(
            "Stable-byte-source RAB async heuristic uses byte-stability evidence, not unsafe API safety docs",
        )),
        OperationFamily::StableByteSourceSabRace => Some(ContractEvidence::present(
            "Stable-byte-source SAB race heuristic uses byte-stability evidence, not unsafe API safety docs",
        )),
        OperationFamily::StableByteSourceNativeFfiRead => Some(ContractEvidence::present(
            "Stable-byte-source native FFI heuristic uses byte-stability evidence, not unsafe API safety docs",
        )),
        _ => None,
    }
}
