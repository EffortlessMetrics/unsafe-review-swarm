use crate::domain::{ObligationEvidence, ReviewClass, WitnessKind, WitnessRoute};

#[allow(
    clippy::too_many_arguments,
    reason = "caller-contract routing needs site context alongside the class inputs; a params struct would churn every unit call site without clarity gain"
)]
pub(super) fn next_action_summary(
    class: &ReviewClass,
    operation: &str,
    public_api_surface: bool,
    visibility: &str,
    routes: &[WitnessRoute],
    obligation_evidence: &[ObligationEvidence],
    contract_present: bool,
    owner: Option<&str>,
    context_before: &[String],
) -> String {
    match class {
        ReviewClass::ContractMissing if public_api_surface => {
            "Add a precise public `# Safety` section that names the required caller obligations."
                .to_string()
        }
        ReviewClass::ContractMissing if visibility == "restricted" => {
            // pub(crate)/pub(super)/pub(in …): not public API, but in-crate
            // callers still need the unsafe contract documented.
            "Document the unsafe contract for in-crate callers with a `# Safety` section or `SAFETY:` comment naming the required conditions."
                .to_string()
        }
        ReviewClass::ContractMissing => "Add a precise `# Safety` section or `SAFETY:` / `Safety:` comment that names the required conditions.".to_string(),
        ReviewClass::GuardMissing if operation == "unsafe_declaration" => {
            "Review the unsafe declaration manually and add or expose the missing obligation-specific guard.".to_string()
        }
        ReviewClass::GuardMissing if operation == "unknown" => "Review the unsafe site manually and add the missing obligation-specific guard once the contract is identified.".to_string(),
        ReviewClass::GuardMissing if operation == "unsafe_fn_call" => "Review the `unsafe_fn_call` callee contract manually and add obligation-specific guard evidence for this call.".to_string(),
        ReviewClass::GuardMissing if operation == "inline_asm" => "Review the `inline_asm` register, memory, and target invariants manually; add explicit guard evidence, and attach a human deep-review receipt only as witness evidence.".to_string(),
        ReviewClass::GuardMissing if operation == "pin_unchecked" => "Review the `pin_unchecked` move-prevention and projection invariants manually; add explicit guard evidence, and attach a human deep-review receipt only as witness evidence.".to_string(),
        ReviewClass::GuardMissing
            if operation != "unsafe_declaration"
                && contract_present
                && let Some(owner) = owner
                && !owner.is_empty()
                && enclosing_unsafe_fn_owner(context_before, owner) =>
        {
            format!(
                "Review the callers of `{owner}` against its documented safety contract; this site inherits the enclosing `unsafe fn` contract. Add a guard at this site only for obligations the caller contract cannot cover."
            )
        }
        ReviewClass::GuardMissing => {
            let missing_obligations: Vec<&ObligationEvidence> = obligation_evidence
                .iter()
                .filter(|ev| !ev.discharge.present)
                .collect();
            if missing_obligations.len() > 1 {
                let list = missing_obligations
                    .iter()
                    .enumerate()
                    .map(|(i, ev)| format!("({}) {}", i + 1, ev.obligation.description))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "Add or expose local guards for these `{operation}` safety obligations: {list}."
                )
            } else {
                format!(
                    "Add or expose the local guard that discharges the `{operation}` safety obligation."
                )
            }
        }
        ReviewClass::GuardedUnwitnessed if has_witness_route(routes, WitnessKind::HumanDeepReview) => {
            "Attach a human deep-review witness receipt or mark the static limitation explicitly."
                .to_string()
        }
        ReviewClass::GuardedUnwitnessed
            if has_witness_route(routes, WitnessKind::Miri)
                && has_witness_route(routes, WitnessKind::CargoCareful) =>
        {
            "Attach a focused Miri or cargo-careful witness receipt or mark the static limitation explicitly.".to_string()
        }
        ReviewClass::GuardedUnwitnessed if has_witness_route(routes, WitnessKind::Miri) => {
            "Attach a focused Miri witness receipt or mark the static limitation explicitly."
                .to_string()
        }
        ReviewClass::GuardedUnwitnessed if has_witness_route(routes, WitnessKind::CargoCareful) => {
            "Attach a focused cargo-careful witness receipt or mark the static limitation explicitly.".to_string()
        }
        ReviewClass::ReachableUnwitnessed => "Attach a focused witness receipt for the reached unsafe seam or mark the static limitation explicitly.".to_string(),
        ReviewClass::WitnessMismatch => "Review the witness identity or tool mismatch and attach a matching receipt for this card.".to_string(),
        ReviewClass::RequiresLoom => "Add or update a Loom/Shuttle model for the changed concurrency invariant.".to_string(),
        ReviewClass::RequiresSanitizer => "Run a focused sanitizer or cargo-careful witness and attach the receipt with limitations.".to_string(),
        ReviewClass::RequiresKaniOrCrux => "Run a bounded Kani/Crux proof harness or attach the receipt with limitations.".to_string(),
        ReviewClass::MiriUnsupported => "Use sanitizer/cargo-careful or an explicit FFI boundary contract; Miri may not exercise this seam.".to_string(),
        ReviewClass::StaticUnknown => "Review the unsafe site manually; identify the missing contract, guard, test, or witness route before claiming progress.".to_string(),
        ReviewClass::UnsafeUnreached => "Add or identify a focused test path that reaches the safe wrapper around this unsafe seam.".to_string(),
        ReviewClass::BaselineKnown => "Keep the baseline ledger owner and review date current.".to_string(),
        ReviewClass::Suppressed => "Suppressed card; keep the owner, reason, evidence, and review or expiry date current.".to_string(),
        _ => "Attach a focused witness receipt or mark the static limitation explicitly.".to_string(),
    }
}

fn has_witness_route(routes: &[WitnessRoute], kind: WitnessKind) -> bool {
    routes.iter().any(|route| route.kind == kind)
}

/// Whether the before-context contains the declaration of `owner` as an
/// `unsafe fn`. Comment lines are skipped so a prose mention of the owner
/// cannot fabricate an enclosing-contract relationship; only the routing of
/// the next action depends on this, never the evidence state or class.
fn enclosing_unsafe_fn_owner(context_before: &[String], owner: &str) -> bool {
    context_before.iter().any(|line| {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*') {
            return false;
        }
        let mut saw_unsafe = false;
        let mut saw_fn = false;
        let mut saw_owner = false;
        for token in trimmed.split(|ch: char| !ch.is_alphanumeric() && ch != '_') {
            match token {
                _ if token == "unsafe" => saw_unsafe = true,
                _ if token == "fn" => saw_fn = true,
                _ if token == owner => saw_owner = true,
                _ => {}
            }
        }
        saw_unsafe && saw_fn && saw_owner
    })
}
