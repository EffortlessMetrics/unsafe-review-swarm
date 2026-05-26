use crate::api::AnalyzeOutput;
use serde::Serialize;

pub(crate) fn render(output: &AnalyzeOutput) -> (String, String) {
    let color = badge_color(output.summary.open_actionable_gaps);
    let main = badge(
        "unsafe-review",
        output.summary.open_actionable_gaps.to_string(),
        color,
    );
    let weak_evidence_gaps = output.summary.contract_missing
        + output.summary.guard_missing
        + output.summary.guarded_unwitnessed;
    let plus = badge(
        "unsafe-review+",
        weak_evidence_gaps.to_string(),
        color,
    );
    (render_pretty(&main), render_pretty(&plus))
}

fn badge_color(open_actionable_gaps: usize) -> &'static str {
    if open_actionable_gaps == 0 {
        "green"
    } else if open_actionable_gaps < 10 {
        "yellow"
    } else {
        "orange"
    }
}

fn render_pretty(value: &impl Serialize) -> String {
    match serde_json::to_string_pretty(value) {
        Ok(mut text) => {
            text.push('\n');
            text
        }
        Err(err) => format!("{{\n  \"error\": \"badge serialization failed: {err}\"\n}}\n"),
    }
}

fn badge(label: &'static str, message: String, color: &'static str) -> ShieldsBadge<'static> {
    ShieldsBadge {
        schema_version: 1,
        label,
        message,
        color,
    }
}

#[derive(Serialize)]
struct ShieldsBadge<'a> {
    #[serde(rename = "schemaVersion")]
    schema_version: u8,
    label: &'a str,
    message: String,
    color: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{AnalysisMode, AnalyzeInput, DiffSource, PolicyMode, Scope};
    use std::path::PathBuf;

    #[test]
    fn badge_json_counts_open_gaps_without_safety_claim() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let (main, plus) = render(&output);
        let main = parse_json(&main)?;
        let plus = parse_json(&plus)?;

        assert_eq!(main["schemaVersion"], 1);
        assert_eq!(main["label"], "unsafe-review");
        assert_eq!(main["message"], "1");
        assert_eq!(main["color"], "yellow");
        assert_ne!(main["message"], "safe");

        assert_eq!(plus["schemaVersion"], 1);
        assert_eq!(plus["label"], "unsafe-review+");
        assert_eq!(plus["message"], "1");
        assert_eq!(plus["color"], "yellow");
        assert_ne!(plus["message"], "UB-free");
        Ok(())
    }

    #[test]
    fn zero_gap_badge_json_still_names_open_gaps_not_safety() -> Result<(), String> {
        let output = fixture_output("safe_code_no_cards")?;
        let (main, plus) = render(&output);
        let main = parse_json(&main)?;
        let plus = parse_json(&plus)?;

        assert_eq!(main["message"], "0");
        assert_eq!(main["color"], "green");
        assert_ne!(main["message"], "safe");
        assert_eq!(plus["message"], "0");
        assert_ne!(plus["message"], "Miri-clean");
        Ok(())
    }

    fn fixture_output(name: &str) -> Result<AnalyzeOutput, String> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name);
        crate::api::analyze(AnalyzeInput {
            root,
            scope: Scope::Repo,
            diff: DiffSource::NoneRepoScan,
            mode: AnalysisMode::Repo,
            policy: PolicyMode::Advisory,
            include_unchanged_tests: true,
            max_cards: None,
        })
    }

    fn parse_json(text: &str) -> Result<serde_json::Value, String> {
        serde_json::from_str(text).map_err(|err| format!("JSON parse failed: {err}"))
    }
}
