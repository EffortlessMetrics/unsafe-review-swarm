use crate::analysis::{classify, evidence, obligations, receipts, witness};
use crate::domain::{MissingEvidence, NextAction, Priority, ReviewCard, ReviewClass};

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
    let (reach, related_tests) =
        evidence::reach_evidence(ctx.root, scanned_site.site.owner.as_ref());
    let mut obligation_evidence =
        evidence::obligation_evidence(&scanned_site, &obligations, &contract, &reach);
    let discharge = evidence::summarize_discharge(&obligation_evidence);
    let routes = witness::routes_for(&hazards, scanned_site.site.owner.as_ref());
    let (mut class, mut priority, confidence) =
        classify::classify(&hazards, &contract, &discharge, &reach);
    let mut missing = Vec::new();

    if !contract.present {
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

    let id = super::card_id(ctx.package, &scanned_site, &hazards, ctx.identity_counts);
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

    let next_action_summary = if is_js_buffer_reentry_heuristic(&scanned_site.operation.expression)
    {
        "JS-backed buffer descriptor is captured before a possible JS reentry point and materialized afterward; parse options before capture or re-fetch/copy bytes after reentry, then attach a focused sanitizer/runtime receipt if available.".to_string()
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
        site: scanned_site.site,
        operation: scanned_site.operation,
        hazards,
        obligations,
        obligation_evidence,
        contract,
        discharge,
        reach,
        witness: witness_evidence,
        missing,
        routes,
        next_action,
        related_tests,
    }
}

fn is_js_buffer_reentry_heuristic(expression: &str) -> bool {
    expression.contains("JS-backed buffer descriptor captured before possible JS reentry")
}
