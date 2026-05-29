use crate::api::AnalyzeOutput;
use crate::domain::ReviewCard;
use crate::output::agent;
use crate::util::path_display;
use serde::Serialize;

const TRUST_BOUNDARY: &str = "Static unsafe contract review only; this is not a proof of memory safety, not UB-free status, not a Miri result, and not an automatic repair queue.";

pub(crate) fn render(output: &AnalyzeOutput) -> String {
    render_pretty(&RepairQueueArtifact::from(output))
}

fn render_pretty(value: &impl Serialize) -> String {
    match serde_json::to_string_pretty(value) {
        Ok(text) => text,
        Err(err) => format!("{{\n  \"error\": \"repair queue serialization failed: {err}\"\n}}"),
    }
}

#[derive(Serialize)]
struct RepairQueueArtifact {
    schema_version: &'static str,
    tool: &'static str,
    mode: &'static str,
    source: &'static str,
    policy: &'static str,
    trust_boundary: &'static str,
    summary: RepairQueueSummary,
    buckets: RepairQueueBuckets,
}

impl From<&AnalyzeOutput> for RepairQueueArtifact {
    fn from(output: &AnalyzeOutput) -> Self {
        let mut buckets = RepairQueueBuckets::default();
        for card in &output.cards {
            let projection = agent::repair_queue_projection(card);
            for bucket in aggregate_buckets(&projection) {
                buckets.push(bucket, RepairQueueEntry::new(card, bucket, &projection));
            }
        }
        let summary = RepairQueueSummary::from(&buckets);
        Self {
            schema_version: "0.1",
            tool: "unsafe-review",
            mode: "aggregate_repair_queue",
            source: "review_card",
            policy: output.policy.as_str(),
            trust_boundary: TRUST_BOUNDARY,
            summary,
            buckets,
        }
    }
}

#[derive(Default, Serialize)]
struct RepairQueueBuckets {
    repairable_by_guard: Vec<RepairQueueEntry>,
    repairable_by_contract: Vec<RepairQueueEntry>,
    repairable_by_test: Vec<RepairQueueEntry>,
    requires_witness_receipt: Vec<RepairQueueEntry>,
    requires_human_review: Vec<RepairQueueEntry>,
    do_not_auto_repair: Vec<RepairQueueEntry>,
}

impl RepairQueueBuckets {
    fn push(&mut self, bucket: &'static str, entry: RepairQueueEntry) {
        match bucket {
            "repairable_by_guard" => self.repairable_by_guard.push(entry),
            "repairable_by_contract" => self.repairable_by_contract.push(entry),
            "repairable_by_test" => self.repairable_by_test.push(entry),
            "requires_witness_receipt" => self.requires_witness_receipt.push(entry),
            "requires_human_review" => self.requires_human_review.push(entry),
            "do_not_auto_repair" => self.do_not_auto_repair.push(entry),
            _ => {}
        }
    }
}

#[derive(Serialize)]
struct RepairQueueSummary {
    cards: usize,
    repairable_by_guard: usize,
    repairable_by_contract: usize,
    repairable_by_test: usize,
    requires_witness_receipt: usize,
    requires_human_review: usize,
    do_not_auto_repair: usize,
}

impl From<&RepairQueueBuckets> for RepairQueueSummary {
    fn from(buckets: &RepairQueueBuckets) -> Self {
        Self {
            cards: unique_card_count(buckets),
            repairable_by_guard: buckets.repairable_by_guard.len(),
            repairable_by_contract: buckets.repairable_by_contract.len(),
            repairable_by_test: buckets.repairable_by_test.len(),
            requires_witness_receipt: buckets.requires_witness_receipt.len(),
            requires_human_review: buckets.requires_human_review.len(),
            do_not_auto_repair: buckets.do_not_auto_repair.len(),
        }
    }
}

#[derive(Serialize)]
struct RepairQueueEntry {
    card_id: String,
    class: &'static str,
    priority: &'static str,
    confidence: &'static str,
    operation_family: &'static str,
    operation: String,
    path: String,
    line: usize,
    missing_evidence: Vec<String>,
    agent_readiness: RepairQueueReadiness,
    bucket_reason: &'static str,
    context_command: String,
    do_not_do: &'static [&'static str],
    trust_boundary: &'static str,
}

impl RepairQueueEntry {
    fn new(
        card: &ReviewCard,
        bucket: &'static str,
        projection: &agent::AgentQueueProjection,
    ) -> Self {
        Self {
            card_id: card.id.0.clone(),
            class: card.class.as_str(),
            priority: card.priority.as_str(),
            confidence: card.confidence.as_str(),
            operation_family: card.operation.family.as_str(),
            operation: card.operation.expression.clone(),
            path: path_display(&card.site.location.file),
            line: card.site.location.line,
            missing_evidence: missing_evidence(card),
            agent_readiness: RepairQueueReadiness::from(&projection.agent_readiness),
            bucket_reason: bucket_reason(bucket),
            context_command: format!("unsafe-review context {} --json", card.id),
            do_not_do: agent::DO_NOT_DO,
            trust_boundary: TRUST_BOUNDARY,
        }
    }
}

#[derive(Serialize)]
struct RepairQueueReadiness {
    ready: bool,
    state: &'static str,
    reasons: Vec<String>,
}

impl From<&agent::AgentReadiness> for RepairQueueReadiness {
    fn from(readiness: &agent::AgentReadiness) -> Self {
        Self {
            ready: readiness.ready,
            state: readiness.state,
            reasons: readiness.reasons.clone(),
        }
    }
}

fn aggregate_buckets(projection: &agent::AgentQueueProjection) -> Vec<&'static str> {
    let mut buckets = Vec::new();
    for bucket in &projection.repair_queue.buckets {
        let mapped = match *bucket {
            "repairable_by_guard"
            | "repairable_by_contract"
            | "repairable_by_test"
            | "requires_witness_receipt"
            | "requires_human_review" => *bucket,
            "review_only" => "do_not_auto_repair",
            _ => continue,
        };
        push_unique(&mut buckets, mapped);
    }
    if !projection.agent_readiness.ready {
        push_unique(&mut buckets, "do_not_auto_repair");
    }
    buckets
}

fn push_unique(values: &mut Vec<&'static str>, value: &'static str) {
    if !values.contains(&value) {
        values.push(value);
    }
}

fn bucket_reason(bucket: &str) -> &'static str {
    match bucket {
        "repairable_by_guard" => "guard_evidence_missing",
        "repairable_by_contract" => "contract_evidence_missing",
        "repairable_by_test" => "reach_evidence_missing",
        "requires_witness_receipt" => "witness_receipt_missing",
        "requires_human_review" => "human_review_required",
        "do_not_auto_repair" => "not_ready_for_automatic_repair",
        _ => "unknown_bucket",
    }
}

fn missing_evidence(card: &ReviewCard) -> Vec<String> {
    card.missing
        .iter()
        .map(|missing| missing.message.clone())
        .collect()
}

fn unique_card_count(buckets: &RepairQueueBuckets) -> usize {
    let mut ids = Vec::<&str>::new();
    for entry in buckets
        .repairable_by_guard
        .iter()
        .chain(&buckets.repairable_by_contract)
        .chain(&buckets.repairable_by_test)
        .chain(&buckets.requires_witness_receipt)
        .chain(&buckets.requires_human_review)
        .chain(&buckets.do_not_auto_repair)
    {
        if !ids.contains(&entry.card_id.as_str()) {
            ids.push(entry.card_id.as_str());
        }
    }
    ids.len()
}

#[cfg(test)]
mod tests {
    use super::render;
    use crate::api::{
        AnalysisMode, AnalyzeInput, AnalyzeOutput, DiffSource, PolicyMode, Scope, analyze,
    };
    use std::path::PathBuf;

    #[test]
    fn repair_queue_groups_ready_guard_cards() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let value = parse_json(&render(&output))?;
        let card_id = output.cards[0].id.0.as_str();

        assert_eq!(value["schema_version"], "0.1");
        assert_eq!(value["tool"], "unsafe-review");
        assert_eq!(value["mode"], "aggregate_repair_queue");
        assert_eq!(value["source"], "review_card");
        assert_eq!(value["policy"], "advisory");
        assert_eq!(value["summary"]["cards"], 1);
        assert_eq!(value["summary"]["repairable_by_guard"], 1);
        assert_eq!(value["summary"]["requires_witness_receipt"], 1);
        assert_eq!(value["summary"]["requires_human_review"], 0);
        assert_eq!(value["summary"]["do_not_auto_repair"], 0);

        let guard = &value["buckets"]["repairable_by_guard"][0];
        assert_eq!(guard["card_id"], card_id);
        assert_eq!(guard["class"], "guard_missing");
        assert_eq!(guard["operation_family"], "raw_pointer_read");
        assert_eq!(guard["bucket_reason"], "guard_evidence_missing");
        assert_eq!(
            guard["context_command"],
            format!("unsafe-review context {card_id} --json")
        );
        assert_eq!(guard["agent_readiness"]["ready"], true);
        assert!(
            serde_json::to_string(&guard["missing_evidence"])
                .map_err(|err| format!("render missing evidence failed: {err}"))?
                .contains("Missing visible local guard")
        );
        assert!(
            guard["trust_boundary"]
                .as_str()
                .unwrap_or("")
                .contains("not UB-free status")
        );
        assert_repair_queue_boundaries(guard)?;
        Ok(())
    }

    #[test]
    fn repair_queue_marks_human_review_cards_do_not_auto_repair() -> Result<(), String> {
        let output = fixture_output("ffi_sanitizer_route")?;
        let value = parse_json(&render(&output))?;
        let card_id = output.cards[0].id.0.as_str();

        assert_eq!(value["summary"]["requires_human_review"], 1);
        assert_eq!(value["summary"]["do_not_auto_repair"], 1);
        let human = &value["buckets"]["requires_human_review"][0];
        let no_auto = &value["buckets"]["do_not_auto_repair"][0];
        assert_eq!(human["card_id"], card_id);
        assert_eq!(no_auto["card_id"], card_id);
        assert_eq!(human["operation_family"], "ffi");
        assert_eq!(no_auto["bucket_reason"], "not_ready_for_automatic_repair");
        assert_eq!(human["agent_readiness"]["ready"], false);
        assert!(
            serde_json::to_string(&human["agent_readiness"]["reasons"])
                .map_err(|err| format!("render readiness reasons failed: {err}"))?
                .contains("ffi")
        );
        assert_repair_queue_boundaries(human)?;
        assert_repair_queue_boundaries(no_auto)?;
        Ok(())
    }

    fn fixture_output(name: &str) -> Result<AnalyzeOutput, String> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name);
        analyze(AnalyzeInput {
            root: root.clone(),
            scope: Scope::Diff,
            diff: DiffSource::File(root.join("change.diff")),
            mode: AnalysisMode::Draft,
            policy: PolicyMode::Advisory,
            include_unchanged_tests: true,
            max_cards: None,
        })
    }

    fn parse_json(text: &str) -> Result<serde_json::Value, String> {
        serde_json::from_str(text).map_err(|err| format!("JSON parse failed: {err}"))
    }

    fn assert_repair_queue_boundaries(entry: &serde_json::Value) -> Result<(), String> {
        let do_not_do = entry["do_not_do"]
            .as_array()
            .ok_or("do_not_do should be an array")?;
        for item in do_not_do {
            let Some(text) = item.as_str() else {
                return Err("do_not_do entries should be strings".to_string());
            };
            if !text.starts_with("do not ") {
                return Err(format!("do_not_do entry must start with `do not`: {text}"));
            }
        }
        let rules = serde_json::to_string(&entry["do_not_do"])
            .map_err(|err| format!("render do_not_do failed: {err}"))?;
        for expected in [
            "suppress this card",
            "broad suppression",
            "executable guard or discharge evidence",
            "automatic safety repair",
            "ran an agent, ran witnesses, applied source edits, or posted comments",
            "unrelated unsafe code",
            "test mention as proof",
        ] {
            if !rules.contains(expected) {
                return Err(format!("repair queue do_not_do must include `{expected}`"));
            }
        }
        Ok(())
    }
}
