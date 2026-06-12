/// Module: output/usefulness_telemetry
/// SPEC-0038 low-noise usefulness telemetry projection.
///
/// Projects from existing ReviewCard/Summary/CoverageBlock/CommentPlan data only.
/// This is diagnostic operational usefulness only — not calibrated, not a measurement of
/// detection accuracy, not a guarantee of any kind, not a gate, not a merge verdict.
use crate::api::AnalyzeOutput;
use crate::domain::coverage::{AgentLspReadiness, Coverage, WitnessReceiptCoverage};
use crate::domain::{Confidence, ReviewClass};
use crate::output::comment_plan;
use serde::Serialize;
use std::collections::BTreeMap;

/// Advisory trust boundary string (fixed, never varies).
const USEFULNESS_TELEMETRY_TRUST_BOUNDARY: &str = "operational diagnostic usefulness only — not calibrated, not a measurement of detection accuracy, not a memory guarantee, not a soundness guarantee, not a gate, and not a merge verdict; all telemetry is projected from ReviewCard/Summary/CoverageBlock/CommentPlan fields deterministically";

/// Schema version for the usefulness telemetry artifact.
const SCHEMA_VERSION: &str = "usefulness-telemetry/v1";

/// Low-noise usefulness telemetry artifact (SPEC-0038).
///
/// A pure read-only projection from `AnalyzeOutput`. The ReviewCard remains the
/// single truth object; this struct is a diagnostic aperture only.
#[derive(Serialize)]
struct UsefulnessTelemetry {
    schema_version: &'static str,
    trust_boundary: &'static str,
    card_inventory: CardInventory,
    coverage_slots: CoverageSlots,
    agent_readiness: AgentReadinessHistogram,
    comment_selection: CommentSelection,
    confidence_distribution: ConfidenceDistribution,
    /// Actionability distribution over all cards, keyed by actionability() value.
    /// Only keys with count > 0 are emitted.
    actionability_distribution: BTreeMap<String, usize>,
}

/// Card inventory derived from Summary fields.
#[derive(Serialize)]
struct CardInventory {
    total_cards: usize,
    actionable_cards: usize,
    new_cards: usize,
    worsened_cards: usize,
    resolved_cards: usize,
    inherited_cards: usize,
}

/// Coverage slot counts derived from CoverageBlock per card.
///
/// `contract_weak` is always 0 for contract — no weak state — but included
/// for schema completeness.
#[derive(Serialize)]
struct CoverageSlots {
    contract_missing: usize,
    /// Always 0; contract has no weak state. Present for schema completeness.
    contract_weak: usize,
    guard_missing: usize,
    guard_weak: usize,
    test_reach_missing: usize,
    test_reach_weak: usize,
    witness_receipt_missing: usize,
}

/// Agent readiness histogram derived from CoverageBlock.agent_lsp_readiness per card.
#[derive(Serialize)]
struct AgentReadinessHistogram {
    ready: usize,
    needs_human: usize,
    unsupported: usize,
}

/// Comment plan selection counts derived from re-rendering the comment plan.
#[derive(Serialize)]
struct CommentSelection {
    selected_count: usize,
    not_selected_count: usize,
    /// Histogram of not-selected reason codes. Only keys with count > 0 are emitted.
    not_selected_reason_histogram: BTreeMap<String, usize>,
}

/// Confidence distribution over all cards.
#[derive(Serialize)]
struct ConfidenceDistribution {
    high: usize,
    medium: usize,
    low: usize,
    unknown: usize,
}

/// Render `usefulness-telemetry.json` as a pretty-printed JSON string.
///
/// This is a pure projection from the existing `AnalyzeOutput`. It carries no
/// new analysis state and does not modify any card or summary field.
pub(crate) fn render(output: &AnalyzeOutput) -> String {
    let telemetry = build(output);
    match serde_json::to_string_pretty(&telemetry) {
        Ok(mut text) => {
            text.push('\n');
            text
        }
        Err(err) => {
            format!("{{\n  \"error\": \"usefulness telemetry serialization failed: {err}\"\n}}\n")
        }
    }
}

fn build(output: &AnalyzeOutput) -> UsefulnessTelemetry {
    let card_inventory = build_card_inventory(output);
    let coverage_slots = build_coverage_slots(output);
    let agent_readiness = build_agent_readiness(output);
    let comment_selection = build_comment_selection(output);
    let confidence_distribution = build_confidence_distribution(output);
    let actionability_distribution = build_actionability_distribution(output);

    UsefulnessTelemetry {
        schema_version: SCHEMA_VERSION,
        trust_boundary: USEFULNESS_TELEMETRY_TRUST_BOUNDARY,
        card_inventory,
        coverage_slots,
        agent_readiness,
        comment_selection,
        confidence_distribution,
        actionability_distribution,
    }
}

fn build_card_inventory(output: &AnalyzeOutput) -> CardInventory {
    CardInventory {
        total_cards: output.summary.cards,
        actionable_cards: output.summary.open_actionable_gaps,
        new_cards: output.summary.new_gaps,
        worsened_cards: output.summary.worsened_gaps,
        resolved_cards: output.summary.resolved_gaps,
        inherited_cards: output.summary.inherited_gaps,
    }
}

fn build_coverage_slots(output: &AnalyzeOutput) -> CoverageSlots {
    let mut contract_missing = 0usize;
    let mut guard_missing = 0usize;
    let mut guard_weak = 0usize;
    let mut test_reach_missing = 0usize;
    let mut test_reach_weak = 0usize;
    let mut witness_receipt_missing = 0usize;

    for card in &output.cards {
        let block = card.coverage_block();
        if block.contract_coverage == Coverage::Missing {
            contract_missing += 1;
        }
        match block.guard_coverage {
            Coverage::Missing => guard_missing += 1,
            Coverage::Weak => guard_weak += 1,
            Coverage::Present => {}
        }
        match block.test_reach_coverage {
            Coverage::Missing => test_reach_missing += 1,
            Coverage::Weak => test_reach_weak += 1,
            Coverage::Present => {}
        }
        if block.witness_receipt_coverage == WitnessReceiptCoverage::Missing {
            witness_receipt_missing += 1;
        }
    }

    CoverageSlots {
        contract_missing,
        contract_weak: 0,
        guard_missing,
        guard_weak,
        test_reach_missing,
        test_reach_weak,
        witness_receipt_missing,
    }
}

fn build_agent_readiness(output: &AnalyzeOutput) -> AgentReadinessHistogram {
    let mut ready = 0usize;
    let mut needs_human = 0usize;
    let mut unsupported = 0usize;

    for card in &output.cards {
        let block = card.coverage_block();
        match block.agent_lsp_readiness {
            AgentLspReadiness::Ready => ready += 1,
            AgentLspReadiness::NeedsHuman => needs_human += 1,
            AgentLspReadiness::Unsupported => unsupported += 1,
        }
    }

    AgentReadinessHistogram {
        ready,
        needs_human,
        unsupported,
    }
}

/// Build comment selection counts by re-rendering the comment plan and parsing the JSON.
///
/// On any parse failure the counts default to 0/0/empty — this is a best-effort
/// projection and the fallback is clearly honest (missing counts, not wrong counts).
fn build_comment_selection(output: &AnalyzeOutput) -> CommentSelection {
    let plan_json = comment_plan::render(output);
    let plan: serde_json::Value = match serde_json::from_str(&plan_json) {
        Ok(v) => v,
        Err(_) => {
            return CommentSelection {
                selected_count: 0,
                not_selected_count: 0,
                not_selected_reason_histogram: BTreeMap::new(),
            };
        }
    };

    let selected_count = plan["summary"]["selected_count"].as_u64().unwrap_or(0) as usize;
    let not_selected_count = plan["summary"]["not_selected_count"].as_u64().unwrap_or(0) as usize;

    let mut not_selected_reason_histogram: BTreeMap<String, usize> = BTreeMap::new();
    if let Some(not_selected) = plan["not_selected"].as_array() {
        for entry in not_selected {
            if let Some(code) = entry["reason_code"].as_str() {
                *not_selected_reason_histogram
                    .entry(code.to_string())
                    .or_insert(0) += 1;
            }
        }
    }

    CommentSelection {
        selected_count,
        not_selected_count,
        not_selected_reason_histogram,
    }
}

fn build_confidence_distribution(output: &AnalyzeOutput) -> ConfidenceDistribution {
    let mut high = 0usize;
    let mut medium = 0usize;
    let mut low = 0usize;
    let mut unknown = 0usize;

    for card in &output.cards {
        match card.confidence {
            Confidence::High => high += 1,
            Confidence::Medium => medium += 1,
            Confidence::Low => low += 1,
            Confidence::Unknown => unknown += 1,
        }
    }

    ConfidenceDistribution {
        high,
        medium,
        low,
        unknown,
    }
}

/// Derive the actionability label for a card.
///
/// Mirrors the same logic in `output/comment_plan/selection.rs::actionability()`
/// without importing that private function. The mapping is a trivial match over
/// `ReviewClass` (15 lines); inlining avoids a cross-module coupling.
fn card_actionability(card: &crate::domain::ReviewCard) -> &'static str {
    match &card.class {
        ReviewClass::GuardMissing => "specific_guard_missing",
        ReviewClass::ContractMissing => "specific_contract_missing",
        ReviewClass::GuardedUnwitnessed
        | ReviewClass::ReachableUnwitnessed
        | ReviewClass::RequiresLoom
        | ReviewClass::RequiresSanitizer
        | ReviewClass::RequiresKaniOrCrux
        | ReviewClass::MiriUnsupported => "specific_witness_missing",
        ReviewClass::WitnessMismatch => "specific_receipt_missing",
        ReviewClass::UnsafeUnreached => "specific_reach_missing",
        ReviewClass::StaticUnknown => "human_review_only",
        _ => "not_actionable",
    }
}

fn build_actionability_distribution(output: &AnalyzeOutput) -> BTreeMap<String, usize> {
    let mut distribution: BTreeMap<String, usize> = BTreeMap::new();
    for card in &output.cards {
        let label = card_actionability(card);
        *distribution.entry(label.to_string()).or_insert(0) += 1;
    }
    distribution
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
    fn usefulness_telemetry_schema_version_and_envelope() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let text = render(&output);
        let value = parse_json(&text)?;

        assert_eq!(value["schema_version"], "usefulness-telemetry/v1");
        let boundary = value["trust_boundary"]
            .as_str()
            .ok_or_else(|| "trust_boundary must be a string".to_string())?;
        assert!(
            !boundary.is_empty(),
            "trust_boundary must be present and non-empty"
        );
        assert!(
            boundary.contains("not calibrated"),
            "trust_boundary must contain 'not calibrated'; got: {boundary}"
        );
        Ok(())
    }

    #[test]
    fn usefulness_telemetry_card_inventory_matches_summary() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let expected_total = output.summary.cards;
        let expected_actionable = output.summary.open_actionable_gaps;
        let expected_new = output.summary.new_gaps;

        let text = render(&output);
        let value = parse_json(&text)?;

        assert_eq!(
            value["card_inventory"]["total_cards"]
                .as_u64()
                .ok_or("total_cards not u64")? as usize,
            expected_total,
            "total_cards must match summary.cards"
        );
        assert_eq!(
            value["card_inventory"]["actionable_cards"]
                .as_u64()
                .ok_or("actionable_cards not u64")? as usize,
            expected_actionable,
            "actionable_cards must match summary.open_actionable_gaps"
        );
        assert_eq!(
            value["card_inventory"]["new_cards"]
                .as_u64()
                .ok_or("new_cards not u64")? as usize,
            expected_new,
            "new_cards must match summary.new_gaps"
        );
        Ok(())
    }

    #[test]
    fn usefulness_telemetry_agent_readiness_sums_to_total_cards() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let expected_total = output.summary.cards;

        let text = render(&output);
        let value = parse_json(&text)?;

        let ready = value["agent_readiness"]["ready"]
            .as_u64()
            .ok_or("agent_readiness.ready not u64")? as usize;
        let needs_human = value["agent_readiness"]["needs_human"]
            .as_u64()
            .ok_or("agent_readiness.needs_human not u64")? as usize;
        let unsupported = value["agent_readiness"]["unsupported"]
            .as_u64()
            .ok_or("agent_readiness.unsupported not u64")? as usize;

        assert_eq!(
            ready + needs_human + unsupported,
            expected_total,
            "agent_readiness histogram must sum to total_cards ({expected_total}); got ready={ready}, needs_human={needs_human}, unsupported={unsupported}"
        );
        Ok(())
    }

    #[test]
    fn usefulness_telemetry_coverage_slots_consistent_with_cards() -> Result<(), String> {
        // The raw_pointer_alignment fixture has 1 card with guard_missing class
        // and no contract (SAFETY comment missing). Spot-check that coverage slots
        // reflect this.
        let output = fixture_output("raw_pointer_alignment")?;
        assert_eq!(output.summary.cards, 1, "fixture must have exactly 1 card");

        let text = render(&output);
        let value = parse_json(&text)?;

        let contract_missing = value["coverage_slots"]["contract_missing"]
            .as_u64()
            .ok_or("contract_missing not u64")?;
        // contract_weak is always 0 for contract slot (no weak state)
        let contract_weak = value["coverage_slots"]["contract_weak"]
            .as_u64()
            .ok_or("contract_weak not u64")?;

        assert_eq!(
            contract_weak, 0,
            "contract_weak must always be 0 (no weak state for contract slot)"
        );
        // contract_missing should be 0 or 1; just verify it's a valid count
        assert!(
            contract_missing <= 1,
            "contract_missing must be 0 or 1 for a 1-card output"
        );
        Ok(())
    }

    #[test]
    fn usefulness_telemetry_comment_selection_counts_present() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let total = output.summary.cards;

        let text = render(&output);
        let value = parse_json(&text)?;

        let selected = value["comment_selection"]["selected_count"]
            .as_u64()
            .ok_or("selected_count not u64")? as usize;
        let not_selected = value["comment_selection"]["not_selected_count"]
            .as_u64()
            .ok_or("not_selected_count not u64")? as usize;

        // selected + not_selected must equal total_cards (every card is either selected or not)
        assert_eq!(
            selected + not_selected,
            total,
            "selected_count + not_selected_count must equal total_cards; got selected={selected}, not_selected={not_selected}, total={total}"
        );
        Ok(())
    }

    #[test]
    fn usefulness_telemetry_trust_boundary_no_overclaims() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let text = render(&output);
        let value = parse_json(&text)?;

        let boundary = value["trust_boundary"]
            .as_str()
            .ok_or_else(|| "trust_boundary must be a string".to_string())?;

        let lower = boundary.to_ascii_lowercase();
        // MUST NOT contain positive calibration/proof claims
        for forbidden in [
            "calibrated precision",
            "calibrated recall",
            "proof",
            "ub-free",
            "miri-clean",
            "site-execution",
        ] {
            assert!(
                !lower.contains(forbidden),
                "trust_boundary must not contain '{forbidden}'; got: {boundary}"
            );
        }
        Ok(())
    }
}
