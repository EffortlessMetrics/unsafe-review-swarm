use crate::api::AnalyzeOutput;
use serde::Serialize;

pub(crate) fn render(output: &AnalyzeOutput) -> (String, String) {
    let base_count = output.summary.open_actionable_gaps;
    let base_color = badge_color(base_count);
    let weak_evidence_findings = output.summary.contract_missing
        + output.summary.guard_missing
        + output.summary.guarded_unwitnessed;
    let plus_count = base_count + weak_evidence_findings;
    let plus_color = badge_color(plus_count);
    let main = badge(
        "unsafe_review",
        "repo",
        "open_actionable_review_gaps",
        "unsafe-review",
        base_count,
        base_color,
        BadgeCounts {
            unsuppressed_review_gaps: base_count,
            unsuppressed_evidence_quality_findings: 0,
            suppressed_review_gaps: 0,
            suppressed_evidence_quality_findings: 0,
            intentional_findings: 0,
            unknowns: output.summary.static_unknown,
            analyzed_unsafe_seams: output.summary.unsafe_sites,
        },
    );
    let plus = badge(
        "unsafe_review_plus",
        "repo",
        "open_actionable_review_gaps_plus_evidence_quality_findings",
        "unsafe-review+",
        plus_count,
        plus_color,
        BadgeCounts {
            unsuppressed_review_gaps: base_count,
            unsuppressed_evidence_quality_findings: weak_evidence_findings,
            suppressed_review_gaps: 0,
            suppressed_evidence_quality_findings: 0,
            intentional_findings: 0,
            unknowns: output.summary.static_unknown,
            analyzed_unsafe_seams: output.summary.unsafe_sites,
        },
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

fn badge(
    kind: &'static str,
    scope: &'static str,
    basis: &'static str,
    label: &'static str,
    count: usize,
    color: &'static str,
    counts: BadgeCounts,
) -> BadgeJson<'static> {
    BadgeJson {
        schema_version: 1,
        contract_version: "0.1",
        kind,
        scope,
        basis,
        label,
        message: count.to_string(),
        status: if count == 0 { "pass" } else { "fail" },
        color,
        counts,
    }
}

#[derive(Serialize)]
struct BadgeJson<'a> {
    #[serde(rename = "schemaVersion")]
    schema_version: u8,
    contract_version: &'a str,
    kind: &'a str,
    scope: &'a str,
    basis: &'a str,
    label: &'a str,
    message: String,
    status: &'a str,
    color: &'static str,
    counts: BadgeCounts,
}

#[derive(Serialize)]
struct BadgeCounts {
    unsuppressed_review_gaps: usize,
    unsuppressed_evidence_quality_findings: usize,
    suppressed_review_gaps: usize,
    suppressed_evidence_quality_findings: usize,
    intentional_findings: usize,
    unknowns: usize,
    analyzed_unsafe_seams: usize,
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
        assert_eq!(main["contract_version"], "0.1");
        assert_eq!(main["kind"], "unsafe_review");
        assert_eq!(main["basis"], "open_actionable_review_gaps");
        assert_eq!(main["label"], "unsafe-review");
        assert_eq!(main["message"], "1");
        assert_eq!(main["status"], "fail");
        assert_eq!(main["color"], "yellow");
        assert_eq!(main["counts"]["unsuppressed_review_gaps"], 1);
        assert_eq!(main["counts"]["unsuppressed_evidence_quality_findings"], 0);
        assert_ne!(main["message"], "safe");

        assert_eq!(plus["schemaVersion"], 1);
        assert_eq!(plus["contract_version"], "0.1");
        assert_eq!(plus["kind"], "unsafe_review_plus");
        assert_eq!(
            plus["basis"],
            "open_actionable_review_gaps_plus_evidence_quality_findings"
        );
        assert_eq!(plus["label"], "unsafe-review+");
        assert_eq!(plus["message"], "2");
        assert_eq!(plus["status"], "fail");
        assert_eq!(plus["color"], "yellow");
        assert_eq!(plus["counts"]["unsuppressed_review_gaps"], 1);
        assert_eq!(plus["counts"]["unsuppressed_evidence_quality_findings"], 1);
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
        assert_eq!(main["schemaVersion"], 1);
        assert_eq!(main["contract_version"], "0.1");
        assert_eq!(main["status"], "pass");
        assert_eq!(main["color"], "green");
        assert_ne!(main["message"], "safe");
        assert_eq!(plus["message"], "0");
        assert_eq!(plus["schemaVersion"], 1);
        assert_eq!(plus["contract_version"], "0.1");
        assert_eq!(plus["status"], "pass");
        assert_ne!(plus["message"], "Miri-clean");
        Ok(())
    }

    #[test]
    fn badge_endpoint_messages_are_numeric_and_overclaim_free() -> Result<(), String> {
        for fixture in ["raw_pointer_alignment", "safe_code_no_cards"] {
            let output = fixture_output(fixture)?;
            let (main, plus) = render(&output);

            assert_badge_endpoint_contract("unsafe-review", "unsafe_review", &main)?;
            assert_badge_endpoint_contract("unsafe-review+", "unsafe_review_plus", &plus)?;
        }

        Ok(())
    }

    fn assert_badge_endpoint_contract(
        expected_label: &str,
        expected_kind: &str,
        text: &str,
    ) -> Result<(), String> {
        let badge = parse_json(text)?;
        assert_eq!(badge["schemaVersion"], 1);
        assert_eq!(badge["contract_version"], "0.1");
        assert_eq!(badge["kind"], expected_kind);
        assert_eq!(badge["label"], expected_label);

        let message = badge["message"]
            .as_str()
            .ok_or_else(|| "badge message must be a string".to_string())?;
        assert!(
            !message.is_empty() && message.chars().all(|ch| ch.is_ascii_digit()),
            "badge message must be a numeric count, got {message:?}"
        );

        let lowercase = text.to_ascii_lowercase();
        for forbidden in [
            "all clear",
            "ub-free",
            "miri-clean",
            "verified",
            "proof",
            "policy-ready",
            "blocking-ready",
            "site execution",
            "memory-safety",
        ] {
            assert!(
                !lowercase.contains(forbidden),
                "badge endpoint JSON must not contain overclaim term {forbidden:?}: {text}"
            );
        }

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
