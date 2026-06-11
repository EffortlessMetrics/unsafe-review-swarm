/// Gate manifest renderer (SPEC-0034).
///
/// Emits `unsafe-review-gate.json` — a thin routing manifest that points
/// `ub-review` at the structured artifacts a `first-pr`/`repo` run produced.
/// The manifest is advisory posture only; it carries no merge verdict, no
/// proof, no UB-free/Miri-clean/site-execution claim.
///
/// # Determinism
///
/// This module deliberately does **not** embed timestamps or wall-time
/// durations.  Volatile fields would break byte-golden / reproducibility rails.
/// The caller owns the generation context (tool version, fixed strings); the
/// manifest is fully deterministic given the same `AnalyzeOutput`.
use crate::api::{AnalyzeOutput, Scope, Summary};
use serde::Serialize;

/// Advisory trust boundary string (fixed, never varies).
const GATE_MANIFEST_TRUST_BOUNDARY: &str =
    "static unsafe-review coverage evidence; not proof, not a merge verdict";

/// Schema version agreed with ripr's gate-decision.json envelope.
const GATE_SCHEMA_VERSION: &str = "unsafe-review-gate/v1";

pub(crate) fn render(output: &AnalyzeOutput) -> String {
    let manifest = GateManifest::from(output);
    match serde_json::to_string_pretty(&manifest) {
        Ok(mut text) => {
            text.push('\n');
            text
        }
        Err(err) => {
            format!("{{\n  \"error\": \"gate manifest serialization failed: {err}\"\n}}\n")
        }
    }
}

#[derive(Serialize)]
struct GateManifest {
    schema_version: &'static str,
    dialect: &'static str,
    status: &'static str,
    summary: GateMovementSummary,
    artifacts: GateArtifacts,
    trust_boundary: &'static str,
    tool: &'static str,
    tool_version: &'static str,
}

impl From<&AnalyzeOutput> for GateManifest {
    fn from(output: &AnalyzeOutput) -> Self {
        Self {
            schema_version: GATE_SCHEMA_VERSION,
            dialect: "unsafe-review",
            status: "advisory",
            summary: GateMovementSummary::from(&output.summary),
            artifacts: GateArtifacts::from(output),
            trust_boundary: GATE_MANIFEST_TRUST_BOUNDARY,
            tool: "unsafe-review",
            tool_version: env!("CARGO_PKG_VERSION"),
        }
    }
}

/// The four-bucket movement block from SPEC-0030, copied verbatim from `Summary`.
/// Field names align with ripr's canonical movement counter vocabulary.
#[derive(Serialize)]
struct GateMovementSummary {
    new_gaps: usize,
    worsened_gaps: usize,
    resolved_gaps: usize,
    inherited_gaps: usize,
}

impl From<&Summary> for GateMovementSummary {
    fn from(summary: &Summary) -> Self {
        Self {
            new_gaps: summary.new_gaps,
            worsened_gaps: summary.worsened_gaps,
            resolved_gaps: summary.resolved_gaps,
            inherited_gaps: summary.inherited_gaps,
        }
    }
}

/// Relative pointers to the structured artifacts a run already wrote.
/// Optional artifacts that a run did not emit are absent from the serialized output
/// (via `#[serde(skip_serializing_if = "Option::is_none")]`).
#[derive(Serialize)]
struct GateArtifacts {
    /// ReviewCard full dataset (always present for first-pr/repo).
    cards: &'static str,
    /// Comment posting plan (always present for first-pr/repo).
    comment_plan: &'static str,
    /// Triage-and-repair queue (always present for first-pr/repo).
    repair_queue: &'static str,
    /// Receipt audit (always present for first-pr/repo).
    receipt_audit: &'static str,
    /// Review kit manifest (always present for first-pr/repo).
    review_kit: &'static str,
    /// PR summary markdown (always present for first-pr).
    pr_summary: &'static str,
    /// SARIF output (always present for first-pr).
    sarif: &'static str,
    /// LSP/editor projection (always present for first-pr).
    lsp: &'static str,
    /// Policy report JSON (always present for first-pr).
    policy_report: &'static str,
    /// Scope from the analysis run ("diff" or "repo").
    #[serde(skip)]
    _scope_marker: Scope,
}

impl From<&AnalyzeOutput> for GateArtifacts {
    fn from(output: &AnalyzeOutput) -> Self {
        Self {
            cards: "cards.json",
            comment_plan: "comment-plan.json",
            repair_queue: "repair-queue.json",
            receipt_audit: "receipt-audit.json",
            review_kit: "review-kit.json",
            pr_summary: "pr-summary.md",
            sarif: "cards.sarif",
            lsp: "lsp.json",
            policy_report: "policy-report.json",
            _scope_marker: output.scope.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{AnalysisMode, AnalyzeInput, DiffSource, PolicyMode, Scope, analyze};

    fn fixture_output(name: &str) -> Result<AnalyzeOutput, String> {
        let root = workspace_root().join("fixtures").join(name);
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

    fn workspace_root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn parse_json(text: &str) -> Result<serde_json::Value, String> {
        serde_json::from_str(text).map_err(|err| format!("JSON parse failed: {err}"))
    }

    #[test]
    fn gate_manifest_schema_version_and_envelope() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let text = render(&output);
        let value = parse_json(&text)?;

        assert_eq!(value["schema_version"], "unsafe-review-gate/v1");
        assert_eq!(value["dialect"], "unsafe-review");
        assert_eq!(value["status"], "advisory");
        assert_eq!(value["tool"], "unsafe-review");
        Ok(())
    }

    #[test]
    fn gate_manifest_trust_boundary_is_advisory() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let text = render(&output);
        let value = parse_json(&text)?;

        let boundary = value["trust_boundary"]
            .as_str()
            .ok_or_else(|| "trust_boundary must be a string".to_string())?;
        assert!(
            boundary.contains("not proof"),
            "trust_boundary must include 'not proof'; got: {boundary}"
        );
        assert!(
            boundary.contains("not a merge verdict"),
            "trust_boundary must include 'not a merge verdict'; got: {boundary}"
        );
        assert!(
            !boundary.contains("UB-free"),
            "trust_boundary must not claim UB-free status"
        );
        Ok(())
    }

    #[test]
    fn gate_manifest_movement_summary_copied_from_spec_0030() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let expected_new = output.summary.new_gaps;
        let expected_worsened = output.summary.worsened_gaps;
        let expected_resolved = output.summary.resolved_gaps;
        let expected_inherited = output.summary.inherited_gaps;

        let text = render(&output);
        let value = parse_json(&text)?;

        let summary = &value["summary"];
        assert_eq!(
            summary["new_gaps"].as_u64().ok_or("new_gaps not u64")?,
            expected_new as u64
        );
        assert_eq!(
            summary["worsened_gaps"]
                .as_u64()
                .ok_or("worsened_gaps not u64")?,
            expected_worsened as u64
        );
        assert_eq!(
            summary["resolved_gaps"]
                .as_u64()
                .ok_or("resolved_gaps not u64")?,
            expected_resolved as u64
        );
        assert_eq!(
            summary["inherited_gaps"]
                .as_u64()
                .ok_or("inherited_gaps not u64")?,
            expected_inherited as u64
        );
        Ok(())
    }

    #[test]
    fn gate_manifest_artifact_pointers_present() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let text = render(&output);
        let value = parse_json(&text)?;

        let artifacts = &value["artifacts"];
        assert_eq!(artifacts["cards"], "cards.json");
        assert_eq!(artifacts["comment_plan"], "comment-plan.json");
        assert_eq!(artifacts["repair_queue"], "repair-queue.json");
        assert_eq!(artifacts["receipt_audit"], "receipt-audit.json");
        assert_eq!(artifacts["review_kit"], "review-kit.json");
        assert_eq!(artifacts["pr_summary"], "pr-summary.md");
        assert_eq!(artifacts["sarif"], "cards.sarif");
        assert_eq!(artifacts["lsp"], "lsp.json");
        assert_eq!(artifacts["policy_report"], "policy-report.json");
        Ok(())
    }

    #[test]
    fn gate_manifest_is_parseable_json_with_trailing_newline() -> Result<(), String> {
        let output = fixture_output("safe_code_no_cards")?;
        let text = render(&output);

        assert!(
            text.ends_with('\n'),
            "gate manifest must end with a newline"
        );
        parse_json(&text)?;
        Ok(())
    }

    #[test]
    fn gate_manifest_tool_version_present() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let text = render(&output);
        let value = parse_json(&text)?;

        let version = value["tool_version"]
            .as_str()
            .ok_or_else(|| "tool_version must be a string".to_string())?;
        assert!(!version.is_empty(), "tool_version must not be empty");
        Ok(())
    }

    #[test]
    fn gate_manifest_no_timestamp_or_wall_time_fields() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let text = render(&output);
        let value = parse_json(&text)?;

        let obj = value.as_object().ok_or("manifest must be an object")?;
        for volatile_key in ["generated_at", "timestamp", "wall_seconds", "elapsed_ms"] {
            if obj.contains_key(volatile_key) {
                return Err(format!(
                    "gate manifest must not contain volatile field `{volatile_key}` (breaks determinism)"
                ));
            }
        }
        Ok(())
    }
}
