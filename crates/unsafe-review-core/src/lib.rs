#![forbid(unsafe_code)]
//! Core SDK and analysis engine for `unsafe-review`.
//!
//! The public API is intentionally small: build an [`AnalyzeInput`], call
//! [`analyze`], and render or consume the returned [`AnalyzeOutput`].

mod analysis;
pub mod api;
mod candidate;
mod domain;
mod input;
mod output;
mod policy;
mod util;

pub use api::{
    AnalysisMode, AnalyzeInput, AnalyzeOutput, BaselineInitResult, DiffSource, DiscoveryOptions,
    FILE_TIMINGS_CAP, OutcomeReport, PerFileScanStats, PolicyMode, PolicyReport, Provenance,
    ReceiptAuditReport, RepoScanEvent, RepoScanPhase, RepoScanStatus, RepoStopReason,
    ReviewCardConfirmationProjection, Scope, analyze, analyze_with_discovery,
    analyze_with_discovery_and_progress, analyze_with_discovery_and_repo_events,
    audit_witness_receipts, baseline_add, baseline_init, bless_fixture_card_goldens,
    collect_context, collect_context_range, compare_outcome_json, discover_repo_files,
    evaluate_policy_report, evaluate_policy_report_from_output, explain_card, project_editor,
    project_review_card_confirmation, render_badge_jsons, render_comment_plan,
    render_gate_manifest, render_github_summary, render_human, render_json,
    render_json_with_provenance, render_lsp, render_lsp_hover, render_markdown,
    render_outcome_json, render_outcome_markdown, render_policy_report_json,
    render_policy_report_markdown, render_pr_summary, render_receipt_audit_json,
    render_receipt_audit_markdown, render_repair_queue, render_sarif, render_usefulness_telemetry,
    render_witness_plan, validate_witness_receipts,
};

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
