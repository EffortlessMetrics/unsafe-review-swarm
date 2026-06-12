mod model;
mod render;
mod selection;

pub(crate) use render::render;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{
        AnalysisMode, AnalyzeInput, AnalyzeOutput, DiffSource, PolicyMode, Scope, analyze,
    };
    use crate::domain::{Confidence, OperationFamily, Priority, ReviewClass};
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
                .contains("Minimal repro cue: confirm ReviewCard")
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
    fn comment_plan_skips_unknown_operation_family_cards() -> Result<(), String> {
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
        assert_eq!(value["not_selected"][0]["operation_family"], "unknown");
        assert_eq!(
            value["not_selected"][0]["next_action"],
            "Add a precise public `# Safety` section that names the required caller obligations."
        );
        assert_eq!(
            value["not_selected"][0]["reason"],
            "operation family unknown"
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
                .contains("operation family `unknown`")
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
