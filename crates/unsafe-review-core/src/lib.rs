#![forbid(unsafe_code)]
//! Core SDK and analysis engine for `unsafe-review`.
//!
//! The public API is intentionally small: build an [`AnalyzeInput`], call
//! [`analyze`], and render or consume the returned [`AnalyzeOutput`].

mod analysis;
pub mod api;
mod domain;
mod input;
mod output;
mod policy;
mod util;

pub use api::{
    AnalysisMode, AnalyzeInput, AnalyzeOutput, DiffSource, OutcomeReport, PolicyMode, PolicyReport,
    ReceiptAuditReport, Scope, analyze, audit_witness_receipts, collect_context,
    compare_outcome_json, evaluate_policy_report, explain_card, render_badge_jsons,
    render_comment_plan, render_human, render_json, render_lsp, render_markdown,
    render_outcome_json, render_outcome_markdown, render_policy_report_json,
    render_policy_report_markdown, render_pr_summary, render_receipt_audit_json,
    render_receipt_audit_markdown, render_sarif, render_witness_plan, validate_witness_receipts,
};
pub use domain::{
    CardId, CargoCarefulReceiptInput, ConcurrencyReceiptInput, Confidence, ContractEvidence,
    DischargeEvidence, HazardKind, MiriReceiptInput, MissingEvidence, NextAction, Priority,
    ProofReceiptInput, ReachEvidence, RelatedTest, ReviewCard, ReviewClass, SafetyObligation,
    SanitizerReceiptInput, SourceLocation, UnsafeOperation, UnsafeSite,
    WITNESS_RECEIPT_SCHEMA_VERSION, WitnessEvidence, WitnessKind, WitnessReceipt, WitnessRoute,
};
