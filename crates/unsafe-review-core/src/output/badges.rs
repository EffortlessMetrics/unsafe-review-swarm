use crate::api::AnalyzeOutput;
use serde::Serialize;

pub(crate) fn render(output: &AnalyzeOutput) -> (String, String) {
    let base_count = output.summary.open_actionable_gaps;
    let base_color = badge_color(base_count);
    let evidence_quality = EvidenceQualityCounts {
        contract_missing: output.summary.contract_missing,
        guard_missing: output.summary.guard_missing,
        guarded_unwitnessed: output.summary.guarded_unwitnessed,
    };
    let weak_evidence_findings = evidence_quality.total();
    let plus_count = base_count + weak_evidence_findings;
    let plus_color = badge_color(plus_count);
    let main = badge("unsafe-review", base_count, base_color);
    let plus = badge("unsafe-review+", plus_count, plus_color);
    (render_pretty(&main), render_pretty(&plus))
}

#[derive(Clone, Copy)]
struct EvidenceQualityCounts {
    contract_missing: usize,
    guard_missing: usize,
    guarded_unwitnessed: usize,
}

impl EvidenceQualityCounts {
    fn total(self) -> usize {
        self.contract_missing + self.guard_missing + self.guarded_unwitnessed
    }
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

fn badge(label: &'static str, count: usize, color: &'static str) -> BadgeJson {
    BadgeJson {
        schema_version: 1,
        label,
        message: count.to_string(),
        color,
    }
}

#[derive(Serialize)]
struct BadgeJson {
    #[serde(rename = "schemaVersion")]
    schema_version: u8,
    label: &'static str,
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
        assert_shields_endpoint_fields_only(&main)?;
        assert_ne!(main["message"], "safe");

        assert_eq!(plus["schemaVersion"], 1);
        assert_eq!(plus["label"], "unsafe-review+");
        assert_eq!(plus["message"], "2");
        assert_eq!(plus["color"], "yellow");
        assert_shields_endpoint_fields_only(&plus)?;
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
        assert_eq!(main["color"], "green");
        assert_shields_endpoint_fields_only(&main)?;
        assert_ne!(main["message"], "safe");
        assert_eq!(plus["message"], "0");
        assert_eq!(plus["schemaVersion"], 1);
        assert_shields_endpoint_fields_only(&plus)?;
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

    #[test]
    fn unsafe_review_plus_count_matches_component_breakdown() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let (main, plus) = render(&output);
        let main = parse_json(&main)?;
        let plus = parse_json(&plus)?;

        assert_eq!(main["message"], "1");
        assert_eq!(plus["message"], "2");

        Ok(())
    }

    #[test]
    fn public_badge_payloads_are_shields_endpoint_json() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let (main, plus) = render(&output);

        for text in [main, plus] {
            let badge = parse_json(&text)?;
            assert_shields_endpoint_fields_only(&badge)?;
            for internal in [
                "contract_version",
                "kind",
                "scope",
                "basis",
                "status",
                "counts",
            ] {
                assert!(
                    badge.get(internal).is_none(),
                    "public badge JSON must not contain internal field `{internal}`"
                );
            }
        }

        Ok(())
    }

    fn assert_badge_endpoint_contract(
        expected_label: &str,
        _expected_kind: &str,
        text: &str,
    ) -> Result<(), String> {
        let badge = parse_json(text)?;
        assert_eq!(badge["schemaVersion"], 1);
        assert_eq!(badge["label"], expected_label);
        assert_shields_endpoint_fields_only(&badge)?;

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

    fn assert_shields_endpoint_fields_only(badge: &serde_json::Value) -> Result<(), String> {
        let object = badge
            .as_object()
            .ok_or_else(|| "badge endpoint JSON must be an object".to_string())?;
        for key in object.keys() {
            if !["schemaVersion", "label", "message", "color"].contains(&key.as_str()) {
                return Err(format!(
                    "public badge JSON contains non-Shields field `{key}`"
                ));
            }
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
