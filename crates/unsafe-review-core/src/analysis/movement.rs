//! Base/head movement evaluation (#2271 PR1).
//!
//! A pure, additive subject-attribution evaluator: compare two complete
//! [`AnalyzeOutput`]s card by card on canonical subject identity and
//! classify each subject as introduced, worsened, improved, resolved,
//! inherited context, or unknown/not-comparable. It never rewrites
//! policy/baseline semantics, the outcome comparator, or any projection;
//! each entry additionally records the existing ledger-derived baseline
//! state and outcome movement side by side so compatibility can be judged
//! without changing them.

use crate::api::{AnalyzeOutput, Scope};
use crate::domain::ReviewCard;
use std::collections::BTreeMap;

/// Base/head movement verdict for one canonical subject.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubjectMovement {
    /// Present at head, absent from a complete base: the PR introduced it.
    Introduced,
    /// Present on both sides with regressed evidence.
    Worsened,
    /// Present on both sides with improved evidence and the gap still open.
    /// Stronger evidence is never blanket completion.
    Improved,
    /// Present at base, genuinely absent at head under complete comparable
    /// analyses that covered the subject's file.
    Resolved,
    /// Present on both sides without evidence regression: pre-existing
    /// context, not a PR introduction.
    Inherited,
    /// Ambiguous identity or incomparable analyses: no clean verdict.
    Unknown,
}

impl SubjectMovement {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Introduced => "introduced",
            Self::Worsened => "worsened",
            Self::Improved => "improved",
            Self::Resolved => "resolved",
            Self::Inherited => "inherited",
            Self::Unknown => "unknown",
        }
    }
}

/// One subject's movement verdict with its basis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubjectMovementEntry {
    /// Canonical subject identity (card identity without the per-run
    /// `-cN` counter).
    pub subject: String,
    pub base_card_id: Option<String>,
    pub head_card_id: Option<String>,
    pub movement: SubjectMovement,
    pub reason: String,
    /// Existing ledger-derived posture of the current (head if present,
    /// else base) card, recorded side by side, never recomputed here.
    pub ledger_baseline_state: String,
    pub ledger_outcome_movement: String,
}

/// Reconciled summary counts: the six buckets always sum to `subjects`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MovementSummary {
    pub introduced: usize,
    pub worsened: usize,
    pub improved: usize,
    pub resolved: usize,
    pub inherited: usize,
    pub unknown: usize,
    pub subjects: usize,
}

/// Evaluated base/head movement for one subject set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MovementReport {
    pub summary: MovementSummary,
    pub subjects: Vec<SubjectMovementEntry>,
}

/// Canonical subject identity: the card identity without the per-run
/// `-cN` counter suffix, which restarts every analysis and must never
/// distinguish the same subject across base and head.
fn subject_base(id: &str) -> &str {
    match id.rsplit_once("-c") {
        Some((base, count))
            if !base.is_empty()
                && !count.is_empty()
                && count.bytes().all(|byte| byte.is_ascii_digit()) =>
        {
            base
        }
        _ => id,
    }
}

/// Whether one analysis side is complete enough to trust absence.
/// A capped scan or unresolved/rejected diff files mean the scan may have
/// missed cards; absence there proves nothing.
fn side_complete(output: &AnalyzeOutput) -> bool {
    !output.summary.scan_capped
        && output.unresolved_diff_files.is_empty()
        && output.rejected_diff_files.is_empty()
}

/// Whether an analysis side covered `file`: a repo-scope analysis scans
/// everything; a diff-scoped analysis covers only its candidate files.
/// Both disappearance directions consult this so a narrow scan can neither
/// invent introductions nor confirm resolutions for files it never saw.
fn side_covers_file(side: &AnalyzeOutput, file: &std::path::Path) -> bool {
    if side.scope == Scope::Repo {
        return true;
    }
    side.diff_scoped_files.contains(file)
}

/// Evidence movement between two cards for the same subject.
/// Only the first regression or improvement direction is reported;
/// precedence is worsened over improved, mirroring snapshot comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EvidenceMovement {
    Regressed,
    Improved,
    Same,
}

fn compare_evidence(before: &ReviewCard, after: &ReviewCard) -> EvidenceMovement {
    let mut improved = false;
    // Contract presence.
    match (before.contract.present, after.contract.present) {
        (true, false) => return EvidenceMovement::Regressed,
        (false, true) => improved = true,
        _ => {}
    }
    // Per-obligation discharge presence.
    let before_by_key: BTreeMap<&str, bool> = before
        .obligation_evidence
        .iter()
        .map(|evidence| (evidence.obligation.key.as_str(), evidence.discharge.present))
        .collect();
    let after_by_key: BTreeMap<&str, bool> = after
        .obligation_evidence
        .iter()
        .map(|evidence| (evidence.obligation.key.as_str(), evidence.discharge.present))
        .collect();
    if before_by_key.keys().ne(after_by_key.keys()) {
        // Different obligation sets are not cleanly comparable; the caller
        // maps this to Unknown rather than forcing a direction.
        return EvidenceMovement::Same;
    }
    for (key, before_present) in &before_by_key {
        let after_present = after_by_key[key];
        match (*before_present, after_present) {
            (true, false) => return EvidenceMovement::Regressed,
            (false, true) => improved = true,
            _ => {}
        }
    }
    // Witness presence and verdict. Any verdict change on a present
    // witness is new observation (improved knowledge), never a repair
    // verdict: a confirmed observation keeps the defect open by product
    // rule, and a verdict-less import closes witness coverage without
    // resolving the underlying operation.
    match (before.witness.present, after.witness.present) {
        (true, false) => return EvidenceMovement::Regressed,
        (false, true) => improved = true,
        (true, true) if before.witness.verdict != after.witness.verdict => improved = true,
        _ => {}
    }
    // Reach: unreached versus reached.
    let reached = |state: &str| state != "unreached";
    match (
        reached(before.reach.state.as_str()),
        reached(after.reach.state.as_str()),
    ) {
        (true, false) => return EvidenceMovement::Regressed,
        (false, true) => improved = true,
        _ => {}
    }
    if improved {
        EvidenceMovement::Improved
    } else {
        EvidenceMovement::Same
    }
}

fn obligation_keys_match(before: &ReviewCard, after: &ReviewCard) -> bool {
    let mut before_keys: Vec<&str> = before
        .obligation_evidence
        .iter()
        .map(|evidence| evidence.obligation.key.as_str())
        .collect();
    let mut after_keys: Vec<&str> = after
        .obligation_evidence
        .iter()
        .map(|evidence| evidence.obligation.key.as_str())
        .collect();
    before_keys.sort_unstable();
    after_keys.sort_unstable();
    before_keys == after_keys
}

/// Evaluate base/head movement over two analyses.
///
/// Fail-closed comparability: only complete sides license
/// disappearance-based verdicts, and resolution additionally requires the
/// head analysis to have covered the subject's file. Ambiguous duplicate
/// subjects and obligation-set drift yield Unknown, never a forced
/// introduced/resolved pair.
pub fn evaluate_base_head_movement(base: &AnalyzeOutput, head: &AnalyzeOutput) -> MovementReport {
    let base_complete = side_complete(base);
    let head_complete = side_complete(head);
    let mut base_by_subject: BTreeMap<String, Vec<&ReviewCard>> = BTreeMap::new();
    for card in &base.cards {
        base_by_subject
            .entry(subject_base(&card.id.0).to_string())
            .or_default()
            .push(card);
    }
    let mut head_by_subject: BTreeMap<String, Vec<&ReviewCard>> = BTreeMap::new();
    for card in &head.cards {
        head_by_subject
            .entry(subject_base(&card.id.0).to_string())
            .or_default()
            .push(card);
    }
    let mut keys: Vec<String> = base_by_subject.keys().cloned().collect();
    for key in head_by_subject.keys() {
        if !base_by_subject.contains_key(key) {
            keys.push(key.clone());
        }
    }
    keys.sort();
    let mut subjects: Vec<SubjectMovementEntry> = Vec::new();
    let mut introduced = 0usize;
    let mut worsened = 0usize;
    let mut improved = 0usize;
    let mut resolved = 0usize;
    let mut inherited = 0usize;
    let mut unknown = 0usize;
    for subject in keys {
        let base_cards: Vec<&ReviewCard> =
            base_by_subject.get(&subject).cloned().unwrap_or_default();
        let head_cards: Vec<&ReviewCard> =
            head_by_subject.get(&subject).cloned().unwrap_or_default();
        let entry = match (base_cards.as_slice(), head_cards.as_slice()) {
            ([base_card], [head_card]) => compare_subject(&subject, base_card, head_card),
            ([], [head_card]) if base_complete
                && side_covers_file(base, &head_card.site.location.file) =>
            {
                SubjectMovementEntry {
                    subject: subject.clone(),
                    base_card_id: None,
                    head_card_id: Some(head_card.id.0.clone()),
                    movement: SubjectMovement::Introduced,
                    reason: "subject is present at head and absent from the complete base analysis that covered its file"
                        .to_string(),
                    ledger_baseline_state: ledger_of(head_card).0,
                    ledger_outcome_movement: ledger_of(head_card).1,
                }
            }
            ([base_card], []) if head_complete
                && side_covers_file(head, &base_card.site.location.file) =>
            {
                SubjectMovementEntry {
                    subject: subject.clone(),
                    base_card_id: Some(base_card.id.0.clone()),
                    head_card_id: None,
                    movement: SubjectMovement::Resolved,
                    reason: "subject is present at base and genuinely absent from the complete head analysis that covered its file"
                        .to_string(),
                    ledger_baseline_state: ledger_of(base_card).0,
                    ledger_outcome_movement: ledger_of(base_card).1,
                }
            }
            ([], [_]) if !base_complete => unknown_entry(
                &subject,
                &[],
                head_cards.as_slice(),
                "the base analysis is incomplete, so a head-only subject cannot be introduced",
            ),
            ([], [_]) => unknown_entry(
                &subject,
                &[],
                head_cards.as_slice(),
                "the base analysis did not cover the subject's file, so a head-only subject cannot be introduced",
            ),
            ([], _) => unknown_entry(
                &subject,
                &[],
                head_cards.as_slice(),
                "duplicate subject identity at head prevents a clean verdict",
            ),
            (_, []) if !head_complete => unknown_entry(
                &subject,
                base_cards.as_slice(),
                &[],
                "the head analysis is incomplete, so a base-only subject cannot be resolved",
            ),
            (_, []) => unknown_entry(
                &subject,
                base_cards.as_slice(),
                &[],
                "the head analysis did not cover the subject's file, so a base-only subject cannot be resolved",
            ),
            _ => unknown_entry(
                &subject,
                base_cards.as_slice(),
                head_cards.as_slice(),
                "duplicate subject identity on base, head, or both prevents a clean verdict",
            ),
        };
        match entry.movement {
            SubjectMovement::Introduced => introduced += 1,
            SubjectMovement::Worsened => worsened += 1,
            SubjectMovement::Improved => improved += 1,
            SubjectMovement::Resolved => resolved += 1,
            SubjectMovement::Inherited => inherited += 1,
            SubjectMovement::Unknown => unknown += 1,
        }
        subjects.push(entry);
    }
    let summary = MovementSummary {
        introduced,
        worsened,
        improved,
        resolved,
        inherited,
        unknown,
        subjects: subjects.len(),
    };
    MovementReport { summary, subjects }
}

/// Unknown verdict with the best available identity context: the first
/// base card when present, else the first head card.
fn unknown_entry(
    subject: &str,
    base_cards: &[&ReviewCard],
    head_cards: &[&ReviewCard],
    reason: &str,
) -> SubjectMovementEntry {
    let (base_id, head_id, ledger) = match (base_cards.first(), head_cards.first()) {
        (Some(base_card), _) => (
            Some(base_card.id.0.clone()),
            head_cards.first().map(|card| card.id.0.clone()),
            ledger_of(base_card),
        ),
        (None, Some(head_card)) => (None, Some(head_card.id.0.clone()), ledger_of(head_card)),
        (None, None) => (None, None, ("unknown".to_string(), "unknown".to_string())),
    };
    SubjectMovementEntry {
        subject: subject.to_string(),
        base_card_id: base_id,
        head_card_id: head_id,
        movement: SubjectMovement::Unknown,
        reason: reason.to_string(),
        ledger_baseline_state: ledger.0,
        ledger_outcome_movement: ledger.1,
    }
}

fn ledger_of(card: &ReviewCard) -> (String, String) {
    let block = card.coverage_block();
    (
        block.baseline_state.as_str().to_string(),
        block.outcome_movement.as_str().to_string(),
    )
}

fn compare_subject(subject: &str, before: &ReviewCard, after: &ReviewCard) -> SubjectMovementEntry {
    let (ledger_baseline_state, ledger_outcome_movement) = ledger_of(after);
    let entry = |movement, reason: String| SubjectMovementEntry {
        subject: subject.to_string(),
        base_card_id: Some(before.id.0.clone()),
        head_card_id: Some(after.id.0.clone()),
        movement,
        reason,
        ledger_baseline_state: ledger_baseline_state.clone(),
        ledger_outcome_movement: ledger_outcome_movement.clone(),
    };
    if !obligation_keys_match(before, after) {
        return entry(
            SubjectMovement::Unknown,
            "obligation sets differ between base and head; the subjects are not cleanly comparable"
                .to_string(),
        );
    }
    match compare_evidence(before, after) {
        EvidenceMovement::Regressed => entry(
            SubjectMovement::Worsened,
            "same subject remains but evidence regressed at head".to_string(),
        ),
        EvidenceMovement::Improved => entry(
            SubjectMovement::Improved,
            "same subject remains open with improved evidence at head; the underlying operation is not resolved"
                .to_string(),
        ),
        EvidenceMovement::Same => entry(
            SubjectMovement::Inherited,
            "same subject is present on both sides without evidence regression".to_string(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subject_base_strips_exactly_one_counted_suffix() {
        assert_eq!(
            subject_base("UR-crate-src-lib-rs-owner-op-read-deadbeef1234-alignment-c1"),
            "UR-crate-src-lib-rs-owner-op-read-deadbeef1234-alignment"
        );
        assert_eq!(
            subject_base("UR-crate-src-lib-rs-owner-op-read-deadbeef1234-alignment-c12"),
            "UR-crate-src-lib-rs-owner-op-read-deadbeef1234-alignment"
        );
        // A trailing `-c` without digits, or content that merely looks
        // counter-like deeper in the identity, is never stripped.
        assert_eq!(subject_base("UR-no-counter"), "UR-no-counter");
        assert_eq!(subject_base("UR-ends-with-c"), "UR-ends-with-c");
        assert_eq!(subject_base("UR-c1-c2"), "UR-c1");
    }
}
