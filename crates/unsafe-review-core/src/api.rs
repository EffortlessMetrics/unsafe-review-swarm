use crate::analysis::{pipeline, receipts};
use crate::domain::{CardId, ReviewCard};
use crate::input::workspace;
use crate::output::{
    agent, badges, comment_plan, human, json, lsp, markdown, outcome, policy_report, receipt_audit,
    repair_queue, sarif, witness_plan,
};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scope {
    Diff,
    Repo,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AnalysisMode {
    Instant,
    Draft,
    Ready,
    Repo,
}

impl AnalysisMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Instant => "instant",
            Self::Draft => "draft",
            Self::Ready => "ready",
            Self::Repo => "repo",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PolicyMode {
    Advisory,
    NoNewDebt,
    Blocking,
}

impl PolicyMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Advisory => "advisory",
            Self::NoNewDebt => "no-new-debt",
            Self::Blocking => "blocking",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiffSource {
    NoneRepoScan,
    Text(String),
    File(PathBuf),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RepoScanPhase {
    Discovering,
    Scanning,
    Complete,
}

impl RepoScanPhase {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Discovering => "discovering",
            Self::Scanning => "scanning",
            Self::Complete => "complete",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoScanStatus {
    pub schema_version: String,
    pub phase: RepoScanPhase,
    pub elapsed_ms: u64,
    pub files_discovered: usize,
    pub files_scanned: usize,
    pub cards_found: usize,
    pub last_path: Option<PathBuf>,
    pub completed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveryOptions {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub respect_gitignore: bool,
    pub large_repo_ignores: bool,
    pub max_files: Option<usize>,
}

impl Default for DiscoveryOptions {
    fn default() -> Self {
        Self {
            include: Vec::new(),
            exclude: Vec::new(),
            respect_gitignore: false,
            large_repo_ignores: false,
            max_files: None,
        }
    }
}

impl DiscoveryOptions {
    pub fn repo_defaults() -> Self {
        Self {
            respect_gitignore: true,
            large_repo_ignores: true,
            ..Self::default()
        }
    }
}

#[derive(Clone, Debug)]
pub struct AnalyzeInput {
    pub root: PathBuf,
    pub scope: Scope,
    pub diff: DiffSource,
    pub mode: AnalysisMode,
    pub policy: PolicyMode,
    pub include_unchanged_tests: bool,
    pub max_cards: Option<usize>,
}

impl Default for AnalyzeInput {
    fn default() -> Self {
        Self {
            root: PathBuf::from("."),
            scope: Scope::Diff,
            diff: DiffSource::NoneRepoScan,
            mode: AnalysisMode::Draft,
            policy: PolicyMode::Advisory,
            include_unchanged_tests: true,
            max_cards: None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Summary {
    pub rust_files: usize,
    pub changed_rust_files: usize,
    pub unsafe_sites: usize,
    pub cards: usize,
    pub open_actionable_gaps: usize,
    pub contract_missing: usize,
    pub guard_missing: usize,
    pub guarded_unwitnessed: usize,
    pub unsafe_unreached: usize,
    pub requires_loom: usize,
    pub miri_unsupported: usize,
    pub static_unknown: usize,
}

#[derive(Clone, Debug)]
pub struct AnalyzeOutput {
    pub schema_version: String,
    pub tool: String,
    pub root: PathBuf,
    pub scope: Scope,
    pub mode: AnalysisMode,
    pub policy: PolicyMode,
    pub summary: Summary,
    pub cards: Vec<ReviewCard>,
}

pub fn analyze(input: AnalyzeInput) -> Result<AnalyzeOutput, String> {
    pipeline::analyze(input)
}

pub fn analyze_with_discovery(
    input: AnalyzeInput,
    discovery: DiscoveryOptions,
) -> Result<AnalyzeOutput, String> {
    pipeline::analyze_with_discovery(input, discovery)
}

pub fn analyze_with_discovery_and_progress<F>(
    input: AnalyzeInput,
    discovery: DiscoveryOptions,
    progress: F,
) -> Result<AnalyzeOutput, String>
where
    F: FnMut(&RepoScanStatus) -> Result<(), String>,
{
    pipeline::analyze_with_discovery_and_progress(input, discovery, progress)
}

pub fn discover_repo_files(
    root: PathBuf,
    discovery: DiscoveryOptions,
) -> Result<Vec<PathBuf>, String> {
    workspace::discover_rust_files(&root, &discovery)
}

pub fn validate_witness_receipts(root: PathBuf) -> Result<usize, String> {
    receipts::validate_receipts(&root)
}

pub fn audit_witness_receipts(input: AnalyzeInput) -> Result<ReceiptAuditReport, String> {
    let output = pipeline::analyze_without_receipts(input)?;
    receipts::audit_receipts(&output)
}

pub fn evaluate_policy_report(mut input: AnalyzeInput) -> Result<PolicyReport, String> {
    input.policy = PolicyMode::Advisory;
    let output = pipeline::analyze(input)?;
    policy_report::evaluate(&output)
}

pub fn render_json(output: &AnalyzeOutput) -> String {
    json::render(output)
}

pub fn render_human(output: &AnalyzeOutput) -> String {
    human::render(output)
}

pub fn render_markdown(output: &AnalyzeOutput) -> String {
    markdown::render(output)
}

pub fn render_pr_summary(output: &AnalyzeOutput) -> String {
    markdown::render_pr_summary(output)
}

pub fn render_github_summary(output: &AnalyzeOutput) -> String {
    markdown::render_github_summary(output)
}

pub fn render_sarif(output: &AnalyzeOutput) -> String {
    sarif::render(output)
}

pub fn render_comment_plan(output: &AnalyzeOutput) -> String {
    comment_plan::render(output)
}

pub fn render_lsp(output: &AnalyzeOutput) -> String {
    lsp::render(output)
}

pub fn project_editor(output: &AnalyzeOutput) -> lsp::EditorProjection {
    lsp::project_editor(output)
}

pub fn render_witness_plan(output: &AnalyzeOutput) -> String {
    witness_plan::render(output)
}

pub fn render_repair_queue(output: &AnalyzeOutput) -> String {
    repair_queue::render(output)
}

pub fn render_badge_jsons(output: &AnalyzeOutput) -> (String, String) {
    badges::render(output)
}

pub fn compare_outcome_json(before_json: &str, after_json: &str) -> Result<OutcomeReport, String> {
    outcome::compare_json(before_json, after_json)
}

pub fn render_outcome_json(report: &OutcomeReport) -> String {
    outcome::render_json(report)
}

pub fn render_outcome_markdown(report: &OutcomeReport) -> String {
    outcome::render_markdown(report)
}

pub fn render_receipt_audit_json(report: &ReceiptAuditReport) -> String {
    receipt_audit::render_json(report)
}

pub fn render_receipt_audit_markdown(report: &ReceiptAuditReport) -> String {
    receipt_audit::render_markdown(report)
}

pub fn render_policy_report_json(report: &PolicyReport) -> String {
    policy_report::render_json(report)
}

pub fn render_policy_report_markdown(report: &PolicyReport) -> String {
    policy_report::render_markdown(report)
}

pub fn explain_card(output: &AnalyzeOutput, id: &CardId) -> Option<String> {
    output
        .cards
        .iter()
        .find(|card| &card.id == id)
        .map(markdown::render_card_detail)
}

pub fn collect_context(output: &AnalyzeOutput, id: &CardId) -> Option<String> {
    output
        .cards
        .iter()
        .find(|card| &card.id == id)
        .map(agent::render)
}

pub use outcome::OutcomeReport;
pub use policy_report::PolicyReport;
pub use receipts::ReceiptAuditReport;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analysis_mode_strings_cover_every_variant() {
        assert_eq!(AnalysisMode::Instant.as_str(), "instant");
        assert_eq!(AnalysisMode::Draft.as_str(), "draft");
        assert_eq!(AnalysisMode::Ready.as_str(), "ready");
        assert_eq!(AnalysisMode::Repo.as_str(), "repo");
    }

    #[test]
    fn policy_mode_strings_cover_every_variant() {
        assert_eq!(PolicyMode::Advisory.as_str(), "advisory");
        assert_eq!(PolicyMode::NoNewDebt.as_str(), "no-new-debt");
        assert_eq!(PolicyMode::Blocking.as_str(), "blocking");
    }

    #[test]
    fn analyze_input_default_is_advisory_diff_draft_with_unchanged_tests() {
        let input = AnalyzeInput::default();

        assert_eq!(input.root, PathBuf::from("."));
        assert_eq!(input.scope, Scope::Diff);
        assert_eq!(input.diff, DiffSource::NoneRepoScan);
        assert_eq!(input.mode, AnalysisMode::Draft);
        assert_eq!(input.policy, PolicyMode::Advisory);
        assert!(input.include_unchanged_tests);
        assert_eq!(input.max_cards, None);
    }
}
