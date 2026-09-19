//! Stage/fact-requirement inventory over the current pipeline (issue #2325 PR1).
//!
//! Shift-left usefulness is time to a credible action, not time to a complete
//! repository inventory. The [`StageInventory`] maps what one analysis actually
//! did onto the progressive-analysis stages named by #2325, classifies which
//! facts each stage required versus which work stayed optional, and derives
//! each fact's state from canonical output — never from a second analyzer,
//! event stream, or re-derived count. A quiet result rendered beside this
//! inventory cannot be misread as a strong claim: every omission is named,
//! with the exact flag or later slice that would supply it.
//!
//! Measurement placement follows #2309: wall-clock timing stays with the CLI
//! [`PhaseClock`](crate::latency) and its `--latency-out` receipts. Each
//! [`StageRecord`] names the latency phase it maps to but records no clock of
//! its own, so no duplicate timers exist.
//!
//! The inventory is not a task list, a readiness verdict, a safety score, or
//! a precision/recall claim. PR2 owns the changed/affected authoring profile
//! that decides when a task may surface; this slice only states what the
//! current run established and what it left out.

/// Version of the stage-inventory schema. Bump when fields change meaning.
pub const STAGE_SCHEMA_VERSION: u32 = 1;

/// Progressive-analysis stages in emit order. These name pipeline positions,
/// not execution gates: the current pipeline always runs to completion, and
/// each record states what its stage established.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum AnalysisStage {
    ScopeResolved,
    ChangedSubjectsAnalyzed,
    AffectedSubjectsAnalyzed,
    ActionReady,
    Enriched,
    CompleteForDeclaredScope,
}

impl AnalysisStage {
    pub fn as_str(&self) -> &'static str {
        match self {
            AnalysisStage::ScopeResolved => "scope_resolved",
            AnalysisStage::ChangedSubjectsAnalyzed => "changed_subjects_analyzed",
            AnalysisStage::AffectedSubjectsAnalyzed => "affected_subjects_analyzed",
            AnalysisStage::ActionReady => "action_ready",
            AnalysisStage::Enriched => "enriched",
            AnalysisStage::CompleteForDeclaredScope => "complete_for_declared_scope",
        }
    }
}

/// Availability of one required fact on this run.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum FactState {
    /// Present and complete for the declared scope.
    Available,
    /// Referenced but not established (e.g. files named but never scanned).
    Unknown,
    /// The pipeline cannot produce this fact; consumers must route around it.
    Unsupported,
    /// Not evaluated on this run; the detail names the flag or slice that
    /// would supply it.
    Pending,
    /// Evaluation was attempted but did not complete for the declared scope.
    Failed,
}

impl FactState {
    pub fn as_str(&self) -> &'static str {
        match self {
            FactState::Available => "available",
            FactState::Unknown => "unknown",
            FactState::Unsupported => "unsupported",
            FactState::Pending => "pending",
            FactState::Failed => "failed",
        }
    }
}

/// One fact a stage requires before its claim holds, with its run state.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FactRequirement {
    pub name: String,
    pub required: bool,
    pub state: FactState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Completeness of one stage on this run, derived from its required facts:
/// every required fact available means complete; any failed fact blocks the
/// stage; any other non-available fact leaves it partial.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum StageCompleteness {
    Complete,
    Partial,
    Blocked,
}

impl StageCompleteness {
    pub fn as_str(&self) -> &'static str {
        match self {
            StageCompleteness::Complete => "complete",
            StageCompleteness::Partial => "partial",
            StageCompleteness::Blocked => "blocked",
        }
    }

    fn derive(required_facts: &[FactRequirement]) -> Self {
        if required_facts
            .iter()
            .any(|fact| fact.required && fact.state == FactState::Failed)
        {
            return StageCompleteness::Blocked;
        }
        if required_facts
            .iter()
            .all(|fact| !fact.required || fact.state == FactState::Available)
        {
            return StageCompleteness::Complete;
        }
        StageCompleteness::Partial
    }
}

/// One pipeline stage with its required facts, its pending optional work,
/// and the CLI latency phase it maps to for timing (see module docs: the
/// inventory records no clocks of its own).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StageRecord {
    pub stage: AnalysisStage,
    pub completeness: StageCompleteness,
    /// Name of the CLI phase-latency span covering this stage
    /// (`input_resolution`, `analyze`, `projections`, `policy_eval`,
    /// `artifact_writes`). Timing comes from `--latency-out` receipts.
    pub latency_phase: String,
    pub required_facts: Vec<FactRequirement>,
    pub optional_work: Vec<String>,
}

/// Stage/fact-requirement inventory for one analysis revision.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StageInventory {
    pub schema_version: u32,
    pub analysis_id: String,
    pub source_generation: u64,
    pub scope: String,
    pub stages: Vec<StageRecord>,
    pub pending_optional_work: Vec<String>,
    pub digest: String,
}

/// Candidate optional enrichment named by #2325. Omitting any of these cannot
/// silently change the current claim or authority; anything that could
/// reverse the selected task is a required fact, never enrichment.
const OPTIONAL_ENRICHMENT_CANDIDATES: &[&str] = &[
    "broader inherited repository inventory beyond changed/affected subjects",
    "additional equivalent-action groups outside changed/affected subjects",
    "bounded history beyond the current relevant decision",
    "further related-test candidates",
    "optional semantic adapter results",
    "repository-map aggregates",
    "presentation-only prose or examples",
];

fn required(name: &str, state: FactState, detail: Option<String>) -> FactRequirement {
    FactRequirement {
        name: name.to_string(),
        required: true,
        state,
        detail,
    }
}

fn record(
    stage: AnalysisStage,
    latency_phase: &str,
    required_facts: Vec<FactRequirement>,
    optional_work: Vec<String>,
) -> StageRecord {
    let completeness = StageCompleteness::derive(&required_facts);
    StageRecord {
        stage,
        completeness,
        latency_phase: latency_phase.to_string(),
        required_facts,
        optional_work,
    }
}

/// Assemble the stage inventory from one analysis output, an optional
/// same-owner impact inventory (`--impact`), and an optional evaluated
/// configuration-envelope digest (explicit `--features`/`--target`
/// selection). Deterministic: fact order is fixed and the digest covers the
/// canonical serialization.
pub fn assemble_stage_inventory(
    output: &crate::AnalyzeOutput,
    impact: Option<&crate::input::impact::ImpactInventory>,
    environment_digest: Option<&str>,
) -> StageInventory {
    let summary = &output.summary;
    let identity = &output.analysis_identity;
    let unresolved = output.unresolved_diff_files.len();
    let rejected = output.rejected_diff_files.len();

    // ScopeResolved: source generation plus the changed-file inventory.
    let mut scope_facts = vec![required(
        "source_generation",
        FactState::Available,
        Some(format!("generation {}", identity.generation)),
    )];
    if unresolved == 0 && rejected == 0 {
        scope_facts.push(required(
            "changed_file_inventory",
            FactState::Available,
            Some(format!(
                "{} changed files named, all resolved from the analysis root",
                summary.changed_files
            )),
        ));
    } else {
        scope_facts.push(required(
            "changed_file_inventory",
            FactState::Unknown,
            Some(format!(
                "{unresolved} unresolved and {rejected} rejected changed paths; their contents are not established"
            )),
        ));
    }

    // ChangedSubjectsAnalyzed: the detector scan over resolved subjects.
    let mut changed_facts = vec![required(
        "changed_subject_scan",
        FactState::Available,
        Some(format!(
            "{} unsafe sites, {} cards",
            summary.unsafe_sites, summary.cards
        )),
    )];
    if summary.scan_capped {
        changed_facts.push(required(
            "uncapped_subject_scan",
            FactState::Failed,
            Some(
                summary
                    .capped_scan_notice()
                    .unwrap_or_else(|| "scan capped by card cap".to_string()),
            ),
        ));
    }

    // AffectedSubjectsAnalyzed: supported #2319 relations, or the exact
    // reason they are absent on this run.
    let affected_facts = match impact {
        Some(inventory) => vec![required(
            "affected_seam_analysis",
            FactState::Available,
            Some(format!(
                "{} affected unchanged seams",
                inventory.affected.len()
            )),
        )],
        None => vec![required(
            "affected_seam_analysis",
            FactState::Pending,
            Some("not evaluated on this run; select --impact".to_string()),
        )],
    };

    // ActionReady: applicability facts the pipeline establishes; readiness
    // itself gates on the PR2 authoring profile, so it stays pending here.
    let action_facts = vec![
        required(
            "task_applicability_roles",
            FactState::Available,
            Some(format!(
                "production {} / test {} / generated {} / unknown {}",
                summary.production_cards,
                summary.test_cards,
                summary.generated_cards,
                summary.unknown_cards
            )),
        ),
        required(
            "route_readiness",
            FactState::Pending,
            Some(
                "readiness gates on the changed/affected authoring profile (#2325 PR2)".to_string(),
            ),
        ),
    ];

    // Enriched: no enrichment runs in the default pipeline; the pending list
    // is the exact omission set.
    let enrichment_facts = vec![required(
        "enrichment_selection",
        FactState::Pending,
        Some("no enrichment selected on this run".to_string()),
    )];

    // CompleteForDeclaredScope: the run is complete for its declared scope
    // only when nothing required was capped, unresolved, or rejected.
    let scope_complete = !summary.scan_capped && unresolved == 0 && rejected == 0;
    let completion_facts = vec![required(
        "declared_scope_completion",
        if scope_complete {
            FactState::Available
        } else {
            FactState::Unknown
        },
        Some(if scope_complete {
            "required changed/affected analysis completed for the declared scope".to_string()
        } else {
            "capped, unresolved, or rejected scope keeps this run partial; see stage details"
                .to_string()
        }),
    )];

    // Configuration is required before a changed-first action (#2325), so it
    // is a required fact on the scope stage, not optional enrichment.
    let configuration_fact = match environment_digest {
        Some(digest) => required(
            "configuration_envelope",
            FactState::Available,
            Some(format!("environment {digest}")),
        ),
        None => required(
            "configuration_envelope",
            FactState::Pending,
            Some("no explicit envelope selected; select --features/--target".to_string()),
        ),
    };

    let pending_optional_work: Vec<String> = OPTIONAL_ENRICHMENT_CANDIDATES
        .iter()
        .map(|candidate| candidate.to_string())
        .collect();

    let mut stages = vec![
        {
            let mut facts = scope_facts;
            facts.push(configuration_fact);
            record(
                AnalysisStage::ScopeResolved,
                "input_resolution",
                facts,
                Vec::new(),
            )
        },
        record(
            AnalysisStage::ChangedSubjectsAnalyzed,
            "analyze",
            changed_facts,
            Vec::new(),
        ),
        record(
            AnalysisStage::AffectedSubjectsAnalyzed,
            "projections",
            affected_facts,
            Vec::new(),
        ),
        record(
            AnalysisStage::ActionReady,
            "policy_eval",
            action_facts,
            Vec::new(),
        ),
        record(
            AnalysisStage::Enriched,
            "projections",
            enrichment_facts,
            pending_optional_work.clone(),
        ),
        record(
            AnalysisStage::CompleteForDeclaredScope,
            "artifact_writes",
            completion_facts,
            Vec::new(),
        ),
    ];
    // Fixed emit order: derivation order above already matches, but sort by
    // stage so future edits cannot reorder the projection.
    stages.sort_by_key(|record| record.stage);

    let mut inventory = StageInventory {
        schema_version: STAGE_SCHEMA_VERSION,
        analysis_id: identity.analysis_id.clone(),
        source_generation: identity.generation,
        scope: output.scope.as_str().to_string(),
        stages,
        pending_optional_work,
        digest: String::new(),
    };
    let encoding = serde_json::to_string(&inventory).unwrap_or_default();
    inventory.digest = format!(
        "stages-sha256:{}",
        crate::sha256_hex_of(encoding.as_bytes())
    );
    inventory
}

/// Compact human stages section: stage completeness first, then every
/// non-available required fact with the flag or slice that supplies it, then
/// the pending optional count. Rendered only for explicit `--stages` runs so
/// default output stays byte-stable.
pub fn render_stages_human(inventory: &StageInventory) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Stages (analysis {}, generation {}, {}):\n",
        inventory.analysis_id, inventory.source_generation, inventory.digest
    ));
    for record in &inventory.stages {
        out.push_str(&format!(
            "- {} [{}]\n",
            record.stage.as_str(),
            record.completeness.as_str()
        ));
        for fact in &record.required_facts {
            if fact.state != FactState::Available {
                out.push_str(&format!(
                    "  - {}: {}{}\n",
                    fact.name,
                    fact.state.as_str(),
                    fact.detail
                        .as_deref()
                        .map(|detail| format!(" ({detail})"))
                        .unwrap_or_default()
                ));
            }
        }
    }
    out.push_str(&format!(
        "- pending optional work: {}\n",
        inventory.pending_optional_work.len()
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::PathBuf;

    fn analyzed_output() -> crate::AnalyzeOutput {
        crate::AnalyzeOutput {
            analysis_identity: crate::AnalysisIdentity::new("diff"),
            schema_version: "0.1".to_string(),
            tool: "unsafe-review".to_string(),
            root: PathBuf::from("."),
            scope: crate::Scope::Diff,
            mode: crate::AnalysisMode::Draft,
            policy: crate::PolicyMode::Advisory,
            summary: crate::api::Summary::default(),
            cards: Vec::new(),
            diff_scoped_files: BTreeSet::from([PathBuf::from("src/lib.rs")]),
            unresolved_diff_files: BTreeSet::new(),
            rejected_diff_files: BTreeSet::new(),
            coverage_snapshot: BTreeMap::new(),
        }
    }

    fn find_fact<'a>(record: &'a StageRecord, name: &str) -> Option<&'a FactRequirement> {
        record.required_facts.iter().find(|fact| fact.name == name)
    }

    fn find_stage(inventory: &StageInventory, wanted: AnalysisStage) -> Option<&StageRecord> {
        inventory
            .stages
            .iter()
            .find(|record| record.stage == wanted)
    }

    fn require_fact<'a>(
        record: &'a StageRecord,
        name: &str,
    ) -> Result<&'a FactRequirement, String> {
        find_fact(record, name).ok_or_else(|| format!("missing fact {name}"))
    }

    fn require_stage(
        inventory: &StageInventory,
        wanted: AnalysisStage,
    ) -> Result<&StageRecord, String> {
        find_stage(inventory, wanted).ok_or_else(|| format!("missing stage {}", wanted.as_str()))
    }

    #[test]
    fn clean_run_marks_scope_and_completion_complete() -> Result<(), String> {
        let inventory = assemble_stage_inventory(&analyzed_output(), None, None);
        if inventory.schema_version != STAGE_SCHEMA_VERSION {
            return Err("schema version must survive assembly".to_string());
        }
        if inventory.stages.len() != 6 {
            return Err("six stages must be present".to_string());
        }
        if !inventory.digest.starts_with("stages-sha256:") {
            return Err("digest must be namespaced".to_string());
        }
        // No explicit envelope on this run, so scope stays partial even
        // though nothing failed: configuration is required, not optional.
        if require_stage(&inventory, AnalysisStage::ScopeResolved)?.completeness
            != StageCompleteness::Partial
        {
            return Err("scope without envelope must stay partial".to_string());
        }
        if require_stage(&inventory, AnalysisStage::CompleteForDeclaredScope)?.completeness
            != StageCompleteness::Complete
        {
            return Err("declared scope must complete on a clean run".to_string());
        }
        // Opt-in facts stay pending with the exact flag that supplies them.
        let affected = require_fact(
            require_stage(&inventory, AnalysisStage::AffectedSubjectsAnalyzed)?,
            "affected_seam_analysis",
        )?;
        if affected.state != FactState::Pending
            || !affected
                .detail
                .as_deref()
                .unwrap_or_default()
                .contains("--impact")
        {
            return Err("affected analysis must name --impact".to_string());
        }
        let envelope = require_fact(
            require_stage(&inventory, AnalysisStage::ScopeResolved)?,
            "configuration_envelope",
        )?;
        if envelope.state != FactState::Pending
            || !envelope
                .detail
                .as_deref()
                .unwrap_or_default()
                .contains("--features")
        {
            return Err("envelope must name --features".to_string());
        }
        // Optional enrichment is named, never silently omitted.
        if inventory.pending_optional_work.len() != 7 {
            return Err("seven enrichment candidates must be named".to_string());
        }
        let human = render_stages_human(&inventory);
        if !human.contains("affected_seam_analysis: pending") {
            return Err("human render must name the pending fact".to_string());
        }
        Ok(())
    }

    #[test]
    fn inventory_is_deterministic_for_the_same_input() {
        let output = analyzed_output();
        let first = assemble_stage_inventory(&output, None, None);
        let second = assemble_stage_inventory(&output, None, None);
        assert_eq!(first.digest, second.digest);
        assert_eq!(first, second);
    }

    #[test]
    fn unresolved_paths_keep_scope_unknown_and_completion_partial() -> Result<(), String> {
        let mut output = analyzed_output();
        output.unresolved_diff_files = BTreeSet::from([PathBuf::from("gone.rs")]);
        let inventory = assemble_stage_inventory(&output, None, None);
        let scope = require_stage(&inventory, AnalysisStage::ScopeResolved)?;
        if scope.completeness != StageCompleteness::Partial {
            return Err("scope with unresolved paths must stay partial".to_string());
        }
        if require_fact(scope, "changed_file_inventory")?.state != FactState::Unknown {
            return Err("unresolved inventory must be unknown".to_string());
        }
        if require_stage(&inventory, AnalysisStage::CompleteForDeclaredScope)?.completeness
            != StageCompleteness::Partial
        {
            return Err("declared scope must stay partial".to_string());
        }
        Ok(())
    }

    #[test]
    fn capped_scan_blocks_the_changed_stage() -> Result<(), String> {
        let mut output = analyzed_output();
        output.summary.scan_capped = true;
        output.summary.card_cap = Some(25);
        let inventory = assemble_stage_inventory(&output, None, None);
        let changed = require_stage(&inventory, AnalysisStage::ChangedSubjectsAnalyzed)?;
        if changed.completeness != StageCompleteness::Blocked {
            return Err("capped scan must block the changed stage".to_string());
        }
        let cap = require_fact(changed, "uncapped_subject_scan")?;
        if cap.state != FactState::Failed
            || !cap.detail.as_deref().unwrap_or_default().contains("25")
        {
            return Err("cap failure must name the cap".to_string());
        }
        Ok(())
    }

    #[test]
    fn supplied_impact_and_envelope_mark_their_facts_available() -> Result<(), String> {
        let output = analyzed_output();
        let impact = crate::input::impact::ImpactInventory {
            schema_version: crate::input::impact::IMPACT_SCHEMA_VERSION,
            items: Vec::new(),
            affected: Vec::new(),
            limitations: Vec::new(),
            digest: String::new(),
        };
        let inventory =
            assemble_stage_inventory(&output, Some(&impact), Some("environment-sha256:abc"));
        let affected_stage = require_stage(&inventory, AnalysisStage::AffectedSubjectsAnalyzed)?;
        if require_fact(affected_stage, "affected_seam_analysis")?.state != FactState::Available {
            return Err("supplied impact must be available".to_string());
        }
        if affected_stage.completeness != StageCompleteness::Complete {
            return Err("affected stage must complete with impact".to_string());
        }
        let scope = require_stage(&inventory, AnalysisStage::ScopeResolved)?;
        if require_fact(scope, "configuration_envelope")?.state != FactState::Available {
            return Err("supplied envelope must be available".to_string());
        }
        if scope.completeness != StageCompleteness::Complete {
            return Err("scope must complete with an envelope".to_string());
        }
        Ok(())
    }
}
