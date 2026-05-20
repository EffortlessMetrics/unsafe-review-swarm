use super::{classify, evidence, obligations, receipts, scanner, witness};
use crate::api::{AnalysisMode, AnalyzeInput, AnalyzeOutput, DiffSource, Scope, Summary};
use crate::domain::{CardId, MissingEvidence, NextAction, Priority, ReviewCard, ReviewClass};
use crate::input::{diff, workspace};
use crate::policy::PolicyState;
use crate::util::{slug, stable_hash_hex};
use std::collections::BTreeMap;
use std::fs;

pub(crate) fn analyze(input: AnalyzeInput) -> Result<AnalyzeOutput, String> {
    analyze_with_receipts(input, true)
}

pub(crate) fn analyze_without_receipts(input: AnalyzeInput) -> Result<AnalyzeOutput, String> {
    analyze_with_receipts(input, false)
}

fn analyze_with_receipts(
    input: AnalyzeInput,
    import_receipts: bool,
) -> Result<AnalyzeOutput, String> {
    let repo_mode = matches!(input.scope, Scope::Repo) || matches!(input.mode, AnalysisMode::Repo);
    let diff_index = load_diff_index(&input.diff)?;
    let all_rust_files = workspace::discover_rust_files(&input.root)?;
    let package = package_name(&input.root);
    let policy_state = PolicyState::load(&input.root)?;
    let receipt_index = if import_receipts {
        receipts::ReceiptIndex::load(&input.root)?
    } else {
        receipts::ReceiptIndex::default()
    };
    let candidate_files = if repo_mode || diff_index.is_empty() {
        all_rust_files.clone()
    } else {
        all_rust_files
            .iter()
            .filter(|path| diff_index.contains_file(path))
            .cloned()
            .collect::<Vec<_>>()
    };

    let mut cards = Vec::new();
    let mut identity_counts = BTreeMap::new();
    let max_cards = input.max_cards.unwrap_or(usize::MAX);
    'files: for rel in &candidate_files {
        if cards.len() >= max_cards {
            break;
        }
        let scanned = scanner::scan_file(&input.root, rel, Some(&diff_index), repo_mode)?;
        for scanned_site in scanned {
            let hazards = obligations::hazards_for(&scanned_site.operation.family);
            let obligations = obligations::obligations_for(&scanned_site.operation.family);
            let contract = evidence::contract_evidence(&scanned_site);
            let (reach, related_tests) =
                evidence::reach_evidence(&input.root, scanned_site.site.owner.as_ref());
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
            let id = card_id(&package, &scanned_site, &hazards, &mut identity_counts);
            let witness_evidence = receipt_index
                .evidence_for(&id)
                .unwrap_or_else(crate::domain::WitnessEvidence::missing);
            if witness_evidence.present {
                for evidence in &mut obligation_evidence {
                    evidence.witness =
                        crate::domain::EvidenceState::present(&witness_evidence.summary);
                }
                if class == ReviewClass::GuardedUnwitnessed {
                    class = ReviewClass::GuardedAndWitnessed;
                    priority = Priority::Low;
                }
            }
            if policy_state.is_suppressed(&id) {
                class = ReviewClass::Suppressed;
                priority = Priority::Low;
            } else if policy_state.is_baseline_known(&id) {
                class = ReviewClass::BaselineKnown;
                priority = Priority::Low;
            }
            let next_action = NextAction {
                summary: next_action_summary(&class, scanned_site.operation.family.as_str()),
                verify_commands,
            };
            if !witness_evidence.present {
                missing.push(MissingEvidence::new(
                    "witness",
                    "No witness receipt imported for this card",
                ));
            }
            cards.push(ReviewCard {
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
            });
            if cards.len() >= max_cards {
                break 'files;
            }
        }
    }
    cards.sort_by(|left, right| {
        left.site
            .location
            .file
            .cmp(&right.site.location.file)
            .then(left.site.location.line.cmp(&right.site.location.line))
    });
    let summary = summarize(all_rust_files.len(), candidate_files.len(), &cards);
    Ok(AnalyzeOutput {
        schema_version: "0.1".to_string(),
        tool: "unsafe-review".to_string(),
        root: input.root,
        scope: input.scope,
        mode: input.mode,
        policy: input.policy,
        summary,
        cards,
    })
}

fn package_name(root: &std::path::Path) -> String {
    let Ok(text) = fs::read_to_string(root.join("Cargo.toml")) else {
        return root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("workspace")
            .to_string();
    };
    let mut in_package = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_package = trimmed == "[package]";
            continue;
        }
        if !in_package || !trimmed.starts_with("name") {
            continue;
        }
        let Some((_key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let name = value.trim().trim_matches('"');
        if !name.is_empty() {
            return name.to_string();
        }
    }
    root.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("workspace")
        .to_string()
}

fn load_diff_index(source: &DiffSource) -> Result<diff::DiffIndex, String> {
    match source {
        DiffSource::NoneRepoScan => Ok(diff::DiffIndex::default()),
        DiffSource::Text(text) => Ok(diff::parse_unified_diff(text)),
        DiffSource::File(path) => {
            let text = fs::read_to_string(path)
                .map_err(|err| format!("read diff {} failed: {err}", path.display()))?;
            Ok(diff::parse_unified_diff(&text))
        }
    }
}

fn summarize(rust_files: usize, changed_rust_files: usize, cards: &[ReviewCard]) -> Summary {
    let mut summary = Summary {
        rust_files,
        changed_rust_files,
        unsafe_sites: cards.len(),
        cards: cards.len(),
        ..Summary::default()
    };
    for card in cards {
        if card.class.is_actionable() {
            summary.open_actionable_gaps += 1;
        }
        match &card.class {
            crate::domain::ReviewClass::ContractMissing => summary.contract_missing += 1,
            crate::domain::ReviewClass::GuardMissing => summary.guard_missing += 1,
            crate::domain::ReviewClass::GuardedUnwitnessed => summary.guarded_unwitnessed += 1,
            crate::domain::ReviewClass::UnsafeUnreached => summary.unsafe_unreached += 1,
            crate::domain::ReviewClass::RequiresLoom => summary.requires_loom += 1,
            crate::domain::ReviewClass::MiriUnsupported => summary.miri_unsupported += 1,
            crate::domain::ReviewClass::StaticUnknown => summary.static_unknown += 1,
            _ => {}
        }
    }
    summary
}

fn next_action_summary(class: &crate::domain::ReviewClass, operation: &str) -> String {
    match class {
        crate::domain::ReviewClass::ContractMissing => "Add a precise `# Safety` section or `SAFETY:` / `Safety:` comment that names the required conditions.".to_string(),
        crate::domain::ReviewClass::GuardMissing => format!("Add or expose the local guard that discharges the `{operation}` safety obligation."),
        crate::domain::ReviewClass::RequiresLoom => "Add or update a Loom/Shuttle model for the changed concurrency invariant.".to_string(),
        crate::domain::ReviewClass::MiriUnsupported => "Use sanitizer/cargo-careful or an explicit FFI boundary contract; Miri may not exercise this seam.".to_string(),
        crate::domain::ReviewClass::UnsafeUnreached => "Add or identify a focused test path that reaches the safe wrapper around this unsafe seam.".to_string(),
        crate::domain::ReviewClass::BaselineKnown => "Known baseline card; keep the ledger owner and review date current.".to_string(),
        crate::domain::ReviewClass::Suppressed => "Suppressed card; keep the owner, reason, evidence, and review or expiry date current.".to_string(),
        _ => "Attach a focused witness receipt or mark the static limitation explicitly.".to_string(),
    }
}

fn card_id(
    package: &str,
    scanned: &scanner::ScannedSite,
    hazards: &[crate::domain::HazardKind],
    identity_counts: &mut BTreeMap<String, usize>,
) -> CardId {
    let base = card_identity_base(package, scanned, hazards);
    let next = identity_counts.entry(base.clone()).or_insert(0);
    *next += 1;
    CardId(format!("{base}-c{}", *next))
}

fn card_identity_base(
    package: &str,
    scanned: &scanner::ScannedSite,
    hazards: &[crate::domain::HazardKind],
) -> String {
    let file = scanned
        .site
        .location
        .file
        .to_string_lossy()
        .replace(['/', '\\'], "_");
    let owner = scanned
        .site
        .owner
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    let normalized = normalize_snippet(&scanned.operation.expression);
    let snippet_hash = stable_hash_hex(&normalized);
    let hazard = hazards.first().map_or("unknown", |hazard| hazard.as_str());
    format!(
        "UR-{}-{}-{}-{}-{}-{}-{}-{}",
        slug(package),
        slug(&file),
        slug(&owner),
        scanned.site.kind.as_str(),
        scanned.operation.family.as_str(),
        slug(&operation_path(scanned)),
        &snippet_hash[..12],
        hazard
    )
}

fn normalize_snippet(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn operation_path(scanned: &scanner::ScannedSite) -> String {
    if scanned.operation.family == crate::domain::OperationFamily::RawPointerDeref {
        return "deref".to_string();
    }
    if scanned.operation.family == crate::domain::OperationFamily::UnreachableUnchecked {
        return "unreachable_unchecked".to_string();
    }
    if scanned.operation.family == crate::domain::OperationFamily::UnsafeFnCall {
        return unsafe_call_path(&scanned.operation.expression);
    }
    if scanned.operation.family == crate::domain::OperationFamily::Unknown {
        return scanned
            .site
            .owner
            .clone()
            .unwrap_or_else(|| scanned.site.kind.as_str().to_string());
    }
    let normalized = normalize_snippet(&scanned.operation.expression);
    let target = normalized
        .split('(')
        .next()
        .unwrap_or(normalized.as_str())
        .trim();
    if let Some((_prefix, method)) = target.rsplit_once('.') {
        return method.trim_matches(':').to_string();
    }
    if let Some((_prefix, function)) = target.rsplit_once("::") {
        return function.trim_matches(':').to_string();
    }
    scanned.operation.family.as_str().to_string()
}

fn unsafe_call_path(expression: &str) -> String {
    let normalized = normalize_snippet(expression);
    if contains_call_name(&normalized, "new_unchecked") {
        return "new_unchecked".to_string();
    }
    let call = normalized
        .split_once("unsafe")
        .and_then(|(_prefix, after_unsafe)| {
            after_unsafe.split_once('{').map(|(_open, after)| after)
        })
        .unwrap_or(normalized.as_str())
        .split('(')
        .next()
        .unwrap_or("unsafe_fn_call")
        .trim()
        .trim_start_matches("match")
        .trim();
    let call = strip_trailing_turbofish(call);
    if call.is_empty() {
        "unsafe_fn_call".to_string()
    } else if let Some((_prefix, method)) = call.rsplit_once('.') {
        method.trim_matches(':').to_string()
    } else if let Some((_prefix, function)) = call.rsplit_once("::") {
        function.trim_matches(':').to_string()
    } else {
        call.trim_matches(':').to_string()
    }
}

fn strip_trailing_turbofish(call: &str) -> &str {
    let call = call.trim();
    if !call.ends_with('>') {
        return call;
    }

    let mut depth = 0usize;
    for (idx, ch) in call.char_indices().rev() {
        match ch {
            '>' => depth += 1,
            '<' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let prefix = &call[..idx];
                    if let Some(without_colons) = prefix.strip_suffix("::") {
                        return without_colons;
                    }
                    return call;
                }
            }
            _ => {}
        }
    }

    call
}

fn contains_call_name(line: &str, name: &str) -> bool {
    let mut cursor = line;
    while let Some(pos) = cursor.find(name) {
        let before = cursor[..pos].chars().next_back();
        let after = &cursor[pos + name.len()..];
        let starts_on_boundary = before.is_none_or(|ch| !is_ident_continue(ch));
        if starts_on_boundary && call_suffix(after) {
            return true;
        }
        cursor = &after[after
            .char_indices()
            .next()
            .map_or(after.len(), |(idx, ch)| idx + ch.len_utf8())..];
    }
    false
}

fn call_suffix(after_name: &str) -> bool {
    let rest = after_name.trim_start();
    if rest.starts_with('(') {
        return true;
    }
    rest.strip_prefix("::")
        .is_some_and(|after_colons| after_colons.trim_start().starts_with('<'))
}

fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{AnalysisMode, DiffSource, PolicyMode, Scope};
    use crate::domain::{
        HazardKind, OperationFamily, ReviewCard, ReviewClass, UnsafeSiteKind, WitnessKind,
    };
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn raw_pointer_v1_operation_cards_are_concrete() -> Result<(), String> {
        let cases = [
            (
                "raw_pointer_alignment",
                OperationFamily::RawPointerRead,
                true,
            ),
            ("raw_pointer_deref", OperationFamily::RawPointerDeref, true),
            (
                "raw_pointer_read_unaligned",
                OperationFamily::RawPointerReadUnaligned,
                false,
            ),
            (
                "raw_pointer_read_volatile",
                OperationFamily::RawPointerRead,
                true,
            ),
            (
                "raw_pointer_write_assignment",
                OperationFamily::RawPointerWrite,
                true,
            ),
            (
                "raw_pointer_write_unaligned",
                OperationFamily::RawPointerWriteUnaligned,
                false,
            ),
            (
                "raw_pointer_write_bytes",
                OperationFamily::RawPointerWrite,
                true,
            ),
            (
                "raw_pointer_write_other_u8_not_guard",
                OperationFamily::RawPointerWrite,
                true,
            ),
            (
                "raw_pointer_write_maybeuninit",
                OperationFamily::RawPointerWrite,
                true,
            ),
            (
                "raw_pointer_write_other_maybeuninit_not_guard",
                OperationFamily::RawPointerWrite,
                true,
            ),
            (
                "raw_pointer_write_volatile",
                OperationFamily::RawPointerWrite,
                true,
            ),
            (
                "split_raw_pointer_read_call",
                OperationFamily::RawPointerRead,
                true,
            ),
        ];

        for (fixture, expected_family, expects_alignment) in cases {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(
                card.operation.family, expected_family,
                "{fixture} should emit the concrete operation family"
            );
            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert_eq!(
                card.hazards.contains(&HazardKind::Alignment),
                expects_alignment,
                "{fixture} alignment hazard expectation drifted"
            );
            assert!(card.contract.present);
            assert_eq!(card.reach.state, "owner_reached");
            assert!(card.missing.iter().any(|missing| missing.kind == "guard"));
            assert!(
                card.next_action
                    .verify_commands
                    .iter()
                    .any(|command| command.contains("miri test")),
                "{fixture} should recommend a concrete Miri witness"
            );
            assert_no_unknown_wrapper_card(fixture, &output);
        }
        Ok(())
    }

    #[test]
    fn unaligned_raw_pointer_read_does_not_require_alignment_guard() -> Result<(), String> {
        let output = fixture_output("raw_pointer_read_unaligned")?;
        let card = single_card("raw_pointer_read_unaligned", &output)?;

        assert_eq!(
            card.operation.family,
            OperationFamily::RawPointerReadUnaligned
        );
        assert!(!card.hazards.contains(&HazardKind::Alignment));
        assert!(
            card.obligations
                .iter()
                .all(|obligation| obligation.key != "alignment")
        );
        assert!(card.contract.present);
        assert!(
            obligation_discharge_present(card, "bounds"),
            "length evidence should still satisfy the bounds obligation"
        );
        assert_eq!(card.class, ReviewClass::GuardMissing);
        Ok(())
    }

    #[test]
    fn unaligned_raw_pointer_write_does_not_require_alignment_guard() -> Result<(), String> {
        let output = fixture_output("raw_pointer_write_unaligned")?;
        let card = single_card("raw_pointer_write_unaligned", &output)?;

        assert_eq!(
            card.operation.family,
            OperationFamily::RawPointerWriteUnaligned
        );
        assert!(!card.hazards.contains(&HazardKind::Alignment));
        assert!(
            card.obligations
                .iter()
                .all(|obligation| obligation.key != "alignment")
        );
        assert!(card.contract.present);
        assert!(
            obligation_discharge_present(card, "bounds"),
            "length evidence should still satisfy the bounds obligation"
        );
        assert_eq!(card.class, ReviewClass::GuardMissing);
        Ok(())
    }

    #[test]
    fn raw_pointer_write_maybeuninit_evidence_requires_target_context() -> Result<(), String> {
        let output = fixture_output("raw_pointer_write_other_maybeuninit_not_guard")?;
        let card = single_card("raw_pointer_write_other_maybeuninit_not_guard", &output)?;

        assert_eq!(card.operation.family, OperationFamily::RawPointerWrite);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!obligation_discharge_present(card, "bounds"));
        assert!(!obligation_discharge_present(card, "initialized"));
        Ok(())
    }

    #[test]
    fn raw_pointer_bounds_evidence_rejects_bare_observations() -> Result<(), String> {
        for fixture in [
            "raw_pointer_bounds_observed_not_guard",
            "raw_pointer_bounds_closed_branch_not_guard",
            "raw_pointer_bounds_post_check_not_guard",
            "raw_pointer_read_len_capacity_other_values_not_guard",
            "raw_pointer_read_len_capacity_observed_not_guard",
            "raw_pointer_read_len_capacity_closed_branch_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.operation.family, OperationFamily::RawPointerRead);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(
                !obligation_discharge_present(card, "bounds"),
                "{fixture} should not discharge bounds"
            );
            assert!(
                !obligation_discharge_present(card, "alignment"),
                "{fixture} should not discharge alignment"
            );
        }
        Ok(())
    }

    #[test]
    fn raw_pointer_read_len_capacity_bounds_evidence_requires_same_source_assertion()
    -> Result<(), String> {
        let output = fixture_output("raw_pointer_read_len_capacity_assert")?;
        let card = single_card("raw_pointer_read_len_capacity_assert", &output)?;

        assert_eq!(card.operation.family, OperationFamily::RawPointerRead);
        assert!(
            obligation_discharge_present(card, "bounds"),
            "same-source len/capacity assertion should discharge bounds"
        );
        assert!(
            !obligation_discharge_present(card, "alignment"),
            "len/capacity assertion must not discharge alignment"
        );
        Ok(())
    }

    #[test]
    fn raw_pointer_write_bounds_evidence_rejects_bare_observations() -> Result<(), String> {
        for fixture in [
            "raw_pointer_write_bounds_observed_not_guard",
            "raw_pointer_write_bounds_closed_branch_not_guard",
            "raw_pointer_write_bounds_post_check_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.operation.family, OperationFamily::RawPointerWrite);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(
                !obligation_discharge_present(card, "bounds"),
                "{fixture} should not discharge bounds"
            );
            assert!(
                !obligation_discharge_present(card, "alignment"),
                "{fixture} should not discharge alignment"
            );
        }
        Ok(())
    }

    #[test]
    fn raw_pointer_write_alignment_guard_is_receiver_sensitive() -> Result<(), String> {
        let guarded = fixture_output("raw_pointer_write_alignment_guard")?;
        let guarded_card = single_card("raw_pointer_write_alignment_guard", &guarded)?;

        assert_eq!(
            guarded_card.operation.family,
            OperationFamily::RawPointerWrite
        );
        assert_eq!(guarded_card.class, ReviewClass::GuardMissing);
        assert!(
            obligation_discharge_present(guarded_card, "bounds"),
            "length guard should discharge bounds"
        );
        assert!(
            obligation_discharge_present(guarded_card, "alignment"),
            "same-receiver alignment guard should discharge alignment"
        );

        for fixture in [
            "raw_pointer_write_alignment_observed_not_guard",
            "raw_pointer_write_alignment_closed_branch_not_guard",
            "raw_pointer_write_alignment_post_check_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.operation.family, OperationFamily::RawPointerWrite);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(
                obligation_discharge_present(card, "bounds"),
                "{fixture} should keep bounds evidence"
            );
            assert!(
                !obligation_discharge_present(card, "alignment"),
                "{fixture} should not discharge alignment"
            );
        }
        Ok(())
    }

    #[test]
    fn raw_pointer_write_nullability_guard_is_receiver_sensitive() -> Result<(), String> {
        let guarded = fixture_output("raw_pointer_write_null_guard")?;
        let guarded_card = single_card("raw_pointer_write_null_guard", &guarded)?;

        assert_eq!(
            guarded_card.operation.family,
            OperationFamily::RawPointerWrite
        );
        assert_eq!(guarded_card.class, ReviewClass::GuardMissing);
        assert!(
            obligation_discharge_present(guarded_card, "pointer-live"),
            "same-receiver null guard should discharge pointer-live"
        );
        assert!(
            obligation_discharge_present(guarded_card, "bounds"),
            "length guard should discharge bounds"
        );
        assert!(
            obligation_discharge_present(guarded_card, "alignment"),
            "alignment guard should discharge alignment"
        );

        for fixture in [
            "raw_pointer_write_null_observed_not_guard",
            "raw_pointer_write_null_other_pointer_not_guard",
            "raw_pointer_write_null_post_check_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.operation.family, OperationFamily::RawPointerWrite);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(
                !obligation_discharge_present(card, "pointer-live"),
                "{fixture} should not discharge pointer-live"
            );
            assert!(
                obligation_discharge_present(card, "bounds"),
                "{fixture} should keep bounds evidence"
            );
            assert!(
                obligation_discharge_present(card, "alignment"),
                "{fixture} should keep alignment evidence"
            );
        }
        Ok(())
    }

    #[test]
    fn raw_pointer_read_nullability_guard_is_receiver_sensitive() -> Result<(), String> {
        let guarded = fixture_output("raw_pointer_read_null_guard")?;
        let guarded_card = single_card("raw_pointer_read_null_guard", &guarded)?;

        assert_eq!(
            guarded_card.operation.family,
            OperationFamily::RawPointerRead
        );
        assert_eq!(guarded_card.class, ReviewClass::GuardMissing);
        assert!(
            obligation_discharge_present(guarded_card, "pointer-live"),
            "same-receiver null guard should discharge pointer-live"
        );
        assert!(
            obligation_discharge_present(guarded_card, "bounds"),
            "length guard should discharge bounds"
        );
        assert!(
            obligation_discharge_present(guarded_card, "alignment"),
            "alignment guard should discharge alignment"
        );

        for fixture in [
            "raw_pointer_read_null_observed_not_guard",
            "raw_pointer_read_null_other_pointer_not_guard",
            "raw_pointer_read_null_post_check_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.operation.family, OperationFamily::RawPointerRead);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(
                !obligation_discharge_present(card, "pointer-live"),
                "{fixture} should not discharge pointer-live"
            );
            assert!(
                obligation_discharge_present(card, "bounds"),
                "{fixture} should keep bounds evidence"
            );
            assert!(
                obligation_discharge_present(card, "alignment"),
                "{fixture} should keep alignment evidence"
            );
        }
        Ok(())
    }

    #[test]
    fn raw_pointer_unaligned_nullability_guard_is_receiver_sensitive() -> Result<(), String> {
        for (fixture, family) in [
            (
                "raw_pointer_read_unaligned_null_guard",
                OperationFamily::RawPointerReadUnaligned,
            ),
            (
                "raw_pointer_write_unaligned_null_guard",
                OperationFamily::RawPointerWriteUnaligned,
            ),
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.operation.family, family);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(
                obligation_discharge_present(card, "pointer-live"),
                "{fixture} should discharge pointer-live"
            );
            assert!(
                obligation_discharge_present(card, "bounds"),
                "{fixture} should keep bounds evidence"
            );
            assert!(
                card.obligation_evidence
                    .iter()
                    .all(|evidence| evidence.obligation.key != "alignment"),
                "{fixture} should not require alignment evidence"
            );
        }

        for (fixture, family) in [
            (
                "raw_pointer_read_unaligned_null_observed_not_guard",
                OperationFamily::RawPointerReadUnaligned,
            ),
            (
                "raw_pointer_read_unaligned_null_other_pointer_not_guard",
                OperationFamily::RawPointerReadUnaligned,
            ),
            (
                "raw_pointer_read_unaligned_null_post_check_not_guard",
                OperationFamily::RawPointerReadUnaligned,
            ),
            (
                "raw_pointer_write_unaligned_null_observed_not_guard",
                OperationFamily::RawPointerWriteUnaligned,
            ),
            (
                "raw_pointer_write_unaligned_null_other_pointer_not_guard",
                OperationFamily::RawPointerWriteUnaligned,
            ),
            (
                "raw_pointer_write_unaligned_null_post_check_not_guard",
                OperationFamily::RawPointerWriteUnaligned,
            ),
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.operation.family, family);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(
                !obligation_discharge_present(card, "pointer-live"),
                "{fixture} should not discharge pointer-live"
            );
            assert!(
                obligation_discharge_present(card, "bounds"),
                "{fixture} should keep bounds evidence"
            );
            assert!(
                card.obligation_evidence
                    .iter()
                    .all(|evidence| evidence.obligation.key != "alignment"),
                "{fixture} should not require alignment evidence"
            );
        }
        Ok(())
    }

    #[test]
    fn raw_pointer_volatile_nullability_guard_is_receiver_sensitive() -> Result<(), String> {
        for (fixture, family) in [
            (
                "raw_pointer_read_volatile_null_guard",
                OperationFamily::RawPointerRead,
            ),
            (
                "raw_pointer_write_volatile_null_guard",
                OperationFamily::RawPointerWrite,
            ),
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.operation.family, family);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(
                obligation_discharge_present(card, "pointer-live"),
                "{fixture} should discharge pointer-live"
            );
            assert!(
                !obligation_discharge_present(card, "alignment"),
                "{fixture} should still require alignment evidence"
            );
        }

        for (fixture, family) in [
            (
                "raw_pointer_read_volatile_null_observed_not_guard",
                OperationFamily::RawPointerRead,
            ),
            (
                "raw_pointer_read_volatile_null_other_pointer_not_guard",
                OperationFamily::RawPointerRead,
            ),
            (
                "raw_pointer_read_volatile_null_post_check_not_guard",
                OperationFamily::RawPointerRead,
            ),
            (
                "raw_pointer_write_volatile_null_observed_not_guard",
                OperationFamily::RawPointerWrite,
            ),
            (
                "raw_pointer_write_volatile_null_other_pointer_not_guard",
                OperationFamily::RawPointerWrite,
            ),
            (
                "raw_pointer_write_volatile_null_post_check_not_guard",
                OperationFamily::RawPointerWrite,
            ),
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.operation.family, family);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(
                !obligation_discharge_present(card, "pointer-live"),
                "{fixture} should not discharge pointer-live"
            );
            assert!(
                !obligation_discharge_present(card, "alignment"),
                "{fixture} should still require alignment evidence"
            );
        }
        Ok(())
    }

    #[test]
    fn raw_pointer_volatile_alignment_guard_is_receiver_sensitive() -> Result<(), String> {
        for (fixture, family) in [
            (
                "raw_pointer_read_volatile_alignment_guard",
                OperationFamily::RawPointerRead,
            ),
            (
                "raw_pointer_write_volatile_alignment_guard",
                OperationFamily::RawPointerWrite,
            ),
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.operation.family, family);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(
                obligation_discharge_present(card, "alignment"),
                "{fixture} should discharge alignment"
            );
            assert!(
                !obligation_discharge_present(card, "pointer-live"),
                "{fixture} should still require pointer-live evidence"
            );
        }

        for (fixture, family) in [
            (
                "raw_pointer_read_volatile_alignment_observed_not_guard",
                OperationFamily::RawPointerRead,
            ),
            (
                "raw_pointer_read_volatile_alignment_other_pointer_not_guard",
                OperationFamily::RawPointerRead,
            ),
            (
                "raw_pointer_read_volatile_alignment_post_check_not_guard",
                OperationFamily::RawPointerRead,
            ),
            (
                "raw_pointer_write_volatile_alignment_observed_not_guard",
                OperationFamily::RawPointerWrite,
            ),
            (
                "raw_pointer_write_volatile_alignment_other_pointer_not_guard",
                OperationFamily::RawPointerWrite,
            ),
            (
                "raw_pointer_write_volatile_alignment_post_check_not_guard",
                OperationFamily::RawPointerWrite,
            ),
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.operation.family, family);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(
                !obligation_discharge_present(card, "alignment"),
                "{fixture} should still require alignment evidence"
            );
            assert!(
                !obligation_discharge_present(card, "pointer-live"),
                "{fixture} should still require pointer-live evidence"
            );
        }
        Ok(())
    }

    #[test]
    fn raw_pointer_write_u8_evidence_requires_target_pointer() -> Result<(), String> {
        let output = fixture_output("raw_pointer_write_other_u8_not_guard")?;
        let card = single_card("raw_pointer_write_other_u8_not_guard", &output)?;

        assert_eq!(card.operation.family, OperationFamily::RawPointerWrite);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!obligation_discharge_present(card, "initialized"));
        assert!(!obligation_discharge_present(card, "alignment"));
        Ok(())
    }

    #[test]
    fn raw_pointer_v1_evidence_stays_obligation_specific() -> Result<(), String> {
        for fixture in [
            "raw_pointer_alignment",
            "align_of_only_not_guard",
            "alignment_other_pointer_not_guard",
            "raw_pointer_alignment_post_check_not_guard",
            "raw_pointer_alignment_observed_not_guard",
            "raw_pointer_alignment_closed_branch_not_guard",
            "raw_pointer_alignment_reassigned_pointer_not_guard",
            "raw_pointer_alignment_modulo_observed_not_guard",
            "raw_pointer_alignment_modulo_closed_branch_not_guard",
            "raw_pointer_alignment_modulo_reassigned_pointer_not_guard",
            "comment_alignment_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert!(
                obligation_discharge_present(card, "bounds"),
                "{fixture} should keep length or bounds evidence"
            );
            assert!(
                !obligation_discharge_present(card, "alignment"),
                "{fixture} should not let comments or length checks discharge alignment"
            );
            assert!(!card.discharge.present);
        }
        Ok(())
    }

    #[test]
    fn raw_pointer_alignment_evidence_accepts_enforced_same_pointer_guard() -> Result<(), String> {
        for fixture in [
            "raw_pointer_alignment_is_aligned_guard",
            "raw_pointer_alignment_modulo_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.operation.family, OperationFamily::RawPointerRead);
            assert!(obligation_discharge_present(card, "bounds"));
            assert!(obligation_discharge_present(card, "alignment"));
            assert!(!obligation_discharge_present(card, "pointer-live"));
            assert!(!obligation_discharge_present(card, "initialized"));
            assert!(!obligation_discharge_present(card, "allocation"));
        }
        Ok(())
    }

    #[test]
    fn raw_pointer_v1_negative_cases_stay_pinned() -> Result<(), String> {
        let safe_reference = fixture_output("safe_reference_deref_no_cards")?;
        assert_eq!(safe_reference.summary.cards, 0);
        assert!(safe_reference.cards.is_empty());

        let split_unknown = fixture_output("split_unsafe_block")?;
        let card = single_card("split_unsafe_block", &split_unknown)?;
        assert_eq!(card.site.kind, UnsafeSiteKind::UnsafeBlock);
        assert_eq!(card.operation.family, OperationFamily::Unknown);
        assert_eq!(card.class, ReviewClass::ContractMissing);
        Ok(())
    }

    #[test]
    fn capped_repo_scan_prefers_source_roots_before_miscellaneous_rust_files() -> Result<(), String>
    {
        let root = unique_temp_dir("unsafe-review-capped-repo")?;
        fs::create_dir_all(root.join("benchmarks/haystacks/code"))
            .map_err(|err| format!("create benchmark dirs failed: {err}"))?;
        fs::create_dir_all(root.join("src")).map_err(|err| format!("create src failed: {err}"))?;
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"capped-repo-fixture\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
        )
        .map_err(|err| format!("write Cargo.toml failed: {err}"))?;
        fs::write(
            root.join("benchmarks/haystacks/code/rust-library.rs"),
            "pub unsafe fn fixture_data() {}\n",
        )
        .map_err(|err| format!("write benchmark file failed: {err}"))?;
        fs::write(root.join("src/lib.rs"), "pub unsafe fn source_root() {}\n")
            .map_err(|err| format!("write src file failed: {err}"))?;

        let output = analyze(AnalyzeInput {
            root: root.clone(),
            scope: Scope::Repo,
            diff: DiffSource::NoneRepoScan,
            mode: AnalysisMode::Repo,
            policy: PolicyMode::Advisory,
            include_unchanged_tests: true,
            max_cards: Some(1),
        })?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert_eq!(output.cards.len(), 1);
        assert_eq!(output.summary.cards, 1);
        assert_eq!(
            output.cards[0].site.location.file,
            PathBuf::from("src/lib.rs")
        );
        assert_eq!(output.cards[0].site.owner, Some("source_root".to_string()));
        Ok(())
    }

    #[test]
    fn operation_cards_inside_unsafe_fn_inherit_owner_safety_docs() -> Result<(), String> {
        let root = unique_temp_dir("unsafe-review-owner-contract")?;
        fs::create_dir_all(root.join("src")).map_err(|err| format!("create src failed: {err}"))?;
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"owner-contract-fixture\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
        )
        .map_err(|err| format!("write Cargo.toml failed: {err}"))?;
        fs::write(
            root.join("src/lib.rs"),
            r#"/// Advances a raw pointer.
///
/// # Safety
///
/// Caller must ensure `ptr.add(offset)` remains within the same allocation.
pub unsafe fn advance(ptr: *const u8, offset: usize) -> *const u8 {
    let _padding0 = offset;
    let _padding1 = offset;
    let _padding2 = offset;
    let _padding3 = offset;
    let _padding4 = offset;
    let _padding5 = offset;
    let _padding6 = offset;
    unsafe { ptr.add(offset) }
}
"#,
        )
        .map_err(|err| format!("write src file failed: {err}"))?;

        let output = analyze(AnalyzeInput {
            root: root.clone(),
            scope: Scope::Repo,
            diff: DiffSource::NoneRepoScan,
            mode: AnalysisMode::Repo,
            policy: PolicyMode::Advisory,
            include_unchanged_tests: true,
            max_cards: None,
        })?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let Some(card) = output
            .cards
            .iter()
            .find(|card| card.operation.family == OperationFamily::PointerArithmetic)
        else {
            return Err(format!(
                "expected pointer arithmetic card: {:#?}",
                output.cards
            ));
        };
        assert_eq!(card.site.owner, Some("advance".to_string()));
        assert!(
            card.contract.present,
            "operation card should inherit enclosing unsafe fn # Safety docs"
        );
        Ok(())
    }

    #[test]
    fn public_unsafe_api_contract_evidence_requires_safety_docs() -> Result<(), String> {
        for fixture in [
            "public_unsafe_fn_missing_safety",
            "public_unsafe_trait_missing_safety",
            "public_unsafe_fn_safety_comment_not_docs",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.class, ReviewClass::ContractMissing);
            assert!(card.site.public_api_surface);
            assert!(!card.contract.present);
            assert!(
                card.contract.summary.contains("# Safety"),
                "{fixture} should ask for public safety docs"
            );
            assert!(
                card.missing.iter().any(|missing| missing.kind == "contract"
                    && missing.message.contains("public `# Safety`")),
                "{fixture} should not accept local SAFETY prose as public API docs"
            );
            assert!(
                card.site.owner.is_some(),
                "{fixture} should preserve the public API owner in the card"
            );
        }
        Ok(())
    }

    #[test]
    fn documented_public_unsafe_api_does_not_require_local_guard() -> Result<(), String> {
        for fixture in [
            "public_unsafe_fn_with_safety_docs",
            "public_unsafe_fn_safety_colon_docs",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.class, ReviewClass::GuardedUnwitnessed);
            assert!(card.site.public_api_surface);
            assert!(card.contract.present);
            assert!(card.discharge.present);
            assert!(
                !card.missing.iter().any(|missing| missing.kind == "guard"),
                "{fixture} should not ask for local declaration guard evidence"
            );
            assert!(
                card.missing.iter().any(|missing| missing.kind == "witness"),
                "{fixture} should still preserve witness prompts"
            );
        }
        Ok(())
    }

    #[test]
    fn documented_private_unsafe_fn_does_not_require_local_guard() -> Result<(), String> {
        let output = fixture_output("documented_private_unsafe_fn")?;
        let card = single_card("documented_private_unsafe_fn", &output)?;

        assert_eq!(card.class, ReviewClass::GuardedUnwitnessed);
        assert!(!card.site.public_api_surface);
        assert!(card.contract.present);
        assert!(card.discharge.present);
        assert!(
            !card.missing.iter().any(|missing| missing.kind == "guard"),
            "documented private unsafe declarations should not ask for local declaration guard evidence"
        );
        assert!(
            card.missing.iter().any(|missing| missing.kind == "witness"),
            "documented private unsafe declarations should still preserve witness prompts"
        );
        Ok(())
    }

    #[test]
    fn unsafe_call_wrapper_uses_concrete_operation_family() -> Result<(), String> {
        let output = fixture_output("unsafe_fn_call_wrapper")?;
        let card = single_card("unsafe_fn_call_wrapper", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::UnsafeFnCall);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.id.0.contains("encode-utf8"));
        Ok(())
    }

    #[test]
    fn multiline_unsafe_call_wrapper_uses_concrete_operation_family() -> Result<(), String> {
        let output = fixture_output("multiline_unsafe_fn_call_wrapper")?;
        let card = single_card("multiline_unsafe_fn_call_wrapper", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::UnsafeFnCall);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.id.0.contains("encode-utf8"));
        Ok(())
    }

    #[test]
    fn unsafe_call_path_prefers_inner_unchecked_constructor_callee() {
        assert_eq!(
            unsafe_call_path("unsafe { Some(One::new_unchecked(needle)) }"),
            "new_unchecked"
        );
        assert_eq!(
            unsafe_call_path("unsafe { Some(One::new_unchecked::<Needle>(needle)) }"),
            "new_unchecked"
        );
        assert_eq!(
            unsafe_call_path("unsafe { self.reserve_rehash(hasher) }"),
            "reserve_rehash"
        );
    }

    #[test]
    fn unsafe_call_path_strips_trailing_turbofish_from_callee() {
        assert_eq!(
            unsafe_call_path("unsafe { crate::ffi::call_unchecked::<Header>(ptr) }"),
            "call_unchecked"
        );
        assert_eq!(
            unsafe_call_path("unsafe { self.call_unchecked::<Header>(ptr) }"),
            "call_unchecked"
        );
        assert_eq!(
            unsafe_call_path("unsafe { call_unchecked::<Header>(ptr) }"),
            "call_unchecked"
        );
        assert_eq!(
            unsafe_call_path("unsafe { crate::ffi::call_unchecked::<Option<Header>>(ptr) }"),
            "call_unchecked"
        );
    }

    #[test]
    fn unchecked_constructor_availability_guard_is_unsafe_call_evidence() -> Result<(), String> {
        for fixture in [
            "unchecked_constructor_availability_guard",
            "unchecked_constructor_availability_assert_guard",
            "unchecked_constructor_unavailable_return_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::UnsafeFnCall);
            assert_eq!(card.class, ReviewClass::GuardedUnwitnessed);
            assert!(card.discharge.present);
            assert!(obligation_discharge_present(card, "callee-contract"));
            assert!(card.site.snippet.contains("new_unchecked"));
            assert!(card.id.0.contains("new-unchecked"));
            assert!(!card.id.0.contains("-some-"));
        }
        Ok(())
    }

    #[test]
    fn unchecked_constructor_availability_guard_requires_same_receiver() -> Result<(), String> {
        for fixture in [
            "unchecked_constructor_other_availability_not_guard",
            "unchecked_constructor_availability_observed_not_guard",
            "unchecked_constructor_availability_closed_branch_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::UnsafeFnCall);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(!card.discharge.present);
            assert!(!obligation_discharge_present(card, "callee-contract"));
            assert!(
                card.missing.iter().any(|missing| missing.kind == "guard"),
                "{fixture} must not resolve this card's guard prompt"
            );
            assert!(card.site.snippet.contains("One::new_unchecked"));
        }
        Ok(())
    }

    #[test]
    fn nonnull_new_guard_discharges_non_null_obligation() -> Result<(), String> {
        let output = fixture_output("nonnull_new_guard")?;
        let card = single_card("nonnull_new_guard", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::NonNullUnchecked);
        assert_eq!(card.class, ReviewClass::GuardedUnwitnessed);
        assert!(card.discharge.present);
        assert!(obligation_discharge_present(card, "non-null"));
        assert!(
            card.missing.iter().all(|missing| missing.kind != "guard"),
            "NonNull::new guard evidence should resolve the local guard prompt"
        );
        assert!(card.id.0.contains("nonnull-unchecked"));
        Ok(())
    }

    #[test]
    fn nonnull_new_guard_for_other_pointer_is_not_evidence() -> Result<(), String> {
        let output = fixture_output("nonnull_other_guard_not_evidence")?;
        let card = single_card("nonnull_other_guard_not_evidence", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::NonNullUnchecked);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!card.discharge.present);
        assert!(!obligation_discharge_present(card, "non-null"));
        assert!(
            card.missing.iter().any(|missing| missing.kind == "guard"),
            "checking a different pointer must not resolve this card's guard prompt"
        );
        assert!(card.id.0.contains("nonnull-unchecked"));
        Ok(())
    }

    #[test]
    fn nonnull_is_null_branch_must_exit_to_count_as_guard() -> Result<(), String> {
        let output = fixture_output("nonnull_is_null_nonreturning_not_guard")?;
        let card = single_card("nonnull_is_null_nonreturning_not_guard", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::NonNullUnchecked);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!card.discharge.present);
        assert!(!obligation_discharge_present(card, "non-null"));
        assert!(
            card.missing.iter().any(|missing| missing.kind == "guard"),
            "observing null without exiting must not resolve this card's guard prompt"
        );
        assert!(card.id.0.contains("nonnull-unchecked"));
        Ok(())
    }

    #[test]
    fn nonnull_new_observation_is_not_guard_evidence() -> Result<(), String> {
        let output = fixture_output("nonnull_observed_not_guard")?;
        let card = single_card("nonnull_observed_not_guard", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::NonNullUnchecked);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!card.discharge.present);
        assert!(!obligation_discharge_present(card, "non-null"));
        assert!(
            card.missing.iter().any(|missing| missing.kind == "guard"),
            "observing NonNull::new without checking the result must not resolve this card's guard prompt"
        );
        assert!(card.id.0.contains("nonnull-unchecked"));
        Ok(())
    }

    #[test]
    fn nonnull_post_check_is_not_guard_evidence() -> Result<(), String> {
        let output = fixture_output("nonnull_post_check_not_guard")?;
        let card = single_card("nonnull_post_check_not_guard", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::NonNullUnchecked);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!card.discharge.present);
        assert!(!obligation_discharge_present(card, "non-null"));
        assert!(
            card.missing.iter().any(|missing| missing.kind == "guard"),
            "checking the pointer after NonNull::new_unchecked must not resolve this card's guard prompt"
        );
        assert!(card.id.0.contains("new-unchecked"));
        Ok(())
    }

    #[test]
    fn nested_unsafe_operation_does_not_emit_parent_duplicate() -> Result<(), String> {
        let output = fixture_output("nested_unsafe_operation_call_dedupe")?;
        let card = single_card("nested_unsafe_operation_call_dedupe", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::NonNullUnchecked);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!obligation_discharge_present(card, "non-null"));
        assert!(card.id.0.contains("nonnull-unchecked"));
        Ok(())
    }

    #[test]
    fn get_unchecked_mut_uses_bounds_operation_family() -> Result<(), String> {
        let output = fixture_output("get_unchecked_mut_bounds")?;
        let card = single_card("get_unchecked_mut_bounds", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::GetUnchecked);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.hazards.contains(&HazardKind::Bounds));
        assert!(!obligation_discharge_present(card, "bounds"));
        assert!(card.id.0.contains("get-unchecked-mut"));
        Ok(())
    }

    #[test]
    fn get_unchecked_mut_bounds_evidence_requires_same_receiver() -> Result<(), String> {
        let guarded = fixture_output("get_unchecked_mut_len_guard")?;
        let guarded_card = single_card("get_unchecked_mut_len_guard", &guarded)?;
        assert_eq!(guarded_card.operation.family, OperationFamily::GetUnchecked);
        assert_eq!(guarded_card.class, ReviewClass::GuardedUnwitnessed);
        assert!(obligation_discharge_present(guarded_card, "bounds"));

        let other_guard = fixture_output("get_unchecked_mut_other_len_not_guard")?;
        let other_guard_card = single_card("get_unchecked_mut_other_len_not_guard", &other_guard)?;
        assert_eq!(
            other_guard_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(other_guard_card.class, ReviewClass::GuardMissing);
        assert!(!obligation_discharge_present(other_guard_card, "bounds"));

        let post_check = fixture_output("get_unchecked_mut_post_check_not_guard")?;
        let post_check_card = single_card("get_unchecked_mut_post_check_not_guard", &post_check)?;
        assert_eq!(
            post_check_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(post_check_card.class, ReviewClass::GuardMissing);
        assert!(!obligation_discharge_present(post_check_card, "bounds"));

        let observed_bounds = fixture_output("get_unchecked_mut_bounds_observed_not_guard")?;
        let observed_bounds_card = single_card(
            "get_unchecked_mut_bounds_observed_not_guard",
            &observed_bounds,
        )?;
        assert_eq!(
            observed_bounds_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(observed_bounds_card.class, ReviewClass::GuardMissing);
        assert!(!obligation_discharge_present(
            observed_bounds_card,
            "bounds"
        ));

        let closed_bounds = fixture_output("get_unchecked_mut_closed_bounds_not_guard")?;
        let closed_bounds_card =
            single_card("get_unchecked_mut_closed_bounds_not_guard", &closed_bounds)?;
        assert_eq!(
            closed_bounds_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(closed_bounds_card.class, ReviewClass::GuardMissing);
        assert!(!obligation_discharge_present(closed_bounds_card, "bounds"));

        let reassigned_index = fixture_output("get_unchecked_mut_reassigned_index_not_guard")?;
        let reassigned_index_card = single_card(
            "get_unchecked_mut_reassigned_index_not_guard",
            &reassigned_index,
        )?;
        assert_eq!(
            reassigned_index_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(reassigned_index_card.class, ReviewClass::GuardMissing);
        assert!(!obligation_discharge_present(
            reassigned_index_card,
            "bounds"
        ));
        Ok(())
    }

    #[test]
    fn pin_new_unchecked_uses_pin_operation_family() -> Result<(), String> {
        let output = fixture_output("pin_new_unchecked")?;
        let card = single_card("pin_new_unchecked", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::PinUnchecked);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.hazards.contains(&HazardKind::PinInvariant));
        assert!(!obligation_discharge_present(card, "pin"));
        assert!(
            card.next_action.verify_commands.is_empty(),
            "Pin invariant review should not invent a generic witness command"
        );
        assert!(card.id.0.contains("new-unchecked"));
        Ok(())
    }

    #[test]
    fn adjacent_unchanged_unsafe_fn_is_not_reported_by_neighboring_change() -> Result<(), String> {
        let output = fixture_output("adjacent_unchanged_unsafe_fn_no_card")?;

        assert!(
            output.cards.is_empty(),
            "neighboring safe-code changes should not report an unchanged unsafe declaration"
        );
        Ok(())
    }

    #[test]
    fn slice_from_raw_parts_mut_uses_slice_operation_family() -> Result<(), String> {
        let output = fixture_output("slice_from_raw_parts_mut")?;
        let card = single_card("slice_from_raw_parts_mut", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::SliceFromRawParts);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.id.0.contains("from-raw-parts-mut"));
        Ok(())
    }

    #[test]
    fn slice_from_raw_parts_mut_maybeuninit_evidence_requires_slice_context() -> Result<(), String>
    {
        let output = fixture_output("slice_from_raw_parts_mut_other_maybeuninit_not_guard")?;
        let card = single_card(
            "slice_from_raw_parts_mut_other_maybeuninit_not_guard",
            &output,
        )?;

        assert_eq!(card.operation.family, OperationFamily::SliceFromRawParts);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!obligation_discharge_present(card, "initialized"));
        Ok(())
    }

    #[test]
    fn vec_from_raw_parts_uses_vec_operation_family() -> Result<(), String> {
        let output = fixture_output("vec_from_raw_parts")?;
        let card = single_card("vec_from_raw_parts", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::VecFromRawParts);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.hazards.contains(&HazardKind::DropOrDeallocation));
        assert!(card.id.0.contains("from-raw-parts"));
        Ok(())
    }

    #[test]
    fn vec_from_raw_parts_manuallydrop_origin_marks_same_origin_evidence() -> Result<(), String> {
        let output = fixture_output("vec_from_raw_parts_manuallydrop_origin")?;
        let card = single_card("vec_from_raw_parts_manuallydrop_origin", &output)?;

        assert_eq!(card.operation.family, OperationFamily::VecFromRawParts);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(obligation_discharge_present(card, "pointer-live"));
        assert!(obligation_discharge_present(card, "ownership"));
        assert!(!obligation_discharge_present(card, "alignment"));
        assert!(obligation_discharge_present(card, "capacity"));
        assert!(obligation_discharge_present(card, "initialized"));
        Ok(())
    }

    #[test]
    fn vec_from_raw_parts_capacity_evidence_requires_enforced_len_cap_guard() -> Result<(), String>
    {
        for fixture in [
            "vec_from_raw_parts_capacity_guard",
            "vec_from_raw_parts_capacity_assert_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.operation.family, OperationFamily::VecFromRawParts);
            assert!(obligation_discharge_present(card, "capacity"));
        }

        for fixture in [
            "vec_from_raw_parts_capacity_observed_not_guard",
            "vec_from_raw_parts_capacity_value_observed_not_guard",
            "vec_from_raw_parts_capacity_closed_branch_not_guard",
            "vec_from_raw_parts_capacity_reassigned_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.operation.family, OperationFamily::VecFromRawParts);
            assert!(!obligation_discharge_present(card, "capacity"));
        }
        Ok(())
    }

    #[test]
    fn box_from_raw_uses_ownership_operation_family() -> Result<(), String> {
        let output = fixture_output("box_from_raw")?;
        let card = single_card("box_from_raw", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::BoxFromRaw);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.hazards.contains(&HazardKind::DropOrDeallocation));
        assert!(card.id.0.contains("from-raw"));
        assert!(
            card.obligation_evidence
                .iter()
                .any(|evidence| evidence.obligation.key == "ownership"
                    && !evidence.discharge.present),
            "Box::from_raw ownership obligation should remain missing without allocator proof"
        );
        Ok(())
    }

    #[test]
    fn box_origin_evidence_rejects_reassigned_pointers() -> Result<(), String> {
        for (fixture, family) in [
            (
                "box_from_raw_reassigned_origin_not_guard",
                OperationFamily::BoxFromRaw,
            ),
            (
                "box_from_raw_box_origin_after_not_guard",
                OperationFamily::BoxFromRaw,
            ),
            (
                "box_from_raw_other_origin_not_guard",
                OperationFamily::BoxFromRaw,
            ),
            (
                "drop_in_place_reassigned_origin_not_guard",
                OperationFamily::DropInPlace,
            ),
            (
                "drop_in_place_box_origin_after_not_guard",
                OperationFamily::DropInPlace,
            ),
            (
                "drop_in_place_other_origin_not_guard",
                OperationFamily::DropInPlace,
            ),
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, family);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(card.hazards.contains(&HazardKind::DropOrDeallocation));
            assert!(!obligation_discharge_present(card, "ownership"));
        }
        Ok(())
    }

    #[test]
    fn static_mut_global_state_routes_to_concurrency_review() -> Result<(), String> {
        let output = fixture_output("static_mut_global_state")?;
        let card = single_card("static_mut_global_state", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::StaticMut);
        assert_eq!(card.operation.family, OperationFamily::StaticMut);
        assert_eq!(card.class, ReviewClass::RequiresLoom);
        assert!(card.hazards.contains(&HazardKind::StaticMutGlobalState));
        assert!(
            card.routes
                .iter()
                .any(|route| route.kind == WitnessKind::Loom)
        );
        assert!(
            card.routes
                .iter()
                .any(|route| route.kind == WitnessKind::Shuttle)
        );
        assert!(
            card.next_action.summary.contains("Loom/Shuttle"),
            "static mut global state should route reviewers to concurrency witnesses"
        );
        Ok(())
    }

    #[test]
    fn copy_nonoverlapping_uses_copy_operation_family() -> Result<(), String> {
        let output = fixture_output("copy_nonoverlapping")?;
        let card = single_card("copy_nonoverlapping", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::CopyNonOverlapping);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.hazards.contains(&HazardKind::AliasingOrProvenance));
        assert!(card.id.0.contains("copy-nonoverlapping"));
        Ok(())
    }

    #[test]
    fn ptr_copy_uses_overlapping_copy_operation_family() -> Result<(), String> {
        let output = fixture_output("ptr_copy_overlapping")?;
        let card = single_card("ptr_copy_overlapping", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::PtrCopy);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.hazards.contains(&HazardKind::PointerValidity));
        assert!(!card.hazards.contains(&HazardKind::AliasingOrProvenance));
        assert!(
            card.obligations
                .iter()
                .all(|obligation| obligation.key != "non-overlap"),
            "ptr::copy permits overlap and should not inherit the copy_nonoverlapping obligation"
        );
        assert!(card.id.0.contains("ptr-copy"));
        Ok(())
    }

    #[test]
    fn ptr_replace_uses_replacement_operation_family() -> Result<(), String> {
        let output = fixture_output("ptr_replace_value")?;
        let card = single_card("ptr_replace_value", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::PtrReplace);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.hazards.contains(&HazardKind::PointerValidity));
        assert!(card.hazards.contains(&HazardKind::DropOrDeallocation));
        assert!(
            card.obligations
                .iter()
                .any(|obligation| obligation.key == "ownership"),
            "ptr::replace should preserve drop/ownership review evidence"
        );
        assert!(card.id.0.contains("ptr-replace"));
        Ok(())
    }

    #[test]
    fn maybeuninit_assume_init_read_uses_assume_init_operation_family() -> Result<(), String> {
        let output = fixture_output("maybeuninit_assume_init_read")?;
        let card = single_card("maybeuninit_assume_init_read", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(
            card.operation.family,
            OperationFamily::MaybeUninitAssumeInit
        );
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.hazards.contains(&HazardKind::InitializedMemory));
        assert!(
            card.obligation_evidence.iter().any(|evidence| {
                evidence.obligation.key == "initialized" && !evidence.discharge.present
            }),
            "assume_init_read should keep initialized-memory evidence missing without proof"
        );
        assert!(card.id.0.contains("assume-init-read"));
        Ok(())
    }

    #[test]
    fn maybeuninit_assume_init_ref_uses_assume_init_operation_family() -> Result<(), String> {
        let output = fixture_output("maybeuninit_assume_init_ref")?;
        let card = single_card("maybeuninit_assume_init_ref", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(
            card.operation.family,
            OperationFamily::MaybeUninitAssumeInit
        );
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.hazards.contains(&HazardKind::InitializedMemory));
        assert!(
            card.obligation_evidence.iter().any(|evidence| {
                evidence.obligation.key == "initialized" && !evidence.discharge.present
            }),
            "assume_init_ref should keep initialized-memory evidence missing without proof"
        );
        assert!(card.id.0.contains("assume-init-ref"));
        Ok(())
    }

    #[test]
    fn maybeuninit_assume_init_mut_uses_assume_init_operation_family() -> Result<(), String> {
        let output = fixture_output("maybeuninit_assume_init_mut")?;
        let card = single_card("maybeuninit_assume_init_mut", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(
            card.operation.family,
            OperationFamily::MaybeUninitAssumeInit
        );
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.hazards.contains(&HazardKind::InitializedMemory));
        assert!(
            card.obligation_evidence.iter().any(|evidence| {
                evidence.obligation.key == "initialized" && !evidence.discharge.present
            }),
            "assume_init_mut should keep initialized-memory evidence missing without proof"
        );
        assert!(card.id.0.contains("assume-init-mut"));
        Ok(())
    }

    #[test]
    fn maybeuninit_assume_init_drop_uses_assume_init_operation_family() -> Result<(), String> {
        let output = fixture_output("maybeuninit_assume_init_drop")?;
        let card = single_card("maybeuninit_assume_init_drop", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(
            card.operation.family,
            OperationFamily::MaybeUninitAssumeInit
        );
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.hazards.contains(&HazardKind::InitializedMemory));
        assert!(
            card.obligation_evidence.iter().any(|evidence| {
                evidence.obligation.key == "initialized" && !evidence.discharge.present
            }),
            "assume_init_drop should keep initialized-memory evidence missing without proof"
        );
        assert!(card.id.0.contains("assume-init-drop"));
        Ok(())
    }

    #[test]
    fn vec_set_len_capacity_guards_bound_new_len() -> Result<(), String> {
        for fixture in [
            "vec_set_len",
            "vec_set_len_capacity_return_guard",
            "vec_set_len_capacity_open_branch_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::VecSetLen);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(obligation_discharge_present(card, "capacity"));
            assert!(!obligation_discharge_present(card, "initialized"));
        }
        Ok(())
    }

    #[test]
    fn vec_set_len_capacity_observation_is_not_capacity_guard() -> Result<(), String> {
        for fixture in [
            "vec_set_len_capacity_observed_not_guard",
            "vec_set_len_capacity_closed_branch_not_guard",
            "vec_set_len_capacity_reassigned_not_guard",
            "vec_set_len_capacity_open_branch_reassigned_len_not_guard",
            "vec_set_len_capacity_open_branch_reassigned_receiver_not_guard",
            "vec_set_len_capacity_receiver_reassigned_not_guard",
            "vec_set_len_capacity_binding_receiver_reassigned_not_guard",
            "vec_set_len_unrelated_capacity_comparison_not_guard",
            "vec_set_len_cap_argument_not_guard",
            "vec_set_len_with_capacity_reassigned_not_guard",
            "vec_set_len_with_capacity_len_reassigned_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::VecSetLen);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(!obligation_discharge_present(card, "capacity"));
            assert!(!obligation_discharge_present(card, "initialized"));
            assert!(
                card.missing.iter().any(|missing| missing.kind == "guard"),
                "observing capacity without bounding new_len must keep the guard prompt"
            );
        }
        Ok(())
    }

    #[test]
    fn vec_set_len_post_initialization_is_not_guard_evidence() -> Result<(), String> {
        let output = fixture_output("vec_set_len_post_init_not_guard")?;
        let card = single_card("vec_set_len_post_init_not_guard", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::VecSetLen);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(obligation_discharge_present(card, "capacity"));
        assert!(!obligation_discharge_present(card, "initialized"));
        assert!(
            card.missing.iter().any(|missing| missing.kind == "guard"),
            "initialization evidence after set_len must keep the guard prompt"
        );
        Ok(())
    }

    #[test]
    fn str_from_utf8_unchecked_uses_utf8_operation_family() -> Result<(), String> {
        for fixture in [
            "str_from_utf8_unchecked",
            "str_from_utf8_unchecked_post_validation_not_guard",
            "str_from_utf8_unchecked_other_buffer_not_guard",
            "str_from_utf8_unchecked_is_ok_observed_not_guard",
            "str_from_utf8_unchecked_guard_then_reassigned_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::StrFromUtf8Unchecked);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(card.hazards.contains(&HazardKind::InvalidValue));
            assert!(card.id.0.contains("from-utf8-unchecked"));
            assert!(
                card.obligation_evidence.iter().any(
                    |evidence| evidence.obligation.key == "utf8" && !evidence.discharge.present
                ),
                "UTF-8 validation obligation should remain missing without a visible guard"
            );
        }
        Ok(())
    }

    #[test]
    fn str_from_utf8_unchecked_validation_guards_are_discharged() -> Result<(), String> {
        for fixture in [
            "str_from_utf8_unchecked_is_ok_guard",
            "str_from_utf8_unchecked_is_err_return_guard",
            "str_from_utf8_unchecked_question_mark_guard",
            "str_from_utf8_unchecked_match_return_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::StrFromUtf8Unchecked);
            assert_eq!(card.class, ReviewClass::GuardedUnwitnessed);
            assert!(
                card.obligation_evidence.iter().any(
                    |evidence| evidence.obligation.key == "utf8" && evidence.discharge.present
                ),
                "{fixture} should discharge same-buffer UTF-8 validation evidence"
            );
        }
        Ok(())
    }

    #[test]
    fn transmute_bool_value_observation_is_not_guard_evidence() -> Result<(), String> {
        for fixture in [
            "transmute_bool_value_observed_not_guard",
            "transmute_bool_closed_if_observed_not_guard",
            "transmute_bool_guard_then_reassigned_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::Transmute);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(obligation_discharge_present(card, "layout"));
            assert!(!obligation_discharge_present(card, "valid-value"));
            assert!(
                card.missing.iter().any(|missing| missing.kind == "guard"),
                "{fixture} must not resolve this card's guard prompt"
            );
        }
        Ok(())
    }

    #[test]
    fn transmute_copy_bool_value_observation_is_not_guard_evidence() -> Result<(), String> {
        for fixture in [
            "transmute_copy_bool_value_observed_not_guard",
            "transmute_copy_bool_closed_if_observed_not_guard",
            "transmute_copy_bool_guard_then_reassigned_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::Transmute);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(obligation_discharge_present(card, "layout"));
            assert!(!obligation_discharge_present(card, "valid-value"));
            assert!(
                card.missing.iter().any(|missing| missing.kind == "guard"),
                "{fixture} must not resolve this card's guard prompt"
            );
        }
        Ok(())
    }

    #[test]
    fn zeroed_uses_valid_zero_operation_family() -> Result<(), String> {
        let output = fixture_output("zeroed_invalid_value")?;
        let card = single_card("zeroed_invalid_value", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::Zeroed);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.hazards.contains(&HazardKind::InvalidValue));
        assert!(card.id.0.contains("zeroed"));
        assert!(
            card.obligation_evidence.iter().any(|evidence| {
                evidence.obligation.key == "valid-zero" && !evidence.discharge.present
            }),
            "valid-zero obligation should remain missing without target-type evidence"
        );
        Ok(())
    }

    #[test]
    fn inline_asm_uses_inline_asm_operation_family() -> Result<(), String> {
        let output = fixture_output("inline_asm_human_review")?;
        let card = single_card("inline_asm_human_review", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::InlineAsm);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.hazards.contains(&HazardKind::InlineAsm));
        assert!(card.hazards.contains(&HazardKind::TargetFeature));
        assert!(
            card.routes
                .iter()
                .any(|route| route.kind == WitnessKind::HumanDeepReview),
            "inline asm should route to human deep review"
        );
        assert!(
            card.next_action.verify_commands.is_empty(),
            "human-review-only inline asm route should not invent a witness command"
        );
        Ok(())
    }

    #[test]
    fn pointer_arithmetic_num_ctrl_bytes_guard_is_discharged() -> Result<(), String> {
        let output = fixture_output("pointer_arithmetic_num_ctrl_bytes_guard")?;
        let card = single_card("pointer_arithmetic_num_ctrl_bytes_guard", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::PointerArithmetic);
        assert_eq!(card.class, ReviewClass::GuardedUnwitnessed);
        assert!(card.discharge.present);
        assert!(card.id.0.contains("add"));
        Ok(())
    }

    #[test]
    fn pointer_arithmetic_bounds_guard_must_match_operation_argument() -> Result<(), String> {
        for fixture in [
            "pointer_arithmetic_num_ctrl_bytes_other_index_not_guard",
            "pointer_arithmetic_num_ctrl_bytes_observed_not_guard",
            "pointer_arithmetic_num_ctrl_bytes_post_check_not_guard",
            "pointer_arithmetic_num_ctrl_bytes_closed_branch_not_guard",
            "pointer_arithmetic_num_ctrl_bytes_invalid_branch_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::PointerArithmetic);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(
                !obligation_discharge_present(card, "bounds"),
                "{fixture} should not discharge bounds"
            );
        }
        Ok(())
    }

    #[test]
    fn pointer_arithmetic_branch_bounds_guards_are_directional() -> Result<(), String> {
        for fixture in [
            "pointer_arithmetic_num_ctrl_bytes_open_branch_guard",
            "pointer_arithmetic_num_ctrl_bytes_return_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::PointerArithmetic);
            assert_eq!(card.class, ReviewClass::GuardedUnwitnessed);
            assert!(
                obligation_discharge_present(card, "bounds"),
                "{fixture} should discharge bounds"
            );
        }
        Ok(())
    }

    #[test]
    fn pointer_arithmetic_slice_end_guard_is_discharged() -> Result<(), String> {
        let output = fixture_output("pointer_arithmetic_slice_end")?;
        let card = single_card("pointer_arithmetic_slice_end", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::PointerArithmetic);
        assert_eq!(card.class, ReviewClass::GuardedUnwitnessed);
        assert!(card.discharge.present);
        assert!(obligation_discharge_present(card, "bounds"));
        assert!(card.id.0.contains("add"));
        Ok(())
    }

    #[test]
    fn documented_target_feature_declaration_is_guarded() -> Result<(), String> {
        let output = fixture_output("target_feature_safety_docs")?;
        let card = single_card("target_feature_safety_docs", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::TargetFeature);
        assert_eq!(card.class, ReviewClass::GuardedUnwitnessed);
        assert!(card.contract.present);
        assert!(card.discharge.present);
        assert!(obligation_discharge_present(card, "target-feature"));
        Ok(())
    }

    #[test]
    fn unwrap_unchecked_uses_concrete_operation_family() -> Result<(), String> {
        let output = fixture_output("unwrap_unchecked_result")?;
        let card = single_card("unwrap_unchecked_result", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::UnwrapUnchecked);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.hazards.contains(&HazardKind::InvalidValue));
        assert!(card.id.0.contains("unwrap-unchecked"));
        Ok(())
    }

    #[test]
    fn unwrap_unchecked_infallible_result_evidence_is_discharged() -> Result<(), String> {
        let output = fixture_output("unwrap_unchecked_infallible_result")?;
        let card = single_card("unwrap_unchecked_infallible_result", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::UnwrapUnchecked);
        assert_eq!(card.class, ReviewClass::GuardedUnwitnessed);
        assert!(card.discharge.present);
        assert!(obligation_discharge_present(card, "valid-value"));
        Ok(())
    }

    #[test]
    fn unwrap_unchecked_infallible_result_evidence_requires_same_receiver() -> Result<(), String> {
        let output = fixture_output("unwrap_unchecked_other_infallible_not_guard")?;
        let card = single_card("unwrap_unchecked_other_infallible_not_guard", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::UnwrapUnchecked);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!obligation_discharge_present(card, "valid-value"));
        Ok(())
    }

    #[test]
    fn unwrap_unchecked_if_let_as_ref_evidence_is_discharged() -> Result<(), String> {
        for fixture in [
            "unwrap_unchecked_if_let_some_guard",
            "unwrap_unchecked_if_let_ok_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::UnwrapUnchecked);
            assert_eq!(card.class, ReviewClass::GuardedUnwitnessed);
            assert!(obligation_discharge_present(card, "valid-value"));
        }
        Ok(())
    }

    #[test]
    fn unwrap_unchecked_state_evidence_rejects_wrong_receiver_or_post_check() -> Result<(), String>
    {
        for fixture in [
            "unwrap_unchecked_is_some_observed_not_guard",
            "unwrap_unchecked_is_ok_observed_not_guard",
            "unwrap_unchecked_other_if_let_not_guard",
            "unwrap_unchecked_other_if_let_ok_not_guard",
            "unwrap_unchecked_post_check_not_guard",
            "unwrap_unchecked_guard_then_reassigned_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::UnwrapUnchecked);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(!obligation_discharge_present(card, "valid-value"));
        }
        Ok(())
    }

    #[test]
    fn unreachable_unchecked_uses_concrete_operation_family() -> Result<(), String> {
        let output = fixture_output("unreachable_unchecked_path")?;
        let card = single_card("unreachable_unchecked_path", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::UnreachableUnchecked);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.hazards.contains(&HazardKind::InvalidValue));
        assert!(card.id.0.contains("unreachable-unchecked"));
        Ok(())
    }

    #[test]
    fn unreachable_unchecked_infallible_path_evidence_is_discharged() -> Result<(), String> {
        let output = fixture_output("unreachable_unchecked_infallible_path")?;
        let card = single_card("unreachable_unchecked_infallible_path", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::UnreachableUnchecked);
        assert_eq!(card.class, ReviewClass::GuardedUnwitnessed);
        assert!(card.discharge.present);
        assert!(obligation_discharge_present(card, "unreachable"));
        Ok(())
    }

    #[test]
    fn unreachable_unchecked_infallible_path_evidence_requires_same_match() -> Result<(), String> {
        for fixture in [
            "unreachable_unchecked_other_infallible_not_guard",
            "unreachable_unchecked_post_infallible_not_guard",
            "unreachable_unchecked_closed_infallible_match_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::UnreachableUnchecked);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(!obligation_discharge_present(card, "unreachable"));
        }
        Ok(())
    }

    #[test]
    fn impl_trait_bound_owner_inference_uses_function_owner() -> Result<(), String> {
        let output = fixture_output("impl_trait_bound_owner_inference")?;
        let card = single_card("impl_trait_bound_owner_inference", &output)?;

        assert_eq!(card.operation.family, OperationFamily::UnsafeFnCall);
        assert_eq!(card.site.owner, Some("try_reserve".to_string()));
        assert!(card.id.0.contains("try-reserve"));
        assert!(
            card.reach.summary.contains("try_reserve"),
            "reach evidence should use the function owner, not the impl Trait bound"
        );
        Ok(())
    }

    #[test]
    fn private_unsafe_helper_can_use_local_safety_comment() -> Result<(), String> {
        let output = fixture_output("private_unsafe_helper_safety_comment")?;
        let card = single_card("private_unsafe_helper_safety_comment", &output)?;

        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!card.site.public_api_surface);
        assert!(card.contract.present);
        assert_eq!(card.reach.state, "owner_reached");
        assert!(
            !card
                .missing
                .iter()
                .any(|missing| missing.kind == "contract")
        );
        Ok(())
    }

    #[test]
    fn card_identity_is_stable_across_line_drift() -> Result<(), String> {
        let original = fixture_output("raw_pointer_alignment")?;
        let drifted = fixture_output("raw_pointer_alignment_line_drift")?;
        let original_card = single_card("raw_pointer_alignment", &original)?;
        let drifted_card = single_card("raw_pointer_alignment_line_drift", &drifted)?;

        assert_ne!(
            original_card.site.location.line, drifted_card.site.location.line,
            "fixture should prove line drift occurred"
        );
        assert_eq!(
            original_card.id, drifted_card.id,
            "card identity should not include the line number"
        );
        Ok(())
    }

    #[test]
    fn card_identity_counts_duplicate_sites() -> Result<(), String> {
        let output = fixture_output("duplicate_raw_pointer_reads")?;
        if output.cards.len() != 2 {
            return Err(format!(
                "duplicate_raw_pointer_reads should emit two cards, got {}",
                output.cards.len()
            ));
        }
        let first = &output.cards[0];
        let second = &output.cards[1];

        assert_eq!(first.operation.family, OperationFamily::RawPointerRead);
        assert_eq!(second.operation.family, OperationFamily::RawPointerRead);
        assert_ne!(first.id, second.id);
        assert!(first.id.0.ends_with("-c1"));
        assert!(second.id.0.ends_with("-c2"));
        assert_eq!(
            identity_without_count(&first.id),
            identity_without_count(&second.id)
        );
        Ok(())
    }

    #[test]
    fn baseline_policy_marks_exact_card_identity_as_known() -> Result<(), String> {
        let root = copy_fixture_to_temp("raw_pointer_alignment", "unsafe-review-baseline-match")?;
        let card_id = single_card("raw_pointer_alignment", &fixture_output_at(&root)?)?
            .id
            .0
            .clone();
        write_policy_ledger(
            &root,
            "unsafe-review-baseline.toml",
            &card_id,
            "review_after",
        )?;

        let output = fixture_output_at(&root)?;
        let card = single_card("raw_pointer_alignment baseline", &output)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp fixture failed: {err}"))?;
        assert_eq!(card.class, ReviewClass::BaselineKnown);
        assert_eq!(card.priority, Priority::Low);
        assert_eq!(output.summary.open_actionable_gaps, 0);
        assert!(card.next_action.summary.contains("Known baseline"));
        Ok(())
    }

    #[test]
    fn suppression_policy_marks_exact_card_identity_as_suppressed() -> Result<(), String> {
        let root =
            copy_fixture_to_temp("raw_pointer_alignment", "unsafe-review-suppression-match")?;
        let card_id = single_card("raw_pointer_alignment", &fixture_output_at(&root)?)?
            .id
            .0
            .clone();
        write_policy_ledger(
            &root,
            "unsafe-review-suppressions.toml",
            &card_id,
            "expires",
        )?;

        let output = fixture_output_at(&root)?;
        let card = single_card("raw_pointer_alignment suppression", &output)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp fixture failed: {err}"))?;
        assert_eq!(card.class, ReviewClass::Suppressed);
        assert_eq!(card.priority, Priority::Low);
        assert_eq!(output.summary.open_actionable_gaps, 0);
        assert!(card.next_action.summary.contains("Suppressed"));
        Ok(())
    }

    #[test]
    fn imported_receipt_marks_witness_evidence_present_by_exact_card_identity() -> Result<(), String>
    {
        let root = copy_fixture_to_temp("raw_pointer_alignment", "unsafe-review-receipt-match")?;
        let card_id = single_card("raw_pointer_alignment", &fixture_output_at(&root)?)?
            .id
            .0
            .clone();
        write_receipt(&root, &card_id)?;

        let output = fixture_output_at(&root)?;
        let card = single_card("raw_pointer_alignment receipt", &output)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp fixture failed: {err}"))?;
        assert!(card.witness.present);
        assert!(card.witness.summary.contains("miri"));
        assert!(card.witness.summary.contains("ran"));
        assert!(!card.missing.iter().any(|missing| missing.kind == "witness"));
        assert!(
            card.obligation_evidence
                .iter()
                .all(|evidence| evidence.witness.present)
        );
        Ok(())
    }

    #[test]
    fn receipted_fixture_keeps_static_guard_gap_visible() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment_receipted")?;
        let card = single_card("raw_pointer_alignment_receipted", &output)?;

        assert_eq!(card.operation.family, OperationFamily::RawPointerRead);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.witness.present);
        assert!(card.witness.summary.contains("miri"));
        assert!(card.witness.summary.contains("ran"));
        assert!(!card.missing.iter().any(|missing| missing.kind == "witness"));
        assert!(card.missing.iter().any(|missing| missing.kind == "guard"));
        assert!(
            !obligation_discharge_present(card, "alignment"),
            "Miri receipt evidence must not discharge the static alignment guard obligation"
        );
        assert!(
            card.obligation_evidence
                .iter()
                .all(|evidence| evidence.witness.present)
        );
        Ok(())
    }

    fn fixture_output(name: &str) -> Result<AnalyzeOutput, String> {
        let root = fixture_root(name);
        fixture_output_at(&root)
    }

    fn fixture_output_at(root: &Path) -> Result<AnalyzeOutput, String> {
        analyze(AnalyzeInput {
            root: root.to_path_buf(),
            scope: Scope::Diff,
            diff: DiffSource::File(root.join("change.diff")),
            mode: AnalysisMode::Draft,
            policy: PolicyMode::Advisory,
            include_unchanged_tests: true,
            max_cards: None,
        })
    }

    fn fixture_root(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
    }

    fn single_card<'a>(fixture: &str, output: &'a AnalyzeOutput) -> Result<&'a ReviewCard, String> {
        if output.cards.len() != 1 {
            return Err(format!(
                "{fixture} should emit exactly one card, got {}",
                output.cards.len()
            ));
        }
        Ok(&output.cards[0])
    }

    fn obligation_discharge_present(card: &ReviewCard, key: &str) -> bool {
        card.obligation_evidence
            .iter()
            .find(|evidence| evidence.obligation.key == key)
            .is_some_and(|evidence| evidence.discharge.present)
    }

    fn assert_no_unknown_wrapper_card(fixture: &str, output: &AnalyzeOutput) {
        assert!(
            output.cards.iter().all(|card| {
                !(card.site.kind == UnsafeSiteKind::UnsafeBlock
                    && card.operation.family == OperationFamily::Unknown)
            }),
            "{fixture} should suppress duplicate unknown unsafe-block wrapper cards"
        );
    }

    fn identity_without_count(id: &CardId) -> &str {
        id.0.rsplit_once("-c")
            .map_or(id.0.as_str(), |(base, _count)| base)
    }

    fn copy_fixture_to_temp(name: &str, prefix: &str) -> Result<PathBuf, String> {
        let source = fixture_root(name);
        let target = unique_temp_dir(prefix)?;
        copy_dir_all(&source, &target)?;
        Ok(target)
    }

    fn copy_dir_all(source: &Path, target: &Path) -> Result<(), String> {
        fs::create_dir_all(target)
            .map_err(|err| format!("create {} failed: {err}", target.display()))?;
        let entries = fs::read_dir(source)
            .map_err(|err| format!("read {} failed: {err}", source.display()))?;
        for entry in entries {
            let entry = entry.map_err(|err| format!("read_dir entry failed: {err}"))?;
            let source_path = entry.path();
            let target_path = target.join(entry.file_name());
            if source_path.is_dir() {
                copy_dir_all(&source_path, &target_path)?;
            } else {
                fs::copy(&source_path, &target_path).map_err(|err| {
                    format!(
                        "copy {} to {} failed: {err}",
                        source_path.display(),
                        target_path.display()
                    )
                })?;
            }
        }
        Ok(())
    }

    fn write_policy_ledger(
        root: &Path,
        file_name: &str,
        card_id: &str,
        date_key: &str,
    ) -> Result<(), String> {
        let policy_dir = root.join("policy");
        fs::create_dir_all(&policy_dir)
            .map_err(|err| format!("create {} failed: {err}", policy_dir.display()))?;
        fs::write(
            policy_dir.join(file_name),
            format!(
                r#"schema_version = "0.1"
status = "active"

[[entries]]
card_id = "{card_id}"
owner = "core/policy"
reason = "fixture policy match"
evidence = "test fixture"
{date_key} = "2026-08-01"
"#
            ),
        )
        .map_err(|err| format!("write policy ledger failed: {err}"))
    }

    fn write_receipt(root: &Path, card_id: &str) -> Result<(), String> {
        let receipt_dir = root.join(".unsafe-review").join("receipts");
        fs::create_dir_all(&receipt_dir)
            .map_err(|err| format!("create {} failed: {err}", receipt_dir.display()))?;
        fs::write(
            receipt_dir.join("miri.json"),
            format!(
                r#"{{
  "schema_version": "0.1",
  "card_id": "{card_id}",
  "tool": "miri",
  "strength": "ran",
  "author": "core/fixtures",
  "recorded_at": "2026-05-18T00:00:00Z",
  "expires_at": "2026-08-18",
  "summary": "focused fixture witness passed",
  "command": "cargo +nightly miri test read_header",
  "limitations": ["fixture only"]
}}"#
            ),
        )
        .map_err(|err| format!("write receipt failed: {err}"))
    }

    fn unique_temp_dir(prefix: &str) -> Result<PathBuf, String> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system clock before UNIX_EPOCH: {err}"))?
            .as_nanos();
        Ok(std::env::temp_dir().join(format!("{prefix}-{nanos}")))
    }
}
