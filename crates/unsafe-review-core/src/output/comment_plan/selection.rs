use crate::domain::coverage::{Coverage, WitnessReceiptCoverage};
use crate::domain::{Confidence, Priority, ReviewCard, ReviewClass, UnsafeSiteKind};
use crate::output::REVIEWCARD_TRUST_BOUNDARY;
use crate::output::confirmation::{build_this_first, confirmation_step, hypothesis_to_confirm};

/// Importance rank used to select the best candidates within the comment-plan
/// budget (SPEC-0022 §5, SPEC-0032).
///
/// Lower numeric value = higher importance. Candidates are sorted ascending
/// before the family/obligation dedup and budget cap are applied, so the
/// highest-importance unique card per family fills each budget slot.
///
/// Ranking key (descending importance):
/// 1. Priority: `High` first (rank 0), all others rank 1.
/// 2. Gap severity:
///    `contract_coverage: missing` = 0
///    `guard_coverage: missing`    = 1
///    `guard_coverage: weak`       = 2
///    `test_reach_coverage: weak`  = 3
///    `test_reach_coverage: missing` = 4
///    `witness_receipt_coverage: missing` = 5
/// 3. Confidence: `High` first (rank 0), all others rank 1.
/// 4. Stable tiebreak: `(file, line)` ascending — matches the global card
///    order from `sort_cards`, so ties produce deterministic output.
///
/// This ranking is purely about which coverage gaps to surface first. It is
/// not a severity claim, proof, or policy gate.
pub(super) fn importance_rank(card: &ReviewCard) -> (u8, u8, u8, &std::path::Path, usize) {
    let priority_rank: u8 = if matches!(card.priority, Priority::High) {
        0
    } else {
        1
    };
    let gap_rank: u8 = gap_severity_rank(card);
    let confidence_rank: u8 = if matches!(card.confidence, Confidence::High) {
        0
    } else {
        1
    };
    (
        priority_rank,
        gap_rank,
        confidence_rank,
        card.site.location.file.as_path(),
        card.site.location.line,
    )
}

/// Numeric rank for the primary coverage gap (lower = more severe / higher importance).
///
/// Priority order matches `selection_reason` and `coverage_gap`:
/// `contract_coverage: missing` → `guard_coverage: missing` →
/// `guard_coverage: weak` → `test_reach_coverage: weak` →
/// `test_reach_coverage: missing` → `witness_receipt_coverage: missing`.
fn gap_severity_rank(card: &ReviewCard) -> u8 {
    let block = card.coverage_block();
    if block.contract_coverage != Coverage::Present {
        return 0;
    }
    if block.guard_coverage == Coverage::Missing {
        return 1;
    }
    if block.guard_coverage == Coverage::Weak {
        return 2;
    }
    if block.test_reach_coverage == Coverage::Weak {
        return 3;
    }
    if block.test_reach_coverage != Coverage::Present {
        return 4;
    }
    // Witness receipt gap or fallback.
    5
}

const PLAN_BOUNDARY: &str = "Plan boundary: artifact-only inline comment candidate; unsafe-review did not post this comment, run witnesses, or make a policy decision.";

/// Maximum word count for a comment-plan comment body.
///
/// This bound is enforced by the producer (`comment_body`) and cross-checked by the
/// xtask `check-first-pr-artifacts` gate. Single-sourced here so producer and gate
/// cannot drift. Word count is computed by `str::split_whitespace().count()`.
pub const COMMENT_BODY_WORD_LIMIT: usize = 220;

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
const NOT_SELECTED_UNSAFE_DECLARATION_REASON: ReviewBudgetReason = ReviewBudgetReason {
    code: "human_deep_review_only",
    message: "unsafe declaration is not selected for inline comments",
};
/// Applied to an owner/declaration/fallback-family card when a more-specific
/// operation card from the same changed region is already present. Replaces the
/// generic `human_deep_review_only` reason for that sub-case so reviewers
/// understand the owner card is grouped behind the operation card, not simply
/// excluded for its broad review route.
pub(super) const NOT_SELECTED_COVERED_BY_OPERATION_CARD_REASON: ReviewBudgetReason =
    ReviewBudgetReason {
        code: "covered_by_specific_operation_card",
        message: "owner-contract obligation covered by a more-specific operation card at the same region",
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CommentSurfacingDisposition {
    InlineCandidate,
    UnsafeDeclaration,
    FallbackUnsafeSite,
}

impl CommentSurfacingDisposition {
    fn allows_inline_comment(self) -> bool {
        matches!(self, Self::InlineCandidate)
    }

    fn is_owner_or_fallback(self) -> bool {
        matches!(self, Self::UnsafeDeclaration | Self::FallbackUnsafeSite)
    }
}

fn comment_surfacing_disposition(card: &ReviewCard) -> CommentSurfacingDisposition {
    match &card.site.kind {
        UnsafeSiteKind::UnsafeFn | UnsafeSiteKind::UnsafeTrait => {
            CommentSurfacingDisposition::UnsafeDeclaration
        }
        UnsafeSiteKind::UnsafeBlock | UnsafeSiteKind::UnsafeImpl => {
            CommentSurfacingDisposition::FallbackUnsafeSite
        }
        UnsafeSiteKind::UnsafeImplSend
        | UnsafeSiteKind::UnsafeImplSync
        | UnsafeSiteKind::ExternBlock
        | UnsafeSiteKind::FfiCall
        | UnsafeSiteKind::StaticMut
        | UnsafeSiteKind::Operation => CommentSurfacingDisposition::InlineCandidate,
    }
}

pub(super) fn should_plan_comment(card: &ReviewCard) -> bool {
    card.site.changed
        && card.class.is_actionable()
        && comment_surfacing_disposition(card).allows_inline_comment()
        && (matches!(card.priority, Priority::High) || matches!(card.confidence, Confidence::High))
        && !matches!(card.confidence, Confidence::Low | Confidence::Unknown)
}

pub(super) fn non_selection_reason(card: &ReviewCard) -> ReviewBudgetReason {
    let surfacing = comment_surfacing_disposition(card);
    if !card.site.changed {
        NOT_SELECTED_OUTSIDE_CHANGED_HUNK_REASON
    } else if !card.class.is_actionable() {
        NOT_SELECTED_CLASS_INELIGIBLE_REASON
    } else if matches!(surfacing, CommentSurfacingDisposition::UnsafeDeclaration) {
        NOT_SELECTED_UNSAFE_DECLARATION_REASON
    } else if matches!(surfacing, CommentSurfacingDisposition::FallbackUnsafeSite) {
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

/// Determine whether an owner/declaration/fallback-site card's changed region
/// is already covered by at least one concrete operation card in `all_cards`.
///
/// "Same region" is defined as the same file and inferred owner/declaration
/// context. Owner cards (`unsafe fn` sites) span an entire function body and the
/// specific operation cards they generate are located inside that function, so
/// owner context prevents unrelated declarations in the same file from being
/// marked as covered.
///
/// This function is used only for the `not_selected` non-selection reason; it
/// does not change card identity, evidence counts, or structured artifacts.
pub(super) fn owner_card_covered_by_specific_operation(
    owner_card: &ReviewCard,
    all_cards: &[ReviewCard],
) -> bool {
    if !comment_surfacing_disposition(owner_card).is_owner_or_fallback() {
        return false;
    }
    let owner_file = &owner_card.site.location.file;
    let owner_context = owner_card.site.owner.as_deref();
    all_cards.iter().any(|other| {
        comment_surfacing_disposition(other).allows_inline_comment()
            && other.site.changed
            && &other.site.location.file == owner_file
            && same_owner_context(owner_context, other.site.owner.as_deref())
    })
}

fn same_owner_context(owner: Option<&str>, other: Option<&str>) -> bool {
    matches!((owner, other), (Some(owner), Some(other)) if !owner.trim().is_empty() && owner == other)
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

/// Render the inline comment body for a planned comment.
///
/// The body is bounded to [`COMMENT_BODY_WORD_LIMIT`] words so the producer
/// never emits a body the `check-first-pr-artifacts` gate would reject.
///
/// Sections are ordered by essentialness. The required sections — `Next
/// action` and `Trust boundary` — are always present. The `Minimal repro cue`
/// and `Witness route` sections are omitted from the body because their
/// information is already carried by the structured fields (`minimal_repro`,
/// `witness_routes`) in the surrounding JSON object and by `Build/run this
/// first` and `Verify command`. Dropping them saves ~50 words without losing
/// any actionable guidance or trust-boundary wording.
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
        "Confirmation step: {}\n\n",
        confirmation_step(card)
    ));
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
