use crate::domain::{Confidence, OperationFamily, Priority, ReviewCard, ReviewClass};

const PLAN_BOUNDARY: &str = "Plan boundary: artifact-only inline comment candidate; unsafe-review did not post this comment, run witnesses, or make a policy decision.";
pub(super) const OPERATION_FAMILY_BUDGET_REASON: &str =
    "covered by selected family/obligation sibling";
pub(super) const MAX_COMMENT_BUDGET_REASON: &str = "comment-plan max of three candidates reached";

const SELECTED_HIGH_CONFIDENCE_REASON: &str = "actionable high-confidence review card";
const SELECTED_HIGH_PRIORITY_REASON: &str = "actionable high-priority review card";
const NOT_SELECTED_OUTSIDE_CHANGED_HUNK_REASON: &str = "outside changed hunk";
const NOT_SELECTED_CLASS_INELIGIBLE_REASON: &str = "class not eligible for inline comments";
const NOT_SELECTED_UNKNOWN_FAMILY_REASON: &str = "operation family unknown";
const NOT_SELECTED_CONFIDENCE_REASON: &str = "confidence below inline comment threshold";
const NOT_SELECTED_PRIORITY_CONFIDENCE_REASON: &str =
    "priority/confidence below inline comment threshold";
const NOT_SELECTED_POLICY_FALLBACK_REASON: &str = "not selected by current inline comment policy";

pub(super) fn should_plan_comment(card: &ReviewCard) -> bool {
    card.site.changed
        && card.class.is_actionable()
        && !matches!(card.operation.family, OperationFamily::Unknown)
        && (matches!(card.priority, Priority::High) || matches!(card.confidence, Confidence::High))
        && !matches!(card.confidence, Confidence::Low | Confidence::Unknown)
}

pub(super) fn non_selection_reason(card: &ReviewCard) -> &'static str {
    if !card.site.changed {
        NOT_SELECTED_OUTSIDE_CHANGED_HUNK_REASON
    } else if !card.class.is_actionable() {
        NOT_SELECTED_CLASS_INELIGIBLE_REASON
    } else if matches!(card.operation.family, OperationFamily::Unknown) {
        NOT_SELECTED_UNKNOWN_FAMILY_REASON
    } else if matches!(card.confidence, Confidence::Low | Confidence::Unknown) {
        NOT_SELECTED_CONFIDENCE_REASON
    } else if !(matches!(card.priority, Priority::High)
        || matches!(card.confidence, Confidence::High))
    {
        NOT_SELECTED_PRIORITY_CONFIDENCE_REASON
    } else {
        NOT_SELECTED_POLICY_FALLBACK_REASON
    }
}

pub(super) fn selection_reason(card: &ReviewCard) -> &'static str {
    if matches!(card.confidence, Confidence::High) {
        SELECTED_HIGH_CONFIDENCE_REASON
    } else {
        SELECTED_HIGH_PRIORITY_REASON
    }
}

/// Transparent reviewer-noise control signal, never a policy gate.
///
/// `relevance` summarizes the priority + confidence signal that already
/// drives selection so reviewers can sort the inline comment plan without
/// having to re-derive it. It is informational only: skipping a `medium`
/// relevance card does not change unsafe-review's analysis or the trust
/// boundary.
pub(super) fn relevance(card: &ReviewCard) -> &'static str {
    let high_priority = matches!(card.priority, Priority::High);
    let high_confidence = matches!(card.confidence, Confidence::High);
    let low_confidence = matches!(card.confidence, Confidence::Low | Confidence::Unknown);

    if low_confidence {
        "low"
    } else if high_priority && high_confidence {
        "high"
    } else if high_priority || high_confidence {
        "medium"
    } else {
        "low"
    }
}

pub(super) fn actionability(card: &ReviewCard) -> &'static str {
    match &card.class {
        ReviewClass::GuardMissing => "specific_guard_missing",
        ReviewClass::ContractMissing => "specific_contract_missing",
        ReviewClass::GuardedUnwitnessed
        | ReviewClass::ReachableUnwitnessed
        | ReviewClass::RequiresLoom
        | ReviewClass::RequiresSanitizer
        | ReviewClass::RequiresKaniOrCrux
        | ReviewClass::MiriUnsupported => "specific_witness_missing",
        ReviewClass::WitnessMismatch => "specific_receipt_missing",
        ReviewClass::UnsafeUnreached => "specific_reach_missing",
        ReviewClass::StaticUnknown => "human_review_only",
        _ => "not_actionable",
    }
}

pub(super) fn comment_body(card: &ReviewCard) -> String {
    let mut body = String::new();
    body.push_str(&format!(
        "`unsafe-review` found `{}` for `{}` (`{}`).\n\n",
        card.class.as_str(),
        one_line(&card.operation.expression),
        card.operation.family.as_str()
    ));
    body.push_str(&format!("Missing evidence: {}\n\n", missing_summary(card)));
    body.push_str(&format!("Next action: {}\n\n", card.next_action.summary));
    if let Some(route) = card.routes.first() {
        body.push_str(&format!(
            "Witness route: `{}` because {}.\n\n",
            route.kind.as_str(),
            route.reason
        ));
    }
    if let Some(command) = card.next_action.verify_commands.first() {
        body.push_str(&format!("Verify command: `{command}`\n\n"));
    }
    body.push_str(PLAN_BOUNDARY);
    body.push_str("\n\n");
    body.push_str("Trust boundary: static unsafe contract review only; not memory-safety proof, not UB-free status, and not a Miri result unless a witness receipt is attached.");
    body
}

fn missing_summary(card: &ReviewCard) -> String {
    if card.missing.is_empty() {
        return "No missing evidence recorded".to_string();
    }
    card.missing
        .iter()
        .map(|missing| missing.message.as_str())
        .collect::<Vec<_>>()
        .join("; ")
}

fn one_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}
