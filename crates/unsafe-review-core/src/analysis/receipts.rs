use crate::api::AnalyzeOutput;
use crate::candidate::{ManualCandidate, ManualCandidateProofMode, load_manual_candidates};
use crate::domain::{
    CardId, ReachEvidence, ReceiptCardIdKind, ReviewCard, WitnessEvidence, WitnessReceipt,
    WitnessRoute,
};
use crate::util::path_display;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const AUDIT_TRUST_BOUNDARY: &str = "Static receipt audit only; this checks saved receipt metadata against current ReviewCards and manual candidates, does not execute witnesses or external tests, does not independently prove site reach, and does not make policy decisions.";

#[derive(Clone, Debug, Default)]
pub(crate) struct ReceiptIndex {
    by_card_id: BTreeMap<String, ImportedReceipt>,
    reach_by_card_id: BTreeMap<String, ImportedReachReceipt>,
    metadata_by_card_id: BTreeMap<String, Vec<ReceiptMetadata>>,
    audit_date: Option<String>,
}

#[derive(Clone, Debug)]
struct ImportedReceipt {
    tool: String,
    evidence: WitnessEvidence,
}

#[derive(Clone, Debug)]
struct ImportedReachReceipt {
    evidence: ReachEvidence,
}

#[derive(Clone, Debug)]
struct ReceiptMetadata {
    tool: String,
    strength: String,
    expires_at: String,
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
        let mut reach_by_card_id = BTreeMap::new();
        let mut metadata_by_card_id = BTreeMap::new();
        let entries =
            fs::read_dir(&dir).map_err(|err| format!("read {} failed: {err}", dir.display()))?;
        for entry in entries {
            let entry = entry.map_err(|err| format!("read_dir entry failed: {err}"))?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let receipt = parse_receipt_file(&path)?;
            metadata_by_card_id
                .entry(receipt.card_id.clone())
                .or_insert_with(Vec::new)
                .push(ReceiptMetadata {
                    tool: receipt.tool.clone(),
                    strength: receipt.strength.clone(),
                    expires_at: receipt.expires_at.clone(),
                });
            if receipt.expires_at.as_str() < audit_date {
                continue;
            }
            if imports_witness_evidence(&receipt.tool, &receipt.strength)
                && by_card_id
                    .insert(
                        receipt.card_id.clone(),
                        ImportedReceipt {
                            tool: receipt.tool.clone(),
                            evidence: receipt.evidence.clone(),
                        },
                    )
                    .is_some()
            {
                return Err(format!(
                    "{} imports duplicate witness receipt for card_id `{}`",
                    path.display(),
                    receipt.card_id
                ));
            }
            if imports_reach_evidence(&receipt.tool, &receipt.strength)
                && reach_by_card_id
                    .insert(
                        receipt.card_id.clone(),
                        ImportedReachReceipt {
                            evidence: ReachEvidence {
                                state: "external_reached".to_string(),
                                summary: format!(
                                    "External integration reach receipt imported: {}",
                                    receipt.evidence.summary
                                ),
                            },
                        },
                    )
                    .is_some()
            {
                return Err(format!(
                    "{} imports duplicate reach receipt for card_id `{}`",
                    path.display(),
                    receipt.card_id
                ));
            }
        }
        Ok(Self {
            by_card_id,
            reach_by_card_id,
            metadata_by_card_id,
            audit_date: Some(audit_date.to_string()),
        })
    }

    pub(crate) fn reach_evidence_for(
        &self,
        id: &CardId,
        static_reach: ReachEvidence,
    ) -> ReachEvidence {
        if static_reach.state != "unreached" {
            return static_reach;
        }
        self.reach_by_card_id
            .get(&id.0)
            .map(|receipt| receipt.evidence.clone())
            .unwrap_or(static_reach)
    }

    pub(crate) fn witness_evidence_for(
        &self,
        id: &CardId,
        routes: &[WitnessRoute],
    ) -> WitnessEvidence {
        let route_tools = routes
            .iter()
            .map(|route| route.kind.as_str())
            .collect::<Vec<_>>();
        if let Some(receipt) = self.by_card_id.get(&id.0) {
            if route_tools.iter().any(|tool| *tool == receipt.tool) {
                return receipt.evidence.clone();
            }
            return WitnessEvidence::missing_with(format!(
                "Saved `{}` receipt for this card does not match routed witness tools: {}",
                receipt.tool,
                route_tools.join(", ")
            ));
        }

        let Some(metadata_entries) = self.metadata_by_card_id.get(&id.0) else {
            return WitnessEvidence::missing();
        };
        let Some(metadata) = metadata_entries
            .iter()
            .find(|metadata| !imports_reach_evidence(&metadata.tool, &metadata.strength))
            .or_else(|| metadata_entries.first())
        else {
            return WitnessEvidence::missing();
        };
        if self.audit_date.as_deref().is_some_and(|audit_date| {
            metadata.expires_at.as_str() < audit_date
                && !metadata_entries.iter().any(|item| {
                    !imports_reach_evidence(&item.tool, &item.strength)
                        && item.expires_at.as_str() >= audit_date
                })
        }) {
            return WitnessEvidence::missing_with(format!(
                "Saved `{}` receipt for this card is expired; attach a current matching witness receipt",
                metadata.tool
            ));
        }
        if !imports_witness_evidence(&metadata.tool, &metadata.strength) {
            return WitnessEvidence::missing_with(format!(
                "Saved `{}` receipt for this card has `{}` strength; attach a matching saved witness or review receipt",
                metadata.tool, metadata.strength
            ));
        }
        WitnessEvidence::missing()
    }
}

pub(crate) fn validate_receipts(root: &Path) -> Result<usize, String> {
    let dir = root.join(".unsafe-review").join("receipts");
    if !dir.is_dir() {
        return Ok(0);
    }
    let mut count = 0;
    let mut import_keys = BTreeSet::new();
    for path in receipt_files(&dir)? {
        let receipt = parse_receipt_file(&path)?;
        if let Some(key) = receipt_import_key(&receipt)
            && !import_keys.insert(key)
        {
            return Err(format!(
                "{} imports duplicate {} receipt for card_id `{}`",
                path.display(),
                receipt_import_kind(&receipt).unwrap_or("evidence"),
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
    pub command_hash_mismatch: usize,
    pub duplicate: usize,
    pub invalid: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ReceiptAuditEntry {
    pub path: String,
    pub card_id: Option<String>,
    pub receipt_tool: Option<String>,
    pub strength: Option<String>,
    pub summary: Option<String>,
    pub author: Option<String>,
    pub recorded_at: Option<String>,
    pub expires_at: Option<String>,
    pub command_hash: Option<String>,
    pub limitations: Vec<String>,
    pub statuses: Vec<String>,
    pub issues: Vec<String>,
    pub matched_card: Option<ReceiptAuditCard>,
    pub matched_manual_candidate: Option<ReceiptAuditManualCandidate>,
    pub route_tools: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ReceiptAuditCard {
    pub id: String,
    #[serde(rename = "class")]
    pub class_name: String,
    pub operation: String,
    pub operation_family: String,
    pub missing_count: usize,
    pub next_action: String,
    pub source: String,
    pub manual_candidate: bool,
    pub analyzer_discovered: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ReceiptAuditManualCandidate {
    pub id: String,
    pub title: String,
    pub location: String,
    pub operation: String,
    pub operation_family: String,
    pub safe_caller: String,
    pub invariant: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proof_mode: Option<ManualCandidateProofMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix_boundary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pr_aperture: Option<String>,
    pub evidence: Vec<ReceiptAuditManualCandidateEvidence>,
    pub fix_options: Vec<String>,
    pub test_targets: Vec<String>,
    pub do_not_touch: Vec<String>,
    pub next_action: String,
    pub trust_boundary: String,
    pub source: String,
    pub manual_candidate: bool,
    pub analyzer_discovered: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ReceiptAuditManualCandidateEvidence {
    pub kind: String,
    pub path: Option<String>,
    pub summary: Option<String>,
    pub command: Option<String>,
    pub limitation: Option<String>,
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
    let manual_candidates = load_manual_candidates(&output.root)?.into_iter().try_fold(
        BTreeMap::new(),
        |mut candidates, candidate| {
            if candidates.insert(candidate.id.clone(), candidate).is_some() {
                return Err("duplicate manual candidate id".to_string());
            }
            Ok(candidates)
        },
    )?;
    let records = audit_receipt_records(&output.root)?;
    let mut summary = ReceiptAuditSummary {
        receipts: records.len(),
        ..ReceiptAuditSummary::default()
    };
    let duplicate_import_keys = duplicate_receipt_import_keys(&records);
    let mut receipts = Vec::new();

    for record in records {
        let entry = audit_receipt_record(
            record,
            &cards,
            &manual_candidates,
            audit_date,
            &duplicate_import_keys,
        );
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
            "audits saved receipt metadata only".to_string(),
            "does not execute Miri, cargo-careful, sanitizers, Loom, Shuttle, Kani, Crux, or external integration tests"
                .to_string(),
            "does not independently prove site reach, memory safety, UB-free status, or repo safety"
                .to_string(),
            "matched witness receipts improve witness evidence only and do not erase missing contracts, guards, or reach evidence"
                .to_string(),
            "matched external integration reach receipts improve reach evidence only and do not erase missing contracts, guards, or witness evidence"
                .to_string(),
            "manual candidate receipts attach external evidence to that manual candidate only and do not make it analyzer-discovered"
                .to_string(),
        ],
        summary,
        receipts,
    })
}

fn duplicate_receipt_import_keys(records: &[AuditReceiptRecord]) -> BTreeSet<(String, String)> {
    let mut counts = BTreeMap::<(String, String), usize>::new();
    for record in records {
        if let Some(receipt) = &record.receipt
            && let Some(key) = receipt_import_key_from_witness_receipt(receipt)
        {
            *counts.entry(key).or_insert(0) += 1;
        }
    }
    counts
        .into_iter()
        .filter_map(|(key, count)| (count > 1).then_some(key))
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
    strength: String,
    tool: String,
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
        strength: receipt.strength,
        tool: receipt.tool,
    })
}

fn imports_witness_evidence(tool: &str, strength: &str) -> bool {
    (tool != "external-integration-test"
        && matches!(strength, "ran" | "test_targeted" | "site_reached"))
        || (tool == "human-deep-review" && strength == "reviewed")
}

fn imports_reach_evidence(tool: &str, strength: &str) -> bool {
    tool == "external-integration-test" && strength == "site_reached"
}

fn receipt_import_key(receipt: &ParsedReceipt) -> Option<(String, String)> {
    receipt_import_kind(receipt).map(|kind| (receipt.card_id.clone(), kind.to_string()))
}

fn receipt_import_kind(receipt: &ParsedReceipt) -> Option<&'static str> {
    if imports_witness_evidence(&receipt.tool, &receipt.strength) {
        Some("witness")
    } else if imports_reach_evidence(&receipt.tool, &receipt.strength) {
        Some("reach")
    } else {
        None
    }
}

fn receipt_import_key_from_witness_receipt(receipt: &WitnessReceipt) -> Option<(String, String)> {
    receipt_import_kind_from_witness_receipt(receipt)
        .map(|kind| (receipt.card_id.clone(), kind.to_string()))
}

fn receipt_import_kind_from_witness_receipt(receipt: &WitnessReceipt) -> Option<&'static str> {
    if imports_witness_evidence(&receipt.tool, &receipt.strength) {
        Some("witness")
    } else if imports_reach_evidence(&receipt.tool, &receipt.strength) {
        Some("reach")
    } else {
        None
    }
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
    manual_candidates: &BTreeMap<String, ManualCandidate>,
    audit_date: &str,
    duplicate_import_keys: &BTreeSet<(String, String)>,
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
            summary: None,
            author: None,
            recorded_at: None,
            expires_at: None,
            command_hash: None,
            limitations: Vec::new(),
            statuses: statuses.into_iter().collect(),
            issues,
            matched_card: None,
            matched_manual_candidate: None,
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
            summary: None,
            author: None,
            recorded_at: None,
            expires_at: None,
            command_hash: None,
            limitations: Vec::new(),
            statuses: statuses.into_iter().collect(),
            issues,
            matched_card: None,
            matched_manual_candidate: None,
            route_tools: Vec::new(),
        };
    };

    if receipt_import_key_from_witness_receipt(&receipt)
        .is_some_and(|key| duplicate_import_keys.contains(&key))
    {
        statuses.insert("duplicate".to_string());
        issues.push(format!(
            "more than one receipt file imports {} evidence for this card_id",
            receipt_import_kind_from_witness_receipt(&receipt).unwrap_or("the same")
        ));
    }

    if let Some(err) = record.validation_error {
        statuses.insert("invalid".to_string());
        issues.push(err.clone());
        if err.contains("card_id") {
            statuses.insert("wrong_identity".to_string());
        }
        if err.contains("unknown receipt tool") {
            statuses.insert("wrong_tool".to_string());
        }
        if err.contains("command_hash") {
            statuses.insert("command_hash_mismatch".to_string());
        }
    }

    let matched_review_card = cards.get(&receipt.card_id).copied();
    let matched_manual_candidate = matched_review_card
        .is_none()
        .then(|| manual_candidates.get(&receipt.card_id))
        .flatten();
    let route_tools = matched_review_card
        .map(route_tools)
        .unwrap_or_default()
        .into_iter()
        .collect::<Vec<_>>();

    if let Some(card) = matched_review_card {
        statuses.insert("matched".to_string());
        if !imports_reach_evidence(&receipt.tool, &receipt.strength)
            && !route_tools.iter().any(|tool| tool == &receipt.tool)
        {
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
                "receipt strength `{}` is weaker than the minimum importable strength for a required witness route",
                receipt.strength
            ));
        }
    } else if let Some(_candidate) = matched_manual_candidate {
        statuses.insert("manual_candidate".to_string());
        statuses.insert("matched".to_string());
    } else if matches!(
        WitnessReceipt::card_id_kind(&receipt.card_id),
        Some(ReceiptCardIdKind::AnalyzerReviewCard)
    ) {
        statuses.insert("unmatched".to_string());
        statuses.insert("stale".to_string());
        issues.push("receipt card_id is not present in the current ReviewCard set".to_string());
    } else if matches!(
        WitnessReceipt::card_id_kind(&receipt.card_id),
        Some(ReceiptCardIdKind::ManualCandidate)
    ) {
        statuses.insert("unmatched".to_string());
        statuses.insert("stale".to_string());
        issues
            .push("receipt card_id is not present in the current manual candidate set".to_string());
    } else {
        statuses.insert("wrong_identity".to_string());
        issues.push("receipt card_id is not an exact counted ReviewCard identity or a path-safe manual candidate id".to_string());
    }

    if let Some(expires_at) = receipt.expires_at.as_deref()
        && expires_at < audit_date
    {
        statuses.insert("expired".to_string());
        issues.push(format!(
            "receipt expired on {expires_at}; audit date is {audit_date}"
        ));
    }

    if receipt_imports_current_witness_evidence(&receipt, &statuses, &route_tools) {
        statuses.insert("imports_witness_evidence".to_string());
    }
    if receipt_imports_current_reach_evidence(&receipt, &statuses) {
        statuses.insert("imports_reach_evidence".to_string());
    }

    ReceiptAuditEntry {
        path,
        card_id: Some(receipt.card_id),
        receipt_tool: Some(receipt.tool),
        strength: Some(receipt.strength),
        summary: receipt.summary,
        author: receipt.author,
        recorded_at: receipt.recorded_at,
        expires_at: receipt.expires_at,
        command_hash: receipt.command_hash,
        limitations: receipt.limitations.unwrap_or_default(),
        statuses: statuses.into_iter().collect(),
        issues,
        matched_card: matched_review_card.map(receipt_audit_card_from_review_card),
        matched_manual_candidate: matched_manual_candidate
            .map(receipt_audit_manual_candidate_from_manual_candidate),
        route_tools,
    }
}

fn receipt_imports_current_witness_evidence(
    receipt: &WitnessReceipt,
    statuses: &BTreeSet<String>,
    route_tools: &[String],
) -> bool {
    statuses.contains("matched")
        && !statuses.contains("manual_candidate")
        && !statuses.contains("invalid")
        && !statuses.contains("expired")
        && !statuses.contains("duplicate")
        && route_tools.iter().any(|tool| tool == &receipt.tool)
        && imports_witness_evidence(&receipt.tool, &receipt.strength)
}

fn receipt_imports_current_reach_evidence(
    receipt: &WitnessReceipt,
    statuses: &BTreeSet<String>,
) -> bool {
    statuses.contains("matched")
        && !statuses.contains("manual_candidate")
        && !statuses.contains("invalid")
        && !statuses.contains("expired")
        && !statuses.contains("duplicate")
        && imports_reach_evidence(&receipt.tool, &receipt.strength)
}

fn receipt_audit_card_from_review_card(card: &ReviewCard) -> ReceiptAuditCard {
    ReceiptAuditCard {
        id: card.id.0.clone(),
        class_name: card.class.as_str().to_string(),
        operation: card.operation.expression.clone(),
        operation_family: card.operation.family.as_str().to_string(),
        missing_count: card.missing.len(),
        next_action: card.next_action.summary.clone(),
        source: "analyzer".to_string(),
        manual_candidate: false,
        analyzer_discovered: true,
    }
}

fn receipt_audit_manual_candidate_from_manual_candidate(
    candidate: &ManualCandidate,
) -> ReceiptAuditManualCandidate {
    ReceiptAuditManualCandidate {
        id: candidate.id.clone(),
        title: candidate.title.clone(),
        location: format!(
            "{}:{}",
            candidate.location.file.display(),
            candidate.location.line
        ),
        operation: candidate.unsafe_operation.clone(),
        operation_family: candidate.operation_family.clone(),
        safe_caller: candidate.safe_caller.clone(),
        invariant: candidate.invariant.clone(),
        proof_mode: candidate.proof_mode.clone(),
        fix_boundary: candidate.fix_boundary.clone(),
        pr_aperture: candidate.pr_aperture.clone(),
        evidence: candidate
            .evidence
            .iter()
            .map(|evidence| ReceiptAuditManualCandidateEvidence {
                kind: evidence.kind.clone(),
                path: evidence
                    .path
                    .as_ref()
                    .map(|path| path.display().to_string()),
                summary: evidence.summary.clone(),
                command: evidence.command.clone(),
                limitation: evidence.limitation.clone(),
            })
            .collect(),
        fix_options: candidate.fix_options.clone(),
        test_targets: candidate.test_targets.clone(),
        do_not_touch: candidate.do_not_touch.clone(),
        next_action:
            "Review the manual candidate and preserve receipts as external evidence for this manual ID"
                .to_string(),
        trust_boundary: candidate.trust_boundary.clone(),
        source: "manual".to_string(),
        manual_candidate: true,
        analyzer_discovered: false,
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
        "reviewed" => Some(1),
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
            "command_hash_mismatch" => summary.command_hash_mismatch += 1,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::pipeline;
    use crate::api::{AnalysisMode, AnalyzeInput, DiffSource, PolicyMode, Scope};
    use crate::domain::WitnessKind;
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
            .witness_evidence_for(&CardId(card_id.to_string()), &routes_for(WitnessKind::Miri));
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
        let evidence = index
            .witness_evidence_for(&CardId(card_id.to_string()), &routes_for(WitnessKind::Miri));
        assert!(!evidence.present);
        assert!(evidence.summary.contains("expired"));
        Ok(())
    }

    #[test]
    fn receipt_index_skips_configured_receipts_for_witness_evidence() -> Result<(), String> {
        let root = unique_temp_dir("unsafe-review-configured-receipt-index")?;
        let receipts = root.join(".unsafe-review").join("receipts");
        fs::create_dir_all(&receipts).map_err(|err| format!("create receipt dir failed: {err}"))?;
        let card_id =
            "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1";
        write_receipt(
            &receipts,
            "configured.json",
            card_id,
            "miri",
            "configured",
            "2026-08-18",
        )?;

        let index = ReceiptIndex::load_with_date(&root, "2026-05-18")?;
        let validated = validate_receipts(&root)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp root failed: {err}"))?;
        assert_eq!(validated, 1);
        let evidence = index
            .witness_evidence_for(&CardId(card_id.to_string()), &routes_for(WitnessKind::Miri));
        assert!(!evidence.present);
        assert!(evidence.summary.contains("configured"));
        Ok(())
    }

    #[test]
    fn receipt_index_skips_unrouted_tools_for_witness_evidence() -> Result<(), String> {
        let root = unique_temp_dir("unsafe-review-unrouted-receipt-index")?;
        let receipts = root.join(".unsafe-review").join("receipts");
        fs::create_dir_all(&receipts).map_err(|err| format!("create receipt dir failed: {err}"))?;
        let card_id =
            "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1";
        write_receipt(&receipts, "loom.json", card_id, "loom", "ran", "2026-08-18")?;

        let index = ReceiptIndex::load_with_date(&root, "2026-05-18")?;
        let validated = validate_receipts(&root)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp root failed: {err}"))?;
        assert_eq!(validated, 1);
        let evidence = index
            .witness_evidence_for(&CardId(card_id.to_string()), &routes_for(WitnessKind::Miri));
        assert!(!evidence.present);
        assert!(
            evidence
                .summary
                .contains("does not match routed witness tools")
        );
        assert!(evidence.summary.contains("loom"));
        Ok(())
    }

    #[test]
    fn receipt_index_imports_external_integration_reach_evidence_only() -> Result<(), String> {
        let root = unique_temp_dir("unsafe-review-external-reach-receipt")?;
        let receipts = root.join(".unsafe-review").join("receipts");
        fs::create_dir_all(&receipts).map_err(|err| format!("create receipt dir failed: {err}"))?;
        let card_id =
            "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1";
        write_receipt(
            &receipts,
            "external-reach.json",
            card_id,
            "external-integration-test",
            "site_reached",
            "2026-08-18",
        )?;

        let index = ReceiptIndex::load_with_date(&root, "2026-05-18")?;
        let validated = validate_receipts(&root)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp root failed: {err}"))?;
        assert_eq!(validated, 1);
        let reach = index.reach_evidence_for(
            &CardId(card_id.to_string()),
            ReachEvidence {
                state: "unreached".to_string(),
                summary: "No static test mention of owner `read` was found".to_string(),
            },
        );
        assert_eq!(reach.state, "external_reached");
        assert!(reach.summary.contains("external-integration-test"));
        assert!(reach.summary.contains("site_reached"));

        let witness = index
            .witness_evidence_for(&CardId(card_id.to_string()), &routes_for(WitnessKind::Miri));
        assert!(!witness.present);
        assert!(witness.summary.contains("external-integration-test"));
        Ok(())
    }

    #[test]
    fn receipt_index_allows_witness_and_reach_receipts_for_same_card() -> Result<(), String> {
        let root = unique_temp_dir("unsafe-review-witness-and-reach-receipts")?;
        let receipts = root.join(".unsafe-review").join("receipts");
        fs::create_dir_all(&receipts).map_err(|err| format!("create receipt dir failed: {err}"))?;
        let card_id =
            "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1";
        write_receipt(&receipts, "miri.json", card_id, "miri", "ran", "2026-08-18")?;
        write_receipt(
            &receipts,
            "external-reach.json",
            card_id,
            "external-integration-test",
            "site_reached",
            "2026-08-18",
        )?;

        let index = ReceiptIndex::load_with_date(&root, "2026-05-18")?;
        let validated = validate_receipts(&root)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp root failed: {err}"))?;
        assert_eq!(validated, 2);
        let witness = index
            .witness_evidence_for(&CardId(card_id.to_string()), &routes_for(WitnessKind::Miri));
        assert!(witness.present);
        assert!(witness.summary.contains("miri"));
        let reach = index.reach_evidence_for(
            &CardId(card_id.to_string()),
            ReachEvidence {
                state: "unreached".to_string(),
                summary: "No static test mention of owner `read` was found".to_string(),
            },
        );
        assert_eq!(reach.state, "external_reached");
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
            "UR-not-counted",
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
        assert_eq!(report.summary.command_hash_mismatch, 0);
        assert_eq!(report.summary.duplicate, 4);
        assert_eq!(report.summary.invalid, 1);
        assert_eq!(report.limitations.len(), 6);
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
                .any(|limitation| limitation.contains("improve witness evidence only"))
        );
        assert!(
            report
                .limitations
                .iter()
                .any(|limitation| limitation.contains("improve reach evidence only"))
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
        let expected_command_hash = WitnessReceipt::command_hash("cargo test");
        assert_eq!(
            matched_entry.command_hash.as_deref(),
            Some(expected_command_hash.as_str())
        );
        assert!(
            !matched_entry
                .statuses
                .iter()
                .any(|status| status == "imports_witness_evidence")
        );
        assert_eq!(
            matched_entry.recorded_at.as_deref(),
            Some("2025-12-18T00:00:00Z")
        );
        assert_eq!(matched_entry.summary.as_deref(), Some("focused witness"));
        assert_eq!(matched_entry.author.as_deref(), Some("core/fixtures"));
        assert_eq!(matched_entry.limitations, vec!["fixture only"]);
        let duplicate_entries = report
            .receipts
            .iter()
            .filter(|entry| entry.statuses.iter().any(|status| status == "duplicate"))
            .count();
        assert_eq!(duplicate_entries, 4);
        Ok(())
    }

    #[test]
    fn receipt_audit_marks_only_importable_current_witness_receipts() -> Result<(), String> {
        let importable_root = copy_fixture_to_temp(
            "raw_pointer_alignment",
            "unsafe-review-receipt-audit-importable",
        )?;
        let importable_output = analyze_fixture_root(&importable_root)?;
        let importable_card_id = importable_output
            .cards
            .first()
            .ok_or_else(|| "fixture produced no card".to_string())?
            .id
            .0
            .clone();
        let importable_dir = importable_root.join(".unsafe-review").join("receipts");
        fs::create_dir_all(&importable_dir)
            .map_err(|err| format!("create receipt dir failed: {err}"))?;
        write_receipt(
            &importable_dir,
            "miri-ran.json",
            &importable_card_id,
            "miri",
            "ran",
            "2026-08-18",
        )?;

        let importable_report = audit_receipts_with_date(&importable_output, "2026-05-18")?;
        let importable_entry = importable_report
            .receipts
            .first()
            .ok_or_else(|| "importable receipt entry missing".to_string())?;
        assert!(
            importable_entry
                .statuses
                .iter()
                .any(|status| status == "imports_witness_evidence")
        );

        let configured_root = copy_fixture_to_temp(
            "raw_pointer_alignment",
            "unsafe-review-receipt-audit-configured",
        )?;
        let configured_output = analyze_fixture_root(&configured_root)?;
        let configured_card_id = configured_output
            .cards
            .first()
            .ok_or_else(|| "fixture produced no card".to_string())?
            .id
            .0
            .clone();
        let configured_dir = configured_root.join(".unsafe-review").join("receipts");
        fs::create_dir_all(&configured_dir)
            .map_err(|err| format!("create receipt dir failed: {err}"))?;
        write_receipt(
            &configured_dir,
            "miri-configured.json",
            &configured_card_id,
            "miri",
            "configured",
            "2026-08-18",
        )?;

        let configured_report = audit_receipts_with_date(&configured_output, "2026-05-18")?;
        let configured_entry = configured_report
            .receipts
            .first()
            .ok_or_else(|| "configured receipt entry missing".to_string())?;
        assert!(
            configured_entry
                .statuses
                .iter()
                .any(|status| status == "matched")
        );
        assert!(
            configured_entry
                .statuses
                .iter()
                .any(|status| status == "weaker_than_required")
        );
        assert!(
            !configured_entry
                .statuses
                .iter()
                .any(|status| status == "imports_witness_evidence")
        );

        let wrong_tool_root = copy_fixture_to_temp(
            "raw_pointer_alignment",
            "unsafe-review-receipt-audit-wrong-tool-importability",
        )?;
        let wrong_tool_output = analyze_fixture_root(&wrong_tool_root)?;
        let wrong_tool_card_id = wrong_tool_output
            .cards
            .first()
            .ok_or_else(|| "fixture produced no card".to_string())?
            .id
            .0
            .clone();
        let wrong_tool_dir = wrong_tool_root.join(".unsafe-review").join("receipts");
        fs::create_dir_all(&wrong_tool_dir)
            .map_err(|err| format!("create receipt dir failed: {err}"))?;
        write_receipt(
            &wrong_tool_dir,
            "loom-ran.json",
            &wrong_tool_card_id,
            "loom",
            "ran",
            "2026-08-18",
        )?;

        let wrong_tool_report = audit_receipts_with_date(&wrong_tool_output, "2026-05-18")?;
        let wrong_tool_entry = wrong_tool_report
            .receipts
            .first()
            .ok_or_else(|| "wrong-tool receipt entry missing".to_string())?;
        assert!(
            wrong_tool_entry
                .statuses
                .iter()
                .any(|status| status == "matched")
        );
        assert!(
            wrong_tool_entry
                .statuses
                .iter()
                .any(|status| status == "wrong_tool")
        );
        assert!(
            !wrong_tool_entry
                .statuses
                .iter()
                .any(|status| status == "imports_witness_evidence")
        );

        fs::remove_dir_all(&importable_root)
            .map_err(|err| format!("remove importable temp root failed: {err}"))?;
        fs::remove_dir_all(&configured_root)
            .map_err(|err| format!("remove configured temp root failed: {err}"))?;
        fs::remove_dir_all(&wrong_tool_root)
            .map_err(|err| format!("remove wrong-tool temp root failed: {err}"))?;
        Ok(())
    }

    #[test]
    fn receipt_audit_marks_external_integration_reach_receipts() -> Result<(), String> {
        let root = copy_fixture_to_temp(
            "ffi_missing_boundary_contract",
            "unsafe-review-receipt-audit-external-reach",
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
            "external-reach.json",
            &card_id,
            "external-integration-test",
            "site_reached",
            "2026-08-18",
        )?;

        let report = audit_receipts_with_date(&output, "2026-05-18")?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp root failed: {err}"))?;
        assert_eq!(report.summary.receipts, 1);
        assert_eq!(report.summary.matched, 1);
        assert_eq!(report.summary.wrong_tool, 0);
        let entry = report
            .receipts
            .first()
            .ok_or_else(|| "external reach receipt entry missing".to_string())?;
        assert!(entry.statuses.iter().any(|status| status == "matched"));
        assert!(
            entry
                .statuses
                .iter()
                .any(|status| status == "imports_reach_evidence")
        );
        assert!(
            !entry
                .statuses
                .iter()
                .any(|status| status == "imports_witness_evidence")
        );
        assert!(!entry.statuses.iter().any(|status| status == "wrong_tool"));
        assert_eq!(
            entry.receipt_tool.as_deref(),
            Some("external-integration-test")
        );
        Ok(())
    }

    #[test]
    fn receipt_audit_matches_manual_candidate_receipts_preserving_manual_marker()
    -> Result<(), String> {
        let root = copy_fixture_to_temp(
            "raw_pointer_alignment",
            "unsafe-review-receipt-audit-manual-candidate",
        )?;
        let candidate_dir = root.join(".unsafe-review").join("candidates");
        fs::create_dir_all(&candidate_dir)
            .map_err(|err| format!("create candidate dir failed: {err}"))?;
        fs::write(
            candidate_dir.join("R4R2-S001.json"),
            manual_candidate_json(),
        )
        .map_err(|err| format!("write manual candidate failed: {err}"))?;
        let receipt_dir = root.join(".unsafe-review").join("receipts");
        fs::create_dir_all(&receipt_dir)
            .map_err(|err| format!("create receipt dir failed: {err}"))?;
        write_receipt(
            &receipt_dir,
            "manual-candidate.json",
            "R4R2-S001",
            "human-deep-review",
            "test_targeted",
            "2026-08-18",
        )?;
        let output = analyze_fixture_root(&root)?;

        let report = audit_receipts_with_date(&output, "2026-05-18")?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp root failed: {err}"))?;
        assert_eq!(report.summary.receipts, 1);
        assert_eq!(report.summary.matched, 1);
        assert_eq!(report.summary.unmatched, 0);
        assert_eq!(report.summary.wrong_identity, 0);
        assert_eq!(report.summary.invalid, 0);
        let entry = report
            .receipts
            .first()
            .ok_or_else(|| "manual candidate receipt entry missing".to_string())?;
        assert!(entry.statuses.iter().any(|status| status == "matched"));
        assert!(
            entry
                .statuses
                .iter()
                .any(|status| status == "manual_candidate")
        );
        assert!(
            !entry
                .statuses
                .iter()
                .any(|status| status == "imports_witness_evidence")
        );
        assert!(entry.route_tools.is_empty());
        assert!(entry.matched_card.is_none());
        let matched = entry.matched_manual_candidate.as_ref().ok_or_else(|| {
            "manual candidate receipt should include candidate context".to_string()
        })?;
        assert_eq!(matched.id, "R4R2-S001");
        assert_eq!(matched.source, "manual");
        assert!(matched.manual_candidate);
        assert!(!matched.analyzer_discovered);
        assert_eq!(
            matched.title,
            "TextDecoder SharedArrayBuffer decode creates &[u8] over shared bytes"
        );
        assert_eq!(matched.location, "src/runtime/webcore/TextDecoder.rs:237");
        assert_eq!(matched.operation_family, "raw_pointer_read");
        assert_eq!(matched.operation, "core::slice::from_raw_parts");
        assert_eq!(
            matched.safe_caller,
            "new TextDecoder().decode(new Uint8Array(new SharedArrayBuffer(...)))"
        );
        assert_eq!(
            matched.invariant,
            "&[u8] memory must not be concurrently mutated"
        );
        assert_eq!(
            matched.proof_mode.as_ref().map(|mode| mode.kind.as_str()),
            Some("mutation-plus-miri")
        );
        assert_eq!(
            matched.fix_boundary.as_deref(),
            Some("Snapshot shared/growable/resizable bytes before Rust receives &[u8]")
        );
        assert!(
            matched
                .pr_aperture
                .as_deref()
                .unwrap_or("")
                .contains("do not patch S3")
        );
        assert_eq!(matched.evidence.len(), 1);
        assert_eq!(
            matched.evidence[0].command.as_deref(),
            Some("bun test test/js/webcore/textdecoder-sharedarraybuffer.test.ts")
        );
        assert!(
            matched.evidence[0]
                .limitation
                .as_deref()
                .unwrap_or("")
                .contains("not memory-safety proof")
        );
        assert!(
            matched.fix_options[0].contains("Copy SharedArrayBuffer-backed bytes"),
            "{:?}",
            matched.fix_options
        );
        assert_eq!(
            matched.test_targets[0],
            "test/js/webcore/textdecoder-sharedarraybuffer.test.ts"
        );
        assert!(
            matched.do_not_touch[0].contains("unrelated TextDecoder"),
            "{:?}",
            matched.do_not_touch
        );
        assert!(
            matched
                .next_action
                .contains("external evidence for this manual ID")
        );
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
            r#"{"schema_version":"0.1","card_id":"UR-not-counted","tool":"proof-bot","strength":"ran"}"#,
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

    #[test]
    fn receipt_audit_reports_command_hash_mismatch_without_losing_card_context()
    -> Result<(), String> {
        let root = copy_fixture_to_temp(
            "raw_pointer_alignment",
            "unsafe-review-receipt-audit-command-hash-mismatch",
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
        fs::write(
            receipt_dir.join("bad-command-hash.json"),
            format!(
                r#"{{
  "schema_version": "0.1",
  "card_id": "{card_id}",
  "tool": "miri",
  "strength": "ran",
  "author": "core/fixtures",
  "recorded_at": "2025-12-18T00:00:00Z",
  "expires_at": "2026-08-18",
  "summary": "focused witness",
  "command": "cargo test",
  "command_hash": "0000000000000000",
  "limitations": ["fixture only"]
}}"#
            ),
        )
        .map_err(|err| format!("write receipt failed: {err}"))?;

        let report = audit_receipts_with_date(&output, "2026-05-18")?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp root failed: {err}"))?;
        assert_eq!(report.summary.receipts, 1);
        assert_eq!(report.summary.matched, 1);
        assert_eq!(report.summary.command_hash_mismatch, 1);
        assert_eq!(report.summary.invalid, 1);
        let entry = report
            .receipts
            .first()
            .ok_or_else(|| "receipt audit entry missing".to_string())?;
        assert_eq!(entry.command_hash.as_deref(), Some("0000000000000000"));
        assert!(entry.statuses.iter().any(|status| status == "matched"));
        assert!(entry.statuses.iter().any(|status| status == "invalid"));
        assert!(
            entry
                .statuses
                .iter()
                .any(|status| status == "command_hash_mismatch")
        );
        assert!(entry.matched_card.is_some());
        assert!(
            entry
                .issues
                .iter()
                .any(|issue| issue.contains("command_hash"))
        );
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
        let command = if tool == "external-integration-test" {
            "bun test test/js/sab-copy-to-unshared.test.ts"
        } else {
            "cargo test"
        };
        let command_hash = WitnessReceipt::command_hash(command);
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
  "command": "{command}",
  "command_hash": "{command_hash}",
  "limitations": ["fixture only"]
}}"#
            ),
        )
        .map_err(|err| format!("write receipt {name} failed: {err}"))
    }

    fn manual_candidate_json() -> &'static str {
        r#"{
  "schema_version": "manual-candidate/v1",
  "id": "R4R2-S001",
  "title": "TextDecoder SharedArrayBuffer decode creates &[u8] over shared bytes",
  "location": {
    "file": "src/runtime/webcore/TextDecoder.rs",
    "line": 237
  },
  "operation_family": "raw_pointer_read",
  "unsafe_operation": "core::slice::from_raw_parts",
  "invariant": "&[u8] memory must not be concurrently mutated",
  "safe_caller": "new TextDecoder().decode(new Uint8Array(new SharedArrayBuffer(...)))",
  "proof_mode": {
    "kind": "mutation-plus-miri",
    "system_bun_expected": "nondiscriminating",
    "mutation_required": true,
    "miri_required": true
  },
  "fix_boundary": "Snapshot shared/growable/resizable bytes before Rust receives &[u8]",
  "pr_aperture": "TextDecoder shared-byte snapshot only; do not patch S3, fs, writev, or unrelated encodings",
  "evidence": [
    {
      "kind": "runtime_witness",
      "path": "target/unsafe-scout/textdecoder-shared-race-route.out",
      "command": "bun test test/js/webcore/textdecoder-sharedarraybuffer.test.ts",
      "limitation": "runtime route evidence only; not memory-safety proof and not analyzer-discovered"
    }
  ],
  "fix_options": [
    "Copy SharedArrayBuffer-backed bytes into stable owned storage before creating a Rust slice"
  ],
  "test_targets": [
    "test/js/webcore/textdecoder-sharedarraybuffer.test.ts"
  ],
  "do_not_touch": [
    "Do not rewrite unrelated TextDecoder encoding paths"
  ],
  "trust_boundary": "manual candidate; not analyzer-discovered; not proof of repository safety"
}"#
    }

    fn routes_for(kind: WitnessKind) -> Vec<WitnessRoute> {
        vec![WitnessRoute {
            kind,
            reason: "fixture route".to_string(),
            command: None,
            required: false,
        }]
    }
}
