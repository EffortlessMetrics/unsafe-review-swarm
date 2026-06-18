mod model;
mod render;
mod selection;

pub(crate) use render::render;
pub use selection::COMMENT_BODY_WORD_LIMIT;

use crate::api::AnalyzeOutput;
use crate::domain::{CardId, CommentPlanStatus};
use std::collections::HashMap;

/// Compute the comment-plan selection status for every card in `output`.
///
/// Reuses the same eligibility gate (`should_plan_comment`) and budget/dedup
/// logic as [`model::CommentPlan`] — the two cannot drift because they share
/// the same `selection` module functions.  The returned map covers every card
/// in `output.cards` with one of:
///
/// - `Selected` — the card was chosen for an inline comment slot.
/// - `NotSelected` — the card was eligible but displaced by the budget cap or
///   family/obligation dedup.
/// - `NotEligible` — the card failed the `should_plan_comment` eligibility
///   gate (e.g. unchanged site, non-actionable class, human-review-only
///   surfacing disposition, or low confidence).
///
/// This function is the single source of truth for `comment_plan_status` in the
/// coverage block (SPEC-0029 / SPEC-0032).  `json::render` and
/// `agent::render_with_output` call it so that `cards.json` and agent packets
/// report the same status as `comment-plan.json`.
pub(crate) fn card_statuses(output: &AnalyzeOutput) -> HashMap<CardId, CommentPlanStatus> {
    use self::model::comment_budget_key;
    use self::selection::{importance_rank, should_plan_comment};
    use std::collections::BTreeSet;

    // Must match MAX_PLANNED_COMMENTS in model.rs — kept in sync by the
    // fixture_card_goldens drift test: if these diverge, the goldens will
    // disagree with comment-plan.json.
    const MAX_PLANNED_COMMENTS: usize = 3;

    let mut statuses: HashMap<CardId, CommentPlanStatus> =
        HashMap::with_capacity(output.cards.len());

    let (mut eligible, ineligible): (
        Vec<&crate::domain::ReviewCard>,
        Vec<&crate::domain::ReviewCard>,
    ) = output
        .cards
        .iter()
        .partition(|card| should_plan_comment(card));
    eligible.sort_by(|a, b| {
        importance_rank(a)
            .cmp(&importance_rank(b))
            .then_with(|| a.id.0.cmp(&b.id.0))
    });

    let mut selected_budget_keys: BTreeSet<String> = BTreeSet::new();
    let mut selected_count = 0usize;

    for card in eligible {
        let budget_key = comment_budget_key(card);
        if selected_budget_keys.contains(&budget_key) {
            statuses.insert(card.id.clone(), CommentPlanStatus::NotSelected);
        } else if selected_count < MAX_PLANNED_COMMENTS {
            selected_budget_keys.insert(budget_key);
            selected_count += 1;
            statuses.insert(card.id.clone(), CommentPlanStatus::Selected);
        } else {
            statuses.insert(card.id.clone(), CommentPlanStatus::NotSelected);
        }
    }
    for card in ineligible {
        statuses.insert(card.id.clone(), CommentPlanStatus::NotEligible);
    }
    statuses
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{
        AnalysisMode, AnalyzeInput, AnalyzeOutput, DiffSource, PolicyMode, Scope, analyze,
    };
    use crate::domain::{Confidence, ContractEvidence, OperationFamily, Priority, ReviewClass};
    use crate::output::{NO_CHANGED_GAPS_LIMITATION, NO_CHANGED_GAPS_MESSAGE};
    use std::path::PathBuf;

    #[test]
    fn comment_plan_projects_high_signal_actionable_cards() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let value = parse_json(&render(&output))?;

        assert_eq!(value["mode"], "plan_only");
        assert_eq!(value["comments"].as_array().map_or(0, Vec::len), 1);
        assert_review_budget_summary(&value, 1, 0)?;
        assert_eq!(value["comments"][0]["class"], "guard_missing");
        assert_eq!(value["comments"][0]["path"], "src/lib.rs");
        assert_eq!(value["comments"][0]["changed_line"], true);
        assert_eq!(
            value["comments"][0]["operation"],
            "unsafe { ptr.cast::<Header>().read() }"
        );
        assert_eq!(value["comments"][0]["operation_family"], "raw_pointer_read");
        assert_eq!(
            value["comments"][0]["hypothesis_to_confirm"],
            "static `guard_missing` ReviewCard for `unsafe { ptr.cast::<Header>().read() }`; confirm with external evidence before treating it as observed runtime behavior"
        );
        assert_eq!(
            value["comments"][0]["next_action"],
            "Add or expose local guards for these `raw_pointer_read` safety obligations: (1) pointer is live and dereferenceable for the accessed type, (2) pointer is aligned for the accessed type, (3) memory is initialized for the accessed type, (4) access remains inside one live allocation."
        );
        assert_eq!(
            value["comments"][0]["build_this_first"]["kind"],
            "verify_command"
        );
        assert_eq!(
            value["comments"][0]["build_this_first"]["command"],
            "cargo +nightly miri test read_header"
        );
        assert_eq!(
            value["comments"][0]["build_this_first"]["route_kind"],
            "miri"
        );
        assert!(
            value["comments"][0]["build_this_first"]["summary"]
                .as_str()
                .unwrap_or("")
                .contains("Build/run `cargo +nightly miri test read_header` first")
        );
        assert_eq!(
            value["comments"][0]["minimal_repro"]["kind"],
            "verify_command"
        );
        assert_eq!(
            value["comments"][0]["minimal_repro"]["command"],
            "cargo +nightly miri test read_header"
        );
        assert_eq!(value["comments"][0]["minimal_repro"]["route_kind"], "miri");
        assert!(
            value["comments"][0]["minimal_repro"]["steps"][0]
                .as_str()
                .unwrap_or("")
                .contains("Confirm ReviewCard")
        );
        assert!(
            value["comments"][0]["minimal_repro"]["limitation"]
                .as_str()
                .unwrap_or("")
                .contains("unsafe-review did not run this command")
        );
        assert_eq!(
            value["comments"][0]["confirmation_step"],
            "build/run `cargo +nightly miri test read_header` first, then attach a matching receipt if it confirms the route"
        );
        assert_eq!(
            value["comments"][0]["coverage_gap"],
            "guard_coverage: missing"
        );
        assert_eq!(value["comments"][0]["confirmation_state"], "pending");
        assert_eq!(
            value["comments"][0]["selection_reason"],
            "guard_coverage: missing — actionable high-priority card"
        );
        assert_eq!(
            value["comments"][0]["selection_reason_code"],
            "top_actionable_card"
        );
        assert_eq!(
            value["comments"][0]["actionability"],
            "specific_guard_missing"
        );
        assert_eq!(value["comments"][0]["agent_readiness"]["ready"], true);
        assert_eq!(
            value["comments"][0]["agent_readiness"]["state"],
            "ready_for_agent"
        );
        assert!(
            serde_json::to_string(&value["comments"][0]["agent_readiness"]["reasons"])
                .map_err(|err| format!("render readiness reasons failed: {err}"))?
                .contains("card-scoped allowed repairs")
        );
        assert_eq!(
            value["comments"][0]["repair_queue_buckets"],
            serde_json::json!(["repairable_by_guard", "requires_witness_receipt"])
        );
        assert_eq!(
            value["comments"][0]["repair_queue_bucket_reasons"],
            serde_json::json!(["guard_evidence_missing", "witness_receipt_missing"])
        );
        assert_eq!(
            value["comments"][0]["context_command"],
            format!("unsafe-review context {} --json", output.cards[0].id)
        );
        assert!(
            value["comments"][0]["trust_boundary"]
                .as_str()
                .unwrap_or("")
                .contains("not UB-free status")
        );
        assert_eq!(value["comments"][0]["witness_routes"][0]["kind"], "miri");
        assert!(
            value["comments"][0]["verify_commands"][0]
                .as_str()
                .unwrap_or("")
                .contains("cargo +nightly miri test read_header")
        );
        assert!(
            value["comments"][0]["body"]
                .as_str()
                .unwrap_or("")
                .contains("unsafe { ptr.cast::<Header>().read() }")
        );
        assert!(
            value["comments"][0]["body"]
                .as_str()
                .unwrap_or("")
                .contains("Verify command: `cargo +nightly miri test read_header`")
        );
        assert!(
            value["comments"][0]["body"]
                .as_str()
                .unwrap_or("")
                .contains("Hypothesis to confirm: static `guard_missing` ReviewCard")
        );
        assert!(
            value["comments"][0]["body"]
                .as_str()
                .unwrap_or("")
                .contains(
                    "Build/run this first: Build/run `cargo +nightly miri test read_header` first"
                )
        );
        assert!(
            value["comments"][0]["body"]
                .as_str()
                .unwrap_or("")
                .contains(
                    "Confirmation step: build/run `cargo +nightly miri test read_header` first"
                )
        );
        assert!(
            value["comments"][0]["body"]
                .as_str()
                .unwrap_or("")
                .contains("unsafe-review did not post this comment")
        );
        assert!(
            value["comments"][0]["body"]
                .as_str()
                .unwrap_or("")
                .contains("not a site-execution claim")
        );
        Ok(())
    }

    #[test]
    fn comment_plan_empty_output_has_no_comments() -> Result<(), String> {
        let output = fixture_output("safe_code_no_cards")?;
        let value = parse_json(&render(&output))?;

        assert_eq!(value["comments"].as_array().map_or(1, Vec::len), 0);
        assert!(value.get("not_selected").is_none());
        assert_review_budget_summary(&value, 0, 0)?;
        assert_eq!(value["no_changed_gaps"]["message"], NO_CHANGED_GAPS_MESSAGE);
        assert_eq!(
            value["no_changed_gaps"]["limitation"],
            NO_CHANGED_GAPS_LIMITATION
        );
        assert!(
            value["trust_boundary"]
                .as_str()
                .unwrap_or("")
                .contains("not UB-free status")
        );
        Ok(())
    }

    #[test]
    fn comment_plan_caps_planned_comments() -> Result<(), String> {
        let mut output = fixture_output("raw_pointer_alignment")?;
        let template = output
            .cards
            .first()
            .cloned()
            .ok_or_else(|| "fixture should emit a card".to_string())?;
        output.cards = (0..5)
            .map(|idx| {
                let mut card = template.clone();
                card.id.0 = format!("card-{idx}");
                card.operation.family = [
                    OperationFamily::RawPointerRead,
                    OperationFamily::StrFromUtf8Unchecked,
                    OperationFamily::GetUnchecked,
                    OperationFamily::MaybeUninitAssumeInit,
                    OperationFamily::VecSetLen,
                ][idx]
                    .clone();
                card
            })
            .collect();
        output.summary.cards = output.cards.len();
        output.summary.open_actionable_gaps = output.cards.len();

        let value = parse_json(&render(&output))?;

        assert_eq!(
            value["comments"]
                .as_array()
                .ok_or_else(|| "comments should be an array".to_string())?
                .len(),
            3
        );
        assert_eq!(
            value["not_selected"]
                .as_array()
                .ok_or_else(|| "not_selected should be an array".to_string())?
                .len(),
            2
        );
        assert_review_budget_summary(&value, 3, 2)?;
        assert_eq!(
            value["not_selected"][0]["reason"],
            "comment-plan max of three candidates reached"
        );
        assert_eq!(value["not_selected"][0]["reason_code"], "budget_exhausted");
        Ok(())
    }

    #[test]
    fn comment_plan_suppresses_duplicate_family_obligation_candidates() -> Result<(), String> {
        let mut output = fixture_output("raw_pointer_alignment")?;
        let template = output
            .cards
            .first()
            .cloned()
            .ok_or_else(|| "fixture should emit a card".to_string())?;
        output.cards = (0..3)
            .map(|idx| {
                let mut card = template.clone();
                card.id.0 = format!("raw-pointer-card-{idx}");
                card
            })
            .collect();
        output.summary.cards = output.cards.len();
        output.summary.open_actionable_gaps = output.cards.len();

        let value = parse_json(&render(&output))?;

        assert_eq!(value["comments"].as_array().map_or(0, Vec::len), 1);
        assert_eq!(value["comments"][0]["operation_family"], "raw_pointer_read");
        assert_eq!(value["not_selected"].as_array().map_or(0, Vec::len), 2);
        assert_review_budget_summary(&value, 1, 2)?;
        assert_eq!(
            value["not_selected"][0]["reason"],
            "covered by selected family/obligation sibling"
        );
        assert_eq!(
            value["not_selected"][0]["reason_code"],
            "covered_by_selected_family_obligation"
        );
        assert_eq!(
            value["not_selected"][1]["reason"],
            "covered by selected family/obligation sibling"
        );
        assert_eq!(
            value["not_selected"][1]["reason_code"],
            "covered_by_selected_family_obligation"
        );
        Ok(())
    }

    #[test]
    fn comment_plan_allows_same_family_with_different_missing_obligations() -> Result<(), String> {
        let mut output = fixture_output("raw_pointer_alignment")?;
        let template = output
            .cards
            .first()
            .cloned()
            .ok_or_else(|| "fixture should emit a card".to_string())?;
        let mut alignment = template.clone();
        alignment.id.0 = "raw-pointer-alignment-card".to_string();
        alignment
            .obligation_evidence
            .retain(|evidence| evidence.obligation.key == "alignment");
        let mut initialized = template;
        initialized.id.0 = "raw-pointer-initialized-card".to_string();
        initialized
            .obligation_evidence
            .retain(|evidence| evidence.obligation.key == "initialized");
        output.cards = vec![alignment, initialized];
        output.summary.cards = output.cards.len();
        output.summary.open_actionable_gaps = output.cards.len();

        let value = parse_json(&render(&output))?;

        assert_eq!(value["comments"].as_array().map_or(0, Vec::len), 2);
        assert_eq!(value["comments"][0]["operation_family"], "raw_pointer_read");
        assert_eq!(value["comments"][1]["operation_family"], "raw_pointer_read");
        assert!(value.get("not_selected").is_none());
        assert_review_budget_summary(&value, 2, 0)?;
        Ok(())
    }

    #[test]
    fn comment_plan_keeps_unchanged_cards_out_of_inline_budget() -> Result<(), String> {
        let mut output = fixture_output("raw_pointer_alignment")?;
        let card = output
            .cards
            .first_mut()
            .ok_or_else(|| "fixture should emit one card".to_string())?;
        card.site.changed = false;

        let value = parse_json(&render(&output))?;

        assert_eq!(value["comments"].as_array().map_or(1, Vec::len), 0);
        assert_eq!(value["not_selected"].as_array().map_or(0, Vec::len), 1);
        assert_review_budget_summary(&value, 0, 1)?;
        assert_eq!(value["not_selected"][0]["reason"], "outside changed hunk");
        assert_eq!(
            value["not_selected"][0]["reason_code"],
            "outside_changed_hunk"
        );
        assert_eq!(value["not_selected"][0]["changed_line"], false);
        assert_eq!(
            value["not_selected"][0]["actionability"],
            "specific_guard_missing"
        );
        assert_eq!(value["not_selected"][0]["relevance"], "medium");
        Ok(())
    }

    #[test]
    fn comment_plan_explains_card_present_but_not_selected() -> Result<(), String> {
        let output = fixture_output("ffi_sanitizer_route")?;
        let value = parse_json(&render(&output))?;

        assert_eq!(value["comments"].as_array().map_or(1, Vec::len), 0);
        assert_eq!(value["not_selected"].as_array().map_or(0, Vec::len), 1);
        assert_review_budget_summary(&value, 0, 1)?;
        assert_eq!(value["not_selected"][0]["class"], "miri_unsupported");
        assert_eq!(
            value["not_selected"][0]["operation"],
            "unsafe extern \"C\" {"
        );
        assert_eq!(value["not_selected"][0]["operation_family"], "ffi");
        assert_eq!(
            value["not_selected"][0]["next_action"],
            "Use sanitizer/cargo-careful or an explicit FFI boundary contract; Miri may not exercise this seam."
        );
        assert_eq!(
            value["not_selected"][0]["reason"],
            "priority/confidence below inline comment threshold"
        );
        assert_eq!(value["not_selected"][0]["reason_code"], "lower_relevance");
        Ok(())
    }

    #[test]
    fn comment_plan_prefers_high_confidence_selection_reason() -> Result<(), String> {
        let mut output = fixture_output("raw_pointer_alignment")?;
        let card = output
            .cards
            .first_mut()
            .ok_or_else(|| "fixture should emit one card".to_string())?;
        card.priority = Priority::Medium;
        card.confidence = Confidence::High;

        let value = parse_json(&render(&output))?;

        assert_eq!(value["comments"].as_array().map_or(0, Vec::len), 1);
        assert_review_budget_summary(&value, 1, 0)?;
        assert_eq!(
            value["comments"][0]["selection_reason"],
            "guard_coverage: missing — actionable high-confidence card"
        );
        assert_eq!(
            value["comments"][0]["selection_reason_code"],
            "top_actionable_card"
        );
        assert_eq!(value["comments"][0]["priority"], "medium");
        assert_eq!(value["comments"][0]["confidence"], "high");
        assert_eq!(value["comments"][0]["relevance"], "medium");
        Ok(())
    }

    #[test]
    fn comment_plan_explains_non_actionable_cards_before_relevance() -> Result<(), String> {
        let mut output = fixture_output("raw_pointer_alignment")?;
        let card = output
            .cards
            .first_mut()
            .ok_or_else(|| "fixture should emit one card".to_string())?;
        card.class = ReviewClass::GuardedAndWitnessed;
        card.confidence = Confidence::High;

        let value = parse_json(&render(&output))?;

        assert_eq!(value["comments"].as_array().map_or(1, Vec::len), 0);
        assert_eq!(value["not_selected"].as_array().map_or(0, Vec::len), 1);
        assert_review_budget_summary(&value, 0, 1)?;
        assert_eq!(value["not_selected"][0]["class"], "guarded_and_witnessed");
        assert_eq!(value["not_selected"][0]["actionability"], "not_actionable");
        assert_eq!(
            value["not_selected"][0]["reason"],
            "class not eligible for inline comments"
        );
        assert_eq!(
            value["not_selected"][0]["reason_code"],
            "human_deep_review_only"
        );
        assert_eq!(value["not_selected"][0]["relevance"], "high");
        Ok(())
    }

    #[test]
    fn comment_plan_explains_low_confidence_before_priority_relevance() -> Result<(), String> {
        let mut output = fixture_output("raw_pointer_alignment")?;
        let card = output
            .cards
            .first_mut()
            .ok_or_else(|| "fixture should emit one card".to_string())?;
        card.priority = Priority::High;
        card.confidence = Confidence::Low;

        let value = parse_json(&render(&output))?;

        assert_eq!(value["comments"].as_array().map_or(1, Vec::len), 0);
        assert_eq!(value["not_selected"].as_array().map_or(0, Vec::len), 1);
        assert_review_budget_summary(&value, 0, 1)?;
        assert_eq!(value["not_selected"][0]["priority"], "high");
        assert_eq!(value["not_selected"][0]["confidence"], "low");
        assert_eq!(
            value["not_selected"][0]["reason"],
            "confidence below inline comment threshold"
        );
        assert_eq!(value["not_selected"][0]["reason_code"], "lower_relevance");
        assert_eq!(value["not_selected"][0]["relevance"], "low");
        Ok(())
    }

    #[test]
    fn comment_plan_projects_relevance_for_selected_and_not_selected_cards() -> Result<(), String> {
        let selected = parse_json(&render(&fixture_output("raw_pointer_alignment")?))?;
        assert_eq!(selected["comments"].as_array().map_or(0, Vec::len), 1);
        assert_eq!(
            selected["comments"][0]["selection_reason"],
            "guard_coverage: missing — actionable high-priority card"
        );
        assert_eq!(
            selected["comments"][0]["selection_reason_code"],
            "top_actionable_card"
        );
        assert_eq!(selected["comments"][0]["relevance"], "medium");

        let not_selected = parse_json(&render(&fixture_output("ffi_sanitizer_route")?))?;
        assert_eq!(not_selected["comments"].as_array().map_or(1, Vec::len), 0);
        assert_eq!(
            not_selected["not_selected"][0]["reason"],
            "priority/confidence below inline comment threshold"
        );
        assert_eq!(
            not_selected["not_selected"][0]["reason_code"],
            "lower_relevance"
        );
        assert_eq!(not_selected["not_selected"][0]["relevance"], "low");

        Ok(())
    }

    #[test]
    fn comment_plan_skips_unsafe_declaration_surfacing_disposition_cards() -> Result<(), String> {
        let output = fixture_output("public_unsafe_fn_missing_safety")?;
        let value = parse_json(&render(&output))?;

        assert_eq!(value["comments"].as_array().map_or(1, Vec::len), 0);
        assert_eq!(value["not_selected"].as_array().map_or(0, Vec::len), 1);
        assert_review_budget_summary(&value, 0, 1)?;
        assert_eq!(value["not_selected"][0]["class"], "contract_missing");
        assert_eq!(
            value["not_selected"][0]["operation"],
            "pub unsafe fn caller_must_uphold_contract() {"
        );
        assert_eq!(
            value["not_selected"][0]["operation_family"],
            "unsafe_declaration"
        );
        assert_eq!(
            value["not_selected"][0]["next_action"],
            "Add a precise public `# Safety` section that names the required caller obligations."
        );
        assert_eq!(
            value["not_selected"][0]["reason"],
            "unsafe declaration is not selected for inline comments"
        );
        assert_eq!(
            value["not_selected"][0]["reason_code"],
            "human_deep_review_only"
        );
        assert_eq!(value["not_selected"][0]["agent_readiness"]["ready"], false);
        assert_eq!(
            value["not_selected"][0]["agent_readiness"]["state"],
            "requires_human_review"
        );
        assert!(
            serde_json::to_string(&value["not_selected"][0]["agent_readiness"]["reasons"])
                .map_err(|err| format!("render readiness reasons failed: {err}"))?
                .contains("operation family `unsafe_declaration`")
        );
        assert_eq!(
            value["not_selected"][0]["repair_queue_buckets"],
            serde_json::json!([
                "repairable_by_safety_docs",
                "repairable_by_test",
                "requires_witness_receipt",
                "requires_human_review",
                "do_not_auto_repair"
            ])
        );
        assert_eq!(
            value["not_selected"][0]["repair_queue_bucket_reasons"],
            serde_json::json!([
                "safety_docs_evidence_missing",
                "reach_evidence_missing",
                "witness_receipt_missing",
                "human_review_required",
                "not_ready_for_automatic_repair"
            ])
        );
        assert_eq!(
            value["not_selected"][0]["context_command"],
            format!("unsafe-review context {} --json", output.cards[0].id)
        );
        Ok(())
    }

    #[test]
    fn comment_plan_declaration_eligibility_does_not_depend_on_family_label() -> Result<(), String>
    {
        let mut output = fixture_output("public_unsafe_fn_missing_safety")?;
        let card = output
            .cards
            .first_mut()
            .ok_or_else(|| "fixture should emit a declaration card".to_string())?;
        card.operation.family = OperationFamily::RawPointerRead;

        let value = parse_json(&render(&output))?;

        assert_eq!(value["comments"].as_array().map_or(1, Vec::len), 0);
        assert_eq!(value["not_selected"].as_array().map_or(0, Vec::len), 1);
        assert_review_budget_summary(&value, 0, 1)?;
        assert_eq!(
            value["not_selected"][0]["operation_family"],
            "raw_pointer_read"
        );
        assert_eq!(
            value["not_selected"][0]["reason"],
            "unsafe declaration is not selected for inline comments"
        );
        assert_eq!(
            value["not_selected"][0]["reason_code"],
            "human_deep_review_only"
        );
        Ok(())
    }

    #[test]
    fn comment_plan_unknown_fallback_site_keeps_human_review_only_reason() -> Result<(), String> {
        let output = fixture_output("split_unsafe_block")?;
        let value = parse_json(&render(&output))?;

        assert_eq!(value["comments"].as_array().map_or(1, Vec::len), 0);
        assert_eq!(value["not_selected"].as_array().map_or(0, Vec::len), 1);
        assert_review_budget_summary(&value, 0, 1)?;
        assert_eq!(value["not_selected"][0]["operation_family"], "unknown");
        assert_eq!(
            value["not_selected"][0]["reason"],
            "operation family unknown"
        );
        assert_eq!(
            value["not_selected"][0]["reason_code"],
            "human_deep_review_only"
        );
        Ok(())
    }

    /// Proves that comment-plan candidate selection is importance-ranked, not file-order first.
    ///
    /// Four eligible cards with distinct budget keys are arranged so that their
    /// importance order differs from their file order (line number order).  The
    /// test asserts that the top-3 selected by importance differ from the
    /// first-3 in file order, demonstrating that a high-priority / severe-gap
    /// card later in the file displaces a lower-priority card earlier in the file.
    ///
    /// Ranking key (descending importance, as implemented in `selection::importance_rank`):
    /// 1. Priority: High first.
    /// 2. Gap severity: contract_coverage: missing > guard_coverage: missing >
    ///    guard_coverage: weak > test_reach_coverage: weak >
    ///    test_reach_coverage: missing > witness_receipt_coverage: missing.
    /// 3. Confidence: High first.
    /// 4. (file, line) ascending — deterministic tiebreak.
    ///
    /// Setup:
    ///   line  5 — family A  — Priority::Medium, Confidence::High, guard_missing  → importance (1,1,0,5)
    ///   line 10 — family B  — Priority::Medium, Confidence::High, guard_missing  → importance (1,1,0,10)
    ///   line 20 — family C  — Priority::Medium, Confidence::High, guard_missing  → importance (1,1,0,20)
    ///   line 30 — family D  — Priority::High,   Confidence::High, contract_missing → importance (0,0,0,30)
    ///
    /// File-order first-3: A(5), B(10), C(20)  — D would be budget_exhausted.
    /// Importance-ranked top-3: D(30), A(5), B(10) — C is now budget_exhausted.
    ///
    /// The expected outcome is that `comments[0]` is the family-D card (line 30,
    /// contract_missing, high priority) and `not_selected` contains the family-C
    /// card (line 20) with reason `budget_exhausted`.
    #[test]
    fn comment_plan_selects_top_importance_not_first_file_order() -> Result<(), String> {
        let mut output = fixture_output("raw_pointer_alignment")?;
        let template = output
            .cards
            .first()
            .cloned()
            .ok_or_else(|| "fixture should emit a card".to_string())?;

        // Four distinct-family eligible cards. All have changed=true and are
        // actionable. Priority and gap type differ so importance order ≠ file order.
        //
        // Cards A/B/C: guard_missing, Medium priority, High confidence.
        // Card D:      contract_missing, High priority, High confidence.
        //              Its importance rank (0,0,0) is higher than A/B/C (1,1,0)
        //              despite being at line 30 — later than A/B/C.
        let make_guard_missing =
            |idx: usize, line: usize, family: OperationFamily| -> crate::domain::ReviewCard {
                let mut card = template.clone();
                card.id.0 = format!("test-card-guard-{idx}");
                card.operation.family = family;
                card.priority = Priority::Medium;
                card.confidence = Confidence::High;
                card.class = ReviewClass::GuardMissing;
                // contract present so coverage_block shows guard_missing as primary gap
                card.contract = ContractEvidence::present("SAFETY comment present");
                card.site.location.line = line;
                card
            };

        let mut contract_missing_card = template.clone();
        contract_missing_card.id.0 = "test-card-contract-missing".to_string();
        contract_missing_card.operation.family = OperationFamily::MaybeUninitAssumeInit;
        contract_missing_card.priority = Priority::High;
        contract_missing_card.confidence = Confidence::High;
        contract_missing_card.class = ReviewClass::ContractMissing;
        // contract absent → coverage_block contract_coverage: missing (gap_rank = 0)
        contract_missing_card.contract = ContractEvidence::missing();
        contract_missing_card.site.location.line = 30;

        let card_a = make_guard_missing(0, 5, OperationFamily::RawPointerRead);
        let card_b = make_guard_missing(1, 10, OperationFamily::GetUnchecked);
        let card_c = make_guard_missing(2, 20, OperationFamily::StrFromUtf8Unchecked);
        let card_d = contract_missing_card;

        // File order: A(5), B(10), C(20), D(30)
        output.cards = vec![card_a, card_b, card_c, card_d];
        output.summary.cards = output.cards.len();
        output.summary.open_actionable_gaps = output.cards.len();

        let value = parse_json(&render(&output))?;

        // Budget of 3 is respected.
        let comments = value["comments"]
            .as_array()
            .ok_or_else(|| "comments should be an array".to_string())?;
        assert_eq!(comments.len(), 3, "expected exactly 3 selected comments");

        let not_selected = value["not_selected"]
            .as_array()
            .ok_or_else(|| "not_selected should be an array".to_string())?;
        assert_eq!(
            not_selected.len(),
            1,
            "expected exactly 1 not_selected card"
        );
        assert_review_budget_summary(&value, 3, 1)?;

        // Importance-ranked selection: D (contract_missing, High priority, line 30)
        // must be first, displacing C (guard_missing, Medium priority, line 20).
        // This is the key assertion: file-order first-3 would be [A,B,C]; importance
        // ranking selects [D,A,B] instead.
        assert_eq!(
            value["comments"][0]["operation_family"], "maybe_uninit_assume_init",
            "first selected comment must be the contract_missing/high-priority card (line 30, family D)"
        );
        assert_eq!(
            value["comments"][0]["coverage_gap"], "contract_coverage: missing",
            "first selected comment gap must be contract_coverage: missing"
        );
        assert_eq!(
            value["comments"][0]["selection_reason"],
            "contract_coverage: missing — actionable high-confidence card",
        );

        // A and B fill slots 2 and 3 (importance order among equal-rank cards uses file/line).
        assert_eq!(value["comments"][1]["operation_family"], "raw_pointer_read");
        assert_eq!(value["comments"][2]["operation_family"], "get_unchecked");

        // C (str_from_utf8_unchecked, line 20) is the card excluded by importance ranking.
        // Under file-order selection it would have been selected (slot 3); with importance
        // ranking it is displaced by D and lands in not_selected with budget_exhausted.
        assert_eq!(
            value["not_selected"][0]["operation_family"], "str_from_utf8_unchecked",
            "the card excluded by importance ranking must be str_from_utf8_unchecked (family C, line 20)"
        );
        assert_eq!(
            value["not_selected"][0]["reason_code"], "budget_exhausted",
            "family C is excluded because the budget was taken by the higher-importance family D card"
        );

        Ok(())
    }

    /// Invariant: every comment body emitted by the core renderer fits within
    /// [`COMMENT_BODY_WORD_LIMIT`] words, so the producer never generates a body
    /// that the `check-first-pr-artifacts` gate would reject.
    ///
    /// Checked against the fixtures that are known to produce comment-plan
    /// candidates (changed, actionable, high-signal cards).  The 242-word case
    /// from `raw_pointer_alignment` is the concrete regression case.
    #[test]
    fn comment_body_word_count_never_exceeds_limit() -> Result<(), String> {
        // Fixtures known to produce comment-plan candidates.
        let fixtures = &[
            "raw_pointer_alignment",
            "copy_nonoverlapping",
            "vec_set_len",
            "transmute_unchecked",
            "get_unchecked",
            "box_from_raw",
        ];
        for &fixture in fixtures {
            let output = match fixture_output(fixture) {
                Ok(o) => o,
                // Skip fixtures that are not found in this workspace layout.
                Err(_) => continue,
            };
            let plan_json = render(&output);
            let plan: serde_json::Value = serde_json::from_str(&plan_json)
                .map_err(|err| format!("JSON parse for fixture `{fixture}`: {err}"))?;
            let comments = plan["comments"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            for (idx, comment) in comments.iter().enumerate() {
                let body = comment["body"].as_str().unwrap_or("");
                let word_count = body.split_whitespace().count();
                if word_count > selection::COMMENT_BODY_WORD_LIMIT {
                    return Err(format!(
                        "fixture `{fixture}` comment[{idx}] body has {word_count} words; limit is {} — body:\n{body}",
                        selection::COMMENT_BODY_WORD_LIMIT
                    ));
                }
            }
        }
        Ok(())
    }

    /// Drift-lock: `no_changed_gaps` must NOT be emitted when `not_selected` is non-empty,
    /// even if zero comments were selected.  Previously it was gated on
    /// `open_actionable_gaps == 0`, which fired when cards existed but were all
    /// filtered out of inline comments (e.g. low-relevance FFI cards), contradicting
    /// the non-empty `not_selected` list in the same comment-plan.json.
    ///
    /// The `ffi_sanitizer_route` fixture produces one card that goes into `not_selected`
    /// (low relevance, no inline comment) — exactly the scenario that was broken.
    #[test]
    fn no_changed_gaps_not_emitted_when_not_selected_is_nonempty() -> Result<(), String> {
        // This fixture has a card but it is filtered to not_selected (low relevance).
        let output = fixture_output("ffi_sanitizer_route")?;
        let value = parse_json(&render(&output))?;

        assert_eq!(
            value["comments"].as_array().map_or(1, Vec::len),
            0,
            "fixture should emit no inline comments"
        );
        assert_eq!(
            value["not_selected"].as_array().map_or(0, Vec::len),
            1,
            "fixture should put the card in not_selected"
        );
        assert!(
            value.get("no_changed_gaps").is_none() || value["no_changed_gaps"].is_null(),
            "no_changed_gaps must not be emitted when not_selected is non-empty; got: {:?}",
            value.get("no_changed_gaps")
        );
        Ok(())
    }

    /// Owner card whose changed region is covered by a specific operation card gets
    /// the `covered_by_specific_operation_card` reason instead of the generic
    /// `human_deep_review_only` reason. The owner card is still present in `not_selected`
    /// (and therefore in `cards.json` and evidence counts) — it is never deleted.
    ///
    /// The `attributed_unsafe_fn_no_duplicate` fixture produces both an owner card
    /// (unsafe_declaration-family, `unsafe fn write_one`) and a specific operation card
    /// (RawPointerWrite, `core::ptr::write`) in the same file — the exact scenario
    /// where the richer reason should appear.
    #[test]
    fn comment_plan_owner_card_covered_by_operation_card_gets_specific_reason() -> Result<(), String>
    {
        let output = fixture_output("attributed_unsafe_fn_no_duplicate")?;
        let value = parse_json(&render(&output))?;

        // The operation card (raw_pointer_write) is eligible and should be selected.
        let comments = value["comments"]
            .as_array()
            .ok_or_else(|| "comments should be an array".to_string())?;
        assert!(
            comments
                .iter()
                .any(|c| c["operation_family"] == "raw_pointer_write"),
            "operation card (raw_pointer_write) must be selected; got: {value}"
        );

        // The owner card (unsafe_declaration family) must appear in not_selected.
        let not_selected = value["not_selected"]
            .as_array()
            .ok_or_else(|| "not_selected should be an array".to_string())?;
        let owner_entry = not_selected
            .iter()
            .find(|c| c["operation_family"] == "unsafe_declaration")
            .ok_or_else(|| {
                format!(
                    "owner card (unsafe_declaration family) must be in not_selected; got: {value}"
                )
            })?;
        assert_eq!(
            owner_entry["reason_code"], "covered_by_specific_operation_card",
            "owner card covered by operation card must get the specific reason code; got: {}",
            owner_entry["reason_code"]
        );
        assert_eq!(
            owner_entry["reason"],
            "owner-contract obligation covered by a more-specific operation card at the same region",
        );
        Ok(())
    }

    /// Guardrail: owner card NOT covered by a specific operation card on the same
    /// file still gets the generic `human_deep_review_only` unsafe-declaration
    /// reason (not the new covered reason).
    #[test]
    fn comment_plan_uncovered_owner_card_keeps_generic_declaration_reason() -> Result<(), String> {
        let output = fixture_output("public_unsafe_fn_missing_safety")?;
        let value = parse_json(&render(&output))?;

        // This fixture has only the owner card (no specific operation card).
        let not_selected = value["not_selected"]
            .as_array()
            .ok_or_else(|| "not_selected should be an array".to_string())?;
        let owner_entry = not_selected
            .iter()
            .find(|c| c["operation_family"] == "unsafe_declaration")
            .ok_or_else(|| {
                "owner card must be in not_selected for public_unsafe_fn_missing_safety".to_string()
            })?;
        assert_eq!(
            owner_entry["reason_code"], "human_deep_review_only",
            "owner card without a covering operation card must keep the generic reason; got: {}",
            owner_entry["reason_code"]
        );
        assert_eq!(
            owner_entry["reason"],
            "unsafe declaration is not selected for inline comments"
        );
        Ok(())
    }

    /// Drift-lock: `no_changed_gaps` IS emitted only when there are truly no cards at all
    /// (both comments and not_selected are empty), matching the human/witness surface semantics.
    #[test]
    fn no_changed_gaps_emitted_only_when_no_cards_at_all() -> Result<(), String> {
        let output = fixture_output("safe_code_no_cards")?;
        let value = parse_json(&render(&output))?;

        assert_eq!(value["comments"].as_array().map_or(1, Vec::len), 0);
        assert!(value.get("not_selected").is_none());
        assert_eq!(value["no_changed_gaps"]["message"], NO_CHANGED_GAPS_MESSAGE);
        assert_eq!(
            value["no_changed_gaps"]["limitation"],
            NO_CHANGED_GAPS_LIMITATION
        );
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

    fn assert_review_budget_summary(
        value: &serde_json::Value,
        selected_count: usize,
        not_selected_count: usize,
    ) -> Result<(), String> {
        assert_eq!(value["summary"]["selected_count"], selected_count);
        assert_eq!(value["summary"]["not_selected_count"], not_selected_count);
        assert_eq!(value["summary"]["budget"], 3);
        assert_eq!(value["summary"]["reason"], "bounded reviewer noise");
        assert_eq!(value["summary"]["reason_code"], "bounded_reviewer_noise");
        Ok(())
    }
}
