//! Baseline health classification (SPEC-0030 extension, issue #1893).
//!
//! Pure, deterministic classification of each baseline ledger entry, plus every
//! currently open actionable [`ReviewCard`] not represented by the ledger, into one of
//! ten health buckets. Every signal used here already exists in `unsafe-review-core`:
//! [`SnapshotCoverage::is_worsened_by`]/[`SnapshotCoverage::is_improved_by`] (movement),
//! exact counted card-id matching (identity), and `review_after` string comparison
//! (expiry). This module does not invent a second movement model, does not change the
//! exact-identity matching contract, and does not add fuzzy/structural identity
//! fallback (SPEC-0030 non-goal; issue #1893 non-goal).
//!
//! `today` is always injected by the caller ([`BaselineHealthInput::today`]) so
//! [`classify`] never reads the system clock — classification stays deterministic and
//! unit-testable without mocking time.
//!
//! Trust boundary: every bucket here is a debt-record / coverage-evidence
//! classification. None of these buckets is a safety, UB-free, Miri-clean, or
//! site-execution claim; a baseline pass remains a no-new-debt statement only.

use super::{RawLedgerEntry, SnapshotCoverage, looks_like_iso_date};
use crate::domain::ReviewCard;
use crate::domain::coverage::CoverageBlock;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// One of the ten SPEC-0030 baseline-health buckets (issue #1893).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthBucket {
    /// Baseline-known card, still open, coverage unchanged since the recorded snapshot.
    ActiveUnchanged,
    /// Baseline-known card, still open, evidence coverage improved (still advisory).
    ActiveImproved,
    /// Baseline-known card, still open, evidence coverage regressed.
    ActiveWorsened,
    /// Baseline ledger entry whose card no longer appears in the current scan. When
    /// [`BaselineHealthReport::card_scan_error`] is set, the scan itself could not
    /// run — `resolved` then means "unverifiable", not "confirmed gone".
    Resolved,
    /// Baseline ledger entry whose `review_after` date has passed.
    ReviewDue,
    /// No usable coverage-snapshot floor for this card (file missing, invalid TOML, or
    /// this card_id absent from an otherwise-valid snapshot).
    SnapshotMissingOrInvalid,
    /// `card_id` appears more than once in the baseline ledger file.
    DuplicateOrConflictingEntry,
    /// `card_id` is recorded in both the baseline ledger and the active suppression
    /// ledger.
    SuppressionOverlap,
    /// `card_id` does not satisfy the exact counted `UR-*-cN` identity contract (so it
    /// can never be matched under the current identity rule), OR the entry is otherwise
    /// structurally invalid: missing/empty `owner`/`reason`/`evidence`, or a
    /// present-but-malformed `review_after` date. There is no eleventh bucket for
    /// malformed metadata — a structurally broken entry is, definitionally, an entry
    /// this module cannot trust to classify as healthy, so it is folded into
    /// `identity_unmatched` rather than silently falling through to
    /// `active_unchanged`/`resolved`.
    IdentityUnmatched,
    /// A currently open actionable card that the baseline ledger does not represent.
    NewUnbaselined,
}

impl HealthBucket {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ActiveUnchanged => "active_unchanged",
            Self::ActiveImproved => "active_improved",
            Self::ActiveWorsened => "active_worsened",
            Self::Resolved => "resolved",
            Self::ReviewDue => "review_due",
            Self::SnapshotMissingOrInvalid => "snapshot_missing_or_invalid",
            Self::DuplicateOrConflictingEntry => "duplicate_or_conflicting_entry",
            Self::SuppressionOverlap => "suppression_overlap",
            Self::IdentityUnmatched => "identity_unmatched",
            Self::NewUnbaselined => "new_unbaselined",
        }
    }

    /// `true` for every bucket except the two "nothing to do" outcomes
    /// (`active_unchanged`, `resolved`). Drives the bounded `pr` warning
    /// (issue #1893 §Integration): `pr` only warns when at least one entry needs
    /// attention.
    pub fn needs_attention(self) -> bool {
        !matches!(self, Self::ActiveUnchanged | Self::Resolved)
    }
}

/// One classified ledger entry or unbaselined card.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct BaselineHealthEntry {
    pub card_id: String,
    pub bucket: HealthBucket,
    /// Short human-readable explanation of why this entry landed in `bucket`.
    pub detail: String,
}

/// Bucket counts, mirrored 1:1 with [`HealthBucket`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct BaselineHealthCounts {
    pub active_unchanged: usize,
    pub active_improved: usize,
    pub active_worsened: usize,
    pub resolved: usize,
    pub review_due: usize,
    pub snapshot_missing_or_invalid: usize,
    pub duplicate_or_conflicting_entry: usize,
    pub suppression_overlap: usize,
    pub identity_unmatched: usize,
    pub new_unbaselined: usize,
}

impl BaselineHealthCounts {
    fn record(&mut self, bucket: HealthBucket) {
        match bucket {
            HealthBucket::ActiveUnchanged => self.active_unchanged += 1,
            HealthBucket::ActiveImproved => self.active_improved += 1,
            HealthBucket::ActiveWorsened => self.active_worsened += 1,
            HealthBucket::Resolved => self.resolved += 1,
            HealthBucket::ReviewDue => self.review_due += 1,
            HealthBucket::SnapshotMissingOrInvalid => self.snapshot_missing_or_invalid += 1,
            HealthBucket::DuplicateOrConflictingEntry => self.duplicate_or_conflicting_entry += 1,
            HealthBucket::SuppressionOverlap => self.suppression_overlap += 1,
            HealthBucket::IdentityUnmatched => self.identity_unmatched += 1,
            HealthBucket::NewUnbaselined => self.new_unbaselined += 1,
        }
    }

    pub fn total(&self) -> usize {
        self.active_unchanged
            + self.active_improved
            + self.active_worsened
            + self.resolved
            + self.review_due
            + self.snapshot_missing_or_invalid
            + self.duplicate_or_conflicting_entry
            + self.suppression_overlap
            + self.identity_unmatched
            + self.new_unbaselined
    }

    /// `true` when every entry landed in the two "nothing to do" buckets
    /// (`active_unchanged`, `resolved`). Used by the bounded `pr` integration warning.
    pub fn is_fully_healthy(&self) -> bool {
        self.active_improved == 0
            && self.active_worsened == 0
            && self.review_due == 0
            && self.snapshot_missing_or_invalid == 0
            && self.duplicate_or_conflicting_entry == 0
            && self.suppression_overlap == 0
            && self.identity_unmatched == 0
            && self.new_unbaselined == 0
    }
}

/// Full baseline-health classification result (`baseline status`, issue #1893).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct BaselineHealthReport {
    /// The date injected by the caller and used for `review_after` comparison.
    pub today: String,
    /// One entry per distinct baseline card_id, plus one per unbaselined open
    /// actionable card. Sorted by `(card_id, bucket)` for determinism.
    pub entries: Vec<BaselineHealthEntry>,
    pub counts: BaselineHealthCounts,
    /// Set when the coverage snapshot file exists but failed to parse — explains why
    /// every ledger entry may have landed in `snapshot_missing_or_invalid`.
    pub snapshot_load_error: Option<String>,
    /// Set by the caller (not by [`classify`]) when the repo-wide card scan could not
    /// run because the baseline ledger itself failed the analyzer's own strict
    /// per-entry validation — exactly the condition `identity_unmatched` exists to
    /// diagnose (issue #1893 review finding). When set, `current_cards` was empty for
    /// this classification: every `resolved` bucket in `entries` means "no current
    /// card was found" in the *degraded, scan-unavailable* sense, not a confirmed
    /// disappearance — the entry itself may still be genuinely present in the
    /// repository. `identity_unmatched`/`duplicate_or_conflicting_entry`/
    /// `suppression_overlap` classifications are unaffected: they never depend on
    /// card data.
    pub card_scan_error: Option<String>,
}

/// Inputs to [`classify`]. Every field is already-loaded data — `classify` performs no
/// file I/O and reads no clock, so it is pure and deterministic.
pub struct BaselineHealthInput<'a> {
    pub today: &'a str,
    /// All cards from the current full-repo scan (not filtered to actionable) —
    /// mirrors the `current_ids` signal used by the canonical SPEC-0030 `Summary`
    /// movement computation (`resolved_gaps`).
    pub current_cards: &'a [ReviewCard],
    pub ledger_entries: &'a [RawLedgerEntry],
    /// Card IDs covered by a **currently active (non-expired)** suppression entry. The
    /// caller is responsible for filtering out expired suppressions before building
    /// this set (via the shared `policy::is_expired` predicate) — an expired
    /// suppression must not count toward `suppression_overlap`, since it is already
    /// surfaced as its own ledger-health problem elsewhere (`policy report`'s
    /// `expired_suppressions`).
    pub suppression_ids: &'a BTreeSet<String>,
    /// `None` means the coverage snapshot file exists but failed to parse (invalid
    /// TOML). A missing file is represented by the caller as `Some(&empty map)` — a
    /// missing file and a validly-parsed-but-empty file both simply mean "no floor
    /// recorded for any card"; per-entry lookups land in `SnapshotMissingOrInvalid`
    /// either way.
    pub snapshot: Option<&'a BTreeMap<String, SnapshotCoverage>>,
    pub snapshot_load_error: Option<&'a str>,
}

/// Classify every baseline ledger entry and every unbaselined open actionable card.
pub fn classify(input: &BaselineHealthInput<'_>) -> BaselineHealthReport {
    let current_by_id: BTreeMap<&str, &ReviewCard> = input
        .current_cards
        .iter()
        .map(|card| (card.id.0.as_str(), card))
        .collect();

    let mut occurrences: BTreeMap<&str, usize> = BTreeMap::new();
    for entry in input.ledger_entries {
        *occurrences.entry(entry.card_id.as_str()).or_insert(0) += 1;
    }

    let mut entries = Vec::new();
    let mut counts = BaselineHealthCounts::default();
    let mut seen_ids: BTreeSet<&str> = BTreeSet::new();

    for entry in input.ledger_entries {
        if !seen_ids.insert(entry.card_id.as_str()) {
            // Duplicate raw row for an id already classified once — the first
            // occurrence already recorded DuplicateOrConflictingEntry for this id.
            continue;
        }
        let (bucket, detail) = classify_entry(entry, &occurrences, input, &current_by_id);
        counts.record(bucket);
        entries.push(BaselineHealthEntry {
            card_id: entry.card_id.clone(),
            bucket,
            detail,
        });
    }

    for card in input.current_cards {
        if card.class.is_actionable() && !seen_ids.contains(card.id.0.as_str()) {
            counts.record(HealthBucket::NewUnbaselined);
            entries.push(BaselineHealthEntry {
                card_id: card.id.0.clone(),
                bucket: HealthBucket::NewUnbaselined,
                detail: "open actionable card is not represented in the baseline ledger"
                    .to_string(),
            });
        }
    }

    entries.sort_by(|a, b| a.card_id.cmp(&b.card_id).then(a.bucket.cmp(&b.bucket)));

    BaselineHealthReport {
        today: input.today.to_string(),
        entries,
        counts,
        snapshot_load_error: input.snapshot_load_error.map(ToOwned::to_owned),
        // Set by the caller after `classify` returns (`api.rs`), not here: `classify`
        // has no idea whether `current_cards` is empty because the repo genuinely has
        // no cards, or because the repo-wide scan couldn't run at all.
        card_scan_error: None,
    }
}

/// Classify a single ledger entry. Precedence (first match wins), top to bottom:
/// duplicate > identity_unmatched (bad card_id shape OR missing/empty
/// owner/reason/evidence OR malformed review_after) > suppression_overlap > resolved >
/// review_due > snapshot_missing_or_invalid > active_worsened > active_improved >
/// active_unchanged.
fn classify_entry<'a>(
    entry: &RawLedgerEntry,
    occurrences: &BTreeMap<&str, usize>,
    input: &BaselineHealthInput<'a>,
    current_by_id: &BTreeMap<&str, &'a ReviewCard>,
) -> (HealthBucket, String) {
    let id = entry.card_id.as_str();

    let occurrence_count = occurrences.get(id).copied().unwrap_or(0);
    if occurrence_count > 1 {
        return (
            HealthBucket::DuplicateOrConflictingEntry,
            format!("card_id appears {occurrence_count} times in the baseline ledger"),
        );
    }
    let structural_problems = structural_problems(entry);
    if !structural_problems.is_empty() {
        return (
            HealthBucket::IdentityUnmatched,
            format!(
                "ledger entry is structurally invalid: {}",
                structural_problems.join("; ")
            ),
        );
    }
    if input.suppression_ids.contains(id) {
        return (
            HealthBucket::SuppressionOverlap,
            "card_id is also recorded in the active suppression ledger".to_string(),
        );
    }
    let Some(card) = current_by_id.get(id) else {
        return (
            HealthBucket::Resolved,
            "baseline entry's card no longer appears in the current scan".to_string(),
        );
    };
    if let Some(review_after) = entry.review_after.as_deref()
        && review_after < input.today
    {
        return (
            HealthBucket::ReviewDue,
            format!(
                "review_after {review_after} has passed (today: {})",
                input.today
            ),
        );
    }
    let Some(snapshot_map) = input.snapshot else {
        return (
            HealthBucket::SnapshotMissingOrInvalid,
            "coverage snapshot file is missing or failed to parse".to_string(),
        );
    };
    let Some(baseline_cov) = snapshot_map.get(id) else {
        return (
            HealthBucket::SnapshotMissingOrInvalid,
            "no coverage snapshot entry recorded for this card_id".to_string(),
        );
    };
    let current_cov = SnapshotCoverage::from(&CoverageBlock::derive(card));
    if baseline_cov.is_worsened_by(&current_cov) {
        return (
            HealthBucket::ActiveWorsened,
            "coverage regressed since the recorded snapshot".to_string(),
        );
    }
    if baseline_cov.is_improved_by(&current_cov) {
        return (
            HealthBucket::ActiveImproved,
            "coverage improved since the recorded snapshot; still advisory, not a safety claim"
                .to_string(),
        );
    }
    (
        HealthBucket::ActiveUnchanged,
        "coverage matches the recorded snapshot".to_string(),
    )
}

/// List every reason `entry` is structurally invalid: a card_id that fails the exact
/// counted-identity shape check, missing/empty `owner`/`reason`/`evidence` (already
/// collapsed to `None` by the lenient loader's `optional_string`), or a missing or
/// malformed `review_after` date. Empty means the entry is structurally sound (its
/// bucket may still be anything else). `review_after` is schema-required on every
/// baseline entry (SPEC-0030; enforced by the strict loader elsewhere), so a row
/// lacking it entirely is exactly as invalid as one with a malformed date — neither
/// should fall through to `classify_entry`'s `review_due`/`active_*` checks and look
/// healthy.
fn structural_problems(entry: &RawLedgerEntry) -> Vec<&'static str> {
    let mut problems = Vec::new();
    if !entry.valid_identity {
        problems.push("card_id does not match the exact counted UR-*-cN identity contract");
    }
    if entry.owner.is_none() {
        problems.push("missing owner");
    }
    if entry.reason.is_none() {
        problems.push("missing reason");
    }
    if entry.evidence.is_none() {
        problems.push("missing evidence");
    }
    match entry.review_after.as_deref() {
        None => problems.push("missing review_after"),
        Some(date) if !looks_like_iso_date(date) => problems.push("malformed review_after date"),
        Some(_) => {}
    }
    problems
}

/// Refresh-plan action for one ledger entry (`baseline refresh --dry-run`, issue #1893).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RefreshAction {
    /// Coverage and review date are both current — no change needed.
    Keep,
    /// Coverage evidence improved; the recorded snapshot floor is stale and could be
    /// raised to match. Safe to automate in a future apply mode (floor only moves up).
    UpdateSnapshot,
    /// The baseline entry's card is gone; the entry can be pruned. Never auto-applied —
    /// no resolved entry is silently deleted (issue #1893 acceptance criterion).
    MarkResolved,
    /// `review_after` has passed. Only advanced with explicit owner review — never
    /// auto-applied.
    AdvanceReviewAfter,
    /// A current open actionable card is not represented by the floor. Adding it is a
    /// separate, explicit decision — never auto-applied (issue #1893 acceptance
    /// criterion: no new debt is silently accepted).
    AddNewDebt,
    /// Requires human resolution: duplicate/conflicting entries, suppression overlap,
    /// an unmatched identity, a regressed card, or a missing/invalid snapshot.
    Conflict,
}

impl RefreshAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Keep => "keep",
            Self::UpdateSnapshot => "update_snapshot",
            Self::MarkResolved => "mark_resolved",
            Self::AdvanceReviewAfter => "advance_review_after",
            Self::AddNewDebt => "add_new_debt",
            Self::Conflict => "conflict",
        }
    }
}

fn action_for(bucket: HealthBucket) -> RefreshAction {
    match bucket {
        HealthBucket::ActiveUnchanged => RefreshAction::Keep,
        HealthBucket::ActiveImproved => RefreshAction::UpdateSnapshot,
        HealthBucket::ActiveWorsened => RefreshAction::Conflict,
        HealthBucket::Resolved => RefreshAction::MarkResolved,
        HealthBucket::ReviewDue => RefreshAction::AdvanceReviewAfter,
        HealthBucket::SnapshotMissingOrInvalid => RefreshAction::Conflict,
        HealthBucket::DuplicateOrConflictingEntry => RefreshAction::Conflict,
        HealthBucket::SuppressionOverlap => RefreshAction::Conflict,
        HealthBucket::IdentityUnmatched => RefreshAction::Conflict,
        HealthBucket::NewUnbaselined => RefreshAction::AddNewDebt,
    }
}

/// `true` only for the two non-destructive, no-judgment-call actions. Informational
/// only — this issue ships no apply mode, so nothing here is actually applied; the
/// field previews what a future, separately-approved apply command could safely
/// automate.
fn auto_eligible_for(action: RefreshAction) -> bool {
    matches!(action, RefreshAction::Keep | RefreshAction::UpdateSnapshot)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RefreshPlanEntry {
    pub card_id: String,
    pub bucket: HealthBucket,
    pub action: RefreshAction,
    pub auto_eligible: bool,
    pub detail: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct RefreshPlanSummary {
    pub keep: usize,
    pub update_snapshot: usize,
    pub mark_resolved: usize,
    pub advance_review_after: usize,
    pub add_new_debt: usize,
    pub conflict: usize,
}

impl RefreshPlanSummary {
    fn record(&mut self, action: RefreshAction) {
        match action {
            RefreshAction::Keep => self.keep += 1,
            RefreshAction::UpdateSnapshot => self.update_snapshot += 1,
            RefreshAction::MarkResolved => self.mark_resolved += 1,
            RefreshAction::AdvanceReviewAfter => self.advance_review_after += 1,
            RefreshAction::AddNewDebt => self.add_new_debt += 1,
            RefreshAction::Conflict => self.conflict += 1,
        }
    }
}

/// Deterministic per-entry refresh preview (`baseline refresh --dry-run`, issue #1893).
/// Writes nothing; a future apply mode is out of scope (see module docs).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct BaselineRefreshPlan {
    pub today: String,
    /// Sorted by `(card_id, bucket)`, inherited from the source health report.
    pub entries: Vec<RefreshPlanEntry>,
    pub summary: RefreshPlanSummary,
}

/// Build the refresh-preview plan from an already-classified health report. Pure
/// function of `report` — same input always yields the same plan (dry-run determinism,
/// issue #1893 acceptance criterion).
pub fn build_refresh_plan(report: &BaselineHealthReport) -> BaselineRefreshPlan {
    let mut summary = RefreshPlanSummary::default();
    let entries = report
        .entries
        .iter()
        .map(|entry| {
            let action = action_for(entry.bucket);
            summary.record(action);
            RefreshPlanEntry {
                card_id: entry.card_id.clone(),
                bucket: entry.bucket,
                action,
                auto_eligible: auto_eligible_for(action),
                detail: entry.detail.clone(),
            }
        })
        .collect();
    BaselineRefreshPlan {
        today: report.today.clone(),
        entries,
        summary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{AnalysisMode, AnalyzeInput, DiffSource, PolicyMode, Scope, analyze};
    use std::path::PathBuf;

    const TODAY: &str = "2026-07-18";

    fn fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
    }

    /// Real analyzed card from a fixture that ships one actionable `guard_missing` card
    /// with `contract.present = true`, `discharge.present = false` (so its derived
    /// coverage block is neither all-`present` nor all-`missing`; safe for worsened /
    /// improved extreme-snapshot tests below).
    fn analyzed_card() -> Result<ReviewCard, String> {
        let root = fixture_path("raw_pointer_alignment");
        let output = analyze(AnalyzeInput {
            root,
            scope: Scope::Repo,
            diff: DiffSource::NoneRepoScan,
            mode: AnalysisMode::Repo,
            policy: PolicyMode::Advisory,
            include_unchanged_tests: true,
            max_cards: None,
        })?;
        output
            .cards
            .into_iter()
            .next()
            .ok_or_else(|| "fixture produced no card".to_string())
    }

    fn raw_entry(card_id: &str) -> RawLedgerEntry {
        RawLedgerEntry {
            card_id: card_id.to_string(),
            owner: Some("owner".to_string()),
            reason: Some("reason".to_string()),
            evidence: Some("evidence".to_string()),
            review_after: Some("2099-01-01".to_string()),
            valid_identity: true,
        }
    }

    fn snap(contract: &str, guard: &str, test_reach: &str, witness: &str) -> SnapshotCoverage {
        SnapshotCoverage {
            contract_coverage: contract.to_string(),
            guard_coverage: guard.to_string(),
            test_reach_coverage: test_reach.to_string(),
            witness_receipt_coverage: witness.to_string(),
        }
    }

    fn empty_suppression() -> BTreeSet<String> {
        BTreeSet::new()
    }

    #[test]
    fn bucket_as_str_matches_serde_rename() -> Result<(), String> {
        let buckets = [
            HealthBucket::ActiveUnchanged,
            HealthBucket::ActiveImproved,
            HealthBucket::ActiveWorsened,
            HealthBucket::Resolved,
            HealthBucket::ReviewDue,
            HealthBucket::SnapshotMissingOrInvalid,
            HealthBucket::DuplicateOrConflictingEntry,
            HealthBucket::SuppressionOverlap,
            HealthBucket::IdentityUnmatched,
            HealthBucket::NewUnbaselined,
        ];
        for bucket in buckets {
            let json = serde_json::to_string(&bucket)
                .map_err(|err| format!("serialize {bucket:?} failed: {err}"))?;
            assert_eq!(json, format!("\"{}\"", bucket.as_str()));
        }
        Ok(())
    }

    #[test]
    fn active_unchanged_when_snapshot_matches_current() -> Result<(), String> {
        let card = analyzed_card()?;
        let baseline_cov = SnapshotCoverage::from(&CoverageBlock::derive(&card));
        let entries = vec![raw_entry(&card.id.0)];
        let suppression = empty_suppression();
        let mut snapshot_map = BTreeMap::new();
        snapshot_map.insert(card.id.0.clone(), baseline_cov);
        let cards = vec![card];

        let report = classify(&BaselineHealthInput {
            today: TODAY,
            current_cards: &cards,
            ledger_entries: &entries,
            suppression_ids: &suppression,
            snapshot: Some(&snapshot_map),
            snapshot_load_error: None,
        });

        assert_eq!(report.counts.active_unchanged, 1);
        assert_eq!(report.entries[0].bucket, HealthBucket::ActiveUnchanged);
        Ok(())
    }

    #[test]
    fn active_improved_when_snapshot_is_all_missing() -> Result<(), String> {
        let card = analyzed_card()?;
        let entries = vec![raw_entry(&card.id.0)];
        let suppression = empty_suppression();
        let mut snapshot_map = BTreeMap::new();
        snapshot_map.insert(
            card.id.0.clone(),
            snap("missing", "missing", "missing", "missing"),
        );
        let cards = vec![card];

        let report = classify(&BaselineHealthInput {
            today: TODAY,
            current_cards: &cards,
            ledger_entries: &entries,
            suppression_ids: &suppression,
            snapshot: Some(&snapshot_map),
            snapshot_load_error: None,
        });

        assert_eq!(
            report.counts.active_improved, 1,
            "fixture card has contract.present=true, so an all-missing baseline must be improved: {:?}",
            report.entries
        );
        Ok(())
    }

    #[test]
    fn active_worsened_when_snapshot_is_all_present() -> Result<(), String> {
        let card = analyzed_card()?;
        let entries = vec![raw_entry(&card.id.0)];
        let suppression = empty_suppression();
        let mut snapshot_map = BTreeMap::new();
        snapshot_map.insert(
            card.id.0.clone(),
            snap("present", "present", "present", "present"),
        );
        let cards = vec![card];

        let report = classify(&BaselineHealthInput {
            today: TODAY,
            current_cards: &cards,
            ledger_entries: &entries,
            suppression_ids: &suppression,
            snapshot: Some(&snapshot_map),
            snapshot_load_error: None,
        });

        assert_eq!(
            report.counts.active_worsened, 1,
            "fixture card has discharge.present=false (guard missing), so an all-present \
             baseline must be worsened: {:?}",
            report.entries
        );
        Ok(())
    }

    #[test]
    fn review_due_takes_precedence_over_snapshot_state() -> Result<(), String> {
        let card = analyzed_card()?;
        let mut entry = raw_entry(&card.id.0);
        entry.review_after = Some("2020-01-01".to_string());
        let entries = vec![entry];
        let suppression = empty_suppression();
        let cards = vec![card];

        // snapshot: None (globally invalid) — review_due must still win, proving
        // precedence order: review_due is checked before snapshot state.
        let report = classify(&BaselineHealthInput {
            today: TODAY,
            current_cards: &cards,
            ledger_entries: &entries,
            suppression_ids: &suppression,
            snapshot: None,
            snapshot_load_error: Some("boom: invalid TOML"),
        });

        assert_eq!(report.counts.review_due, 1);
        assert_eq!(report.counts.snapshot_missing_or_invalid, 0);
        assert_eq!(
            report.snapshot_load_error.as_deref(),
            Some("boom: invalid TOML")
        );
        Ok(())
    }

    #[test]
    fn snapshot_missing_or_invalid_when_card_absent_from_valid_snapshot() -> Result<(), String> {
        let card = analyzed_card()?;
        let entries = vec![raw_entry(&card.id.0)];
        let suppression = empty_suppression();
        let snapshot_map: BTreeMap<String, SnapshotCoverage> = BTreeMap::new();
        let cards = vec![card];

        let report = classify(&BaselineHealthInput {
            today: TODAY,
            current_cards: &cards,
            ledger_entries: &entries,
            suppression_ids: &suppression,
            snapshot: Some(&snapshot_map),
            snapshot_load_error: None,
        });

        assert_eq!(report.counts.snapshot_missing_or_invalid, 1);
        Ok(())
    }

    #[test]
    fn resolved_when_card_id_has_no_current_match() {
        let entries = vec![raw_entry(
            "UR-gone-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1",
        )];
        let suppression = empty_suppression();
        let cards: Vec<ReviewCard> = Vec::new();

        let report = classify(&BaselineHealthInput {
            today: TODAY,
            current_cards: &cards,
            ledger_entries: &entries,
            suppression_ids: &suppression,
            snapshot: None,
            snapshot_load_error: None,
        });

        assert_eq!(report.counts.resolved, 1);
        assert_eq!(report.entries[0].bucket, HealthBucket::Resolved);
    }

    #[test]
    fn duplicate_or_conflicting_entry_when_card_id_repeated() {
        let id =
            "UR-dup-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1";
        let entries = vec![raw_entry(id), raw_entry(id)];
        let suppression = empty_suppression();
        let cards: Vec<ReviewCard> = Vec::new();

        let report = classify(&BaselineHealthInput {
            today: TODAY,
            current_cards: &cards,
            ledger_entries: &entries,
            suppression_ids: &suppression,
            snapshot: None,
            snapshot_load_error: None,
        });

        // Exactly one entry is reported for the duplicated id (identities, not raw rows).
        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.counts.duplicate_or_conflicting_entry, 1);
        assert_eq!(
            report.entries[0].bucket,
            HealthBucket::DuplicateOrConflictingEntry
        );
    }

    #[test]
    fn identity_unmatched_when_card_id_fails_shape_check() {
        let mut entry = raw_entry("not-a-valid-identity");
        entry.valid_identity = false;
        let entries = vec![entry];
        let suppression = empty_suppression();
        let cards: Vec<ReviewCard> = Vec::new();

        let report = classify(&BaselineHealthInput {
            today: TODAY,
            current_cards: &cards,
            ledger_entries: &entries,
            suppression_ids: &suppression,
            snapshot: None,
            snapshot_load_error: None,
        });

        assert_eq!(report.counts.identity_unmatched, 1);
        assert_eq!(report.entries[0].bucket, HealthBucket::IdentityUnmatched);
    }

    #[test]
    fn identity_unmatched_when_metadata_is_missing_or_review_after_is_malformed()
    -> Result<(), String> {
        let id =
            "UR-broken-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1";
        let mut missing_owner = raw_entry(id);
        missing_owner.owner = None;
        let mut malformed_date = raw_entry(
            "UR-broken2-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1",
        );
        malformed_date.review_after = Some("not-a-date".to_string());
        // A missing review_after (schema-required on every baseline entry) must be
        // treated exactly like a malformed one — without the fix this card_id has no
        // current match, so it would silently fall through to `resolved` instead.
        let missing_review_after_id =
            "UR-broken3-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1";
        let mut missing_review_after = raw_entry(missing_review_after_id);
        missing_review_after.review_after = None;
        let entries = vec![missing_owner, malformed_date, missing_review_after];
        let suppression = empty_suppression();
        let cards: Vec<ReviewCard> = Vec::new();

        let report = classify(&BaselineHealthInput {
            today: TODAY,
            current_cards: &cards,
            ledger_entries: &entries,
            suppression_ids: &suppression,
            snapshot: None,
            snapshot_load_error: None,
        });

        // No entry falls through to active_unchanged/resolved looking healthy — all
        // three are folded into the existing identity_unmatched bucket (no 11th bucket).
        assert_eq!(report.counts.identity_unmatched, 3, "{:?}", report.entries);
        assert_eq!(report.counts.active_unchanged, 0);
        assert_eq!(report.counts.resolved, 0);
        for entry in &report.entries {
            assert_eq!(entry.bucket, HealthBucket::IdentityUnmatched);
        }
        let owner_entry = report
            .entries
            .iter()
            .find(|entry| entry.card_id == id)
            .ok_or("missing-owner entry must be present")?;
        assert!(
            owner_entry.detail.contains("missing owner"),
            "{owner_entry:?}"
        );
        let missing_review_after_entry = report
            .entries
            .iter()
            .find(|entry| entry.card_id == missing_review_after_id)
            .ok_or("missing-review_after entry must be present")?;
        assert!(
            missing_review_after_entry
                .detail
                .contains("missing review_after"),
            "{missing_review_after_entry:?}"
        );
        Ok(())
    }

    #[test]
    fn suppression_overlap_when_card_id_also_suppressed() {
        let id =
            "UR-both-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1";
        let entries = vec![raw_entry(id)];
        let mut suppression = empty_suppression();
        suppression.insert(id.to_string());
        let cards: Vec<ReviewCard> = Vec::new();

        let report = classify(&BaselineHealthInput {
            today: TODAY,
            current_cards: &cards,
            ledger_entries: &entries,
            suppression_ids: &suppression,
            snapshot: None,
            snapshot_load_error: None,
        });

        assert_eq!(report.counts.suppression_overlap, 1);
        assert_eq!(report.entries[0].bucket, HealthBucket::SuppressionOverlap);
    }

    #[test]
    fn new_unbaselined_for_current_actionable_card_not_in_ledger() -> Result<(), String> {
        let card = analyzed_card()?;
        assert!(
            card.class.is_actionable(),
            "fixture card must be actionable for this test to be meaningful"
        );
        let entries: Vec<RawLedgerEntry> = Vec::new();
        let suppression = empty_suppression();
        let cards = vec![card];

        let report = classify(&BaselineHealthInput {
            today: TODAY,
            current_cards: &cards,
            ledger_entries: &entries,
            suppression_ids: &suppression,
            snapshot: None,
            snapshot_load_error: None,
        });

        assert_eq!(report.counts.new_unbaselined, 1);
        assert_eq!(report.entries[0].bucket, HealthBucket::NewUnbaselined);
        Ok(())
    }

    #[test]
    fn classify_is_deterministic_for_the_same_input() -> Result<(), String> {
        let card = analyzed_card()?;
        let entries = vec![raw_entry(&card.id.0)];
        let suppression = empty_suppression();
        let cards = vec![card];

        let input = BaselineHealthInput {
            today: TODAY,
            current_cards: &cards,
            ledger_entries: &entries,
            suppression_ids: &suppression,
            snapshot: None,
            snapshot_load_error: None,
        };
        let first = classify(&input);
        let second = classify(&input);
        assert_eq!(first, second);
        Ok(())
    }

    #[test]
    fn counts_is_fully_healthy_true_only_for_unchanged_and_resolved() {
        let mut counts = BaselineHealthCounts::default();
        assert!(counts.is_fully_healthy());
        counts.record(HealthBucket::ActiveUnchanged);
        counts.record(HealthBucket::Resolved);
        assert!(counts.is_fully_healthy());
        counts.record(HealthBucket::ReviewDue);
        assert!(!counts.is_fully_healthy());
    }

    #[test]
    fn refresh_plan_maps_buckets_to_documented_actions() -> Result<(), String> {
        let card = analyzed_card()?;
        let entries = vec![raw_entry(&card.id.0)];
        let suppression = empty_suppression();
        let cards = vec![card];

        let health = classify(&BaselineHealthInput {
            today: TODAY,
            current_cards: &cards,
            ledger_entries: &entries,
            suppression_ids: &suppression,
            snapshot: None, // -> snapshot_missing_or_invalid -> conflict
            snapshot_load_error: None,
        });
        let plan = build_refresh_plan(&health);

        assert_eq!(plan.entries.len(), 1);
        assert_eq!(
            plan.entries[0].bucket,
            HealthBucket::SnapshotMissingOrInvalid
        );
        assert_eq!(plan.entries[0].action, RefreshAction::Conflict);
        assert!(!plan.entries[0].auto_eligible);
        assert_eq!(plan.summary.conflict, 1);
        Ok(())
    }

    #[test]
    fn refresh_plan_action_never_auto_for_resolved_review_or_new_debt() {
        for bucket in [
            HealthBucket::Resolved,
            HealthBucket::ReviewDue,
            HealthBucket::NewUnbaselined,
        ] {
            let action = action_for(bucket);
            assert!(
                !auto_eligible_for(action),
                "{bucket:?} -> {action:?} must never be auto-eligible \
                 (no silent deletion / no silent debt acceptance)"
            );
        }
    }

    #[test]
    fn refresh_plan_is_deterministic_for_the_same_report() -> Result<(), String> {
        let card = analyzed_card()?;
        let entries = vec![raw_entry(&card.id.0)];
        let suppression = empty_suppression();
        let cards = vec![card];
        let health = classify(&BaselineHealthInput {
            today: TODAY,
            current_cards: &cards,
            ledger_entries: &entries,
            suppression_ids: &suppression,
            snapshot: None,
            snapshot_load_error: None,
        });

        let first = build_refresh_plan(&health);
        let second = build_refresh_plan(&health);
        assert_eq!(first, second);
        Ok(())
    }
}
