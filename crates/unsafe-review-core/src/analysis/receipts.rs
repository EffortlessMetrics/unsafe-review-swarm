use crate::api::AnalyzeOutput;
use crate::domain::{CardId, ReviewCard, WitnessEvidence, WitnessReceipt};
use crate::util::path_display;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const AUDIT_TRUST_BOUNDARY: &str = "Static witness receipt audit only; this checks saved receipt metadata against current ReviewCards, does not execute witnesses, does not prove site reach, and does not make policy decisions.";

#[derive(Clone, Debug, Default)]
pub(crate) struct ReceiptIndex {
    by_card_id: BTreeMap<String, WitnessEvidence>,
}

impl ReceiptIndex {
    pub(crate) fn load(root: &Path) -> Result<Self, String> {
        let audit_date = current_utc_date()?;
        Self::load_with_date(root, &audit_date)
    }

    fn load_with_date(root: &Path, audit_date: &str) -> Result<Self, String> {
        let dir = root.join(".unsafe-review").join("receipts");
        if !dir.is_dir() {
            return Ok(Self::default());
        }
        let mut by_card_id = BTreeMap::new();
        let entries =
            fs::read_dir(&dir).map_err(|err| format!("read {} failed: {err}", dir.display()))?;
        for entry in entries {
            let entry = entry.map_err(|err| format!("read_dir entry failed: {err}"))?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let receipt = parse_receipt_file(&path)?;
            if receipt.expires_at.as_str() < audit_date {
                continue;
            }
            if by_card_id
                .insert(receipt.card_id.clone(), receipt.evidence)
                .is_some()
            {
                return Err(format!(
                    "{} imports duplicate receipt for card_id `{}`",
                    path.display(),
                    receipt.card_id
                ));
            }
        }
        Ok(Self { by_card_id })
    }

    pub(crate) fn evidence_for(&self, id: &CardId) -> Option<WitnessEvidence> {
        self.by_card_id.get(&id.0).cloned()
    }

    pub(crate) fn len(&self) -> usize {
        self.by_card_id.len()
    }
}

pub(crate) fn validate_receipts(root: &Path) -> Result<usize, String> {
    let dir = root.join(".unsafe-review").join("receipts");
    if !dir.is_dir() {
        return Ok(0);
    }
    let mut count = 0;
    let mut card_ids = BTreeSet::new();
    for path in receipt_files(&dir)? {
        let receipt = parse_receipt_file(&path)?;
        if !card_ids.insert(receipt.card_id.clone()) {
            return Err(format!(
                "{} imports duplicate receipt for card_id `{}`",
                path.display(),
                receipt.card_id
            ));
        }
        count += 1;
    }
    Ok(count)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ReceiptAuditReport {
    pub schema_version: String,
    pub tool: String,
    pub mode: String,
    pub policy: String,
    pub audit_date: String,
    pub trust_boundary: String,
    pub limitations: Vec<String>,
    pub summary: ReceiptAuditSummary,
    pub receipts: Vec<ReceiptAuditEntry>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ReceiptAuditSummary {
    pub receipts: usize,
    pub matched: usize,
    pub unmatched: usize,
    pub expired: usize,
    pub stale: usize,
    pub wrong_identity: usize,
    pub wrong_tool: usize,
    pub weaker_than_required: usize,
    pub duplicate: usize,
    pub invalid: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ReceiptAuditEntry {
    pub path: String,
    pub card_id: Option<String>,
    pub receipt_tool: Option<String>,
    pub strength: Option<String>,
    pub expires_at: Option<String>,
    pub statuses: Vec<String>,
    pub issues: Vec<String>,
    pub matched_card: Option<ReceiptAuditCard>,
    pub route_tools: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ReceiptAuditCard {
    pub id: String,
    #[serde(rename = "class")]
    pub class_name: &'static str,
    pub operation: String,
    pub operation_family: &'static str,
    pub missing_count: usize,
    pub next_action: String,
}

pub(crate) fn audit_receipts(output: &AnalyzeOutput) -> Result<ReceiptAuditReport, String> {
    let audit_date = current_utc_date()?;
    audit_receipts_with_date(output, &audit_date)
}

fn audit_receipts_with_date(
    output: &AnalyzeOutput,
    audit_date: &str,
) -> Result<ReceiptAuditReport, String> {
    let cards = output
        .cards
        .iter()
        .map(|card| (card.id.0.clone(), card))
        .collect::<BTreeMap<_, _>>();
    let records = audit_receipt_records(&output.root)?;
    let mut summary = ReceiptAuditSummary {
        receipts: records.len(),
        ..ReceiptAuditSummary::default()
    };
    let duplicate_card_ids = duplicate_receipt_card_ids(&records);
    let mut receipts = Vec::new();

    for record in records {
        let entry = audit_receipt_record(record, &cards, audit_date, &duplicate_card_ids);
        count_statuses(&mut summary, &entry.statuses);
        receipts.push(entry);
    }

    Ok(ReceiptAuditReport {
        schema_version: "0.1".to_string(),
        tool: "unsafe-review".to_string(),
        mode: "receipt-audit".to_string(),
        policy: "advisory".to_string(),
        audit_date: audit_date.to_string(),
        trust_boundary: AUDIT_TRUST_BOUNDARY.to_string(),
        limitations: vec![
            "audits saved witness receipt metadata only".to_string(),
            "does not execute Miri, cargo-careful, sanitizers, Loom, Shuttle, Kani, or Crux"
                .to_string(),
            "does not prove site reach, memory safety, UB-free status, or repo safety".to_string(),
            "matched receipts improve witness evidence only and do not erase missing contracts, guards, or reach evidence"
                .to_string(),
        ],
        summary,
        receipts,
    })
}

fn duplicate_receipt_card_ids(records: &[AuditReceiptRecord]) -> BTreeSet<String> {
    let mut counts = BTreeMap::<String, usize>::new();
    for record in records {
        if let Some(receipt) = &record.receipt {
            *counts.entry(receipt.card_id.clone()).or_insert(0) += 1;
        }
    }
    counts
        .into_iter()
        .filter_map(|(card_id, count)| (count > 1).then_some(card_id))
        .collect()
}

struct AuditReceiptRecord {
    path: PathBuf,
    receipt: Option<WitnessReceipt>,
    parse_error: Option<String>,
    validation_error: Option<String>,
}

struct ParsedReceipt {
    card_id: String,
    evidence: WitnessEvidence,
    expires_at: String,
}

fn parse_receipt_file(path: &Path) -> Result<ParsedReceipt, String> {
    let text =
        fs::read_to_string(path).map_err(|err| format!("read {} failed: {err}", path.display()))?;
    let receipt: WitnessReceipt = serde_json::from_str(&text)
        .map_err(|err| format!("{} is not valid receipt JSON: {err}", path.display()))?;
    receipt
        .validate()
        .map_err(|err| format!("{} {err}", path.display()))?;
    Ok(ParsedReceipt {
        card_id: receipt.card_id.clone(),
        evidence: WitnessEvidence::present(receipt.evidence_summary()),
        expires_at: receipt.expires_at.clone().unwrap_or_default(),
    })
}

fn audit_receipt_records(root: &Path) -> Result<Vec<AuditReceiptRecord>, String> {
    let dir = root.join(".unsafe-review").join("receipts");
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut records = Vec::new();
    for path in receipt_files(&dir)? {
        let text = fs::read_to_string(&path)
            .map_err(|err| format!("read {} failed: {err}", path.display()))?;
        match serde_json::from_str::<WitnessReceipt>(&text) {
            Ok(receipt) => {
                let validation_error = receipt.validate().err();
                records.push(AuditReceiptRecord {
                    path,
                    receipt: Some(receipt),
                    parse_error: None,
                    validation_error,
                });
            }
            Err(err) => records.push(AuditReceiptRecord {
                path,
                receipt: None,
                parse_error: Some(format!("not valid receipt JSON: {err}")),
                validation_error: None,
            }),
        }
    }
    Ok(records)
}

fn receipt_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let entries =
        fs::read_dir(dir).map_err(|err| format!("read {} failed: {err}", dir.display()))?;
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|err| format!("read_dir entry failed: {err}"))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn audit_receipt_record(
    record: AuditReceiptRecord,
    cards: &BTreeMap<String, &ReviewCard>,
    audit_date: &str,
    duplicate_card_ids: &BTreeSet<String>,
) -> ReceiptAuditEntry {
    let mut statuses = BTreeSet::new();
    let mut issues = Vec::new();
    let path = path_display(&record.path);

    if let Some(err) = record.parse_error {
        statuses.insert("invalid".to_string());
        issues.push(err);
        return ReceiptAuditEntry {
            path,
            card_id: None,
            receipt_tool: None,
            strength: None,
            expires_at: None,
            statuses: statuses.into_iter().collect(),
            issues,
            matched_card: None,
            route_tools: Vec::new(),
        };
    }

    let Some(receipt) = record.receipt else {
        statuses.insert("invalid".to_string());
        issues.push("receipt could not be parsed".to_string());
        return ReceiptAuditEntry {
            path,
            card_id: None,
            receipt_tool: None,
            strength: None,
            expires_at: None,
            statuses: statuses.into_iter().collect(),
            issues,
            matched_card: None,
            route_tools: Vec::new(),
        };
    };

    if duplicate_card_ids.contains(&receipt.card_id) {
        statuses.insert("duplicate".to_string());
        issues.push("more than one receipt file references this card_id".to_string());
    }

    if let Some(err) = record.validation_error {
        statuses.insert("invalid".to_string());
        issues.push(err.clone());
        if err.contains("exact counted") {
            statuses.insert("wrong_identity".to_string());
        }
        if err.contains("unknown receipt tool") {
            statuses.insert("wrong_tool".to_string());
        }
    }

    let matched = cards.get(&receipt.card_id).copied();
    let route_tools = matched
        .map(route_tools)
        .unwrap_or_default()
        .into_iter()
        .collect::<Vec<_>>();

    if let Some(card) = matched {
        statuses.insert("matched".to_string());
        if !route_tools.iter().any(|tool| tool == &receipt.tool) {
            statuses.insert("wrong_tool".to_string());
            issues.push(format!(
                "receipt tool `{}` is not one of this card's routed witness tools: {}",
                receipt.tool,
                route_tools.join(", ")
            ));
        }
        if is_weaker_than_required(&receipt, card) {
            statuses.insert("weaker_than_required".to_string());
            issues.push(format!(
                "receipt strength `{}` is weaker than the minimum `ran` strength for a required witness route",
                receipt.strength
            ));
        }
    } else if looks_counted(&receipt.card_id) {
        statuses.insert("unmatched".to_string());
        statuses.insert("stale".to_string());
        issues.push("receipt card_id is not present in the current ReviewCard set".to_string());
    } else {
        statuses.insert("wrong_identity".to_string());
        issues.push("receipt card_id is not an exact counted ReviewCard identity".to_string());
    }

    if let Some(expires_at) = receipt.expires_at.as_deref()
        && expires_at < audit_date
    {
        statuses.insert("expired".to_string());
        issues.push(format!(
            "receipt expired on {expires_at}; audit date is {audit_date}"
        ));
    }

    ReceiptAuditEntry {
        path,
        card_id: Some(receipt.card_id),
        receipt_tool: Some(receipt.tool),
        strength: Some(receipt.strength),
        expires_at: receipt.expires_at,
        statuses: statuses.into_iter().collect(),
        issues,
        matched_card: matched.map(|card| ReceiptAuditCard {
            id: card.id.0.clone(),
            class_name: card.class.as_str(),
            operation: card.operation.expression.clone(),
            operation_family: card.operation.family.as_str(),
            missing_count: card.missing.len(),
            next_action: card.next_action.summary.clone(),
        }),
        route_tools,
    }
}

fn route_tools(card: &ReviewCard) -> BTreeSet<String> {
    card.routes
        .iter()
        .map(|route| route.kind.as_str().to_string())
        .collect()
}

fn is_weaker_than_required(receipt: &WitnessReceipt, card: &ReviewCard) -> bool {
    if !card.routes.iter().any(|route| route.required)
        && !card.missing.iter().any(|missing| missing.kind == "witness")
    {
        return false;
    }
    strength_rank(&receipt.strength).is_some_and(|rank| rank < strength_rank("ran").unwrap_or(1))
}

fn strength_rank(value: &str) -> Option<u8> {
    match value {
        "configured" => Some(0),
        "ran" => Some(1),
        "test_targeted" => Some(2),
        "site_reached" => Some(3),
        _ => None,
    }
}

fn count_statuses(summary: &mut ReceiptAuditSummary, statuses: &[String]) {
    for status in statuses {
        match status.as_str() {
            "matched" => summary.matched += 1,
            "unmatched" => summary.unmatched += 1,
            "expired" => summary.expired += 1,
            "stale" => summary.stale += 1,
            "wrong_identity" => summary.wrong_identity += 1,
            "wrong_tool" => summary.wrong_tool += 1,
            "weaker_than_required" => summary.weaker_than_required += 1,
            "duplicate" => summary.duplicate += 1,
            "invalid" => summary.invalid += 1,
            _ => {}
        }
    }
}

fn current_utc_date() -> Result<String, String> {
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("system clock before UNIX_EPOCH: {err}"))?
        .as_secs()
        / 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    Ok(format!("{year:04}-{month:02}-{day:02}"))
}

fn civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + i64::from(month <= 2);
    (year as i32, month as u32, day as u32)
}

fn looks_counted(value: &str) -> bool {
    let Some((prefix, count)) = value.rsplit_once("-c") else {
        return false;
    };
    value.starts_with("UR-")
        && !prefix.is_empty()
        && !count.is_empty()
        && count.bytes().all(|byte| byte.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::pipeline;
    use crate::api::{AnalysisMode, AnalyzeInput, DiffSource, PolicyMode, Scope};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn receipt_index_loads_exact_card_receipts() -> Result<(), String> {
        let root = unique_temp_dir("unsafe-review-receipt-index")?;
        let receipts = root.join(".unsafe-review").join("receipts");
        fs::create_dir_all(&receipts).map_err(|err| format!("create receipt dir failed: {err}"))?;
        let card_id =
            "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1";
        fs::write(
            receipts.join("miri.json"),
            format!(
                r#"{{
  "schema_version": "0.1",
  "card_id": "{card_id}",
  "tool": "miri",
  "strength": "ran",
  "author": "core/fixtures",
  "recorded_at": "2025-12-18T00:00:00Z",
  "expires_at": "2026-08-18",
  "summary": "focused witness passed",
  "command": "cargo +nightly miri test read_header",
  "limitations": ["fixture only"]
}}"#
            ),
        )
        .map_err(|err| format!("write receipt failed: {err}"))?;

        let index = ReceiptIndex::load(&root)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp root failed: {err}"))?;
        let evidence = index
            .evidence_for(&CardId(card_id.to_string()))
            .ok_or_else(|| "receipt evidence missing".to_string())?;
        assert!(evidence.present);
        assert!(evidence.summary.contains("miri"));
        assert!(evidence.summary.contains("ran"));
        assert!(evidence.summary.contains("core/fixtures"));
        assert!(evidence.summary.contains("2026-08-18"));
        assert!(evidence.summary.contains("fixture only"));
        Ok(())
    }

    #[test]
    fn receipt_index_skips_expired_receipts_for_witness_evidence() -> Result<(), String> {
        let root = unique_temp_dir("unsafe-review-expired-receipt-index")?;
        let receipts = root.join(".unsafe-review").join("receipts");
        fs::create_dir_all(&receipts).map_err(|err| format!("create receipt dir failed: {err}"))?;
        let card_id =
            "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1";
        write_receipt(
            &receipts,
            "expired.json",
            card_id,
            "miri",
            "ran",
            "2026-05-17",
        )?;

        let index = ReceiptIndex::load_with_date(&root, "2026-05-18")?;
        let validated = validate_receipts(&root)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp root failed: {err}"))?;
        assert_eq!(validated, 1);
        assert_eq!(index.len(), 0);
        assert!(index.evidence_for(&CardId(card_id.to_string())).is_none());
        Ok(())
    }

    #[test]
    fn receipt_index_rejects_unknown_strength() -> Result<(), String> {
        let root = unique_temp_dir("unsafe-review-bad-receipt")?;
        let receipts = root.join(".unsafe-review").join("receipts");
        fs::create_dir_all(&receipts).map_err(|err| format!("create receipt dir failed: {err}"))?;
        fs::write(
            receipts.join("bad.json"),
            r#"{
  "schema_version": "0.1",
  "card_id": "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1",
  "tool": "miri",
  "strength": "proved"
}"#,
        )
        .map_err(|err| format!("write receipt failed: {err}"))?;

        let result = ReceiptIndex::load(&root);

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp root failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("unknown receipt strength")
        );
        Ok(())
    }

    #[test]
    fn receipt_index_rejects_unknown_tool() -> Result<(), String> {
        let root = unique_temp_dir("unsafe-review-bad-tool-receipt")?;
        let receipts = root.join(".unsafe-review").join("receipts");
        fs::create_dir_all(&receipts).map_err(|err| format!("create receipt dir failed: {err}"))?;
        fs::write(
            receipts.join("bad.json"),
            r#"{
  "schema_version": "0.1",
  "card_id": "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1",
  "tool": "proof-bot",
  "strength": "ran",
  "author": "core/fixtures",
  "recorded_at": "2025-12-18T00:00:00Z",
  "expires_at": "2026-08-18"
}"#,
        )
        .map_err(|err| format!("write receipt failed: {err}"))?;

        let result = ReceiptIndex::load(&root);

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp root failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("unknown receipt tool")
        );
        Ok(())
    }

    #[test]
    fn receipt_index_rejects_uncounted_card_identity() -> Result<(), String> {
        let root = unique_temp_dir("unsafe-review-uncounted-receipt")?;
        let receipts = root.join(".unsafe-review").join("receipts");
        fs::create_dir_all(&receipts).map_err(|err| format!("create receipt dir failed: {err}"))?;
        fs::write(
            receipts.join("bad.json"),
            r#"{
  "schema_version": "0.1",
  "card_id": "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment",
  "tool": "miri",
  "strength": "ran"
}"#,
        )
        .map_err(|err| format!("write receipt failed: {err}"))?;

        let result = ReceiptIndex::load(&root);

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp root failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("exact counted UR-* identity")
        );
        Ok(())
    }

    #[test]
    fn receipt_index_rejects_missing_author() -> Result<(), String> {
        let root = unique_temp_dir("unsafe-review-missing-author-receipt")?;
        let receipts = root.join(".unsafe-review").join("receipts");
        fs::create_dir_all(&receipts).map_err(|err| format!("create receipt dir failed: {err}"))?;
        fs::write(
            receipts.join("bad.json"),
            r#"{
  "schema_version": "0.1",
  "card_id": "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1",
  "tool": "miri",
  "strength": "ran",
  "recorded_at": "2026-05-18T00:00:00Z",
  "expires_at": "2026-08-18"
}"#,
        )
        .map_err(|err| format!("write receipt failed: {err}"))?;

        let result = ReceiptIndex::load(&root);

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp root failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("`author` is required")
        );
        Ok(())
    }

    #[test]
    fn receipt_index_rejects_invalid_recorded_at_timestamp() -> Result<(), String> {
        let root = unique_temp_dir("unsafe-review-invalid-recorded-at-receipt")?;
        let receipts = root.join(".unsafe-review").join("receipts");
        fs::create_dir_all(&receipts).map_err(|err| format!("create receipt dir failed: {err}"))?;
        fs::write(
            receipts.join("bad.json"),
            r#"{
  "schema_version": "0.1",
  "card_id": "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1",
  "tool": "miri",
  "strength": "ran",
  "author": "core/fixtures",
  "recorded_at": "2026-05-18",
  "expires_at": "2026-08-18"
}"#,
        )
        .map_err(|err| format!("write receipt failed: {err}"))?;

        let result = ReceiptIndex::load(&root);

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp root failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("UTC timestamp format")
        );
        Ok(())
    }

    #[test]
    fn receipt_index_rejects_expiry_before_recorded_date() -> Result<(), String> {
        let root = unique_temp_dir("unsafe-review-expired-receipt")?;
        let receipts = root.join(".unsafe-review").join("receipts");
        fs::create_dir_all(&receipts).map_err(|err| format!("create receipt dir failed: {err}"))?;
        fs::write(
            receipts.join("bad.json"),
            r#"{
  "schema_version": "0.1",
  "card_id": "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1",
  "tool": "miri",
  "strength": "ran",
  "author": "core/fixtures",
  "recorded_at": "2026-05-18T00:00:00Z",
  "expires_at": "2026-05-17"
}"#,
        )
        .map_err(|err| format!("write receipt failed: {err}"))?;

        let result = ReceiptIndex::load(&root);

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp root failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("on or after the `recorded_at` date")
        );
        Ok(())
    }

    #[test]
    fn receipt_audit_reports_match_and_problem_statuses() -> Result<(), String> {
        let root = copy_fixture_to_temp(
            "raw_pointer_alignment",
            "unsafe-review-receipt-audit-problems",
        )?;
        let output = analyze_fixture_root(&root)?;
        let card_id = output
            .cards
            .first()
            .ok_or_else(|| "fixture produced no card".to_string())?
            .id
            .0
            .clone();
        let receipt_dir = root.join(".unsafe-review").join("receipts");
        fs::create_dir_all(&receipt_dir)
            .map_err(|err| format!("create receipt dir failed: {err}"))?;
        write_receipt(
            &receipt_dir,
            "matched.json",
            &card_id,
            "miri",
            "ran",
            "2026-08-18",
        )?;
        write_receipt(
            &receipt_dir,
            "wrong-tool.json",
            &card_id,
            "loom",
            "ran",
            "2026-08-18",
        )?;
        write_receipt(
            &receipt_dir,
            "weak.json",
            &card_id,
            "miri",
            "configured",
            "2026-08-18",
        )?;
        write_receipt(
            &receipt_dir,
            "expired.json",
            &card_id,
            "miri",
            "ran",
            "2026-05-17",
        )?;
        write_receipt(
            &receipt_dir,
            "stale.json",
            "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1",
            "miri",
            "ran",
            "2026-08-18",
        )?;
        write_receipt(
            &receipt_dir,
            "wrong-identity.json",
            "not-counted",
            "miri",
            "ran",
            "2026-08-18",
        )?;
        write_receipt(
            &receipt_dir,
            "duplicate.json",
            &card_id,
            "miri",
            "ran",
            "2026-08-18",
        )?;

        let report = audit_receipts_with_date(&output, "2026-05-18")?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp root failed: {err}"))?;
        assert_eq!(report.summary.receipts, 7);
        assert_eq!(report.summary.matched, 5);
        assert_eq!(report.summary.unmatched, 1);
        assert_eq!(report.summary.stale, 1);
        assert_eq!(report.summary.expired, 1);
        assert_eq!(report.summary.wrong_identity, 1);
        assert_eq!(report.summary.wrong_tool, 1);
        assert_eq!(report.summary.weaker_than_required, 1);
        assert_eq!(report.summary.duplicate, 5);
        assert_eq!(report.summary.invalid, 1);
        assert_eq!(report.limitations.len(), 4);
        assert!(
            report
                .limitations
                .iter()
                .any(|limitation| limitation.contains("does not execute Miri"))
        );
        assert!(
            report
                .limitations
                .iter()
                .any(|limitation| limitation.contains("do not erase missing contracts"))
        );
        assert!(report.trust_boundary.contains("does not execute witnesses"));
        let matched_entry = report
            .receipts
            .iter()
            .find(|entry| entry.path.ends_with("matched.json"))
            .ok_or_else(|| "matched receipt entry missing".to_string())?;
        let matched_card = matched_entry
            .matched_card
            .as_ref()
            .ok_or_else(|| "matched receipt should include card context".to_string())?;
        assert_eq!(
            matched_card.operation,
            "unsafe { ptr.cast::<Header>().read() }"
        );
        assert_eq!(matched_card.operation_family, "raw_pointer_read");
        assert_eq!(matched_card.missing_count, 2);
        assert!(matched_card.next_action.contains("Add or expose"));
        let duplicate_entries = report
            .receipts
            .iter()
            .filter(|entry| entry.statuses.iter().any(|status| status == "duplicate"))
            .count();
        assert_eq!(duplicate_entries, 5);
        Ok(())
    }

    #[test]
    fn receipt_audit_does_not_require_valid_receipts_for_card_scan() -> Result<(), String> {
        let root = copy_fixture_to_temp(
            "raw_pointer_alignment",
            "unsafe-review-receipt-audit-invalid",
        )?;
        let receipt_dir = root.join(".unsafe-review").join("receipts");
        fs::create_dir_all(&receipt_dir)
            .map_err(|err| format!("create receipt dir failed: {err}"))?;
        fs::write(
            receipt_dir.join("invalid.json"),
            r#"{"schema_version":"0.1","card_id":"not-counted","tool":"proof-bot","strength":"ran"}"#,
        )
        .map_err(|err| format!("write invalid receipt failed: {err}"))?;

        let output = pipeline::analyze_without_receipts(analyze_input(&root))?;
        let report = audit_receipts_with_date(&output, "2026-05-18")?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp root failed: {err}"))?;
        assert_eq!(report.summary.invalid, 1);
        assert_eq!(report.summary.wrong_identity, 1);
        assert_eq!(report.summary.wrong_tool, 1);
        assert_eq!(report.summary.receipts, 1);
        Ok(())
    }

    fn unique_temp_dir(prefix: &str) -> Result<PathBuf, String> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system clock before UNIX_EPOCH: {err}"))?
            .as_nanos();
        Ok(std::env::temp_dir().join(format!("{prefix}-{nanos}")))
    }

    fn copy_fixture_to_temp(fixture: &str, prefix: &str) -> Result<PathBuf, String> {
        let src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(fixture);
        let dst = unique_temp_dir(prefix)?;
        copy_dir(&src, &dst)?;
        Ok(dst)
    }

    fn copy_dir(src: &Path, dst: &Path) -> Result<(), String> {
        fs::create_dir_all(dst).map_err(|err| format!("create {} failed: {err}", dst.display()))?;
        for entry in
            fs::read_dir(src).map_err(|err| format!("read {} failed: {err}", src.display()))?
        {
            let entry = entry.map_err(|err| format!("read_dir entry failed: {err}"))?;
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());
            if src_path.is_dir() {
                copy_dir(&src_path, &dst_path)?;
            } else {
                fs::copy(&src_path, &dst_path).map_err(|err| {
                    format!(
                        "copy {} to {} failed: {err}",
                        src_path.display(),
                        dst_path.display()
                    )
                })?;
            }
        }
        Ok(())
    }

    fn analyze_fixture_root(root: &Path) -> Result<crate::api::AnalyzeOutput, String> {
        pipeline::analyze_without_receipts(analyze_input(root))
    }

    fn analyze_input(root: &Path) -> AnalyzeInput {
        AnalyzeInput {
            root: root.to_path_buf(),
            scope: Scope::Diff,
            diff: DiffSource::File(root.join("change.diff")),
            mode: AnalysisMode::Draft,
            policy: PolicyMode::Advisory,
            include_unchanged_tests: true,
            max_cards: None,
        }
    }

    fn write_receipt(
        dir: &Path,
        name: &str,
        card_id: &str,
        tool: &str,
        strength: &str,
        expires_at: &str,
    ) -> Result<(), String> {
        fs::write(
            dir.join(name),
            format!(
                r#"{{
  "schema_version": "0.1",
  "card_id": "{card_id}",
  "tool": "{tool}",
  "strength": "{strength}",
  "author": "core/fixtures",
  "recorded_at": "2025-12-18T00:00:00Z",
  "expires_at": "{expires_at}",
  "summary": "focused witness",
  "command": "cargo test",
  "limitations": ["fixture only"]
}}"#
            ),
        )
        .map_err(|err| format!("write receipt {name} failed: {err}"))
    }
}
