//! Target-feature-summary projection (issue #1894).
//!
//! Real-crate dogfood on SIMD-heavy code (memchr-class multi-arch dispatch)
//! found large sets of structurally similar `#[target_feature]` `ReviewCard`s
//! across architecture and feature variants. Each card can be individually
//! correct while the set overwhelms human-facing summaries and the
//! comment-plan budget.
//!
//! This module defines the *full-inventory* equivalence grouping of
//! `target_feature`-family `ReviewCard`s by a canonical, obligation-preserving
//! identity ([`GroupKey`]) so their volume is understandable without deleting,
//! suppressing, or reclassifying any of them. `target_feature_groups` groups
//! every matching card regardless of comment-plan eligibility -- it is the
//! full-inventory view the markdown surfaces (`output/markdown.rs`) render.
//! It is a presentation-only projection:
//!
//! - It derives every field from `ReviewCard`/`CoverageBlock` data already
//!   computed by the pipeline; it never mutates a card, drops a card, or
//!   invents a second classifier.
//! - `cards.json` and every per-card surface (raw card table, SARIF, LSP,
//!   comment-plan candidate list, badges) remain the complete evidence
//!   inventory; this module only adds a grouping/counting view.
//! - Architecture/feature literals (e.g. `enable = "avx2"` vs
//!   `enable = "neon"`) are treated as group *metadata*, never group
//!   *identity* -- see [`normalized_shape`].
//! - Cards whose obligation (including the structured set of *which*
//!   obligations are unsatisfied), review class, movement (baseline state),
//!   receipt state, or next action differ NEVER collapse into the same
//!   group -- see [`GroupKey`].
//!
//! A second, narrower concern -- which member of a group is eligible to
//! occupy a comment-plan slot -- is deliberately NOT decided here. Comment-
//! plan eligibility (`should_plan_comment`) and importance ranking
//! (`importance_rank`) are `comment_plan`-internal concerns, so the
//! eligibility-aware "at most one representative per group" selection used
//! by the comment-plan lives in
//! `output::comment_plan::selection::target_feature_grouped_repetition_ids`,
//! which is built on top of the groups this module returns. Grouping members
//! by equivalence and choosing a comment-plan representative among them are
//! two different questions with two different answers: an equivalence group
//! can (and often does) contain ineligible members that have their own
//! canonical non-selection reason and must never be mislabeled
//! `grouped_repetition`.
//!
//! See `docs/specs/UNSAFE-REVIEW-SPEC-0011-pr-ci-output.md` §3.2 for the
//! rendered contract.

use crate::api::AnalyzeOutput;
use crate::domain::{CoverageBlock, OperationFamily, ReviewCard};
use crate::util::{path_display, slug, stable_hash_hex};
use std::collections::{BTreeMap, BTreeSet};

/// `group_kind` tag stamped on every target-feature-repetition group
/// (candidate schema, issue #1894).
pub(crate) const TARGET_FEATURE_GROUP_KIND: &str = "target_feature_repetition";

/// Bound on the number of representative card IDs a renderer prints inline
/// per group. The full membership is always available via
/// `underlying_card_ids` and in `cards.json`; this cap only bounds what a
/// human-facing surface prints so a repetition-heavy file cannot flood the
/// summary. Mirrors `declaration_summary::DECLARATION_SUMMARY_REPRESENTATIVE_LIMIT`.
pub(crate) const TARGET_FEATURE_SUMMARY_REPRESENTATIVE_LIMIT: usize = 3;

/// Field separator used only to build the canonical string hashed into
/// `group_id` (see [`group_id`]). A control character so it cannot collide
/// with file paths, next-action prose, or obligation keys.
const GROUP_ID_FIELD_SEP: char = '\u{1}';
/// List-item separator for the `missing_obligations` field within the
/// canonical `group_id` string.
const GROUP_ID_LIST_SEP: char = '\u{2}';

/// Canonical, obligation-preserving group identity for a `target_feature`
/// `ReviewCard`.
///
/// Architecture/feature literals are deliberately excluded: `shape` is the
/// operation expression with every quoted literal normalized away (see
/// [`normalized_shape`]), so `enable = "avx2"` and `enable = "neon"` share a
/// shape. Every other field is an equivalence requirement from issue #1894:
/// two cards group together only when their file, class, baseline movement,
/// full coverage-slot state, structured set of unsatisfied obligations, and
/// next action are all identical.
///
/// `missing_obligations` mirrors `comment_plan::model::comment_budget_key`'s
/// obligation derivation exactly (sorted, deduplicated unsatisfied
/// `obligation_evidence[].obligation.key` values, falling back to
/// `["review"]` when every obligation is fully discharged) -- see
/// [`unsatisfied_obligation_keys`]. Without this field two cards with
/// different unmet obligations could collapse into one group even though
/// the canonical family/obligation comment budget would keep them distinct
/// candidates.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct GroupKey {
    file: String,
    class: &'static str,
    baseline_state: &'static str,
    contract_coverage: &'static str,
    guard_coverage: &'static str,
    test_reach_coverage: &'static str,
    witness_receipt_coverage: &'static str,
    next_action: String,
    missing_obligations: Vec<String>,
    shape: String,
}

/// One equivalence-class group of `target_feature`-family `ReviewCard`s.
///
/// Field names follow the candidate projection in issue #1894 and avoid
/// duplicating fields already owned by `ReviewCard`/`CoverageBlock` -- this
/// struct only adds grouping/counting, not new evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TargetFeatureGroup {
    /// Always `"target_feature_repetition"` -- reserved for future group kinds.
    pub(crate) group_kind: &'static str,
    /// Stable, deterministic identifier for this group. A digest of the full
    /// `GroupKey` (see [`group_id`]), so it depends only on this group's own
    /// key -- never on how many sibling groups exist or in what order they
    /// were discovered.
    pub(crate) group_id: String,
    /// Source file this group is scoped to (display-normalized, forward
    /// slashes).
    pub(crate) module_or_file: String,
    /// The shared `ReviewClass` of every card in this group (as_str form).
    pub(crate) class: &'static str,
    /// The shared baseline movement posture of every card in this group.
    pub(crate) baseline_state: &'static str,
    /// Every underlying card ID in this group, in the group's deterministic
    /// order (ascending line, then card id). This is the complete
    /// membership list -- not bounded -- so a consumer can always resolve
    /// the full set without guessing.
    pub(crate) underlying_card_ids: Vec<String>,
    /// Total `target_feature` cards in this group.
    pub(crate) total: usize,
    /// Bounded, deterministic sample of `underlying_card_ids` for inline
    /// display, capped at `TARGET_FEATURE_SUMMARY_REPRESENTATIVE_LIMIT`.
    pub(crate) representatives: Vec<String>,
    /// Deduplicated, sorted architecture/feature literals observed across
    /// the group's operation expressions (e.g. `avx2`, `sse2`, `neon`).
    /// Metadata only -- never part of `GroupKey` / group identity.
    pub(crate) features: Vec<String>,
}

/// Derive deterministic target-feature-repetition groups from `output.cards`.
///
/// Filters to `operation_family == target_feature` and groups by [`GroupKey`]
/// -- every matching card is placed in exactly one group regardless of
/// comment-plan eligibility (that is a separate, narrower concern; see the
/// module docs). Groups with a `new`/`worsened` baseline posture sort ahead
/// of other groups (ties broken by descending size, then file, then group
/// id) so a changed obligation cannot be hidden behind unchanged repetition
/// volume.
///
/// Returns an empty `Vec` when there are no `target_feature` cards --
/// callers must render nothing new in that case so quiet PRs stay quiet.
pub(crate) fn target_feature_groups(output: &AnalyzeOutput) -> Vec<TargetFeatureGroup> {
    let mut by_key: BTreeMap<GroupKey, Vec<&ReviewCard>> = BTreeMap::new();
    for card in &output.cards {
        if card.operation.family != OperationFamily::TargetFeature {
            continue;
        }
        by_key
            .entry(group_key(card, output))
            .or_default()
            .push(card);
    }

    let mut groups: Vec<TargetFeatureGroup> = by_key
        .into_iter()
        .map(|(key, cards)| build_group(key, cards))
        .collect();

    groups.sort_by(|left, right| {
        let left_new = is_new_movement(left.baseline_state);
        let right_new = is_new_movement(right.baseline_state);
        // Groups with a new/worsened baseline posture sort first.
        right_new
            .cmp(&left_new)
            .then_with(|| right.total.cmp(&left.total))
            .then_with(|| left.module_or_file.cmp(&right.module_or_file))
            .then_with(|| left.group_id.cmp(&right.group_id))
    });
    groups
}

fn is_new_movement(baseline_state: &str) -> bool {
    matches!(baseline_state, "new" | "worsened")
}

fn group_key(card: &ReviewCard, output: &AnalyzeOutput) -> GroupKey {
    let block = coverage_block_with_movement(card, output);
    GroupKey {
        file: path_display(&card.site.location.file),
        class: card.class.as_str(),
        baseline_state: block.baseline_state.as_str(),
        contract_coverage: block.contract_coverage.as_str(),
        guard_coverage: block.guard_coverage.as_str(),
        test_reach_coverage: block.test_reach_coverage.as_str(),
        witness_receipt_coverage: block.witness_receipt_coverage.as_str(),
        next_action: card.next_action.summary.clone(),
        missing_obligations: unsatisfied_obligation_keys(card),
        shape: normalized_shape(&card.operation.expression),
    }
}

/// Sorted, deduplicated set of unsatisfied obligation keys for `card`.
///
/// Mirrors `comment_plan::model::comment_budget_key`'s obligation derivation
/// exactly (an obligation is "unsatisfied" when its contract, discharge,
/// reach, or witness evidence is not all present), minus the
/// `operation.family` prefix -- callers of this function already filter to
/// one family (`target_feature`) before deriving a key, so the prefix would
/// be a constant. Falls back to `["review"]` when every obligation is fully
/// discharged, exactly like `comment_budget_key`, so a fully-discharged
/// card's key is still well-defined and distinct from one with a real unmet
/// obligation.
fn unsatisfied_obligation_keys(card: &ReviewCard) -> Vec<String> {
    let mut obligations: Vec<String> = card
        .obligation_evidence
        .iter()
        .filter(|evidence| {
            !evidence.contract.present
                || !evidence.discharge.present
                || !evidence.reach.present
                || !evidence.witness.present
        })
        .map(|evidence| evidence.obligation.key.clone())
        .collect();
    obligations.sort_unstable();
    obligations.dedup();
    if obligations.is_empty() {
        obligations.push("review".to_string());
    }
    obligations
}

/// Compute `card`'s coverage block with snapshot-slot movement applied, the
/// same derivation `declaration_summary::is_new_or_worsened` and every other
/// baseline-movement surface uses (SPEC-0030 single-truth) -- not a second
/// classifier, just the shared coverage block read from a different call
/// site.
fn coverage_block_with_movement(card: &ReviewCard, output: &AnalyzeOutput) -> CoverageBlock {
    let mut block = card.coverage_block();
    if let Some(snapshot) = output.coverage_snapshot.get(&card.id.0) {
        block.apply_snapshot_slots(
            &snapshot.contract_coverage,
            &snapshot.guard_coverage,
            &snapshot.test_reach_coverage,
            &snapshot.witness_receipt_coverage,
        );
    }
    block
}

/// Normalize a `target_feature` operation expression into an
/// architecture/feature-invariant shape by replacing every quoted string
/// literal with a placeholder.
///
/// `#[target_feature(enable = "avx2")]` and `#[target_feature(enable =
/// "neon")]` normalize to the identical shape `#[target_feature(enable =
/// "*")]`; a `cfg_attr`-wrapped variant normalizes to its own distinct shape
/// (the surrounding attribute text differs), which is intentionally
/// conservative -- it never merges syntactically different attribute forms.
fn normalized_shape(expression: &str) -> String {
    let mut shape = String::with_capacity(expression.len());
    let mut in_quotes = false;
    for ch in expression.chars() {
        if ch == '"' {
            in_quotes = !in_quotes;
            shape.push('"');
            if in_quotes {
                shape.push('*');
            }
            continue;
        }
        if in_quotes {
            continue;
        }
        shape.push(ch);
    }
    shape.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Extract the raw contents of every quoted string literal in `expression`
/// (e.g. `avx2` from `enable = "avx2"`). Used only to populate the
/// `features` metadata field; never used for group identity.
fn feature_literals(expression: &str) -> Vec<String> {
    let mut literals = Vec::new();
    let mut chars = expression.char_indices();
    while let Some((start, ch)) = chars.next() {
        if ch != '"' {
            continue;
        }
        let rest = &expression[start + 1..];
        if let Some(end) = rest.find('"') {
            literals.push(rest[..end].to_string());
            // Skip past the consumed literal and its closing quote.
            for _ in 0..=end {
                chars.next();
            }
        }
    }
    literals
}

fn build_group(key: GroupKey, mut cards: Vec<&ReviewCard>) -> TargetFeatureGroup {
    // Deterministic in-group order: ascending line, then card id.
    cards.sort_by(|left, right| {
        left.site
            .location
            .line
            .cmp(&right.site.location.line)
            .then_with(|| left.id.0.cmp(&right.id.0))
    });

    let underlying_card_ids: Vec<String> = cards.iter().map(|card| card.id.to_string()).collect();
    let representatives = underlying_card_ids
        .iter()
        .take(TARGET_FEATURE_SUMMARY_REPRESENTATIVE_LIMIT)
        .cloned()
        .collect();

    let mut features_set: BTreeSet<String> = BTreeSet::new();
    for card in &cards {
        features_set.extend(feature_literals(&card.operation.expression));
    }

    let total = cards.len();
    TargetFeatureGroup {
        group_kind: TARGET_FEATURE_GROUP_KIND,
        group_id: group_id(&key),
        module_or_file: key.file.clone(),
        class: key.class,
        baseline_state: key.baseline_state,
        underlying_card_ids,
        total,
        representatives,
        features: features_set.into_iter().collect(),
    }
}

/// Stable, injective group identifier: a digest of the FULL `GroupKey`, so a
/// group's id depends only on its own key -- never on how many sibling
/// groups exist, their discovery order, or whether an unrelated group is
/// inserted or removed elsewhere in the same output (issue #1894 finding 5).
///
/// Uses the same `stable_hash_hex` FNV-1a primitive and 12-hex-char
/// truncation convention `analysis::pipeline::card_identity` already uses
/// for embedding a content hash into a readable identifier, so collision
/// risk is the same well-understood, already-accepted risk profile as a
/// `ReviewCard` id.
fn group_id(key: &GroupKey) -> String {
    let canonical = [
        key.file.as_str(),
        key.class,
        key.baseline_state,
        key.contract_coverage,
        key.guard_coverage,
        key.test_reach_coverage,
        key.witness_receipt_coverage,
        key.next_action.as_str(),
        &key.missing_obligations.join(&GROUP_ID_LIST_SEP.to_string()),
        key.shape.as_str(),
    ]
    .join(&GROUP_ID_FIELD_SEP.to_string());
    let digest = stable_hash_hex(&canonical);
    format!(
        "target-feature::{}::{}::{}",
        key.file,
        slug(key.class),
        &digest[..12]
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{PolicyMode, Scope, Summary};
    use crate::domain::{
        CardId, Confidence, ContractEvidence, DischargeEvidence, EvidenceState, HazardKind,
        MissingEvidence, NextAction, ObligationEvidence, Priority, ProofPath, ReachEvidence,
        ReviewClass, SafetyObligation, SourceLocation, UnsafeOperation, UnsafeSite, UnsafeSiteKind,
        WitnessEvidence,
    };
    use crate::freshness::AnalysisIdentity;
    use std::collections::BTreeSet as StdBTreeSet;

    fn target_feature_card(id: &str, file: &str, line: usize, feature: &str) -> ReviewCard {
        target_feature_card_with_obligation(id, file, line, feature, "target-feature")
    }

    fn target_feature_card_with_obligation(
        id: &str,
        file: &str,
        line: usize,
        feature: &str,
        obligation_key: &str,
    ) -> ReviewCard {
        let expression = format!("#[target_feature(enable = \"{feature}\")]");
        ReviewCard {
            id: CardId(id.to_string()),
            class: ReviewClass::ContractMissing,
            priority: Priority::High,
            confidence: Confidence::High,
            proof_path: ProofPath::HumanReviewOnly,
            site: UnsafeSite {
                location: SourceLocation {
                    file: file.into(),
                    line,
                    column: 1,
                },
                kind: UnsafeSiteKind::Operation,
                owner: Some(format!("sum_{feature}")),
                visibility: "private".to_string(),
                public_api_surface: false,
                changed: true,
                snippet: expression.clone(),
            },
            operation: UnsafeOperation {
                expression,
                family: OperationFamily::TargetFeature,
            },
            hazards: vec![HazardKind::TargetFeature],
            obligations: vec![],
            obligation_evidence: vec![ObligationEvidence {
                obligation: SafetyObligation::new(
                    obligation_key,
                    "callers only execute this path on supported hardware",
                ),
                contract: EvidenceState::missing("no safety contract"),
                discharge: EvidenceState::missing("no guard"),
                reach: EvidenceState::missing("no tests"),
                witness: EvidenceState::missing("no receipt"),
            }],
            contract: ContractEvidence::missing(),
            discharge: DischargeEvidence::missing(),
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
                summary: "Add a precise `# Safety` section or `SAFETY:` / `Safety:` comment that names the required conditions.".to_string(),
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
            diff_scoped_files: StdBTreeSet::new(),
            coverage_snapshot: BTreeMap::new(),
        }
    }

    #[test]
    fn quiet_output_has_no_target_feature_groups() {
        let output = output_with_cards(vec![]);
        assert!(target_feature_groups(&output).is_empty());
    }

    #[test]
    fn non_target_feature_cards_are_excluded() {
        let mut card = target_feature_card("UR-c1", "src/lib.rs", 1, "avx2");
        card.operation.family = OperationFamily::RawPointerDeref;
        let output = output_with_cards(vec![card]);
        assert!(target_feature_groups(&output).is_empty());
    }

    #[test]
    fn equivalent_arch_variants_collapse_into_one_group() {
        let cards = vec![
            target_feature_card("UR-c1", "src/simd.rs", 3, "avx2"),
            target_feature_card("UR-c2", "src/simd.rs", 8, "sse2"),
            target_feature_card("UR-c3", "src/simd.rs", 13, "neon"),
        ];
        let output = output_with_cards(cards);
        let groups = target_feature_groups(&output);

        assert_eq!(
            groups.len(),
            1,
            "three equivalent arch variants group into one row"
        );
        let group = &groups[0];
        assert_eq!(group.total, 3);
        assert_eq!(group.class, "contract_missing");
        assert_eq!(
            group.underlying_card_ids,
            vec![
                "UR-c1".to_string(),
                "UR-c2".to_string(),
                "UR-c3".to_string()
            ]
        );
        assert_eq!(group.features, vec!["avx2", "neon", "sse2"]);
        assert_eq!(group.group_kind, TARGET_FEATURE_GROUP_KIND);
    }

    #[test]
    fn a_card_with_a_different_class_never_collapses_into_the_repetition_group()
    -> Result<(), String> {
        let mut documented = target_feature_card("UR-c4", "src/simd.rs", 20, "avx512f");
        // A documented site has a satisfied obligation; its class differs
        // from the undocumented sites, so it must NOT collapse with them
        // even though it shares the same file and normalized shape.
        documented.class = ReviewClass::UnsafeUnreached;
        documented.contract = ContractEvidence::present("safety docs present");
        documented.discharge = DischargeEvidence::present("target-feature contract discharge");
        documented.obligation_evidence = vec![ObligationEvidence {
            obligation: SafetyObligation::new(
                "target-feature",
                "callers only execute this path on supported hardware",
            ),
            contract: EvidenceState::present("safety docs present"),
            discharge: EvidenceState::present("target-feature contract discharge"),
            reach: EvidenceState::missing("no tests"),
            witness: EvidenceState::missing("no receipt"),
        }];
        documented.next_action = NextAction {
            summary: "Add or identify a focused test path that reaches the safe wrapper around this unsafe seam.".to_string(),
            verify_commands: vec![],
        };

        let cards = vec![
            target_feature_card("UR-c1", "src/simd.rs", 3, "avx2"),
            target_feature_card("UR-c2", "src/simd.rs", 8, "sse2"),
            documented,
        ];
        let output = output_with_cards(cards);
        let groups = target_feature_groups(&output);

        assert_eq!(
            groups.len(),
            2,
            "the documented, differently-classed site must form its own group"
        );
        let repetition_group = groups
            .iter()
            .find(|group| group.total == 2)
            .ok_or_else(|| "undocumented pair should still group".to_string())?;
        assert_eq!(repetition_group.class, "contract_missing");
        let singleton = groups
            .iter()
            .find(|group| group.total == 1)
            .ok_or_else(|| "documented site should be its own singleton group".to_string())?;
        assert_eq!(singleton.class, "unsafe_unreached");
        assert_eq!(singleton.underlying_card_ids, vec!["UR-c4".to_string()]);
        Ok(())
    }

    /// Issue #1894 finding 2: two `target_feature` cards that share file,
    /// class, coverage, and shape but have DIFFERENT unsatisfied-obligation
    /// sets must never collapse into the same group -- the canonical
    /// family/obligation comment budget (`comment_budget_key`) keeps them
    /// distinct candidates, so the grouping identity must too.
    #[test]
    fn cards_with_different_unsatisfied_obligations_never_collapse() {
        let cards = vec![
            target_feature_card_with_obligation(
                "UR-c1",
                "src/simd.rs",
                3,
                "avx2",
                "target-feature",
            ),
            target_feature_card_with_obligation(
                "UR-c2",
                "src/simd.rs",
                8,
                "sse2",
                "target-feature-alt",
            ),
        ];
        let output = output_with_cards(cards);
        let groups = target_feature_groups(&output);

        assert_eq!(
            groups.len(),
            2,
            "different unsatisfied-obligation keys must produce distinct groups; got {groups:?}"
        );
        assert!(groups.iter().all(|group| group.total == 1));
    }

    #[test]
    fn representatives_are_bounded_and_full_membership_is_preserved() {
        let cards = (0..6)
            .map(|i| target_feature_card(&format!("UR-c{i}"), "src/simd.rs", i + 1, "avx2"))
            .collect();
        let output = output_with_cards(cards);
        let groups = target_feature_groups(&output);
        assert_eq!(groups.len(), 1);
        let group = &groups[0];
        assert_eq!(group.total, 6);
        assert_eq!(group.underlying_card_ids.len(), 6);
        assert_eq!(
            group.representatives.len(),
            TARGET_FEATURE_SUMMARY_REPRESENTATIVE_LIMIT
        );
        // Representatives are a prefix of the deterministic membership order.
        assert_eq!(
            group.representatives,
            group.underlying_card_ids[..TARGET_FEATURE_SUMMARY_REPRESENTATIVE_LIMIT]
        );
    }

    /// Issue #1894 finding 5: a group's `group_id` must depend only on its
    /// own `GroupKey`, never on how many sibling groups exist in the same
    /// output or the order they were discovered in.
    #[test]
    fn group_id_is_stable_when_an_unrelated_sibling_group_is_inserted() -> Result<(), String> {
        let base_output =
            output_with_cards(vec![target_feature_card("UR-c1", "src/simd.rs", 3, "avx2")]);
        let base_groups = target_feature_groups(&base_output);
        assert_eq!(base_groups.len(), 1);
        let base_id = base_groups[0].group_id.clone();

        // Insert an unrelated sibling group (different file, different
        // class) ahead of and behind the original card's group in BTreeMap
        // iteration order.
        let mut documented = target_feature_card("UR-z9", "zzz/other.rs", 1, "neon");
        documented.class = ReviewClass::UnsafeUnreached;
        let mut earlier = target_feature_card("UR-a1", "aaa/other.rs", 1, "sse2");
        earlier.class = ReviewClass::GuardMissing;

        let widened_output = output_with_cards(vec![
            target_feature_card("UR-c1", "src/simd.rs", 3, "avx2"),
            documented,
            earlier,
        ]);
        let widened_groups = target_feature_groups(&widened_output);
        assert_eq!(widened_groups.len(), 3);
        let same_group = widened_groups
            .iter()
            .find(|group| group.underlying_card_ids == vec!["UR-c1".to_string()])
            .ok_or_else(|| "original card's group must still be present".to_string())?;
        assert_eq!(
            same_group.group_id, base_id,
            "group_id must not change when unrelated sibling groups are inserted"
        );
        Ok(())
    }

    #[test]
    fn normalized_shape_ignores_the_feature_literal() {
        assert_eq!(
            normalized_shape("#[target_feature(enable = \"avx2\")]"),
            normalized_shape("#[target_feature(enable = \"neon\")]")
        );
        assert_eq!(
            normalized_shape("#[target_feature(enable = \"avx2\")]"),
            "#[target_feature(enable = \"*\")]"
        );
    }

    #[test]
    fn feature_literals_extracts_quoted_substrings() {
        assert_eq!(
            feature_literals("#[target_feature(enable = \"avx2\")]"),
            vec!["avx2".to_string()]
        );
        assert_eq!(
            feature_literals(
                "#[cfg_attr(target_arch = \"aarch64\", target_feature(enable = \"neon\"))]"
            ),
            vec!["aarch64".to_string(), "neon".to_string()]
        );
    }

    #[test]
    fn unsatisfied_obligation_keys_falls_back_to_review_when_fully_discharged() {
        let mut card = target_feature_card("UR-c1", "src/simd.rs", 3, "avx2");
        card.obligation_evidence = vec![ObligationEvidence {
            obligation: SafetyObligation::new("target-feature", "desc"),
            contract: EvidenceState::present("ok"),
            discharge: EvidenceState::present("ok"),
            reach: EvidenceState::present("ok"),
            witness: EvidenceState::present("ok"),
        }];
        assert_eq!(
            unsatisfied_obligation_keys(&card),
            vec!["review".to_string()]
        );
    }

    #[test]
    fn grouping_is_deterministic_across_repeated_derivation() {
        let cards = vec![
            target_feature_card("UR-c1", "src/b.rs", 1, "avx2"),
            target_feature_card("UR-c2", "src/a.rs", 3, "sse2"),
            target_feature_card("UR-c3", "src/a.rs", 1, "neon"),
        ];
        let output = output_with_cards(cards);
        let first = target_feature_groups(&output);
        let second = target_feature_groups(&output);
        assert_eq!(first, second);
    }
}
