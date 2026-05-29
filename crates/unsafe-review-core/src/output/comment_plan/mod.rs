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
    use crate::domain::OperationFamily;
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
            value["comments"][0]["next_action"],
            "Add or expose the local guard that discharges the `raw_pointer_read` safety obligation."
        );
        assert_eq!(
            value["comments"][0]["selection_reason"],
            "actionable high-priority review card"
        );
        assert_eq!(
            value["comments"][0]["actionability"],
            "specific_guard_missing"
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
                .contains("unsafe-review did not post this comment")
        );
        assert!(
            value["comments"][0]["body"]
                .as_str()
                .unwrap_or("")
                .contains("not a Miri result")
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
            value["not_selected"][1]["reason"],
            "covered by selected family/obligation sibling"
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
        Ok(())
    }

    #[test]
    fn comment_plan_projects_relevance_for_selected_and_not_selected_cards() -> Result<(), String> {
        let selected = parse_json(&render(&fixture_output("raw_pointer_alignment")?))?;
        assert_eq!(selected["comments"].as_array().map_or(0, Vec::len), 1);
        assert_eq!(
            selected["comments"][0]["selection_reason"],
            "actionable high-priority review card"
        );
        assert_eq!(selected["comments"][0]["relevance"], "medium");

        let not_selected = parse_json(&render(&fixture_output("ffi_sanitizer_route")?))?;
        assert_eq!(not_selected["comments"].as_array().map_or(1, Vec::len), 0);
        assert_eq!(
            not_selected["not_selected"][0]["reason"],
            "priority/confidence below inline comment threshold"
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
        Ok(())
    }
}
