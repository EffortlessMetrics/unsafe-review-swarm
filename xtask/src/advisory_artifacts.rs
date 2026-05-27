use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

struct AdvisoryArtifactSummary {
    card_ids: BTreeSet<String>,
    card_projections: BTreeMap<String, CardProjection>,
    card_count: usize,
}

struct CardProjection {
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
    verify_commands: Vec<String>,
    witness_routes: Vec<WitnessRouteProjection>,
}

struct WitnessRouteProjection {
    kind: String,
    reason: String,
    command: Option<String>,
}

const COMMENT_PLAN_BODY_WORD_LIMIT: usize = 220;

pub(crate) fn check_advisory_artifacts(dir: &Path) -> Result<(), String> {
    check_advisory_artifact_set(dir)?;
    println!("check-advisory-artifacts: ok ({})", dir.display());
    Ok(())
}

pub(crate) fn check_first_pr_artifacts(dir: &Path) -> Result<(), String> {
    let summary = check_advisory_artifact_set(dir)?;
    check_witness_plan_artifact(dir, summary.card_count, &summary.card_projections)?;
    check_lsp_artifact(dir, &summary.card_projections)?;
    check_github_summary_artifact(dir, summary.card_count, &summary.card_projections)?;
    check_first_pr_markdown_card_identity(dir, &summary.card_ids, &summary.card_projections)?;
    check_first_pr_artifact_overclaims(dir)?;

    println!("check-first-pr-artifacts: ok ({})", dir.display());
    Ok(())
}

const GITHUB_SUMMARY_WORD_LIMIT: usize = 600;

fn check_github_summary_artifact(
    dir: &Path,
    card_count: usize,
    card_projections: &BTreeMap<String, CardProjection>,
) -> Result<(), String> {
    let path = dir.join("github-summary.md");
    let text = super::read_to_string(&path)?;

    super::require_text_contains(&text, "## unsafe-review advisory summary", &path)?;
    super::require_text_contains(&text, &format!("- Review cards: {card_count}"), &path)?;
    super::require_text_contains(&text, "## Top card", &path)?;
    super::require_text_contains(&text, "static unsafe contract review", &path)?;
    super::require_text_contains(&text, "not memory-safety proof", &path)?;
    super::require_text_contains(&text, "not UB-free status", &path)?;
    super::require_text_contains(&text, "not Miri-clean status", &path)?;
    super::require_text_contains(&text, "not site-execution proof", &path)?;
    super::require_text_contains(
        &text,
        "Full advisory bundle (cards.json, pr-summary.md, github-summary.md, cards.sarif, comment-plan.json, witness-plan.md, lsp.json)",
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
    let mut top_card_next_action = None;

    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("- ID: `") {
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
        } else if let Some(next_action) = trimmed.strip_prefix("- Next action: ") {
            top_card_next_action = Some(next_action.to_string());
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
    )
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
    let card_projections = advisory_card_projections(&cards)?;
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
    let sarif_rule_ids = sarif_rule_ids(&sarif)?;
    let card_class_names = card_projections
        .values()
        .map(|projection| projection.class_name.as_str())
        .collect::<BTreeSet<_>>();
    for class_name in card_class_names {
        if !sarif_rule_ids.contains(class_name) {
            return Err(format!(
                "cards.sarif is missing rule id `{class_name}` for cards.json class"
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
        super::json_array_at(result, "/properties/verifyCommands", "cards.sarif result")?;
    }
    for card_id in &card_ids {
        if !sarif_card_ids.contains(card_id) {
            return Err(format!("cards.sarif results missing card id `{card_id}`"));
        }
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
        let next_action =
            super::require_non_empty_json_str(comment, "next_action", "comment-plan.json comment")?;
        let selection_reason = super::require_non_empty_json_str(
            comment,
            "selection_reason",
            "comment-plan.json comment",
        )?;
        require_expected_value(
            selection_reason,
            expected_selection_reason(card_projection),
            "comment-plan.json comment selection_reason",
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
            require_expected_value(
                reason,
                expected_non_selection_reason(card_projection, comments.len()),
                "comment-plan.json not_selected reason",
            )?;
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
        card_projections,
        card_count,
    })
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
                        Ok::<WitnessRouteProjection, String>(WitnessRouteProjection {
                            kind,
                            reason,
                            command,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?
            .unwrap_or_default();
        projections.insert(
            id,
            CardProjection {
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
                verify_commands,
                witness_routes,
            },
        );
    }
    Ok(projections)
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
    }
    Ok(())
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

fn expected_non_selection_reason(card: &CardProjection, planned_count: usize) -> &'static str {
    if !class_is_actionable(&card.class_name) {
        "class not eligible for inline comments"
    } else if card.operation_family == "unknown" {
        "operation family unknown"
    } else if matches!(card.confidence.as_str(), "low" | "unknown") {
        "confidence below inline comment threshold"
    } else if !(card.priority == "high" || card.confidence == "high") {
        "priority/confidence below inline comment threshold"
    } else if planned_count >= 3 {
        "comment-plan max of three candidates reached"
    } else {
        "not selected by current inline comment policy"
    }
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
    card_projections: &BTreeMap<String, CardProjection>,
) -> Result<(), String> {
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
) -> Result<(), String> {
    let pr_summary_path = dir.join("pr-summary.md");
    let pr_summary = super::read_to_string(&pr_summary_path)?;
    require_text_mentions_all_card_ids(&pr_summary, &pr_summary_path, card_ids)?;
    require_markdown_top_card_projection(&pr_summary, &pr_summary_path, card_projections)?;

    let witness_plan_path = dir.join("witness-plan.md");
    let witness_plan = super::read_to_string(&witness_plan_path)?;
    require_witness_plan_headings_known(&witness_plan, &witness_plan_path, card_ids)?;
    require_text_mentions_all_card_ids(&witness_plan, &witness_plan_path, card_ids)
}

fn check_lsp_artifact(
    dir: &Path,
    card_projections: &BTreeMap<String, CardProjection>,
) -> Result<(), String> {
    let path = dir.join("lsp.json");
    let lsp = super::parse_json_file(&path)?;
    let card_ids = card_projections.keys().cloned().collect::<BTreeSet<_>>();
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
        check_lsp_diagnostic_evidence(diagnostic)?;
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
        super::require_non_empty_json_str(action, "title", "lsp.json code_action")?;
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
        let arguments = super::json_array_at(action, "/arguments", "lsp.json code_action")?;
        check_lsp_code_action_payload(action, action_card_id, command, &card_ids, arguments)?;
        if action.get("edit").is_some() || action.get("workspace_edit").is_some() {
            return Err("lsp.json code_action must not contain source edits".to_string());
        }
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
    require_projected_string_array(diagnostic, "hazards", &card.hazards, "lsp.json diagnostic")
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
