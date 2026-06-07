use crate::domain::coverage::{Coverage, WitnessReceiptCoverage};
use crate::domain::{Confidence, OperationFamily, Priority, ReviewCard, ReviewClass};
use crate::output::REVIEWCARD_TRUST_BOUNDARY;
use crate::output::confirmation::{
    build_this_first, confirmation_step, hypothesis_to_confirm, minimal_repro_comment,
};

const PLAN_BOUNDARY: &str = "Plan boundary: artifact-only inline comment candidate; unsafe-review did not post this comment, run witnesses, or make a policy decision.";

#[derive(Clone, Copy)]
pub(super) struct ReviewBudgetReason {
    pub(super) code: &'static str,
    pub(super) message: &'static str,
}

pub(super) const OPERATION_FAMILY_BUDGET_REASON: ReviewBudgetReason = ReviewBudgetReason {
    code: "covered_by_selected_family_obligation",
    message: "covered by selected family/obligation sibling",
};
pub(super) const MAX_COMMENT_BUDGET_REASON: ReviewBudgetReason = ReviewBudgetReason {
    code: "budget_exhausted",
    message: "comment-plan max of three candidates reached",
};

// Selection reasons referencing coverage gap (SPEC-0032).
const SELECTED_CONTRACT_MISSING_HIGH_CONFIDENCE: ReviewBudgetReason = ReviewBudgetReason {
    code: "top_actionable_card",
    message: "contract_coverage: missing — actionable high-confidence card",
};
const SELECTED_CONTRACT_MISSING_HIGH_PRIORITY: ReviewBudgetReason = ReviewBudgetReason {
    code: "top_actionable_card",
    message: "contract_coverage: missing — actionable high-priority card",
};
const SELECTED_GUARD_MISSING_HIGH_CONFIDENCE: ReviewBudgetReason = ReviewBudgetReason {
    code: "top_actionable_card",
    message: "guard_coverage: missing — actionable high-confidence card",
};
const SELECTED_GUARD_MISSING_HIGH_PRIORITY: ReviewBudgetReason = ReviewBudgetReason {
    code: "top_actionable_card",
    message: "guard_coverage: missing — actionable high-priority card",
};
const SELECTED_GUARD_WEAK_HIGH_CONFIDENCE: ReviewBudgetReason = ReviewBudgetReason {
    code: "top_actionable_card",
    message: "guard_coverage: weak — actionable high-confidence card",
};
const SELECTED_GUARD_WEAK_HIGH_PRIORITY: ReviewBudgetReason = ReviewBudgetReason {
    code: "top_actionable_card",
    message: "guard_coverage: weak — actionable high-priority card",
};
const SELECTED_TEST_REACH_MISSING_HIGH_CONFIDENCE: ReviewBudgetReason = ReviewBudgetReason {
    code: "top_actionable_card",
    message: "test_reach_coverage: missing — actionable high-confidence card",
};
const SELECTED_TEST_REACH_MISSING_HIGH_PRIORITY: ReviewBudgetReason = ReviewBudgetReason {
    code: "top_actionable_card",
    message: "test_reach_coverage: missing — actionable high-priority card",
};
const SELECTED_TEST_REACH_WEAK_HIGH_CONFIDENCE: ReviewBudgetReason = ReviewBudgetReason {
    code: "top_actionable_card",
    message: "test_reach_coverage: weak — actionable high-confidence card",
};
const SELECTED_TEST_REACH_WEAK_HIGH_PRIORITY: ReviewBudgetReason = ReviewBudgetReason {
    code: "top_actionable_card",
    message: "test_reach_coverage: weak — actionable high-priority card",
};
const SELECTED_WITNESS_RECEIPT_MISSING_HIGH_CONFIDENCE: ReviewBudgetReason = ReviewBudgetReason {
    code: "top_actionable_card",
    message: "witness_receipt_coverage: missing — actionable high-confidence card",
};
const SELECTED_WITNESS_RECEIPT_MISSING_HIGH_PRIORITY: ReviewBudgetReason = ReviewBudgetReason {
    code: "top_actionable_card",
    message: "witness_receipt_coverage: missing — actionable high-priority card",
};
const NOT_SELECTED_OUTSIDE_CHANGED_HUNK_REASON: ReviewBudgetReason = ReviewBudgetReason {
    code: "outside_changed_hunk",
    message: "outside changed hunk",
};
const NOT_SELECTED_CLASS_INELIGIBLE_REASON: ReviewBudgetReason = ReviewBudgetReason {
    code: "human_deep_review_only",
    message: "class not eligible for inline comments",
};
const NOT_SELECTED_UNKNOWN_FAMILY_REASON: ReviewBudgetReason = ReviewBudgetReason {
    code: "human_deep_review_only",
    message: "operation family unknown",
};
const NOT_SELECTED_CONFIDENCE_REASON: ReviewBudgetReason = ReviewBudgetReason {
    code: "lower_relevance",
    message: "confidence below inline comment threshold",
};
const NOT_SELECTED_PRIORITY_CONFIDENCE_REASON: ReviewBudgetReason = ReviewBudgetReason {
    code: "lower_relevance",
    message: "priority/confidence below inline comment threshold",
};
const NOT_SELECTED_POLICY_FALLBACK_REASON: ReviewBudgetReason = ReviewBudgetReason {
    code: "not_selected_by_policy",
    message: "not selected by current inline comment policy",
};

pub(super) fn should_plan_comment(card: &ReviewCard) -> bool {
    card.site.changed
        && card.class.is_actionable()
        && !matches!(card.operation.family, OperationFamily::Unknown)
        && (matches!(card.priority, Priority::High) || matches!(card.confidence, Confidence::High))
        && !matches!(card.confidence, Confidence::Low | Confidence::Unknown)
}

pub(super) fn non_selection_reason(card: &ReviewCard) -> ReviewBudgetReason {
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

/// Derive the primary coverage gap for a card (SPEC-0032).
///
/// Returns a string of the form `"<slot>: <state>"` naming the weak or missing
/// SPEC-0029 coverage slot that makes this card worth surfacing. Priority order:
/// `contract_coverage` → `guard_coverage` → `test_reach_coverage` →
/// `witness_receipt_coverage`. Falls back to `"witness_receipt_coverage: missing"`
/// when all slots appear present (should not happen for actionable cards).
pub(super) fn coverage_gap(card: &ReviewCard) -> String {
    let block = card.coverage_block();
    if block.contract_coverage != Coverage::Present {
        return format!("contract_coverage: {}", block.contract_coverage.as_str());
    }
    if block.guard_coverage != Coverage::Present {
        return format!("guard_coverage: {}", block.guard_coverage.as_str());
    }
    if block.test_reach_coverage != Coverage::Present {
        return format!(
            "test_reach_coverage: {}",
            block.test_reach_coverage.as_str()
        );
    }
    if block.witness_receipt_coverage != WitnessReceiptCoverage::Present {
        return format!(
            "witness_receipt_coverage: {}",
            block.witness_receipt_coverage.as_str()
        );
    }
    // Fallback: all slots appear present on an actionable card.
    "witness_receipt_coverage: missing".to_string()
}

pub(super) fn selection_reason(card: &ReviewCard) -> ReviewBudgetReason {
    let block = card.coverage_block();
    let high_confidence = matches!(card.confidence, Confidence::High);
    // Select the gap-specific reason matching the primary coverage gap.
    if block.contract_coverage != Coverage::Present {
        if high_confidence {
            return SELECTED_CONTRACT_MISSING_HIGH_CONFIDENCE;
        }
        return SELECTED_CONTRACT_MISSING_HIGH_PRIORITY;
    }
    if block.guard_coverage == Coverage::Missing {
        if high_confidence {
            return SELECTED_GUARD_MISSING_HIGH_CONFIDENCE;
        }
        return SELECTED_GUARD_MISSING_HIGH_PRIORITY;
    }
    if block.guard_coverage == Coverage::Weak {
        if high_confidence {
            return SELECTED_GUARD_WEAK_HIGH_CONFIDENCE;
        }
        return SELECTED_GUARD_WEAK_HIGH_PRIORITY;
    }
    if block.test_reach_coverage == Coverage::Weak {
        if high_confidence {
            return SELECTED_TEST_REACH_WEAK_HIGH_CONFIDENCE;
        }
        return SELECTED_TEST_REACH_WEAK_HIGH_PRIORITY;
    }
    if block.test_reach_coverage != Coverage::Present {
        if high_confidence {
            return SELECTED_TEST_REACH_MISSING_HIGH_CONFIDENCE;
        }
        return SELECTED_TEST_REACH_MISSING_HIGH_PRIORITY;
    }
    // Witness receipt gap or fallback.
    if high_confidence {
        SELECTED_WITNESS_RECEIPT_MISSING_HIGH_CONFIDENCE
    } else {
        SELECTED_WITNESS_RECEIPT_MISSING_HIGH_PRIORITY
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
    body.push_str(&format!("Proof path: `{}`.\n\n", card.proof_path.as_str()));
    body.push_str(&format!(
        "Hypothesis to confirm: {}.\n\n",
        hypothesis_to_confirm(card)
    ));
    body.push_str(&format!("Next action: {}\n\n", card.next_action.summary));
    body.push_str(&format!(
        "Build/run this first: {}\n\n",
        build_this_first(card).summary
    ));
    body.push_str(&format!(
        "Minimal repro cue: {}.\n\n",
        minimal_repro_comment(card)
    ));
    body.push_str(&format!(
        "Confirmation step: {}\n\n",
        confirmation_step(card)
    ));
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
    body.push_str("Trust boundary: ");
    body.push_str(REVIEWCARD_TRUST_BOUNDARY);
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
