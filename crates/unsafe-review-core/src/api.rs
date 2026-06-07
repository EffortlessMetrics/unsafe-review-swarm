use crate::analysis::{pipeline, receipts};
use crate::domain::{CardId, ReviewCard};
use crate::input::workspace;
use crate::output::{
    agent, badges, comment_plan, confirmation, gate_manifest, human, json, lsp, markdown, outcome,
    policy_report, receipt_audit, repair_queue, sarif, witness_plan,
};
use crate::util::path_display;
use std::path::{Path, PathBuf};

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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DiscoveryOptions {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub respect_gitignore: bool,
    pub large_repo_ignores: bool,
    pub max_files: Option<usize>,
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
    pub changed_files: usize,
    pub changed_rust_files: usize,
    pub changed_non_rust_files: usize,
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
    /// Coverage movement counts (SPEC-0030).
    ///
    /// `new_gaps`      — open actionable cards not in the baseline ledger.
    /// `worsened_gaps` — baseline cards whose coverage regressed (requires a saved coverage
    ///                   snapshot; always 0 until `baseline init` authoring lands).
    /// `resolved_gaps` — baseline ledger entries whose card is no longer present.
    /// `inherited_gaps`— baseline-known cards still open and unchanged.
    ///
    /// On a diff-scoped run `new_gaps` is constrained to changed-line sites;
    /// on a repo-mode run it counts all open actionable non-baseline gaps.
    pub new_gaps: usize,
    pub worsened_gaps: usize,
    pub resolved_gaps: usize,
    pub inherited_gaps: usize,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewCardConfirmationProjection {
    pub hypothesis_to_confirm: String,
    pub build_this_first: String,
    pub minimal_repro_steps: Vec<String>,
    pub minimal_repro_limitation: String,
    pub confirmation_step: String,
}

#[derive(Clone, Debug)]
pub struct RepoScanEvent {
    pub status: RepoScanStatus,
    pub partial_output: Option<AnalyzeOutput>,
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

pub fn analyze_with_discovery_and_repo_events<F>(
    input: AnalyzeInput,
    discovery: DiscoveryOptions,
    events: F,
) -> Result<AnalyzeOutput, String>
where
    F: FnMut(&RepoScanEvent) -> Result<(), String>,
{
    pipeline::analyze_with_discovery_and_repo_events(input, discovery, events)
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

pub fn evaluate_policy_report_from_output(output: &AnalyzeOutput) -> Result<PolicyReport, String> {
    policy_report::evaluate(output)
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

/// Render the `unsafe-review-gate.json` routing manifest (SPEC-0034).
///
/// The manifest is a thin index over the artifacts a `first-pr`/`repo` run
/// produced, plus the SPEC-0030 movement summary.  It is fully deterministic —
/// it carries no timestamp or wall-time field — so it is safe to include in
/// byte-compared goldens or reproducibility rails.
pub fn render_gate_manifest(output: &AnalyzeOutput) -> String {
    gate_manifest::render(output)
}

pub fn project_review_card_confirmation(card: &ReviewCard) -> ReviewCardConfirmationProjection {
    let minimal_repro = confirmation::minimal_repro(card);
    ReviewCardConfirmationProjection {
        hypothesis_to_confirm: confirmation::hypothesis_to_confirm(card),
        build_this_first: confirmation::build_this_first(card).summary,
        minimal_repro_steps: minimal_repro.steps().to_vec(),
        minimal_repro_limitation: minimal_repro.limitation().to_string(),
        confirmation_step: confirmation::confirmation_step(card),
    }
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

/// Render a `file_range_scan` envelope for SPEC-0033.
///
/// Collects packets for all cards whose unsafe site overlaps `file:line_start-line_end`
/// (1-based, both endpoints inclusive).  If `changed_only` is `true`, further
/// restricts to cards whose `baseline_state` is `new` or `worsened` (SPEC-0030).
///
/// File paths are matched by normalizing both to forward-slash display form,
/// then checking whether the card's site file ends with the queried fragment so
/// callers may pass either root-relative or short relative paths (e.g. `src/lib.rs`).
/// The `root` parameter is used to strip a leading root prefix from the queried path
/// before the suffix comparison.
pub fn collect_context_range(
    output: &AnalyzeOutput,
    root: &Path,
    file: &Path,
    line_start: u32,
    line_end: u32,
    changed_only: bool,
) -> String {
    let queried_display = path_display(file);
    let root_display = path_display(root);

    // Strip the workspace root prefix from the queried path so that callers
    // can use either a short relative path ("src/lib.rs") or an absolute one.
    let queried_suffix = queried_display
        .strip_prefix(&root_display)
        .map(|rest| rest.trim_start_matches('/'))
        .unwrap_or(&queried_display);

    // Pre-filter to the requested file; range + changed-only filtering happens
    // inside render_range_scan.
    let file_cards: Vec<&ReviewCard> = output
        .cards
        .iter()
        .filter(|card| {
            let card_file = path_display(&card.site.location.file);
            card_file == queried_display
                || card_file == queried_suffix
                || card_file.ends_with(&format!("/{queried_suffix}"))
        })
        .collect();

    agent::render_range_scan(
        queried_display,
        line_start,
        line_end,
        changed_only,
        &file_cards,
        &output.schema_version,
    )
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
