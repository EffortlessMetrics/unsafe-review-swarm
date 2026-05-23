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
    use crate::output::{NO_CHANGED_GAPS_LIMITATION, NO_CHANGED_GAPS_MESSAGE};
    use std::path::PathBuf;

    #[test]
    fn comment_plan_projects_high_signal_actionable_cards() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let value = parse_json(&render(&output))?;

        assert_eq!(value["mode"], "plan_only");
        assert_eq!(value["comments"].as_array().map_or(0, Vec::len), 1);
        assert_eq!(value["comments"][0]["class"], "guard_missing");
        assert_eq!(value["comments"][0]["path"], "src/lib.rs");
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
}
