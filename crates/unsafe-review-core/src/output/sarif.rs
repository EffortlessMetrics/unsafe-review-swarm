use crate::api::AnalyzeOutput;
use crate::domain::{ReviewCard, ReviewClass, WitnessRoute};
use crate::util::path_display;
use serde::Serialize;
use std::collections::BTreeSet;

const SARIF_SCHEMA: &str =
    "https://docs.oasis-open.org/sarif/sarif/v2.1.0/cs01/schemas/sarif-schema-2.1.0.json";
const TRUST_BOUNDARY: &str = "Static unsafe contract review only; this is not a proof of memory safety, not UB-free status, and not a Miri result unless a witness receipt is attached.";

pub(crate) fn render(output: &AnalyzeOutput) -> String {
    render_pretty(&SarifLog::from(output))
}

fn render_pretty(value: &impl Serialize) -> String {
    match serde_json::to_string_pretty(value) {
        Ok(text) => text,
        Err(err) => format!("{{\n  \"error\": \"sarif serialization failed: {err}\"\n}}"),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifLog {
    #[serde(rename = "$schema")]
    schema: &'static str,
    version: &'static str,
    runs: Vec<SarifRun>,
}

impl From<&AnalyzeOutput> for SarifLog {
    fn from(output: &AnalyzeOutput) -> Self {
        Self {
            schema: SARIF_SCHEMA,
            version: "2.1.0",
            runs: vec![SarifRun::from(output)],
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifRun {
    tool: SarifTool,
    results: Vec<SarifResult>,
    properties: SarifRunProperties,
}

impl From<&AnalyzeOutput> for SarifRun {
    fn from(output: &AnalyzeOutput) -> Self {
        Self {
            tool: SarifTool {
                driver: SarifDriver {
                    name: "unsafe-review",
                    semantic_version: env!("CARGO_PKG_VERSION"),
                    information_uri: "https://github.com/EffortlessMetrics/unsafe-review",
                    rules: sarif_rules(output),
                },
            },
            results: output.cards.iter().map(SarifResult::from).collect(),
            properties: SarifRunProperties {
                schema_version: output.schema_version.clone(),
                scope: scope_label(output),
                mode: output.mode.as_str(),
                policy: output.policy.as_str(),
                trust_boundary: TRUST_BOUNDARY,
            },
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifTool {
    driver: SarifDriver,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifDriver {
    name: &'static str,
    semantic_version: &'static str,
    information_uri: &'static str,
    rules: Vec<SarifRule>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifRule {
    id: &'static str,
    short_description: SarifText,
    full_description: SarifText,
    help_uri: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifText {
    text: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifRunProperties {
    schema_version: String,
    scope: &'static str,
    mode: &'static str,
    policy: &'static str,
    trust_boundary: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifResult {
    rule_id: &'static str,
    level: &'static str,
    message: SarifText,
    locations: Vec<SarifLocation>,
    properties: SarifResultProperties,
}

impl From<&ReviewCard> for SarifResult {
    fn from(card: &ReviewCard) -> Self {
        Self {
            rule_id: card.class.as_str(),
            level: sarif_level(&card.class),
            message: SarifText {
                text: format!("{}: {}", card.class.as_str(), card.next_action.summary),
            },
            locations: vec![SarifLocation::from(card)],
            properties: SarifResultProperties::from(card),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifLocation {
    physical_location: SarifPhysicalLocation,
}

impl From<&ReviewCard> for SarifLocation {
    fn from(card: &ReviewCard) -> Self {
        Self {
            physical_location: SarifPhysicalLocation {
                artifact_location: SarifArtifactLocation {
                    uri: path_display(&card.site.location.file),
                },
                region: SarifRegion {
                    start_line: card.site.location.line,
                    start_column: card.site.location.column,
                },
            },
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifPhysicalLocation {
    artifact_location: SarifArtifactLocation,
    region: SarifRegion,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifArtifactLocation {
    uri: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifRegion {
    start_line: usize,
    start_column: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifResultProperties {
    card_id: String,
    class: &'static str,
    priority: &'static str,
    confidence: &'static str,
    operation_family: &'static str,
    operation: String,
    hazards: Vec<&'static str>,
    missing_evidence: Vec<String>,
    witness_routes: Vec<String>,
    witness_route_details: Vec<SarifWitnessRoute>,
    next_action: String,
    verify_commands: Vec<String>,
    trust_boundary: &'static str,
}

impl From<&ReviewCard> for SarifResultProperties {
    fn from(card: &ReviewCard) -> Self {
        Self {
            card_id: card.id.0.clone(),
            class: card.class.as_str(),
            priority: card.priority.as_str(),
            confidence: card.confidence.as_str(),
            operation_family: card.operation.family.as_str(),
            operation: card.operation.expression.clone(),
            hazards: card.hazards.iter().map(|hazard| hazard.as_str()).collect(),
            missing_evidence: card
                .missing
                .iter()
                .map(|missing| missing.message.clone())
                .collect(),
            witness_routes: card
                .routes
                .iter()
                .map(|route| format!("{}: {}", route.kind.as_str(), route.reason))
                .collect(),
            witness_route_details: card.routes.iter().map(SarifWitnessRoute::from).collect(),
            next_action: card.next_action.summary.clone(),
            verify_commands: card.next_action.verify_commands.clone(),
            trust_boundary: TRUST_BOUNDARY,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifWitnessRoute {
    kind: &'static str,
    reason: String,
    command: Option<String>,
    required: bool,
}

impl From<&WitnessRoute> for SarifWitnessRoute {
    fn from(route: &WitnessRoute) -> Self {
        Self {
            kind: route.kind.as_str(),
            reason: route.reason.clone(),
            command: route.command.clone(),
            required: route.required,
        }
    }
}

fn sarif_rules(output: &AnalyzeOutput) -> Vec<SarifRule> {
    let mut classes = BTreeSet::new();
    for card in &output.cards {
        classes.insert(card.class.as_str());
    }
    classes
        .into_iter()
        .map(|id| SarifRule {
            id,
            short_description: SarifText {
                text: format!("unsafe-review {id}"),
            },
            full_description: SarifText {
                text: "Review-card finding for missing unsafe contract evidence.".to_string(),
            },
            help_uri: "https://github.com/EffortlessMetrics/unsafe-review",
        })
        .collect()
}

fn sarif_level(class: &ReviewClass) -> &'static str {
    match class {
        ReviewClass::ContractMissing
        | ReviewClass::GuardMissing
        | ReviewClass::ReachableUnwitnessed
        | ReviewClass::RequiresLoom
        | ReviewClass::RequiresSanitizer
        | ReviewClass::RequiresKaniOrCrux
        | ReviewClass::MiriUnsupported
        | ReviewClass::WitnessMismatch => "warning",
        ReviewClass::GuardedAndWitnessed | ReviewClass::BaselineKnown | ReviewClass::Suppressed => {
            "none"
        }
        ReviewClass::GuardedUnwitnessed
        | ReviewClass::UnsafeUnreached
        | ReviewClass::StaticUnknown => "note",
    }
}

fn scope_label(output: &AnalyzeOutput) -> &'static str {
    match output.scope {
        crate::api::Scope::Diff => "diff",
        crate::api::Scope::Repo => "repo",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{AnalysisMode, AnalyzeInput, DiffSource, PolicyMode, Scope, analyze};
    use std::path::PathBuf;

    #[test]
    fn sarif_output_is_parseable_and_projects_review_cards() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let value = parse_json(&render(&output))?;

        assert_eq!(value["version"], "2.1.0");
        assert_eq!(value["runs"][0]["tool"]["driver"]["name"], "unsafe-review");
        assert_eq!(value["runs"][0]["results"][0]["ruleId"], "guard_missing");
        assert_eq!(
            value["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"]
                ["uri"],
            "src/lib.rs"
        );
        assert_eq!(
            value["runs"][0]["results"][0]["properties"]["operationFamily"],
            "raw_pointer_read"
        );
        assert_eq!(
            value["runs"][0]["results"][0]["properties"]["operation"],
            "unsafe { ptr.cast::<Header>().read() }"
        );
        assert_eq!(
            value["runs"][0]["results"][0]["properties"]["witnessRouteDetails"][0]["kind"],
            "miri"
        );
        assert!(
            value["runs"][0]["results"][0]["properties"]["verifyCommands"][0]
                .as_str()
                .unwrap_or("")
                .contains("cargo +nightly miri test read_header")
        );
        assert!(
            value["runs"][0]["results"][0]["properties"]["trustBoundary"]
                .as_str()
                .unwrap_or("")
                .contains("not a Miri result")
        );
        Ok(())
    }

    #[test]
    fn sarif_empty_output_has_no_results_and_keeps_trust_boundary() -> Result<(), String> {
        let output = fixture_output("safe_code_no_cards")?;
        let value = parse_json(&render(&output))?;

        assert_eq!(
            value["runs"][0]["results"].as_array().map_or(1, Vec::len),
            0
        );
        assert!(
            value["runs"][0]["properties"]["trustBoundary"]
                .as_str()
                .unwrap_or("")
                .contains("not UB-free status")
        );
        Ok(())
    }

    #[test]
    fn sarif_rule_ids_and_levels_are_stable_review_class_contract() {
        let cases = [
            (
                ReviewClass::GuardedAndWitnessed,
                "guarded_and_witnessed",
                "none",
            ),
            (
                ReviewClass::GuardedUnwitnessed,
                "guarded_unwitnessed",
                "note",
            ),
            (ReviewClass::ContractMissing, "contract_missing", "warning"),
            (ReviewClass::GuardMissing, "guard_missing", "warning"),
            (
                ReviewClass::ReachableUnwitnessed,
                "reachable_unwitnessed",
                "warning",
            ),
            (ReviewClass::UnsafeUnreached, "unsafe_unreached", "note"),
            (ReviewClass::WitnessMismatch, "witness_mismatch", "warning"),
            (ReviewClass::RequiresLoom, "requires_loom", "warning"),
            (
                ReviewClass::RequiresSanitizer,
                "requires_sanitizer",
                "warning",
            ),
            (
                ReviewClass::RequiresKaniOrCrux,
                "requires_kani_or_crux",
                "warning",
            ),
            (ReviewClass::MiriUnsupported, "miri_unsupported", "warning"),
            (ReviewClass::StaticUnknown, "static_unknown", "note"),
            (ReviewClass::BaselineKnown, "baseline_known", "none"),
            (ReviewClass::Suppressed, "suppressed", "none"),
        ];

        for (class, expected_rule_id, expected_level) in cases {
            assert_eq!(class.as_str(), expected_rule_id);
            assert_eq!(sarif_level(&class), expected_level);
        }
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
