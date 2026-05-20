use crate::api::AnalyzeOutput;
use crate::domain::ReviewCard;
use crate::output::{NO_CHANGED_GAPS_LIMITATION, NO_CHANGED_GAPS_MESSAGE};
use crate::util::path_display;

pub(crate) fn render(output: &AnalyzeOutput) -> String {
    let mut out = String::new();
    out.push_str("# unsafe-review witness plan\n\n");
    out.push_str(&format!("- Review cards: {}\n", output.summary.cards));
    out.push_str(&format!(
        "- Open actionable gaps: {}\n",
        output.summary.open_actionable_gaps
    ));
    out.push_str(&format!("- Policy mode: `{}`\n\n", output.policy.as_str()));

    if output.cards.is_empty() {
        out.push_str(NO_CHANGED_GAPS_MESSAGE);
        out.push('\n');
        out.push_str(NO_CHANGED_GAPS_LIMITATION);
        out.push_str(
            "\n\nNo witness routes are recommended because no review cards were emitted.\n\n",
        );
    } else {
        out.push_str("## Routes\n\n");
        for card in &output.cards {
            render_card(&mut out, card);
        }
    }

    out.push_str("## Trust boundary\n\n");
    out.push_str("This artifact is static unsafe contract review. It routes reviewers to credible witnesses but does not run Miri, cargo-careful, sanitizers, Loom, Shuttle, Kani, or Crux. It is not a proof of memory safety, not UB-free status, and not a Miri result unless a witness receipt is attached.\n");
    out
}

fn render_card(out: &mut String, card: &ReviewCard) {
    out.push_str(&format!(
        "### `{}`\n\n- Class: `{}`\n- Location: {}:{}\n- Operation: `{}`\n- Missing evidence: {}\n- Witness evidence: {}\n\n",
        card.id,
        card.class.as_str(),
        path_display(&card.site.location.file),
        card.site.location.line,
        one_line(&card.operation.expression),
        missing_summary(card),
        card.witness.summary
    ));

    if card.routes.is_empty() {
        out.push_str("- Route: `human-deep-review`\n");
        out.push_str("  - Reason: no automatic witness route was selected\n\n");
        return;
    }

    for route in &card.routes {
        out.push_str(&format!(
            "- Route: `{}`{}\n",
            route.kind.as_str(),
            if route.required { " (required)" } else { "" }
        ));
        out.push_str(&format!("  - Reason: {}\n", route.reason));
        if let Some(command) = &route.command {
            out.push_str("  - Command:\n\n");
            out.push_str("```bash\n");
            out.push_str(command);
            out.push_str("\n```\n");
        } else {
            out.push_str("  - Command: no automatic command; route to human review.\n");
        }
    }
    out.push('\n');
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{AnalysisMode, AnalyzeInput, DiffSource, PolicyMode, Scope, analyze};
    use std::path::PathBuf;

    #[test]
    fn witness_plan_routes_cards_without_claiming_execution() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let rendered = render(&output);

        assert!(rendered.contains("# unsafe-review witness plan"));
        assert!(rendered.contains("Route: `miri`"));
        assert!(rendered.contains("cargo +nightly miri test read_header"));
        assert!(rendered.contains("Missing visible local guard"));
        assert!(rendered.contains("does not run Miri"));
        assert!(rendered.contains("not UB-free status"));
        Ok(())
    }

    #[test]
    fn witness_plan_empty_output_uses_standard_advisory_wording() -> Result<(), String> {
        let output = fixture_output("safe_code_no_cards")?;
        let rendered = render(&output);

        assert!(rendered.contains(NO_CHANGED_GAPS_MESSAGE));
        assert!(rendered.contains(NO_CHANGED_GAPS_LIMITATION));
        assert!(rendered.contains("No witness routes are recommended"));
        assert!(!rendered.contains("All clear"));
        Ok(())
    }

    #[test]
    fn witness_plan_shows_imported_receipts_and_remaining_gaps() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment_receipted")?;
        let rendered = render(&output);

        assert!(rendered.contains("Imported miri receipt"));
        assert!(rendered.contains("expires_at: 2026-08-18"));
        assert!(rendered.contains("Missing visible local guard"));
        assert!(rendered.contains("not a Miri result unless a witness receipt is attached"));
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
}
