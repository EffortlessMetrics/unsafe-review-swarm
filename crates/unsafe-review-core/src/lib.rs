#![forbid(unsafe_code)]
//! Core SDK and analysis engine for `unsafe-review`.
//!
//! The public API is intentionally small: build an [`AnalyzeInput`], call
//! [`analyze`], and render or consume the returned [`AnalyzeOutput`].

mod analysis;
pub mod api;
mod candidate;
mod domain;
pub mod freshness;
mod input;
mod output;
mod policy;
mod util;

pub use output::agent::{
    RepairCandidate, RepairCandidateApplicability, RepairCandidateKind, RepairCandidatePosition,
    RepairCandidateRange, RepairCandidateTarget, RepairEvidenceMovement,
};
pub use output::comment_plan::COMMENT_BODY_WORD_LIMIT;
pub use output::lsp::{
    EditorActionApplicability, EditorActionArguments, EditorActionCommand, EditorActionContract,
    EditorActionDiagnostic, EditorActionPayload, EditorActionReadiness, EditorCoverageBlock,
    EditorDiagnostic, EditorEvidenceState, EditorEvidenceSummary, EditorObligationEvidence,
    EditorPosition, EditorProjection, EditorRange, EditorReachEvidence, EditorSafetyCondition,
    EditorSimpleEvidence, EditorWitnessRoute, actions_for_card,
};
pub use policy::baseline_ledger_path;

pub use api::{
    AnalysisMode, AnalyzeInput, AnalyzeOutput, BaselineHealthCounts, BaselineHealthEntry,
    BaselineHealthReport, BaselineInitResult, BaselineRefreshPlan, DiffSource, DiscoveryOptions,
    FILE_TIMINGS_CAP, HealthBucket, OutcomeReport, PerFileScanStats, PolicyMode, PolicyReport,
    Provenance, ReceiptAuditReport, RefreshAction, RefreshPlanEntry, RefreshPlanSummary,
    RepoScanEvent, RepoScanPhase, RepoScanStatus, RepoStopReason, ReviewCardConfirmationProjection,
    ScanCost, Scope, analyze, analyze_with_discovery, analyze_with_discovery_and_progress,
    analyze_with_discovery_and_repo_events, audit_witness_receipts, baseline_add, baseline_init,
    baseline_init_preview, baseline_refresh_preview, baseline_status, bless_fixture_card_goldens,
    bless_fixture_card_goldens_from_workspace, bless_fixture_surface_goldens,
    bless_fixture_surface_goldens_from_workspace, collect_context, collect_context_range,
    compare_outcome_json, discover_repo_files, evaluate_policy_report,
    evaluate_policy_report_from_output, explain_card, project_actionable_editor_diagnostics,
    project_editor, project_editor_diagnostics, project_review_card_confirmation,
    render_badge_jsons, render_baseline_refresh_human, render_baseline_refresh_json,
    render_baseline_status_human, render_baseline_status_json, render_comment_plan,
    render_fixture_surface, render_fixture_surface_from_workspace, render_gate_manifest,
    render_gate_manifest_repo, render_github_summary, render_human, render_json,
    render_json_with_provenance, render_lsp, render_lsp_hover, render_markdown,
    render_outcome_json, render_outcome_markdown, render_policy_report_json,
    render_policy_report_markdown, render_pr_summary, render_receipt_audit_json,
    render_receipt_audit_markdown, render_repair_queue, render_sarif, render_usefulness_telemetry,
    render_usefulness_telemetry_with_cost, render_witness_plan, validate_witness_receipts,
};
pub use freshness::{AnalysisIdentity, AnalysisState};

/// Compute the SHA-256 hex digest of raw bytes.
///
/// Exposed for use in the CLI layer where the diff content or file bytes need to be
/// bound to the JSON artifact via a collision-resistant digest.
pub fn sha256_hex_of(data: &[u8]) -> String {
    util::sha256_hex(data)
}
pub use candidate::{
    MANUAL_CANDIDATE_SCHEMA_VERSION, MANUAL_CANDIDATE_STABLE_BYTE_CLASSES,
    MANUAL_CANDIDATE_TRUST_BOUNDARY, ManualCandidate, ManualCandidateEvidence,
    ManualCandidateLocation, ManualCandidateOracleMap, ManualCandidateProofMode,
    ManualCandidateStableByte, lint_manual_candidate_text, load_manual_candidate,
    load_manual_candidates, manual_candidate_implementer_handoff, manual_candidate_path,
    new_manual_candidate_skeleton, read_manual_candidate, render_manual_candidate_context,
    render_manual_candidate_explain, render_manual_candidate_witness_plan,
};
pub use domain::{
    CardId, CargoCarefulReceiptInput, ConcurrencyReceiptInput, Confidence, ContractEvidence,
    DischargeEvidence, HazardKind, MiriReceiptInput, MissingEvidence, NextAction, Priority,
    ProofPath, ProofReceiptInput, ReachEvidence, ReceiptCardIdKind, RelatedTest, ReviewCard,
    ReviewClass, SafetyObligation, SanitizerReceiptInput, SourceLocation, UnsafeOperation,
    UnsafeSite, WITNESS_RECEIPT_SCHEMA_VERSION, WitnessEvidence, WitnessKind, WitnessReceipt,
    WitnessRoute,
};
