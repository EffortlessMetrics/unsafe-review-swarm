mod action_summary;
mod card_builder;
mod card_identity;
mod input_loading;
mod summary;

use self::action_summary::next_action_summary;
#[cfg(test)]
use self::card_identity::unsafe_call_path;
use self::input_loading::{load_diff_index, package_name};
use self::summary::summarize;
use super::{receipts, scanner};
use crate::api::{
    AnalysisMode, AnalyzeInput, AnalyzeOutput, DiffSource, DiscoveryOptions, RepoScanEvent,
    RepoScanPhase, RepoScanStatus, Scope,
};
use crate::domain::ReviewCard;
use crate::input::workspace;
use crate::policy::PolicyState;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

type RepoEventFn<'a> = &'a mut dyn FnMut(&RepoScanEvent) -> Result<(), String>;

pub(crate) fn analyze(input: AnalyzeInput) -> Result<AnalyzeOutput, String> {
    let discovery = default_discovery_for(&input);
    analyze_with_receipts(input, true, discovery, None)
}

pub(crate) fn analyze_with_discovery(
    input: AnalyzeInput,
    discovery: DiscoveryOptions,
) -> Result<AnalyzeOutput, String> {
    analyze_with_receipts(input, true, discovery, None)
}

pub(crate) fn analyze_with_discovery_and_progress<F>(
    input: AnalyzeInput,
    discovery: DiscoveryOptions,
    mut progress: F,
) -> Result<AnalyzeOutput, String>
where
    F: FnMut(&RepoScanStatus) -> Result<(), String>,
{
    let mut events = |event: &RepoScanEvent| progress(&event.status);
    analyze_with_receipts(input, true, discovery, Some(&mut events))
}

pub(crate) fn analyze_with_discovery_and_repo_events<F>(
    input: AnalyzeInput,
    discovery: DiscoveryOptions,
    mut events: F,
) -> Result<AnalyzeOutput, String>
where
    F: FnMut(&RepoScanEvent) -> Result<(), String>,
{
    analyze_with_receipts(input, true, discovery, Some(&mut events))
}

pub(crate) fn analyze_without_receipts(input: AnalyzeInput) -> Result<AnalyzeOutput, String> {
    let discovery = default_discovery_for(&input);
    analyze_with_receipts(input, false, discovery, None)
}

fn default_discovery_for(input: &AnalyzeInput) -> DiscoveryOptions {
    if matches!(input.scope, Scope::Repo) || matches!(input.mode, AnalysisMode::Repo) {
        DiscoveryOptions::repo_defaults()
    } else {
        DiscoveryOptions::default()
    }
}

fn analyze_with_receipts(
    input: AnalyzeInput,
    import_receipts: bool,
    discovery: DiscoveryOptions,
    mut events: Option<RepoEventFn<'_>>,
) -> Result<AnalyzeOutput, String> {
    let started = Instant::now();
    let repo_mode = matches!(input.scope, Scope::Repo) || matches!(input.mode, AnalysisMode::Repo);
    let diff_index = load_diff_index(&input.diff)?;
    let changed_files = diff_index.changed_file_count();
    let changed_non_rust_files = diff_index.changed_non_rust_file_count();
    emit_repo_status(
        &mut events,
        repo_status(RepoScanPhase::Discovering, &started, 0, 0, 0, None, false),
    )?;
    let mut discovered_files = 0usize;
    let all_rust_files = {
        let mut discovery_progress = |count: usize, path: &Path| {
            discovered_files = count;
            emit_repo_status(
                &mut events,
                repo_status(
                    RepoScanPhase::Discovering,
                    &started,
                    discovered_files,
                    0,
                    0,
                    Some(path.to_path_buf()),
                    false,
                ),
            )
        };
        workspace::discover_rust_files_with_progress(
            &input.root,
            &discovery,
            Some(&mut discovery_progress),
        )?
    };
    discovered_files = all_rust_files.len();
    let package = package_name(&input.root);
    let policy_state = PolicyState::load(&input.root)?;
    let receipt_index = if import_receipts {
        receipts::ReceiptIndex::load(&input.root)?
    } else {
        receipts::ReceiptIndex::default()
    };
    // A diff was *supplied* when the source is Text or File (even if the
    // parsed index is empty — an empty `git diff` is a valid zero-change diff,
    // not a repo-scan trigger).  Only NoneRepoScan means "no diff at all" and
    // should fall back to scanning everything.
    let diff_supplied = !matches!(input.diff, DiffSource::NoneRepoScan);
    let candidate_files = if repo_mode || !diff_supplied {
        all_rust_files.clone()
    } else {
        all_rust_files
            .iter()
            .filter(|path| diff_index.contains_file(path))
            .cloned()
            .collect::<Vec<_>>()
    };
    let changed_rust_files = if repo_mode || !diff_supplied {
        candidate_files.len()
    } else {
        diff_index.changed_rust_file_count()
    };

    let mut cards = Vec::new();
    let mut identity_counts = BTreeMap::new();
    let max_cards = input.max_cards.unwrap_or(usize::MAX);
    let mut files_scanned = 0usize;
    let mut last_scanned_path = None;
    emit_repo_status(
        &mut events,
        repo_status(
            RepoScanPhase::Scanning,
            &started,
            discovered_files,
            files_scanned,
            cards.len(),
            None,
            false,
        ),
    )?;
    'files: for rel in &candidate_files {
        if cards.len() >= max_cards {
            break;
        }
        emit_repo_status(
            &mut events,
            repo_status(
                RepoScanPhase::Scanning,
                &started,
                discovered_files,
                files_scanned,
                cards.len(),
                Some(rel.clone()),
                false,
            ),
        )?;
        let scanned = scanner::scan_file(&input.root, rel, Some(&diff_index), repo_mode)?;
        files_scanned += 1;
        let mut build_ctx = card_builder::CardBuildContext {
            root: &input.root,
            package: &package,
            receipt_index: &receipt_index,
            policy_state: &policy_state,
            identity_counts: &mut identity_counts,
        };
        let mut reached_max_cards = false;
        for scanned_site in scanned {
            cards.push(card_builder::build_card(&mut build_ctx, scanned_site));
            if cards.len() >= max_cards {
                reached_max_cards = true;
                break;
            }
        }
        emit_repo_event(
            &mut events,
            repo_status(
                RepoScanPhase::Scanning,
                &started,
                discovered_files,
                files_scanned,
                cards.len(),
                Some(rel.clone()),
                false,
            ),
            Some(partial_analyze_output(
                &input,
                all_rust_files.len(),
                changed_files,
                changed_rust_files,
                changed_non_rust_files,
                &cards,
                policy_state.baseline_ids(),
                &policy_state,
            )),
        )?;
        last_scanned_path = Some(rel.clone());
        if reached_max_cards {
            break 'files;
        }
    }
    sort_cards(&mut cards);
    let summary = summarize(
        all_rust_files.len(),
        changed_files,
        changed_rust_files,
        changed_non_rust_files,
        &cards,
        &input.scope,
        policy_state.baseline_ids(),
        &policy_state,
    );
    let output = AnalyzeOutput {
        schema_version: "0.1".to_string(),
        tool: "unsafe-review".to_string(),
        root: input.root.clone(),
        scope: input.scope.clone(),
        mode: input.mode.clone(),
        policy: input.policy.clone(),
        summary,
        cards,
    };
    emit_repo_event(
        &mut events,
        repo_status(
            RepoScanPhase::Complete,
            &started,
            discovered_files,
            files_scanned,
            output.cards.len(),
            last_scanned_path,
            true,
        ),
        Some(output.clone()),
    )?;
    Ok(output)
}

fn sort_cards(cards: &mut [ReviewCard]) {
    cards.sort_by(|left, right| {
        left.site
            .location
            .file
            .cmp(&right.site.location.file)
            .then(left.site.location.line.cmp(&right.site.location.line))
    });
}

#[allow(
    clippy::too_many_arguments,
    reason = "mirrors summarize() signature; grouping into a struct would add churn without clarity gain"
)]
fn partial_analyze_output(
    input: &AnalyzeInput,
    rust_files: usize,
    changed_files: usize,
    changed_rust_files: usize,
    changed_non_rust_files: usize,
    cards: &[ReviewCard],
    baseline_ids: &BTreeSet<String>,
    policy_state: &PolicyState,
) -> AnalyzeOutput {
    let mut cards = cards.to_vec();
    sort_cards(&mut cards);
    let summary = summarize(
        rust_files,
        changed_files,
        changed_rust_files,
        changed_non_rust_files,
        &cards,
        &input.scope,
        baseline_ids,
        policy_state,
    );
    AnalyzeOutput {
        schema_version: "0.1".to_string(),
        tool: "unsafe-review".to_string(),
        root: input.root.clone(),
        scope: input.scope.clone(),
        mode: input.mode.clone(),
        policy: input.policy.clone(),
        summary,
        cards,
    }
}

fn emit_repo_status(
    events: &mut Option<RepoEventFn<'_>>,
    status: RepoScanStatus,
) -> Result<(), String> {
    emit_repo_event(events, status, None)
}

fn emit_repo_event(
    events: &mut Option<RepoEventFn<'_>>,
    status: RepoScanStatus,
    partial_output: Option<AnalyzeOutput>,
) -> Result<(), String> {
    if let Some(events) = events.as_deref_mut() {
        events(&RepoScanEvent {
            status,
            partial_output,
        })?;
    }
    Ok(())
}

fn repo_status(
    phase: RepoScanPhase,
    started: &Instant,
    files_discovered: usize,
    files_scanned: usize,
    cards_found: usize,
    last_path: Option<PathBuf>,
    completed: bool,
) -> RepoScanStatus {
    RepoScanStatus {
        schema_version: "repo-scan-status/v1".to_string(),
        phase,
        elapsed_ms: started.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
        files_discovered,
        files_scanned,
        cards_found,
        last_path,
        completed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{AnalysisMode, DiffSource, DiscoveryOptions, PolicyMode, Scope};
    use crate::domain::{
        CardId, HazardKind, OperationFamily, Priority, ProofPath, ReviewCard, ReviewClass,
        UnsafeSiteKind, WitnessKind, WitnessRoute,
    };
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_witness_route(kind: WitnessKind) -> WitnessRoute {
        WitnessRoute {
            kind,
            reason: "test route".to_string(),
            command: None,
            required: false,
        }
    }

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
                "raw_pointer_write_bool_bytes_guard",
                OperationFamily::RawPointerWrite,
                true,
            ),
            (
                "raw_pointer_write_bool_reassigned_byte_not_guard",
                OperationFamily::RawPointerWrite,
                true,
            ),
            (
                "raw_pointer_write_bool_closed_branch_not_guard",
                OperationFamily::RawPointerWrite,
                true,
            ),
            (
                "raw_pointer_write_previous_slice_not_guard",
                OperationFamily::RawPointerWrite,
                true,
            ),
            (
                "raw_pointer_write_previous_u8_not_guard",
                OperationFamily::RawPointerWrite,
                true,
            ),
            (
                "raw_pointer_write_previous_bool_not_guard",
                OperationFamily::RawPointerWrite,
                true,
            ),
            (
                "raw_pointer_write_previous_maybeuninit_not_guard",
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
        assert!(!obligation_discharge_present(card, "initialized"));
        assert!(!obligation_discharge_present(card, "bounds"));
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
    fn raw_pointer_write_bool_bytes_guard_discharges_byte_obligations() -> Result<(), String> {
        for fixture in [
            "raw_pointer_write_bool_bytes_guard",
            "raw_pointer_write_bool_conjunct_branch_guard",
            "raw_pointer_write_bool_disjunct_return_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.operation.family, OperationFamily::RawPointerWrite);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(obligation_discharge_present(card, "initialized"));
            assert!(obligation_discharge_present(card, "alignment"));
            assert!(!obligation_discharge_present(card, "pointer-live"));
            assert!(!obligation_discharge_present(card, "bounds"));
            assert!(!obligation_discharge_present(card, "allocation"));
        }
        Ok(())
    }

    #[test]
    fn raw_pointer_write_bool_bytes_guard_requires_fresh_byte() -> Result<(), String> {
        let output = fixture_output("raw_pointer_write_bool_reassigned_byte_not_guard")?;
        let card = single_card("raw_pointer_write_bool_reassigned_byte_not_guard", &output)?;

        assert_eq!(card.operation.family, OperationFamily::RawPointerWrite);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(obligation_discharge_present(card, "alignment"));
        assert!(!obligation_discharge_present(card, "initialized"));
        assert!(!obligation_discharge_present(card, "pointer-live"));
        assert!(!obligation_discharge_present(card, "bounds"));
        assert!(!obligation_discharge_present(card, "allocation"));
        Ok(())
    }

    #[test]
    fn raw_pointer_write_bool_bytes_guard_requires_open_branch() -> Result<(), String> {
        for fixture in [
            "raw_pointer_write_bool_closed_branch_not_guard",
            "raw_pointer_write_bool_disjunct_branch_not_guard",
            "raw_pointer_write_bool_conjunct_return_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.operation.family, OperationFamily::RawPointerWrite);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(obligation_discharge_present(card, "alignment"));
            assert!(!obligation_discharge_present(card, "initialized"));
            assert!(!obligation_discharge_present(card, "pointer-live"));
            assert!(!obligation_discharge_present(card, "bounds"));
            assert!(!obligation_discharge_present(card, "allocation"));
        }
        Ok(())
    }

    #[test]
    fn raw_pointer_write_bounds_evidence_requires_current_operation() -> Result<(), String> {
        let output = fixture_output("raw_pointer_write_previous_slice_not_guard")?;
        let card = single_card("raw_pointer_write_previous_slice_not_guard", &output)?;

        assert_eq!(card.operation.family, OperationFamily::RawPointerWrite);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!obligation_discharge_present(card, "bounds"));
        assert!(!obligation_discharge_present(card, "initialized"));
        Ok(())
    }

    #[test]
    fn raw_pointer_write_target_evidence_requires_current_operation() -> Result<(), String> {
        for fixture in [
            "raw_pointer_write_previous_u8_not_guard",
            "raw_pointer_write_previous_bool_not_guard",
            "raw_pointer_write_previous_maybeuninit_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.operation.family, OperationFamily::RawPointerWrite);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(!obligation_discharge_present(card, "alignment"));
            assert!(!obligation_discharge_present(card, "initialized"));
        }
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
    fn raw_pointer_read_bounds_evidence_rejects_unenforced_guards() -> Result<(), String> {
        for fixture in [
            "raw_pointer_read_bounds_observed_not_guard",
            "raw_pointer_read_len_capacity_observed_not_guard",
            "raw_pointer_read_assert_shadowed_origin_not_guard",
            "raw_pointer_read_len_capacity_assert_shadowed_origin_not_guard",
            "raw_pointer_read_open_branch_shadowed_origin_not_guard",
            "raw_pointer_read_typed_shadowed_origin_not_guard",
            "raw_pointer_read_other_len_not_guard",
            "raw_pointer_read_reassigned_origin_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.operation.family, OperationFamily::RawPointerRead);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(!obligation_discharge_present(card, "bounds"));
            assert!(!obligation_discharge_present(card, "alignment"));
            assert!(!card.discharge.present);
        }
        Ok(())
    }

    #[test]
    fn raw_pointer_read_bounds_evidence_accepts_open_same_origin_branch() -> Result<(), String> {
        let output = fixture_output("raw_pointer_read_open_branch_bounds_guard")?;
        let card = single_card("raw_pointer_read_open_branch_bounds_guard", &output)?;

        assert_eq!(card.operation.family, OperationFamily::RawPointerRead);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(obligation_discharge_present(card, "bounds"));
        assert!(!obligation_discharge_present(card, "alignment"));
        Ok(())
    }

    #[test]
    fn raw_pointer_read_bounds_evidence_accepts_cast_pointer_origin() -> Result<(), String> {
        let output = fixture_output("raw_pointer_read_cast_origin_bounds_guard")?;
        let card = single_card("raw_pointer_read_cast_origin_bounds_guard", &output)?;

        assert_eq!(card.operation.family, OperationFamily::RawPointerRead);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(obligation_discharge_present(card, "bounds"));
        assert!(!obligation_discharge_present(card, "alignment"));
        Ok(())
    }

    #[test]
    fn raw_pointer_read_bounds_evidence_accepts_as_cast_pointer_origin() -> Result<(), String> {
        let output = fixture_output("raw_pointer_read_as_cast_origin_bounds_guard")?;
        let card = single_card("raw_pointer_read_as_cast_origin_bounds_guard", &output)?;

        assert_eq!(card.operation.family, OperationFamily::RawPointerRead);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(obligation_discharge_present(card, "bounds"));
        assert!(!obligation_discharge_present(card, "alignment"));
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
    fn specialist_review_classes_get_routed_next_actions() {
        for (class, expected) in [
            (ReviewClass::RequiresSanitizer, "sanitizer"),
            (ReviewClass::RequiresKaniOrCrux, "Kani/Crux"),
            (ReviewClass::ReachableUnwitnessed, "witness receipt"),
            (ReviewClass::WitnessMismatch, "matching receipt"),
            (ReviewClass::StaticUnknown, "witness route"),
        ] {
            let summary = next_action_summary(&class, "raw_pointer_read", false, &[]);
            assert!(
                summary.contains(expected),
                "`{}` next action `{summary}` should mention `{expected}`",
                class.as_str()
            );
        }
    }

    #[test]
    fn guarded_cards_get_route_specific_witness_next_actions() {
        let human_route = [test_witness_route(WitnessKind::HumanDeepReview)];
        let summary = next_action_summary(
            &ReviewClass::GuardedUnwitnessed,
            "unknown",
            false,
            &human_route,
        );
        assert!(
            summary.contains("human deep-review witness receipt"),
            "human-routed guarded card next action `{summary}` should route to human deep-review receipt evidence"
        );
        assert!(summary.contains("static limitation"));

        let miri_careful_routes = [
            test_witness_route(WitnessKind::Miri),
            test_witness_route(WitnessKind::CargoCareful),
        ];
        let miri_supported = next_action_summary(
            &ReviewClass::GuardedUnwitnessed,
            "raw_pointer_read",
            false,
            &miri_careful_routes,
        );
        assert!(miri_supported.contains("Miri"));
        assert!(miri_supported.contains("cargo-careful"));
        assert!(miri_supported.contains("witness receipt"));
        assert!(!miri_supported.contains("human deep-review"));
    }

    #[test]
    fn inline_asm_guard_missing_next_action_routes_to_manual_invariant_review() {
        let human_route = [test_witness_route(WitnessKind::HumanDeepReview)];
        let summary = next_action_summary(
            &ReviewClass::GuardMissing,
            "inline_asm",
            false,
            &human_route,
        );

        assert!(summary.contains("inline_asm"));
        assert!(summary.contains("manually"));
        assert!(summary.contains("guard evidence"));
        assert!(summary.contains("human deep-review receipt"));
        assert!(!summary.contains("local guard that discharges"));
    }

    #[test]
    fn pin_unchecked_guard_missing_next_action_routes_to_manual_invariant_review() {
        let human_route = [test_witness_route(WitnessKind::HumanDeepReview)];
        let summary = next_action_summary(
            &ReviewClass::GuardMissing,
            "pin_unchecked",
            false,
            &human_route,
        );

        assert!(summary.contains("pin_unchecked"));
        assert!(summary.contains("move-prevention"));
        assert!(summary.contains("projection invariants"));
        assert!(summary.contains("guard evidence"));
        assert!(summary.contains("human deep-review receipt"));
        assert!(!summary.contains("local guard that discharges"));
    }

    #[test]
    fn unsafe_fn_call_guard_missing_next_action_routes_to_callee_contract_review() {
        let human_route = [test_witness_route(WitnessKind::HumanDeepReview)];
        let summary = next_action_summary(
            &ReviewClass::GuardMissing,
            "unsafe_fn_call",
            false,
            &human_route,
        );

        assert!(summary.contains("unsafe_fn_call"));
        assert!(summary.contains("callee contract"));
        assert!(summary.contains("obligation-specific guard evidence"));
        assert!(!summary.contains("local guard that discharges"));
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
    fn repo_scan_honors_discovery_filters_before_analysis() -> Result<(), String> {
        let root = unique_temp_dir("unsafe-review-filtered-repo")?;
        fs::create_dir_all(root.join("src")).map_err(|err| format!("create src failed: {err}"))?;
        fs::create_dir_all(root.join("packages/pkg/src"))
            .map_err(|err| format!("create package src failed: {err}"))?;
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"filtered-repo-fixture\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
        )
        .map_err(|err| format!("write Cargo.toml failed: {err}"))?;
        fs::write(root.join("src/lib.rs"), "pub unsafe fn selected() {}\n")
            .map_err(|err| format!("write src file failed: {err}"))?;
        fs::write(
            root.join("packages/pkg/src/lib.rs"),
            "pub unsafe fn excluded() {}\n",
        )
        .map_err(|err| format!("write package file failed: {err}"))?;

        let output = analyze_with_discovery(
            AnalyzeInput {
                root: root.clone(),
                scope: Scope::Repo,
                diff: DiffSource::NoneRepoScan,
                mode: AnalysisMode::Repo,
                policy: PolicyMode::Advisory,
                include_unchanged_tests: true,
                max_cards: None,
            },
            DiscoveryOptions {
                include: vec!["src/**/*.rs".to_string(), "packages/**/*.rs".to_string()],
                exclude: vec!["packages/**".to_string()],
                ..DiscoveryOptions::repo_defaults()
            },
        )?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert_eq!(output.summary.rust_files, 1);
        assert_eq!(output.summary.changed_rust_files, 1);
        assert_eq!(output.cards.len(), 1);
        assert_eq!(
            output.cards[0].site.location.file,
            PathBuf::from("src/lib.rs")
        );
        Ok(())
    }

    #[test]
    fn repo_scan_status_reports_discovery_scanning_and_completion() -> Result<(), String> {
        let root = unique_temp_dir("unsafe-review-repo-status")?;
        fs::create_dir_all(root.join("src")).map_err(|err| format!("create src failed: {err}"))?;
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"repo-status-fixture\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
        )
        .map_err(|err| format!("write Cargo.toml failed: {err}"))?;
        fs::write(
            root.join("src/lib.rs"),
            "pub unsafe fn read_header(ptr: *const u8) -> u32 { unsafe { ptr.cast::<u32>().read() } }\n",
        )
        .map_err(|err| format!("write src file failed: {err}"))?;

        let mut statuses = Vec::new();
        let output = analyze_with_discovery_and_progress(
            AnalyzeInput {
                root: root.clone(),
                scope: Scope::Repo,
                diff: DiffSource::NoneRepoScan,
                mode: AnalysisMode::Repo,
                policy: PolicyMode::Advisory,
                include_unchanged_tests: true,
                max_cards: None,
            },
            DiscoveryOptions::repo_defaults(),
            |status| {
                statuses.push(status.clone());
                Ok(())
            },
        )?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            statuses
                .iter()
                .any(|status| status.phase == RepoScanPhase::Discovering)
        );
        assert!(
            statuses
                .iter()
                .any(|status| status.phase == RepoScanPhase::Scanning)
        );
        let complete = statuses
            .last()
            .ok_or_else(|| "expected at least one repo scan status".to_string())?;
        assert_eq!(complete.phase, RepoScanPhase::Complete);
        assert!(complete.completed);
        assert_eq!(complete.files_discovered, 1);
        assert_eq!(complete.files_scanned, 1);
        assert_eq!(complete.cards_found, output.cards.len());
        assert_eq!(complete.last_path, Some(PathBuf::from("src/lib.rs")));
        Ok(())
    }

    #[test]
    fn repo_scan_events_include_completed_file_snapshots() -> Result<(), String> {
        let root = unique_temp_dir("unsafe-review-repo-events")?;
        fs::create_dir_all(root.join("src")).map_err(|err| format!("create src failed: {err}"))?;
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"repo-event-fixture\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
        )
        .map_err(|err| format!("write Cargo.toml failed: {err}"))?;
        fs::write(root.join("src/lib.rs"), "pub unsafe fn source_root() {}\n")
            .map_err(|err| format!("write src file failed: {err}"))?;
        fs::write(root.join("src/z.rs"), "pub fn safe() {}\n")
            .map_err(|err| format!("write safe source failed: {err}"))?;

        let mut partials = Vec::new();
        let output = analyze_with_discovery_and_repo_events(
            AnalyzeInput {
                root: root.clone(),
                scope: Scope::Repo,
                diff: DiffSource::NoneRepoScan,
                mode: AnalysisMode::Repo,
                policy: PolicyMode::Advisory,
                include_unchanged_tests: true,
                max_cards: Some(1),
            },
            DiscoveryOptions::repo_defaults(),
            |event| {
                if let Some(partial) = &event.partial_output
                    && event.status.phase == RepoScanPhase::Scanning
                    && event.status.files_scanned == 1
                {
                    partials.push(partial.clone());
                }
                Ok(())
            },
        )?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let first_partial = partials
            .first()
            .ok_or_else(|| "expected a partial snapshot after the first file".to_string())?;
        assert_eq!(first_partial.summary.rust_files, 2);
        assert_eq!(first_partial.summary.changed_rust_files, 2);
        assert_eq!(first_partial.cards.len(), 1);
        assert_eq!(
            first_partial.cards[0].site.location.file,
            PathBuf::from("src/lib.rs")
        );
        assert_eq!(output.cards.len(), 1);
        Ok(())
    }

    #[test]
    fn diff_summary_reports_mixed_language_scope_without_scanning_non_rust_files()
    -> Result<(), String> {
        let root = unique_temp_dir("unsafe-review-mixed-diff-summary")?;
        fs::create_dir_all(root.join("src")).map_err(|err| format!("create src failed: {err}"))?;
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"mixed-diff-summary-fixture\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
        )
        .map_err(|err| format!("write Cargo.toml failed: {err}"))?;
        fs::write(root.join("src/lib.rs"), "pub unsafe fn source_root() {}\n")
            .map_err(|err| format!("write src file failed: {err}"))?;
        let diff = r#"diff --git a/src/lib.rs b/src/lib.rs
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,0 +1,1 @@
+pub unsafe fn source_root() {}
diff --git a/src/js/buffer.ts b/src/js/buffer.ts
--- a/src/js/buffer.ts
+++ b/src/js/buffer.ts
@@ -1,0 +1,1 @@
+export const changed = true;
diff --git a/src/binding.cpp b/src/binding.cpp
--- a/src/binding.cpp
+++ b/src/binding.cpp
@@ -1,0 +1,1 @@
+void changed() {}
"#;

        let mut partials = Vec::new();
        let output = analyze_with_discovery_and_repo_events(
            AnalyzeInput {
                root: root.clone(),
                scope: Scope::Diff,
                diff: DiffSource::Text(diff.to_string()),
                mode: AnalysisMode::Draft,
                policy: PolicyMode::Advisory,
                include_unchanged_tests: true,
                max_cards: None,
            },
            DiscoveryOptions::default(),
            |event| {
                if let Some(partial) = &event.partial_output
                    && event.status.phase == RepoScanPhase::Scanning
                    && event.status.files_scanned == 1
                {
                    partials.push(partial.clone());
                }
                Ok(())
            },
        )?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert_eq!(output.summary.rust_files, 1);
        assert_eq!(output.summary.changed_files, 3);
        assert_eq!(output.summary.changed_rust_files, 1);
        assert_eq!(output.summary.changed_non_rust_files, 2);
        assert_eq!(output.cards.len(), 1);

        let first_partial = partials
            .first()
            .ok_or_else(|| "expected a partial snapshot after the first file".to_string())?;
        assert_eq!(first_partial.summary.changed_files, 3);
        assert_eq!(first_partial.summary.changed_rust_files, 1);
        assert_eq!(first_partial.summary.changed_non_rust_files, 2);
        assert_eq!(first_partial.cards.len(), 1);
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
                card.next_action.summary.contains("public `# Safety`")
                    && !card.next_action.summary.contains("SAFETY:"),
                "{fixture} next action should require public safety docs, not a local SAFETY comment"
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
            "unchecked_constructor_unavailable_return_comment_not_guard",
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
    fn nonnull_new_guard_variants_require_same_pointer_applicability() -> Result<(), String> {
        for fixture in [
            "nonnull_new_guard",
            "nonnull_if_let_new_guard",
            "nonnull_let_else_new_guard",
            "nonnull_match_new_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::NonNullUnchecked);
            assert_eq!(card.class, ReviewClass::GuardedUnwitnessed);
            assert!(card.discharge.present);
            assert!(obligation_discharge_present(card, "non-null"));
            assert!(
                card.missing.iter().all(|missing| missing.kind != "guard"),
                "{fixture} should resolve the local guard prompt with same-pointer evidence"
            );
            assert!(card.id.0.contains("nonnull-unchecked"));
        }
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
        for fixture in [
            "nonnull_is_null_nonreturning_not_guard",
            "nonnull_is_null_return_comment_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::NonNullUnchecked);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(!card.discharge.present);
            assert!(!obligation_discharge_present(card, "non-null"));
            assert!(
                card.missing.iter().any(|missing| missing.kind == "guard"),
                "{fixture}: observing null without exiting must not resolve this card's guard prompt"
            );
            assert!(card.id.0.contains("nonnull-unchecked"));
        }
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
    fn nonnull_pointer_evidence_rejects_stale_reassignment_shadowing_or_cast_target()
    -> Result<(), String> {
        for fixture in [
            "nonnull_new_reassigned_ptr_not_guard",
            "nonnull_new_shadowed_ptr_not_guard",
            "nonnull_cast_checked_pointer_not_guard",
            "nonnull_method_receiver_reassigned_not_guard",
            "nonnull_method_receiver_shadowed_not_guard",
            "nonnull_is_null_reassigned_ptr_not_guard",
            "nonnull_is_null_shadowed_ptr_not_guard",
            "nonnull_is_null_open_branch_shadowed_ptr_not_guard",
            "nonnull_if_let_new_reassigned_ptr_not_guard",
            "nonnull_if_let_new_shadowed_ptr_not_guard",
            "nonnull_let_else_new_reassigned_ptr_not_guard",
            "nonnull_let_else_new_shadowed_ptr_not_guard",
            "nonnull_match_new_reassigned_ptr_not_guard",
            "nonnull_match_new_shadowed_ptr_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::NonNullUnchecked);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(!card.discharge.present);
            assert!(!obligation_discharge_present(card, "non-null"));
            assert!(
                card.missing.iter().any(|missing| missing.kind == "guard"),
                "{fixture} must keep stale pointer evidence from resolving the guard prompt"
            );
            assert!(card.id.0.contains("nonnull-unchecked"));
        }
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
    fn get_unchecked_mut_get_probe_bounds_evidence_is_discharged() -> Result<(), String> {
        for fixture in [
            "get_unchecked_mut_get_probe_guard",
            "get_unchecked_mut_get_probe_early_return_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::GetUnchecked);
            assert_eq!(card.class, ReviewClass::GuardedUnwitnessed);
            assert!(card.discharge.present);
            assert!(obligation_discharge_present(card, "bounds"));
        }
        Ok(())
    }

    #[test]
    fn get_unchecked_mut_get_probe_pattern_evidence_is_discharged() -> Result<(), String> {
        for fixture in [
            "get_unchecked_mut_if_let_get_guard",
            "get_unchecked_mut_let_else_get_guard",
            "get_unchecked_mut_match_get_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::GetUnchecked);
            assert_eq!(card.class, ReviewClass::GuardedUnwitnessed);
            assert!(card.discharge.present);
            assert!(obligation_discharge_present(card, "bounds"));
        }
        Ok(())
    }

    #[test]
    fn get_unchecked_mut_len_branch_evidence_respects_branch_shape() -> Result<(), String> {
        let conjunct = fixture_output("get_unchecked_mut_conjunct_len_guard")?;
        let conjunct_card = single_card("get_unchecked_mut_conjunct_len_guard", &conjunct)?;
        assert_eq!(conjunct_card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(
            conjunct_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(conjunct_card.class, ReviewClass::GuardedUnwitnessed);
        assert!(conjunct_card.discharge.present);
        assert!(obligation_discharge_present(conjunct_card, "bounds"));

        let disjunct = fixture_output("get_unchecked_mut_disjunct_len_not_guard")?;
        let disjunct_card = single_card("get_unchecked_mut_disjunct_len_not_guard", &disjunct)?;
        assert_eq!(disjunct_card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(
            disjunct_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(disjunct_card.class, ReviewClass::GuardMissing);
        assert!(!disjunct_card.discharge.present);
        assert!(!obligation_discharge_present(disjunct_card, "bounds"));
        Ok(())
    }

    #[test]
    fn get_unchecked_mut_len_evidence_rejects_comment_only_return() -> Result<(), String> {
        let output = fixture_output("get_unchecked_mut_return_comment_not_guard")?;
        let card = single_card("get_unchecked_mut_return_comment_not_guard", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::GetUnchecked);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!card.discharge.present);
        assert!(!obligation_discharge_present(card, "bounds"));
        Ok(())
    }

    #[test]
    fn get_unchecked_mut_len_evidence_rejects_other_receiver() -> Result<(), String> {
        let output = fixture_output("get_unchecked_mut_other_len_not_guard")?;
        let card = single_card("get_unchecked_mut_other_len_not_guard", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::GetUnchecked);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!card.discharge.present);
        assert!(!obligation_discharge_present(card, "bounds"));
        Ok(())
    }

    #[test]
    fn get_unchecked_mut_len_evidence_rejects_post_check() -> Result<(), String> {
        let output = fixture_output("get_unchecked_mut_post_check_not_guard")?;
        let card = single_card("get_unchecked_mut_post_check_not_guard", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::GetUnchecked);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!card.discharge.present);
        assert!(!obligation_discharge_present(card, "bounds"));
        Ok(())
    }

    #[test]
    fn get_unchecked_mut_len_evidence_rejects_observed_bounds() -> Result<(), String> {
        let output = fixture_output("get_unchecked_mut_bounds_observed_not_guard")?;
        let card = single_card("get_unchecked_mut_bounds_observed_not_guard", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::GetUnchecked);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!card.discharge.present);
        assert!(!obligation_discharge_present(card, "bounds"));
        Ok(())
    }

    #[test]
    fn get_unchecked_mut_len_evidence_rejects_closed_bounds() -> Result<(), String> {
        let output = fixture_output("get_unchecked_mut_closed_bounds_not_guard")?;
        let card = single_card("get_unchecked_mut_closed_bounds_not_guard", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::GetUnchecked);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!card.discharge.present);
        assert!(!obligation_discharge_present(card, "bounds"));
        Ok(())
    }

    #[test]
    fn get_unchecked_mut_len_evidence_rejects_reassigned_index() -> Result<(), String> {
        let output = fixture_output("get_unchecked_mut_reassigned_index_not_guard")?;
        let card = single_card("get_unchecked_mut_reassigned_index_not_guard", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::GetUnchecked);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!card.discharge.present);
        assert!(!obligation_discharge_present(card, "bounds"));
        Ok(())
    }

    #[test]
    fn get_unchecked_mut_len_evidence_rejects_compound_reassigned_index() -> Result<(), String> {
        let output = fixture_output("get_unchecked_mut_compound_reassigned_index_not_guard")?;
        let card = single_card(
            "get_unchecked_mut_compound_reassigned_index_not_guard",
            &output,
        )?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::GetUnchecked);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!card.discharge.present);
        assert!(!obligation_discharge_present(card, "bounds"));
        Ok(())
    }

    #[test]
    fn get_unchecked_mut_len_evidence_rejects_shadowed_index() -> Result<(), String> {
        let output = fixture_output("get_unchecked_mut_shadowed_index_not_guard")?;
        let card = single_card("get_unchecked_mut_shadowed_index_not_guard", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::GetUnchecked);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!card.discharge.present);
        assert!(!obligation_discharge_present(card, "bounds"));
        Ok(())
    }

    #[test]
    fn get_unchecked_mut_get_probe_evidence_rejects_reassigned_index() -> Result<(), String> {
        for fixture in [
            "get_unchecked_mut_get_probe_reassigned_index_not_guard",
            "get_unchecked_mut_get_probe_early_return_reassigned_index_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::GetUnchecked);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(!card.discharge.present);
            assert!(!obligation_discharge_present(card, "bounds"));
        }
        Ok(())
    }

    #[test]
    fn get_unchecked_mut_get_probe_evidence_rejects_shadowed_index() -> Result<(), String> {
        for fixture in [
            "get_unchecked_mut_get_probe_shadowed_index_not_guard",
            "get_unchecked_mut_get_probe_early_return_shadowed_index_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::GetUnchecked);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(!card.discharge.present);
            assert!(!obligation_discharge_present(card, "bounds"));
        }
        Ok(())
    }

    #[test]
    fn get_unchecked_mut_get_probe_evidence_rejects_reassigned_receiver() -> Result<(), String> {
        for fixture in [
            "get_unchecked_mut_get_probe_reassigned_receiver_not_guard",
            "get_unchecked_mut_get_probe_early_return_reassigned_receiver_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::GetUnchecked);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(!card.discharge.present);
            assert!(!obligation_discharge_present(card, "bounds"));
        }
        Ok(())
    }

    #[test]
    fn get_unchecked_mut_get_probe_evidence_rejects_shadowed_receiver() -> Result<(), String> {
        for fixture in [
            "get_unchecked_mut_get_probe_shadowed_receiver_not_guard",
            "get_unchecked_mut_get_probe_early_return_shadowed_receiver_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::GetUnchecked);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(!card.discharge.present);
            assert!(!obligation_discharge_present(card, "bounds"));
        }
        Ok(())
    }

    #[test]
    fn get_unchecked_mut_get_probe_evidence_rejects_reassigned_receiver_path() -> Result<(), String>
    {
        let output =
            fixture_output("get_unchecked_mut_get_probe_reassigned_receiver_path_not_guard")?;
        let card = single_card(
            "get_unchecked_mut_get_probe_reassigned_receiver_path_not_guard",
            &output,
        )?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::GetUnchecked);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!card.discharge.present);
        assert!(!obligation_discharge_present(card, "bounds"));
        Ok(())
    }

    #[test]
    fn get_unchecked_mut_get_probe_evidence_rejects_shadowed_receiver_path() -> Result<(), String> {
        let output =
            fixture_output("get_unchecked_mut_get_probe_shadowed_receiver_path_not_guard")?;
        let card = single_card(
            "get_unchecked_mut_get_probe_shadowed_receiver_path_not_guard",
            &output,
        )?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::GetUnchecked);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!card.discharge.present);
        assert!(!obligation_discharge_present(card, "bounds"));
        Ok(())
    }

    #[test]
    fn get_unchecked_mut_get_probe_evidence_rejects_other_slice() -> Result<(), String> {
        let output = fixture_output("get_unchecked_mut_get_probe_other_slice_not_guard")?;
        let card = single_card("get_unchecked_mut_get_probe_other_slice_not_guard", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::GetUnchecked);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!card.discharge.present);
        assert!(!obligation_discharge_present(card, "bounds"));
        Ok(())
    }

    #[test]
    fn get_unchecked_mut_get_probe_pattern_evidence_rejects_reassigned_index() -> Result<(), String>
    {
        for fixture in [
            "get_unchecked_mut_if_let_get_reassigned_index_not_guard",
            "get_unchecked_mut_let_else_get_reassigned_index_not_guard",
            "get_unchecked_mut_match_get_reassigned_index_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::GetUnchecked);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(!card.discharge.present);
            assert!(!obligation_discharge_present(card, "bounds"));
        }
        Ok(())
    }

    #[test]
    fn get_unchecked_mut_get_probe_pattern_evidence_rejects_shadowed_index() -> Result<(), String> {
        for fixture in [
            "get_unchecked_mut_if_let_get_shadowed_index_not_guard",
            "get_unchecked_mut_let_else_get_shadowed_index_not_guard",
            "get_unchecked_mut_match_get_shadowed_index_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::GetUnchecked);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(!card.discharge.present);
            assert!(!obligation_discharge_present(card, "bounds"));
        }
        Ok(())
    }

    #[test]
    fn get_unchecked_mut_get_probe_pattern_evidence_rejects_reassigned_receiver()
    -> Result<(), String> {
        for fixture in [
            "get_unchecked_mut_if_let_get_reassigned_receiver_not_guard",
            "get_unchecked_mut_let_else_get_reassigned_receiver_not_guard",
            "get_unchecked_mut_match_get_reassigned_receiver_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::GetUnchecked);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(!card.discharge.present);
            assert!(!obligation_discharge_present(card, "bounds"));
        }
        Ok(())
    }

    #[test]
    fn get_unchecked_mut_get_probe_pattern_evidence_rejects_shadowed_receiver() -> Result<(), String>
    {
        for fixture in [
            "get_unchecked_mut_if_let_get_shadowed_receiver_not_guard",
            "get_unchecked_mut_let_else_get_shadowed_receiver_not_guard",
            "get_unchecked_mut_match_get_shadowed_receiver_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::GetUnchecked);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(!card.discharge.present);
            assert!(!obligation_discharge_present(card, "bounds"));
        }
        Ok(())
    }

    #[test]
    fn get_unchecked_mut_get_probe_pattern_evidence_rejects_reassigned_receiver_path()
    -> Result<(), String> {
        for fixture in [
            "get_unchecked_mut_if_let_get_reassigned_receiver_path_not_guard",
            "get_unchecked_mut_let_else_get_reassigned_receiver_path_not_guard",
            "get_unchecked_mut_match_get_reassigned_receiver_path_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::GetUnchecked);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(!card.discharge.present);
            assert!(!obligation_discharge_present(card, "bounds"));
        }
        Ok(())
    }

    #[test]
    fn get_unchecked_mut_get_probe_pattern_evidence_rejects_shadowed_receiver_path()
    -> Result<(), String> {
        for fixture in [
            "get_unchecked_mut_if_let_get_shadowed_receiver_path_not_guard",
            "get_unchecked_mut_let_else_get_shadowed_receiver_path_not_guard",
            "get_unchecked_mut_match_get_shadowed_receiver_path_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::GetUnchecked);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(!card.discharge.present);
            assert!(!obligation_discharge_present(card, "bounds"));
        }
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

        let compound_reassigned_index =
            fixture_output("get_unchecked_mut_compound_reassigned_index_not_guard")?;
        let compound_reassigned_index_card = single_card(
            "get_unchecked_mut_compound_reassigned_index_not_guard",
            &compound_reassigned_index,
        )?;
        assert_eq!(
            compound_reassigned_index_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            compound_reassigned_index_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            compound_reassigned_index_card,
            "bounds"
        ));

        let shadowed_index = fixture_output("get_unchecked_mut_shadowed_index_not_guard")?;
        let shadowed_index_card = single_card(
            "get_unchecked_mut_shadowed_index_not_guard",
            &shadowed_index,
        )?;
        assert_eq!(
            shadowed_index_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(shadowed_index_card.class, ReviewClass::GuardMissing);
        assert!(!obligation_discharge_present(shadowed_index_card, "bounds"));

        let shadowed_probe_index =
            fixture_output("get_unchecked_mut_get_probe_shadowed_index_not_guard")?;
        let shadowed_probe_index_card = single_card(
            "get_unchecked_mut_get_probe_shadowed_index_not_guard",
            &shadowed_probe_index,
        )?;
        assert_eq!(
            shadowed_probe_index_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(shadowed_probe_index_card.class, ReviewClass::GuardMissing);
        assert!(!obligation_discharge_present(
            shadowed_probe_index_card,
            "bounds"
        ));

        let reassigned_receiver =
            fixture_output("get_unchecked_mut_reassigned_receiver_not_guard")?;
        let reassigned_receiver_card = single_card(
            "get_unchecked_mut_reassigned_receiver_not_guard",
            &reassigned_receiver,
        )?;
        assert_eq!(
            reassigned_receiver_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(reassigned_receiver_card.class, ReviewClass::GuardMissing);
        assert!(!obligation_discharge_present(
            reassigned_receiver_card,
            "bounds"
        ));

        let reassigned_receiver_path =
            fixture_output("get_unchecked_mut_reassigned_receiver_path_not_guard")?;
        let reassigned_receiver_path_card = single_card(
            "get_unchecked_mut_reassigned_receiver_path_not_guard",
            &reassigned_receiver_path,
        )?;
        assert_eq!(
            reassigned_receiver_path_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            reassigned_receiver_path_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            reassigned_receiver_path_card,
            "bounds"
        ));

        let shadowed_receiver = fixture_output("get_unchecked_mut_shadowed_receiver_not_guard")?;
        let shadowed_receiver_card = single_card(
            "get_unchecked_mut_shadowed_receiver_not_guard",
            &shadowed_receiver,
        )?;
        assert_eq!(
            shadowed_receiver_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(shadowed_receiver_card.class, ReviewClass::GuardMissing);
        assert!(!obligation_discharge_present(
            shadowed_receiver_card,
            "bounds"
        ));

        let reassigned_probe_receiver =
            fixture_output("get_unchecked_mut_get_probe_reassigned_receiver_not_guard")?;
        let reassigned_probe_receiver_card = single_card(
            "get_unchecked_mut_get_probe_reassigned_receiver_not_guard",
            &reassigned_probe_receiver,
        )?;
        assert_eq!(
            reassigned_probe_receiver_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            reassigned_probe_receiver_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            reassigned_probe_receiver_card,
            "bounds"
        ));

        let reassigned_probe_receiver_path =
            fixture_output("get_unchecked_mut_get_probe_reassigned_receiver_path_not_guard")?;
        let reassigned_probe_receiver_path_card = single_card(
            "get_unchecked_mut_get_probe_reassigned_receiver_path_not_guard",
            &reassigned_probe_receiver_path,
        )?;
        assert_eq!(
            reassigned_probe_receiver_path_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            reassigned_probe_receiver_path_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            reassigned_probe_receiver_path_card,
            "bounds"
        ));

        let shadowed_probe_receiver_path =
            fixture_output("get_unchecked_mut_get_probe_shadowed_receiver_path_not_guard")?;
        let shadowed_probe_receiver_path_card = single_card(
            "get_unchecked_mut_get_probe_shadowed_receiver_path_not_guard",
            &shadowed_probe_receiver_path,
        )?;
        assert_eq!(
            shadowed_probe_receiver_path_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            shadowed_probe_receiver_path_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            shadowed_probe_receiver_path_card,
            "bounds"
        ));

        let shadowed_probe_receiver =
            fixture_output("get_unchecked_mut_get_probe_shadowed_receiver_not_guard")?;
        let shadowed_probe_receiver_card = single_card(
            "get_unchecked_mut_get_probe_shadowed_receiver_not_guard",
            &shadowed_probe_receiver,
        )?;
        assert_eq!(
            shadowed_probe_receiver_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            shadowed_probe_receiver_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            shadowed_probe_receiver_card,
            "bounds"
        ));

        let stale_return_probe_receiver = fixture_output(
            "get_unchecked_mut_get_probe_early_return_reassigned_receiver_not_guard",
        )?;
        let stale_return_probe_receiver_card = single_card(
            "get_unchecked_mut_get_probe_early_return_reassigned_receiver_not_guard",
            &stale_return_probe_receiver,
        )?;
        assert_eq!(
            stale_return_probe_receiver_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            stale_return_probe_receiver_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            stale_return_probe_receiver_card,
            "bounds"
        ));

        let shadowed_return_probe_index =
            fixture_output("get_unchecked_mut_get_probe_early_return_shadowed_index_not_guard")?;
        let shadowed_return_probe_index_card = single_card(
            "get_unchecked_mut_get_probe_early_return_shadowed_index_not_guard",
            &shadowed_return_probe_index,
        )?;
        assert_eq!(
            shadowed_return_probe_index_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            shadowed_return_probe_index_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            shadowed_return_probe_index_card,
            "bounds"
        ));

        let shadowed_return_probe_receiver =
            fixture_output("get_unchecked_mut_get_probe_early_return_shadowed_receiver_not_guard")?;
        let shadowed_return_probe_receiver_card = single_card(
            "get_unchecked_mut_get_probe_early_return_shadowed_receiver_not_guard",
            &shadowed_return_probe_receiver,
        )?;
        assert_eq!(
            shadowed_return_probe_receiver_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            shadowed_return_probe_receiver_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            shadowed_return_probe_receiver_card,
            "bounds"
        ));

        let shadowed_if_let_probe_index =
            fixture_output("get_unchecked_mut_if_let_get_shadowed_index_not_guard")?;
        let shadowed_if_let_probe_index_card = single_card(
            "get_unchecked_mut_if_let_get_shadowed_index_not_guard",
            &shadowed_if_let_probe_index,
        )?;
        assert_eq!(
            shadowed_if_let_probe_index_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            shadowed_if_let_probe_index_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            shadowed_if_let_probe_index_card,
            "bounds"
        ));

        let stale_if_let_probe_receiver =
            fixture_output("get_unchecked_mut_if_let_get_reassigned_receiver_not_guard")?;
        let stale_if_let_probe_receiver_card = single_card(
            "get_unchecked_mut_if_let_get_reassigned_receiver_not_guard",
            &stale_if_let_probe_receiver,
        )?;
        assert_eq!(
            stale_if_let_probe_receiver_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            stale_if_let_probe_receiver_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            stale_if_let_probe_receiver_card,
            "bounds"
        ));

        let stale_if_let_probe_receiver_path =
            fixture_output("get_unchecked_mut_if_let_get_reassigned_receiver_path_not_guard")?;
        let stale_if_let_probe_receiver_path_card = single_card(
            "get_unchecked_mut_if_let_get_reassigned_receiver_path_not_guard",
            &stale_if_let_probe_receiver_path,
        )?;
        assert_eq!(
            stale_if_let_probe_receiver_path_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            stale_if_let_probe_receiver_path_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            stale_if_let_probe_receiver_path_card,
            "bounds"
        ));

        let shadowed_if_let_probe_receiver_path =
            fixture_output("get_unchecked_mut_if_let_get_shadowed_receiver_path_not_guard")?;
        let shadowed_if_let_probe_receiver_path_card = single_card(
            "get_unchecked_mut_if_let_get_shadowed_receiver_path_not_guard",
            &shadowed_if_let_probe_receiver_path,
        )?;
        assert_eq!(
            shadowed_if_let_probe_receiver_path_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            shadowed_if_let_probe_receiver_path_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            shadowed_if_let_probe_receiver_path_card,
            "bounds"
        ));

        let shadowed_if_let_probe_receiver =
            fixture_output("get_unchecked_mut_if_let_get_shadowed_receiver_not_guard")?;
        let shadowed_if_let_probe_receiver_card = single_card(
            "get_unchecked_mut_if_let_get_shadowed_receiver_not_guard",
            &shadowed_if_let_probe_receiver,
        )?;
        assert_eq!(
            shadowed_if_let_probe_receiver_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            shadowed_if_let_probe_receiver_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            shadowed_if_let_probe_receiver_card,
            "bounds"
        ));

        let shadowed_let_else_probe_index =
            fixture_output("get_unchecked_mut_let_else_get_shadowed_index_not_guard")?;
        let shadowed_let_else_probe_index_card = single_card(
            "get_unchecked_mut_let_else_get_shadowed_index_not_guard",
            &shadowed_let_else_probe_index,
        )?;
        assert_eq!(
            shadowed_let_else_probe_index_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            shadowed_let_else_probe_index_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            shadowed_let_else_probe_index_card,
            "bounds"
        ));

        let stale_let_else_probe_receiver =
            fixture_output("get_unchecked_mut_let_else_get_reassigned_receiver_not_guard")?;
        let stale_let_else_probe_receiver_card = single_card(
            "get_unchecked_mut_let_else_get_reassigned_receiver_not_guard",
            &stale_let_else_probe_receiver,
        )?;
        assert_eq!(
            stale_let_else_probe_receiver_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            stale_let_else_probe_receiver_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            stale_let_else_probe_receiver_card,
            "bounds"
        ));

        let stale_let_else_probe_receiver_path =
            fixture_output("get_unchecked_mut_let_else_get_reassigned_receiver_path_not_guard")?;
        let stale_let_else_probe_receiver_path_card = single_card(
            "get_unchecked_mut_let_else_get_reassigned_receiver_path_not_guard",
            &stale_let_else_probe_receiver_path,
        )?;
        assert_eq!(
            stale_let_else_probe_receiver_path_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            stale_let_else_probe_receiver_path_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            stale_let_else_probe_receiver_path_card,
            "bounds"
        ));

        let shadowed_let_else_probe_receiver_path =
            fixture_output("get_unchecked_mut_let_else_get_shadowed_receiver_path_not_guard")?;
        let shadowed_let_else_probe_receiver_path_card = single_card(
            "get_unchecked_mut_let_else_get_shadowed_receiver_path_not_guard",
            &shadowed_let_else_probe_receiver_path,
        )?;
        assert_eq!(
            shadowed_let_else_probe_receiver_path_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            shadowed_let_else_probe_receiver_path_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            shadowed_let_else_probe_receiver_path_card,
            "bounds"
        ));

        let shadowed_let_else_probe_receiver =
            fixture_output("get_unchecked_mut_let_else_get_shadowed_receiver_not_guard")?;
        let shadowed_let_else_probe_receiver_card = single_card(
            "get_unchecked_mut_let_else_get_shadowed_receiver_not_guard",
            &shadowed_let_else_probe_receiver,
        )?;
        assert_eq!(
            shadowed_let_else_probe_receiver_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            shadowed_let_else_probe_receiver_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            shadowed_let_else_probe_receiver_card,
            "bounds"
        ));

        let shadowed_match_probe_index =
            fixture_output("get_unchecked_mut_match_get_shadowed_index_not_guard")?;
        let shadowed_match_probe_index_card = single_card(
            "get_unchecked_mut_match_get_shadowed_index_not_guard",
            &shadowed_match_probe_index,
        )?;
        assert_eq!(
            shadowed_match_probe_index_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            shadowed_match_probe_index_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            shadowed_match_probe_index_card,
            "bounds"
        ));

        let stale_match_probe_receiver =
            fixture_output("get_unchecked_mut_match_get_reassigned_receiver_not_guard")?;
        let stale_match_probe_receiver_card = single_card(
            "get_unchecked_mut_match_get_reassigned_receiver_not_guard",
            &stale_match_probe_receiver,
        )?;
        assert_eq!(
            stale_match_probe_receiver_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            stale_match_probe_receiver_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            stale_match_probe_receiver_card,
            "bounds"
        ));

        let stale_match_probe_receiver_path =
            fixture_output("get_unchecked_mut_match_get_reassigned_receiver_path_not_guard")?;
        let stale_match_probe_receiver_path_card = single_card(
            "get_unchecked_mut_match_get_reassigned_receiver_path_not_guard",
            &stale_match_probe_receiver_path,
        )?;
        assert_eq!(
            stale_match_probe_receiver_path_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            stale_match_probe_receiver_path_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            stale_match_probe_receiver_path_card,
            "bounds"
        ));

        let shadowed_match_probe_receiver_path =
            fixture_output("get_unchecked_mut_match_get_shadowed_receiver_path_not_guard")?;
        let shadowed_match_probe_receiver_path_card = single_card(
            "get_unchecked_mut_match_get_shadowed_receiver_path_not_guard",
            &shadowed_match_probe_receiver_path,
        )?;
        assert_eq!(
            shadowed_match_probe_receiver_path_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            shadowed_match_probe_receiver_path_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            shadowed_match_probe_receiver_path_card,
            "bounds"
        ));

        let shadowed_match_probe_receiver =
            fixture_output("get_unchecked_mut_match_get_shadowed_receiver_not_guard")?;
        let shadowed_match_probe_receiver_card = single_card(
            "get_unchecked_mut_match_get_shadowed_receiver_not_guard",
            &shadowed_match_probe_receiver,
        )?;
        assert_eq!(
            shadowed_match_probe_receiver_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            shadowed_match_probe_receiver_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            shadowed_match_probe_receiver_card,
            "bounds"
        ));

        let other_slice_probe =
            fixture_output("get_unchecked_mut_get_probe_other_slice_not_guard")?;
        let other_slice_probe_card = single_card(
            "get_unchecked_mut_get_probe_other_slice_not_guard",
            &other_slice_probe,
        )?;
        assert_eq!(
            other_slice_probe_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(other_slice_probe_card.class, ReviewClass::GuardMissing);
        assert!(!obligation_discharge_present(
            other_slice_probe_card,
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
    fn js_buffer_reentry_after_descriptor_capture_emits_advisory_card() -> Result<(), String> {
        let output = temp_source_output(
            "unsafe-review-js-buffer-reentry",
            r#"
pub struct JSValue;
pub struct GlobalObject;
pub struct Options;
pub struct StringOrBuffer;

impl StringOrBuffer {
    pub fn from_js(_global: &mut GlobalObject, _value: JSValue) -> Result<Self, ()> {
        Ok(Self)
    }

    pub fn byte_slice(&self) -> &[u8] {
        &[]
    }
}

impl Options {
    pub fn get(&self, _global: &mut GlobalObject, _name: &str) -> Result<i32, ()> {
        Ok(1)
    }
}

pub fn zstd_sync(
    global: &mut GlobalObject,
    arg0: JSValue,
    options: Options,
) -> Result<usize, ()> {
    let input = StringOrBuffer::from_js(global, arg0)?;
    let level = options.get(global, "level")?;
    native_compress(&input, level)
}

fn native_compress(input: &StringOrBuffer, level: i32) -> Result<usize, ()> {
    let bytes = input.byte_slice();
    Ok(bytes.len() + level as usize)
}
"#,
        )?;
        let card = single_card("js_buffer_reentry", &output)?;

        assert_eq!(
            card.operation.family,
            OperationFamily::StableByteSourceGetterReentry
        );
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert_eq!(card.proof_path, ProofPath::ObservableRedGreen);
        assert_eq!(card.site.owner.as_deref(), Some("zstd_sync"));
        assert!(
            card.operation
                .expression
                .contains("stable-byte-source-getter-reentry")
        );
        assert!(
            card.operation
                .expression
                .contains("StringOrBuffer::from_js")
        );
        assert!(card.operation.expression.contains("options.get"));
        assert!(card.operation.expression.contains("native_compress"));
        assert!(
            card.next_action
                .summary
                .contains("observable-red-green proof path")
        );
        assert!(
            card.routes
                .iter()
                .any(|route| route.kind == WitnessKind::HumanDeepReview)
        );
        Ok(())
    }

    #[test]
    fn js_buffer_reentry_heuristic_respects_capture_before_reentry_order() -> Result<(), String> {
        let output = temp_source_output(
            "unsafe-review-js-buffer-reentry-negative",
            r#"
pub struct JSValue;
pub struct GlobalObject;
pub struct Options;
pub struct StringOrBuffer;

impl StringOrBuffer {
    pub fn from_js(_global: &mut GlobalObject, _value: JSValue) -> Result<Self, ()> {
        Ok(Self)
    }

    pub fn byte_slice(&self) -> &[u8] {
        &[]
    }
}

impl Options {
    pub fn get(&self, _global: &mut GlobalObject, _name: &str) -> Result<i32, ()> {
        Ok(1)
    }
}

pub fn zstd_sync(
    global: &mut GlobalObject,
    arg0: JSValue,
    options: Options,
) -> Result<usize, ()> {
    let level = options.get(global, "level")?;
    let input = StringOrBuffer::from_js(global, arg0)?;
    let bytes = input.byte_slice();
    Ok(bytes.len() + level as usize)
}
"#,
        )?;

        assert!(
            output.cards.is_empty(),
            "parsing options before descriptor capture should not trigger the reentry heuristic"
        );
        Ok(())
    }

    #[test]
    fn js_buffer_reentry_fixture_pins_sync_compression_card() -> Result<(), String> {
        let output = fixture_output("js_buffer_reentry_sync_compression")?;
        let card = single_card("js_buffer_reentry_sync_compression", &output)?;

        assert_eq!(
            card.operation.family,
            OperationFamily::StableByteSourceGetterReentry
        );
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert_eq!(card.proof_path, ProofPath::ObservableRedGreen);
        assert_eq!(card.site.owner.as_deref(), Some("zstd_sync"));
        assert!(card.hazards.contains(&HazardKind::StableByteSource));
        assert!(
            card.operation
                .expression
                .contains("stable-byte-source-getter-reentry")
        );
        assert!(
            card.next_action
                .summary
                .contains("observable-red-green proof path")
        );
        assert!(
            card.routes
                .iter()
                .any(|route| route.kind == WitnessKind::HumanDeepReview)
        );
        Ok(())
    }

    #[test]
    fn js_buffer_reentry_fixture_pins_async_helper_capture_card() -> Result<(), String> {
        let output = fixture_output("js_buffer_reentry_async_helper_capture")?;
        let card = single_card("js_buffer_reentry_async_helper_capture", &output)?;

        assert_eq!(
            card.operation.family,
            OperationFamily::StableByteSourceRabAsync
        );
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert_eq!(card.proof_path, ProofPath::ObservableRedGreen);
        assert_eq!(card.site.owner.as_deref(), Some("async_rab_input"));
        assert!(
            card.operation
                .expression
                .contains("stable-byte-source-rab-async")
        );
        assert!(
            card.operation
                .expression
                .contains("from_js_maybe_async_into")
        );
        assert!(card.operation.expression.contains("callback.call"));
        assert!(card.operation.expression.contains("finish_async_input"));
        assert!(
            card.next_action
                .summary
                .contains("observable-red-green proof path")
        );
        assert!(
            card.routes
                .iter()
                .any(|route| route.kind == WitnessKind::HumanDeepReview)
        );
        Ok(())
    }

    #[test]
    fn js_buffer_reentry_fixture_pins_node_fs_rab_scalar_write_card() -> Result<(), String> {
        let output = fixture_output("js_buffer_reentry_node_fs_rab_scalar_write")?;
        let card = single_card("js_buffer_reentry_node_fs_rab_scalar_write", &output)?;

        assert_eq!(
            card.operation.family,
            OperationFamily::StableByteSourceRabAsync
        );
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert_eq!(card.proof_path, ProofPath::ObservableRedGreen);
        assert_eq!(card.site.owner.as_deref(), Some("node_fs_rab_scalar_write"));
        assert_eq!(card.site.location.line, 36);
        assert!(
            card.operation
                .expression
                .contains("stable-byte-source-rab-async")
        );
        assert!(card.operation.expression.contains("dispatch_async_worker"));
        assert!(card.operation.expression.contains("write_scalar_worker"));
        assert!(
            card.next_action
                .summary
                .contains("observable-red-green proof path")
        );
        Ok(())
    }

    #[test]
    fn js_buffer_reentry_fixture_pins_node_fs_rab_encoded_write_card() -> Result<(), String> {
        let output = fixture_output("js_buffer_reentry_node_fs_rab_encoded_write_file")?;
        let card = single_card("js_buffer_reentry_node_fs_rab_encoded_write_file", &output)?;

        assert_eq!(
            card.operation.family,
            OperationFamily::StableByteSourceRabAsync
        );
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert_eq!(card.proof_path, ProofPath::ObservableRedGreen);
        assert_eq!(
            card.site.owner.as_deref(),
            Some("node_fs_rab_encoded_write_file")
        );
        assert_eq!(card.site.location.line, 42);
        assert!(
            card.operation
                .expression
                .contains("stable-byte-source-rab-async")
        );
        assert!(
            card.operation
                .expression
                .contains("from_js_with_encoding_maybe_async_into")
        );
        assert!(card.operation.expression.contains("dispatch_async_worker"));
        assert!(card.operation.expression.contains("write_file_worker"));
        Ok(())
    }

    #[test]
    fn js_buffer_reentry_fixture_pins_raw_parts_materialization_card() -> Result<(), String> {
        let output = fixture_output("js_buffer_reentry_raw_parts_materialization")?;
        assert_eq!(output.cards.len(), 2);
        let raw_parts_card = output
            .cards
            .iter()
            .find(|card| card.operation.family == OperationFamily::SliceFromRawParts)
            .ok_or_else(|| "fixture should retain the raw-parts operation card".to_string())?;
        let reentry_card = output
            .cards
            .iter()
            .find(|card| card.operation.family == OperationFamily::StableByteSourceGetterReentry)
            .ok_or_else(|| "fixture should emit the JS-backed reentry card".to_string())?;

        assert_eq!(raw_parts_card.site.owner.as_deref(), Some("zstd_raw_parts"));
        assert_eq!(reentry_card.site.owner.as_deref(), Some("zstd_raw_parts"));
        assert_eq!(raw_parts_card.site.location.line, 32);
        assert_eq!(reentry_card.site.location.line, 32);
        assert!(
            raw_parts_card
                .operation
                .expression
                .contains("core::slice::from_raw_parts")
        );
        assert!(
            reentry_card
                .operation
                .expression
                .contains("stable-byte-source-getter-reentry")
        );
        assert!(
            reentry_card
                .operation
                .expression
                .contains("core::slice::from_raw_parts")
        );
        assert!(reentry_card.operation.expression.contains("options.get"));
        assert!(
            reentry_card
                .next_action
                .summary
                .contains("observable-red-green proof path")
        );
        assert!(
            reentry_card
                .routes
                .iter()
                .any(|route| route.kind == WitnessKind::HumanDeepReview)
        );
        Ok(())
    }

    #[test]
    fn js_buffer_reentry_fixture_pins_as_array_buffer_coercion_card() -> Result<(), String> {
        let output = fixture_output("js_buffer_reentry_coerce_after_as_array_buffer")?;
        let card = single_card("js_buffer_reentry_coerce_after_as_array_buffer", &output)?;

        assert_eq!(
            card.operation.family,
            OperationFamily::StableByteSourceGetterReentry
        );
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert_eq!(card.proof_path, ProofPath::ObservableRedGreen);
        assert_eq!(card.site.owner.as_deref(), Some("index_of_line"));
        assert_eq!(card.site.location.line, 30);
        assert!(card.hazards.contains(&HazardKind::StableByteSource));
        assert!(
            card.operation
                .expression
                .contains("stable-byte-source-getter-reentry")
        );
        assert!(card.operation.expression.contains("as_array_buffer"));
        assert!(card.operation.expression.contains("coerce_to_int64"));
        assert!(card.operation.expression.contains("byte_slice"));
        assert!(
            card.next_action
                .summary
                .contains("observable-red-green proof path")
        );
        Ok(())
    }

    #[test]
    fn js_buffer_reentry_fixture_pins_vector_materialization_card() -> Result<(), String> {
        let output = fixture_output("js_buffer_reentry_vector_materialization")?;
        let vector_card = single_card("js_buffer_reentry_vector_materialization", &output)?;

        assert_eq!(
            vector_card.operation.family,
            OperationFamily::StableByteSourceGetterReentry
        );
        assert_eq!(vector_card.class, ReviewClass::GuardMissing);
        assert_eq!(vector_card.proof_path, ProofPath::ObservableRedGreen);
        assert_eq!(vector_card.site.owner.as_deref(), Some("vector_route"));
        assert_eq!(vector_card.site.location.line, 30);
        assert!(vector_card.operation.expression.contains("vector"));
        assert!(vector_card.operation.expression.contains("as_array_buffer"));
        assert!(vector_card.operation.expression.contains("coerce_to_int64"));
        assert!(
            vector_card
                .routes
                .iter()
                .any(|route| route.kind == WitnessKind::HumanDeepReview)
        );
        Ok(())
    }

    #[test]
    fn js_buffer_reentry_fixture_pins_as_ptr_materialization_card() -> Result<(), String> {
        let output = fixture_output("js_buffer_reentry_as_ptr_materialization")?;
        let as_ptr_card = single_card("js_buffer_reentry_as_ptr_materialization", &output)?;

        assert_eq!(
            as_ptr_card.operation.family,
            OperationFamily::StableByteSourceGetterReentry
        );
        assert_eq!(as_ptr_card.class, ReviewClass::GuardMissing);
        assert_eq!(as_ptr_card.proof_path, ProofPath::ObservableRedGreen);
        assert_eq!(as_ptr_card.site.owner.as_deref(), Some("pointer_route"));
        assert_eq!(as_ptr_card.site.location.line, 30);
        assert!(as_ptr_card.operation.expression.contains("as_ptr"));
        assert!(as_ptr_card.operation.expression.contains("as_array_buffer"));
        assert!(as_ptr_card.operation.expression.contains("coerce_to_int64"));
        assert!(
            as_ptr_card
                .routes
                .iter()
                .any(|route| route.kind == WitnessKind::HumanDeepReview)
        );
        Ok(())
    }

    #[test]
    fn js_buffer_reentry_fixture_keeps_options_before_capture_no_card() -> Result<(), String> {
        let output = fixture_output("js_buffer_reentry_options_before_capture_no_card")?;

        assert!(
            output.cards.is_empty(),
            "parsing options before descriptor capture should stay a no-card control"
        );
        Ok(())
    }

    #[test]
    fn js_buffer_reentry_fixture_keeps_async_options_before_capture_no_card() -> Result<(), String>
    {
        let output = fixture_output("js_buffer_reentry_async_options_before_capture_no_card")?;

        assert!(
            output.cards.is_empty(),
            "callback reentry before async descriptor capture should stay a no-card control"
        );
        Ok(())
    }

    #[test]
    fn js_buffer_reentry_fixture_keeps_node_fs_schedule_before_capture_no_card()
    -> Result<(), String> {
        let output = fixture_output(
            "js_buffer_reentry_node_fs_rab_scalar_write_scheduled_before_capture_no_card",
        )?;

        assert!(
            output.cards.is_empty(),
            "async scheduling before scalar write capture should stay a no-card control"
        );
        Ok(())
    }

    #[test]
    fn js_buffer_reentry_fixture_keeps_node_fs_encoded_recapture_after_dispatch_no_card()
    -> Result<(), String> {
        let output = fixture_output(
            "js_buffer_reentry_node_fs_rab_encoded_write_recapture_after_dispatch_no_card",
        )?;

        assert!(
            output.cards.is_empty(),
            "encoded write input recaptured after dispatch should stay a no-card control"
        );
        Ok(())
    }

    #[test]
    fn js_buffer_reentry_fixture_keeps_recapture_after_reentry_no_card() -> Result<(), String> {
        let output = fixture_output("js_buffer_reentry_recapture_after_reentry_no_card")?;

        assert!(
            output.cards.is_empty(),
            "materialization of a descriptor recaptured after reentry should stay a no-card control"
        );
        Ok(())
    }

    #[test]
    fn js_buffer_reentry_fixture_keeps_refetch_after_coercion_no_card() -> Result<(), String> {
        let output = fixture_output("js_buffer_reentry_refetch_after_coercion_no_card")?;

        assert!(
            output.cards.is_empty(),
            "re-fetching an ArrayBuffer descriptor after coercion should stay a no-card control"
        );
        Ok(())
    }

    #[test]
    fn js_buffer_reentry_fixture_keeps_vector_refetch_after_coercion_no_card() -> Result<(), String>
    {
        let output = fixture_output("js_buffer_reentry_vector_refetch_after_coercion_no_card")?;

        assert!(
            output.cards.is_empty(),
            "vector materialization of a re-fetched descriptor after coercion should stay a no-card control"
        );
        Ok(())
    }

    #[test]
    fn js_buffer_reentry_fixture_keeps_as_ptr_refetch_after_coercion_no_card() -> Result<(), String>
    {
        let output = fixture_output("js_buffer_reentry_as_ptr_refetch_after_coercion_no_card")?;

        assert!(
            output.cards.is_empty(),
            "as_ptr materialization of a re-fetched descriptor after coercion should stay a no-card control"
        );
        Ok(())
    }

    #[test]
    fn js_buffer_reentry_fixture_keeps_async_recapture_after_reentry_no_card() -> Result<(), String>
    {
        let output = fixture_output("js_buffer_reentry_async_recapture_after_reentry_no_card")?;

        assert!(
            output.cards.is_empty(),
            "materialization of an async descriptor recaptured after reentry should stay a no-card control"
        );
        Ok(())
    }

    #[test]
    fn stable_byte_sab_fixture_pins_borrowed_slice_card() -> Result<(), String> {
        let output = fixture_output("stable_byte_sab_borrowed_slice")?;
        assert_eq!(output.cards.len(), 2);
        let raw_parts_card = output
            .cards
            .iter()
            .find(|card| card.operation.family == OperationFamily::SliceFromRawParts)
            .ok_or_else(|| "fixture should retain the raw-parts operation card".to_string())?;
        let stable_byte_card = output
            .cards
            .iter()
            .find(|card| card.operation.family == OperationFamily::StableByteSourceSabRace)
            .ok_or_else(|| "fixture should emit the SAB stable-byte card".to_string())?;

        assert_eq!(
            raw_parts_card.site.owner.as_deref(),
            Some("textdecoder_sab_decode")
        );
        assert_eq!(
            stable_byte_card.site.owner.as_deref(),
            Some("textdecoder_sab_decode")
        );
        assert_eq!(raw_parts_card.site.location.line, 23);
        assert_eq!(stable_byte_card.site.location.line, 23);
        assert_eq!(stable_byte_card.class, ReviewClass::GuardMissing);
        assert_eq!(stable_byte_card.proof_path, ProofPath::MutationMiriModel);
        assert!(
            stable_byte_card
                .operation
                .expression
                .contains("stable-byte-source-sab-race")
        );
        assert!(
            stable_byte_card
                .next_action
                .summary
                .contains("mutation-plus-Miri/model proof path")
        );
        Ok(())
    }

    #[test]
    fn stable_byte_sab_fixture_pins_mysql_blob_rawslice_card() -> Result<(), String> {
        let output = fixture_output("stable_byte_sab_mysql_blob_rawslice")?;
        assert_eq!(output.cards.len(), 2);
        let raw_parts_card = output
            .cards
            .iter()
            .find(|card| card.operation.family == OperationFamily::SliceFromRawParts)
            .ok_or_else(|| "fixture should retain the raw-parts operation card".to_string())?;
        let stable_byte_card = output
            .cards
            .iter()
            .find(|card| card.operation.family == OperationFamily::StableByteSourceSabRace)
            .ok_or_else(|| "fixture should emit the SAB stable-byte card".to_string())?;

        assert_eq!(
            raw_parts_card.site.owner.as_deref(),
            Some("mysql_blob_sab_bind")
        );
        assert_eq!(
            stable_byte_card.site.owner.as_deref(),
            Some("mysql_blob_sab_bind")
        );
        assert_eq!(raw_parts_card.site.location.line, 30);
        assert_eq!(stable_byte_card.site.location.line, 30);
        assert_eq!(stable_byte_card.class, ReviewClass::GuardMissing);
        assert_eq!(stable_byte_card.proof_path, ProofPath::MutationMiriModel);
        assert!(
            stable_byte_card
                .operation
                .expression
                .contains("stable-byte-source-sab-race")
        );
        assert!(
            stable_byte_card
                .next_action
                .summary
                .contains("mutation-plus-Miri/model proof path")
        );
        Ok(())
    }

    #[test]
    fn stable_byte_sab_fixture_keeps_snapshot_no_card() -> Result<(), String> {
        let output = fixture_output("stable_byte_sab_snapshot_no_card")?;

        assert!(
            output.cards.is_empty(),
            "copying shared bytes into owned storage should stay a no-card control"
        );
        Ok(())
    }

    #[test]
    fn stable_byte_sab_fixture_keeps_mysql_blob_owned_copy_no_card() -> Result<(), String> {
        let output = fixture_output("stable_byte_sab_mysql_blob_owned_copy_no_card")?;

        assert!(
            output.cards.is_empty(),
            "copying MySQL BLOB shared bytes into owned storage should stay a no-card control"
        );
        Ok(())
    }

    #[test]
    fn stable_byte_native_ffi_after_js_capture_emits_advisory_card() -> Result<(), String> {
        let output = temp_source_output(
            "unsafe-review-stable-byte-native-ffi",
            r#"
pub struct JSValue;
pub struct GlobalObject;
pub struct JSArrayBufferView {
    ptr: *const u8,
    len: usize,
}

impl JSArrayBufferView {
    pub fn from_js(_global: &mut GlobalObject, _value: JSValue) -> Result<Self, ()> {
        Ok(Self {
            ptr: core::ptr::null(),
            len: 0,
        })
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.ptr
    }

    pub fn len(&self) -> usize {
        self.len
    }
}

unsafe extern "C" {
    fn zstd_compress_into(
        src: *const u8,
        src_len: usize,
        dst: *mut u8,
        dst_len: usize,
    ) -> usize;
}

pub fn zstd_overlap_handoff(
    global: &mut GlobalObject,
    value: JSValue,
    output: &mut [u8],
) -> Result<usize, ()> {
    let input = JSArrayBufferView::from_js(global, value)?;
    Ok(unsafe {
        zstd_compress_into(input.as_ptr(), input.len(), output.as_mut_ptr(), output.len())
    })
}
"#,
        )?;

        let stable_byte_card = output
            .cards
            .iter()
            .find(|card| card.operation.family == OperationFamily::StableByteSourceNativeFfiRead)
            .ok_or_else(|| "native FFI stable-byte card should be emitted".to_string())?;

        assert_eq!(stable_byte_card.class, ReviewClass::GuardMissing);
        assert_eq!(stable_byte_card.proof_path, ProofPath::ObservableRedGreen);
        assert_eq!(
            stable_byte_card.site.owner.as_deref(),
            Some("zstd_overlap_handoff")
        );
        assert!(
            stable_byte_card
                .operation
                .expression
                .contains("stable-byte-source-native-ffi-read")
        );
        assert!(
            stable_byte_card
                .operation
                .expression
                .contains("JSArrayBufferView::from_js")
        );
        assert!(
            stable_byte_card
                .operation
                .expression
                .contains("zstd_compress_into")
        );
        assert!(
            stable_byte_card
                .hazards
                .contains(&HazardKind::StableByteSource)
        );
        assert!(
            stable_byte_card
                .next_action
                .summary
                .contains("observable-red-green proof path")
        );
        assert!(
            stable_byte_card
                .routes
                .iter()
                .any(|route| route.kind == WitnessKind::HumanDeepReview)
        );
        Ok(())
    }

    #[test]
    fn stable_byte_native_ffi_fixture_pins_zstd_handoff_card() -> Result<(), String> {
        let output = fixture_output("stable_byte_native_ffi_zstd_handoff")?;
        assert_eq!(output.cards.len(), 2);
        let ffi_card = output
            .cards
            .iter()
            .find(|card| card.operation.family == OperationFamily::Ffi)
            .ok_or_else(|| "fixture should retain the generic FFI card".to_string())?;
        let stable_byte_card = output
            .cards
            .iter()
            .find(|card| card.operation.family == OperationFamily::StableByteSourceNativeFfiRead)
            .ok_or_else(|| "fixture should emit the native FFI stable-byte card".to_string())?;

        assert_eq!(ffi_card.site.owner.as_deref(), Some("zstd_overlap_handoff"));
        assert_eq!(
            stable_byte_card.site.owner.as_deref(),
            Some("zstd_overlap_handoff")
        );
        assert_eq!(stable_byte_card.class, ReviewClass::GuardMissing);
        assert_eq!(stable_byte_card.proof_path, ProofPath::ObservableRedGreen);
        assert!(
            stable_byte_card
                .operation
                .expression
                .contains("stable-byte-source-native-ffi-read")
        );
        assert!(
            stable_byte_card
                .next_action
                .summary
                .contains("native FFI aperture")
        );
        Ok(())
    }

    #[test]
    fn stable_byte_native_ffi_fixture_keeps_owned_copy_control_to_ffi_only() -> Result<(), String> {
        let output = fixture_output("stable_byte_native_ffi_zstd_owned_copy_control")?;
        assert_eq!(output.cards.len(), 1);
        assert!(
            output.cards.iter().all(|card| card.operation.family
                != OperationFamily::StableByteSourceNativeFfiRead),
            "owned copy before native FFI should not trigger the native FFI stable-byte heuristic"
        );
        assert!(
            output
                .cards
                .iter()
                .any(|card| card.operation.family == OperationFamily::Ffi),
            "owned-copy control should still retain the generic FFI seam card"
        );
        Ok(())
    }

    #[test]
    fn panic_from_safe_js_direct_try_from_expect_emits_guard_missing_card() -> Result<(), String> {
        let output = temp_source_output(
            "unsafe-review-panic-from-safe-js-direct",
            r#"
pub struct JSValue;

impl JSValue {
    pub fn to_int32(&self) -> i32 {
        0
    }
}

pub fn read_at(arguments: &[JSValue]) -> Result<usize, ()> {
    let offset = usize::try_from(arguments[0].to_int32()).expect("offset");
    Ok(offset)
}
"#,
        )?;
        let card = single_card("panic_from_safe_js_direct", &output)?;

        assert_eq!(card.operation.family, OperationFamily::PanicFromSafeJs);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert_eq!(card.site.owner.as_deref(), Some("read_at"));
        assert!(card.hazards.contains(&HazardKind::PanicSafety));
        assert!(!obligation_discharge_present(card, "panic-guard"));
        assert!(
            card.missing
                .iter()
                .all(|missing| missing.kind != "contract"),
            "panic-from-safe-JS cards should ask for guards, not unsafe API safety docs"
        );
        assert_eq!(card.proof_path, crate::domain::ProofPath::HumanReviewOnly);
        assert!(
            card.next_action.summary.contains("sign/range guard"),
            "next action should steer toward a safe JS boundary"
        );
        Ok(())
    }

    #[test]
    fn panic_from_safe_js_bound_try_from_unwrap_emits_guard_missing_card() -> Result<(), String> {
        let output = temp_source_output(
            "unsafe-review-panic-from-safe-js-bound",
            r#"
pub struct JSValue;

impl JSValue {
    pub fn to_int32(&self) -> i32 {
        0
    }
}

pub fn resize(arguments: &[JSValue]) -> Result<usize, ()> {
    let new_len = arguments[0].to_int32();
    let new_len = usize::try_from(new_len).unwrap();
    Ok(new_len)
}
"#,
        )?;
        let card = single_card("panic_from_safe_js_bound", &output)?;

        assert_eq!(card.operation.family, OperationFamily::PanicFromSafeJs);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert_eq!(card.site.owner.as_deref(), Some("resize"));
        assert!(card.operation.expression.contains("new_len"));
        Ok(())
    }

    #[test]
    fn panic_from_safe_js_return_guard_emits_no_card() -> Result<(), String> {
        let output = temp_source_output(
            "unsafe-review-panic-from-safe-js-return-guard",
            r#"
pub struct JSValue;

impl JSValue {
    pub fn to_int32(&self) -> i32 {
        0
    }
}

pub fn read_at(arguments: &[JSValue]) -> Result<usize, ()> {
    let offset = arguments[0].to_int32();
    if offset < 0 { return Err(()); }
    let offset = usize::try_from(offset).expect("offset");
    Ok(offset)
}
"#,
        )?;

        assert!(
            output.cards.is_empty(),
            "an explicit negative-value error return should satisfy the local panic guard"
        );
        Ok(())
    }

    #[test]
    fn panic_from_safe_js_inline_max_guard_emits_no_card() -> Result<(), String> {
        let output = temp_source_output(
            "unsafe-review-panic-from-safe-js-inline-max",
            r#"
pub struct JSValue;

impl JSValue {
    pub fn to_int32(&self) -> i32 {
        0
    }
}

pub fn read_at(arguments: &[JSValue]) -> Result<usize, ()> {
    let offset = usize::try_from(arguments[0].to_int32().max(0)).expect("offset");
    Ok(offset)
}
"#,
        )?;

        assert!(
            output.cards.is_empty(),
            "inline nonnegative clamping should stay a no-card control"
        );
        Ok(())
    }

    #[test]
    fn panic_from_safe_js_observed_negative_branch_still_emits_card() -> Result<(), String> {
        let output = temp_source_output(
            "unsafe-review-panic-from-safe-js-observed-only",
            r#"
pub struct JSValue;

impl JSValue {
    pub fn to_int32(&self) -> i32 {
        0
    }
}

pub fn read_at(arguments: &[JSValue]) -> Result<usize, ()> {
    let offset = arguments[0].to_int32();
    if offset < 0 { record_negative(offset); }
    let offset = usize::try_from(offset).expect("offset");
    Ok(offset)
}

fn record_negative(_offset: i32) {}
"#,
        )?;
        let card = single_card("panic_from_safe_js_observed_only", &output)?;

        assert_eq!(card.operation.family, OperationFamily::PanicFromSafeJs);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        Ok(())
    }

    #[test]
    fn panic_from_safe_js_non_js_signed_local_emits_no_card() -> Result<(), String> {
        let output = temp_source_output(
            "unsafe-review-panic-from-safe-js-non-js",
            r#"
pub fn read_at(offset: i32) -> Result<usize, ()> {
    let offset = usize::try_from(offset).expect("offset");
    Ok(offset)
}
"#,
        )?;

        assert!(
            output.cards.is_empty(),
            "ordinary signed Rust locals are outside the JS-derived safe-caller heuristic"
        );
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
                "drop_in_place_reassigned_origin_not_guard",
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
    fn copy_range_evidence_rejects_unrelated_length_guards() -> Result<(), String> {
        for fixture in [
            "copy_nonoverlapping_other_len_not_guard",
            "ptr_copy_other_len_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert!(
                !obligation_discharge_present(card, "valid-range"),
                "{fixture} should not accept an unrelated length assertion as copy range evidence"
            );
        }
        Ok(())
    }

    #[test]
    fn copy_range_evidence_rejects_one_sided_slice_length_guards() -> Result<(), String> {
        for fixture in [
            "copy_nonoverlapping_slice_range_src_only_not_guard",
            "copy_nonoverlapping_slice_range_dst_only_not_guard",
            "ptr_copy_slice_range_src_only_not_guard",
            "ptr_copy_slice_range_dst_only_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert!(
                !obligation_discharge_present(card, "valid-range"),
                "{fixture} should require both source and destination slice range evidence"
            );
        }
        Ok(())
    }

    #[test]
    fn copy_range_evidence_rejects_closed_slice_length_branches() -> Result<(), String> {
        for fixture in [
            "copy_nonoverlapping_slice_range_closed_branch_not_guard",
            "ptr_copy_slice_range_closed_branch_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert!(
                !obligation_discharge_present(card, "valid-range"),
                "{fixture} should not accept closed branch observations as active range evidence"
            );
        }
        Ok(())
    }

    #[test]
    fn copy_range_evidence_rejects_or_slice_length_branches() -> Result<(), String> {
        for fixture in [
            "copy_nonoverlapping_slice_range_or_branch_not_guard",
            "ptr_copy_slice_range_or_branch_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert!(
                !obligation_discharge_present(card, "valid-range"),
                "{fixture} should not accept disjunctive range branches as full range evidence"
            );
        }
        Ok(())
    }

    #[test]
    fn copy_range_evidence_rejects_comment_only_early_returns() -> Result<(), String> {
        for fixture in [
            "copy_nonoverlapping_slice_range_disjunctive_early_return_block_comment_not_guard",
            "ptr_copy_slice_range_disjunctive_early_return_block_comment_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert!(
                !obligation_discharge_present(card, "valid-range"),
                "{fixture} should not accept commented return text as copy range evidence"
            );
        }
        Ok(())
    }

    #[test]
    fn copy_range_evidence_accepts_slice_length_guards() -> Result<(), String> {
        let copy_nonoverlapping = fixture_output("copy_nonoverlapping_slice_range_guard")?;
        let copy_nonoverlapping_card = single_card(
            "copy_nonoverlapping_slice_range_guard",
            &copy_nonoverlapping,
        )?;
        assert!(obligation_discharge_present(
            copy_nonoverlapping_card,
            "valid-range"
        ));
        assert!(
            !obligation_discharge_present(copy_nonoverlapping_card, "non-overlap"),
            "slice range guards should not prove non-overlap"
        );

        let ptr_copy = fixture_output("ptr_copy_slice_range_guard")?;
        let ptr_copy_card = single_card("ptr_copy_slice_range_guard", &ptr_copy)?;
        assert!(obligation_discharge_present(ptr_copy_card, "valid-range"));
        assert!(
            !obligation_discharge_present(ptr_copy_card, "initialized"),
            "range guards should not prove initialized memory"
        );
        Ok(())
    }

    #[test]
    fn copy_range_evidence_accepts_conjunctive_slice_length_guards() -> Result<(), String> {
        for (fixture, missing_key) in [
            (
                "copy_nonoverlapping_slice_range_conjunctive_assert_guard",
                "non-overlap",
            ),
            (
                "copy_nonoverlapping_slice_range_conjunctive_open_branch_guard",
                "non-overlap",
            ),
            (
                "ptr_copy_slice_range_conjunctive_assert_guard",
                "initialized",
            ),
            (
                "ptr_copy_slice_range_conjunctive_open_branch_guard",
                "initialized",
            ),
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;
            assert!(
                obligation_discharge_present(card, "valid-range"),
                "{fixture} should accept conjunctive source/destination slice range evidence"
            );
            assert!(
                !obligation_discharge_present(card, missing_key),
                "{fixture} should not prove {missing_key}"
            );
        }
        Ok(())
    }

    #[test]
    fn copy_range_evidence_accepts_slice_length_early_returns() -> Result<(), String> {
        let copy_nonoverlapping =
            fixture_output("copy_nonoverlapping_slice_range_early_return_guard")?;
        let copy_nonoverlapping_card = single_card(
            "copy_nonoverlapping_slice_range_early_return_guard",
            &copy_nonoverlapping,
        )?;
        assert!(obligation_discharge_present(
            copy_nonoverlapping_card,
            "valid-range"
        ));
        assert!(
            !obligation_discharge_present(copy_nonoverlapping_card, "non-overlap"),
            "early-return range guards should not prove non-overlap"
        );

        let ptr_copy = fixture_output("ptr_copy_slice_range_early_return_guard")?;
        let ptr_copy_card = single_card("ptr_copy_slice_range_early_return_guard", &ptr_copy)?;
        assert!(obligation_discharge_present(ptr_copy_card, "valid-range"));
        assert!(
            !obligation_discharge_present(ptr_copy_card, "initialized"),
            "early-return range guards should not prove initialized memory"
        );
        Ok(())
    }

    #[test]
    fn copy_range_evidence_accepts_disjunctive_slice_length_early_returns() -> Result<(), String> {
        for (fixture, missing_key) in [
            (
                "copy_nonoverlapping_slice_range_disjunctive_early_return_guard",
                "non-overlap",
            ),
            (
                "ptr_copy_slice_range_disjunctive_early_return_guard",
                "initialized",
            ),
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;
            assert!(
                obligation_discharge_present(card, "valid-range"),
                "{fixture} should accept disjunctive invalid-range early returns as range evidence"
            );
            assert!(
                !obligation_discharge_present(card, missing_key),
                "{fixture} should not prove {missing_key}"
            );
        }
        Ok(())
    }

    #[test]
    fn copy_range_evidence_accepts_open_slice_length_branches() -> Result<(), String> {
        let copy_nonoverlapping =
            fixture_output("copy_nonoverlapping_slice_range_open_branch_guard")?;
        let copy_nonoverlapping_card = single_card(
            "copy_nonoverlapping_slice_range_open_branch_guard",
            &copy_nonoverlapping,
        )?;
        assert!(obligation_discharge_present(
            copy_nonoverlapping_card,
            "valid-range"
        ));
        assert!(
            !obligation_discharge_present(copy_nonoverlapping_card, "non-overlap"),
            "open-branch range guards should not prove non-overlap"
        );

        let ptr_copy = fixture_output("ptr_copy_slice_range_open_branch_guard")?;
        let ptr_copy_card = single_card("ptr_copy_slice_range_open_branch_guard", &ptr_copy)?;
        assert!(obligation_discharge_present(ptr_copy_card, "valid-range"));
        assert!(
            !obligation_discharge_present(ptr_copy_card, "initialized"),
            "open-branch range guards should not prove initialized memory"
        );
        Ok(())
    }

    #[test]
    fn copy_range_evidence_rejects_stale_slice_length_guards() -> Result<(), String> {
        for fixture in [
            "copy_nonoverlapping_slice_range_open_branch_reassigned_count_not_guard",
            "copy_nonoverlapping_slice_range_open_branch_compound_reassigned_count_not_guard",
            "copy_nonoverlapping_slice_range_open_branch_shadowed_count_not_guard",
            "copy_nonoverlapping_slice_range_open_branch_reassigned_src_not_guard",
            "copy_nonoverlapping_slice_range_open_branch_reassigned_src_path_not_guard",
            "copy_nonoverlapping_slice_range_open_branch_shadowed_src_path_not_guard",
            "copy_nonoverlapping_slice_range_open_branch_shadowed_src_not_guard",
            "copy_nonoverlapping_slice_range_open_branch_reassigned_dst_not_guard",
            "copy_nonoverlapping_slice_range_open_branch_reassigned_dst_path_not_guard",
            "copy_nonoverlapping_slice_range_open_branch_shadowed_dst_path_not_guard",
            "copy_nonoverlapping_slice_range_open_branch_shadowed_dst_not_guard",
            "copy_nonoverlapping_slice_range_disjunctive_early_return_reassigned_count_not_guard",
            "copy_nonoverlapping_slice_range_disjunctive_early_return_compound_reassigned_count_not_guard",
            "copy_nonoverlapping_slice_range_disjunctive_early_return_shadowed_count_not_guard",
            "copy_nonoverlapping_slice_range_disjunctive_early_return_reassigned_src_not_guard",
            "copy_nonoverlapping_slice_range_disjunctive_early_return_reassigned_src_path_not_guard",
            "copy_nonoverlapping_slice_range_disjunctive_early_return_shadowed_src_path_not_guard",
            "copy_nonoverlapping_slice_range_disjunctive_early_return_shadowed_src_not_guard",
            "copy_nonoverlapping_slice_range_disjunctive_early_return_reassigned_dst_not_guard",
            "copy_nonoverlapping_slice_range_disjunctive_early_return_reassigned_dst_path_not_guard",
            "copy_nonoverlapping_slice_range_disjunctive_early_return_shadowed_dst_path_not_guard",
            "copy_nonoverlapping_slice_range_disjunctive_early_return_shadowed_dst_not_guard",
            "copy_nonoverlapping_slice_range_reassigned_count_not_guard",
            "copy_nonoverlapping_slice_range_shadowed_count_not_guard",
            "copy_nonoverlapping_slice_range_reassigned_src_not_guard",
            "copy_nonoverlapping_slice_range_reassigned_src_path_not_guard",
            "copy_nonoverlapping_slice_range_shadowed_src_path_not_guard",
            "copy_nonoverlapping_slice_range_shadowed_src_not_guard",
            "copy_nonoverlapping_slice_range_reassigned_dst_not_guard",
            "copy_nonoverlapping_slice_range_reassigned_dst_path_not_guard",
            "copy_nonoverlapping_slice_range_shadowed_dst_path_not_guard",
            "copy_nonoverlapping_slice_range_shadowed_dst_not_guard",
            "ptr_copy_slice_range_open_branch_reassigned_count_not_guard",
            "ptr_copy_slice_range_open_branch_reassigned_dst_not_guard",
            "ptr_copy_slice_range_open_branch_reassigned_src_path_not_guard",
            "ptr_copy_slice_range_open_branch_reassigned_dst_path_not_guard",
            "ptr_copy_slice_range_open_branch_shadowed_src_path_not_guard",
            "ptr_copy_slice_range_open_branch_shadowed_dst_path_not_guard",
            "ptr_copy_slice_range_disjunctive_early_return_reassigned_count_not_guard",
            "ptr_copy_slice_range_disjunctive_early_return_compound_reassigned_count_not_guard",
            "ptr_copy_slice_range_disjunctive_early_return_shadowed_count_not_guard",
            "ptr_copy_slice_range_disjunctive_early_return_reassigned_src_not_guard",
            "ptr_copy_slice_range_disjunctive_early_return_reassigned_src_path_not_guard",
            "ptr_copy_slice_range_disjunctive_early_return_shadowed_src_path_not_guard",
            "ptr_copy_slice_range_disjunctive_early_return_shadowed_src_not_guard",
            "ptr_copy_slice_range_disjunctive_early_return_reassigned_dst_not_guard",
            "ptr_copy_slice_range_disjunctive_early_return_reassigned_dst_path_not_guard",
            "ptr_copy_slice_range_disjunctive_early_return_shadowed_dst_path_not_guard",
            "ptr_copy_slice_range_disjunctive_early_return_shadowed_dst_not_guard",
            "ptr_copy_slice_range_reassigned_count_not_guard",
            "ptr_copy_slice_range_shadowed_count_not_guard",
            "ptr_copy_slice_range_open_branch_reassigned_src_not_guard",
            "ptr_copy_slice_range_open_branch_compound_reassigned_count_not_guard",
            "ptr_copy_slice_range_open_branch_shadowed_count_not_guard",
            "ptr_copy_slice_range_open_branch_shadowed_src_not_guard",
            "ptr_copy_slice_range_open_branch_shadowed_dst_not_guard",
            "ptr_copy_slice_range_reassigned_src_not_guard",
            "ptr_copy_slice_range_reassigned_src_path_not_guard",
            "ptr_copy_slice_range_shadowed_src_path_not_guard",
            "ptr_copy_slice_range_shadowed_src_not_guard",
            "ptr_copy_slice_range_reassigned_dst_not_guard",
            "ptr_copy_slice_range_reassigned_dst_path_not_guard",
            "ptr_copy_slice_range_shadowed_dst_path_not_guard",
            "ptr_copy_slice_range_shadowed_dst_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert!(
                !obligation_discharge_present(card, "valid-range"),
                "{fixture} should not accept stale slice length evidence after reassignment"
            );
        }
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
    fn maybeuninit_assume_init_accepts_same_slot_initialization_evidence() -> Result<(), String> {
        for fixture in [
            "maybeuninit_assume_init_write_guard",
            "maybeuninit_assume_init_read_write_guard",
            "maybeuninit_assume_init_ref_write_guard",
            "maybeuninit_assume_init_mut_write_guard",
            "maybeuninit_assume_init_drop_write_guard",
            "maybeuninit_assume_init_open_branch_write_guard",
            "maybeuninit_assume_init_read_open_branch_write_guard",
            "maybeuninit_assume_init_ref_open_branch_write_guard",
            "maybeuninit_assume_init_mut_open_branch_write_guard",
            "maybeuninit_assume_init_drop_open_branch_write_guard",
            "maybeuninit_assume_init_open_branch_new_guard",
            "maybeuninit_assume_init_read_open_branch_new_guard",
            "maybeuninit_assume_init_ref_open_branch_new_guard",
            "maybeuninit_assume_init_mut_open_branch_new_guard",
            "maybeuninit_assume_init_drop_open_branch_new_guard",
            "maybeuninit_assume_init_new_guard",
            "maybeuninit_assume_init_read_new_guard",
            "maybeuninit_assume_init_ref_new_guard",
            "maybeuninit_assume_init_mut_method_new_guard",
            "maybeuninit_assume_init_mut_new_guard",
            "maybeuninit_assume_init_drop_new_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(
                card.operation.family,
                OperationFamily::MaybeUninitAssumeInit
            );
            assert_eq!(card.class, ReviewClass::GuardedUnwitnessed);
            assert!(obligation_discharge_present(card, "initialized"));
            assert!(
                card.missing.iter().all(|missing| missing.kind != "guard"),
                "{fixture} should resolve the local initialized-memory guard prompt"
            );
        }
        Ok(())
    }

    #[test]
    fn maybeuninit_assume_init_rejects_non_dominating_or_stale_slot_evidence() -> Result<(), String>
    {
        for fixture in [
            "maybeuninit_assume_init_comment_not_guard",
            "maybeuninit_assume_init_closed_branch_write_not_guard",
            "maybeuninit_assume_init_read_closed_branch_write_not_guard",
            "maybeuninit_assume_init_ref_closed_branch_write_not_guard",
            "maybeuninit_assume_init_mut_closed_branch_write_not_guard",
            "maybeuninit_assume_init_drop_closed_branch_write_not_guard",
            "maybeuninit_assume_init_closed_branch_new_not_guard",
            "maybeuninit_assume_init_read_closed_branch_new_not_guard",
            "maybeuninit_assume_init_ref_closed_branch_new_not_guard",
            "maybeuninit_assume_init_mut_closed_branch_new_not_guard",
            "maybeuninit_assume_init_drop_closed_branch_new_not_guard",
            "maybeuninit_assume_init_other_slot_write_not_guard",
            "maybeuninit_assume_init_stale_write_not_guard",
            "maybeuninit_assume_init_read_stale_write_not_guard",
            "maybeuninit_assume_init_ref_stale_write_not_guard",
            "maybeuninit_assume_init_mut_stale_write_not_guard",
            "maybeuninit_assume_init_drop_stale_write_not_guard",
            "maybeuninit_assume_init_read_other_slot_write_not_guard",
            "maybeuninit_assume_init_ref_other_slot_write_not_guard",
            "maybeuninit_assume_init_mut_other_slot_write_not_guard",
            "maybeuninit_assume_init_drop_other_slot_write_not_guard",
            "maybeuninit_assume_init_stale_field_write_not_guard",
            "maybeuninit_assume_init_stale_new_not_guard",
            "maybeuninit_assume_init_read_stale_new_not_guard",
            "maybeuninit_assume_init_ref_stale_new_not_guard",
            "maybeuninit_assume_init_mut_stale_new_not_guard",
            "maybeuninit_assume_init_drop_stale_new_not_guard",
            "maybeuninit_assume_init_shadowed_slot_not_guard",
            "maybeuninit_assume_init_read_shadowed_slot_not_guard",
            "maybeuninit_assume_init_ref_shadowed_slot_not_guard",
            "maybeuninit_assume_init_mut_shadowed_slot_not_guard",
            "maybeuninit_assume_init_drop_shadowed_slot_not_guard",
            "maybeuninit_assume_init_mutslot_new_not_guard",
            "maybeuninit_assume_init_read_mutslot_new_not_guard",
            "maybeuninit_assume_init_ref_mutslot_new_not_guard",
            "maybeuninit_assume_init_mut_mutslot_new_not_guard",
            "maybeuninit_assume_init_drop_mutslot_new_not_guard",
            "maybeuninit_assume_init_partial_field_not_guard",
            "maybeuninit_assume_init_partial_array_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(
                card.operation.family,
                OperationFamily::MaybeUninitAssumeInit
            );
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(!obligation_discharge_present(card, "initialized"));
            assert!(
                card.missing.iter().any(|missing| missing.kind == "guard"),
                "{fixture} must not resolve the initialized-memory guard prompt"
            );
        }
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
    fn vec_set_len_capacity_observation_is_not_capacity_guard() -> Result<(), String> {
        for fixture in [
            "vec_set_len_capacity_observed_not_guard",
            "vec_set_len_unrelated_capacity_comparison_not_guard",
            "vec_set_len_cap_argument_not_guard",
            "vec_set_len_stale_capacity_binding_not_guard",
            "vec_set_len_stale_start_bound_shrink_not_guard",
            "vec_set_len_stale_last_index_shrink_not_guard",
            "vec_set_len_reassigned_receiver_not_guard",
            "vec_set_len_reassigned_new_len_not_guard",
            "vec_set_len_compound_reassigned_new_len_not_guard",
            "vec_set_len_shadowed_new_len_not_guard",
            "vec_set_len_reserve_reassigned_additional_not_guard",
            "vec_set_len_try_reserve_reassigned_additional_not_guard",
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
    fn vec_set_len_self_new_const_cap_keeps_capacity_missing() -> Result<(), String> {
        let output = fixture_output("vec_set_len_self_new_const_cap_not_guard")?;
        let card = single_card("vec_set_len_self_new_const_cap_not_guard", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::VecSetLen);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!obligation_discharge_present(card, "capacity"));
        assert!(obligation_discharge_present(card, "initialized"));
        assert!(
            card.missing.iter().any(|missing| missing.kind == "guard"),
            "opaque Self::new capacity evidence must keep the guard prompt"
        );
        Ok(())
    }

    #[test]
    fn vec_set_len_accepts_pre_call_initialized_range_evidence() -> Result<(), String> {
        for fixture in [
            "vec_set_len_initialized_loop",
            "vec_set_len_slice_binding_initialized_loop",
            "vec_set_len_call_result_init",
            "vec_set_len_shrink",
            "vec_set_len_last_index_shrink",
            "vec_set_len_start_bound_shrink",
            "vec_set_len_zero_clear",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::VecSetLen);
            assert!(
                matches!(
                    card.class,
                    ReviewClass::GuardedUnwitnessed | ReviewClass::UnsafeUnreached
                ),
                "{fixture} should have initialized-range evidence without an open guard prompt"
            );
            assert!(obligation_discharge_present(card, "initialized"));
            assert!(
                card.missing.iter().all(|missing| missing.kind != "guard"),
                "{fixture} should resolve the local initialized-range guard prompt"
            );
        }
        Ok(())
    }

    #[test]
    fn vec_set_len_capacity_only_evidence_keeps_initialized_range_gap() -> Result<(), String> {
        for fixture in [
            "vec_set_len_capacity_binding",
            "vec_set_len_with_capacity",
            "vec_set_len_reserve_capacity",
            "vec_set_len_try_reserve_capacity",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::VecSetLen);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(obligation_discharge_present(card, "capacity"));
            assert!(!obligation_discharge_present(card, "initialized"));
            assert!(
                card.missing.iter().any(|missing| missing.kind == "guard"),
                "{fixture} should keep the initialized-range guard prompt"
            );
        }
        Ok(())
    }

    #[test]
    fn vec_set_len_post_initialization_is_not_guard_evidence() -> Result<(), String> {
        for fixture in [
            "vec_set_len_post_init_not_guard",
            "vec_set_len_unrelated_initialization_not_guard",
            "vec_set_len_other_slice_binding_not_guard",
            "vec_set_len_partial_slice_binding_not_guard",
            "vec_set_len_stale_slice_binding_not_guard",
            "vec_set_len_single_index_init_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::VecSetLen);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(obligation_discharge_present(card, "capacity"));
            assert!(!obligation_discharge_present(card, "initialized"));
            assert!(
                card.missing.iter().any(|missing| missing.kind == "guard"),
                "{fixture} must keep the initialization guard prompt"
            );
        }
        Ok(())
    }

    #[test]
    fn str_from_utf8_unchecked_uses_utf8_operation_family() -> Result<(), String> {
        for fixture in [
            "str_from_utf8_unchecked",
            "str_from_utf8_unchecked_comment_not_guard",
            "str_from_utf8_unchecked_post_validation_not_guard",
            "str_from_utf8_unchecked_other_buffer_not_guard",
            "str_from_utf8_unchecked_prefix_validation_not_guard",
            "str_from_utf8_unchecked_suffix_validation_not_guard",
            "str_from_utf8_unchecked_if_let_err_return_comment_not_guard",
            "str_from_utf8_unchecked_if_let_err_return_string_not_guard",
            "str_from_utf8_unchecked_is_err_return_comment_not_guard",
            "str_from_utf8_unchecked_is_err_return_string_not_guard",
            "str_from_utf8_unchecked_is_ok_observed_not_guard",
            "str_from_utf8_unchecked_is_ok_comment_not_guard",
            "str_from_utf8_unchecked_is_ok_string_not_guard",
            "str_from_utf8_unchecked_guard_then_reassigned_not_guard",
            "str_from_utf8_unchecked_guard_then_mutated_not_guard",
            "str_from_utf8_unchecked_guard_then_shadowed_not_guard",
            "str_from_utf8_unchecked_is_err_return_reassigned_not_guard",
            "str_from_utf8_unchecked_is_err_return_shadowed_not_guard",
            "str_from_utf8_unchecked_if_let_ok_comment_not_guard",
            "str_from_utf8_unchecked_if_let_ok_string_not_guard",
            "str_from_utf8_unchecked_question_mark_comment_not_guard",
            "str_from_utf8_unchecked_question_mark_string_not_guard",
            "str_from_utf8_unchecked_match_return_comment_not_guard",
            "str_from_utf8_unchecked_match_return_string_not_guard",
            "str_from_utf8_unchecked_if_let_ok_reassigned_not_guard",
            "str_from_utf8_unchecked_if_let_ok_shadowed_not_guard",
            "str_from_utf8_unchecked_if_let_err_reassigned_not_guard",
            "str_from_utf8_unchecked_if_let_err_shadowed_not_guard",
            "str_from_utf8_unchecked_match_err_reassigned_not_guard",
            "str_from_utf8_unchecked_match_err_shadowed_not_guard",
            "str_from_utf8_unchecked_let_else_ok_comment_not_guard",
            "str_from_utf8_unchecked_let_else_ok_string_not_guard",
            "str_from_utf8_unchecked_let_else_ok_reassigned_not_guard",
            "str_from_utf8_unchecked_let_else_ok_shadowed_not_guard",
            "str_from_utf8_unchecked_match_ok_comment_not_guard",
            "str_from_utf8_unchecked_match_ok_string_not_guard",
            "str_from_utf8_unchecked_match_ok_reassigned_not_guard",
            "str_from_utf8_unchecked_match_ok_shadowed_not_guard",
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
            "str_from_utf8_unchecked_if_let_ok_guard",
            "str_from_utf8_unchecked_if_let_err_return_guard",
            "str_from_utf8_unchecked_is_err_return_guard",
            "str_from_utf8_unchecked_question_mark_guard",
            "str_from_utf8_unchecked_match_return_guard",
            "str_from_utf8_unchecked_let_else_ok_guard",
            "str_from_utf8_unchecked_match_ok_guard",
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
            "transmute_layout_size_guard",
            "transmute_bool_comment_not_guard",
            "transmute_bool_other_value_not_guard",
            "transmute_bool_prior_guarded_call_not_guard",
            "transmute_bool_disjunct_branch_not_guard",
            "transmute_bool_conjunct_return_not_guard",
            "transmute_bool_value_observed_not_guard",
            "transmute_bool_closed_if_observed_not_guard",
            "transmute_bool_invalid_return_comment_not_guard",
            "transmute_bool_guard_then_reassigned_not_guard",
            "transmute_bool_guard_then_compound_reassigned_not_guard",
            "transmute_bool_guard_then_shadowed_not_guard",
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
    fn transmute_layout_branch_evidence_requires_top_level_conjunct() -> Result<(), String> {
        for fixture in [
            "transmute_layout_conjunct_branch_guard",
            "transmute_copy_layout_conjunct_branch_guard",
        ] {
            let guarded = fixture_output(fixture)?;
            let guarded_card = single_card(fixture, &guarded)?;

            assert_eq!(guarded_card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(guarded_card.operation.family, OperationFamily::Transmute);
            assert_eq!(guarded_card.class, ReviewClass::GuardMissing);
            assert!(obligation_discharge_present(guarded_card, "layout"));
            assert!(!obligation_discharge_present(guarded_card, "valid-value"));
        }

        for fixture in [
            "transmute_layout_disjunct_branch_not_guard",
            "transmute_layout_closed_branch_not_guard",
            "transmute_layout_observed_not_guard",
            "transmute_copy_layout_disjunct_branch_not_guard",
        ] {
            let disjunct = fixture_output(fixture)?;
            let disjunct_card = single_card(fixture, &disjunct)?;

            assert_eq!(disjunct_card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(disjunct_card.operation.family, OperationFamily::Transmute);
            assert_eq!(disjunct_card.class, ReviewClass::GuardMissing);
            assert!(!obligation_discharge_present(disjunct_card, "layout"));
            assert!(!obligation_discharge_present(disjunct_card, "valid-value"));
        }
        Ok(())
    }

    #[test]
    fn transmute_layout_mismatch_return_requires_top_level_disjunct() -> Result<(), String> {
        for fixture in [
            "transmute_layout_mismatch_return_guard",
            "transmute_copy_layout_mismatch_return_guard",
        ] {
            let guarded = fixture_output(fixture)?;
            let guarded_card = single_card(fixture, &guarded)?;

            assert_eq!(guarded_card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(guarded_card.operation.family, OperationFamily::Transmute);
            assert_eq!(guarded_card.class, ReviewClass::GuardMissing);
            assert!(obligation_discharge_present(guarded_card, "layout"));
            assert!(!obligation_discharge_present(guarded_card, "valid-value"));
        }

        for fixture in [
            "transmute_layout_conjunct_return_not_guard",
            "transmute_layout_mismatch_return_comment_not_guard",
            "transmute_copy_layout_conjunct_return_not_guard",
            "transmute_copy_layout_mismatch_return_comment_not_guard",
        ] {
            let conjunct = fixture_output(fixture)?;
            let conjunct_card = single_card(fixture, &conjunct)?;

            assert_eq!(conjunct_card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(conjunct_card.operation.family, OperationFamily::Transmute);
            assert_eq!(conjunct_card.class, ReviewClass::GuardMissing);
            assert!(!obligation_discharge_present(conjunct_card, "layout"));
            assert!(!obligation_discharge_present(conjunct_card, "valid-value"));
        }
        Ok(())
    }

    #[test]
    fn transmute_bool_value_domain_guards_are_discharged() -> Result<(), String> {
        for fixture in [
            "transmute_bool_valid_value_guard",
            "transmute_bool_conjunct_branch_guard",
            "transmute_bool_invalid_return_guard",
            "transmute_bool_disjunct_return_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::Transmute);
            assert_eq!(card.class, ReviewClass::GuardedUnwitnessed);
            assert!(obligation_discharge_present(card, "layout"));
            assert!(obligation_discharge_present(card, "valid-value"));
            assert!(
                card.missing.iter().all(|missing| missing.kind != "guard"),
                "{fixture} should resolve the local valid-value guard prompt"
            );
        }
        Ok(())
    }

    #[test]
    fn transmute_copy_bool_value_observation_is_not_guard_evidence() -> Result<(), String> {
        for fixture in [
            "transmute_copy_layout_size_guard",
            "transmute_copy_bool_comment_not_guard",
            "transmute_copy_bool_other_value_not_guard",
            "transmute_copy_bool_prior_guarded_call_not_guard",
            "transmute_copy_bool_disjunct_branch_not_guard",
            "transmute_copy_bool_conjunct_return_not_guard",
            "transmute_copy_bool_value_observed_not_guard",
            "transmute_copy_bool_closed_if_observed_not_guard",
            "transmute_copy_bool_invalid_return_comment_not_guard",
            "transmute_copy_bool_guard_then_reassigned_not_guard",
            "transmute_copy_bool_guard_then_compound_reassigned_not_guard",
            "transmute_copy_bool_guard_then_shadowed_not_guard",
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
    fn transmute_copy_bool_value_domain_guards_are_discharged() -> Result<(), String> {
        for fixture in [
            "transmute_copy_bool_valid_value_guard",
            "transmute_copy_bool_conjunct_branch_guard",
            "transmute_copy_bool_invalid_return_guard",
            "transmute_copy_bool_disjunct_return_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::Transmute);
            assert_eq!(card.class, ReviewClass::GuardedUnwitnessed);
            assert!(obligation_discharge_present(card, "layout"));
            assert!(obligation_discharge_present(card, "valid-value"));
            assert!(
                card.missing.iter().all(|missing| missing.kind != "guard"),
                "{fixture} should resolve the local valid-value guard prompt"
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

        let valid_output = fixture_output("zeroed_valid_u32")?;
        let valid_card = single_card("zeroed_valid_u32", &valid_output)?;

        assert_eq!(valid_card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(valid_card.operation.family, OperationFamily::Zeroed);
        assert_eq!(valid_card.class, ReviewClass::GuardedUnwitnessed);
        assert!(valid_card.hazards.contains(&HazardKind::InvalidValue));
        assert!(valid_card.id.0.contains("zeroed"));
        assert!(
            valid_card.obligation_evidence.iter().any(|evidence| {
                evidence.obligation.key == "valid-zero" && evidence.discharge.present
            }),
            "valid-zero obligation should be discharged for known valid-zero target types"
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
    fn pointer_arithmetic_other_offset_guard_is_not_discharged() -> Result<(), String> {
        let output = fixture_output("pointer_arithmetic_other_offset_not_guard")?;
        let card = single_card("pointer_arithmetic_other_offset_not_guard", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::PointerArithmetic);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!card.discharge.present);
        assert!(!obligation_discharge_present(card, "bounds"));
        assert!(card.id.0.contains("add"));
        Ok(())
    }

    #[test]
    fn pointer_arithmetic_reassigned_offset_guard_is_not_discharged() -> Result<(), String> {
        let output = fixture_output("pointer_arithmetic_reassigned_offset_not_guard")?;
        let card = single_card("pointer_arithmetic_reassigned_offset_not_guard", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::PointerArithmetic);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!card.discharge.present);
        assert!(!obligation_discharge_present(card, "bounds"));
        assert!(card.id.0.contains("add"));
        Ok(())
    }

    #[test]
    fn pointer_arithmetic_shadowed_offset_guard_is_not_discharged() -> Result<(), String> {
        let output = fixture_output("pointer_arithmetic_shadowed_offset_not_guard")?;
        let card = single_card("pointer_arithmetic_shadowed_offset_not_guard", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::PointerArithmetic);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!card.discharge.present);
        assert!(!obligation_discharge_present(card, "bounds"));
        assert!(card.id.0.contains("add"));
        Ok(())
    }

    #[test]
    fn pointer_arithmetic_compound_offset_guard_is_not_discharged() -> Result<(), String> {
        let output = fixture_output("pointer_arithmetic_compound_offset_not_guard")?;
        let card = single_card("pointer_arithmetic_compound_offset_not_guard", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::PointerArithmetic);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!card.discharge.present);
        assert!(!obligation_discharge_present(card, "bounds"));
        assert!(card.id.0.contains("add"));
        Ok(())
    }

    #[test]
    fn pointer_arithmetic_stale_bound_guard_is_not_discharged() -> Result<(), String> {
        let output = fixture_output("pointer_arithmetic_stale_bound_not_guard")?;
        let card = single_card("pointer_arithmetic_stale_bound_not_guard", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::PointerArithmetic);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!card.discharge.present);
        assert!(!obligation_discharge_present(card, "bounds"));
        assert!(card.id.0.contains("add"));
        Ok(())
    }

    #[test]
    fn pointer_arithmetic_disjunctive_bounds_branch_is_not_discharged() -> Result<(), String> {
        let output = fixture_output("pointer_arithmetic_disjunct_bounds_not_guard")?;
        let card = single_card("pointer_arithmetic_disjunct_bounds_not_guard", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::PointerArithmetic);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!card.discharge.present);
        assert!(!obligation_discharge_present(card, "bounds"));
        assert!(card.id.0.contains("add"));
        Ok(())
    }

    #[test]
    fn pointer_arithmetic_closed_bounds_branch_is_not_discharged() -> Result<(), String> {
        let output = fixture_output("pointer_arithmetic_closed_branch_not_guard")?;
        let card = single_card("pointer_arithmetic_closed_branch_not_guard", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::PointerArithmetic);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!card.discharge.present);
        assert!(!obligation_discharge_present(card, "bounds"));
        assert!(card.id.0.contains("add"));
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
    fn undocumented_target_feature_declaration_requires_contract() -> Result<(), String> {
        let output = fixture_output("target_feature_missing_safety_docs")?;
        let card = single_card("target_feature_missing_safety_docs", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::TargetFeature);
        assert_eq!(card.class, ReviewClass::ContractMissing);
        assert!(!card.contract.present);
        assert!(!card.discharge.present);
        assert!(!obligation_discharge_present(card, "target-feature"));
        assert!(
            card.routes
                .iter()
                .any(|route| route.kind == WitnessKind::HumanDeepReview),
            "undocumented target_feature sites should route to manual contract review"
        );
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
    fn unwrap_unchecked_direct_state_evidence_is_discharged() -> Result<(), String> {
        for fixture in [
            "unwrap_unchecked_is_some_guard",
            "unwrap_unchecked_is_ok_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::UnwrapUnchecked);
            assert_eq!(card.class, ReviewClass::GuardedUnwitnessed);
            assert!(card.discharge.present);
            assert!(obligation_discharge_present(card, "valid-value"));
        }
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
    fn unwrap_unchecked_let_else_state_evidence_is_discharged() -> Result<(), String> {
        for fixture in [
            "unwrap_unchecked_let_else_some_guard",
            "unwrap_unchecked_let_else_ok_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::UnwrapUnchecked);
            assert_eq!(card.class, ReviewClass::GuardedUnwitnessed);
            assert!(card.discharge.present);
            assert!(obligation_discharge_present(card, "valid-value"));
        }
        Ok(())
    }

    #[test]
    fn unwrap_unchecked_match_state_evidence_is_discharged() -> Result<(), String> {
        for fixture in [
            "unwrap_unchecked_match_some_guard",
            "unwrap_unchecked_match_ok_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::UnwrapUnchecked);
            assert_eq!(card.class, ReviewClass::GuardedUnwitnessed);
            assert!(card.discharge.present);
            assert!(obligation_discharge_present(card, "valid-value"));
        }
        Ok(())
    }

    #[test]
    fn unwrap_unchecked_early_return_state_evidence_is_discharged() -> Result<(), String> {
        for fixture in [
            "unwrap_unchecked_is_none_return_guard",
            "unwrap_unchecked_is_err_return_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::UnwrapUnchecked);
            assert_eq!(card.class, ReviewClass::GuardedUnwitnessed);
            assert!(card.discharge.present);
            assert!(obligation_discharge_present(card, "valid-value"));
        }
        Ok(())
    }

    #[test]
    fn unwrap_unchecked_early_return_state_evidence_rejects_comment_only_return()
    -> Result<(), String> {
        let output = fixture_output("unwrap_unchecked_is_none_return_comment_not_guard")?;
        let card = single_card("unwrap_unchecked_is_none_return_comment_not_guard", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::UnwrapUnchecked);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!card.discharge.present);
        assert!(!obligation_discharge_present(card, "valid-value"));
        Ok(())
    }

    #[test]
    fn unwrap_unchecked_direct_state_evidence_rejects_reassignment() -> Result<(), String> {
        for fixture in [
            "unwrap_unchecked_is_some_reassigned_not_guard",
            "unwrap_unchecked_is_ok_reassigned_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::UnwrapUnchecked);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(!card.discharge.present);
            assert!(!obligation_discharge_present(card, "valid-value"));
        }
        Ok(())
    }

    #[test]
    fn unwrap_unchecked_let_else_state_evidence_rejects_reassignment() -> Result<(), String> {
        for fixture in [
            "unwrap_unchecked_let_else_some_reassigned_not_guard",
            "unwrap_unchecked_let_else_ok_reassigned_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::UnwrapUnchecked);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(!card.discharge.present);
            assert!(!obligation_discharge_present(card, "valid-value"));
        }
        Ok(())
    }

    #[test]
    fn unwrap_unchecked_match_state_evidence_rejects_reassignment() -> Result<(), String> {
        for fixture in [
            "unwrap_unchecked_match_some_reassigned_not_guard",
            "unwrap_unchecked_match_ok_reassigned_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::UnwrapUnchecked);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(!card.discharge.present);
            assert!(!obligation_discharge_present(card, "valid-value"));
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
    fn long_unsafe_fn_owner_inference_uses_enclosing_function_owner() -> Result<(), String> {
        let output = fixture_output("long_unsafe_fn_owner_inference")?;
        let card = output
            .cards
            .iter()
            .find(|card| card.operation.family == OperationFamily::DropInPlace)
            .ok_or_else(|| {
                format!(
                    "long_unsafe_fn_owner_inference should emit a drop_in_place card: {:#?}",
                    output.cards
                )
            })?;

        assert_eq!(card.operation.family, OperationFamily::DropInPlace);
        assert_eq!(card.site.owner, Some("run".to_string()));
        assert!(
            card.id.0.contains("-run-operation-drop_in_place-"),
            "card identity should include the enclosing owner: {}",
            card.id.0
        );
        Ok(())
    }

    #[test]
    fn macro_rules_owner_inference_uses_macro_name() -> Result<(), String> {
        let output = fixture_output("macro_rules_owner_inference")?;
        let card = single_card("macro_rules_owner_inference", &output)?;

        assert_eq!(card.operation.family, OperationFamily::BoxFromRaw);
        assert_eq!(card.site.owner, Some("spawn_unchecked".to_string()));
        assert!(
            card.id
                .0
                .contains("-spawn-unchecked-operation-box_from_raw-"),
            "card identity should include the macro owner: {}",
            card.id.0
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
        assert!(
            !card.next_action.summary.contains("`unknown`"),
            "unknown operation next action should not ask reviewers to discharge an unknown obligation"
        );
        assert!(card.next_action.summary.contains("unsafe site"));
        assert!(
            card.next_action
                .summary
                .contains("obligation-specific guard")
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
    fn card_identity_includes_exact_policy_matching_components() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let card = single_card("raw_pointer_alignment", &output)?;

        assert_eq!(
            card.id.0,
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1"
        );
        Ok(())
    }

    #[test]
    fn ffi_extern_block_identity_changes_when_signature_changes() -> Result<(), String> {
        let original = temp_source_output(
            "unsafe-review-ffi-identity-original",
            r#"
unsafe extern "C" {
    fn strlen(ptr: *const u8) -> usize;
}
"#,
        )?;
        let changed = temp_source_output(
            "unsafe-review-ffi-identity-changed",
            r#"
unsafe extern "C" {
    fn strlen(ptr: *const core::ffi::c_char) -> usize;
}
"#,
        )?;
        let original_card = single_card("ffi original", &original)?;
        let changed_card = single_card("ffi changed", &changed)?;

        assert_eq!(original_card.operation.family, OperationFamily::Ffi);
        assert_eq!(changed_card.operation.family, OperationFamily::Ffi);
        assert_ne!(
            identity_without_count(&original_card.id),
            identity_without_count(&changed_card.id),
            "FFI extern-block identity must include the declaration lines, not only the block opener"
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
    fn configured_receipt_keeps_witness_gap_visible() -> Result<(), String> {
        let root =
            copy_fixture_to_temp("raw_pointer_alignment", "unsafe-review-configured-receipt")?;
        let card_id = single_card("raw_pointer_alignment", &fixture_output_at(&root)?)?
            .id
            .0
            .clone();
        write_receipt_with_strength(&root, &card_id, "configured")?;

        let output = fixture_output_at(&root)?;
        let card = single_card("raw_pointer_alignment configured receipt", &output)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp fixture failed: {err}"))?;
        assert!(!card.witness.present);
        assert!(card.witness.summary.contains("configured"));
        assert!(card.missing.iter().any(|missing| missing.kind == "witness"));
        assert!(
            card.obligation_evidence
                .iter()
                .all(|evidence| !evidence.witness.present)
        );
        Ok(())
    }

    #[test]
    fn wrong_tool_receipt_keeps_witness_gap_visible() -> Result<(), String> {
        let root =
            copy_fixture_to_temp("raw_pointer_alignment", "unsafe-review-wrong-tool-receipt")?;
        let card_id = single_card("raw_pointer_alignment", &fixture_output_at(&root)?)?
            .id
            .0
            .clone();
        write_receipt_with_tool_and_strength(&root, &card_id, "loom", "ran")?;

        let output = fixture_output_at(&root)?;
        let card = single_card("raw_pointer_alignment wrong-tool receipt", &output)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp fixture failed: {err}"))?;
        assert!(!card.witness.present);
        assert!(
            card.witness
                .summary
                .contains("does not match routed witness tools")
        );
        assert!(card.witness.summary.contains("loom"));
        assert!(card.missing.iter().any(|missing| missing.kind == "witness"));
        assert!(
            card.obligation_evidence
                .iter()
                .all(|evidence| !evidence.witness.present)
        );
        Ok(())
    }

    #[test]
    fn ffi_human_review_receipt_marks_witness_evidence_present_only() -> Result<(), String> {
        let root =
            copy_fixture_to_temp("ffi_missing_boundary_contract", "unsafe-review-ffi-receipt")?;
        let card_id = single_card("ffi_missing_boundary_contract", &fixture_output_at(&root)?)?
            .id
            .0
            .clone();
        write_receipt_with_tool_and_strength(&root, &card_id, "human-deep-review", "reviewed")?;

        let output = fixture_output_at(&root)?;
        let card = single_card("ffi_missing_boundary_contract receipt", &output)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp fixture failed: {err}"))?;
        assert_eq!(card.operation.family, OperationFamily::Ffi);
        assert!(
            card.routes
                .iter()
                .any(|route| route.kind == crate::domain::WitnessKind::HumanDeepReview)
        );
        assert!(card.witness.present);
        assert!(card.witness.summary.contains("human-deep-review"));
        assert!(card.witness.summary.contains("reviewed"));
        assert!(!card.missing.iter().any(|missing| missing.kind == "witness"));
        assert!(
            card.missing
                .iter()
                .any(|missing| missing.kind == "contract")
        );
        assert!(card.missing.iter().any(|missing| missing.kind == "guard"));
        assert!(card.missing.iter().any(|missing| missing.kind == "reach"));
        assert!(
            card.obligation_evidence
                .iter()
                .all(|evidence| evidence.witness.present)
        );
        assert!(
            card.obligation_evidence
                .iter()
                .any(|evidence| !evidence.discharge.present),
            "human review receipts must not become static guard/discharge evidence"
        );
        Ok(())
    }

    #[test]
    fn external_integration_receipt_marks_reach_evidence_present_only() -> Result<(), String> {
        let root = copy_fixture_to_temp(
            "ffi_missing_boundary_contract",
            "unsafe-review-external-reach-receipt",
        )?;
        let card_id = single_card("ffi_missing_boundary_contract", &fixture_output_at(&root)?)?
            .id
            .0
            .clone();
        write_receipt_with_tool_and_strength(
            &root,
            &card_id,
            "external-integration-test",
            "site_reached",
        )?;

        let output = fixture_output_at(&root)?;
        let card = single_card("ffi_missing_boundary_contract reach receipt", &output)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp fixture failed: {err}"))?;
        assert_eq!(card.reach.state, "external_reached");
        assert!(card.reach.summary.contains("external-integration-test"));
        assert!(!card.missing.iter().any(|missing| missing.kind == "reach"));
        assert!(card.missing.iter().any(|missing| missing.kind == "witness"));
        assert!(!card.witness.present);
        assert!(
            card.missing
                .iter()
                .any(|missing| missing.kind == "contract")
        );
        assert!(card.missing.iter().any(|missing| missing.kind == "guard"));
        assert!(
            card.obligation_evidence
                .iter()
                .all(|evidence| evidence.reach.present)
        );
        assert!(
            card.obligation_evidence
                .iter()
                .all(|evidence| !evidence.witness.present)
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

    fn temp_source_output(prefix: &str, source: &str) -> Result<AnalyzeOutput, String> {
        let root = unique_temp_dir(prefix)?;
        fs::create_dir_all(root.join("src"))
            .map_err(|err| format!("create temp fixture failed: {err}"))?;
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"js-buffer-reentry-fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
        )
        .map_err(|err| format!("write temp Cargo.toml failed: {err}"))?;
        fs::write(root.join("src/lib.rs"), source)
            .map_err(|err| format!("write temp source failed: {err}"))?;

        let output = analyze(AnalyzeInput {
            root: root.clone(),
            scope: Scope::Repo,
            diff: DiffSource::NoneRepoScan,
            mode: AnalysisMode::Repo,
            policy: PolicyMode::Advisory,
            include_unchanged_tests: true,
            max_cards: None,
        });
        let cleanup =
            fs::remove_dir_all(&root).map_err(|err| format!("remove temp fixture failed: {err}"));
        match (output, cleanup) {
            (Ok(output), Ok(())) => Ok(output),
            (Err(err), _) => Err(err),
            (Ok(_), Err(err)) => Err(err),
        }
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
        write_receipt_with_strength(root, card_id, "ran")
    }

    fn write_receipt_with_strength(
        root: &Path,
        card_id: &str,
        strength: &str,
    ) -> Result<(), String> {
        write_receipt_with_tool_and_strength(root, card_id, "miri", strength)
    }

    fn write_receipt_with_tool_and_strength(
        root: &Path,
        card_id: &str,
        tool: &str,
        strength: &str,
    ) -> Result<(), String> {
        let command = if tool == "human-deep-review" {
            "manual review of cited foreign declaration and Rust extern signature"
        } else if tool == "external-integration-test" {
            "bun test test/js/sab-copy-to-unshared.test.ts"
        } else {
            "cargo +nightly miri test read_header"
        };
        let receipt_dir = root.join(".unsafe-review").join("receipts");
        fs::create_dir_all(&receipt_dir)
            .map_err(|err| format!("create {} failed: {err}", receipt_dir.display()))?;
        fs::write(
            receipt_dir.join(format!("{}.json", tool.replace('-', "_"))),
            format!(
                r#"{{
  "schema_version": "0.1",
  "card_id": "{card_id}",
  "tool": "{tool}",
  "strength": "{strength}",
  "author": "core/fixtures",
  "recorded_at": "2026-05-18T00:00:00Z",
  "expires_at": "2026-08-18",
  "summary": "focused fixture witness passed",
  "command": "{command}",
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

    /// Build a minimal temporary crate with one `unsafe fn` so the repo-scan
    /// path and diff-scoped paths produce contrasting results.
    fn temp_unsafe_crate(prefix: &str) -> Result<PathBuf, String> {
        let root = unique_temp_dir(prefix)?;
        fs::create_dir_all(root.join("src")).map_err(|err| format!("create dirs failed: {err}"))?;
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"empty-diff-fixture\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
        )
        .map_err(|err| format!("write Cargo.toml failed: {err}"))?;
        fs::write(
            root.join("src/lib.rs"),
            "pub unsafe fn danger(ptr: *const u8) -> u8 { unsafe { *ptr } }\n",
        )
        .map_err(|err| format!("write src/lib.rs failed: {err}"))?;
        Ok(root)
    }

    #[test]
    fn empty_diff_text_yields_zero_candidates_zero_cards() -> Result<(), String> {
        // Scope::Diff + DiffSource::Text("") → zero candidate files, zero cards.
        // This is the core fix for issue #1558: a valid empty diff must NOT
        // fall back to scanning all files.
        let root = temp_unsafe_crate("unsafe-review-empty-diff-text")?;
        let output = analyze(AnalyzeInput {
            root: root.clone(),
            scope: Scope::Diff,
            diff: DiffSource::Text(String::new()),
            mode: AnalysisMode::Draft,
            policy: PolicyMode::Advisory,
            include_unchanged_tests: true,
            max_cards: None,
        });
        let _ = fs::remove_dir_all(&root);
        let output = output?;

        assert_eq!(output.scope, Scope::Diff);
        assert_eq!(
            output.summary.cards, 0,
            "empty diff should produce zero cards, not whole-repo cards"
        );
        assert_eq!(
            output.summary.changed_rust_files, 0,
            "empty diff should report zero changed rust files"
        );
        assert_eq!(
            output.summary.changed_files, 0,
            "empty diff should report zero changed files"
        );
        assert!(output.cards.is_empty(), "card list must be empty");
        Ok(())
    }

    #[test]
    fn none_repo_scan_repo_scope_still_produces_cards() -> Result<(), String> {
        // Control: Scope::Repo + NoneRepoScan → all files scanned, cards produced.
        // This is the existing repo-scan default; the fix must not change it.
        let root = temp_unsafe_crate("unsafe-review-none-repo-scan")?;
        let output = analyze(AnalyzeInput {
            root: root.clone(),
            scope: Scope::Repo,
            diff: DiffSource::NoneRepoScan,
            mode: AnalysisMode::Repo,
            policy: PolicyMode::Advisory,
            include_unchanged_tests: true,
            max_cards: None,
        });
        let _ = fs::remove_dir_all(&root);
        let output = output?;

        assert_eq!(output.scope, Scope::Repo);
        assert!(
            output.summary.changed_rust_files > 0,
            "NoneRepoScan repo scan with an unsafe file should report changed_rust_files > 0"
        );
        assert!(
            !output.cards.is_empty(),
            "NoneRepoScan repo scan should still produce cards from the unsafe file"
        );
        Ok(())
    }
}
