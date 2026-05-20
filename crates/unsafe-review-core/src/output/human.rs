use crate::api::AnalyzeOutput;
use crate::output::{NO_CHANGED_GAPS_LIMITATION, NO_CHANGED_GAPS_MESSAGE};
use crate::util::path_display;

pub(crate) fn render(output: &AnalyzeOutput) -> String {
    let mut out = String::new();
    out.push_str("unsafe-review\n");
    out.push_str(&format!(
        "scope: {:?}, mode: {}, policy: {}\n",
        output.scope,
        output.mode.as_str(),
        output.policy.as_str()
    ));
    out.push_str(&format!(
        "cards: {}, open gaps: {}, contract_missing: {}, guard_missing: {}, witness gaps: {}\n\n",
        output.summary.cards,
        output.summary.open_actionable_gaps,
        output.summary.contract_missing,
        output.summary.guard_missing,
        output.summary.guarded_unwitnessed
    ));

    if output.cards.is_empty() {
        out.push_str(NO_CHANGED_GAPS_MESSAGE);
        out.push('\n');
        out.push_str(NO_CHANGED_GAPS_LIMITATION);
        out.push('\n');
        return out;
    }

    for card in &output.cards {
        out.push_str(&format!(
            "{} {}:{}\n",
            card.class.as_str().to_uppercase(),
            path_display(&card.site.location.file),
            card.site.location.line
        ));
        out.push_str(&format!("  id: {}\n", card.id));
        out.push_str(&format!("  operation: {}\n", card.operation.expression));
        out.push_str("  hazards:\n");
        for hazard in &card.hazards {
            out.push_str(&format!("    - {}\n", hazard.as_str()));
        }
        out.push_str("  required safety conditions:\n");
        for obligation in &card.obligations {
            out.push_str(&format!("    - {}\n", obligation.description));
        }
        out.push_str(&format!("  contract: {}\n", card.contract.summary));
        out.push_str(&format!("  discharge: {}\n", card.discharge.summary));
        out.push_str(&format!("  reach: {}\n", card.reach.summary));
        out.push_str(
            "  reach note: static reach evidence only; it does not prove site execution.\n",
        );
        if !card.obligation_evidence.is_empty() {
            out.push_str("  obligation evidence:\n");
            for evidence in &card.obligation_evidence {
                out.push_str(&format!(
                    "    - {}: contract {}, guard {}, reach {}, witness {}\n",
                    evidence.obligation.key,
                    evidence.contract.state,
                    evidence.discharge.state,
                    evidence.reach.state,
                    evidence.witness.state
                ));
            }
        }
        out.push_str("  missing:\n");
        for missing in &card.missing {
            out.push_str(&format!("    - {}\n", missing.message));
        }
        if !card.routes.is_empty() {
            out.push_str("  witness routes:\n");
            for route in &card.routes {
                out.push_str(&format!(
                    "    - {}: {}\n",
                    route.kind.as_str(),
                    route.reason
                ));
                if let Some(command) = &route.command {
                    out.push_str(&format!("      command: {}\n", command));
                }
            }
        }
        out.push_str(&format!("  next: {}\n", card.next_action.summary));
        if !card.next_action.verify_commands.is_empty() {
            out.push_str("  verify:\n");
            for cmd in &card.next_action.verify_commands {
                out.push_str(&format!("    {}\n", cmd));
            }
        }
        out.push('\n');
    }

    out.push_str("Trust boundary: static unsafe contract review; not a proof of memory safety and not a Miri result unless a witness receipt is attached.\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{AnalysisMode, AnalyzeInput, DiffSource, PolicyMode, Scope, analyze};
    use std::path::PathBuf;

    #[test]
    fn human_output_names_conditions_evidence_and_routes() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let rendered = render(&output);

        assert!(rendered.contains("required safety conditions:"));
        assert!(rendered.contains("pointer is aligned for the accessed type"));
        assert!(rendered.contains("obligation evidence:"));
        assert!(rendered.contains("alignment: contract present, guard missing"));
        assert!(rendered.contains("witness routes:"));
        assert!(rendered.contains("miri: Pure-Rust UB-adjacent hazard"));
        assert!(rendered.contains("does not prove site execution"));
        assert!(rendered.contains("Trust boundary: static unsafe contract review"));
        Ok(())
    }

    #[test]
    fn human_empty_output_uses_standard_advisory_wording() -> Result<(), String> {
        let output = fixture_output("safe_code_no_cards")?;
        let rendered = render(&output);

        assert!(rendered.contains(NO_CHANGED_GAPS_MESSAGE));
        assert!(rendered.contains(NO_CHANGED_GAPS_LIMITATION));
        assert!(!rendered.contains("All clear"));
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
