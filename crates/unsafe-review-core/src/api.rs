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

/// Why a repo scan stopped short of scanning every discovered file.
///
/// A `Complete` scan has `stop_reason: None` (or equivalently `"none"`
/// in the JSON sidecar).  Every other variant indicates a bounded-but-partial
/// run; `completed` stays `false` for all partial variants.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RepoStopReason {
    /// Scan ran to completion — every in-scope file was read.
    None,
    /// `--max-cards N` was reached; scanning stopped after `N` cards were emitted.
    MaxCards,
    /// `--timeout-seconds N` elapsed while the scan was in progress.
    Timeout,
    /// A unix signal (SIGTERM / SIGINT) interrupted the scan.
    Terminated,
    /// The scan did not complete due to an analysis or report-write error
    /// (anything that is not a timeout, signal, or cap).
    Error,
}

impl RepoStopReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::MaxCards => "max_cards",
            Self::Timeout => "timeout",
            Self::Terminated => "terminated",
            Self::Error => "error",
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
    /// Whether this is a partial (bounded) scan result.
    /// `true` for max-cards, timeout, and signal-terminated scans.
    pub partial: bool,
    /// The reason the scan stopped.  `None` for a complete scan, or one of the
    /// named stop reasons for a bounded/interrupted scan.
    pub stop_reason: RepoStopReason,
    /// The configured card cap when `stop_reason == MaxCards`; `None` otherwise.
    pub cap: Option<usize>,
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

/// Traceable evidence metadata that the CLI layer assembles from argv, git, and the
/// filesystem before calling the JSON renderer.
///
/// This is "traceable evidence metadata", not proof: the fields identify the inputs
/// used to produce an artifact so that two runs against different diffs cannot emit
/// byte-identical clean receipts, but they do not prove correctness or memory safety.
#[derive(Clone, Debug, Default)]
pub struct Provenance {
    /// Absolute path of the resolved workspace root (additive alongside the existing
    /// relative `root` field which remains unchanged for compatibility).
    pub root_abs: Option<String>,
    /// Resolved base commit SHA (when `--base` was supplied and git resolution succeeded).
    pub base_sha: Option<String>,
    /// Resolved HEAD commit SHA (when `--base` was supplied and git resolution succeeded).
    pub head_sha: Option<String>,
    /// Path of the diff file (when `--diff <file>` was supplied).
    pub diff_path: Option<String>,
    /// SHA-256 hex digest of the diff file content (when `--diff <file>` was supplied).
    pub diff_sha256: Option<String>,
    /// RFC3339 UTC timestamp at which the artifact was generated.
    pub generated_at: String,
    /// Whether the working tree had uncommitted changes when the tool ran (None = git unavailable).
    pub dirty_worktree: Option<bool>,
}

impl Provenance {
    /// Build a minimal provenance block stamped with the current UTC time.
    pub fn new_now() -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Self {
            generated_at: unix_secs_to_iso_datetime_utc(secs),
            ..Self::default()
        }
    }
}

/// Regenerate `expected.cards.json` for each named fixture (or all registered
/// fixtures if `names` is empty), always writing LF line endings.
///
/// Called by `cargo run -p xtask -- bless-goldens [fixture ...]`.
/// Does not execute witnesses or assess soundness.
pub fn bless_fixture_card_goldens(names: &[&str]) -> Result<Vec<PathBuf>, String> {
    json::bless_fixture_card_goldens(names)
}

pub fn render_json(output: &AnalyzeOutput) -> String {
    json::render(output)
}

/// Render the JSON analyze artifact with attached traceable evidence metadata.
///
/// The `provenance` block is inserted as a nested object in the output.
/// `tool_version` also appears top-level beside `tool` for consumer grep-ability.
pub fn render_json_with_provenance(output: &AnalyzeOutput, provenance: &Provenance) -> String {
    json::render_with_provenance(output, provenance)
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

/// Result returned by `baseline_init` summarizing what was captured.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BaselineInitResult {
    /// Number of open actionable cards captured as baseline entries.
    pub captured: usize,
    /// Whether the baseline ledger file already existed before this run.
    pub ledger_existed: bool,
    /// Path to the baseline ledger written.
    pub ledger_path: PathBuf,
    /// Path to the coverage snapshot written.
    pub snapshot_path: PathBuf,
}

/// `baseline init` (SPEC-0030): scan the repo for open actionable cards, capture each
/// card's identity and coverage state, and write both the baseline ledger and the coverage
/// snapshot.  Idempotent — re-running overwrites with a fresh snapshot of the current state.
///
/// The honest default `reason` and `review_after` are set to record pre-existing debt only;
/// no card is marked safe, reviewed, or UB-free.
///
/// `review_after` defaults to one year from today's date.
pub fn baseline_init(
    root: &Path,
    out: Option<&Path>,
    review_after: Option<&str>,
) -> Result<BaselineInitResult, String> {
    use crate::domain::coverage::CoverageBlock;
    use crate::policy::{
        LedgerEntry, SnapshotCoverage, merge_and_write_baseline_ledger, write_coverage_snapshot,
    };
    use std::collections::BTreeMap;

    let ledger_path = out
        .map(Path::to_path_buf)
        .unwrap_or_else(|| root.join("policy/unsafe-review-baseline.toml"));
    let snapshot_path = baseline_snapshot_path(&ledger_path);
    let ledger_existed = ledger_path.is_file();

    // Run a full repo scan to get all current cards.
    let output = pipeline::analyze(AnalyzeInput {
        root: root.to_path_buf(),
        scope: Scope::Repo,
        diff: DiffSource::NoneRepoScan,
        mode: AnalysisMode::Repo,
        policy: PolicyMode::Advisory,
        include_unchanged_tests: true,
        max_cards: None,
    })?;

    // Determine review_after date (required by ledger validator).
    let review_after = review_after
        .map(ToOwned::to_owned)
        .unwrap_or_else(default_review_after_date);

    // Collect open actionable cards.
    let mut ledger_entries: Vec<LedgerEntry> = Vec::new();
    let mut snapshot_entries: BTreeMap<String, SnapshotCoverage> = BTreeMap::new();

    for card in &output.cards {
        if card.class.is_actionable() {
            ledger_entries.push(LedgerEntry {
                card_id: card.id.0.clone(),
                owner: "baseline-init".to_string(),
                reason: "captured by `baseline init`; pre-existing debt, not reviewed as safe"
                    .to_string(),
                evidence: "baseline-init: captured by baseline init; pre-existing debt".to_string(),
                review_after: Some(review_after.clone()),
                expires: None,
            });
            let block = CoverageBlock::derive(card);
            snapshot_entries.insert(
                card.id.0.clone(),
                SnapshotCoverage {
                    contract_coverage: block.contract_coverage.as_str().to_string(),
                    guard_coverage: block.guard_coverage.as_str().to_string(),
                    test_reach_coverage: block.test_reach_coverage.as_str().to_string(),
                    witness_receipt_coverage: block.witness_receipt_coverage.as_str().to_string(),
                },
            );
        }
    }

    let captured = ledger_entries.len();
    merge_and_write_baseline_ledger(&ledger_path, &ledger_entries)?;
    write_coverage_snapshot(&snapshot_path, &snapshot_entries)?;

    Ok(BaselineInitResult {
        captured,
        ledger_existed,
        ledger_path,
        snapshot_path,
    })
}

/// `baseline add` (SPEC-0030): add or update a single baseline entry (plus its snapshot state)
/// by re-analyzing the repo, finding the card matching `card_id`, and recording its current
/// coverage state.
///
/// Returns `Err` if the card cannot be found in the current scan.
pub fn baseline_add(
    root: &Path,
    card_id: &str,
    owner: &str,
    reason: &str,
    evidence: &str,
    review_after: Option<&str>,
    out: Option<&Path>,
) -> Result<(), String> {
    use crate::domain::coverage::CoverageBlock;
    use crate::policy::{
        LedgerEntry, SnapshotCoverage, load_coverage_snapshot, merge_and_write_baseline_ledger,
        write_coverage_snapshot,
    };
    use std::collections::BTreeMap;

    let ledger_path = out
        .map(Path::to_path_buf)
        .unwrap_or_else(|| root.join("policy/unsafe-review-baseline.toml"));
    let snapshot_path = baseline_snapshot_path(&ledger_path);

    // Run a full repo scan.
    let output = pipeline::analyze(AnalyzeInput {
        root: root.to_path_buf(),
        scope: Scope::Repo,
        diff: DiffSource::NoneRepoScan,
        mode: AnalysisMode::Repo,
        policy: PolicyMode::Advisory,
        include_unchanged_tests: true,
        max_cards: None,
    })?;

    // Find the specific card.
    let card = output
        .cards
        .iter()
        .find(|card| card.id.0 == card_id)
        .ok_or_else(|| format!("card `{card_id}` not found in current repo scan"))?;

    let review_after = review_after
        .map(ToOwned::to_owned)
        .unwrap_or_else(default_review_after_date);

    let entry = LedgerEntry {
        card_id: card_id.to_string(),
        owner: owner.to_string(),
        reason: reason.to_string(),
        evidence: evidence.to_string(),
        review_after: Some(review_after),
        expires: None,
    };

    // Update the snapshot.
    let mut snapshot = load_coverage_snapshot(&snapshot_path)?;
    let block = CoverageBlock::derive(card);
    snapshot.insert(
        card_id.to_string(),
        SnapshotCoverage {
            contract_coverage: block.contract_coverage.as_str().to_string(),
            guard_coverage: block.guard_coverage.as_str().to_string(),
            test_reach_coverage: block.test_reach_coverage.as_str().to_string(),
            witness_receipt_coverage: block.witness_receipt_coverage.as_str().to_string(),
        },
    );

    // Sort snapshot to BTreeMap (already sorted).
    let sorted_snapshot: BTreeMap<String, SnapshotCoverage> = snapshot.into_iter().collect();

    merge_and_write_baseline_ledger(&ledger_path, &[entry])?;
    write_coverage_snapshot(&snapshot_path, &sorted_snapshot)?;

    Ok(())
}

/// Derive the coverage snapshot path from the baseline ledger path: the snapshot is written
/// as a sibling `<ledger-stem>-snapshot.toml`. The default ledger
/// `policy/unsafe-review-baseline.toml` keeps producing
/// `policy/unsafe-review-baseline-snapshot.toml`, and a custom `--out` keeps both files
/// together instead of writing the snapshot into the scanned `--root` (which would edit a
/// repo unsafe-review only promised to read).
fn baseline_snapshot_path(ledger_path: &Path) -> PathBuf {
    let stem = ledger_path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "unsafe-review-baseline".to_string());
    ledger_path.with_file_name(format!("{stem}-snapshot.toml"))
}

/// Default `review_after` date: one year from today (ISO 8601 YYYY-MM-DD).
fn default_review_after_date() -> String {
    // Use a fixed date offset from June 2026 (the current date per context).
    // We can't use std::time for date arithmetic without chrono, so we hardcode
    // a safe one-year increment from a known epoch base.
    // This is called only in baseline authoring, not in analysis; precision is not critical.
    compute_review_after_date()
}

fn compute_review_after_date() -> String {
    // Use SystemTime to get the current date offset by ~365 days.
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Approximate: add 365 days worth of seconds.
    let future_secs = secs + 365 * 24 * 3600;
    // Convert to a YYYY-MM-DD string using a simple algorithm.
    unix_secs_to_iso_date(future_secs)
}

/// Extend the date-only helper to a full RFC3339 UTC timestamp (e.g. `2026-06-07T21:30:00Z`).
///
/// The time portion is always `T00:00:00Z` (midnight UTC) because we only have
/// second-level granularity and already discard the sub-day remainder in the
/// date calculation.  For a provenance `generated_at` field this is sufficient
/// — the date binds the artifact to a calendar day without requiring chrono.
pub(crate) fn unix_secs_to_iso_datetime_utc(secs: u64) -> String {
    let date = unix_secs_to_iso_date(secs);
    // Compute HH:MM:SS from the remaining seconds in the day.
    let remainder = secs % 86400;
    let hh = remainder / 3600;
    let mm = (remainder % 3600) / 60;
    let ss = remainder % 60;
    format!("{date}T{hh:02}:{mm:02}:{ss:02}Z")
}

fn unix_secs_to_iso_date(secs: u64) -> String {
    // Days since Unix epoch.
    let days = secs / 86400;
    // Gregorian calendar calculation.
    let mut remaining_days = days;
    let mut year = 1970u32;
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }
    let mut month = 1u32;
    loop {
        let days_in_month = days_in_month(year, month);
        if remaining_days < days_in_month {
            break;
        }
        remaining_days -= days_in_month;
        month += 1;
    }
    let day = remaining_days + 1;
    format!("{year:04}-{month:02}-{day:02}")
}

fn is_leap_year(year: u32) -> bool {
    year.is_multiple_of(400) || (year.is_multiple_of(4) && !year.is_multiple_of(100))
}

fn days_in_month(year: u32, month: u32) -> u64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
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

    #[test]
    fn baseline_snapshot_path_keeps_default_canonical_location() {
        let ledger = Path::new("repo/policy/unsafe-review-baseline.toml");
        assert_eq!(
            baseline_snapshot_path(ledger),
            PathBuf::from("repo/policy/unsafe-review-baseline-snapshot.toml")
        );
    }

    #[test]
    fn baseline_snapshot_path_follows_custom_out_as_sibling() {
        let ledger = Path::new("elsewhere/bun-baseline.toml");
        assert_eq!(
            baseline_snapshot_path(ledger),
            PathBuf::from("elsewhere/bun-baseline-snapshot.toml")
        );
    }

    #[test]
    fn baseline_snapshot_path_handles_extension_less_out() {
        let ledger = Path::new("elsewhere/baseline");
        assert_eq!(
            baseline_snapshot_path(ledger),
            PathBuf::from("elsewhere/baseline-snapshot.toml")
        );
    }
}
