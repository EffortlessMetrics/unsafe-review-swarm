mod card;
mod header;

use crate::api::AnalyzeOutput;
use crate::output::{NO_CHANGED_GAPS_LIMITATION, NO_CHANGED_GAPS_MESSAGE};

pub(crate) fn render(output: &AnalyzeOutput) -> String {
    let mut out = String::new();
    header::render_header(&mut out, output);

    if output.cards.is_empty() {
        push_line(&mut out, NO_CHANGED_GAPS_MESSAGE);
        push_line(&mut out, NO_CHANGED_GAPS_LIMITATION);
        return out;
    }

    for card in &output.cards {
        card::render_card(&mut out, card);
    }

    out.push_str("Trust boundary: static unsafe contract review; not a proof of memory safety and not a Miri result unless a witness receipt is attached.\n");
    out
}

fn push_line(out: &mut String, line: &str) {
    out.push_str(line);
    out.push('\n');
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
        assert!(rendered.contains("operation: unsafe { ptr.cast::<Header>().read() }"));
        assert!(rendered.contains("operation_family: raw_pointer_read"));
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
