//! Declaration-summary projection (issue #1895).
//!
//! Unsafe function and unsafe trait declarations intentionally produce owner/
//! contract `ReviewCard`s (`operation_family = unsafe_declaration`). On a
//! declaration-heavy crate those cards can dominate a raw card list even
//! though each one is correct and generally unsuitable for a flood of inline
//! comments.
//!
//! This module groups the *existing* `unsafe_declaration` cards by source
//! file into a bounded, deterministic summary so their volume is
//! understandable without deleting or reclassifying any of them. It is a
//! presentation-only projection:
//!
//! - It derives every field from `ReviewCard`/`CoverageBlock` data already
//!   computed by the pipeline; it never mutates a card, drops a card, or
//!   invents a second classifier.
//! - `cards.json` and every per-card surface (raw card table, SARIF, LSP,
//!   comment-plan, badges) are untouched by this module and remain the
//!   complete evidence inventory.
//! - New-or-worsened declarations are counted and sorted ahead of
//!   inherited-only groups so they cannot be hidden behind unchanged volume.
//!
//! See `docs/specs/UNSAFE-REVIEW-SPEC-0011-pr-ci-output.md` §3.2 for the
//! rendered contract.

use crate::api::AnalyzeOutput;
use crate::domain::coverage::BaselineState;
use crate::domain::{Coverage, CoverageBlock, OperationFamily, ReviewCard};
use crate::util::path_display;
use std::collections::BTreeMap;

/// `group_kind` tag stamped on every declaration group (candidate schema,
/// issue #1895).
pub(crate) const DECLARATION_GROUP_KIND: &str = "unsafe_declarations";

/// Bound on the number of representative card IDs a renderer prints inline
/// per group. The full membership is always available via
/// `underlying_card_ids` and in `cards.json`; this cap only bounds what a
/// human-facing surface prints so a declaration-heavy file cannot flood the
/// summary. Kept small and odd-shaped-safe: any group total above this cap
/// is denoted with a "+N more" style pointer at the render layer.
pub(crate) const DECLARATION_SUMMARY_REPRESENTATIVE_LIMIT: usize = 3;

/// One file-scoped group of `unsafe_declaration`-family `ReviewCard`s.
///
/// Field names follow the candidate projection in issue #1895 and avoid
/// duplicating fields already owned by `ReviewCard`/`CoverageBlock` -- this
/// struct only adds grouping/counting, not new evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DeclarationGroup {
    /// Always `"unsafe_declarations"` -- reserved for future group kinds.
    pub(crate) group_kind: &'static str,
    /// Stable, deterministic identifier for this group (derived from
    /// `module_or_file`; does not encode a card count or ordering so it does
    /// not change if unrelated groups shift rank).
    pub(crate) group_id: String,
    /// Source file this group is scoped to (display-normalized, forward
    /// slashes).
    pub(crate) module_or_file: String,
    /// Every underlying card ID in this group, in the group's deterministic
    /// order. This is the complete membership list -- not bounded -- so a
    /// consumer can always resolve the full set without guessing.
    pub(crate) underlying_card_ids: Vec<String>,
    /// Total `unsafe_declaration` cards in this group (all baseline states).
    pub(crate) total: usize,
    /// Cards whose `CoverageBlock::baseline_state` is `New` or `Worsened`
    /// after snapshot-slot movement is applied -- i.e. this PR introduced or
    /// worsened the obligation.
    pub(crate) new_or_worsened: usize,
    /// `total - new_or_worsened`: everything not newly introduced or
    /// worsened by this PR, including baseline-known debt and cards whose
    /// obligation is already fully discharged (non-actionable, unmoved).
    pub(crate) inherited: usize,
    /// Cards whose contract-evidence slot (`CoverageBlock::contract_coverage`)
    /// is `Missing` or `Weak`.
    pub(crate) contract_missing: usize,
    /// Cards whose contract-evidence slot is `Present`.
    pub(crate) contract_present: usize,
    /// Bounded, deterministic sample of `underlying_card_ids` for inline
    /// display -- new/worsened cards first, capped at
    /// `DECLARATION_SUMMARY_REPRESENTATIVE_LIMIT`.
    pub(crate) representatives: Vec<String>,
}

/// Derive deterministic declaration-summary groups from `output.cards`.
///
/// Filters to `operation_family == unsafe_declaration`, groups by source
/// file, and ranks groups that contain at least one new-or-worsened card
/// ahead of inherited-only groups (ties broken by file path, then group id)
/// so changed obligations cannot be hidden behind unchanged volume.
///
/// Returns an empty `Vec` when there are no `unsafe_declaration` cards --
/// callers must render nothing new in that case so quiet PRs stay quiet.
pub(crate) fn declaration_groups(output: &AnalyzeOutput) -> Vec<DeclarationGroup> {
    let mut by_file: BTreeMap<String, Vec<&ReviewCard>> = BTreeMap::new();
    for card in &output.cards {
        if card.operation.family != OperationFamily::UnsafeDeclaration {
            continue;
        }
        let file = path_display(&card.site.location.file);
        by_file.entry(file).or_default().push(card);
    }

    let mut groups: Vec<DeclarationGroup> = by_file
        .into_iter()
        .map(|(file, cards)| build_group(file, cards, output))
        .collect();

    groups.sort_by(|left, right| {
        let left_has_new = left.new_or_worsened > 0;
        let right_has_new = right.new_or_worsened > 0;
        // Groups WITH a new/worsened card sort first (`Ordering::Less`).
        right_has_new
            .cmp(&left_has_new)
            .then_with(|| left.module_or_file.cmp(&right.module_or_file))
            .then_with(|| left.group_id.cmp(&right.group_id))
    });
    groups
}

fn build_group(file: String, cards: Vec<&ReviewCard>, output: &AnalyzeOutput) -> DeclarationGroup {
    // Derive each card's movement flag and contract-coverage slot exactly once
    // up front, so the sort comparator and the counting loop below reuse the
    // results instead of re-running `is_new_or_worsened` (O(n log n) times) and
    // `CoverageBlock::derive` (again per card). Same values, fewer derivations.
    let mut entries: Vec<(&ReviewCard, bool, Coverage)> = cards
        .into_iter()
        .map(|card| {
            let is_new = is_new_or_worsened(card, output);
            let contract = CoverageBlock::derive(card).contract_coverage;
            (card, is_new, contract)
        })
        .collect();

    // Deterministic in-group order: new/worsened cards first, then by
    // ascending line, then by card id -- this also fixes the deterministic
    // representative sample selected below.
    entries.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| left.0.site.location.line.cmp(&right.0.site.location.line))
            .then_with(|| left.0.id.0.cmp(&right.0.id.0))
    });

    let mut new_or_worsened = 0usize;
    let mut contract_missing = 0usize;
    let mut contract_present = 0usize;
    for (_, is_new, contract) in &entries {
        if *is_new {
            new_or_worsened += 1;
        }
        match contract {
            Coverage::Present => contract_present += 1,
            Coverage::Missing | Coverage::Weak => contract_missing += 1,
        }
    }

    let total = entries.len();
    let inherited = total.saturating_sub(new_or_worsened);
    let underlying_card_ids: Vec<String> = entries
        .iter()
        .map(|(card, _, _)| card.id.to_string())
        .collect();
    let representatives = underlying_card_ids
        .iter()
        .take(DECLARATION_SUMMARY_REPRESENTATIVE_LIMIT)
        .cloned()
        .collect();

    DeclarationGroup {
        group_kind: DECLARATION_GROUP_KIND,
        group_id: format!("unsafe-declarations::{file}"),
        module_or_file: file,
        underlying_card_ids,
        total,
        new_or_worsened,
        inherited,
        contract_missing,
        contract_present,
        representatives,
    }
}

/// Whether `card`'s baseline posture is `New` or `Worsened` after applying
/// the saved coverage snapshot, if any. Reuses the exact
/// `CoverageBlock::derive` + `apply_snapshot_slots` derivation every other
/// baseline-movement surface uses (SPEC-0030 §single-truth) -- this is not a
/// second classifier, just the shared coverage block read from a different
/// call site.
fn is_new_or_worsened(card: &ReviewCard, output: &AnalyzeOutput) -> bool {
    let mut block = card.coverage_block();
    if let Some(snapshot) = output.coverage_snapshot.get(&card.id.0) {
        block.apply_snapshot_slots(
            &snapshot.contract_coverage,
            &snapshot.guard_coverage,
            &snapshot.test_reach_coverage,
            &snapshot.witness_receipt_coverage,
        );
    }
    matches!(
        block.baseline_state,
        BaselineState::New | BaselineState::Worsened
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{PolicyMode, Scope, Summary};
    use crate::domain::{
        CardId, Confidence, ContractEvidence, DischargeEvidence, HazardKind, MissingEvidence,
        NextAction, Priority, ProofPath, ReachEvidence, ReviewClass, SourceLocation,
        UnsafeOperation, UnsafeSite, UnsafeSiteKind, WitnessEvidence,
    };
    use crate::freshness::AnalysisIdentity;
    use std::collections::BTreeSet;

    fn declaration_card(id: &str, file: &str, line: usize, class: ReviewClass) -> ReviewCard {
        ReviewCard {
            id: CardId(id.to_string()),
            class,
            priority: Priority::Medium,
            confidence: Confidence::Medium,
            proof_path: ProofPath::HumanReviewOnly,
            site: UnsafeSite {
                location: SourceLocation {
                    file: file.into(),
                    line,
                    column: 1,
                },
                kind: UnsafeSiteKind::UnsafeFn,
                owner: Some(format!("owner_{line}")),
                visibility: "public".to_string(),
                public_api_surface: true,
                changed: true,
                snippet: "pub unsafe fn f() {}".to_string(),
            },
            operation: UnsafeOperation {
                expression: "pub unsafe fn f() {}".to_string(),
                family: OperationFamily::UnsafeDeclaration,
            },
            hazards: vec![HazardKind::Unknown],
            obligations: vec![],
            obligation_evidence: vec![],
            contract: ContractEvidence::missing(),
            discharge: DischargeEvidence::present("declaration site; no local guard expected"),
            reach: ReachEvidence {
                state: "missing".to_string(),
                summary: "no tests".to_string(),
            },
            witness: WitnessEvidence::missing(),
            missing: vec![MissingEvidence {
                kind: "contract".to_string(),
                message: "no safety contract was found".to_string(),
            }],
            routes: vec![],
            next_action: NextAction {
                summary: "add a # Safety section".to_string(),
                verify_commands: vec![],
            },
            related_tests: vec![],
        }
    }

    fn output_with_cards(cards: Vec<ReviewCard>) -> AnalyzeOutput {
        AnalyzeOutput {
            analysis_identity: AnalysisIdentity::new("diff"),
            schema_version: "0.2".to_string(),
            tool: "unsafe-review".to_string(),
            root: "/tmp".into(),
            scope: Scope::Diff,
            mode: crate::api::AnalysisMode::Draft,
            policy: PolicyMode::Advisory,
            summary: Summary::default(),
            cards,
            diff_scoped_files: BTreeSet::new(),
            coverage_snapshot: BTreeMap::new(),
        }
    }

    #[test]
    fn quiet_output_has_no_declaration_groups() {
        let output = output_with_cards(vec![]);
        assert!(declaration_groups(&output).is_empty());
    }

    #[test]
    fn non_declaration_cards_are_excluded() {
        let mut card = declaration_card("UR-c1", "src/lib.rs", 1, ReviewClass::ContractMissing);
        card.operation.family = OperationFamily::RawPointerDeref;
        let output = output_with_cards(vec![card]);
        assert!(declaration_groups(&output).is_empty());
    }

    #[test]
    fn groups_by_file_and_counts_totals() {
        let cards = vec![
            declaration_card("UR-c1", "src/lib.rs", 1, ReviewClass::ContractMissing),
            declaration_card("UR-c2", "src/lib.rs", 5, ReviewClass::ContractMissing),
            declaration_card("UR-c3", "src/ffi.rs", 1, ReviewClass::ContractMissing),
        ];
        let output = output_with_cards(cards);
        let groups = declaration_groups(&output);
        assert_eq!(groups.len(), 2);
        // Both groups have a new/worsened card, so ties break on
        // `module_or_file` ascending: "src/ffi.rs" sorts before "src/lib.rs".
        assert_eq!(groups[0].module_or_file, "src/ffi.rs");
        assert_eq!(groups[1].module_or_file, "src/lib.rs");
        assert_eq!(groups[1].total, 2);
        assert_eq!(groups[1].underlying_card_ids, vec!["UR-c1", "UR-c2"]);
        assert_eq!(groups[1].group_kind, DECLARATION_GROUP_KIND);
    }

    #[test]
    fn new_or_worsened_groups_sort_ahead_of_inherited_only_groups() {
        // `a.rs` has only baseline-known (inherited) cards; `z.rs` has one
        // new card. Without the new/worsened-first rule, alphabetical file
        // order would put `a.rs` first -- the rule must override that.
        let cards = vec![
            declaration_card("UR-a1", "src/a.rs", 1, ReviewClass::BaselineKnown),
            declaration_card("UR-z1", "src/z.rs", 1, ReviewClass::ContractMissing),
        ];
        let output = output_with_cards(cards);
        let groups = declaration_groups(&output);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].module_or_file, "src/z.rs");
        assert_eq!(groups[0].new_or_worsened, 1);
        assert_eq!(groups[1].module_or_file, "src/a.rs");
        assert_eq!(groups[1].new_or_worsened, 0);
        assert_eq!(groups[1].inherited, 1);
    }

    #[test]
    fn representatives_are_bounded_and_full_membership_is_preserved() {
        let cards = (0..6)
            .map(|i| {
                declaration_card(
                    &format!("UR-c{i}"),
                    "src/lib.rs",
                    i + 1,
                    ReviewClass::ContractMissing,
                )
            })
            .collect();
        let output = output_with_cards(cards);
        let groups = declaration_groups(&output);
        assert_eq!(groups.len(), 1);
        let group = &groups[0];
        assert_eq!(group.total, 6);
        assert_eq!(group.underlying_card_ids.len(), 6);
        assert_eq!(
            group.representatives.len(),
            DECLARATION_SUMMARY_REPRESENTATIVE_LIMIT
        );
        // Representatives are a prefix of the deterministic membership order.
        assert_eq!(
            group.representatives,
            group.underlying_card_ids[..DECLARATION_SUMMARY_REPRESENTATIVE_LIMIT]
        );
    }

    #[test]
    fn contract_coverage_counts_split_present_and_missing() {
        let mut present_card =
            declaration_card("UR-c1", "src/lib.rs", 1, ReviewClass::GuardedAndWitnessed);
        present_card.contract = ContractEvidence::present("# Safety section documented");
        let missing_card = declaration_card("UR-c2", "src/lib.rs", 2, ReviewClass::ContractMissing);
        let output = output_with_cards(vec![present_card, missing_card]);
        let groups = declaration_groups(&output);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].contract_present, 1);
        assert_eq!(groups[0].contract_missing, 1);
    }

    #[test]
    fn is_new_or_worsened_detects_worsened_against_a_populated_snapshot() {
        // The other tests all use an empty `coverage_snapshot`; this one exercises
        // the `apply_snapshot_slots` branch. A baseline-known declaration whose
        // recorded snapshot floor had contract evidence present, but whose current
        // card has none, must read as worsened once the snapshot slots are applied.
        let card = declaration_card("UR-snap", "src/lib.rs", 1, ReviewClass::BaselineKnown);
        let mut output = output_with_cards(vec![card.clone()]);

        // No snapshot: a baseline-known card is inherited, not new/worsened.
        assert!(!is_new_or_worsened(&card, &output));

        output.coverage_snapshot.insert(
            card.id.0.clone(),
            crate::policy::SnapshotCoverage {
                contract_coverage: "present".to_string(),
                guard_coverage: "missing".to_string(),
                test_reach_coverage: "missing".to_string(),
                witness_receipt_coverage: "missing".to_string(),
            },
        );
        assert!(
            is_new_or_worsened(&card, &output),
            "contract dropped present -> missing since the snapshot floor: must be worsened"
        );

        // The group reflects the worsened movement counted from the same path.
        let groups = declaration_groups(&output);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].new_or_worsened, 1);
        assert_eq!(groups[0].inherited, 0);
    }

    #[test]
    fn grouping_is_deterministic_across_repeated_derivation() {
        let cards = vec![
            declaration_card("UR-c1", "src/b.rs", 1, ReviewClass::ContractMissing),
            declaration_card("UR-c2", "src/a.rs", 3, ReviewClass::BaselineKnown),
            declaration_card("UR-c3", "src/a.rs", 1, ReviewClass::ContractMissing),
        ];
        let output = output_with_cards(cards);
        let first = declaration_groups(&output);
        let second = declaration_groups(&output);
        assert_eq!(first, second);
    }
}
