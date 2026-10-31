use crate::analysis::{classify, evidence, obligations, receipts, witness};
use crate::domain::{
    ContractEvidence, GENERIC_GUARD_MISSING_MESSAGE, MissingEvidence, NextAction,
    ObligationEvidence, OperationFamily, Priority, ProofPath, ReviewCard, ReviewClass, WitnessKind,
    WitnessRoute,
};

const GENERIC_DISCHARGE_SUMMARIES: &[&str] = &[
    "No obligation-specific guard code was detected",
    "No length or bounds guard code was detected",
    "No alignment guard code was detected",
    "No capacity guard code was detected",
    "No nullability guard code was detected",
    "No pointer-liveness evidence was detected",
    "No Vec::from_raw_parts same-origin pointer/capacity evidence was detected",
    "No return-value check with a diverging error arm was detected",
    "No initialization evidence was detected",
];

fn is_generic_discharge_summary(evidence: &ObligationEvidence) -> bool {
    let summary = evidence.discharge.summary.as_str();
    GENERIC_DISCHARGE_SUMMARIES.contains(&summary)
        || summary
            == format!(
                "No obligation-specific guard code was detected for the `{}` obligation (no guard pattern is recognized for this obligation)",
                evidence.obligation.key
            )
}

fn specific_discharge_summaries(evidence: &[ObligationEvidence]) -> Vec<&str> {
    let mut summaries: Vec<&str> = Vec::new();
    for entry in evidence {
        if !entry.discharge.present
            && !is_generic_discharge_summary(entry)
            && !summaries.contains(&entry.discharge.summary.as_str())
        {
            summaries.push(entry.discharge.summary.as_str());
        }
    }
    summaries
}

fn guard_missing_message(evidence: &[ObligationEvidence]) -> String {
    let specific = specific_discharge_summaries(evidence);
    if specific.is_empty() {
        GENERIC_GUARD_MISSING_MESSAGE.to_string()
    } else {
        format!("{}: {}", GENERIC_GUARD_MISSING_MESSAGE, specific.join("; "))
    }
}

pub(super) struct CardBuildContext<'a> {
    pub root: &'a std::path::Path,
    pub package: &'a str,
    pub receipt_index: &'a receipts::ReceiptIndex,
    pub policy_state: &'a crate::policy::PolicyState,
    pub identity_counts: &'a mut std::collections::BTreeMap<String, usize>,
    pub tree_facts: &'a receipts::CurrentTreeFacts,
}

pub(super) fn build_card(
    ctx: &mut CardBuildContext<'_>,
    scanned_site: crate::analysis::scanner::ScannedSite,
) -> ReviewCard {
    let hazards = obligations::hazards_for(&scanned_site.operation.family);
    let mut obligations = obligations::obligations_for(&scanned_site.operation.family);
    if scanned_site.operation.family == OperationFamily::Ffi {
        obligations.extend(obligations::ffi_return_value_obligation(
            scanned_site.operation.bound_name.as_deref(),
        ));
    }
    let contract = evidence::contract_evidence(&scanned_site);
    let contract_for_classification = operation_contract_override(&scanned_site.operation.family)
        .unwrap_or_else(|| contract.clone());
    let (static_reach, related_tests) =
        evidence::reach_evidence(ctx.root, scanned_site.site.owner.as_ref());
    let id =
        super::card_identity::card_id(ctx.package, &scanned_site, &hazards, ctx.identity_counts);
    // Probe routes from static reach carry the same commands as the final
    // routes (`reached` only adjusts reason text), so the invocation digest
    // re-derived here is stable regardless of receipt-augmented reach.
    let probe_routes = witness::routes_for(
        &hazards,
        scanned_site.site.owner.as_ref(),
        static_reach.state != "unreached",
    );
    let current = receipts::current_binding_for_parts(
        &id.0,
        scanned_site.operation.family.as_str(),
        scanned_site.site.owner.as_deref(),
        &scanned_site.site.location.file,
        &scanned_site.site.snippet,
        &probe_routes,
        ctx.tree_facts,
    );
    let reach = ctx
        .receipt_index
        .reach_evidence_for(&id, static_reach, &current);
    let mut obligation_evidence = evidence::obligation_evidence(
        &scanned_site,
        &obligations,
        &contract_for_classification,
        &reach,
    );
    let discharge = evidence::summarize_discharge(&obligation_evidence);
    let routes = witness::routes_for(
        &hazards,
        scanned_site.site.owner.as_ref(),
        reach.state != "unreached",
    );
    let (mut class, mut priority, confidence) =
        classify::classify(&hazards, &contract_for_classification, &discharge, &reach);
    let mut missing = Vec::new();

    if !operation_skips_safety_contract(&scanned_site.operation.family) && !contract.present {
        let contract_missing_message = if scanned_site.site.public_api_surface {
            "Missing public `# Safety` documentation for unsafe API"
        } else if scanned_site.site.visibility == "restricted" {
            "Missing `# Safety` documentation for restricted-visibility unsafe fn (in-crate callers need the contract)"
        } else {
            "Missing `# Safety` documentation or `SAFETY:` / `Safety:` comment"
        };
        missing.push(MissingEvidence::new("contract", contract_missing_message));
    }
    if !discharge.present {
        missing.push(MissingEvidence::new(
            "guard",
            guard_missing_message(&obligation_evidence),
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

    let witness_evidence = ctx
        .receipt_index
        .witness_evidence_for(&id, &routes, &current);

    if witness_evidence.present {
        for evidence in &mut obligation_evidence {
            evidence.witness = crate::domain::EvidenceState::present(&witness_evidence.summary);
        }
        // A structured verdict (`confirmed` / `not_reproduced` /
        // `inconclusive`) is an observation about one run, not a discharge of
        // the witness obligation: a reproduced failure still requires
        // attention, an inconclusive run leaves adequacy unresolved, and a
        // single non-reproduction is not a safety claim. Keep the witness
        // work open (and the hazard-derived priority intact) while retaining
        // the receipt evidence and its limitations in the slots. Only a
        // verdict-less receipt (legacy/human `executed` / `receipt_imported`
        // provenance) closes the card as before.
        if class == ReviewClass::GuardedUnwitnessed && witness_evidence.verdict.is_none() {
            class = ReviewClass::GuardedAndWitnessed;
            priority = Priority::Low;
        }
    } else {
        // Missing, mismatched, stale, or unknown: no imported receipt counts
        // as current evidence. Every obligation-level witness slot carries
        // the same missing summary as the top-level card witness slot so
        // that the two remain consistent (the check-fixtures gate enforces
        // this invariant); the summary names the cause — absent, wrong
        // tool, stale, or unknown subject — while the card keeps its
        // urgency and the witness gap stays open.
        for evidence in &mut obligation_evidence {
            evidence.witness = crate::domain::EvidenceState::missing(&witness_evidence.summary);
        }
        if class == ReviewClass::GuardedUnwitnessed
            && ctx.receipt_index.has_tool_mismatch_for(&id, &routes)
        {
            // A receipt exists but its tool does not match any routed witness tool.
            // Only upgrade from GuardedUnwitnessed: the card is otherwise sound
            // (contract + guard + reach present) but the receipt is broken.
            // Surfacing it as WitnessMismatch lets every output surface treat it as
            // an open actionable condition (see is_actionable() in classification.rs).
            class = ReviewClass::WitnessMismatch;
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
            &scanned_site.site.visibility,
            &routes,
            &obligation_evidence,
            contract_for_classification.present,
            scanned_site.site.owner.as_deref(),
            &scanned_site.context_before,
            witness_evidence.confirmation_state(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{EvidenceState, SafetyObligation};

    fn evidence(key: &str, summary: &str, present: bool) -> ObligationEvidence {
        let discharge = if present {
            EvidenceState::present(summary)
        } else {
            EvidenceState::missing(summary)
        };
        ObligationEvidence {
            obligation: SafetyObligation::new(key, "desc"),
            contract: EvidenceState::missing("contract"),
            discharge,
            reach: EvidenceState::missing("reach"),
            witness: EvidenceState::missing("witness"),
        }
    }

    #[test]
    fn specific_discharge_is_appended_after_generic_guard_text() {
        for summary in [
            "asm template addresses memory while options declare nomem",
            "asm template writes memory through its destination operand while options declare readonly",
        ] {
            let message = guard_missing_message(&[evidence("asm", summary, false)]);
            assert_eq!(
                message,
                format!("{GENERIC_GUARD_MISSING_MESSAGE}: {summary}")
            );
        }
    }

    #[test]
    fn generic_discharge_stays_byte_identical() {
        for (key, summary) in [
            ("asm", "No obligation-specific guard code was detected"),
            ("bounds", "No length or bounds guard code was detected"),
            ("alignment", "No alignment guard code was detected"),
            (
                "unhandled",
                "No obligation-specific guard code was detected for the `unhandled` obligation (no guard pattern is recognized for this obligation)",
            ),
        ] {
            assert_eq!(
                guard_missing_message(&[evidence(key, summary, false)]),
                GENERIC_GUARD_MISSING_MESSAGE
            );
        }
    }

    #[test]
    fn present_and_duplicate_discharge_do_not_project() {
        let present = guard_missing_message(&[evidence(
            "asm",
            "asm template addresses memory while options declare nomem",
            true,
        )]);
        assert_eq!(present, GENERIC_GUARD_MISSING_MESSAGE);
        let dup = guard_missing_message(&[
            evidence(
                "asm",
                "asm template addresses memory while options declare nomem",
                false,
            ),
            evidence(
                "asm",
                "asm template addresses memory while options declare nomem",
                false,
            ),
        ]);
        assert_eq!(dup.matches("asm template addresses memory").count(), 1);
    }
}
