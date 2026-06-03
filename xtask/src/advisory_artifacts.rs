use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

struct AdvisoryArtifactSummary {
    card_ids: BTreeSet<String>,
    card_projections: BTreeMap<String, CardProjection>,
    repair_queue_projections: BTreeMap<String, RepairQueueProjection>,
    scope: String,
    card_count: usize,
    open_actionable_gaps: usize,
    high_priority_cards: usize,
}

struct AdvisoryArtifactManifest {
    card_ids: BTreeSet<String>,
    card_projections: BTreeMap<String, CardProjection>,
    scope: String,
    card_count: usize,
    open_actionable_gaps: usize,
    high_priority_cards: usize,
}

struct CardProjection {
    id: String,
    class_name: String,
    priority: String,
    confidence: String,
    hazards: Vec<String>,
    path: String,
    line: u64,
    column: u64,
    operation: String,
    operation_family: String,
    next_action: String,
    missing: Vec<String>,
    contract: Option<String>,
    discharge: Option<String>,
    reach: Option<String>,
    witness: Option<String>,
    required_safety_conditions: Vec<serde_json::Value>,
    obligation_evidence: Vec<serde_json::Value>,
    verify_commands: Vec<String>,
    witness_routes: Vec<WitnessRouteProjection>,
}

struct WitnessRouteProjection {
    kind: String,
    reason: String,
    command: Option<String>,
    required: bool,
}

struct RepairQueueProjection {
    buckets: Vec<String>,
    readiness_state: String,
    readiness_reasons: Vec<String>,
}

struct RepairQueueEntryProjection {
    card_id: String,
    readiness_state: String,
    readiness_reasons: Vec<String>,
}

struct RepairQueueReadinessProjection {
    state: String,
    reasons: Vec<String>,
}

const COMMENT_PLAN_BODY_WORD_LIMIT: usize = 220;
const COMMENT_PLAN_REVIEW_BUDGET: usize = 3;
const COMMENT_PLAN_REVIEW_BUDGET_REASON: &str = "bounded reviewer noise";
const COMMENT_PLAN_REVIEW_BUDGET_REASON_CODE: &str = "bounded_reviewer_noise";
const COMMENT_PLAN_SELECTION_REASONS: &[&str] = &[
    "actionable high-confidence review card",
    "actionable high-priority review card",
];
const COMMENT_PLAN_SELECTION_REASON_CODES: &[&str] = &["top_actionable_card"];
const COMMENT_PLAN_NON_SELECTION_REASONS: &[&str] = &[
    "outside changed hunk",
    "class not eligible for inline comments",
    "operation family unknown",
    "confidence below inline comment threshold",
    "priority/confidence below inline comment threshold",
    "covered by selected family/obligation sibling",
    "comment-plan max of three candidates reached",
    "not selected by current inline comment policy",
];
const COMMENT_PLAN_NON_SELECTION_REASON_CODES: &[&str] = &[
    "outside_changed_hunk",
    "human_deep_review_only",
    "lower_relevance",
    "covered_by_selected_family_obligation",
    "budget_exhausted",
    "not_selected_by_policy",
];
const REPAIR_QUEUE_BUCKETS: [&str; 6] = [
    "repairable_by_guard",
    "repairable_by_safety_docs",
    "repairable_by_test",
    "requires_witness_receipt",
    "requires_human_review",
    "do_not_auto_repair",
];
const REPAIR_QUEUE_READINESS_STATES: [&str; 4] = [
    "ready_for_agent",
    "requires_human_review",
    "requires_witness_receipt",
    "unsupported",
];
const FIRST_PR_BUNDLE_ARTIFACTS: [&str; 11] = [
    "review-kit.json",
    "cards.json",
    "pr-summary.md",
    "github-summary.md",
    "cards.sarif",
    "comment-plan.json",
    "witness-plan.md",
    "receipt-audit.md",
    "manual-candidates.json",
    "lsp.json",
    "repair-queue.json",
];
const REPAIR_QUEUE_TRUST_BOUNDARY_LIMITS: [&str; 7] = [
    "not an automatic repair queue",
    "does not run agents",
    "does not run witnesses",
    "does not edit source",
    "does not post comments",
    "does not suppress cards",
    "does not resolve cards",
];

pub(crate) fn check_advisory_artifacts(dir: &Path) -> Result<(), String> {
    check_advisory_artifact_set(dir)?;
    check_advisory_artifact_overclaims(dir)?;
    println!("check-advisory-artifacts: ok ({})", dir.display());
    Ok(())
}

pub(crate) fn check_first_pr_artifacts(dir: &Path) -> Result<(), String> {
    let summary = check_advisory_artifact_set(dir)?;
    require_expected_value(
        &summary.scope,
        "diff",
        "cards.json scope for first-pr artifacts",
    )?;
    check_witness_plan_artifact(
        dir,
        summary.card_count,
        summary.open_actionable_gaps,
        &summary.card_projections,
    )?;
    check_receipt_audit_artifact(dir)?;
    check_manual_candidates_artifact(dir)?;
    check_lsp_artifact(dir, &summary)?;
    check_github_summary_artifact(
        dir,
        &summary.scope,
        summary.card_count,
        summary.open_actionable_gaps,
        &summary.card_ids,
        &summary.card_projections,
    )?;
    check_first_pr_markdown_card_identity(
        dir,
        &summary.card_ids,
        &summary.card_projections,
        &summary.repair_queue_projections,
    )?;
    check_review_kit_manifest(
        dir,
        &summary.scope,
        summary.card_count,
        summary.open_actionable_gaps,
        &summary.card_ids,
    )?;
    check_advisory_artifact_overclaims(dir)?;

    println!("check-first-pr-artifacts: ok ({})", dir.display());
    Ok(())
}

const GITHUB_SUMMARY_WORD_LIMIT: usize = 600;

fn check_github_summary_artifact(
    dir: &Path,
    scope: &str,
    card_count: usize,
    open_actionable_gaps: usize,
    card_ids: &BTreeSet<String>,
    card_projections: &BTreeMap<String, CardProjection>,
) -> Result<(), String> {
    let path = dir.join("github-summary.md");
    let text = super::read_to_string(&path)?;
    require_text_mentions_only_known_card_ids(&text, &path, card_ids)?;

    super::require_text_contains(&text, "## unsafe-review advisory summary", &path)?;
    super::require_text_contains(&text, &format!("- Scope: `{scope}`"), &path)?;
    super::require_text_contains(&text, &format!("- Review cards: {card_count}"), &path)?;
    super::require_text_contains(
        &text,
        &format!("- Open actionable gaps: {open_actionable_gaps}"),
        &path,
    )?;
    super::require_text_contains(&text, "- Policy mode: `advisory`", &path)?;
    super::require_text_contains(&text, "## Top card", &path)?;
    super::require_text_contains(&text, "## Open next", &path)?;
    super::require_text_contains(&text, "- Review kit manifest: `review-kit.json`", &path)?;
    super::require_text_contains(&text, "- Full reviewer cockpit: `pr-summary.md`", &path)?;
    super::require_text_contains(&text, "- Machine-readable ReviewCards: `cards.json`", &path)?;
    super::require_text_contains(&text, "- Witness routes: `witness-plan.md`", &path)?;
    super::require_text_contains(
        &text,
        "- Receipt audit: `receipt-audit.md` checks saved receipt metadata only; no witness was run.",
        &path,
    )?;
    super::require_text_contains(
        &text,
        "- Manual candidate index: `manual-candidates.json` lists imported advisory candidates separately from ReviewCards.",
        &path,
    )?;
    super::require_text_contains(
        &text,
        "- Agent repair queue: `repair-queue.json` is copy-only; no agent was run.",
        &path,
    )?;
    super::require_text_contains(
        &text,
        "- Comment budget: `comment-plan.json` is plan-only; no comments were posted.",
        &path,
    )?;
    super::require_text_contains(&text, "static unsafe contract review", &path)?;
    super::require_text_contains(&text, "not memory-safety proof", &path)?;
    super::require_text_contains(&text, "not UB-free status", &path)?;
    super::require_text_contains(&text, "not Miri-clean status", &path)?;
    super::require_text_contains(&text, "not site-execution proof", &path)?;
    super::require_text_contains(
        &text,
        "Full advisory bundle (review-kit.json, cards.json, pr-summary.md, github-summary.md, cards.sarif, comment-plan.json, witness-plan.md, receipt-audit.md, manual-candidates.json, lsp.json, repair-queue.json)",
        &path,
    )?;

    if text.contains("# unsafe-review PR summary") {
        return Err(format!(
            "{} must not include the full `# unsafe-review PR summary` document (use pr-summary.md for that)",
            path.display()
        ));
    }
    if text.contains("## Card table") {
        return Err(format!(
            "{} must not include the full `## Card table` section (use pr-summary.md for that)",
            path.display()
        ));
    }
    if text.contains("## Witness plan") {
        return Err(format!(
            "{} must not include the full `## Witness plan` section (use pr-summary.md for that)",
            path.display()
        ));
    }

    let word_count = text.split_whitespace().count();
    if word_count > GITHUB_SUMMARY_WORD_LIMIT {
        return Err(format!(
            "{} is {word_count} words; github-summary.md must stay under {GITHUB_SUMMARY_WORD_LIMIT}",
            path.display()
        ));
    }

    if card_count == 0 {
        super::require_text_contains(&text, "No changed unsafe-review gaps were found.", &path)?;
        super::require_text_contains(&text, "This does not prove the repo safe", &path)?;
        super::require_text_contains(&text, "unsafe site executed", &path)?;
    } else {
        require_markdown_top_card_projection(&text, &path, card_projections)?;
    }

    Ok(())
}

fn check_receipt_audit_artifact(dir: &Path) -> Result<(), String> {
    let path = dir.join("receipt-audit.md");
    let text = super::read_to_string(&path)?;

    super::require_text_contains(&text, "# unsafe-review receipt audit", &path)?;
    super::require_text_contains(
        &text,
        "Static audit of saved witness receipt metadata",
        &path,
    )?;
    super::require_text_contains(&text, "## Summary", &path)?;
    super::require_text_contains(&text, "## Reviewer front panel", &path)?;
    super::require_text_contains(&text, "## Trust boundary", &path)?;
    super::require_text_contains(&text, "does not execute witnesses", &path)?;
    super::require_text_contains(&text, "does not prove site reach", &path)?;
    super::require_text_contains(
        &text,
        "matched receipts improve witness evidence only",
        &path,
    )?;

    Ok(())
}

fn check_manual_candidates_artifact(dir: &Path) -> Result<(), String> {
    let path = dir.join("manual-candidates.json");
    let value = super::parse_json_file(&path)?;
    super::require_json_str(
        &value,
        "schema_version",
        "manual-candidates/v1",
        "manual-candidates.json",
    )?;
    super::require_json_str(&value, "tool", "unsafe-review", "manual-candidates.json")?;
    super::require_json_str(
        &value,
        "mode",
        "manual_candidate_index",
        "manual-candidates.json",
    )?;
    super::require_json_str(&value, "source", "first_pr", "manual-candidates.json")?;
    super::require_non_empty_json_str(&value, "tool_version", "manual-candidates.json")?;

    let candidates = super::json_array_at(&value, "/candidates", "manual-candidates.json")?;
    let summary_count = super::json_usize_at(
        &value,
        "/summary/manual_candidates",
        "manual-candidates.json",
    )?;
    if summary_count != candidates.len() {
        return Err(format!(
            "manual-candidates.json summary.manual_candidates is {summary_count}, but candidates array has {}",
            candidates.len()
        ));
    }
    let analyzer_discovered = super::json_usize_at(
        &value,
        "/summary/analyzer_discovered",
        "manual-candidates.json",
    )?;
    if analyzer_discovered != 0 {
        return Err("manual-candidates.json summary.analyzer_discovered must stay 0".to_string());
    }

    let relationship = value
        .get("reviewcard_artifact_relationship")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| {
            "manual-candidates.json is missing reviewcard_artifact_relationship object".to_string()
        })?;
    for artifact in [
        "cards.json",
        "cards.sarif",
        "comment-plan.json",
        "lsp.json",
        "repair-queue.json",
        "policy-report",
    ] {
        let Some(text) = relationship
            .get(artifact)
            .and_then(serde_json::Value::as_str)
        else {
            return Err(format!(
                "manual-candidates.json relationship is missing `{artifact}`"
            ));
        };
        if !super::text_contains_ignore_ascii_case(text, "ReviewCard-only") {
            return Err(format!(
                "manual-candidates.json relationship `{artifact}` must say ReviewCard-only"
            ));
        }
    }

    let boundary =
        super::require_non_empty_json_str(&value, "trust_boundary", "manual-candidates.json")?;
    super::require_boundary_text(boundary, "manual-candidates.json")?;
    for expected in [
        "manual/advisory",
        "not analyzer-discovered",
        "not site-execution proof",
        "not policy gating",
        "did not run witnesses",
        "post comments",
        "edit source",
        "run an agent",
        "enforce blocking policy",
    ] {
        if !super::text_contains_ignore_ascii_case(boundary, expected) {
            return Err(format!(
                "manual-candidates.json trust_boundary must include `{expected}`"
            ));
        }
    }

    for candidate in candidates {
        check_manual_candidate_artifact_entry(candidate)?;
    }

    Ok(())
}

fn check_manual_candidate_artifact_entry(candidate: &serde_json::Value) -> Result<(), String> {
    super::require_json_str(
        candidate,
        "schema_version",
        "manual-candidate/v1",
        "manual-candidates.json candidate",
    )?;
    super::require_non_empty_json_str(candidate, "id", "manual-candidates.json candidate")?;
    super::require_json_str(
        candidate,
        "source",
        "manual",
        "manual-candidates.json candidate",
    )?;
    if candidate.get("manual_candidate") != Some(&serde_json::Value::Bool(true)) {
        return Err("manual-candidates.json candidate manual_candidate must be true".to_string());
    }
    if candidate.get("analyzer_discovered") != Some(&serde_json::Value::Bool(false)) {
        return Err(
            "manual-candidates.json candidate analyzer_discovered must be false".to_string(),
        );
    }
    super::require_non_empty_json_str(
        candidate,
        "operation_family",
        "manual-candidates.json candidate",
    )?;
    super::require_non_empty_json_str(
        candidate,
        "unsafe_operation",
        "manual-candidates.json candidate",
    )?;
    super::require_non_empty_json_str(candidate, "invariant", "manual-candidates.json candidate")?;
    super::require_non_empty_json_str(
        candidate,
        "safe_caller",
        "manual-candidates.json candidate",
    )?;
    super::require_non_empty_json_str(
        candidate,
        "location_text",
        "manual-candidates.json candidate",
    )?;
    super::require_non_empty_json_str(
        candidate,
        "explain_command",
        "manual-candidates.json candidate",
    )?;
    super::require_non_empty_json_str(
        candidate,
        "context_command",
        "manual-candidates.json candidate",
    )?;
    super::require_non_empty_json_str(
        candidate,
        "witness_plan_command",
        "manual-candidates.json candidate",
    )?;
    let boundary = super::require_non_empty_json_str(
        candidate,
        "trust_boundary",
        "manual-candidates.json candidate",
    )?;
    if !super::text_contains_ignore_ascii_case(boundary, "not analyzer-discovered") {
        return Err(
            "manual-candidates.json candidate trust_boundary must say not analyzer-discovered"
                .to_string(),
        );
    }
    Ok(())
}

fn check_review_kit_manifest(
    dir: &Path,
    scope: &str,
    card_count: usize,
    open_actionable_gaps: usize,
    card_ids: &BTreeSet<String>,
) -> Result<(), String> {
    let path = dir.join("review-kit.json");
    let review_kit = super::parse_json_file(&path)?;
    super::require_json_str(&review_kit, "schema_version", "0.1", "review-kit.json")?;
    super::require_json_str(&review_kit, "tool", "unsafe-review", "review-kit.json")?;
    super::require_json_str(
        &review_kit,
        "mode",
        "review_kit_manifest",
        "review-kit.json",
    )?;
    super::require_json_str(&review_kit, "source", "first_pr", "review-kit.json")?;
    super::require_json_str(&review_kit, "policy", "advisory", "review-kit.json")?;
    super::require_json_str(&review_kit, "scope", scope, "review-kit.json")?;
    super::require_non_empty_json_str(&review_kit, "tool_version", "review-kit.json")?;
    let summary_cards = super::json_usize_at(&review_kit, "/summary/cards", "review-kit.json")?;
    if summary_cards != card_count {
        return Err(format!(
            "review-kit.json summary.cards is {summary_cards}, but cards.json has {card_count}"
        ));
    }
    let summary_open = super::json_usize_at(
        &review_kit,
        "/summary/open_actionable_gaps",
        "review-kit.json",
    )?;
    if summary_open != open_actionable_gaps {
        return Err(format!(
            "review-kit.json summary.open_actionable_gaps is {summary_open}, but cards.json has {open_actionable_gaps}"
        ));
    }

    let top_card_id = match review_kit.get("top_card_id") {
        Some(serde_json::Value::String(card_id)) => {
            if !card_ids.contains(card_id) {
                return Err(format!(
                    "review-kit.json top_card_id `{card_id}` is not present in cards.json"
                ));
            }
            Some(card_id.clone())
        }
        Some(serde_json::Value::Null) if card_count == 0 => None,
        Some(serde_json::Value::Null) => {
            return Err(
                "review-kit.json top_card_id must name a card when cards exist".to_string(),
            );
        }
        _ => return Err("review-kit.json top_card_id must be a string or null".to_string()),
    };

    let boundary = review_kit
        .get("trust_boundary")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "review-kit.json is missing trust_boundary".to_string())?;
    super::require_boundary_text(boundary, "review-kit.json")?;
    for expected in [
        "not Miri-clean status",
        "not site-execution proof",
        "did not run witnesses",
        "post comments",
        "edit source",
        "run an agent",
        "enforce blocking policy",
    ] {
        if !super::text_contains_ignore_ascii_case(boundary, expected) {
            return Err(format!(
                "review-kit.json trust_boundary must include `{expected}`"
            ));
        }
    }

    check_review_kit_handoff(&review_kit, top_card_id.as_deref(), card_count)?;

    let artifacts = super::json_array_at(&review_kit, "/artifacts", "review-kit.json")?;
    let mut seen = BTreeSet::new();
    for entry in artifacts {
        let artifact_path =
            super::require_non_empty_json_str(entry, "path", "review-kit.json artifact")?;
        check_review_kit_artifact_path(artifact_path)?;
        if !seen.insert(artifact_path.to_string()) {
            return Err(format!(
                "review-kit.json repeats artifact path `{artifact_path}`"
            ));
        }
        if !dir.join(artifact_path).is_file() {
            return Err(format!(
                "review-kit.json lists missing artifact `{artifact_path}`"
            ));
        }
        require_expected_value(
            super::require_non_empty_json_str(entry, "kind", "review-kit.json artifact")?,
            expected_review_kit_artifact_kind(artifact_path),
            "review-kit.json artifact kind",
        )?;
        require_expected_value(
            super::require_non_empty_json_str(entry, "format", "review-kit.json artifact")?,
            expected_review_kit_artifact_format(artifact_path),
            "review-kit.json artifact format",
        )?;
        check_review_kit_artifact_schema_version(entry, artifact_path)?;
    }

    let expected = FIRST_PR_BUNDLE_ARTIFACTS
        .iter()
        .map(|artifact| artifact.to_string())
        .collect::<BTreeSet<_>>();
    if seen != expected {
        return Err(format!(
            "review-kit.json artifact set must be {:?}; got {:?}",
            expected, seen
        ));
    }
    Ok(())
}

fn check_review_kit_handoff(
    review_kit: &serde_json::Value,
    top_card_id: Option<&str>,
    card_count: usize,
) -> Result<(), String> {
    let handoff = review_kit
        .get("handoff")
        .ok_or_else(|| "review-kit.json is missing handoff".to_string())?;
    if !handoff.is_object() {
        return Err("review-kit.json handoff must be an object".to_string());
    }

    require_expected_value(
        super::require_non_empty_json_str(handoff, "reviewer_summary", "review-kit.json handoff")?,
        "pr-summary.md",
        "review-kit.json handoff reviewer_summary",
    )?;

    let receipt_command = super::require_non_empty_json_str(
        handoff,
        "receipt_audit_markdown",
        "review-kit.json handoff",
    )?;
    if !receipt_command.starts_with("unsafe-review receipt audit ") {
        return Err(
            "review-kit.json handoff receipt_audit_markdown must start with `unsafe-review receipt audit`"
                .to_string(),
        );
    }
    if !receipt_command.contains("--format markdown") {
        return Err(
            "review-kit.json handoff receipt_audit_markdown must include `--format markdown`"
                .to_string(),
        );
    }

    check_review_kit_top_card_handoff(handoff, top_card_id, card_count)?;
    check_review_kit_manual_candidate_handoff(handoff)?;

    let boundary =
        super::require_non_empty_json_str(handoff, "trust_boundary", "review-kit.json handoff")?;
    for expected in [
        "did not run witnesses",
        "run agents",
        "post comments",
        "edit source",
        "blocking policy",
    ] {
        if !super::text_contains_ignore_ascii_case(boundary, expected) {
            return Err(format!(
                "review-kit.json handoff trust_boundary must include `{expected}`"
            ));
        }
    }

    Ok(())
}

fn check_review_kit_manual_candidate_handoff(handoff: &serde_json::Value) -> Result<(), String> {
    let manual = handoff
        .get("manual_candidates")
        .ok_or_else(|| "review-kit.json handoff is missing manual_candidates".to_string())?;
    if !manual.is_object() {
        return Err("review-kit.json handoff manual_candidates must be an object".to_string());
    }
    require_expected_value(
        super::require_non_empty_json_str(
            manual,
            "artifact",
            "review-kit.json handoff manual_candidates",
        )?,
        "manual-candidates.json",
        "review-kit.json handoff manual_candidates artifact",
    )?;
    let count = super::json_usize_at(
        manual,
        "/manual_candidates",
        "review-kit.json handoff manual_candidates",
    )?;
    let analyzer_discovered = super::json_usize_at(
        manual,
        "/analyzer_discovered",
        "review-kit.json handoff manual_candidates",
    )?;
    if analyzer_discovered != 0 {
        return Err(
            "review-kit.json handoff manual_candidates analyzer_discovered must stay 0".to_string(),
        );
    }
    check_review_kit_first_manual_candidate_handoff(manual, count)?;
    let boundary = super::require_non_empty_json_str(
        manual,
        "trust_boundary",
        "review-kit.json handoff manual_candidates",
    )?;
    for expected in [
        "manual/advisory",
        "not analyzer-discovered ReviewCards",
        "not policy inputs",
        "not witness execution",
        "do not import ReviewCard witness evidence",
    ] {
        if !super::text_contains_ignore_ascii_case(boundary, expected) {
            return Err(format!(
                "review-kit.json handoff manual_candidates trust_boundary must include `{expected}`"
            ));
        }
    }
    Ok(())
}

fn check_review_kit_first_manual_candidate_handoff(
    manual: &serde_json::Value,
    count: usize,
) -> Result<(), String> {
    let Some(first_candidate) = manual.get("first_candidate") else {
        return Err(
            "review-kit.json handoff manual_candidates is missing first_candidate".to_string(),
        );
    };
    if count == 0 {
        if first_candidate.is_null() {
            return Ok(());
        }
        return Err(
            "review-kit.json handoff manual_candidates first_candidate must be null when count is 0"
                .to_string(),
        );
    }
    if !first_candidate.is_object() {
        return Err(
            "review-kit.json handoff manual_candidates first_candidate must be an object when candidates exist"
                .to_string(),
        );
    }
    let id = super::require_non_empty_json_str(
        first_candidate,
        "id",
        "review-kit.json handoff manual_candidates first_candidate",
    )?;
    super::require_json_str(
        first_candidate,
        "source",
        "manual",
        "review-kit.json handoff manual_candidates first_candidate",
    )?;
    if first_candidate.get("manual_candidate") != Some(&serde_json::Value::Bool(true)) {
        return Err(
            "review-kit.json handoff manual_candidates first_candidate manual_candidate must be true"
                .to_string(),
        );
    }
    if first_candidate.get("analyzer_discovered") != Some(&serde_json::Value::Bool(false)) {
        return Err(
            "review-kit.json handoff manual_candidates first_candidate analyzer_discovered must be false"
                .to_string(),
        );
    }
    for (field, command) in [
        ("explain", "unsafe-review explain "),
        ("context_json", "unsafe-review context "),
        ("witness_plan", "unsafe-review candidate witness-plan "),
    ] {
        let text = super::require_non_empty_json_str(
            first_candidate,
            field,
            "review-kit.json handoff manual_candidates first_candidate",
        )?;
        if !text.starts_with(command) || !text.contains(id) {
            return Err(format!(
                "review-kit.json handoff manual_candidates first_candidate {field} must reference `{id}`"
            ));
        }
    }
    Ok(())
}

fn check_review_kit_top_card_handoff(
    handoff: &serde_json::Value,
    top_card_id: Option<&str>,
    card_count: usize,
) -> Result<(), String> {
    let Some(top_card) = handoff.get("top_card") else {
        return Err("review-kit.json handoff is missing top_card".to_string());
    };
    if card_count == 0 {
        if top_card.is_null() {
            return Ok(());
        }
        return Err(
            "review-kit.json handoff top_card must be null when no cards exist".to_string(),
        );
    }

    if !top_card.is_object() {
        return Err(
            "review-kit.json handoff top_card must be an object when cards exist".to_string(),
        );
    }
    let handoff_card_id =
        super::require_non_empty_json_str(top_card, "card_id", "review-kit.json handoff top_card")?;
    if Some(handoff_card_id) != top_card_id {
        return Err(format!(
            "review-kit.json handoff top_card card_id `{handoff_card_id}` does not match top_card_id `{}`",
            top_card_id.unwrap_or("<missing>")
        ));
    }

    let explain =
        super::require_non_empty_json_str(top_card, "explain", "review-kit.json handoff top_card")?;
    if !explain.starts_with("unsafe-review explain ") || !explain.contains(handoff_card_id) {
        return Err(format!(
            "review-kit.json handoff top_card explain must reference `{handoff_card_id}`"
        ));
    }

    let context_json = super::require_non_empty_json_str(
        top_card,
        "context_json",
        "review-kit.json handoff top_card",
    )?;
    if !context_json.starts_with("unsafe-review context ")
        || !context_json.contains(handoff_card_id)
        || !context_json.contains("--json")
    {
        return Err(format!(
            "review-kit.json handoff top_card context_json must reference `{handoff_card_id}` and include `--json`"
        ));
    }

    Ok(())
}

fn check_review_kit_artifact_path(path: &str) -> Result<(), String> {
    let artifact = Path::new(path);
    if artifact.is_absolute() {
        return Err(format!(
            "review-kit.json artifact path `{path}` must be relative"
        ));
    }
    if artifact.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(format!(
            "review-kit.json artifact path `{path}` must not escape the artifact directory"
        ));
    }
    Ok(())
}

fn expected_review_kit_artifact_kind(path: &str) -> &'static str {
    match path {
        "review-kit.json" => "review_kit_manifest",
        "cards.json" => "review_cards",
        "pr-summary.md" => "reviewer_summary",
        "github-summary.md" => "github_summary",
        "cards.sarif" => "sarif",
        "comment-plan.json" => "comment_plan",
        "witness-plan.md" => "witness_plan",
        "receipt-audit.md" => "receipt_audit",
        "manual-candidates.json" => "manual_candidates",
        "lsp.json" => "saved_lsp",
        "repair-queue.json" => "repair_queue",
        _ => "unknown",
    }
}

fn expected_review_kit_artifact_format(path: &str) -> &'static str {
    match path {
        "review-kit.json"
        | "cards.json"
        | "comment-plan.json"
        | "lsp.json"
        | "repair-queue.json"
        | "manual-candidates.json" => "json",
        "pr-summary.md" | "github-summary.md" | "witness-plan.md" | "receipt-audit.md" => {
            "markdown"
        }
        "cards.sarif" => "sarif",
        _ => "unknown",
    }
}

fn check_review_kit_artifact_schema_version(
    entry: &serde_json::Value,
    path: &str,
) -> Result<(), String> {
    let Some(schema_version) = entry.get("schema_version") else {
        return Err(format!(
            "review-kit.json artifact `{path}` is missing schema_version"
        ));
    };
    let expected = match path {
        "review-kit.json" | "cards.json" | "comment-plan.json" | "lsp.json"
        | "repair-queue.json" => Some("0.1"),
        "manual-candidates.json" => Some("manual-candidates/v1"),
        "cards.sarif" => Some("2.1.0"),
        "pr-summary.md" | "github-summary.md" | "witness-plan.md" | "receipt-audit.md" => None,
        _ => {
            return Err(format!("review-kit.json artifact `{path}` is unknown"));
        }
    };
    match expected {
        Some(expected) => {
            let Some(actual) = schema_version.as_str() else {
                return Err(format!(
                    "review-kit.json artifact `{path}` schema_version must be `{expected}`"
                ));
            };
            require_expected_value(
                actual,
                expected,
                &format!("review-kit.json artifact `{path}` schema_version"),
            )
        }
        None if schema_version.is_null() => Ok(()),
        None => Err(format!(
            "review-kit.json artifact `{path}` schema_version must be null for unversioned markdown"
        )),
    }
}

fn require_text_mentions_all_card_ids(
    text: &str,
    path: &Path,
    card_ids: &BTreeSet<String>,
) -> Result<(), String> {
    for card_id in card_ids {
        if !text.contains(card_id) {
            return Err(format!(
                "{} must mention ReviewCard id `{card_id}`",
                path.display()
            ));
        }
    }
    Ok(())
}

fn require_text_mentions_only_known_card_ids(
    text: &str,
    path: &Path,
    card_ids: &BTreeSet<String>,
) -> Result<(), String> {
    for card_id in markdown_review_card_ids(text) {
        if !card_ids.contains(&card_id) {
            return Err(format!(
                "{} mentions unknown ReviewCard id `{card_id}`",
                path.display()
            ));
        }
    }
    Ok(())
}

fn markdown_review_card_ids(text: &str) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            continue;
        }
        let mut rest = line;
        while let Some(start) = rest.find('`') {
            let after_start = &rest[start + 1..];
            let Some(end) = after_start.find('`') else {
                break;
            };
            let candidate = &after_start[..end];
            if looks_like_markdown_card_id(candidate) {
                ids.insert(candidate.to_string());
            }
            rest = &after_start[end + 1..];
        }
    }
    ids
}

fn looks_like_markdown_card_id(value: &str) -> bool {
    (value.starts_with("UR-") || value.starts_with("card-"))
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.')
}

fn require_witness_plan_headings_known(
    text: &str,
    path: &Path,
    card_ids: &BTreeSet<String>,
) -> Result<(), String> {
    for line in text.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("#### `") else {
            continue;
        };
        let Some((card_id, suffix)) = rest.split_once('`') else {
            return Err(format!(
                "{} witness-plan route heading must close its ReviewCard id backtick",
                path.display()
            ));
        };
        if !suffix.trim().is_empty() {
            return Err(format!(
                "{} witness-plan route heading for `{card_id}` must contain only a ReviewCard id",
                path.display()
            ));
        }
        if !card_ids.contains(card_id) {
            return Err(format!(
                "{} witness-plan route heading references unknown card id `{card_id}`",
                path.display()
            ));
        }
    }
    Ok(())
}

fn require_markdown_top_card_projection(
    text: &str,
    path: &Path,
    card_projections: &BTreeMap<String, CardProjection>,
) -> Result<(), String> {
    if card_projections.is_empty() {
        return Ok(());
    }

    let mut top_card_id = None;
    let mut top_card_class = None;
    let mut top_card_location = None;
    let mut top_card_operation = None;
    let mut top_card_operation_family = None;
    let mut top_card_missing_evidence = None;
    let mut top_card_primary_route = None;
    let mut top_card_next_action = None;
    let mut top_card_explain_command = None;
    let mut top_card_agent_context_command = None;

    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed
            .strip_prefix("- ID: `")
            .or_else(|| trimmed.strip_prefix("- Top card: `"))
        {
            let Some((card_id, _)) = rest.split_once('`') else {
                continue;
            };
            if !card_projections.contains_key(card_id) {
                return Err(format!(
                    "{} top card id `{card_id}` is not present in cards.json",
                    path.display()
                ));
            }
            top_card_id = Some(card_id.to_string());
        } else if let Some(rest) = trimmed.strip_prefix("- Class: `") {
            let Some((class_name, _)) = rest.split_once('`') else {
                continue;
            };
            top_card_class = Some(class_name.to_string());
        } else if let Some(location) = trimmed.strip_prefix("- Location: ") {
            top_card_location = Some(location.to_string());
        } else if let Some(rest) = trimmed.strip_prefix("- Operation: `") {
            let Some((operation, _)) = rest.split_once('`') else {
                continue;
            };
            top_card_operation = Some(operation.to_string());
        } else if let Some(rest) = trimmed.strip_prefix("- Operation family: `") {
            let Some((operation_family, _)) = rest.split_once('`') else {
                continue;
            };
            top_card_operation_family = Some(operation_family.to_string());
        } else if let Some(missing_evidence) = trimmed
            .strip_prefix("- Missing evidence: ")
            .or_else(|| trimmed.strip_prefix("- Missing/weak evidence: "))
        {
            top_card_missing_evidence = Some(missing_evidence.to_string());
        } else if let Some(rest) = trimmed
            .strip_prefix("- Primary route: `")
            .or_else(|| trimmed.strip_prefix("- Witness route: `"))
        {
            let Some((route_kind, after_kind)) = rest.split_once('`') else {
                continue;
            };
            let Some(route_reason) = after_kind.strip_prefix(" because ") else {
                continue;
            };
            top_card_primary_route = Some((route_kind.to_string(), route_reason.to_string()));
        } else if let Some(next_action) = trimmed
            .strip_prefix("- Next action: ")
            .or_else(|| trimmed.strip_prefix("- Next reviewer action: "))
        {
            top_card_next_action = Some(next_action.to_string());
        } else if let Some(rest) = trimmed.strip_prefix("- Explain: `") {
            let Some((command, _)) = rest.split_once('`') else {
                continue;
            };
            top_card_explain_command = Some(command.to_string());
        } else if let Some(rest) = trimmed.strip_prefix("- Agent context: `") {
            let Some((command, _)) = rest.split_once('`') else {
                continue;
            };
            top_card_agent_context_command = Some(command.to_string());
        }
    }

    let Some(card_id) = top_card_id else {
        return Err(format!(
            "{} must include a top ReviewCard id line",
            path.display()
        ));
    };
    let card = card_projections.get(&card_id).ok_or_else(|| {
        format!(
            "{} top card id `{card_id}` is not present in cards.json",
            path.display()
        )
    })?;

    let Some(actual_class) = top_card_class else {
        return Err(format!(
            "{} must include a top ReviewCard class line",
            path.display()
        ));
    };
    require_expected_value(
        &actual_class,
        &card.class_name,
        &format!("{} top card `{card_id}` class", path.display()),
    )?;

    let Some(actual_location) = top_card_location else {
        return Err(format!(
            "{} must include a top ReviewCard location line",
            path.display()
        ));
    };
    let expected_location = format!("{}:{}", card.path, card.line);
    require_expected_value(
        &actual_location,
        &expected_location,
        &format!("{} top card `{card_id}` location", path.display()),
    )?;

    let Some(actual_operation) = top_card_operation else {
        return Err(format!(
            "{} must include a top ReviewCard operation line",
            path.display()
        ));
    };
    require_expected_value(
        &actual_operation,
        &card.operation,
        &format!("{} top card `{card_id}` operation", path.display()),
    )?;

    let Some(actual_operation_family) = top_card_operation_family else {
        return Err(format!(
            "{} must include a top ReviewCard operation family line",
            path.display()
        ));
    };
    require_expected_value(
        &actual_operation_family,
        &card.operation_family,
        &format!("{} top card `{card_id}` operation family", path.display()),
    )?;

    let Some(actual_missing_evidence) = top_card_missing_evidence else {
        return Err(format!(
            "{} must include a top ReviewCard missing evidence line",
            path.display()
        ));
    };
    require_expected_value(
        &actual_missing_evidence,
        &expected_missing_summary(card),
        &format!("{} top card `{card_id}` missing evidence", path.display()),
    )?;

    if let Some(expected_route) = card.witness_routes.first() {
        let Some((actual_route_kind, actual_route_reason)) = top_card_primary_route else {
            return Err(format!(
                "{} must include a top ReviewCard primary route line",
                path.display()
            ));
        };
        require_expected_value(
            &actual_route_kind,
            &expected_route.kind,
            &format!("{} top card `{card_id}` primary route kind", path.display()),
        )?;
        require_expected_value(
            &actual_route_reason,
            &expected_route.reason,
            &format!(
                "{} top card `{card_id}` primary route reason",
                path.display()
            ),
        )?;
        if let Some(command) = &expected_route.command {
            require_top_card_primary_route_command(
                text,
                path,
                &card_id,
                &expected_route.kind,
                &expected_route.reason,
                command,
            )?;
        }
    }

    let Some(actual_next_action) = top_card_next_action else {
        return Err(format!(
            "{} must include a top ReviewCard next action line",
            path.display()
        ));
    };
    require_expected_value(
        &actual_next_action,
        &card.next_action,
        &format!("{} top card `{card_id}` next action", path.display()),
    )?;

    let Some(actual_explain_command) = top_card_explain_command else {
        return Err(format!(
            "{} must include a top ReviewCard explain command line",
            path.display()
        ));
    };
    require_expected_value(
        &actual_explain_command,
        &format!("unsafe-review explain {card_id}"),
        &format!("{} top card `{card_id}` explain command", path.display()),
    )?;

    let Some(actual_agent_context_command) = top_card_agent_context_command else {
        return Err(format!(
            "{} must include a top ReviewCard agent context command line",
            path.display()
        ));
    };
    require_expected_value(
        &actual_agent_context_command,
        &format!("unsafe-review context {card_id} --json"),
        &format!(
            "{} top card `{card_id}` agent context command",
            path.display()
        ),
    )
}

fn expected_missing_summary(card: &CardProjection) -> String {
    if card.missing.is_empty() {
        "No missing evidence recorded".to_string()
    } else {
        card.missing.join("; ")
    }
}

fn require_top_card_primary_route_command(
    text: &str,
    path: &Path,
    card_id: &str,
    route_kind: &str,
    route_reason: &str,
    command: &str,
) -> Result<(), String> {
    let expected = format!(
        "- Primary route: `{route_kind}` because {route_reason}\n\n```bash\n{command}\n```"
    );
    let expected_front_panel = format!(
        "- Witness route: `{route_kind}` because {route_reason}\n  - Suggested command:\n\n```bash\n{command}\n```"
    );
    if text.contains(&expected) || text.contains(&expected_front_panel) {
        Ok(())
    } else {
        Err(format!(
            "{} top card `{card_id}` primary route command must include fenced command `{command}`",
            path.display()
        ))
    }
}

fn check_advisory_artifact_set(dir: &Path) -> Result<AdvisoryArtifactSummary, String> {
    let manifest = check_cards_json_artifact(dir)?;
    check_pr_summary_artifact(dir, &manifest)?;
    check_sarif_artifact(dir, &manifest)?;
    check_comment_plan_artifact(dir, &manifest)?;
    let repair_queue_projections = check_repair_queue_artifact(
        dir,
        manifest.card_count,
        &manifest.card_ids,
        &manifest.card_projections,
    )?;

    Ok(AdvisoryArtifactSummary {
        card_ids: manifest.card_ids,
        card_projections: manifest.card_projections,
        repair_queue_projections,
        scope: manifest.scope,
        card_count: manifest.card_count,
        open_actionable_gaps: manifest.open_actionable_gaps,
        high_priority_cards: manifest.high_priority_cards,
    })
}

fn check_cards_json_artifact(dir: &Path) -> Result<AdvisoryArtifactManifest, String> {
    if !dir.is_dir() {
        return Err(format!(
            "advisory artifact directory missing: {}",
            dir.display()
        ));
    }

    let cards = super::parse_json_file(&dir.join("cards.json"))?;
    super::require_json_str(&cards, "schema_version", "0.1", "cards.json")?;
    super::require_json_str(&cards, "tool", "unsafe-review", "cards.json")?;
    super::require_json_str(&cards, "policy", "advisory", "cards.json")?;
    super::require_json_array(&cards, "cards", "cards.json")?;
    let cards_boundary = cards
        .get("trust_boundary")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "cards.json is missing trust_boundary".to_string())?;
    super::require_boundary_text(cards_boundary, "cards.json")?;
    let scope = super::require_non_empty_json_str(&cards, "scope", "cards.json")?.to_string();
    require_known_advisory_scope(&scope)?;
    let card_ids = super::advisory_card_ids(&cards)?;
    let card_projections = advisory_card_projections(&cards)?;
    let card_count = card_ids.len();
    let summary_cards = super::json_usize_at(&cards, "/summary/cards", "cards.json")?;
    let open_actionable_gaps =
        super::json_usize_at(&cards, "/summary/open_actionable_gaps", "cards.json")?;
    if summary_cards != card_count {
        return Err(format!(
            "cards.json summary.cards is {summary_cards}, but cards array has {card_count}"
        ));
    }
    let high_priority_cards = card_projections
        .values()
        .filter(|card| card.priority == "high")
        .count();

    Ok(AdvisoryArtifactManifest {
        card_ids,
        card_projections,
        scope,
        card_count,
        open_actionable_gaps,
        high_priority_cards,
    })
}

fn check_pr_summary_artifact(
    dir: &Path,
    manifest: &AdvisoryArtifactManifest,
) -> Result<(), String> {
    let scope = &manifest.scope;
    let card_count = manifest.card_count;
    let open_actionable_gaps = manifest.open_actionable_gaps;
    let pr_summary_path = dir.join("pr-summary.md");
    let pr_summary = super::read_to_string(&pr_summary_path)?;
    super::require_text_contains(
        &pr_summary,
        &format!("- Scope: `{scope}`"),
        &pr_summary_path,
    )?;
    super::require_text_contains(
        &pr_summary,
        &format!("- Review cards: {card_count}"),
        &pr_summary_path,
    )?;
    super::require_text_contains(
        &pr_summary,
        &format!("- Open actionable gaps: {open_actionable_gaps}"),
        &pr_summary_path,
    )?;
    super::require_text_contains(&pr_summary, "- Policy mode: `advisory`", &pr_summary_path)?;
    super::require_text_contains(
        &pr_summary,
        "static unsafe contract review",
        &pr_summary_path,
    )?;
    super::require_text_contains(
        &pr_summary,
        "not a proof of memory safety",
        &pr_summary_path,
    )?;
    super::require_text_contains(&pr_summary, "not UB-free status", &pr_summary_path)?;
    super::require_text_contains(&pr_summary, "not a Miri result", &pr_summary_path)?;
    super::require_text_contains(
        &pr_summary,
        "- Receipt audit: `receipt-audit.md` checks saved receipt metadata only; no witness was run.",
        &pr_summary_path,
    )?;
    if card_count == 0 {
        super::require_text_contains(
            &pr_summary,
            "No changed unsafe-review gaps were found.",
            &pr_summary_path,
        )?;
        super::require_text_contains(&pr_summary, "unsafe site executed", &pr_summary_path)?;
    }
    Ok(())
}

fn check_sarif_artifact(dir: &Path, manifest: &AdvisoryArtifactManifest) -> Result<(), String> {
    let scope = manifest.scope.as_str();
    let card_ids = &manifest.card_ids;
    let card_projections = &manifest.card_projections;
    let card_count = manifest.card_count;
    let sarif = super::parse_json_file(&dir.join("cards.sarif"))?;
    super::require_json_str(&sarif, "version", "2.1.0", "cards.sarif")?;
    super::require_json_array(&sarif, "runs", "cards.sarif")?;
    let sarif_rule_ids = sarif_rule_ids(&sarif)?;
    let card_class_names = card_projections
        .values()
        .map(|projection| projection.class_name.as_str())
        .collect::<BTreeSet<_>>();
    for class_name in &card_class_names {
        if !sarif_rule_ids.contains(class_name) {
            return Err(format!(
                "cards.sarif is missing rule id `{class_name}` for cards.json class"
            ));
        }
    }
    for rule_id in &sarif_rule_ids {
        if !card_class_names.contains(rule_id) {
            return Err(format!(
                "cards.sarif declares unused rule id `{rule_id}` not present in cards.json classes"
            ));
        }
    }
    let sarif_results = super::json_array_at(&sarif, "/runs/0/results", "cards.sarif")?;
    if sarif_results.len() != card_count {
        return Err(format!(
            "cards.sarif has {} result(s), but cards.json has {card_count} card(s)",
            sarif_results.len()
        ));
    }
    let mut sarif_card_ids = BTreeSet::new();
    for result in sarif_results {
        let card_id =
            check_sarif_result_projection(result, &sarif_rule_ids, card_ids, card_projections)?;
        if !sarif_card_ids.insert(card_id.to_string()) {
            return Err(format!("cards.sarif results repeat card id `{card_id}`"));
        }
    }
    for card_id in card_ids {
        if !sarif_card_ids.contains(card_id) {
            return Err(format!("cards.sarif results missing card id `{card_id}`"));
        }
    }
    let sarif_boundary = sarif
        .pointer("/runs/0/properties/trustBoundary")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "cards.sarif is missing /runs/0/properties/trustBoundary".to_string())?;
    super::require_boundary_text(sarif_boundary, "cards.sarif")?;
    let sarif_scope = sarif
        .pointer("/runs/0/properties/scope")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "cards.sarif is missing /runs/0/properties/scope".to_string())?;
    require_expected_value(sarif_scope, scope, "cards.sarif /runs/0/properties/scope")?;
    Ok(())
}

fn check_sarif_result_projection<'a>(
    result: &'a serde_json::Value,
    sarif_rule_ids: &BTreeSet<&str>,
    card_ids: &BTreeSet<String>,
    card_projections: &BTreeMap<String, CardProjection>,
) -> Result<&'a str, String> {
    let Some(card_id) = result
        .pointer("/properties/cardId")
        .and_then(serde_json::Value::as_str)
    else {
        return Err("cards.sarif result is missing properties.cardId".to_string());
    };
    if !card_ids.contains(card_id) {
        return Err(format!(
            "cards.sarif result references unknown card id `{card_id}`"
        ));
    }
    let Some(card_projection) = card_projections.get(card_id) else {
        return Err(format!(
            "cards.sarif result references unknown card id `{card_id}`"
        ));
    };
    let rule_id = super::require_non_empty_json_str(result, "ruleId", "cards.sarif result")?;
    require_expected_value(
        rule_id,
        &card_projection.class_name,
        "cards.sarif result ruleId",
    )?;
    if !sarif_rule_ids.contains(rule_id) {
        return Err(format!(
            "cards.sarif result ruleId `{rule_id}` is not declared in tool.driver.rules"
        ));
    }
    require_projected_str(
        result
            .pointer("/properties")
            .ok_or_else(|| "cards.sarif result is missing properties".to_string())?,
        "class",
        &card_projection.class_name,
        "cards.sarif result properties",
    )?;
    let properties = result
        .pointer("/properties")
        .ok_or_else(|| "cards.sarif result is missing properties".to_string())?;
    require_sarif_location_projection(result, card_projection)?;
    require_projected_str(
        properties,
        "priority",
        &card_projection.priority,
        "cards.sarif result properties",
    )?;
    require_projected_str(
        properties,
        "confidence",
        &card_projection.confidence,
        "cards.sarif result properties",
    )?;
    require_projected_str(
        properties,
        "operationFamily",
        &card_projection.operation_family,
        "cards.sarif result properties",
    )?;
    require_projected_str(
        properties,
        "operation",
        &card_projection.operation,
        "cards.sarif result properties",
    )?;
    require_projected_str(
        properties,
        "nextAction",
        &card_projection.next_action,
        "cards.sarif result properties",
    )?;
    require_projected_string_array(
        properties,
        "verifyCommands",
        &card_projection.verify_commands,
        "cards.sarif result properties",
    )?;
    require_projected_witness_routes_field(
        properties,
        "witnessRouteDetails",
        &card_projection.witness_routes,
        "cards.sarif result properties",
    )?;
    require_projected_string_array(
        properties,
        "witnessRoutes",
        &witness_route_summaries(&card_projection.witness_routes),
        "cards.sarif result properties",
    )?;
    require_projected_string_array(
        properties,
        "hazards",
        &card_projection.hazards,
        "cards.sarif result properties",
    )?;
    require_projected_string_array(
        properties,
        "missingEvidence",
        &card_projection.missing,
        "cards.sarif result properties",
    )?;
    let result_boundary = properties
        .get("trustBoundary")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "cards.sarif result properties is missing trustBoundary".to_string())?;
    super::require_boundary_text(result_boundary, "cards.sarif result properties")?;
    super::json_array_at(result, "/properties/verifyCommands", "cards.sarif result")?;
    Ok(card_id)
}

fn check_comment_plan_artifact(
    dir: &Path,
    manifest: &AdvisoryArtifactManifest,
) -> Result<(), String> {
    let card_ids = &manifest.card_ids;
    let card_projections = &manifest.card_projections;
    let card_count = manifest.card_count;
    let comment_plan_path = dir.join("comment-plan.json");
    let comment_plan = super::parse_json_file(&comment_plan_path)?;
    super::require_json_str(&comment_plan, "schema_version", "0.1", "comment-plan.json")?;
    super::require_json_str(&comment_plan, "mode", "plan_only", "comment-plan.json")?;
    super::require_json_str(&comment_plan, "policy", "advisory", "comment-plan.json")?;
    super::require_json_array(&comment_plan, "comments", "comment-plan.json")?;
    let comments = super::json_array_at(&comment_plan, "/comments", "comment-plan.json")?;
    if comments.len() > 3 {
        return Err(format!(
            "comment-plan.json has {} comment(s), expected at most 3",
            comments.len()
        ));
    }
    let mut comment_card_ids = BTreeSet::new();
    let mut comment_locations = BTreeSet::new();
    let mut comment_budget_keys = BTreeSet::new();
    let mut comment_body_projections = Vec::new();
    for comment in comments {
        let Some(card_id) = comment.get("card_id").and_then(serde_json::Value::as_str) else {
            return Err("comment-plan.json comment is missing card_id".to_string());
        };
        let Some(card_projection) = card_projections.get(card_id) else {
            return Err(format!(
                "comment-plan.json references unknown card id `{card_id}`"
            ));
        };
        if !comment_card_ids.insert(card_id.to_string()) {
            return Err(format!(
                "comment-plan.json repeats card id `{card_id}` in planned comments"
            ));
        }
        let Some(path) = comment.get("path").and_then(serde_json::Value::as_str) else {
            return Err("comment-plan.json comment is missing path".to_string());
        };
        if path.trim().is_empty() {
            return Err("comment-plan.json comment path must not be empty".to_string());
        }
        let Some(line) = comment.get("line").and_then(serde_json::Value::as_u64) else {
            return Err("comment-plan.json comment is missing line".to_string());
        };
        if line == 0 {
            return Err("comment-plan.json comment line must be one-based".to_string());
        }
        let Some(changed_line) = comment
            .get("changed_line")
            .and_then(serde_json::Value::as_bool)
        else {
            return Err("comment-plan.json comment is missing changed_line".to_string());
        };
        if !changed_line {
            return Err(
                "comment-plan.json planned comments must have changed_line=true".to_string(),
            );
        }
        require_comment_card_projection(comment, card_projection, "comment-plan.json comment")?;
        let location_key = (path.to_string(), line);
        if !comment_locations.insert(location_key) {
            return Err(format!(
                "comment-plan.json repeats inline location `{path}:{line}` in planned comments"
            ));
        }
        super::json_array_at(comment, "/witness_routes", "comment-plan.json comment")?;
        super::json_array_at(comment, "/verify_commands", "comment-plan.json comment")?;
        let Some(body) = comment.get("body").and_then(serde_json::Value::as_str) else {
            return Err("comment-plan.json comment is missing body".to_string());
        };
        require_text_mentions_only_known_card_ids(body, &comment_plan_path, card_ids)?;
        require_comment_body_boundary(body)?;
        let body_word_count = body.split_whitespace().count();
        if body_word_count > COMMENT_PLAN_BODY_WORD_LIMIT {
            return Err(format!(
                "comment-plan.json comment body has {body_word_count} word(s), expected at most {COMMENT_PLAN_BODY_WORD_LIMIT}"
            ));
        }
        let class_name =
            super::require_non_empty_json_str(comment, "class", "comment-plan.json comment")?;
        if !should_project_planned_comment(card_projection) {
            return Err(format!(
                "comment-plan.json planned comment `{card_id}` is not eligible under the current inline comment policy"
            ));
        }
        if matches!(
            class_name,
            "static_unknown" | "baseline_known" | "suppressed"
        ) {
            return Err(format!(
                "comment-plan.json comment class `{class_name}` must not be selected for inline comments"
            ));
        }
        super::require_non_empty_json_str(comment, "priority", "comment-plan.json comment")?;
        super::require_non_empty_json_str(comment, "confidence", "comment-plan.json comment")?;
        super::require_non_empty_json_str(comment, "operation", "comment-plan.json comment")?;
        super::require_non_empty_json_str(
            comment,
            "operation_family",
            "comment-plan.json comment",
        )?;
        let budget_key = comment_budget_key(card_projection);
        if !comment_budget_keys.insert(budget_key.clone()) {
            return Err(format!(
                "comment-plan.json repeats operation family and obligation budget key `{budget_key}` in planned comments"
            ));
        }
        let next_action =
            super::require_non_empty_json_str(comment, "next_action", "comment-plan.json comment")?;
        let selection_reason = super::require_non_empty_json_str(
            comment,
            "selection_reason",
            "comment-plan.json comment",
        )?;
        require_allowed_value(
            selection_reason,
            COMMENT_PLAN_SELECTION_REASONS,
            "comment-plan.json comment selection_reason",
        )?;
        require_expected_value(
            selection_reason,
            expected_selection_reason(card_projection),
            "comment-plan.json comment selection_reason",
        )?;
        let selection_reason_code = super::require_non_empty_json_str(
            comment,
            "selection_reason_code",
            "comment-plan.json comment",
        )?;
        require_allowed_value(
            selection_reason_code,
            COMMENT_PLAN_SELECTION_REASON_CODES,
            "comment-plan.json comment selection_reason_code",
        )?;
        require_expected_value(
            selection_reason_code,
            expected_selection_reason_code(card_projection),
            "comment-plan.json comment selection_reason_code",
        )?;
        let actionability = super::require_non_empty_json_str(
            comment,
            "actionability",
            "comment-plan.json comment",
        )?;
        require_expected_value(
            actionability,
            expected_actionability(&card_projection.class_name),
            "comment-plan.json comment actionability",
        )?;
        let relevance =
            super::require_non_empty_json_str(comment, "relevance", "comment-plan.json comment")?;
        require_relevance_value(relevance, "comment-plan.json comment")?;
        require_expected_value(
            relevance,
            expected_relevance(card_projection),
            "comment-plan.json comment relevance",
        )?;
        let comment_boundary = comment
            .get("trust_boundary")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "comment-plan.json comment is missing trust_boundary".to_string())?;
        super::require_boundary_text(comment_boundary, "comment-plan.json comment")?;
        if !body.contains(next_action) {
            return Err(
                "comment-plan.json comment body must include the structured next_action"
                    .to_string(),
            );
        }
        comment_body_projections.push((body, card_projection));
    }
    let mut not_selected_card_ids = BTreeSet::new();
    if let Some(not_selected) = comment_plan.get("not_selected") {
        let Some(not_selected) = not_selected.as_array() else {
            return Err("comment-plan.json not_selected must be an array".to_string());
        };
        for card in not_selected {
            let Some(card_id) = card.get("card_id").and_then(serde_json::Value::as_str) else {
                return Err("comment-plan.json not_selected entry is missing card_id".to_string());
            };
            let Some(card_projection) = card_projections.get(card_id) else {
                return Err(format!(
                    "comment-plan.json not_selected references unknown card id `{card_id}`"
                ));
            };
            if comment_card_ids.contains(card_id) {
                return Err(format!(
                    "comment-plan.json not_selected repeats planned comment card id `{card_id}`"
                ));
            }
            if !not_selected_card_ids.insert(card_id.to_string()) {
                return Err(format!(
                    "comment-plan.json not_selected repeats card id `{card_id}`"
                ));
            }
            let Some(path) = card.get("path").and_then(serde_json::Value::as_str) else {
                return Err("comment-plan.json not_selected entry is missing path".to_string());
            };
            if path.trim().is_empty() {
                return Err("comment-plan.json not_selected path must not be empty".to_string());
            }
            let Some(line) = card.get("line").and_then(serde_json::Value::as_u64) else {
                return Err("comment-plan.json not_selected entry is missing line".to_string());
            };
            if line == 0 {
                return Err("comment-plan.json not_selected line must be one-based".to_string());
            }
            let Some(changed_line) = card
                .get("changed_line")
                .and_then(serde_json::Value::as_bool)
            else {
                return Err(
                    "comment-plan.json not_selected entry is missing changed_line".to_string(),
                );
            };
            require_not_selected_card_projection(
                card,
                card_projection,
                "comment-plan.json not_selected",
            )?;
            let actionability = super::require_non_empty_json_str(
                card,
                "actionability",
                "comment-plan.json not_selected",
            )?;
            require_expected_value(
                actionability,
                expected_actionability(&card_projection.class_name),
                "comment-plan.json not_selected actionability",
            )?;
            let relevance = super::require_non_empty_json_str(
                card,
                "relevance",
                "comment-plan.json not_selected",
            )?;
            require_relevance_value(relevance, "comment-plan.json not_selected")?;
            require_expected_value(
                relevance,
                expected_relevance(card_projection),
                "comment-plan.json not_selected relevance",
            )?;
            let reason = super::require_non_empty_json_str(
                card,
                "reason",
                "comment-plan.json not_selected",
            )?;
            require_allowed_value(
                reason,
                COMMENT_PLAN_NON_SELECTION_REASONS,
                "comment-plan.json not_selected reason",
            )?;
            require_expected_value(
                reason,
                expected_non_selection_reason(
                    card_projection,
                    comments.len(),
                    &comment_budget_keys,
                    changed_line,
                ),
                "comment-plan.json not_selected reason",
            )?;
            let reason_code = super::require_non_empty_json_str(
                card,
                "reason_code",
                "comment-plan.json not_selected",
            )?;
            require_allowed_value(
                reason_code,
                COMMENT_PLAN_NON_SELECTION_REASON_CODES,
                "comment-plan.json not_selected reason_code",
            )?;
            require_expected_value(
                reason_code,
                expected_non_selection_reason_code(
                    card_projection,
                    comments.len(),
                    &comment_budget_keys,
                    changed_line,
                ),
                "comment-plan.json not_selected reason_code",
            )?;
        }
    }
    for card_id in card_ids {
        if !comment_card_ids.contains(card_id) && !not_selected_card_ids.contains(card_id) {
            return Err(format!(
                "comment-plan.json must account for ReviewCard id `{card_id}` in comments[] or not_selected[]"
            ));
        }
    }
    for (body, card_projection) in comment_body_projections {
        require_comment_body_card_projection(body, card_projection, "comment-plan.json comment")?;
    }
    let comment_boundary = comment_plan
        .get("trust_boundary")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "comment-plan.json is missing trust_boundary".to_string())?;
    super::require_boundary_text(comment_boundary, "comment-plan.json")?;
    if card_count == 0 {
        let no_changed = comment_plan
            .get("no_changed_gaps")
            .ok_or_else(|| "comment-plan.json is missing no_changed_gaps".to_string())?;
        super::require_json_str(
            no_changed,
            "message",
            "No changed unsafe-review gaps were found.",
            "comment-plan.json no_changed_gaps",
        )?;
        let limitation = super::require_non_empty_json_str(
            no_changed,
            "limitation",
            "comment-plan.json no_changed_gaps",
        )?;
        if !super::text_contains_ignore_ascii_case(limitation, "unsafe site executed") {
            return Err(
                "comment-plan.json no_changed_gaps.limitation must mention unsafe site execution"
                    .to_string(),
            );
        }
    }
    require_comment_plan_summary(&comment_plan, comments.len(), not_selected_card_ids.len())?;
    Ok(())
}

fn require_comment_plan_summary(
    comment_plan: &serde_json::Value,
    selected_count: usize,
    not_selected_count: usize,
) -> Result<(), String> {
    let summary = comment_plan
        .get("summary")
        .ok_or_else(|| "comment-plan.json is missing summary".to_string())?;
    if !summary.is_object() {
        return Err("comment-plan.json summary must be an object".to_string());
    }
    let actual_selected =
        super::json_usize_at(comment_plan, "/summary/selected_count", "comment-plan.json")?;
    if actual_selected != selected_count {
        return Err(format!(
            "comment-plan.json summary.selected_count is {actual_selected}, but comments[] has {selected_count} entrie(s)"
        ));
    }
    let actual_not_selected = super::json_usize_at(
        comment_plan,
        "/summary/not_selected_count",
        "comment-plan.json",
    )?;
    if actual_not_selected != not_selected_count {
        return Err(format!(
            "comment-plan.json summary.not_selected_count is {actual_not_selected}, but not_selected[] has {not_selected_count} entrie(s)"
        ));
    }
    let budget = super::json_usize_at(comment_plan, "/summary/budget", "comment-plan.json")?;
    if budget != COMMENT_PLAN_REVIEW_BUDGET {
        return Err(format!(
            "comment-plan.json summary.budget is {budget}, expected {COMMENT_PLAN_REVIEW_BUDGET}"
        ));
    }
    let reason = super::require_non_empty_json_str(summary, "reason", "comment-plan.json summary")?;
    require_expected_value(
        reason,
        COMMENT_PLAN_REVIEW_BUDGET_REASON,
        "comment-plan.json summary reason",
    )?;
    let reason_code =
        super::require_non_empty_json_str(summary, "reason_code", "comment-plan.json summary")?;
    require_expected_value(
        reason_code,
        COMMENT_PLAN_REVIEW_BUDGET_REASON_CODE,
        "comment-plan.json summary reason_code",
    )?;
    Ok(())
}

fn check_repair_queue_artifact(
    dir: &Path,
    card_count: usize,
    card_ids: &BTreeSet<String>,
    card_projections: &BTreeMap<String, CardProjection>,
) -> Result<BTreeMap<String, RepairQueueProjection>, String> {
    let path = dir.join("repair-queue.json");
    let repair_queue = super::parse_json_file(&path)?;
    super::require_json_str(&repair_queue, "schema_version", "0.1", "repair-queue.json")?;
    super::require_json_str(
        &repair_queue,
        "mode",
        "aggregate_repair_queue",
        "repair-queue.json",
    )?;
    super::require_json_str(&repair_queue, "tool", "unsafe-review", "repair-queue.json")?;
    super::require_json_str(&repair_queue, "source", "review_card", "repair-queue.json")?;
    super::require_json_str(&repair_queue, "policy", "advisory", "repair-queue.json")?;
    let boundary = repair_queue
        .get("trust_boundary")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "repair-queue.json is missing trust_boundary".to_string())?;
    check_repair_queue_trust_boundary(boundary, "repair-queue.json")?;

    let summary_cards = super::json_usize_at(&repair_queue, "/summary/cards", "repair-queue.json")?;
    if summary_cards != card_count {
        return Err(format!(
            "repair-queue.json summary.cards is {summary_cards}, but cards.json has {card_count}"
        ));
    }

    let buckets = repair_queue
        .get("buckets")
        .ok_or_else(|| "repair-queue.json is missing buckets".to_string())?;
    if !buckets.is_object() {
        return Err("repair-queue.json buckets must be an object".to_string());
    }
    for bucket in buckets
        .as_object()
        .ok_or_else(|| "repair-queue.json buckets must be an object".to_string())?
        .keys()
    {
        if !REPAIR_QUEUE_BUCKETS.contains(&bucket.as_str()) {
            return Err(format!(
                "repair-queue.json buckets contain unknown bucket `{bucket}`"
            ));
        }
    }

    let mut queued_card_ids = BTreeSet::new();
    let mut repair_queue_projections = BTreeMap::<String, RepairQueueProjection>::new();
    for bucket in REPAIR_QUEUE_BUCKETS {
        let entries = super::json_array_at(
            &repair_queue,
            &format!("/buckets/{bucket}"),
            "repair-queue.json",
        )?;
        let summary_count = super::json_usize_at(
            &repair_queue,
            &format!("/summary/{bucket}"),
            "repair-queue.json",
        )?;
        if summary_count != entries.len() {
            return Err(format!(
                "repair-queue.json summary.{bucket} is {summary_count}, but bucket has {} entrie(s)",
                entries.len()
            ));
        }
        let mut bucket_card_ids = BTreeSet::new();
        for entry in entries {
            let entry_projection =
                check_repair_queue_entry(entry, bucket, card_ids, card_projections)?;
            if !bucket_card_ids.insert(entry_projection.card_id.clone()) {
                return Err(format!(
                    "repair-queue.json bucket `{bucket}` repeats card id `{}`",
                    entry_projection.card_id
                ));
            }
            queued_card_ids.insert(entry_projection.card_id.clone());
            push_repair_queue_projection(&mut repair_queue_projections, bucket, entry_projection)?;
        }
    }
    for card_id in card_ids {
        if !queued_card_ids.contains(card_id) {
            return Err(format!(
                "repair-queue.json does not account for ReviewCard id `{card_id}`"
            ));
        }
    }
    Ok(repair_queue_projections)
}

fn check_repair_queue_entry(
    entry: &serde_json::Value,
    bucket: &str,
    card_ids: &BTreeSet<String>,
    card_projections: &BTreeMap<String, CardProjection>,
) -> Result<RepairQueueEntryProjection, String> {
    let card_id = require_known_card_id(entry, "repair-queue.json entry", card_ids)?;
    let card = card_projections
        .get(card_id)
        .ok_or_else(|| format!("repair-queue.json entry references unknown card id `{card_id}`"))?;
    require_projected_str(entry, "class", &card.class_name, "repair-queue.json entry")?;
    require_projected_str(entry, "priority", &card.priority, "repair-queue.json entry")?;
    require_projected_str(
        entry,
        "confidence",
        &card.confidence,
        "repair-queue.json entry",
    )?;
    require_projected_str(
        entry,
        "operation_family",
        &card.operation_family,
        "repair-queue.json entry",
    )?;
    require_projected_str(
        entry,
        "operation",
        &card.operation,
        "repair-queue.json entry",
    )?;
    require_projected_str(entry, "path", &card.path, "repair-queue.json entry")?;
    let line = super::json_usize_at(entry, "/line", "repair-queue.json entry")?;
    if line as u64 != card.line {
        return Err(format!(
            "repair-queue.json entry `{card_id}` line must project cards.json line {}; got {line}",
            card.line
        ));
    }
    super::json_array_at(entry, "/missing_evidence", "repair-queue.json entry")?;
    require_projected_string_array(
        entry,
        "missing_evidence",
        &card.missing,
        "repair-queue.json entry",
    )?;
    let reason =
        super::require_non_empty_json_str(entry, "bucket_reason", "repair-queue.json entry")?;
    require_expected_value(
        reason,
        expected_repair_queue_bucket_reason(bucket),
        "repair-queue.json bucket_reason",
    )?;
    let context_command =
        super::require_non_empty_json_str(entry, "context_command", "repair-queue.json entry")?;
    require_expected_value(
        context_command,
        &format!("unsafe-review context {card_id} --json"),
        "repair-queue.json context_command",
    )?;
    check_repair_queue_do_not_do(entry)?;
    let boundary = entry
        .get("trust_boundary")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "repair-queue.json entry is missing trust_boundary".to_string())?;
    check_repair_queue_trust_boundary(boundary, "repair-queue.json entry")?;
    let readiness = entry
        .get("agent_readiness")
        .ok_or_else(|| "repair-queue.json entry is missing agent_readiness".to_string())?;
    let readiness = check_repair_queue_readiness(readiness, bucket)?;
    Ok(RepairQueueEntryProjection {
        card_id: card_id.to_string(),
        readiness_state: readiness.state,
        readiness_reasons: readiness.reasons,
    })
}

fn check_repair_queue_do_not_do(entry: &serde_json::Value) -> Result<(), String> {
    let rules = super::json_array_at(entry, "/do_not_do", "repair-queue.json entry")?;
    for rule in rules {
        let Some(text) = rule.as_str() else {
            return Err("repair-queue.json entry do_not_do entries must be strings".to_string());
        };
        if !text.starts_with("do not ") {
            return Err(format!(
                "repair-queue.json entry do_not_do rule must start with `do not`: {text}"
            ));
        }
    }
    let rendered =
        serde_json::to_string(rules).map_err(|err| format!("render do_not_do failed: {err}"))?;
    for expected in [
        "suppress this card",
        "broad suppression",
        "executable guard or discharge evidence",
        "comments or docs",
        "Miri proof",
        "automatic safety repair",
        "ran an agent, ran witnesses, applied source edits, or posted comments",
        "unrelated unsafe code",
        "test mention as proof that the unsafe site executed",
    ] {
        if !rendered.contains(expected) {
            return Err(format!(
                "repair-queue.json entry do_not_do must include boundary `{expected}`"
            ));
        }
    }
    Ok(())
}

fn check_repair_queue_trust_boundary(text: &str, context: &str) -> Result<(), String> {
    super::require_boundary_text(text, context)?;
    for expected in REPAIR_QUEUE_TRUST_BOUNDARY_LIMITS {
        if !super::text_contains_ignore_ascii_case(text, expected) {
            return Err(format!(
                "{context} trust_boundary must include `{expected}`"
            ));
        }
    }
    Ok(())
}

fn push_repair_queue_projection(
    projections: &mut BTreeMap<String, RepairQueueProjection>,
    bucket: &str,
    entry: RepairQueueEntryProjection,
) -> Result<(), String> {
    let projection =
        projections
            .entry(entry.card_id.clone())
            .or_insert_with(|| RepairQueueProjection {
                buckets: Vec::new(),
                readiness_state: entry.readiness_state.clone(),
                readiness_reasons: entry.readiness_reasons.clone(),
            });
    if projection.readiness_state != entry.readiness_state {
        return Err(format!(
            "repair-queue.json card `{}` has inconsistent agent_readiness.state across buckets",
            entry.card_id
        ));
    }
    if projection.readiness_reasons != entry.readiness_reasons {
        return Err(format!(
            "repair-queue.json card `{}` has inconsistent agent_readiness.reasons across buckets",
            entry.card_id
        ));
    }
    if !projection
        .buckets
        .iter()
        .any(|candidate| candidate == bucket)
    {
        projection.buckets.push(bucket.to_string());
    }
    Ok(())
}

fn expected_repair_queue_bucket_reason(bucket: &str) -> &'static str {
    match bucket {
        "repairable_by_guard" => "guard_evidence_missing",
        "repairable_by_safety_docs" => "safety_docs_evidence_missing",
        "repairable_by_test" => "reach_evidence_missing",
        "requires_witness_receipt" => "witness_receipt_missing",
        "requires_human_review" => "human_review_required",
        "do_not_auto_repair" => "not_ready_for_automatic_repair",
        _ => "",
    }
}

fn check_repair_queue_readiness(
    readiness: &serde_json::Value,
    bucket: &str,
) -> Result<RepairQueueReadinessProjection, String> {
    let Some(ready) = readiness.get("ready").and_then(serde_json::Value::as_bool) else {
        return Err("repair-queue.json agent_readiness.ready must be a boolean".to_string());
    };
    let state =
        super::require_non_empty_json_str(readiness, "state", "repair-queue.json agent_readiness")?;
    if !REPAIR_QUEUE_READINESS_STATES.contains(&state) {
        return Err(format!(
            "repair-queue.json agent_readiness.state must be `ready_for_agent`, `requires_human_review`, `requires_witness_receipt`, or `unsupported`; got `{state}`"
        ));
    }
    if ready && state != "ready_for_agent" {
        return Err(
            "repair-queue.json agent_readiness.state must be `ready_for_agent` when ready is true"
                .to_string(),
        );
    }
    if !ready && state == "ready_for_agent" {
        return Err(
            "repair-queue.json agent_readiness.state `ready_for_agent` requires ready = true"
                .to_string(),
        );
    }
    let reasons = super::json_array_at(readiness, "/reasons", "repair-queue.json agent_readiness")?;
    if reasons.is_empty() {
        return Err("repair-queue.json agent_readiness.reasons must not be empty".to_string());
    }
    for reason in reasons {
        if !reason.is_string() {
            return Err(
                "repair-queue.json agent_readiness.reasons entries must be strings".to_string(),
            );
        }
    }
    if matches!(bucket, "requires_human_review" | "do_not_auto_repair") && ready {
        return Err(format!(
            "repair-queue.json {bucket} entries must not be agent-ready"
        ));
    }
    let readiness_reasons = reasons
        .iter()
        .filter_map(serde_json::Value::as_str)
        .map(str::to_string)
        .collect();
    Ok(RepairQueueReadinessProjection {
        state: state.to_string(),
        reasons: readiness_reasons,
    })
}

fn require_known_advisory_scope(scope: &str) -> Result<(), String> {
    match scope {
        "diff" | "repo" => Ok(()),
        _ => Err(format!(
            "cards.json scope must be `diff` or `repo`; got `{scope}`"
        )),
    }
}

fn require_comment_body_boundary(body: &str) -> Result<(), String> {
    for expected in [
        "artifact-only inline comment candidate",
        "unsafe-review did not post this comment",
        "run witnesses",
        "make a policy decision",
    ] {
        if !body.contains(expected) {
            return Err(format!(
                "comment-plan.json comment body must state `{expected}`"
            ));
        }
    }
    Ok(())
}

fn require_comment_body_card_projection(
    body: &str,
    card: &CardProjection,
    context: &str,
) -> Result<(), String> {
    for (field, expected) in [
        (
            "class",
            format!("`unsafe-review` found `{}`", card.class_name),
        ),
        (
            "operation",
            format!("for `{}`", collapse_whitespace(&card.operation)),
        ),
        ("operation_family", format!("(`{}`)", card.operation_family)),
        (
            "missing evidence",
            format!("Missing evidence: {}", expected_missing_summary(card)),
        ),
        ("next_action", format!("Next action: {}", card.next_action)),
    ] {
        if !body.contains(&expected) {
            return Err(format!(
                "{context} body must project ReviewCard {field} `{expected}`"
            ));
        }
    }
    if let Some(route) = card.witness_routes.first() {
        let expected = format!("Witness route: `{}` because {}.", route.kind, route.reason);
        if !body.contains(&expected) {
            return Err(format!(
                "{context} body must project ReviewCard witness route `{expected}`"
            ));
        }
    }
    if let Some(command) = card.verify_commands.first() {
        let expected = format!("Verify command: `{command}`");
        if !body.contains(&expected) {
            return Err(format!(
                "{context} body must project ReviewCard verify command `{expected}`"
            ));
        }
    }
    Ok(())
}

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn sarif_rule_ids(sarif: &serde_json::Value) -> Result<BTreeSet<&str>, String> {
    let mut rule_ids = BTreeSet::new();
    for rule in super::json_array_at(
        sarif,
        "/runs/0/tool/driver/rules",
        "cards.sarif tool.driver",
    )? {
        let id = super::require_non_empty_json_str(rule, "id", "cards.sarif rule")?;
        if !rule_ids.insert(id) {
            return Err(format!("cards.sarif repeats rule id `{id}`"));
        }
    }
    Ok(rule_ids)
}

fn advisory_card_projections(
    cards: &serde_json::Value,
) -> Result<BTreeMap<String, CardProjection>, String> {
    let mut projections = BTreeMap::new();
    for card in super::json_array_at(cards, "/cards", "cards.json")? {
        let id = super::require_non_empty_json_str(card, "id", "cards.json card")?.to_string();
        let class_name =
            super::require_non_empty_json_str(card, "class", "cards.json card")?.to_string();
        let priority =
            super::require_non_empty_json_str(card, "priority", "cards.json card")?.to_string();
        let confidence =
            super::require_non_empty_json_str(card, "confidence", "cards.json card")?.to_string();
        let hazards = card
            .get("hazards")
            .map(|hazards| {
                let Some(hazards) = hazards.as_array() else {
                    return Err("cards.json card hazards must be an array".to_string());
                };
                hazards
                    .iter()
                    .map(|hazard| {
                        let Some(hazard) = hazard.as_str() else {
                            return Err(
                                "cards.json card hazards values must be strings".to_string()
                            );
                        };
                        if hazard.trim().is_empty() {
                            return Err(
                                "cards.json card hazards values must not be empty".to_string()
                            );
                        }
                        Ok(hazard.to_string())
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?
            .unwrap_or_default();
        let path = super::require_non_empty_json_str(
            card.pointer("/site")
                .ok_or_else(|| "cards.json card is missing site".to_string())?,
            "file",
            "cards.json card site",
        )?
        .to_string();
        let line = super::json_usize_at(card, "/site/line", "cards.json card")? as u64;
        let column = super::json_usize_at(card, "/site/column", "cards.json card")? as u64;
        let operation =
            super::require_non_empty_json_str(card, "operation", "cards.json card")?.to_string();
        let operation_family =
            super::require_non_empty_json_str(card, "operation_family", "cards.json card")?
                .to_string();
        let next_action =
            super::require_non_empty_json_str(card, "next_action", "cards.json card")?.to_string();
        let missing = card
            .get("missing")
            .map(|missing| {
                missing
                    .as_array()
                    .ok_or_else(|| "cards.json card missing must be an array".to_string())?
                    .iter()
                    .map(|missing| {
                        missing.as_str().map(str::to_string).ok_or_else(|| {
                            "cards.json card missing values must be strings".to_string()
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?
            .unwrap_or_default();
        let contract = optional_card_string(card, "contract")?;
        let discharge = optional_card_string(card, "discharge")?;
        let reach = optional_card_string(card, "reach")?;
        let witness = optional_card_string(card, "witness")?;
        let obligation_evidence = card
            .get("obligation_evidence")
            .map(|evidence| {
                evidence
                    .as_array()
                    .ok_or_else(|| {
                        "cards.json card obligation_evidence must be an array".to_string()
                    })?
                    .iter()
                    .enumerate()
                    .map(|(idx, evidence)| {
                        let context = format!("cards.json card obligation_evidence[{idx}]");
                        check_obligation_evidence_projection_shape(evidence, &context)?;
                        Ok(evidence.clone())
                    })
                    .collect::<Result<Vec<_>, String>>()
            })
            .transpose()?
            .unwrap_or_default();
        let required_safety_conditions = obligation_evidence
            .iter()
            .enumerate()
            .map(|(idx, evidence)| {
                required_safety_condition_projection(
                    evidence,
                    &format!("cards.json card obligation_evidence[{idx}]"),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let verify_commands = super::json_array_at(card, "/verify_commands", "cards.json card")?
            .iter()
            .map(|command| {
                command
                    .as_str()
                    .map(str::to_string)
                    .ok_or_else(|| "cards.json card verify_commands must be strings".to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let witness_routes = card
            .get("witness_routes")
            .map(|routes| {
                routes
                    .as_array()
                    .ok_or_else(|| "cards.json card witness_routes must be an array".to_string())?
                    .iter()
                    .map(|route| {
                        let kind = super::require_non_empty_json_str(
                            route,
                            "kind",
                            "cards.json card witness_routes[]",
                        )
                        .map(str::to_string)?;
                        let reason = super::require_non_empty_json_str(
                            route,
                            "reason",
                            "cards.json card witness_routes[]",
                        )
                        .map(str::to_string)?;
                        let command = witness_route_command_projection(
                            route,
                            "cards.json card witness_routes[]",
                        )?;
                        let required = witness_route_required_projection(
                            route,
                            "cards.json card witness_routes[]",
                        )?;
                        if required {
                            return Err(
                                "cards.json card witness_routes[] required must remain false; unsafe-review routes witnesses but does not require execution by default"
                                    .to_string(),
                            );
                        }
                        Ok::<WitnessRouteProjection, String>(WitnessRouteProjection {
                            kind,
                            reason,
                            command,
                            required,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?
            .unwrap_or_default();
        projections.insert(
            id.clone(),
            CardProjection {
                id,
                class_name,
                priority,
                confidence,
                hazards,
                path,
                line,
                column,
                operation,
                operation_family,
                next_action,
                missing,
                contract,
                discharge,
                reach,
                witness,
                required_safety_conditions,
                obligation_evidence,
                verify_commands,
                witness_routes,
            },
        );
    }
    Ok(projections)
}

fn optional_card_string(card: &serde_json::Value, field: &str) -> Result<Option<String>, String> {
    card.get(field)
        .map(|_| {
            super::require_non_empty_json_str(card, field, "cards.json card").map(str::to_string)
        })
        .transpose()
}

fn require_lsp_hover_hazard_projection(
    contents: &str,
    card: &CardProjection,
    context: &str,
) -> Result<(), String> {
    if card.hazards.is_empty() {
        return Ok(());
    }
    if !contents.contains("Relevant hazard families") {
        return Err(format!(
            "{context} contents must include ReviewCard hazard families"
        ));
    }
    for hazard in &card.hazards {
        let marker = format!("`{hazard}`");
        if !contents.contains(&marker) {
            return Err(format!(
                "{context} contents must include ReviewCard hazard `{hazard}`"
            ));
        }
    }
    Ok(())
}

fn require_lsp_hover_card_projection(
    contents: &str,
    card: &CardProjection,
    context: &str,
) -> Result<(), String> {
    for required in [
        "Why this card exists",
        "Required safety conditions",
        "Evidence found",
        "Evidence missing",
        "What would resolve this",
        "What would not resolve this",
        "Do not widen unsafe scope, suppress the card, or change unrelated unsafe code",
        "Handoff commands",
        "Trust boundary",
    ] {
        if !contents.contains(required) {
            return Err(format!("{context} contents must include `{required}`"));
        }
    }
    for (field, expected) in [
        ("class", format!("`{}`", card.class_name)),
        (
            "operation family",
            format!("`{}` unsafe operation", card.operation_family),
        ),
        ("location", format!("Location: {}:{}", card.path, card.line)),
        ("operation", format!("- Operation: `{}`", card.operation)),
        ("next action", format!("- {}", card.next_action)),
        (
            "explain command",
            format!("- Explain: `unsafe-review explain {}`", card.id.as_str()),
        ),
        (
            "agent context command",
            format!(
                "- Agent context: `unsafe-review context {} --json`",
                card.id.as_str()
            ),
        ),
    ] {
        if !contents.contains(&expected) {
            return Err(format!(
                "{context} contents must project ReviewCard {field} `{expected}`"
            ));
        }
    }
    if card.missing.is_empty() {
        if !contents.contains("- none recorded") {
            return Err(format!(
                "{context} contents must state when ReviewCard missing evidence is empty"
            ));
        }
    } else {
        for missing in &card.missing {
            let expected = format!("- {missing}");
            if !contents.contains(&expected) {
                return Err(format!(
                    "{context} contents must project ReviewCard missing evidence `{missing}`"
                ));
            }
        }
    }
    for condition in &card.required_safety_conditions {
        let description = condition
            .get("description")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                "cards.json required safety condition is missing description".to_string()
            })?;
        let expected = format!("- {description}");
        if !contents.contains(&expected) {
            return Err(format!(
                "{context} contents must project ReviewCard required safety condition `{description}`"
            ));
        }
    }
    for (field, expected) in [
        ("contract", card.contract.as_deref()),
        ("discharge", card.discharge.as_deref()),
        ("reach", card.reach.as_deref()),
        ("witness", card.witness.as_deref()),
    ] {
        let Some(expected) = expected else {
            continue;
        };
        if !contents.contains(expected) {
            return Err(format!(
                "{context} contents must project ReviewCard {field} evidence summary `{expected}`"
            ));
        }
    }
    for command in &card.verify_commands {
        let expected = format!("- `{command}`");
        if !contents.contains(&expected) {
            return Err(format!(
                "{context} contents must project ReviewCard verify command `{command}`"
            ));
        }
    }
    if let Some(route) = card.witness_routes.first() {
        let expected = format!("Witness route: `{}` because {}", route.kind, route.reason);
        if !contents.contains(&expected) {
            return Err(format!(
                "{context} contents must project ReviewCard witness route `{}`",
                route.kind
            ));
        }
    }
    Ok(())
}

fn require_sarif_location_projection(
    result: &serde_json::Value,
    card: &CardProjection,
) -> Result<(), String> {
    let Some(location) = result.pointer("/locations/0/physicalLocation") else {
        return Err("cards.sarif result is missing primary physicalLocation".to_string());
    };
    let uri = location
        .pointer("/artifactLocation/uri")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "cards.sarif result is missing artifactLocation.uri".to_string())?;
    require_expected_value(uri, &card.path, "cards.sarif result location uri")?;
    let start_line = location
        .pointer("/region/startLine")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "cards.sarif result is missing region.startLine".to_string())?;
    if start_line != card.line {
        return Err(format!(
            "cards.sarif result location startLine must project cards.json value `{}`; got `{start_line}`",
            card.line
        ));
    }
    let start_column = location
        .pointer("/region/startColumn")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "cards.sarif result is missing region.startColumn".to_string())?;
    if start_column != card.column {
        return Err(format!(
            "cards.sarif result location startColumn must project cards.json value `{}`; got `{start_column}`",
            card.column
        ));
    }
    Ok(())
}

fn require_comment_card_projection(
    comment: &serde_json::Value,
    card: &CardProjection,
    context: &str,
) -> Result<(), String> {
    require_projected_str(comment, "class", &card.class_name, context)?;
    require_projected_str(comment, "priority", &card.priority, context)?;
    require_projected_str(comment, "confidence", &card.confidence, context)?;
    require_projected_str(comment, "path", &card.path, context)?;
    require_projected_u64(comment, "line", card.line, context)?;
    require_projected_str(comment, "operation", &card.operation, context)?;
    require_projected_str(comment, "next_action", &card.next_action, context)?;
    require_projected_string_array(comment, "verify_commands", &card.verify_commands, context)?;
    require_projected_witness_routes(comment, &card.witness_routes, context)?;
    require_projected_str(comment, "operation_family", &card.operation_family, context)
}

fn require_not_selected_card_projection(
    card: &serde_json::Value,
    projection: &CardProjection,
    context: &str,
) -> Result<(), String> {
    require_projected_str(card, "class", &projection.class_name, context)?;
    require_projected_str(card, "priority", &projection.priority, context)?;
    require_projected_str(card, "confidence", &projection.confidence, context)?;
    require_projected_str(card, "path", &projection.path, context)?;
    require_projected_u64(card, "line", projection.line, context)?;
    require_projected_str(card, "operation", &projection.operation, context)?;
    require_projected_str(
        card,
        "operation_family",
        &projection.operation_family,
        context,
    )?;
    require_projected_str(card, "next_action", &projection.next_action, context)
}

fn require_projected_witness_routes(
    value: &serde_json::Value,
    expected: &[WitnessRouteProjection],
    context: &str,
) -> Result<(), String> {
    require_projected_witness_routes_field(value, "witness_routes", expected, context)
}

fn require_projected_witness_routes_field(
    value: &serde_json::Value,
    field: &str,
    expected: &[WitnessRouteProjection],
    context: &str,
) -> Result<(), String> {
    let Some(actual) = value.get(field).and_then(serde_json::Value::as_array) else {
        return Err(format!("{context} is missing array field `{field}`"));
    };
    if actual.len() != expected.len() {
        return Err(format!(
            "{context} {field} must project {} cards.json route(s); got {}",
            expected.len(),
            actual.len()
        ));
    }
    for (idx, (actual, expected)) in actual.iter().zip(expected.iter()).enumerate() {
        let route_context = format!("{context} {field}[{idx}]");
        require_projected_str(actual, "kind", &expected.kind, &route_context)?;
        require_projected_str(actual, "reason", &expected.reason, &route_context)?;
        let actual_command = witness_route_command_projection(actual, &route_context)?;
        if actual_command != expected.command {
            return Err(format!(
                "{route_context} command must project cards.json value {:?}; got {:?}",
                expected.command, actual_command
            ));
        }
        let actual_required = witness_route_required_projection(actual, &route_context)?;
        if actual_required != expected.required {
            return Err(format!(
                "{route_context} required must project cards.json value `{}`; got `{actual_required}`",
                expected.required
            ));
        }
    }
    Ok(())
}

fn witness_route_required_projection(
    route: &serde_json::Value,
    context: &str,
) -> Result<bool, String> {
    let Some(required) = route.get("required").and_then(serde_json::Value::as_bool) else {
        return Err(format!("{context} required must be a boolean"));
    };
    Ok(required)
}

fn witness_route_summaries(routes: &[WitnessRouteProjection]) -> Vec<String> {
    routes
        .iter()
        .map(|route| format!("{}: {}", route.kind, route.reason))
        .collect()
}

fn require_projected_str(
    value: &serde_json::Value,
    field: &str,
    expected: &str,
    context: &str,
) -> Result<(), String> {
    let actual = super::require_non_empty_json_str(value, field, context)?;
    require_expected_value(actual, expected, &format!("{context} {field}"))
}

fn require_projected_u64(
    value: &serde_json::Value,
    field: &str,
    expected: u64,
    context: &str,
) -> Result<(), String> {
    let Some(actual) = value.get(field).and_then(serde_json::Value::as_u64) else {
        return Err(format!("{context} is missing {field}"));
    };
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "{context} {field} must project cards.json value `{expected}`; got `{actual}`"
        ))
    }
}

fn require_projected_string_array(
    value: &serde_json::Value,
    field: &str,
    expected: &[String],
    context: &str,
) -> Result<(), String> {
    let Some(actual) = value.get(field).and_then(serde_json::Value::as_array) else {
        return Err(format!("{context} is missing array field `{field}`"));
    };
    let actual = actual
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_string)
                .ok_or_else(|| format!("{context} {field} values must be strings"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "{context} {field} must project cards.json value {:?}; got {:?}",
            expected, actual
        ))
    }
}

fn require_expected_value(actual: &str, expected: &str, context: &str) -> Result<(), String> {
    if actual == expected {
        Ok(())
    } else {
        Err(format!("{context} must be `{expected}`; got `{actual}`"))
    }
}

fn require_allowed_value(actual: &str, allowed: &[&str], context: &str) -> Result<(), String> {
    if allowed.contains(&actual) {
        Ok(())
    } else {
        Err(format!(
            "{context} must use a known review-budget reason; got `{actual}`"
        ))
    }
}

fn should_project_planned_comment(card: &CardProjection) -> bool {
    class_is_actionable(&card.class_name)
        && card.operation_family != "unknown"
        && (card.priority == "high" || card.confidence == "high")
        && !matches!(card.confidence.as_str(), "low" | "unknown")
}

fn expected_selection_reason(card: &CardProjection) -> &'static str {
    if card.confidence == "high" {
        "actionable high-confidence review card"
    } else {
        "actionable high-priority review card"
    }
}

fn expected_selection_reason_code(_card: &CardProjection) -> &'static str {
    "top_actionable_card"
}

fn expected_non_selection_reason(
    card: &CardProjection,
    planned_count: usize,
    selected_budget_keys: &BTreeSet<String>,
    changed_line: bool,
) -> &'static str {
    if !changed_line {
        "outside changed hunk"
    } else if !class_is_actionable(&card.class_name) {
        "class not eligible for inline comments"
    } else if card.operation_family == "unknown" {
        "operation family unknown"
    } else if matches!(card.confidence.as_str(), "low" | "unknown") {
        "confidence below inline comment threshold"
    } else if !(card.priority == "high" || card.confidence == "high") {
        "priority/confidence below inline comment threshold"
    } else if selected_budget_keys.contains(&comment_budget_key(card)) {
        "covered by selected family/obligation sibling"
    } else if planned_count >= 3 {
        "comment-plan max of three candidates reached"
    } else {
        "not selected by current inline comment policy"
    }
}

fn expected_non_selection_reason_code(
    card: &CardProjection,
    planned_count: usize,
    selected_budget_keys: &BTreeSet<String>,
    changed_line: bool,
) -> &'static str {
    if !changed_line {
        "outside_changed_hunk"
    } else if !class_is_actionable(&card.class_name) || card.operation_family == "unknown" {
        "human_deep_review_only"
    } else if matches!(card.confidence.as_str(), "low" | "unknown")
        || !(card.priority == "high" || card.confidence == "high")
    {
        "lower_relevance"
    } else if selected_budget_keys.contains(&comment_budget_key(card)) {
        "covered_by_selected_family_obligation"
    } else if planned_count >= 3 {
        "budget_exhausted"
    } else {
        "not_selected_by_policy"
    }
}

fn comment_budget_key(card: &CardProjection) -> String {
    let mut obligations = card
        .obligation_evidence
        .iter()
        .filter(|evidence| {
            !evidence_axis_present(evidence, "contract")
                || !evidence_axis_present(evidence, "discharge")
                || !evidence_axis_present(evidence, "reach")
                || !evidence_axis_present(evidence, "witness")
        })
        .filter_map(|evidence| evidence.get("key").and_then(serde_json::Value::as_str))
        .collect::<Vec<_>>();
    obligations.sort_unstable();
    obligations.dedup();
    if obligations.is_empty() {
        obligations.push("review");
    }
    format!("{}:{}", card.operation_family, obligations.join("|"))
}

fn evidence_axis_present(evidence: &serde_json::Value, axis: &str) -> bool {
    evidence
        .get(axis)
        .and_then(|axis| axis.get("present"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

fn expected_relevance(card: &CardProjection) -> &'static str {
    let high_priority = card.priority == "high";
    let high_confidence = card.confidence == "high";
    if matches!(card.confidence.as_str(), "low" | "unknown") {
        "low"
    } else if high_priority && high_confidence {
        "high"
    } else if high_priority || high_confidence {
        "medium"
    } else {
        "low"
    }
}

fn expected_actionability(class_name: &str) -> &'static str {
    match class_name {
        "guard_missing" => "specific_guard_missing",
        "contract_missing" => "specific_contract_missing",
        "guarded_unwitnessed"
        | "reachable_unwitnessed"
        | "requires_loom"
        | "requires_sanitizer"
        | "requires_kani_or_crux"
        | "miri_unsupported" => "specific_witness_missing",
        "witness_mismatch" => "specific_receipt_missing",
        "unsafe_unreached" => "specific_reach_missing",
        "static_unknown" => "human_review_only",
        _ => "not_actionable",
    }
}

fn class_is_actionable(class_name: &str) -> bool {
    matches!(
        class_name,
        "guarded_unwitnessed"
            | "contract_missing"
            | "guard_missing"
            | "reachable_unwitnessed"
            | "unsafe_unreached"
            | "requires_loom"
            | "requires_sanitizer"
            | "requires_kani_or_crux"
            | "miri_unsupported"
            | "static_unknown"
    )
}

const KNOWN_RELEVANCE_VALUES: &[&str] = &["high", "medium", "low"];

fn require_relevance_value(value: &str, context: &str) -> Result<(), String> {
    if KNOWN_RELEVANCE_VALUES.contains(&value) {
        Ok(())
    } else {
        Err(format!(
            "{context} relevance must be one of high/medium/low; got `{value}`"
        ))
    }
}

fn check_witness_plan_artifact(
    dir: &Path,
    card_count: usize,
    open_actionable_gaps: usize,
    card_projections: &BTreeMap<String, CardProjection>,
) -> Result<(), String> {
    let path = dir.join("witness-plan.md");
    let text = super::read_to_string(&path)?;
    let review_cards_line = format!("- Review cards: {card_count}");
    let open_actionable_line = format!("- Open actionable gaps: {open_actionable_gaps}");
    let policy_mode_line = "- Policy mode: `advisory`";
    super::require_text_contains_all(
        &text,
        &path,
        &[
            "# unsafe-review witness plan",
            review_cards_line.as_str(),
            open_actionable_line.as_str(),
            policy_mode_line,
            "does not run Miri",
            "cargo-careful",
            "not a proof of memory safety",
            "not UB-free status",
            "not a Miri result",
        ],
    )?;
    if card_count > 0 {
        super::require_text_contains_all(
            &text,
            &path,
            &[
                "## Route groups",
                "- Route:",
                "What it can show",
                "What it cannot prove",
                "Receipt hint",
            ],
        )?;
        require_witness_plan_verify_commands(&text, &path, card_projections)?;
        require_witness_plan_card_projections(&text, &path, card_projections)?;
    } else {
        super::require_text_contains_all(
            &text,
            &path,
            &[
                "No changed unsafe-review gaps were found.",
                "unsafe site executed",
            ],
        )?;
    }
    Ok(())
}

fn require_witness_plan_card_projections(
    text: &str,
    path: &Path,
    card_projections: &BTreeMap<String, CardProjection>,
) -> Result<(), String> {
    for (card_id, card) in card_projections {
        let section = witness_plan_card_section(text, card_id).ok_or_else(|| {
            format!(
                "{} witness-plan must include a section for ReviewCard `{card_id}`",
                path.display()
            )
        })?;
        require_witness_plan_card_line(
            section,
            path,
            card_id,
            "class",
            &format!("- Class: `{}`", card.class_name),
        )?;
        require_witness_plan_card_line(
            section,
            path,
            card_id,
            "location",
            &format!("- Location: {}:{}", card.path, card.line),
        )?;
        require_witness_plan_card_line(
            section,
            path,
            card_id,
            "operation",
            &format!("- Operation: `{}`", card.operation),
        )?;
        require_witness_plan_card_line(
            section,
            path,
            card_id,
            "next action",
            &format!("- Next action: {}", card.next_action),
        )?;
        for route in &card.witness_routes {
            require_witness_plan_card_line(
                section,
                path,
                card_id,
                "witness route",
                &format!("- Route: `{}`", route.kind),
            )?;
            require_witness_plan_card_line(
                section,
                path,
                card_id,
                "witness route reason",
                &format!("  - Reason: {}", route.reason),
            )?;
            if let Some(command) = &route.command {
                require_witness_plan_route_command(section, path, card_id, command)?;
            }
        }
    }
    Ok(())
}

fn witness_route_command_projection(
    route: &serde_json::Value,
    context: &str,
) -> Result<Option<String>, String> {
    let Some(command) = route.get("command") else {
        return Ok(None);
    };
    if command.is_null() {
        return Ok(None);
    }
    let Some(command) = command.as_str() else {
        return Err(format!("{context} command must be null or a string"));
    };
    if command.trim().is_empty() {
        return Err(format!("{context} command must not be empty"));
    }
    Ok(Some(command.to_string()))
}

fn witness_plan_card_section<'a>(text: &'a str, card_id: &str) -> Option<&'a str> {
    let heading = format!("#### `{card_id}`");
    let start = text.find(&heading)?;
    let body_start = start + heading.len();
    let tail = &text[body_start..];
    let end = [tail.find("\n#### `"), tail.find("\n## Trust boundary")]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(tail.len());
    Some(&tail[..end])
}

fn require_witness_plan_card_line(
    section: &str,
    path: &Path,
    card_id: &str,
    field: &str,
    expected: &str,
) -> Result<(), String> {
    if section.contains(expected) {
        Ok(())
    } else {
        Err(format!(
            "{} witness-plan ReviewCard `{card_id}` {field} must include `{expected}`",
            path.display()
        ))
    }
}

fn require_witness_plan_route_command(
    section: &str,
    path: &Path,
    card_id: &str,
    command: &str,
) -> Result<(), String> {
    let expected = format!("```bash\n{command}\n```");
    if section.contains(&expected) {
        Ok(())
    } else {
        Err(format!(
            "{} witness-plan ReviewCard `{card_id}` witness route command must include fenced command `{command}`",
            path.display()
        ))
    }
}

fn require_witness_plan_verify_commands(
    text: &str,
    path: &Path,
    card_projections: &BTreeMap<String, CardProjection>,
) -> Result<(), String> {
    for (card_id, card) in card_projections {
        for command in &card.verify_commands {
            if !text.contains(command) {
                return Err(format!(
                    "{} must include verify command `{command}` for ReviewCard `{card_id}`",
                    path.display()
                ));
            }
        }
    }
    Ok(())
}

fn check_first_pr_markdown_card_identity(
    dir: &Path,
    card_ids: &BTreeSet<String>,
    card_projections: &BTreeMap<String, CardProjection>,
    repair_queue_projections: &BTreeMap<String, RepairQueueProjection>,
) -> Result<(), String> {
    let pr_summary_path = dir.join("pr-summary.md");
    let pr_summary = super::read_to_string(&pr_summary_path)?;
    require_text_mentions_only_known_card_ids(&pr_summary, &pr_summary_path, card_ids)?;
    require_text_mentions_all_card_ids(&pr_summary, &pr_summary_path, card_ids)?;
    require_markdown_top_card_projection(&pr_summary, &pr_summary_path, card_projections)?;
    require_pr_summary_top_card_repair_queue_projection(
        &pr_summary,
        &pr_summary_path,
        card_ids,
        repair_queue_projections,
    )?;
    require_pr_summary_card_table_projection(&pr_summary, &pr_summary_path, card_projections)?;
    require_pr_summary_witness_plan_projection(&pr_summary, &pr_summary_path, card_projections)?;

    let witness_plan_path = dir.join("witness-plan.md");
    let witness_plan = super::read_to_string(&witness_plan_path)?;
    require_text_mentions_only_known_card_ids(&witness_plan, &witness_plan_path, card_ids)?;
    require_witness_plan_headings_known(&witness_plan, &witness_plan_path, card_ids)?;
    require_text_mentions_all_card_ids(&witness_plan, &witness_plan_path, card_ids)
}

fn require_pr_summary_top_card_repair_queue_projection(
    text: &str,
    path: &Path,
    card_ids: &BTreeSet<String>,
    repair_queue_projections: &BTreeMap<String, RepairQueueProjection>,
) -> Result<(), String> {
    if card_ids.is_empty() {
        return Ok(());
    }
    let top_card_id = markdown_top_card_id(text, path, card_ids)?;
    let projection = repair_queue_projections.get(&top_card_id).ok_or_else(|| {
        format!(
            "repair-queue.json does not include top card `{top_card_id}` for {}",
            path.display()
        )
    })?;
    let expected = expected_agent_handoff_summary(projection);
    if text.contains(&expected) {
        Ok(())
    } else {
        Err(format!(
            "{} top card `{top_card_id}` agent handoff must include `{expected}`",
            path.display()
        ))
    }
}

fn markdown_top_card_id(
    text: &str,
    path: &Path,
    card_ids: &BTreeSet<String>,
) -> Result<String, String> {
    for line in text.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed
            .strip_prefix("- ID: `")
            .or_else(|| trimmed.strip_prefix("- Top card: `"))
        else {
            continue;
        };
        let Some((card_id, _)) = rest.split_once('`') else {
            continue;
        };
        if !card_ids.contains(card_id) {
            return Err(format!(
                "{} top card id `{card_id}` is not present in cards.json",
                path.display()
            ));
        }
        return Ok(card_id.to_string());
    }
    Err(format!(
        "{} must include a top ReviewCard id line",
        path.display()
    ))
}

fn expected_agent_handoff_summary(projection: &RepairQueueProjection) -> String {
    format!(
        "- Agent handoff: `{}`; buckets: {}; reasons: {}",
        projection.readiness_state,
        render_backtick_list(&projection.buckets),
        projection.readiness_reasons.join("; ")
    )
}

fn render_backtick_list(values: &[String]) -> String {
    if values.is_empty() {
        return "`none`".to_string();
    }
    values
        .iter()
        .map(|value| format!("`{value}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn require_pr_summary_card_table_projection(
    text: &str,
    path: &Path,
    card_projections: &BTreeMap<String, CardProjection>,
) -> Result<(), String> {
    if card_projections.is_empty() {
        return Ok(());
    }
    super::require_text_contains(text, "## Card table", path)?;
    for (card_id, card) in card_projections {
        let expected = format!(
            "| `{}` | `{}` | {} | `{}` | `{}` | {} | `{}` | {} |",
            markdown_table_cell(card_id),
            card.class_name,
            markdown_table_cell(&format!("{}:{}", card.path, card.line)),
            card.operation_family,
            markdown_table_cell(&card.operation),
            markdown_table_cell(&expected_missing_summary(card)),
            card.witness_routes
                .first()
                .map_or("human", |route| route.kind.as_str()),
            markdown_table_cell(&card.next_action)
        );
        if !text.contains(&expected) {
            return Err(format!(
                "{} card table row for `{card_id}` must include `{expected}`",
                path.display()
            ));
        }
    }
    Ok(())
}

fn markdown_table_cell(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('|', "\\|")
}

fn require_pr_summary_witness_plan_projection(
    text: &str,
    path: &Path,
    card_projections: &BTreeMap<String, CardProjection>,
) -> Result<(), String> {
    if card_projections.is_empty() {
        return Ok(());
    }
    let section = pr_summary_witness_plan_section(text).ok_or_else(|| {
        format!(
            "{} must include a `## Witness plan` section before `## Trust boundary`",
            path.display()
        )
    })?;
    for (card_id, card) in card_projections {
        if let Some(route) = card.witness_routes.first() {
            let expected = format!("- `{card_id}`: `{}` because {}", route.kind, route.reason);
            require_pr_summary_witness_line(section, path, card_id, "primary route", &expected)?;
            if let Some(command) = &route.command {
                let expected = format!("```bash\n{command}\n```");
                require_pr_summary_witness_line(
                    section,
                    path,
                    card_id,
                    "primary route command",
                    &expected,
                )?;
            } else {
                require_pr_summary_witness_line(
                    section,
                    path,
                    card_id,
                    "manual route limit",
                    "  - No automatic command is available; route this to human review.",
                )?;
            }
        } else {
            let expected = format!(
                "- `{card_id}`: no witness route was selected; route this to human review."
            );
            require_pr_summary_witness_line(section, path, card_id, "manual route", &expected)?;
        }
    }
    Ok(())
}

fn pr_summary_witness_plan_section(text: &str) -> Option<&str> {
    let start = text.find("## Witness plan")?;
    let tail = &text[start..];
    let end = tail.find("\n## Trust boundary")?;
    Some(&tail[..end])
}

fn require_pr_summary_witness_line(
    section: &str,
    path: &Path,
    card_id: &str,
    field: &str,
    expected: &str,
) -> Result<(), String> {
    if section.contains(expected) {
        Ok(())
    } else {
        Err(format!(
            "{} pr-summary witness plan for `{card_id}` {field} must include `{expected}`",
            path.display()
        ))
    }
}

fn check_lsp_artifact(dir: &Path, summary: &AdvisoryArtifactSummary) -> Result<(), String> {
    let path = dir.join("lsp.json");
    let lsp = super::parse_json_file(&path)?;
    let card_projections = &summary.card_projections;
    let card_ids = card_projections.keys().cloned().collect::<BTreeSet<_>>();
    super::require_json_str(&lsp, "schema_version", "0.1", "lsp.json")?;
    super::require_json_str(&lsp, "tool", "unsafe-review", "lsp.json")?;
    super::require_json_str(&lsp, "mode", "read_only_projection", "lsp.json")?;
    super::require_json_str(&lsp, "policy", "advisory", "lsp.json")?;
    super::require_json_str(&lsp, "scope", &summary.scope, "lsp.json")?;
    super::require_json_array(&lsp, "diagnostics", "lsp.json")?;
    super::require_json_array(&lsp, "hovers", "lsp.json")?;
    super::require_json_array(&lsp, "code_actions", "lsp.json")?;
    let boundary = lsp
        .get("trust_boundary")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "lsp.json is missing trust_boundary".to_string())?;
    super::require_boundary_text(boundary, "lsp.json")?;
    require_lsp_status_projection(&lsp, summary)?;

    let mut diagnostic_card_ids = BTreeSet::new();
    for diagnostic in super::json_array_at(&lsp, "/diagnostics", "lsp.json")? {
        let Some(card_id) = diagnostic
            .get("card_id")
            .and_then(serde_json::Value::as_str)
        else {
            return Err("lsp.json diagnostic is missing card_id".to_string());
        };
        if !card_ids.contains(card_id) {
            return Err(format!(
                "lsp.json diagnostic references unknown card id `{card_id}`"
            ));
        }
        if !diagnostic_card_ids.insert(card_id.to_string()) {
            return Err(format!("lsp.json diagnostics repeat card id `{card_id}`"));
        }
        let Some(card_projection) = card_projections.get(card_id) else {
            return Err(format!(
                "lsp.json diagnostic references unknown card id `{card_id}`"
            ));
        };
        super::require_non_empty_json_str(diagnostic, "path", "lsp.json diagnostic")?;
        check_lsp_range(diagnostic, "lsp.json diagnostic")?;
        check_lsp_projection_location(
            diagnostic,
            card_projection,
            "lsp.json diagnostic",
            "/range/start/line",
        )?;
        require_lsp_diagnostic_card_projection(diagnostic, card_projection)?;
        super::json_array_at(
            diagnostic,
            "/required_safety_conditions",
            "lsp.json diagnostic",
        )?;
        super::json_array_at(diagnostic, "/obligation_evidence", "lsp.json diagnostic")?;
        check_lsp_diagnostic_evidence(diagnostic, card_projection)?;
        require_projected_string_array(
            diagnostic,
            "missing_evidence",
            &card_projection.missing,
            "lsp.json diagnostic",
        )?;
        check_lsp_diagnostic_witness_commands(diagnostic)?;
        require_projected_string_array(
            diagnostic,
            "verify_commands",
            &card_projection.verify_commands,
            "lsp.json diagnostic",
        )?;
        let boundary = diagnostic
            .get("trust_boundary")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "lsp.json diagnostic is missing trust_boundary".to_string())?;
        super::require_boundary_text(boundary, "lsp.json diagnostic")?;
    }
    for card_id in &card_ids {
        if !diagnostic_card_ids.contains(card_id) {
            return Err(format!("lsp.json diagnostics missing card id `{card_id}`"));
        }
    }

    let mut hover_card_ids = BTreeSet::new();
    for hover in super::json_array_at(&lsp, "/hovers", "lsp.json")? {
        let hover_card_id = require_known_card_id(hover, "lsp.json hover", &card_ids)?;
        if !hover_card_ids.insert(hover_card_id.to_string()) {
            return Err(format!("lsp.json hovers repeat card id `{hover_card_id}`"));
        }
        let Some(card_projection) = card_projections.get(hover_card_id) else {
            return Err(format!(
                "lsp.json hover references unknown card id `{hover_card_id}`"
            ));
        };
        super::require_non_empty_json_str(hover, "path", "lsp.json hover")?;
        super::json_usize_at(hover, "/position/line", "lsp.json hover")?;
        super::json_usize_at(hover, "/position/character", "lsp.json hover")?;
        check_lsp_projection_location(hover, card_projection, "lsp.json hover", "/position/line")?;
        let contents = hover
            .get("contents")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "lsp.json hover is missing contents".to_string())?;
        if !contents.contains(&format!("Card: `{hover_card_id}`")) {
            return Err(format!(
                "lsp.json hover contents must mention card id `{hover_card_id}`"
            ));
        }
        require_text_mentions_only_known_card_ids(contents, &path, &card_ids)?;
        require_lsp_hover_card_projection(contents, card_projection, "lsp.json hover")?;
        require_lsp_hover_hazard_projection(contents, card_projection, "lsp.json hover")?;
        super::require_text_contains(contents, "Trust boundary", &path)?;
        let boundary = hover
            .get("trust_boundary")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "lsp.json hover is missing trust_boundary".to_string())?;
        super::require_boundary_text(boundary, "lsp.json hover")?;
    }
    for card_id in &card_ids {
        if !hover_card_ids.contains(card_id) {
            return Err(format!("lsp.json hovers missing card id `{card_id}`"));
        }
    }

    let mut code_action_commands = BTreeSet::new();
    for action in super::json_array_at(&lsp, "/code_actions", "lsp.json")? {
        let action_card_id = require_known_card_id(action, "lsp.json code_action", &card_ids)?;
        super::require_non_empty_json_str(action, "path", "lsp.json code_action")?;
        check_lsp_range(action, "lsp.json code_action")?;
        let title = super::require_non_empty_json_str(action, "title", "lsp.json code_action")?;
        super::require_json_str(action, "kind", "quickfix", "lsp.json code_action")?;
        let Some(command) = action.get("command").and_then(serde_json::Value::as_str) else {
            return Err("lsp.json code_action is missing command".to_string());
        };
        if command.trim().is_empty() {
            return Err("lsp.json code_action command must not be empty".to_string());
        }
        let Some(card_projection) = card_projections.get(action_card_id) else {
            return Err(format!(
                "lsp.json code_action references unknown card id `{action_card_id}`"
            ));
        };
        check_lsp_code_action_location(action, card_projection, command)?;
        let action_key = (action_card_id.to_string(), command.to_string());
        if !code_action_commands.insert(action_key) {
            return Err(format!(
                "lsp.json code_actions repeat command `{command}` for card id `{action_card_id}`"
            ));
        }
        reject_lsp_code_action_edit_fields(action, "lsp.json code_action")?;
        let arguments = super::json_array_at(action, "/arguments", "lsp.json code_action")?;
        require_lsp_code_action_title(action_card_id, command, title, action)?;
        check_lsp_code_action_payload(
            action,
            action_card_id,
            command,
            card_projection,
            &card_ids,
            arguments,
        )?;
    }
    for card_id in &card_ids {
        for command in [
            "unsafe-review.copyAgentPacket",
            "unsafe-review.explainWitnessRoute",
        ] {
            if !code_action_commands.contains(&(card_id.to_string(), command.to_string())) {
                return Err(format!(
                    "lsp.json code_actions missing command `{command}` for card id `{card_id}`"
                ));
            }
        }
    }
    Ok(())
}

fn require_lsp_status_projection(
    lsp: &serde_json::Value,
    summary: &AdvisoryArtifactSummary,
) -> Result<(), String> {
    let status = lsp
        .get("status")
        .ok_or_else(|| "lsp.json is missing status".to_string())?;
    let status_boundary = status
        .get("trust_boundary")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "lsp.json is missing /status/trust_boundary".to_string())?;
    super::require_boundary_text(status_boundary, "lsp.json status")?;

    let expected_state = expected_lsp_status_state(summary);
    let actual_state = super::require_non_empty_json_str(status, "state", "lsp.json status")?;
    require_expected_value(actual_state, expected_state, "lsp.json status state")?;

    require_lsp_status_count(
        status,
        "cards",
        summary.card_count,
        "cards.json summary.cards",
    )?;
    require_lsp_status_count(
        status,
        "open_actionable_gaps",
        summary.open_actionable_gaps,
        "cards.json summary.open_actionable_gaps",
    )?;
    require_lsp_status_count(
        status,
        "high_priority_cards",
        summary.high_priority_cards,
        "cards.json ReviewCard priority",
    )?;

    let expected_message = expected_lsp_status_message(summary, expected_state);
    let actual_message = super::require_non_empty_json_str(status, "message", "lsp.json status")?;
    require_expected_value(actual_message, &expected_message, "lsp.json status message")
}

fn expected_lsp_status_state(summary: &AdvisoryArtifactSummary) -> &'static str {
    if summary.card_count == 0 {
        "quiet"
    } else if summary.open_actionable_gaps > 0 {
        "actionable"
    } else {
        "informational"
    }
}

fn expected_lsp_status_message(summary: &AdvisoryArtifactSummary, state: &str) -> String {
    match state {
        "quiet" => "No unsafe-review cards for this scope".to_string(),
        _ => format!(
            "{} unsafe-review card(s), {} open actionable gap(s)",
            summary.card_count, summary.open_actionable_gaps
        ),
    }
}

fn require_lsp_status_count(
    status: &serde_json::Value,
    field: &str,
    expected: usize,
    source: &str,
) -> Result<(), String> {
    let actual = super::json_usize_at(status, &format!("/{field}"), "lsp.json status")?;
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "lsp.json status {field} must project {source} `{expected}`; got `{actual}`"
        ))
    }
}

fn require_lsp_code_action_title(
    action_card_id: &str,
    command: &str,
    title: &str,
    action: &serde_json::Value,
) -> Result<(), String> {
    let expected = match command {
        "unsafe-review.copyAgentPacket" => {
            format!("Copy unsafe-review packet for {action_card_id}")
        }
        "unsafe-review.explainWitnessRoute" => "Explain unsafe-review witness route".to_string(),
        "unsafe-review.openRelatedTest" => {
            let payload = action
                .get("payload")
                .ok_or_else(|| "lsp.json code_action is missing payload".to_string())?;
            let name =
                super::require_non_empty_json_str(payload, "name", "lsp.json code_action payload")?;
            format!("Open related test {name}")
        }
        "unsafe-review.copyWitnessCommand" => "Copy witness command (does not run)".to_string(),
        _ => {
            return Err(format!(
                "lsp.json code_action command `{command}` is not verifier-known"
            ));
        }
    };
    if title == expected {
        Ok(())
    } else {
        Err(format!(
            "lsp.json code_action `{command}` title must be `{expected}`; got `{title}`"
        ))
    }
}

fn reject_lsp_code_action_edit_fields(
    value: &serde_json::Value,
    context: &str,
) -> Result<(), String> {
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                if matches!(key.as_str(), "edit" | "workspace_edit" | "workspaceEdit") {
                    return Err(format!(
                        "{context} must not contain source edit field `{key}`"
                    ));
                }
                reject_lsp_code_action_edit_fields(child, &format!("{context}/{key}"))?;
            }
        }
        serde_json::Value::Array(items) => {
            for (idx, child) in items.iter().enumerate() {
                reject_lsp_code_action_edit_fields(child, &format!("{context}[{idx}]"))?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn check_lsp_range(value: &serde_json::Value, context: &str) -> Result<(), String> {
    let start_line = super::json_usize_at(value, "/range/start/line", context)?;
    let start_character = super::json_usize_at(value, "/range/start/character", context)?;
    let end_line = super::json_usize_at(value, "/range/end/line", context)?;
    let end_character = super::json_usize_at(value, "/range/end/character", context)?;

    if end_line < start_line || (end_line == start_line && end_character < start_character) {
        return Err(format!("{context} range end must not precede start"));
    }

    Ok(())
}

fn check_lsp_projection_location(
    value: &serde_json::Value,
    card: &CardProjection,
    context: &str,
    line_pointer: &str,
) -> Result<(), String> {
    let path = super::require_non_empty_json_str(value, "path", context)?;
    require_expected_value(path, &card.path, &format!("{context} path"))?;

    let zero_based_line = super::json_usize_at(value, line_pointer, context)?;
    let one_based_line = zero_based_line + 1;
    if one_based_line as u64 != card.line {
        return Err(format!(
            "{context} line must point at ReviewCard site line {}; got {}",
            card.line, one_based_line
        ));
    }

    Ok(())
}

fn check_lsp_code_action_location(
    action: &serde_json::Value,
    card: &CardProjection,
    command: &str,
) -> Result<(), String> {
    if command == "unsafe-review.openRelatedTest" {
        let payload = action
            .get("payload")
            .ok_or_else(|| "lsp.json code_action is missing payload".to_string())?;
        let file = super::require_non_empty_json_str(
            payload,
            "file",
            "lsp.json code_action related_test payload",
        )?;
        let line = super::json_usize_at(
            payload,
            "/line",
            "lsp.json code_action related_test payload",
        )?;
        let path = super::require_non_empty_json_str(action, "path", "lsp.json code_action")?;
        require_expected_value(path, file, "lsp.json code_action related_test path")?;
        let zero_based_line =
            super::json_usize_at(action, "/range/start/line", "lsp.json code_action")?;
        let one_based_line = zero_based_line + 1;
        if one_based_line != line {
            return Err(format!(
                "lsp.json code_action related_test line must point at payload line {line}; got {one_based_line}"
            ));
        }
        return Ok(());
    }

    check_lsp_projection_location(action, card, "lsp.json code_action", "/range/start/line")
}

fn require_lsp_diagnostic_card_projection(
    diagnostic: &serde_json::Value,
    card: &CardProjection,
) -> Result<(), String> {
    require_projected_str(diagnostic, "code", &card.class_name, "lsp.json diagnostic")?;
    require_projected_str(
        diagnostic,
        "operation",
        &card.operation,
        "lsp.json diagnostic",
    )?;
    require_projected_str(
        diagnostic,
        "operation_family",
        &card.operation_family,
        "lsp.json diagnostic",
    )?;
    require_projected_str(
        diagnostic,
        "next_action",
        &card.next_action,
        "lsp.json diagnostic",
    )?;
    require_projected_string_array(diagnostic, "hazards", &card.hazards, "lsp.json diagnostic")
}

fn check_lsp_diagnostic_evidence(
    diagnostic: &serde_json::Value,
    card: &CardProjection,
) -> Result<(), String> {
    let conditions = super::json_array_at(
        diagnostic,
        "/required_safety_conditions",
        "lsp.json diagnostic",
    )?;
    for condition in conditions {
        super::require_non_empty_json_str(condition, "key", "lsp.json diagnostic condition")?;
        super::require_non_empty_json_str(
            condition,
            "description",
            "lsp.json diagnostic condition",
        )?;
    }
    require_projected_json_array(
        conditions,
        &card.required_safety_conditions,
        "lsp.json diagnostic required_safety_conditions",
    )?;

    let evidence_summary = diagnostic
        .get("evidence_summary")
        .ok_or_else(|| "lsp.json diagnostic is missing evidence_summary".to_string())?;
    for key in ["contract", "discharge", "witness"] {
        let Some(evidence) = evidence_summary.get(key) else {
            return Err(format!(
                "lsp.json diagnostic evidence_summary is missing {key}"
            ));
        };
        if !evidence
            .get("present")
            .is_some_and(serde_json::Value::is_boolean)
        {
            return Err(format!(
                "lsp.json diagnostic evidence_summary.{key} is missing boolean present"
            ));
        }
        super::require_non_empty_json_str(
            evidence,
            "state",
            &format!("lsp.json diagnostic evidence_summary.{key}"),
        )?;
        super::require_non_empty_json_str(
            evidence,
            "summary",
            &format!("lsp.json diagnostic evidence_summary.{key}"),
        )?;
    }
    let Some(reach) = evidence_summary.get("reach") else {
        return Err("lsp.json diagnostic evidence_summary is missing reach".to_string());
    };
    super::require_non_empty_json_str(
        reach,
        "state",
        "lsp.json diagnostic evidence_summary.reach",
    )?;
    super::require_non_empty_json_str(
        reach,
        "summary",
        "lsp.json diagnostic evidence_summary.reach",
    )?;
    let reach_limitation = super::require_non_empty_json_str(
        evidence_summary,
        "reach_limitation",
        "lsp.json diagnostic evidence_summary",
    )?;
    if !super::text_contains_ignore_ascii_case(reach_limitation, "not proof") {
        return Err(
            "lsp.json diagnostic evidence_summary.reach_limitation must say reach evidence is not proof"
                .to_string(),
        );
    }
    require_lsp_evidence_summary_projection(evidence_summary, card)?;

    let obligation_evidence =
        super::json_array_at(diagnostic, "/obligation_evidence", "lsp.json diagnostic")?;
    for (idx, evidence) in obligation_evidence.iter().enumerate() {
        check_obligation_evidence_projection_shape(
            evidence,
            &format!("lsp.json diagnostic obligation_evidence[{idx}]"),
        )?;
    }
    require_projected_json_array(
        obligation_evidence,
        &card.obligation_evidence,
        "lsp.json diagnostic obligation_evidence",
    )?;

    Ok(())
}

fn require_lsp_evidence_summary_projection(
    evidence_summary: &serde_json::Value,
    card: &CardProjection,
) -> Result<(), String> {
    for (field, expected) in [
        ("contract", card.contract.as_deref()),
        ("discharge", card.discharge.as_deref()),
        ("reach", card.reach.as_deref()),
        ("witness", card.witness.as_deref()),
    ] {
        let Some(expected) = expected else {
            continue;
        };
        let Some(evidence) = evidence_summary.get(field) else {
            return Err(format!(
                "lsp.json diagnostic evidence_summary is missing {field}"
            ));
        };
        let actual = super::require_non_empty_json_str(
            evidence,
            "summary",
            &format!("lsp.json diagnostic evidence_summary.{field}"),
        )?;
        if actual != expected {
            return Err(format!(
                "lsp.json diagnostic evidence_summary.{field}.summary must project cards.json value `{expected}`; got `{actual}`"
            ));
        }
    }
    Ok(())
}

fn required_safety_condition_projection(
    evidence: &serde_json::Value,
    context: &str,
) -> Result<serde_json::Value, String> {
    let key = super::require_non_empty_json_str(evidence, "key", context)?;
    let description = super::require_non_empty_json_str(evidence, "description", context)?;
    Ok(serde_json::json!({
        "key": key,
        "description": description,
    }))
}

fn check_obligation_evidence_projection_shape(
    evidence: &serde_json::Value,
    context: &str,
) -> Result<(), String> {
    super::require_non_empty_json_str(evidence, "key", context)?;
    super::require_non_empty_json_str(evidence, "description", context)?;
    for key in ["contract", "discharge", "reach", "witness"] {
        let Some(state) = evidence.get(key) else {
            return Err(format!("{context} is missing {key}"));
        };
        if !state
            .get("present")
            .is_some_and(serde_json::Value::is_boolean)
        {
            return Err(format!("{context}.{key} is missing boolean present"));
        }
        super::require_non_empty_json_str(state, "state", &format!("{context}.{key}"))?;
        super::require_non_empty_json_str(state, "summary", &format!("{context}.{key}"))?;
    }
    Ok(())
}

fn require_projected_json_array(
    actual: &[serde_json::Value],
    expected: &[serde_json::Value],
    context: &str,
) -> Result<(), String> {
    if actual.len() != expected.len() {
        return Err(format!(
            "{context} must project {} cards.json value(s); got {}",
            expected.len(),
            actual.len()
        ));
    }
    for (idx, (actual, expected)) in actual.iter().zip(expected.iter()).enumerate() {
        if actual != expected {
            return Err(format!(
                "{context}[{idx}] must project cards.json value `{expected}`; got `{actual}`"
            ));
        }
    }
    Ok(())
}

fn check_lsp_diagnostic_witness_commands(diagnostic: &serde_json::Value) -> Result<(), String> {
    let mut route_commands = BTreeSet::new();
    for (idx, route) in super::json_array_at(diagnostic, "/witness_routes", "lsp.json diagnostic")?
        .iter()
        .enumerate()
    {
        super::require_non_empty_json_str(
            route,
            "kind",
            &format!("lsp.json diagnostic witness_routes[{idx}]"),
        )?;
        super::require_non_empty_json_str(
            route,
            "reason",
            &format!("lsp.json diagnostic witness_routes[{idx}]"),
        )?;
        let Some(required) = route.get("required").and_then(serde_json::Value::as_bool) else {
            return Err(format!(
                "lsp.json diagnostic witness_routes[{idx}] required must be a boolean"
            ));
        };
        if required {
            return Err(format!(
                "lsp.json diagnostic witness_routes[{idx}] required must remain false; unsafe-review routes witnesses but does not require execution by default"
            ));
        }
        if let Some(command) = route.get("command")
            && !command.is_null()
        {
            let Some(command) = command.as_str() else {
                return Err(format!(
                    "lsp.json diagnostic witness_routes[{idx}] command must be null or a string"
                ));
            };
            if command.trim().is_empty() {
                return Err(format!(
                    "lsp.json diagnostic witness_routes[{idx}] command must not be empty"
                ));
            }
            route_commands.insert(command.to_string());
        }
    }

    let mut verify_commands = BTreeSet::new();
    for (idx, command) in
        super::json_array_at(diagnostic, "/verify_commands", "lsp.json diagnostic")?
            .iter()
            .enumerate()
    {
        let Some(command) = command.as_str() else {
            return Err(format!(
                "lsp.json diagnostic verify_commands[{idx}] must be a string"
            ));
        };
        if command.trim().is_empty() {
            return Err(format!(
                "lsp.json diagnostic verify_commands[{idx}] must not be empty"
            ));
        }
        if !verify_commands.insert(command.to_string()) {
            return Err(format!(
                "lsp.json diagnostic verify_commands[{idx}] repeats command `{command}`"
            ));
        }
        if !route_commands.contains(command) {
            return Err(format!(
                "lsp.json diagnostic verify_commands[{idx}] `{command}` must be backed by a witness route command"
            ));
        }
    }
    for command in route_commands {
        if !verify_commands.contains(&command) {
            return Err(format!(
                "lsp.json diagnostic witness route command `{command}` must appear in verify_commands"
            ));
        }
    }

    Ok(())
}

fn check_lsp_code_action_payload(
    action: &serde_json::Value,
    action_card_id: &str,
    command: &str,
    card_projection: &CardProjection,
    card_ids: &BTreeSet<String>,
    arguments: &[serde_json::Value],
) -> Result<(), String> {
    let Some(payload) = action.get("payload") else {
        return Err("lsp.json code_action is missing payload".to_string());
    };
    if !payload.is_object() {
        return Err("lsp.json code_action payload must be an object".to_string());
    }
    let payload_card_id = require_known_card_id(payload, "lsp.json code_action payload", card_ids)?;
    if payload_card_id != action_card_id {
        return Err(format!(
            "lsp.json code_action payload card_id `{payload_card_id}` does not match action card_id `{action_card_id}`"
        ));
    }
    let expected_kind = match command {
        "unsafe-review.copyAgentPacket" => {
            require_lsp_code_action_arguments(command, arguments, &[action_card_id.to_string()])?;
            "unsafe-review.agent_packet"
        }
        "unsafe-review.explainWitnessRoute" => {
            require_lsp_code_action_arguments(command, arguments, &[action_card_id.to_string()])?;
            "unsafe-review.witness_route"
        }
        "unsafe-review.openRelatedTest" => {
            let file =
                super::require_non_empty_json_str(payload, "file", "lsp.json code_action payload")?;
            let line = super::json_usize_at(payload, "/line", "lsp.json code_action payload")?;
            if line == 0 {
                return Err("lsp.json code_action payload line must be one-based".to_string());
            }
            let name =
                super::require_non_empty_json_str(payload, "name", "lsp.json code_action payload")?;
            require_lsp_code_action_arguments(
                command,
                arguments,
                &[
                    action_card_id.to_string(),
                    file.to_string(),
                    line.to_string(),
                    name.to_string(),
                ],
            )?;
            "unsafe-review.related_test"
        }
        "unsafe-review.copyWitnessCommand" => {
            let witness_command = super::require_non_empty_json_str(
                payload,
                "command",
                "lsp.json code_action payload",
            )?;
            if !card_projection
                .verify_commands
                .iter()
                .any(|expected| expected == witness_command)
            {
                return Err(format!(
                    "lsp.json code_action copyWitnessCommand payload command `{witness_command}` must match a ReviewCard verify command for card id `{action_card_id}`"
                ));
            }
            require_lsp_code_action_arguments(command, arguments, &[witness_command.to_string()])?;
            "unsafe-review.witness_command"
        }
        _ => {
            return Err(format!(
                "lsp.json code_action command `{command}` is not verifier-known"
            ));
        }
    };
    super::require_json_str(
        payload,
        "kind",
        expected_kind,
        "lsp.json code_action payload",
    )?;
    let boundary = payload
        .get("trust_boundary")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "lsp.json code_action payload is missing trust_boundary".to_string())?;
    super::require_boundary_text(boundary, "lsp.json code_action payload")?;
    Ok(())
}

fn require_lsp_code_action_arguments(
    command: &str,
    arguments: &[serde_json::Value],
    expected: &[String],
) -> Result<(), String> {
    if arguments.len() != expected.len() {
        return Err(format!(
            "lsp.json code_action `{command}` arguments length must be {}; got {}",
            expected.len(),
            arguments.len()
        ));
    }
    for (idx, expected) in expected.iter().enumerate() {
        let actual = arguments
            .get(idx)
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                format!("lsp.json code_action `{command}` arguments[{idx}] must be a string")
            })?;
        if actual != expected {
            return Err(format!(
                "lsp.json code_action `{command}` arguments[{idx}] must be `{expected}`; got `{actual}`"
            ));
        }
    }
    Ok(())
}

fn require_known_card_id<'a>(
    value: &'a serde_json::Value,
    context: &str,
    card_ids: &BTreeSet<String>,
) -> Result<&'a str, String> {
    let Some(card_id) = value.get("card_id").and_then(serde_json::Value::as_str) else {
        return Err(format!("{context} is missing card_id"));
    };
    if card_ids.contains(card_id) {
        Ok(card_id)
    } else {
        Err(format!("{context} references unknown card id `{card_id}`"))
    }
}

fn check_advisory_artifact_overclaims(dir: &Path) -> Result<(), String> {
    for name in [
        "review-kit.json",
        "cards.json",
        "pr-summary.md",
        "github-summary.md",
        "cards.sarif",
        "comment-plan.json",
        "witness-plan.md",
        "receipt-audit.md",
        "manual-candidates.json",
        "lsp.json",
        "repair-queue.json",
    ] {
        let path = dir.join(name);
        if path.is_file() {
            if is_machine_json_artifact(name) {
                let value = super::parse_json_file(&path)?;
                reject_json_positive_overclaims(&path, &value)?;
            } else {
                super::reject_positive_overclaims(&path, &super::read_to_string(&path)?)?;
            }
        }
    }
    Ok(())
}

fn is_machine_json_artifact(name: &str) -> bool {
    matches!(
        name,
        "review-kit.json"
            | "cards.json"
            | "cards.sarif"
            | "comment-plan.json"
            | "manual-candidates.json"
            | "lsp.json"
            | "repair-queue.json"
    )
}

fn reject_json_positive_overclaims(path: &Path, value: &serde_json::Value) -> Result<(), String> {
    match value {
        serde_json::Value::String(text) => super::reject_positive_overclaims(path, text),
        serde_json::Value::Array(items) => {
            for item in items {
                reject_json_positive_overclaims(path, item)?;
            }
            Ok(())
        }
        serde_json::Value::Object(entries) => {
            for (key, value) in entries {
                super::reject_positive_overclaims(path, key)?;
                reject_json_positive_overclaims(path, value)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
