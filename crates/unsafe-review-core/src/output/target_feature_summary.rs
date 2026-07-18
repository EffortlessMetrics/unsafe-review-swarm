//! Target-feature-summary projection (issue #1894).
//!
//! Real-crate dogfood on SIMD-heavy code (memchr-class multi-arch dispatch)
//! found large sets of structurally similar `#[target_feature]` `ReviewCard`s
//! across architecture and feature variants. Each card can be individually
//! correct while the set overwhelms human-facing summaries and the
//! comment-plan budget.
//!
//! This module groups the *existing* `target_feature`-family `ReviewCard`s
//! by a canonical, obligation-preserving identity so their volume is
//! understandable without deleting, suppressing, or reclassifying any of
//! them. It is a presentation-only projection:
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
//! - Cards whose obligation, review class, movement (baseline state),
//!   receipt state, or next action differ NEVER collapse into the same
//!   group -- see [`GroupKey`].
//!
//! See `docs/specs/UNSAFE-REVIEW-SPEC-0011-pr-ci-output.md` §3.2 for the
//! rendered contract.

use crate::api::AnalyzeOutput;
use crate::domain::{CoverageBlock, OperationFamily, ReviewCard};
use crate::util::path_display;
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

/// Canonical, obligation-preserving group identity for a `target_feature`
/// `ReviewCard`.
///
/// Architecture/feature literals are deliberately excluded: `shape` is the
/// operation expression with every quoted literal normalized away (see
/// [`normalized_shape`]), so `enable = "avx2"` and `enable = "neon"` share a
/// shape. Every other field is an equivalence requirement from issue #1894:
/// two cards group together only when their file, class, baseline movement,
/// full coverage-slot state, and next action are all identical.
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
    /// Stable, deterministic identifier for this group.
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
/// Filters to `operation_family == target_feature`, groups by [`GroupKey`],
/// and ranks groups with a `new`/`worsened` baseline posture ahead of other
/// groups (ties broken by descending size, then file, then group id) so a
/// changed obligation cannot be hidden behind unchanged repetition volume.
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

    // Deterministic ordinal disambiguator: groups that share (file, class,
    // shape) but differ only in baseline/coverage/next-action state (a real
    // but rare case) still get distinct, stable, injective ids. Iteration
    // order over `by_key` is the `GroupKey` `Ord` order, which is itself
    // fully determined by card data, so the assignment below is
    // deterministic across runs.
    let mut ordinal_by_prefix: BTreeMap<(String, &'static str, String), usize> = BTreeMap::new();
    let mut groups: Vec<TargetFeatureGroup> = by_key
        .into_iter()
        .map(|(key, cards)| {
            let prefix = (key.file.clone(), key.class, key.shape.clone());
            let ordinal = ordinal_by_prefix.entry(prefix).or_insert(0);
            *ordinal += 1;
            build_group(key, *ordinal, cards)
        })
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

/// The set of `target_feature` card ids that are non-representative members
/// of a repetition group (`group.total > 1`) -- every id in the group
/// except the first, deterministic representative.
///
/// Used by the comment-plan (SPEC-0022/0032) to select at most one
/// representative per equivalent group; the rest are recorded with the
/// `grouped_repetition` omission reason rather than silently dropped. This
/// is the single source of truth for that exclusion set so the markdown
/// group projection and the comment-plan selection can never drift on which
/// card is "the" representative -- both read `underlying_card_ids[0]`.
pub(crate) fn grouped_repetition_card_ids(output: &AnalyzeOutput) -> BTreeSet<String> {
    target_feature_groups(output)
        .into_iter()
        .filter(|group| group.total > 1)
        .flat_map(|group| group.underlying_card_ids.into_iter().skip(1))
        .collect()
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
        shape: normalized_shape(&card.operation.expression),
    }
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

fn build_group(key: GroupKey, ordinal: usize, mut cards: Vec<&ReviewCard>) -> TargetFeatureGroup {
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
        group_id: group_id(&key, ordinal),
        module_or_file: key.file.clone(),
        class: key.class,
        baseline_state: key.baseline_state,
        underlying_card_ids,
        total,
        representatives,
        features: features_set.into_iter().collect(),
    }
}

fn group_id(key: &GroupKey, ordinal: usize) -> String {
    format!(
        "target-feature::{}::{}::{}#{ordinal}",
        key.file,
        key.class,
        slug(&key.shape)
    )
}

/// Lossy, deterministic slug: lowercase alphanumerics, runs of anything else
/// collapsed to a single `-`, trimmed. Used only to keep `group_id` readable
/// -- the `ordinal` suffix (not this slug) is what guarantees injectivity.
fn slug(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut last_was_dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash && !out.is_empty() {
            out.push('-');
            last_was_dash = true;
        }
    }
    out.trim_end_matches('-').to_string()
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
    use std::collections::BTreeSet as StdBTreeSet;

    fn target_feature_card(id: &str, file: &str, line: usize, feature: &str) -> ReviewCard {
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
            obligation_evidence: vec![],
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

    #[test]
    fn grouped_repetition_ids_exclude_only_the_first_representative() {
        let cards = vec![
            target_feature_card("UR-c1", "src/simd.rs", 3, "avx2"),
            target_feature_card("UR-c2", "src/simd.rs", 8, "sse2"),
            target_feature_card("UR-c3", "src/simd.rs", 13, "neon"),
        ];
        let output = output_with_cards(cards);
        let omitted = grouped_repetition_card_ids(&output);
        assert_eq!(
            omitted,
            StdBTreeSet::from(["UR-c2".to_string(), "UR-c3".to_string()])
        );
    }

    #[test]
    fn grouped_repetition_ids_are_empty_for_a_singleton_group() {
        let output =
            output_with_cards(vec![target_feature_card("UR-c1", "src/simd.rs", 3, "avx2")]);
        assert!(grouped_repetition_card_ids(&output).is_empty());
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
