use crate::api::AnalyzeOutput;
use crate::domain::{ReviewCard, ReviewClass, WitnessRoute};
use crate::output::REVIEWCARD_TRUST_BOUNDARY as TRUST_BOUNDARY;
use crate::util::path_display;
use serde::Serialize;
use std::collections::BTreeMap;

const SARIF_SCHEMA: &str =
    "https://docs.oasis-open.org/sarif/sarif/v2.1.0/cs01/schemas/sarif-schema-2.1.0.json";
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
    proof_path: &'static str,
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
            proof_path: card.proof_path.as_str(),
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
    // Collect one rule per distinct class string; use BTreeMap to deduplicate
    // while preserving the class-aware description for each id.
    let mut classes: BTreeMap<&'static str, &'static str> = BTreeMap::new();
    for card in &output.cards {
        classes
            .entry(card.class.as_str())
            .or_insert_with(|| sarif_rule_description(&card.class));
    }
    classes
        .into_iter()
        .map(|(id, description)| SarifRule {
            id,
            short_description: SarifText {
                text: format!("unsafe-review {id}"),
            },
            full_description: SarifText {
                text: description.to_string(),
            },
            help_uri: "https://github.com/EffortlessMetrics/unsafe-review",
        })
        .collect()
}

/// Return a rule-level description that matches the class's own evidence state.
/// Actionable classes (missing evidence) get missing-evidence wording; closed
/// classes (evidence present, baselined, or suppressed) get descriptions that
/// do not contradict the card's own evidence.  No proof, UB-free, or
/// Miri-clean claims — advisory boundary is always preserved.
fn sarif_rule_description(class: &ReviewClass) -> &'static str {
    match class {
        // Non-actionable: evidence is present or the card is administratively closed.
        ReviewClass::GuardedAndWitnessed => {
            "Review-card finding: unsafe code has a safety contract, guard, and witness receipt."
        }
        ReviewClass::BaselineKnown => {
            "Review-card finding: unsafe code is tracked in the baseline; gap is inherited, not new."
        }
        ReviewClass::Suppressed => {
            "Review-card finding: unsafe code is suppressed by the policy ledger; review the suppression entry."
        }
        // Actionable: evidence is missing or incomplete — keep the missing-evidence wording.
        ReviewClass::ContractMissing => "Review-card finding for missing unsafe contract evidence.",
        ReviewClass::GuardMissing => "Review-card finding for missing unsafe contract evidence.",
        ReviewClass::ReachableUnwitnessed => {
            "Review-card finding for missing unsafe contract evidence."
        }
        ReviewClass::GuardedUnwitnessed => {
            "Review-card finding for missing unsafe contract evidence."
        }
        ReviewClass::UnsafeUnreached => "Review-card finding for missing unsafe contract evidence.",
        ReviewClass::WitnessMismatch => "Review-card finding for missing unsafe contract evidence.",
        ReviewClass::RequiresLoom => "Review-card finding for missing unsafe contract evidence.",
        ReviewClass::RequiresSanitizer => {
            "Review-card finding for missing unsafe contract evidence."
        }
        ReviewClass::RequiresKaniOrCrux => {
            "Review-card finding for missing unsafe contract evidence."
        }
        ReviewClass::MiriUnsupported => "Review-card finding for missing unsafe contract evidence.",
        ReviewClass::StaticUnknown => "Review-card finding for missing unsafe contract evidence.",
    }
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
                .contains("not a site-execution claim")
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

    /// Verify that non-actionable classes carry descriptions that do NOT
    /// say "missing evidence", while actionable classes retain that wording.
    /// This is the regression guard for output-audit finding #1687 (finding 2).
    #[test]
    fn sarif_rule_description_is_class_aware_not_always_missing_evidence() {
        // Non-actionable: GuardedAndWitnessed — evidence IS present.
        let desc = sarif_rule_description(&ReviewClass::GuardedAndWitnessed);
        assert!(
            !desc.contains("missing"),
            "GuardedAndWitnessed rule must not say 'missing': {desc}"
        );
        assert!(
            desc.contains("contract") || desc.contains("witness"),
            "GuardedAndWitnessed rule description should mention contract or witness: {desc}"
        );

        // Non-actionable: BaselineKnown — gap is baselined, not new.
        let desc = sarif_rule_description(&ReviewClass::BaselineKnown);
        assert!(
            !desc.contains("missing"),
            "BaselineKnown rule must not say 'missing': {desc}"
        );
        assert!(
            desc.contains("baseline") || desc.contains("inherited"),
            "BaselineKnown rule description should mention baseline or inherited: {desc}"
        );

        // Non-actionable: Suppressed — suppressed by policy ledger.
        let desc = sarif_rule_description(&ReviewClass::Suppressed);
        assert!(
            !desc.contains("missing"),
            "Suppressed rule must not say 'missing': {desc}"
        );
        assert!(
            desc.contains("suppressed") || desc.contains("policy"),
            "Suppressed rule description should mention suppressed or policy: {desc}"
        );

        // Actionable: GuardMissing — evidence IS missing.
        let desc = sarif_rule_description(&ReviewClass::GuardMissing);
        assert!(
            desc.contains("missing"),
            "GuardMissing rule must still say 'missing': {desc}"
        );

        // Actionable: ContractMissing — evidence IS missing.
        let desc = sarif_rule_description(&ReviewClass::ContractMissing);
        assert!(
            desc.contains("missing"),
            "ContractMissing rule must still say 'missing': {desc}"
        );
    }

    /// Integration test: a baseline_known card in the SARIF output must carry
    /// a rule description that does NOT say "missing evidence".
    #[test]
    fn sarif_baseline_known_rule_description_does_not_say_missing_evidence() -> Result<(), String> {
        let output = fixture_output("raw_pointer_deref_brownfield_inherited")?;
        // Ensure the fixture actually produced a baseline_known card.
        assert!(
            output
                .cards
                .iter()
                .any(|c| c.class == ReviewClass::BaselineKnown),
            "expected at least one baseline_known card from fixture"
        );
        let value = parse_json(&render(&output))?;

        // Find the rule for baseline_known in the tool.driver.rules array.
        let rules = value["runs"][0]["tool"]["driver"]["rules"]
            .as_array()
            .ok_or("rules must be an array")?;
        let baseline_rule = rules
            .iter()
            .find(|r| r["id"] == "baseline_known")
            .ok_or("expected a rule with id baseline_known")?;
        let full_desc = baseline_rule["fullDescription"]["text"]
            .as_str()
            .unwrap_or("");
        assert!(
            !full_desc.contains("missing"),
            "baseline_known rule fullDescription must not say 'missing': {full_desc}"
        );
        assert!(
            full_desc.contains("baseline") || full_desc.contains("inherited"),
            "baseline_known rule fullDescription should mention baseline or inherited: {full_desc}"
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
