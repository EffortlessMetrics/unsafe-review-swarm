use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
use crate::output::{NO_CHANGED_GAPS_LIMITATION, NO_CHANGED_GAPS_MESSAGE};

mod markdown;
mod witness;

const TRUST_BOUNDARY: &str = "Static unsafe contract review outcome only; this compares existing ReviewCard snapshots, not memory-safety proof, not UB-free status, and not witness execution.";

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OutcomeReport {
    pub schema_version: String,
    pub tool: String,
    pub mode: String,
    pub before_id: String,
    pub after_id: String,
    pub trust_boundary: String,
    pub limitations: Vec<String>,
    pub before: OutcomeSnapshotSummary,
    pub after: OutcomeSnapshotSummary,
    pub summary: OutcomeSummary,
    pub reviewer_delta: OutcomeReviewerDelta,
    pub cards: OutcomeCards,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OutcomeSnapshotSummary {
    pub schema_version: String,
    pub cards: usize,
    pub open_actionable_gaps: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct OutcomeSummary {
    pub new: usize,
    pub resolved: usize,
    pub improved: usize,
    pub regressed: usize,
    pub unchanged: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct OutcomeReviewerDelta {
    pub new_cards: usize,
    pub resolved_cards: usize,
    pub improved_cards: usize,
    pub regressed_cards: usize,
    pub unchanged_cards: usize,
    pub receipt_movement: OutcomeReceiptMovement,
    pub top_remaining_gaps: Vec<OutcomeRemainingGap>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct OutcomeReceiptMovement {
    pub improved: usize,
    pub regressed: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OutcomeRemainingGap {
    pub card_id: String,
    #[serde(rename = "class")]
    pub class_name: String,
    pub priority: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_family: Option<String>,
    pub missing_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_action: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct OutcomeCards {
    pub new: Vec<OutcomeCard>,
    pub resolved: Vec<OutcomeCard>,
    pub improved: Vec<OutcomeCard>,
    pub regressed: Vec<OutcomeCard>,
    pub unchanged: Vec<OutcomeCard>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OutcomeCard {
    pub card_id: String,
    pub reason: String,
    pub before: Option<OutcomeCardState>,
    pub after: Option<OutcomeCardState>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OutcomeCardState {
    #[serde(rename = "class")]
    pub class_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_family: Option<String>,
    pub priority: String,
    pub missing_count: usize,
    pub witness: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_action: Option<String>,
    pub missing: Vec<String>,
}

#[derive(Deserialize)]
struct Snapshot {
    schema_version: String,
    summary: SnapshotSummary,
    cards: Vec<SnapshotCard>,
}

#[derive(Deserialize)]
struct SnapshotSummary {
    cards: usize,
    open_actionable_gaps: usize,
}

#[derive(Clone, Deserialize)]
struct SnapshotCard {
    id: String,
    #[serde(rename = "class")]
    class_name: String,
    #[serde(default)]
    operation: Option<String>,
    #[serde(default)]
    operation_family: Option<String>,
    priority: String,
    #[serde(default)]
    witness: String,
    #[serde(default)]
    next_action: Option<String>,
    #[serde(default)]
    missing: Vec<String>,
}

pub fn compare_json(before_json: &str, after_json: &str) -> Result<OutcomeReport, String> {
    let before = parse_snapshot(before_json, "before")?;
    let after = parse_snapshot(after_json, "after")?;
    compare_snapshots(before, after)
}

pub fn render_json(report: &OutcomeReport) -> String {
    match serde_json::to_string_pretty(report) {
        Ok(text) => text,
        Err(err) => format!("{{\n  \"error\": \"outcome serialization failed: {err}\"\n}}"),
    }
}

pub fn render_markdown(report: &OutcomeReport) -> String {
    markdown::render_markdown(report)
}

fn parse_snapshot(text: &str, label: &str) -> Result<Snapshot, String> {
    let snapshot: Snapshot = serde_json::from_str(text)
        .map_err(|err| format!("parse {label} unsafe-review JSON snapshot failed: {err}"))?;
    if snapshot.schema_version.trim().is_empty() {
        return Err(format!("{label} snapshot is missing `schema_version`"));
    }
    if snapshot.summary.cards != snapshot.cards.len() {
        return Err(format!(
            "{label} snapshot summary card count {} does not match {} card object(s)",
            snapshot.summary.cards,
            snapshot.cards.len()
        ));
    }
    Ok(snapshot)
}

fn compare_snapshots(before: Snapshot, after: Snapshot) -> Result<OutcomeReport, String> {
    let before_id = snapshot_id(&before);
    let after_id = snapshot_id(&after);
    let before_summary = OutcomeSnapshotSummary::from(&before);
    let after_summary = OutcomeSnapshotSummary::from(&after);
    let before_cards = cards_by_id(before.cards, "before")?;
    let after_cards = cards_by_id(after.cards, "after")?;
    let ids = before_cards
        .keys()
        .chain(after_cards.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut summary = OutcomeSummary::default();
    let mut cards = OutcomeCards::default();
    for id in ids {
        let before = before_cards.get(&id);
        let after = after_cards.get(&id);
        let status = outcome_status(before, after);
        let reason = outcome_reason(status, before, after);
        let card = OutcomeCard {
            card_id: id,
            reason,
            before: before.map(OutcomeCardState::from),
            after: after.map(OutcomeCardState::from),
        };
        match status {
            "new" => {
                summary.new += 1;
                cards.new.push(card);
            }
            "resolved" => {
                summary.resolved += 1;
                cards.resolved.push(card);
            }
            "improved" => {
                summary.improved += 1;
                cards.improved.push(card);
            }
            "regressed" => {
                summary.regressed += 1;
                cards.regressed.push(card);
            }
            _ => {
                summary.unchanged += 1;
                cards.unchanged.push(card);
            }
        }
    }
    let reviewer_delta = OutcomeReviewerDelta::from_outcome(&summary, &cards);
    Ok(OutcomeReport {
        schema_version: "0.1".to_string(),
        tool: "unsafe-review".to_string(),
        mode: "outcome".to_string(),
        before_id,
        after_id,
        trust_boundary: TRUST_BOUNDARY.to_string(),
        limitations: vec![
            "compares existing saved ReviewCard JSON snapshots only".to_string(),
            "does not rerun analysis or execute witness tools".to_string(),
            "does not make policy or blocking decisions".to_string(),
        ],
        before: before_summary,
        after: after_summary,
        summary,
        reviewer_delta,
        cards,
    })
}

fn cards_by_id(
    cards: Vec<SnapshotCard>,
    label: &str,
) -> Result<BTreeMap<String, SnapshotCard>, String> {
    let mut by_id = BTreeMap::new();
    for card in cards {
        if by_id.insert(card.id.clone(), card).is_some() {
            return Err(format!("{label} snapshot contains duplicate card id"));
        }
    }
    Ok(by_id)
}

fn outcome_status(before: Option<&SnapshotCard>, after: Option<&SnapshotCard>) -> &'static str {
    match (before, after) {
        (None, Some(_)) => "new",
        (Some(_), None) => "resolved",
        (Some(before), Some(after)) => changed_status(before, after),
        (None, None) => "unchanged",
    }
}

fn outcome_reason(
    status: &str,
    before: Option<&SnapshotCard>,
    after: Option<&SnapshotCard>,
) -> String {
    match (status, before, after) {
        ("new", None, Some(after)) => format!(
            "card appears in the after snapshot as `{}` with {} missing evidence item(s)",
            after.class_name,
            after.missing.len()
        ),
        ("resolved", Some(before), None) => format!(
            "card was present in the before snapshot as `{}` and is absent from the after snapshot",
            before.class_name
        ),
        ("improved" | "regressed", Some(before), Some(after)) => {
            changed_reason(status, before, after)
        }
        ("unchanged", Some(before), Some(after)) => changed_reason(status, before, after),
        _ => "snapshot membership did not match an expected outcome case".to_string(),
    }
}

fn changed_reason(status: &str, before: &SnapshotCard, after: &SnapshotCard) -> String {
    let mut reasons = Vec::new();
    if before.class_name != after.class_name {
        reasons.push(format!(
            "class changed from `{}` to `{}`",
            before.class_name, after.class_name
        ));
    }
    let before_missing = before.missing.len();
    let after_missing = after.missing.len();
    if before_missing != after_missing {
        reasons.push(format!(
            "missing evidence count changed from {before_missing} to {after_missing}"
        ));
    }
    let before_witness = witness::witness_state(before);
    let after_witness = witness::witness_state(after);
    if before_witness.label != after_witness.label {
        reasons.push(format!(
            "witness receipt strength changed from `{}` to `{}`",
            before_witness.label, after_witness.label
        ));
    }
    if reasons.is_empty() {
        reasons.push("class and missing evidence count are unchanged".to_string());
    }
    format!("{status}: {}", reasons.join("; "))
}

fn changed_status(before: &SnapshotCard, after: &SnapshotCard) -> &'static str {
    let before_actionable = is_actionable_class(&before.class_name);
    let after_actionable = is_actionable_class(&after.class_name);
    if before_actionable && !after_actionable {
        return "improved";
    }
    if !before_actionable && after_actionable {
        return "regressed";
    }
    let before_missing = before.missing.len();
    let after_missing = after.missing.len();
    if after_missing < before_missing {
        "improved"
    } else if after_missing > before_missing {
        "regressed"
    } else if witness::witness_state(after).rank > witness::witness_state(before).rank {
        "improved"
    } else if witness::witness_state(after).rank < witness::witness_state(before).rank {
        "regressed"
    } else {
        "unchanged"
    }
}

fn is_actionable_class(value: &str) -> bool {
    matches!(
        value,
        "guarded_unwitnessed"
            | "contract_missing"
            | "guard_missing"
            | "reachable_unwitnessed"
            | "unsafe_unreached"
            | "witness_mismatch"
            | "requires_loom"
            | "requires_sanitizer"
            | "requires_kani_or_crux"
            | "miri_unsupported"
            | "static_unknown"
    )
}

fn snapshot_id(snapshot: &Snapshot) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    feed_hash(&mut hash, &snapshot.schema_version);
    feed_hash(&mut hash, &snapshot.summary.cards.to_string());
    feed_hash(
        &mut hash,
        &snapshot.summary.open_actionable_gaps.to_string(),
    );
    let mut cards = snapshot.cards.iter().collect::<Vec<_>>();
    cards.sort_by(|left, right| left.id.cmp(&right.id));
    for card in cards {
        feed_hash(&mut hash, &card.id);
        feed_hash(&mut hash, &card.class_name);
        feed_hash(&mut hash, card.operation.as_deref().unwrap_or(""));
        feed_hash(&mut hash, card.operation_family.as_deref().unwrap_or(""));
        feed_hash(&mut hash, &card.priority);
        feed_hash(&mut hash, &card.witness);
        feed_hash(&mut hash, card.next_action.as_deref().unwrap_or(""));
        for missing in &card.missing {
            feed_hash(&mut hash, missing);
        }
    }
    format!("snapshot-{hash:016x}")
}

fn feed_hash(hash: &mut u64, text: &str) {
    for byte in text.bytes().chain([0]) {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

impl OutcomeCards {
    fn is_empty(&self) -> bool {
        self.new.is_empty()
            && self.resolved.is_empty()
            && self.improved.is_empty()
            && self.regressed.is_empty()
            && self.unchanged.is_empty()
    }

    fn groups(&self) -> [(&'static str, &[OutcomeCard]); 5] {
        [
            ("new", &self.new),
            ("resolved", &self.resolved),
            ("improved", &self.improved),
            ("regressed", &self.regressed),
            ("unchanged", &self.unchanged),
        ]
    }
}

impl OutcomeReviewerDelta {
    fn from_outcome(summary: &OutcomeSummary, cards: &OutcomeCards) -> Self {
        let mut delta = Self {
            new_cards: summary.new,
            resolved_cards: summary.resolved,
            improved_cards: summary.improved,
            regressed_cards: summary.regressed,
            unchanged_cards: summary.unchanged,
            receipt_movement: receipt_movement(cards),
            top_remaining_gaps: top_remaining_gaps(cards),
        };
        delta.top_remaining_gaps.truncate(5);
        delta
    }
}

fn receipt_movement(cards: &OutcomeCards) -> OutcomeReceiptMovement {
    let mut movement = OutcomeReceiptMovement::default();
    for card in cards
        .improved
        .iter()
        .chain(cards.regressed.iter())
        .chain(cards.unchanged.iter())
    {
        let Some(before) = card.before.as_ref() else {
            continue;
        };
        let Some(after) = card.after.as_ref() else {
            continue;
        };
        let before_rank = witness::witness_rank(&before.witness);
        let after_rank = witness::witness_rank(&after.witness);
        if after_rank > before_rank {
            movement.improved += 1;
        } else if after_rank < before_rank {
            movement.regressed += 1;
        }
    }
    movement
}

fn top_remaining_gaps(cards: &OutcomeCards) -> Vec<OutcomeRemainingGap> {
    let mut gaps = cards
        .new
        .iter()
        .chain(cards.improved.iter())
        .chain(cards.regressed.iter())
        .chain(cards.unchanged.iter())
        .filter_map(|card| {
            let after = card.after.as_ref()?;
            if !is_actionable_class(&after.class_name) {
                return None;
            }
            Some(OutcomeRemainingGap {
                card_id: card.card_id.clone(),
                class_name: after.class_name.clone(),
                priority: after.priority.clone(),
                operation_family: after.operation_family.clone(),
                missing_count: after.missing_count,
                next_action: after.next_action.clone(),
            })
        })
        .collect::<Vec<_>>();
    gaps.sort_by(|left, right| {
        priority_rank(&left.priority)
            .cmp(&priority_rank(&right.priority))
            .then_with(|| right.missing_count.cmp(&left.missing_count))
            .then_with(|| left.card_id.cmp(&right.card_id))
    });
    gaps
}

fn priority_rank(value: &str) -> u8 {
    match value {
        "high" => 0,
        "medium" => 1,
        "low" => 2,
        _ => 3,
    }
}

impl From<&Snapshot> for OutcomeSnapshotSummary {
    fn from(snapshot: &Snapshot) -> Self {
        Self {
            schema_version: snapshot.schema_version.clone(),
            cards: snapshot.summary.cards,
            open_actionable_gaps: snapshot.summary.open_actionable_gaps,
        }
    }
}

impl From<&SnapshotCard> for OutcomeCardState {
    fn from(card: &SnapshotCard) -> Self {
        Self {
            class_name: card.class_name.clone(),
            operation: card.operation.clone(),
            operation_family: card.operation_family.clone(),
            priority: card.priority.clone(),
            missing_count: card.missing.len(),
            witness: witness::witness_state(card).label,
            next_action: card.next_action.clone(),
            missing: card.missing.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcome_json_reports_new_resolved_improved_regressed_and_unchanged() -> Result<(), String> {
        let before = snapshot_json(&[
            card("UR-a-c1", "guard_missing", "high", &["guard", "witness"]),
            card("UR-b-c1", "guard_missing", "high", &["guard"]),
            card("UR-c-c1", "guarded_and_witnessed", "low", &[]),
            card("UR-d-c1", "guard_missing", "high", &["guard"]),
        ]);
        let after = snapshot_json(&[
            card("UR-a-c1", "guard_missing", "high", &["guard"]),
            card("UR-c-c1", "guard_missing", "high", &["guard"]),
            card("UR-d-c1", "guard_missing", "high", &["guard"]),
            card("UR-e-c1", "contract_missing", "high", &["contract"]),
        ]);

        let report = compare_json(&before, &after)?;

        assert_eq!(report.schema_version, "0.1");
        assert_eq!(report.mode, "outcome");
        assert_eq!(report.summary.new, 1);
        assert_eq!(report.summary.resolved, 1);
        assert_eq!(report.summary.improved, 1);
        assert_eq!(report.summary.regressed, 1);
        assert_eq!(report.summary.unchanged, 1);
        assert_eq!(report.reviewer_delta.new_cards, 1);
        assert_eq!(report.reviewer_delta.resolved_cards, 1);
        assert_eq!(report.reviewer_delta.improved_cards, 1);
        assert_eq!(report.reviewer_delta.regressed_cards, 1);
        assert_eq!(report.reviewer_delta.unchanged_cards, 1);
        assert_eq!(report.reviewer_delta.top_remaining_gaps.len(), 4);
        assert_eq!(report.reviewer_delta.top_remaining_gaps[0].priority, "high");
        assert!(report.before_id.starts_with("snapshot-"));
        assert!(report.after_id.starts_with("snapshot-"));
        assert_eq!(report.cards.new.len(), 1);
        assert_eq!(report.cards.resolved.len(), 1);
        assert_eq!(report.cards.improved.len(), 1);
        assert_eq!(report.cards.regressed.len(), 1);
        assert_eq!(report.cards.unchanged.len(), 1);
        assert!(
            report.cards.improved[0]
                .reason
                .contains("missing evidence count changed from 2 to 1")
        );
        assert!(
            report.cards.regressed[0]
                .reason
                .contains("class changed from `guarded_and_witnessed` to `guard_missing`")
        );
        assert!(
            report
                .trust_boundary
                .contains("compares existing ReviewCard snapshots")
        );
        Ok(())
    }

    #[test]
    fn outcome_renderers_are_parseable_and_keep_trust_boundary() -> Result<(), String> {
        let before = snapshot_json(&[]);
        let after = snapshot_json(&[card("UR-new-c1", "guard_missing", "high", &["guard"])]);
        let report = compare_json(&before, &after)?;
        let json = render_json(&report);
        let value: serde_json::Value =
            serde_json::from_str(&json).map_err(|err| format!("parse JSON failed: {err}"))?;
        assert_eq!(value["mode"], "outcome");
        assert_eq!(value["summary"]["new"], 1);
        assert_eq!(value["reviewer_delta"]["new_cards"], 1);
        assert_eq!(value["reviewer_delta"]["resolved_cards"], 0);
        assert_eq!(
            value["reviewer_delta"]["top_remaining_gaps"][0]["card_id"],
            "UR-new-c1"
        );
        assert_eq!(
            value["reviewer_delta"]["top_remaining_gaps"][0]["operation_family"],
            "raw_pointer_read"
        );
        assert_eq!(value["cards"]["new"][0]["card_id"], "UR-new-c1");
        assert_eq!(
            value["cards"]["new"][0]["after"]["operation_family"],
            "raw_pointer_read"
        );
        assert_eq!(
            value["cards"]["new"][0]["after"]["operation"],
            "unsafe { ptr.cast::<Header>().read() }"
        );
        assert!(
            value["cards"]["new"][0]["after"]["next_action"]
                .as_str()
                .unwrap_or("")
                .contains("Add or expose")
        );
        assert!(
            value["cards"]["new"][0]["reason"]
                .as_str()
                .unwrap_or("")
                .contains("after snapshot")
        );
        assert!(
            value["before_id"]
                .as_str()
                .unwrap_or("")
                .starts_with("snapshot-")
        );
        assert!(
            value["after_id"]
                .as_str()
                .unwrap_or("")
                .starts_with("snapshot-")
        );
        assert!(value["limitations"].is_array());
        assert!(
            value["trust_boundary"]
                .as_str()
                .unwrap_or("")
                .contains("not memory-safety proof")
        );

        let markdown = render_markdown(&report);
        assert!(markdown.contains("# unsafe-review outcome"));
        assert!(markdown.contains("## Reviewer delta"));
        assert!(markdown.contains("- New cards: 1"));
        assert!(markdown.contains("- Receipt movement: 0 improved, 0 regressed"));
        assert!(markdown.contains("Top remaining gaps"));
        assert!(markdown.contains("## Movement reasons"));
        assert!(markdown.contains("- `new` `UR-new-c1`: card appears in the after snapshot"));
        assert!(markdown.contains("| Status | Card | Reason | Before | After |"));
        assert!(markdown.contains("## Limitations"));
        assert!(markdown.contains("## Trust boundary"));
        assert!(markdown.contains("UR-new-c1"));
        assert!(markdown.contains("raw_pointer_read"));
        assert!(markdown.contains("unsafe { ptr.cast::<Header>().read() }"));
        assert!(markdown.contains("Add or expose"));
        Ok(())
    }

    #[test]
    fn outcome_empty_markdown_uses_standard_advisory_wording() -> Result<(), String> {
        let before = snapshot_json(&[]);
        let after = snapshot_json(&[]);
        let report = compare_json(&before, &after)?;
        let markdown = render_markdown(&report);

        assert!(markdown.contains(NO_CHANGED_GAPS_MESSAGE));
        assert!(markdown.contains(NO_CHANGED_GAPS_LIMITATION));
        assert!(markdown.contains(
            "No new, resolved, improved, or regressed ReviewCards in these saved snapshots."
        ));
        assert!(!markdown.contains("All clear"));
        Ok(())
    }

    #[test]
    fn outcome_reports_witness_receipt_improvement_reason() -> Result<(), String> {
        let before = snapshot_json(&[card_with_witness(
            "UR-witness-c1",
            "guard_missing",
            "high",
            &["guard", "witness"],
            "No imported witness receipt was found",
        )]);
        let after = snapshot_json(&[card_with_witness(
            "UR-witness-c1",
            "guard_missing",
            "high",
            &["guard"],
            "Imported miri receipt with `ran` strength: focused fixture witness passed",
        )]);

        let report = compare_json(&before, &after)?;

        assert_eq!(report.summary.improved, 1);
        assert_eq!(report.reviewer_delta.receipt_movement.improved, 1);
        assert_eq!(report.reviewer_delta.receipt_movement.regressed, 0);
        assert!(
            report.cards.improved[0]
                .reason
                .contains("witness receipt strength changed from `missing` to `ran`")
        );
        let after_state = report.cards.improved[0]
            .after
            .as_ref()
            .ok_or("improved card should include after state")?;
        assert_eq!(after_state.witness, "ran");
        Ok(())
    }

    #[test]
    fn outcome_reports_witness_receipt_strength_regression() -> Result<(), String> {
        let before = snapshot_json(&[card_with_witness(
            "UR-witness-c1",
            "guarded_and_witnessed",
            "low",
            &[],
            "Imported miri receipt with `test_targeted` strength: focused fixture witness passed",
        )]);
        let after = snapshot_json(&[card_with_witness(
            "UR-witness-c1",
            "guarded_and_witnessed",
            "low",
            &[],
            "Imported miri receipt with `configured` strength: configured only",
        )]);

        let report = compare_json(&before, &after)?;

        assert_eq!(report.summary.regressed, 1);
        assert_eq!(report.reviewer_delta.receipt_movement.improved, 0);
        assert_eq!(report.reviewer_delta.receipt_movement.regressed, 1);
        assert!(
            report.cards.regressed[0]
                .reason
                .contains("witness receipt strength changed from `test_targeted` to `configured`")
        );
        let before_state = report.cards.regressed[0]
            .before
            .as_ref()
            .ok_or("regressed card should include before state")?;
        let after_state = report.cards.regressed[0]
            .after
            .as_ref()
            .ok_or("regressed card should include after state")?;
        assert_eq!(before_state.witness, "test_targeted");
        assert_eq!(after_state.witness, "configured");
        Ok(())
    }

    #[test]
    fn outcome_rejects_duplicate_card_identity() {
        let before = snapshot_json(&[
            card("UR-dup-c1", "guard_missing", "high", &["guard"]),
            card("UR-dup-c1", "contract_missing", "high", &["contract"]),
        ]);
        let after = snapshot_json(&[]);

        assert!(
            compare_json(&before, &after)
                .err()
                .unwrap_or_default()
                .contains("duplicate card id")
        );
    }

    #[test]
    fn outcome_rejects_summary_card_count_mismatch() {
        let before = r#"{
  "schema_version": "0.1",
  "summary": {
    "cards": 2,
    "open_actionable_gaps": 0
  },
  "cards": []
}"#;
        let after = snapshot_json(&[]);

        assert!(
            compare_json(before, &after)
                .err()
                .unwrap_or_default()
                .contains("summary card count")
        );
    }

    fn snapshot_json(cards: &[String]) -> String {
        format!(
            r#"{{
  "schema_version": "0.1",
  "summary": {{
    "cards": {},
    "open_actionable_gaps": {}
  }},
  "cards": [
    {}
  ]
}}"#,
            cards.len(),
            cards.len(),
            cards.join(",\n    ")
        )
    }

    fn card(id: &str, class_name: &str, priority: &str, missing: &[&str]) -> String {
        card_with_witness(id, class_name, priority, missing, "")
    }

    fn card_with_witness(
        id: &str,
        class_name: &str,
        priority: &str,
        missing: &[&str],
        witness: &str,
    ) -> String {
        let missing = missing
            .iter()
            .map(|item| format!(r#""{item}""#))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            r#"{{
      "id": "{id}",
      "class": "{class_name}",
      "operation": "unsafe {{ ptr.cast::<Header>().read() }}",
      "operation_family": "raw_pointer_read",
      "priority": "{priority}",
      "witness": "{witness}",
      "next_action": "Add or expose a safety contract, guard, test, or witness for raw_pointer_read.",
      "missing": [{missing}]
    }}"#
        )
    }
}
