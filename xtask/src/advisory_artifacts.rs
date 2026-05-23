use std::collections::BTreeSet;
use std::path::Path;

struct AdvisoryArtifactSummary {
    card_ids: BTreeSet<String>,
    card_count: usize,
}

const COMMENT_PLAN_BODY_WORD_LIMIT: usize = 220;

pub(crate) fn check_advisory_artifacts(dir: &Path) -> Result<(), String> {
    check_advisory_artifact_set(dir)?;
    println!("check-advisory-artifacts: ok ({})", dir.display());
    Ok(())
}

pub(crate) fn check_first_pr_artifacts(dir: &Path) -> Result<(), String> {
    let summary = check_advisory_artifact_set(dir)?;
    check_witness_plan_artifact(dir, summary.card_count)?;
    check_lsp_artifact(dir, &summary.card_ids)?;
    check_first_pr_artifact_overclaims(dir)?;

    println!("check-first-pr-artifacts: ok ({})", dir.display());
    Ok(())
}

fn check_advisory_artifact_set(dir: &Path) -> Result<AdvisoryArtifactSummary, String> {
    if !dir.is_dir() {
        return Err(format!(
            "advisory artifact directory missing: {}",
            dir.display()
        ));
    }

    let cards = super::parse_json_file(&dir.join("cards.json"))?;
    super::require_json_str(&cards, "tool", "unsafe-review", "cards.json")?;
    super::require_json_str(&cards, "policy", "advisory", "cards.json")?;
    super::require_json_array(&cards, "cards", "cards.json")?;
    let cards_boundary = cards
        .get("trust_boundary")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "cards.json is missing trust_boundary".to_string())?;
    super::require_boundary_text(cards_boundary, "cards.json")?;
    let card_ids = super::advisory_card_ids(&cards)?;
    let card_count = card_ids.len();
    let summary_cards = super::json_usize_at(&cards, "/summary/cards", "cards.json")?;
    if summary_cards != card_count {
        return Err(format!(
            "cards.json summary.cards is {summary_cards}, but cards array has {card_count}"
        ));
    }

    let pr_summary_path = dir.join("pr-summary.md");
    let pr_summary = super::read_to_string(&pr_summary_path)?;
    super::require_text_contains(
        &pr_summary,
        &format!("- Review cards: {card_count}"),
        &pr_summary_path,
    )?;
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
    if card_count == 0 {
        super::require_text_contains(
            &pr_summary,
            "No changed unsafe-review gaps were found.",
            &pr_summary_path,
        )?;
        super::require_text_contains(&pr_summary, "unsafe site executed", &pr_summary_path)?;
    }

    let sarif = super::parse_json_file(&dir.join("cards.sarif"))?;
    super::require_json_str(&sarif, "version", "2.1.0", "cards.sarif")?;
    super::require_json_array(&sarif, "runs", "cards.sarif")?;
    let sarif_results = super::json_array_at(&sarif, "/runs/0/results", "cards.sarif")?;
    if sarif_results.len() != card_count {
        return Err(format!(
            "cards.sarif has {} result(s), but cards.json has {card_count} card(s)",
            sarif_results.len()
        ));
    }
    for result in sarif_results {
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
        super::json_array_at(
            result,
            "/properties/witnessRouteDetails",
            "cards.sarif result",
        )?;
        super::json_array_at(result, "/properties/verifyCommands", "cards.sarif result")?;
    }
    let sarif_boundary = sarif
        .pointer("/runs/0/properties/trustBoundary")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "cards.sarif is missing /runs/0/properties/trustBoundary".to_string())?;
    super::require_boundary_text(sarif_boundary, "cards.sarif")?;

    let comment_plan = super::parse_json_file(&dir.join("comment-plan.json"))?;
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
    for comment in comments {
        let Some(card_id) = comment.get("card_id").and_then(serde_json::Value::as_str) else {
            return Err("comment-plan.json comment is missing card_id".to_string());
        };
        if !card_ids.contains(card_id) {
            return Err(format!(
                "comment-plan.json references unknown card id `{card_id}`"
            ));
        }
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
        if !body.contains("unsafe-review did not post this comment") {
            return Err(
                "comment-plan.json comment body must state that unsafe-review did not post this comment"
                    .to_string(),
            );
        }
        let body_word_count = body.split_whitespace().count();
        if body_word_count > COMMENT_PLAN_BODY_WORD_LIMIT {
            return Err(format!(
                "comment-plan.json comment body has {body_word_count} word(s), expected at most {COMMENT_PLAN_BODY_WORD_LIMIT}"
            ));
        }
        let class_name =
            super::require_non_empty_json_str(comment, "class", "comment-plan.json comment")?;
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
        let next_action =
            super::require_non_empty_json_str(comment, "next_action", "comment-plan.json comment")?;
        super::require_non_empty_json_str(
            comment,
            "selection_reason",
            "comment-plan.json comment",
        )?;
        super::require_non_empty_json_str(comment, "actionability", "comment-plan.json comment")?;
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
    }
    if let Some(not_selected) = comment_plan.get("not_selected") {
        let Some(not_selected) = not_selected.as_array() else {
            return Err("comment-plan.json not_selected must be an array".to_string());
        };
        let mut not_selected_card_ids = BTreeSet::new();
        for card in not_selected {
            let Some(card_id) = card.get("card_id").and_then(serde_json::Value::as_str) else {
                return Err("comment-plan.json not_selected entry is missing card_id".to_string());
            };
            if !card_ids.contains(card_id) {
                return Err(format!(
                    "comment-plan.json not_selected references unknown card id `{card_id}`"
                ));
            }
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
            super::require_non_empty_json_str(card, "class", "comment-plan.json not_selected")?;
            super::require_non_empty_json_str(card, "priority", "comment-plan.json not_selected")?;
            super::require_non_empty_json_str(
                card,
                "confidence",
                "comment-plan.json not_selected",
            )?;
            super::require_non_empty_json_str(
                card,
                "operation_family",
                "comment-plan.json not_selected",
            )?;
            super::require_non_empty_json_str(
                card,
                "actionability",
                "comment-plan.json not_selected",
            )?;
            super::require_non_empty_json_str(card, "reason", "comment-plan.json not_selected")?;
        }
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

    Ok(AdvisoryArtifactSummary {
        card_ids,
        card_count,
    })
}

fn check_witness_plan_artifact(dir: &Path, card_count: usize) -> Result<(), String> {
    let path = dir.join("witness-plan.md");
    let text = super::read_to_string(&path)?;
    let review_cards_line = format!("- Review cards: {card_count}");
    super::require_text_contains_all(
        &text,
        &path,
        &[
            "# unsafe-review witness plan",
            review_cards_line.as_str(),
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

fn check_lsp_artifact(dir: &Path, card_ids: &BTreeSet<String>) -> Result<(), String> {
    let path = dir.join("lsp.json");
    let lsp = super::parse_json_file(&path)?;
    super::require_json_str(&lsp, "tool", "unsafe-review", "lsp.json")?;
    super::require_json_str(&lsp, "mode", "read_only_projection", "lsp.json")?;
    super::require_json_str(&lsp, "policy", "advisory", "lsp.json")?;
    super::require_json_array(&lsp, "diagnostics", "lsp.json")?;
    super::require_json_array(&lsp, "hovers", "lsp.json")?;
    super::require_json_array(&lsp, "code_actions", "lsp.json")?;
    let boundary = lsp
        .get("trust_boundary")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "lsp.json is missing trust_boundary".to_string())?;
    super::require_boundary_text(boundary, "lsp.json")?;
    let status_boundary = lsp
        .pointer("/status/trust_boundary")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "lsp.json is missing /status/trust_boundary".to_string())?;
    super::require_boundary_text(status_boundary, "lsp.json status")?;

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
        super::json_array_at(
            diagnostic,
            "/required_safety_conditions",
            "lsp.json diagnostic",
        )?;
        super::json_array_at(diagnostic, "/obligation_evidence", "lsp.json diagnostic")?;
        check_lsp_diagnostic_evidence(diagnostic)?;
        super::json_array_at(diagnostic, "/witness_routes", "lsp.json diagnostic")?;
        super::json_array_at(diagnostic, "/verify_commands", "lsp.json diagnostic")?;
        let boundary = diagnostic
            .get("trust_boundary")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "lsp.json diagnostic is missing trust_boundary".to_string())?;
        super::require_boundary_text(boundary, "lsp.json diagnostic")?;
    }

    for hover in super::json_array_at(&lsp, "/hovers", "lsp.json")? {
        require_known_card_id(hover, "lsp.json hover", card_ids)?;
        let contents = hover
            .get("contents")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "lsp.json hover is missing contents".to_string())?;
        super::require_text_contains(contents, "Trust boundary", &path)?;
        let boundary = hover
            .get("trust_boundary")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "lsp.json hover is missing trust_boundary".to_string())?;
        super::require_boundary_text(boundary, "lsp.json hover")?;
    }

    for action in super::json_array_at(&lsp, "/code_actions", "lsp.json")? {
        let action_card_id = require_known_card_id(action, "lsp.json code_action", card_ids)?;
        let Some(command) = action.get("command").and_then(serde_json::Value::as_str) else {
            return Err("lsp.json code_action is missing command".to_string());
        };
        if command.trim().is_empty() {
            return Err("lsp.json code_action command must not be empty".to_string());
        }
        super::json_array_at(action, "/arguments", "lsp.json code_action")?;
        check_lsp_code_action_payload(action, action_card_id, command, card_ids)?;
        if action.get("edit").is_some() || action.get("workspace_edit").is_some() {
            return Err("lsp.json code_action must not contain source edits".to_string());
        }
    }
    Ok(())
}

fn check_lsp_diagnostic_evidence(diagnostic: &serde_json::Value) -> Result<(), String> {
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

    for evidence in super::json_array_at(diagnostic, "/obligation_evidence", "lsp.json diagnostic")?
    {
        super::require_non_empty_json_str(
            evidence,
            "key",
            "lsp.json diagnostic obligation_evidence",
        )?;
        super::require_non_empty_json_str(
            evidence,
            "description",
            "lsp.json diagnostic obligation_evidence",
        )?;
        for key in ["contract", "discharge", "reach", "witness"] {
            let Some(state) = evidence.get(key) else {
                return Err(format!(
                    "lsp.json diagnostic obligation_evidence is missing {key}"
                ));
            };
            if !state
                .get("present")
                .is_some_and(serde_json::Value::is_boolean)
            {
                return Err(format!(
                    "lsp.json diagnostic obligation_evidence.{key} is missing boolean present"
                ));
            }
            super::require_non_empty_json_str(
                state,
                "state",
                &format!("lsp.json diagnostic obligation_evidence.{key}"),
            )?;
            super::require_non_empty_json_str(
                state,
                "summary",
                &format!("lsp.json diagnostic obligation_evidence.{key}"),
            )?;
        }
    }

    Ok(())
}

fn check_lsp_code_action_payload(
    action: &serde_json::Value,
    action_card_id: &str,
    command: &str,
    card_ids: &BTreeSet<String>,
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
        "unsafe-review.copyAgentPacket" => "unsafe-review.agent_packet",
        "unsafe-review.explainWitnessRoute" => "unsafe-review.witness_route",
        "unsafe-review.openRelatedTest" => {
            super::require_non_empty_json_str(payload, "file", "lsp.json code_action payload")?;
            let line = super::json_usize_at(payload, "/line", "lsp.json code_action payload")?;
            if line == 0 {
                return Err("lsp.json code_action payload line must be one-based".to_string());
            }
            super::require_non_empty_json_str(payload, "name", "lsp.json code_action payload")?;
            "unsafe-review.related_test"
        }
        "unsafe-review.copyWitnessCommand" => {
            super::require_non_empty_json_str(payload, "command", "lsp.json code_action payload")?;
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

fn check_first_pr_artifact_overclaims(dir: &Path) -> Result<(), String> {
    for name in [
        "pr-summary.md",
        "comment-plan.json",
        "witness-plan.md",
        "lsp.json",
    ] {
        let path = dir.join(name);
        if path.is_file() {
            super::reject_positive_overclaims(&path, &super::read_to_string(&path)?)?;
        }
    }
    Ok(())
}
