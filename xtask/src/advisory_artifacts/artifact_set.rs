use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::*;

struct CardsArtifact {
    card_ids: BTreeSet<String>,
    card_projections: BTreeMap<String, CardProjection>,
    scope: String,
    card_count: usize,
    open_actionable_gaps: usize,
    high_priority_cards: usize,
}

pub(super) fn check_advisory_artifact_set(dir: &Path) -> Result<AdvisoryArtifactSummary, String> {
    require_advisory_artifact_dir(dir)?;
    let CardsArtifact {
        card_ids,
        card_projections,
        scope,
        card_count,
        open_actionable_gaps,
        high_priority_cards,
    } = load_cards_artifact(dir)?;

    check_pr_summary_artifact(dir, &scope, card_count, open_actionable_gaps)?;
    check_sarif_artifact(dir, &scope, card_count, &card_ids, &card_projections)?;
    check_comment_plan_artifact(dir, card_count, &card_ids, &card_projections)?;
    let repair_queue_projections =
        check_repair_queue_artifact(dir, card_count, &card_ids, &card_projections)?;

    Ok(AdvisoryArtifactSummary {
        card_ids,
        card_projections,
        repair_queue_projections,
        scope,
        card_count,
        open_actionable_gaps,
        high_priority_cards,
    })
}

fn require_advisory_artifact_dir(dir: &Path) -> Result<(), String> {
    if !dir.is_dir() {
        return Err(format!(
            "advisory artifact directory missing: {}",
            dir.display()
        ));
    }
    Ok(())
}

fn load_cards_artifact(dir: &Path) -> Result<CardsArtifact, String> {
    let cards = crate::parse_json_file(&dir.join("cards.json"))?;
    crate::require_json_str(&cards, "schema_version", "0.1", "cards.json")?;
    crate::require_json_str(&cards, "tool", "unsafe-review", "cards.json")?;
    crate::require_json_str(&cards, "policy", "advisory", "cards.json")?;
    crate::require_json_array(&cards, "cards", "cards.json")?;
    let cards_boundary = cards
        .get("trust_boundary")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "cards.json is missing trust_boundary".to_string())?;
    crate::require_boundary_text(cards_boundary, "cards.json")?;
    let scope = crate::require_non_empty_json_str(&cards, "scope", "cards.json")?.to_string();
    require_known_advisory_scope(&scope)?;
    let card_ids = crate::advisory_card_ids(&cards)?;
    let card_projections = advisory_card_projections(&cards)?;
    let card_count = card_ids.len();
    let summary_cards = crate::json_usize_at(&cards, "/summary/cards", "cards.json")?;
    let open_actionable_gaps =
        crate::json_usize_at(&cards, "/summary/open_actionable_gaps", "cards.json")?;
    if summary_cards != card_count {
        return Err(format!(
            "cards.json summary.cards is {summary_cards}, but cards array has {card_count}"
        ));
    }
    let high_priority_cards = card_projections
        .values()
        .filter(|card| card.priority == "high")
        .count();

    Ok(CardsArtifact {
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
    scope: &str,
    card_count: usize,
    open_actionable_gaps: usize,
) -> Result<(), String> {
    let pr_summary_path = dir.join("pr-summary.md");
    let pr_summary = crate::read_to_string(&pr_summary_path)?;
    crate::require_text_contains(
        &pr_summary,
        &format!("- Scope: `{scope}`"),
        &pr_summary_path,
    )?;
    crate::require_text_contains(
        &pr_summary,
        &format!("- Review cards: {card_count}"),
        &pr_summary_path,
    )?;
    crate::require_text_contains(
        &pr_summary,
        &format!("- Open actionable gaps: {open_actionable_gaps}"),
        &pr_summary_path,
    )?;
    crate::require_text_contains(&pr_summary, "- Policy mode: `advisory`", &pr_summary_path)?;
    crate::require_text_contains(
        &pr_summary,
        "static unsafe contract review",
        &pr_summary_path,
    )?;
    crate::require_text_contains(
        &pr_summary,
        "not a proof of memory safety",
        &pr_summary_path,
    )?;
    crate::require_text_contains(&pr_summary, "not UB-free status", &pr_summary_path)?;
    crate::require_text_contains(&pr_summary, "not a Miri result", &pr_summary_path)?;
    crate::require_text_contains(
        &pr_summary,
        "- Receipt audit: `receipt-audit.md` checks saved receipt metadata only; no witness was run.",
        &pr_summary_path,
    )?;
    if card_count == 0 {
        crate::require_text_contains(
            &pr_summary,
            "No changed unsafe-review gaps were found.",
            &pr_summary_path,
        )?;
        crate::require_text_contains(&pr_summary, "unsafe site executed", &pr_summary_path)?;
    }
    Ok(())
}

fn check_sarif_artifact(
    dir: &Path,
    scope: &str,
    card_count: usize,
    card_ids: &BTreeSet<String>,
    card_projections: &BTreeMap<String, CardProjection>,
) -> Result<(), String> {
    let sarif = crate::parse_json_file(&dir.join("cards.sarif"))?;
    crate::require_json_str(&sarif, "version", "2.1.0", "cards.sarif")?;
    crate::require_json_array(&sarif, "runs", "cards.sarif")?;
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
    let sarif_results = crate::json_array_at(&sarif, "/runs/0/results", "cards.sarif")?;
    if sarif_results.len() != card_count {
        return Err(format!(
            "cards.sarif has {} result(s), but cards.json has {card_count} card(s)",
            sarif_results.len()
        ));
    }
    let mut sarif_card_ids = BTreeSet::new();
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
        if !sarif_card_ids.insert(card_id.to_string()) {
            return Err(format!("cards.sarif results repeat card id `{card_id}`"));
        }
        let Some(card_projection) = card_projections.get(card_id) else {
            return Err(format!(
                "cards.sarif result references unknown card id `{card_id}`"
            ));
        };
        let rule_id = crate::require_non_empty_json_str(result, "ruleId", "cards.sarif result")?;
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
        crate::require_boundary_text(result_boundary, "cards.sarif result properties")?;
        crate::json_array_at(result, "/properties/verifyCommands", "cards.sarif result")?;
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
    crate::require_boundary_text(sarif_boundary, "cards.sarif")?;
    let sarif_scope = sarif
        .pointer("/runs/0/properties/scope")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "cards.sarif is missing /runs/0/properties/scope".to_string())?;
    require_expected_value(sarif_scope, scope, "cards.sarif /runs/0/properties/scope")?;
    Ok(())
}

fn check_comment_plan_artifact(
    dir: &Path,
    card_count: usize,
    card_ids: &BTreeSet<String>,
    card_projections: &BTreeMap<String, CardProjection>,
) -> Result<(), String> {
    let comment_plan_path = dir.join("comment-plan.json");
    let comment_plan = crate::parse_json_file(&comment_plan_path)?;
    crate::require_json_str(&comment_plan, "schema_version", "0.1", "comment-plan.json")?;
    crate::require_json_str(&comment_plan, "mode", "plan_only", "comment-plan.json")?;
    crate::require_json_str(&comment_plan, "policy", "advisory", "comment-plan.json")?;
    crate::require_json_array(&comment_plan, "comments", "comment-plan.json")?;
    let comments = crate::json_array_at(&comment_plan, "/comments", "comment-plan.json")?;
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
        crate::json_array_at(comment, "/witness_routes", "comment-plan.json comment")?;
        crate::json_array_at(comment, "/verify_commands", "comment-plan.json comment")?;
        let Some(body) = comment.get("body").and_then(serde_json::Value::as_str) else {
            return Err("comment-plan.json comment is missing body".to_string());
        };
        require_text_mentions_only_known_card_ids(body, &comment_plan_path, &card_ids)?;
        require_comment_body_boundary(body)?;
        let body_word_count = body.split_whitespace().count();
        if body_word_count > COMMENT_PLAN_BODY_WORD_LIMIT {
            return Err(format!(
                "comment-plan.json comment body has {body_word_count} word(s), expected at most {COMMENT_PLAN_BODY_WORD_LIMIT}"
            ));
        }
        let class_name =
            crate::require_non_empty_json_str(comment, "class", "comment-plan.json comment")?;
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
        crate::require_non_empty_json_str(comment, "priority", "comment-plan.json comment")?;
        crate::require_non_empty_json_str(comment, "confidence", "comment-plan.json comment")?;
        crate::require_non_empty_json_str(comment, "operation", "comment-plan.json comment")?;
        crate::require_non_empty_json_str(
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
            crate::require_non_empty_json_str(comment, "next_action", "comment-plan.json comment")?;
        let selection_reason = crate::require_non_empty_json_str(
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
        let selection_reason_code = crate::require_non_empty_json_str(
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
        let actionability = crate::require_non_empty_json_str(
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
            crate::require_non_empty_json_str(comment, "relevance", "comment-plan.json comment")?;
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
        crate::require_boundary_text(comment_boundary, "comment-plan.json comment")?;
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
            let actionability = crate::require_non_empty_json_str(
                card,
                "actionability",
                "comment-plan.json not_selected",
            )?;
            require_expected_value(
                actionability,
                expected_actionability(&card_projection.class_name),
                "comment-plan.json not_selected actionability",
            )?;
            let relevance = crate::require_non_empty_json_str(
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
            let reason = crate::require_non_empty_json_str(
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
            let reason_code = crate::require_non_empty_json_str(
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
    crate::require_boundary_text(comment_boundary, "comment-plan.json")?;
    if card_count == 0 {
        let no_changed = comment_plan
            .get("no_changed_gaps")
            .ok_or_else(|| "comment-plan.json is missing no_changed_gaps".to_string())?;
        crate::require_json_str(
            no_changed,
            "message",
            "No changed unsafe-review gaps were found.",
            "comment-plan.json no_changed_gaps",
        )?;
        let limitation = crate::require_non_empty_json_str(
            no_changed,
            "limitation",
            "comment-plan.json no_changed_gaps",
        )?;
        if !crate::text_contains_ignore_ascii_case(limitation, "unsafe site executed") {
            return Err(
                "comment-plan.json no_changed_gaps.limitation must mention unsafe site execution"
                    .to_string(),
            );
        }
    }
    require_comment_plan_summary(&comment_plan, comments.len(), not_selected_card_ids.len())?;
    Ok(())
}
