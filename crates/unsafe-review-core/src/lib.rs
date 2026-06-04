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
    AnalysisMode, AnalyzeInput, AnalyzeOutput, DiffSource, DiscoveryOptions, OutcomeReport,
    PolicyMode, PolicyReport, ReceiptAuditReport, RepoScanEvent, RepoScanPhase, RepoScanStatus,
    ReviewCardConfirmationProjection, Scope, analyze, analyze_with_discovery,
    analyze_with_discovery_and_progress, analyze_with_discovery_and_repo_events,
    audit_witness_receipts, collect_context, compare_outcome_json, discover_repo_files,
    evaluate_policy_report, evaluate_policy_report_from_output, explain_card, project_editor,
    project_review_card_confirmation, render_badge_jsons, render_comment_plan,
    render_github_summary, render_human, render_json, render_lsp, render_markdown,
    render_outcome_json, render_outcome_markdown, render_policy_report_json,
    render_policy_report_markdown, render_pr_summary, render_receipt_audit_json,
    render_receipt_audit_markdown, render_repair_queue, render_sarif, render_witness_plan,
    validate_witness_receipts,
};
pub use candidate::{
    MANUAL_CANDIDATE_SCHEMA_VERSION, ManualCandidate, ManualCandidateEvidence,
    ManualCandidateLocation, ManualCandidateProofMode, ManualCandidateStableByte,
    load_manual_candidate, load_manual_candidates, manual_candidate_implementer_handoff,
    manual_candidate_path, read_manual_candidate, render_manual_candidate_context,
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
