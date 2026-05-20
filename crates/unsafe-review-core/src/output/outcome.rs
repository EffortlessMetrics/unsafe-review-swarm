use crate::output::markdown_table;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

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
    pub priority: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub site: Option<OutcomeCardSite>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_family: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub hazards: Vec<String>,
    pub missing_count: usize,
    pub witness: String,
    pub missing: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OutcomeCardSite {
    pub file: String,
    pub line: usize,
    pub kind: String,
    pub owner: String,
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
    priority: String,
    #[serde(default)]
    site: Option<SnapshotSite>,
    #[serde(default)]
    operation_family: Option<String>,
    #[serde(default)]
    hazards: Vec<String>,
    #[serde(default)]
    witness: String,
    #[serde(default)]
    missing: Vec<String>,
}

#[derive(Clone, Deserialize)]
struct SnapshotSite {
    file: String,
    line: usize,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    owner: String,
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
    let mut out = String::new();
    out.push_str("# unsafe-review outcome\n\n");
    out.push_str("Static comparison of two existing unsafe-review JSON snapshots.\n\n");
    out.push_str("## Summary\n\n");
    out.push_str("| New | Resolved | Improved | Regressed | Unchanged |\n");
    out.push_str("|---:|---:|---:|---:|---:|\n");
    out.push_str(&format!(
        "| {} | {} | {} | {} | {} |\n\n",
        report.summary.new,
        report.summary.resolved,
        report.summary.improved,
        report.summary.regressed,
        report.summary.unchanged
    ));
    out.push_str("## Card outcomes\n\n");
    if report.cards.is_empty() {
        out.push_str("No cards in either snapshot.\n\n");
    } else {
        out.push_str("| Status | Card | Reason | Before | After |\n");
        out.push_str("|---|---|---|---|---|\n");
        for (status, cards) in report.cards.groups() {
            for card in cards {
                out.push_str(&format!(
                    "| `{status}` | `{}` | {} | {} | {} |\n",
                    markdown_cell(&card.card_id),
                    markdown_cell(&card.reason),
                    markdown_state(card.before.as_ref()),
                    markdown_state(card.after.as_ref())
                ));
            }
        }
        out.push('\n');
    }
    out.push_str("## Limitations\n\n");
    for limitation in &report.limitations {
        out.push_str("- ");
        out.push_str(limitation);
        out.push('\n');
    }
    out.push('\n');
    out.push_str("## Trust boundary\n\n");
    out.push_str(&report.trust_boundary);
    out.push('\n');
    out
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
    let before_witness = witness_state(before);
    let after_witness = witness_state(after);
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
    } else if witness_state(after).rank > witness_state(before).rank {
        "improved"
    } else if witness_state(after).rank < witness_state(before).rank {
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
        feed_hash(&mut hash, &card.priority);
        if let Some(site) = &card.site {
            feed_hash(&mut hash, &site.file);
            feed_hash(&mut hash, &site.line.to_string());
            feed_hash(&mut hash, &site.kind);
            feed_hash(&mut hash, &site.owner);
        }
        if let Some(operation_family) = &card.operation_family {
            feed_hash(&mut hash, operation_family);
        }
        for hazard in &card.hazards {
            feed_hash(&mut hash, hazard);
        }
        feed_hash(&mut hash, &card.witness);
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

fn markdown_state(state: Option<&OutcomeCardState>) -> String {
    match state {
        Some(state) => {
            let mut parts = vec![
                format!("`{}`", markdown_cell(&state.class_name)),
                format!("`{}`", markdown_cell(&state.priority)),
                format!("{} missing", state.missing_count),
                format!("witness `{}`", markdown_cell(&state.witness)),
            ];
            if let Some(site) = &state.site {
                parts.push(format!(
                    "site `{}:{}`",
                    markdown_cell(&site.file),
                    site.line
                ));
            }
            if let Some(operation_family) = &state.operation_family {
                parts.push(format!("operation `{}`", markdown_cell(operation_family)));
            }
            if !state.hazards.is_empty() {
                parts.push(format!(
                    "hazards `{}`",
                    markdown_cell(&state.hazards.join(", "))
                ));
            }
            parts.join(" / ")
        }
        None => "-".to_string(),
    }
}

fn markdown_cell(value: &str) -> String {
    markdown_table::cell(value)
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
            priority: card.priority.clone(),
            site: card.site.as_ref().map(OutcomeCardSite::from),
            operation_family: card.operation_family.clone(),
            hazards: card.hazards.clone(),
            missing_count: card.missing.len(),
            witness: witness_state(card).label,
            missing: card.missing.clone(),
        }
    }
}

impl From<&SnapshotSite> for OutcomeCardSite {
    fn from(site: &SnapshotSite) -> Self {
        Self {
            file: site.file.clone(),
            line: site.line,
            kind: site.kind.clone(),
            owner: site.owner.clone(),
        }
    }
}

struct WitnessState {
    label: String,
    rank: u8,
}

fn witness_state(card: &SnapshotCard) -> WitnessState {
    if let Some(strength) = imported_receipt_strength(&card.witness) {
        return WitnessState {
            rank: witness_rank(&strength),
            label: strength,
        };
    }
    if card
        .missing
        .iter()
        .any(|item| item.to_ascii_lowercase().contains("witness"))
        || card.witness.contains("No imported witness receipt")
        || card.witness.trim().is_empty()
    {
        return WitnessState {
            label: "missing".to_string(),
            rank: 0,
        };
    }
    WitnessState {
        label: "present".to_string(),
        rank: witness_rank("ran"),
    }
}

fn imported_receipt_strength(summary: &str) -> Option<String> {
    let marker = " receipt with `";
    let start = summary.find(marker)? + marker.len();
    let end = summary[start..].find("` strength")? + start;
    Some(summary[start..end].to_string())
}

fn witness_rank(value: &str) -> u8 {
    match value {
        "missing" => 0,
        "configured" => 1,
        "ran" => 2,
        "test_targeted" => 3,
        "site_reached" => 4,
        _ => 2,
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
        assert_eq!(value["cards"]["new"][0]["card_id"], "UR-new-c1");
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
        assert!(markdown.contains("| Status | Card | Reason | Before | After |"));
        assert!(markdown.contains("## Limitations"));
        assert!(markdown.contains("## Trust boundary"));
        assert!(markdown.contains("UR-new-c1"));
        Ok(())
    }

    #[test]
    fn outcome_card_state_preserves_saved_review_card_context() -> Result<(), String> {
        let before = snapshot_json(&[]);
        let after = snapshot_json(&[card_with_context(
            "UR-context-c1",
            "guard_missing",
            "high",
            &["alignment guard", "witness"],
            "src/lib.rs",
            42,
            "operation",
            "read_header",
            "raw_pointer_read",
            &["pointer_validity", "alignment"],
        )]);

        let report = compare_json(&before, &after)?;
        let state = report.cards.new[0]
            .after
            .as_ref()
            .ok_or("new card should include after state")?;

        let site = state
            .site
            .as_ref()
            .ok_or("saved ReviewCard site should be preserved")?;
        assert_eq!(site.file, "src/lib.rs");
        assert_eq!(site.line, 42);
        assert_eq!(site.kind, "operation");
        assert_eq!(site.owner, "read_header");
        assert_eq!(state.operation_family.as_deref(), Some("raw_pointer_read"));
        assert_eq!(state.hazards, ["pointer_validity", "alignment"]);

        let json = render_json(&report);
        let value: serde_json::Value =
            serde_json::from_str(&json).map_err(|err| format!("parse JSON failed: {err}"))?;
        assert_eq!(
            value["cards"]["new"][0]["after"]["operation_family"],
            "raw_pointer_read"
        );
        assert_eq!(
            value["cards"]["new"][0]["after"]["site"]["owner"],
            "read_header"
        );
        assert_eq!(value["cards"]["new"][0]["after"]["hazards"][1], "alignment");

        let markdown = render_markdown(&report);
        assert!(markdown.contains("site `src/lib.rs:42`"));
        assert!(markdown.contains("operation `raw_pointer_read`"));
        assert!(markdown.contains("hazards `pointer_validity, alignment`"));
        Ok(())
    }

    #[test]
    fn outcome_markdown_escapes_table_cells() -> Result<(), String> {
        let before = snapshot_json(&[]);
        let after = snapshot_json(&[card_with_witness(
            "UR-pipe|card-c1",
            "guard|missing",
            "high",
            &["guard"],
            "Imported miri receipt with `ran|odd` strength: focused fixture witness passed",
        )]);
        let report = compare_json(&before, &after)?;

        let markdown = render_markdown(&report);

        assert!(markdown.contains("`UR-pipe\\|card-c1`"));
        assert!(markdown.contains("`guard\\|missing`"));
        assert!(markdown.contains("witness `ran\\|odd`"));
        assert!(!markdown.contains("`UR-pipe|card-c1`"));
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

    #[allow(
        clippy::too_many_arguments,
        reason = "test fixture helper keeps ReviewCard context fields explicit"
    )]
    fn card_with_context(
        id: &str,
        class_name: &str,
        priority: &str,
        missing: &[&str],
        file: &str,
        line: usize,
        kind: &str,
        owner: &str,
        operation_family: &str,
        hazards: &[&str],
    ) -> String {
        let missing = missing
            .iter()
            .map(|item| format!(r#""{item}""#))
            .collect::<Vec<_>>()
            .join(", ");
        let hazards = hazards
            .iter()
            .map(|item| format!(r#""{item}""#))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            r#"{{
      "id": "{id}",
      "class": "{class_name}",
      "priority": "{priority}",
      "site": {{
        "file": "{file}",
        "line": {line},
        "kind": "{kind}",
        "owner": "{owner}"
      }},
      "operation_family": "{operation_family}",
      "hazards": [{hazards}],
      "witness": "",
      "missing": [{missing}]
    }}"#
        )
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
      "priority": "{priority}",
      "witness": "{witness}",
      "missing": [{missing}]
    }}"#
        )
    }
}
