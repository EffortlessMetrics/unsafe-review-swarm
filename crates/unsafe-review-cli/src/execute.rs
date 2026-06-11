use crate::command::{
    BaselineAddOptions, BaselineCommand, BaselineInitOptions, CandidateCommand,
    CandidateImportOptions, CandidateLintOptions, CandidateListOptions, CandidateNewOptions,
    CandidateWitnessPlanOptions, CheckOptions, Command, ContextQuery, DiffInput, FirstPrOptions,
    Format, OutcomeOptions, ReceiptTemplateOptions, RepoOptions, SavedOutputReceiptOptions,
};
#[cfg(unix)]
use signal_hook::consts::signal::{SIGINT, SIGTERM};
#[cfg(unix)]
use signal_hook::iterator::{Handle as SignalHandle, Signals};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::sync::{Arc, Mutex};
#[cfg(unix)]
use std::thread;
use std::time::{Duration, Instant};
use unsafe_review_core::{
    AnalysisMode, AnalyzeInput, AnalyzeOutput, CardId, CargoCarefulReceiptInput,
    ConcurrencyReceiptInput, DiffSource, DiscoveryOptions, MiriReceiptInput, PolicyMode,
    ProofReceiptInput, Provenance, RepoScanEvent, RepoScanPhase, RepoScanStatus, RepoStopReason,
    SanitizerReceiptInput, Scope, WITNESS_RECEIPT_SCHEMA_VERSION, WitnessReceipt, analyze,
    analyze_with_discovery, analyze_with_discovery_and_repo_events, audit_witness_receipts,
    baseline_add, baseline_init, collect_context_range, compare_outcome_json, discover_repo_files,
    evaluate_policy_report, evaluate_policy_report_from_output, lint_manual_candidate_text,
    load_manual_candidates, manual_candidate_implementer_handoff, new_manual_candidate_skeleton,
    read_manual_candidate, render_badge_jsons, render_comment_plan, render_gate_manifest,
    render_github_summary, render_human, render_json, render_json_with_provenance, render_lsp,
    render_manual_candidate_witness_plan, render_markdown, render_outcome_json,
    render_outcome_markdown, render_policy_report_json, render_policy_report_markdown,
    render_pr_summary, render_receipt_audit_json, render_receipt_audit_markdown,
    render_repair_queue, render_sarif, render_witness_plan, validate_witness_receipts,
};

mod card_lookup;
mod confirm;
mod first_pr;

const NO_CHANGED_GAPS_MESSAGE: &str = "No changed unsafe-review gaps were found.";
const NO_CHANGED_GAPS_LIMITATION: &str =
    "This does not prove the repo safe, UB-free, Miri-clean, or that any unsafe site executed.";
const FIRST_RUN_TRUST_BOUNDARY: &str = "static unsafe contract review only; not memory-safety proof, not UB-free status, not Miri-clean status, and not a site-execution claim unless a matching witness receipt says so.";
type FirstPrRenderer = fn(&AnalyzeOutput) -> String;

const REVIEW_KIT_ARTIFACT: &str = "review-kit.json";
const GATE_MANIFEST_ARTIFACT: &str = "unsafe-review-gate.json";
const RECEIPT_AUDIT_ARTIFACT: &str = "receipt-audit.md";
const POLICY_REPORT_JSON_ARTIFACT: &str = "policy-report.json";
const POLICY_REPORT_MARKDOWN_ARTIFACT: &str = "policy-report.md";
const MANUAL_CANDIDATES_ARTIFACT: &str = "manual-candidates.json";
const MANUAL_REPAIR_QUEUE_ARTIFACT: &str = "manual-repair-queue.json";
const TOKMD_PACKETS_ARTIFACT: &str = "tokmd-packets.json";
const FIRST_PR_RENDERED_ARTIFACTS: [(&str, FirstPrRenderer); 8] = [
    ("cards.json", render_json),
    ("pr-summary.md", render_pr_summary),
    ("github-summary.md", render_github_summary),
    ("cards.sarif", render_sarif),
    ("comment-plan.json", render_comment_plan),
    ("witness-plan.md", render_witness_plan),
    ("lsp.json", render_lsp),
    ("repair-queue.json", render_repair_queue),
];
const FIRST_PR_ARTIFACTS: [&str; 16] = [
    REVIEW_KIT_ARTIFACT,
    GATE_MANIFEST_ARTIFACT,
    "cards.json",
    "pr-summary.md",
    "github-summary.md",
    "cards.sarif",
    "comment-plan.json",
    "witness-plan.md",
    RECEIPT_AUDIT_ARTIFACT,
    POLICY_REPORT_JSON_ARTIFACT,
    POLICY_REPORT_MARKDOWN_ARTIFACT,
    MANUAL_CANDIDATES_ARTIFACT,
    MANUAL_REPAIR_QUEUE_ARTIFACT,
    TOKMD_PACKETS_ARTIFACT,
    "lsp.json",
    "repair-queue.json",
];

pub(crate) fn execute(command: Command) -> Result<(), crate::RunFailure> {
    match command {
        Command::Help => {
            print_help();
            Ok(())
        }
        Command::RepoHelp => {
            print_repo_help();
            Ok(())
        }
        Command::CandidateHelp => {
            print_candidate_help();
            Ok(())
        }
        Command::BaselineHelp => {
            print_baseline_help();
            Ok(())
        }
        Command::Version => {
            println!("unsafe-review {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Command::Support => {
            print_support();
            Ok(())
        }
        Command::Doctor { root } => doctor(&root).map_err(crate::RunFailure::Tool),
        Command::Check(options) => run_check(
            options,
            Scope::Diff,
            AnalysisMode::Draft,
            DiscoveryOptions::default(),
        ),
        Command::Repo(options) => repo(options),
        Command::Pilot(options) => run_check(
            options,
            Scope::Diff,
            AnalysisMode::Draft,
            DiscoveryOptions::default(),
        ),
        Command::FirstPr(options) => first_pr(options).map_err(crate::RunFailure::Tool),
        Command::Badges { root, out } => badges(&root, &out).map_err(crate::RunFailure::Tool),
        Command::Explain { root, id, format } => {
            explain(&root, &id, format).map_err(crate::RunFailure::Tool)
        }
        Command::Context { root, query } => context(&root, query).map_err(crate::RunFailure::Tool),
        Command::Candidate(command) => candidate(command).map_err(crate::RunFailure::Tool),
        Command::Baseline(command) => run_baseline(command).map_err(crate::RunFailure::Tool),
        Command::Confirm(options) => confirm::run(options).map_err(crate::RunFailure::Tool),
        Command::ReceiptTemplate(options) => {
            receipt_template(options).map_err(crate::RunFailure::Tool)
        }
        Command::ReceiptValidate { root } => {
            receipt_validate(&root).map_err(crate::RunFailure::Tool)
        }
        Command::ReceiptAudit(options) => receipt_audit(options).map_err(crate::RunFailure::Tool),
        Command::ReceiptImportMiri(options) => {
            receipt_import_miri(options).map_err(crate::RunFailure::Tool)
        }
        Command::ReceiptImportCareful(options) => {
            receipt_import_careful(options).map_err(crate::RunFailure::Tool)
        }
        Command::ReceiptImportSanitizer(options) => {
            receipt_import_sanitizer(options).map_err(crate::RunFailure::Tool)
        }
        Command::ReceiptImportConcurrency(options) => {
            receipt_import_concurrency(options).map_err(crate::RunFailure::Tool)
        }
        Command::ReceiptImportProof(options) => {
            receipt_import_proof(options).map_err(crate::RunFailure::Tool)
        }
        Command::Outcome(options) => outcome(options).map_err(crate::RunFailure::Tool),
        Command::PolicyReport(options) => policy_report(options).map_err(crate::RunFailure::Tool),
        Command::Lsp => crate::lsp::serve().map_err(crate::RunFailure::Tool),
    }
}

fn print_support() {
    println!("unsafe-review support");
    println!();
    println!("Current posture:");
    println!("- ReviewCards: experimental; selected slices are fixture-backed or dogfood-backed.");
    println!(
        "- first-pr bundle: advisory; projects cards, summaries, SARIF, comment plans, witness plans, saved LSP JSON, and repair queues from ReviewCards, with manual candidates indexed separately."
    );
    println!(
        "- receipts: saved-output template/import/audit only; receipts attach external evidence to exact ReviewCard or manual candidate identities."
    );
    println!("- outcome comparison: saved snapshot comparison only.");
    println!("- policy report: advisory no-new-debt simulation only.");
    println!("- comment posting: not default.");
    println!("- source edits: not supported.");
    println!("- witness execution: not default.");
    println!("- blocking policy: not default.");
    println!("- live LSP: deferred; saved lsp.json is the current editor-adjacent artifact.");
    println!();
    println!("Trust boundary:");
    println!("- static unsafe contract review only.");
    println!("- not memory-safety proof.");
    println!("- not UB-free status.");
    println!("- not Miri-clean status.");
    println!("- not a site-execution claim unless a matching witness receipt says so.");
    println!();
    println!("Docs:");
    println!("- docs/status/SUPPORT_SUMMARY.md");
    println!("- docs/status/SUPPORT_TIERS.md");
}

fn run_check(
    options: CheckOptions,
    scope: Scope,
    mode: AnalysisMode,
    discovery: DiscoveryOptions,
) -> Result<(), crate::RunFailure> {
    let provenance = build_provenance(&options);
    let diff = diff_source(&options).map_err(crate::RunFailure::Tool)?;
    let policy = options.policy.clone();
    let output = analyze_with_discovery(
        AnalyzeInput {
            root: options.root,
            scope,
            diff,
            mode,
            policy,
            include_unchanged_tests: true,
            max_cards: options.max_cards,
        },
        discovery,
    )
    .map_err(crate::RunFailure::Tool)?;
    let rendered = render_with_format_and_provenance(&output, &options.format, Some(&provenance));
    if let Some(path) = options.out {
        ensure_parent_dir(&path).map_err(crate::RunFailure::Tool)?;
        fs::write(&path, rendered).map_err(|err| {
            crate::RunFailure::Tool(format!("write {} failed: {err}", path.display()))
        })?;
    } else {
        println!("{rendered}");
    }
    enforce_policy(&output)?;
    Ok(())
}

fn repo(options: RepoOptions) -> Result<(), crate::RunFailure> {
    if options.list_files {
        return repo_list_files(options).map_err(crate::RunFailure::Tool);
    }
    run_repo_check(options)
}

fn run_repo_check(options: RepoOptions) -> Result<(), crate::RunFailure> {
    let check = options.check;
    let provenance = build_provenance(&check);
    let diff = diff_source(&check).map_err(crate::RunFailure::Tool)?;
    let policy = check.policy.clone();
    let report_path = check.out.clone();
    let partial_path = report_path.as_deref().map(repo_partial_path);
    let status_path = report_path.as_deref().map(repo_status_path);
    let scan_scope = RepoScanScopeMetadata::new(&check.root, &options.discovery);
    let mut reporter = RepoStatusReporter::new(
        status_path,
        partial_path.clone(),
        options.progress,
        check.format.clone(),
        options.timeout_seconds,
        scan_scope,
    )
    .map_err(crate::RunFailure::Tool)?;
    if let Some(path) = reporter.status_path.as_deref() {
        write_scan_start_stub(path, &reporter.scan_scope).map_err(crate::RunFailure::Tool)?;
    }
    maybe_pause_for_repo_interrupt_test();
    maybe_exit_for_repo_stub_test();
    let output = match analyze_with_discovery_and_repo_events(
        AnalyzeInput {
            root: check.root,
            scope: Scope::Repo,
            diff,
            mode: AnalysisMode::Repo,
            policy,
            include_unchanged_tests: true,
            max_cards: check.max_cards,
        },
        options.discovery,
        |event| reporter.record_event(event),
    ) {
        Ok(output) => output,
        Err(err) => {
            return Err(crate::RunFailure::Tool(repo_incomplete_error(
                &mut reporter,
                &err,
                partial_path.as_deref(),
            )));
        }
    };
    if reporter.files_discovered() == 0 {
        eprintln!(
            "unsafe-review repo: no Rust files selected after include/exclude/ignores; check --root, --include, --exclude, --[no-]large-repo-ignores, and --[no-]respect-gitignore"
        );
    }
    let rendered = render_with_format_and_provenance(&output, &check.format, Some(&provenance));
    if let Some(path) = report_path {
        let partial = repo_partial_path(&path);
        if let Err(err) = write_repo_report(&path, &partial, rendered) {
            return Err(crate::RunFailure::Tool(repo_incomplete_error(
                &mut reporter,
                &err,
                Some(&partial),
            )));
        }
    } else {
        println!("{rendered}");
    }
    enforce_policy(&output)?;
    Ok(())
}

fn repo_list_files(options: RepoOptions) -> Result<(), String> {
    let root = options.check.root.clone();
    let scan_scope = RepoScanScopeMetadata::new(&root, &options.discovery);
    let files = discover_repo_files(root.clone(), options.discovery)?;
    let rendered = render_repo_file_list(&root, &files, &options.check.format, &scan_scope)?;
    if let Some(path) = options.check.out {
        ensure_parent_dir(&path)?;
        fs::write(&path, rendered)
            .map_err(|err| format!("write {} failed: {err}", path.display()))?;
    } else {
        print!("{rendered}");
    }
    Ok(())
}

fn render_repo_file_list(
    root: &Path,
    files: &[PathBuf],
    format: &Format,
    scan_scope: &RepoScanScopeMetadata,
) -> Result<String, String> {
    match format {
        Format::Human => Ok(render_repo_file_list_human(root, files)),
        Format::Json => render_repo_file_list_json(root, files, scan_scope),
        Format::Markdown => Ok(render_repo_file_list_markdown(root, files, scan_scope)),
        _ => Err("repo --list-files only supports human, json, or markdown output".to_string()),
    }
}

fn render_repo_file_list_human(root: &Path, files: &[PathBuf]) -> String {
    let mut rendered = format!(
        "unsafe-review repo file list\nroot: {}\nfiles: {}\n",
        root.display(),
        files.len()
    );
    for file in files {
        rendered.push_str(&repo_path_display(file));
        rendered.push('\n');
    }
    rendered
}

fn render_repo_file_list_json(
    root: &Path,
    files: &[PathBuf],
    scan_scope: &RepoScanScopeMetadata,
) -> Result<String, String> {
    let file_paths = files
        .iter()
        .map(|file| repo_path_display(file))
        .collect::<Vec<_>>();
    let value = serde_json::json!({
        "schema_version": "repo-file-list/v1",
        "tool": "unsafe-review",
        "tool_version": env!("CARGO_PKG_VERSION"),
        "mode": "repo_list_files",
        "root": root.display().to_string(),
        "scan_scope": repo_scan_scope_json(scan_scope),
        "summary": {
            "selected_rust_files": file_paths.len(),
            "analysis_run": false,
            "reviewcards_created": 0,
            "witnesses_run": false,
        },
        "files": file_paths,
        "trust_boundary": repo_file_list_trust_boundary(),
    });
    let mut rendered = serde_json::to_string_pretty(&value)
        .map_err(|err| format!("render repo file list JSON failed: {err}"))?;
    rendered.push('\n');
    Ok(rendered)
}

fn render_repo_file_list_markdown(
    root: &Path,
    files: &[PathBuf],
    scan_scope: &RepoScanScopeMetadata,
) -> String {
    let mut out = String::new();
    out.push_str("# unsafe-review repo file list\n\n");
    out.push_str("Selected Rust files from the repo discovery pipeline.\n\n");
    out.push_str("## Summary\n\n");
    out.push_str(&format!("- Root: `{}`\n", root.display()));
    out.push_str(&format!("- Selected Rust files: `{}`\n", files.len()));
    out.push_str("- Analysis run: `false`\n");
    out.push_str("- ReviewCards created: `0`\n");
    out.push_str("- Witnesses run: `false`\n\n");
    out.push_str("## Scan Scope\n\n");
    out.push_str(&format!(
        "- Include: `{}`\n",
        repo_scope_patterns_display(&scan_scope.include)
    ));
    out.push_str(&format!(
        "- Exclude: `{}`\n",
        repo_scope_patterns_display(&scan_scope.exclude)
    ));
    out.push_str(&format!(
        "- Respect gitignore: `{}`\n",
        scan_scope.respect_gitignore
    ));
    out.push_str(&format!(
        "- Large-repo ignores: `{}`\n",
        scan_scope.large_repo_ignores
    ));
    out.push_str(&format!(
        "- Max files: `{}`\n\n",
        scan_scope
            .max_files
            .map(|max_files| max_files.to_string())
            .unwrap_or_else(|| "none".to_string())
    ));
    out.push_str("## Files\n\n");
    if files.is_empty() {
        out.push_str("No Rust files selected.\n\n");
    } else {
        for file in files {
            out.push_str(&format!("- `{}`\n", repo_path_display(file)));
        }
        out.push('\n');
    }
    out.push_str("## Trust Boundary\n\n");
    out.push_str(repo_file_list_trust_boundary());
    out.push('\n');
    out
}

fn repo_scope_patterns_display(patterns: &[String]) -> String {
    if patterns.is_empty() {
        "all".to_string()
    } else {
        patterns.join(", ")
    }
}

fn repo_file_list_trust_boundary() -> &'static str {
    "File selection dry run only; this does not analyze files, create ReviewCards, execute witnesses, prove site reach, prove repository safety, or make UB-free/Miri-clean claims."
}

fn repo_path_display(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

struct RepoStatusReporter {
    status_path: Option<PathBuf>,
    partial_path: Option<PathBuf>,
    progress: bool,
    timeout_seconds: Option<u64>,
    timeout: Option<Duration>,
    started: Instant,
    last_status: Arc<Mutex<Option<RepoScanStatus>>>,
    partial_output: Arc<Mutex<Option<AnalyzeOutput>>>,
    format: Format,
    scan_scope: RepoScanScopeMetadata,
    last_phase: Option<String>,
    last_discovery_heartbeat: usize,
    last_scan_heartbeat: usize,
    /// Set true the moment `record_event` detects a `--timeout-seconds` timeout
    /// and returns the timeout error.  This is the single source of truth that
    /// distinguishes a timeout incomplete-scan from an analysis/write error on
    /// the shared `record_incomplete` path.
    hit_timeout: bool,
    _signal_guard: Option<RepoSignalGuard>,
}

#[derive(Clone, Debug)]
struct RepoScanScopeMetadata {
    root: PathBuf,
    include: Vec<String>,
    exclude: Vec<String>,
    respect_gitignore: bool,
    large_repo_ignores: bool,
    max_files: Option<usize>,
}

impl RepoScanScopeMetadata {
    fn new(root: &Path, discovery: &DiscoveryOptions) -> Self {
        Self {
            root: root.to_path_buf(),
            include: discovery.include.clone(),
            exclude: discovery.exclude.clone(),
            respect_gitignore: discovery.respect_gitignore,
            large_repo_ignores: discovery.large_repo_ignores,
            max_files: discovery.max_files,
        }
    }
}

impl RepoStatusReporter {
    fn new(
        status_path: Option<PathBuf>,
        partial_path: Option<PathBuf>,
        progress: bool,
        format: Format,
        timeout_seconds: Option<u64>,
        scan_scope: RepoScanScopeMetadata,
    ) -> Result<Self, String> {
        let last_status = Arc::new(Mutex::new(None));
        let partial_output = Arc::new(Mutex::new(None));
        let signal_guard = install_repo_signal_guard(
            status_path.clone(),
            partial_path.clone(),
            last_status.clone(),
            partial_output.clone(),
            format.clone(),
            scan_scope.clone(),
        )?;
        Ok(Self {
            status_path,
            partial_path,
            progress,
            timeout_seconds,
            timeout: timeout_seconds.map(Duration::from_secs),
            started: Instant::now(),
            last_status,
            partial_output,
            format,
            scan_scope,
            last_phase: None,
            last_discovery_heartbeat: 0,
            last_scan_heartbeat: 0,
            hit_timeout: false,
            _signal_guard: signal_guard,
        })
    }

    fn record_event(&mut self, event: &RepoScanEvent) -> Result<(), String> {
        let mut status = event.status.clone();
        self.refresh_elapsed(&mut status);
        *self
            .last_status
            .lock()
            .map_err(|err| format!("repo status lock poisoned: {err}"))? = Some(status.clone());
        if let Some(output) = &event.partial_output {
            *self
                .partial_output
                .lock()
                .map_err(|err| format!("repo partial output lock poisoned: {err}"))? =
                Some(output.clone());
        }
        maybe_pause_for_repo_interrupt_after_scan(&status);
        self.refresh_elapsed(&mut status);
        *self
            .last_status
            .lock()
            .map_err(|err| format!("repo status lock poisoned: {err}"))? = Some(status.clone());
        if let Some(path) = &self.status_path {
            ensure_parent_dir(path)?;
            fs::write(path, render_repo_scan_status(&status, &self.scan_scope)?)
                .map_err(|err| format!("write {} failed: {err}", path.display()))?;
        }
        if self.progress && self.should_print(&status) {
            eprintln!("{}", format_repo_progress(&status));
        }
        if self.timed_out(&status) {
            self.hit_timeout = true;
            return Err(self.timeout_error());
        }
        Ok(())
    }

    fn record_incomplete(
        &mut self,
        error: &str,
        partial_path: Option<&Path>,
    ) -> Result<Option<PathBuf>, String> {
        let Some(path) = self.status_path.clone() else {
            return Ok(None);
        };
        ensure_parent_dir(&path)?;
        let last_status = self
            .last_status
            .lock()
            .map_err(|err| format!("repo status lock poisoned: {err}"))?
            .clone();
        // A timeout is only ever surfaced when `record_event` detects it and
        // returns the timeout error (setting `hit_timeout`).  Every other
        // incomplete stop on this shared path — analysis error mid-scan or a
        // report-write failure — is an `Error`, not a timeout.
        let stop_reason = if self.hit_timeout {
            RepoStopReason::Timeout
        } else {
            RepoStopReason::Error
        };
        fs::write(
            &path,
            render_repo_scan_incomplete_status(
                last_status.as_ref(),
                error,
                partial_path.filter(|path| path.exists()),
                &self.scan_scope,
                stop_reason,
            )?,
        )
        .map_err(|err| format!("write {} failed: {err}", path.display()))?;
        Ok(Some(path))
    }

    fn should_print(&mut self, status: &RepoScanStatus) -> bool {
        let phase = status.phase.as_str();
        let phase_changed = self.last_phase.as_deref() != Some(phase);
        if phase_changed {
            self.last_phase = Some(phase.to_string());
            return true;
        }
        if status.completed {
            return true;
        }
        if status.files_scanned >= self.last_scan_heartbeat + 100 {
            self.last_scan_heartbeat = status.files_scanned;
            return true;
        }
        if status.files_discovered >= self.last_discovery_heartbeat + 100 {
            self.last_discovery_heartbeat = status.files_discovered;
            return true;
        }
        false
    }

    fn write_partial_report(&self) -> Result<Option<PathBuf>, String> {
        let Some(path) = self.partial_path.as_ref() else {
            return Ok(None);
        };
        let partial_output = self
            .partial_output
            .lock()
            .map_err(|err| format!("repo partial output lock poisoned: {err}"))?
            .clone();
        let Some(output) = partial_output else {
            return Ok(None);
        };
        ensure_parent_dir(path)?;
        fs::write(path, render_with_format(&output, &self.format))
            .map_err(|err| format!("write partial repo report {} failed: {err}", path.display()))?;
        Ok(Some(path.clone()))
    }

    fn refresh_elapsed(&self, status: &mut RepoScanStatus) {
        status.elapsed_ms = status.elapsed_ms.max(self.elapsed_ms());
    }

    fn elapsed_ms(&self) -> u64 {
        self.started
            .elapsed()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX)
    }

    fn timed_out(&self, status: &RepoScanStatus) -> bool {
        !status.completed
            && self
                .timeout
                .is_some_and(|timeout| self.started.elapsed() >= timeout)
    }

    fn timeout_error(&self) -> String {
        let seconds = self.timeout_seconds.unwrap_or_default();
        format!(
            "repo scan timed out after {seconds}s; use --include/--exclude/--max-files or a scoped --root to reduce scan scope"
        )
    }

    fn files_discovered(&self) -> usize {
        self.last_status
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().map(|s| s.files_discovered))
            .unwrap_or(0)
    }
}

#[cfg(unix)]
struct RepoSignalState {
    status_path: Option<PathBuf>,
    partial_path: Option<PathBuf>,
    last_status: Arc<Mutex<Option<RepoScanStatus>>>,
    partial_output: Arc<Mutex<Option<AnalyzeOutput>>>,
    format: Format,
    scan_scope: RepoScanScopeMetadata,
}

#[cfg(unix)]
struct RepoInterruptedArtifacts {
    status_path: PathBuf,
    partial_path: Option<PathBuf>,
}

#[cfg(unix)]
impl RepoSignalState {
    fn record_interrupted(
        &self,
        signal_name: &str,
    ) -> Result<Option<RepoInterruptedArtifacts>, String> {
        let Some(path) = self.status_path.as_ref() else {
            return Ok(None);
        };
        let last_status = self
            .last_status
            .lock()
            .map_err(|err| format!("repo status lock poisoned: {err}"))?
            .clone();
        let partial_path = self.write_interrupted_partial()?;
        ensure_parent_dir(path)?;
        fs::write(
            path,
            render_repo_scan_interrupted_status(
                last_status.as_ref(),
                signal_name,
                partial_path.as_deref(),
                &self.scan_scope,
            )?,
        )
        .map_err(|err| format!("write {} failed: {err}", path.display()))?;
        Ok(Some(RepoInterruptedArtifacts {
            status_path: path.clone(),
            partial_path,
        }))
    }

    fn write_interrupted_partial(&self) -> Result<Option<PathBuf>, String> {
        let Some(path) = self.partial_path.as_ref() else {
            return Ok(None);
        };
        let partial_output = self
            .partial_output
            .lock()
            .map_err(|err| format!("repo partial output lock poisoned: {err}"))?
            .clone();
        let Some(output) = partial_output else {
            return Ok(None);
        };
        ensure_parent_dir(path)?;
        fs::write(path, render_with_format(&output, &self.format))
            .map_err(|err| format!("write partial repo report {} failed: {err}", path.display()))?;
        Ok(Some(path.clone()))
    }
}

#[cfg(unix)]
struct RepoSignalGuard {
    handle: SignalHandle,
    thread: Option<thread::JoinHandle<()>>,
}

#[cfg(not(unix))]
struct RepoSignalGuard;

#[cfg(unix)]
impl Drop for RepoSignalGuard {
    fn drop(&mut self) {
        self.handle.close();
        if let Some(thread) = self.thread.take() {
            let _joined = thread.join();
        }
    }
}

fn install_repo_signal_guard(
    status_path: Option<PathBuf>,
    partial_path: Option<PathBuf>,
    last_status: Arc<Mutex<Option<RepoScanStatus>>>,
    partial_output: Arc<Mutex<Option<AnalyzeOutput>>>,
    format: Format,
    scan_scope: RepoScanScopeMetadata,
) -> Result<Option<RepoSignalGuard>, String> {
    install_repo_signal_guard_impl(
        status_path,
        partial_path,
        last_status,
        partial_output,
        format,
        scan_scope,
    )
}

#[cfg(unix)]
fn install_repo_signal_guard_impl(
    status_path: Option<PathBuf>,
    partial_path: Option<PathBuf>,
    last_status: Arc<Mutex<Option<RepoScanStatus>>>,
    partial_output: Arc<Mutex<Option<AnalyzeOutput>>>,
    format: Format,
    scan_scope: RepoScanScopeMetadata,
) -> Result<Option<RepoSignalGuard>, String> {
    let state = RepoSignalState {
        status_path,
        partial_path,
        last_status,
        partial_output,
        format,
        scan_scope,
    };
    let mut signals = Signals::new([SIGTERM, SIGINT])
        .map_err(|err| format!("install repo signal handler failed: {err}"))?;
    let handle = signals.handle();
    let thread = thread::spawn(move || {
        if let Some(signal) = signals.forever().next() {
            let signal_name = repo_signal_name(signal);
            match state.record_interrupted(signal_name) {
                Ok(Some(artifacts)) => {
                    eprintln!(
                        "unsafe-review repo: interrupted by {signal_name}; incomplete repo status written to {}",
                        artifacts.status_path.display()
                    );
                    if let Some(partial_path) = artifacts.partial_path {
                        eprintln!(
                            "unsafe-review repo: partial repo report kept at {}",
                            partial_path.display()
                        );
                    }
                }
                Ok(None) => eprintln!(
                    "unsafe-review repo: interrupted by {signal_name}; rerun with --out to keep <out>.status.json"
                ),
                Err(err) => eprintln!(
                    "unsafe-review repo: interrupted by {signal_name}; failed to write incomplete repo status: {err}"
                ),
            }
            std::process::exit(128 + signal);
        }
    });
    Ok(Some(RepoSignalGuard {
        handle,
        thread: Some(thread),
    }))
}

#[cfg(not(unix))]
fn install_repo_signal_guard_impl(
    _status_path: Option<PathBuf>,
    _partial_path: Option<PathBuf>,
    _last_status: Arc<Mutex<Option<RepoScanStatus>>>,
    _partial_output: Arc<Mutex<Option<AnalyzeOutput>>>,
    _format: Format,
    _scan_scope: RepoScanScopeMetadata,
) -> Result<Option<RepoSignalGuard>, String> {
    Ok(None)
}

#[cfg(unix)]
fn repo_signal_name(signal: i32) -> &'static str {
    match signal {
        SIGTERM => "SIGTERM",
        SIGINT => "SIGINT",
        _ => "signal",
    }
}

fn render_repo_scan_status(
    status: &RepoScanStatus,
    scan_scope: &RepoScanScopeMetadata,
) -> Result<String, String> {
    let (operator_state, partial, stop_reason) = if status.partial {
        // max-cards cap: successful but bounded — not failure, not signal.
        ("capped", true, status.stop_reason.as_str())
    } else {
        ("complete", false, status.stop_reason.as_str())
    };
    let value = serde_json::json!({
        "schema_version": status.schema_version.as_str(),
        "phase": status.phase.as_str(),
        "scan_scope": repo_scan_scope_json(scan_scope),
        "elapsed_ms": status.elapsed_ms,
        "files_discovered": status.files_discovered,
        "files_scanned": status.files_scanned,
        "files_remaining": files_remaining(status),
        "cards_found": status.cards_found,
        "last_path": status.last_path.as_ref().map(|path| repo_path_display(path)),
        "completed": status.completed,
        "partial": partial,
        "stop_reason": stop_reason,
        "cap": status.cap,
        "error": null,
        "signal": null,
        "partial_path": null,
        "operator": repo_status_operator_json(operator_state, None, status.cap),
    });
    let mut rendered = serde_json::to_string_pretty(&value)
        .map_err(|err| format!("render repo status JSON failed: {err}"))?;
    rendered.push('\n');
    Ok(rendered)
}

fn render_repo_scan_incomplete_status(
    status: Option<&RepoScanStatus>,
    error: &str,
    partial_path: Option<&Path>,
    scan_scope: &RepoScanScopeMetadata,
    stop_reason: RepoStopReason,
) -> Result<String, String> {
    let value = serde_json::json!({
        "schema_version": status
            .map(|status| status.schema_version.as_str())
            .unwrap_or("repo-scan-status/v1"),
        "phase": "failed",
        "scan_scope": repo_scan_scope_json(scan_scope),
        "elapsed_ms": status.map_or(0, |status| status.elapsed_ms),
        "files_discovered": status.map_or(0, |status| status.files_discovered),
        "files_scanned": status.map_or(0, |status| status.files_scanned),
        "files_remaining": status.map_or(0, files_remaining),
        "cards_found": status.map_or(0, |status| status.cards_found),
        "last_path": status
            .and_then(|status| status.last_path.as_ref())
            .map(|path| repo_path_display(path)),
        "completed": false,
        "partial": true,
        "stop_reason": stop_reason.as_str(),
        "cap": null,
        "error": error,
        "signal": null,
        "partial_path": partial_path.map(|path| path.display().to_string()),
        "operator": repo_status_operator_json("failed", partial_path, None),
    });
    let mut rendered = serde_json::to_string_pretty(&value)
        .map_err(|err| format!("render repo status JSON failed: {err}"))?;
    rendered.push('\n');
    Ok(rendered)
}

#[cfg(unix)]
fn render_repo_scan_interrupted_status(
    status: Option<&RepoScanStatus>,
    signal_name: &str,
    partial_path: Option<&Path>,
    scan_scope: &RepoScanScopeMetadata,
) -> Result<String, String> {
    let value = serde_json::json!({
        "schema_version": status
            .map(|status| status.schema_version.as_str())
            .unwrap_or("repo-scan-status/v1"),
        "phase": "terminated",
        "scan_scope": repo_scan_scope_json(scan_scope),
        "elapsed_ms": status.map_or(0, |status| status.elapsed_ms),
        "files_discovered": status.map_or(0, |status| status.files_discovered),
        "files_scanned": status.map_or(0, |status| status.files_scanned),
        "files_remaining": status.map_or(0, files_remaining),
        "cards_found": status.map_or(0, |status| status.cards_found),
        "last_path": status
            .and_then(|status| status.last_path.as_ref())
            .map(|path| repo_path_display(path)),
        "completed": false,
        "partial": true,
        "stop_reason": RepoStopReason::Terminated.as_str(),
        "cap": null,
        "error": format!("repo scan interrupted by {signal_name}"),
        "signal": signal_name,
        "partial_path": partial_path.map(|path| path.display().to_string()),
        "operator": repo_status_operator_json("terminated", partial_path, None),
    });
    let mut rendered = serde_json::to_string_pretty(&value)
        .map_err(|err| format!("render repo status JSON failed: {err}"))?;
    rendered.push('\n');
    Ok(rendered)
}

fn write_scan_start_stub(
    status_path: &Path,
    scan_scope: &RepoScanScopeMetadata,
) -> Result<(), String> {
    ensure_parent_dir(status_path)?;
    let value = serde_json::json!({
        "schema_version": "repo-scan-status/v1",
        "phase": "discovering",
        "scan_scope": repo_scan_scope_json(scan_scope),
        "elapsed_ms": 0u64,
        "files_discovered": 0u64,
        "files_scanned": 0u64,
        "files_remaining": 0u64,
        "cards_found": 0u64,
        "last_path": serde_json::Value::Null,
        "completed": false,
        "partial": false,
        "stop_reason": "none",
        "cap": serde_json::Value::Null,
        "error": serde_json::Value::Null,
        "signal": serde_json::Value::Null,
        "partial_path": serde_json::Value::Null,
        "operator": repo_status_operator_json("in_progress", None, None),
    });
    let mut rendered = serde_json::to_string_pretty(&value)
        .map_err(|err| format!("render repo status stub JSON failed: {err}"))?;
    rendered.push('\n');
    fs::write(status_path, rendered)
        .map_err(|err| format!("write {} failed: {err}", status_path.display()))?;
    Ok(())
}

fn repo_status_operator_json(
    state: &'static str,
    partial_path: Option<&Path>,
    cap: Option<usize>,
) -> serde_json::Value {
    let partial_report_available = partial_path.is_some();
    // Operational routing flag: true when a consumer (ub-review / agent / CI) can
    // safely ingest the scan output without further disambiguation.  A complete scan
    // and a max-cards capped scan both produce a usable report; in-progress, failed,
    // and terminated states do not.  This is routing metadata only — it is not a
    // memory-safety, UB-free, Miri-clean, site-execution, calibrated, or proof claim.
    let downstream_consumable = matches!(state, "complete" | "capped");
    let partial_report_limitation = match (state, partial_report_available) {
        ("complete", _) => {
            "No partial report is retained after a successful scan; use the final report for the recorded scan scope."
        }
        ("capped", _) => "Completed-file snapshot only; not complete repo posture.",
        ("in_progress", _) => {
            "No partial report is retained; the scan has not yet produced output."
        }
        (_, true) => "Completed-file snapshot only; not complete repo posture.",
        _ => {
            "No completed-file partial report was retained; the status sidecar is the durable incomplete-scan artifact."
        }
    };
    let next_action = match (state, partial_report_available) {
        ("complete", _) => {
            "Use the promoted final report for the recorded scan_scope; rerun with adjusted include/exclude filters if the scope is not the intended review lane."
        }
        ("capped", _) => {
            "Inspect the capped report for completed-file findings, then narrow include/exclude filters or raise --max-cards to scan the remaining files; rerun with the same scan_scope to reproduce."
        }
        ("in_progress", _) => {
            "Scan has not yet produced output; if this sidecar persists the process was likely killed before the scan started. Rerun with the same scan_scope to reproduce."
        }
        ("failed", true) => {
            "Inspect partial_path for completed-file findings, then rerun repo with the recorded scan_scope after fixing the error, narrowing scope, or increasing timeout."
        }
        ("failed", false) => {
            "Inspect error and scan_scope, then rerun repo after fixing the error, narrowing scope, or increasing timeout."
        }
        ("terminated", true) => {
            "Inspect partial_path for completed-file findings, then rerun repo with the recorded scan_scope after restarting or narrowing the scan."
        }
        ("terminated", false) => {
            "Inspect signal and scan_scope, then rerun repo with --out after restarting or narrowing the scan."
        }
        _ => "Inspect scan_scope and status fields, then rerun repo with the intended scope.",
    };
    let mut obj = serde_json::json!({
        "state": state,
        "downstream_consumable": downstream_consumable,
        "partial_report_available": partial_report_available,
        "partial_report_limitation": partial_report_limitation,
        "next_action": next_action,
        "claim_boundary": "Operational scan status only; partial reports are completed-file snapshots and are not complete repo posture, witness execution, proof, UB-free status, Miri-clean status, site-execution proof, or policy gating.",
    });
    if let Some(cap_value) = cap {
        obj["cap"] = serde_json::json!(cap_value);
    }
    obj
}

fn repo_scan_scope_json(scan_scope: &RepoScanScopeMetadata) -> serde_json::Value {
    serde_json::json!({
        "root": scan_scope.root.display().to_string(),
        "include": &scan_scope.include,
        "exclude": &scan_scope.exclude,
        "respect_gitignore": scan_scope.respect_gitignore,
        "large_repo_ignores": scan_scope.large_repo_ignores,
        "max_files": scan_scope.max_files,
    })
}

fn files_remaining(status: &RepoScanStatus) -> usize {
    status.files_discovered.saturating_sub(status.files_scanned)
}

fn format_repo_progress(status: &RepoScanStatus) -> String {
    let last_path = status
        .last_path
        .as_ref()
        .map(|path| repo_path_display(path))
        .unwrap_or_else(|| "-".to_string());
    format!(
        "unsafe-review repo: phase={} elapsed_ms={} files_discovered={} files_scanned={} files_remaining={} cards_found={} last_path={} completed={}",
        status.phase.as_str(),
        status.elapsed_ms,
        status.files_discovered,
        status.files_scanned,
        files_remaining(status),
        status.cards_found,
        last_path,
        status.completed,
    )
}

fn repo_status_path(out: &Path) -> PathBuf {
    out_with_suffix(out, ".status.json")
}

fn repo_partial_path(out: &Path) -> PathBuf {
    out_with_suffix(out, ".partial")
}

fn out_with_suffix(out: &Path, suffix: &str) -> PathBuf {
    if let Some(file_name) = out.file_name() {
        let mut suffixed_file_name = file_name.to_os_string();
        suffixed_file_name.push(suffix);
        out.with_file_name(suffixed_file_name)
    } else {
        PathBuf::from(format!("{}{}", out.display(), suffix))
    }
}

fn write_repo_report(path: &Path, partial_path: &Path, rendered: String) -> Result<(), String> {
    ensure_parent_dir(partial_path)?;
    fs::write(partial_path, rendered).map_err(|err| {
        format!(
            "write partial repo report {} failed: {err}",
            partial_path.display()
        )
    })?;
    fs::rename(partial_path, path).map_err(|err| {
        format!(
            "rename partial repo report {} to {} failed: {err}",
            partial_path.display(),
            path.display()
        )
    })?;
    Ok(())
}

fn repo_incomplete_error(
    reporter: &mut RepoStatusReporter,
    error: &str,
    partial_path: Option<&Path>,
) -> String {
    let mut message = error.to_string();
    let mut retained_partial = partial_path
        .filter(|path| path.exists())
        .map(Path::to_path_buf);
    if retained_partial.is_none() {
        match reporter.write_partial_report() {
            Ok(partial) => retained_partial = partial,
            Err(partial_err) => {
                message.push_str(&format!(
                    "; failed to write partial repo report: {partial_err}"
                ));
            }
        }
    }
    match reporter.record_incomplete(error, retained_partial.as_deref()) {
        Ok(Some(status_path)) => {
            message.push_str(&format!(
                "; incomplete repo status written to {}",
                status_path.display()
            ));
        }
        Ok(None) => {}
        Err(status_err) => {
            message.push_str(&format!(
                "; failed to update incomplete repo status: {status_err}"
            ));
        }
    }
    if let Some(partial_path) = retained_partial {
        message.push_str(&format!(
            "; partial repo report kept at {}",
            partial_path.display()
        ));
    }
    message
}

#[cfg(debug_assertions)]
fn maybe_pause_for_repo_interrupt_test() {
    let Ok(raw) = std::env::var("UNSAFE_REVIEW_INTERNAL_REPO_SIGNAL_TEST_PAUSE_MS") else {
        return;
    };
    let Ok(ms) = raw.parse::<u64>() else {
        return;
    };
    std::thread::sleep(Duration::from_millis(ms));
}

#[cfg(not(debug_assertions))]
fn maybe_pause_for_repo_interrupt_test() {}

#[cfg(debug_assertions)]
fn maybe_pause_for_repo_interrupt_after_scan(status: &RepoScanStatus) {
    let Ok(raw_threshold) =
        std::env::var("UNSAFE_REVIEW_INTERNAL_REPO_SIGNAL_TEST_PAUSE_AFTER_SCANNED")
    else {
        return;
    };
    let Ok(threshold) = raw_threshold.parse::<usize>() else {
        return;
    };
    if status.phase != RepoScanPhase::Scanning
        || status.completed
        || status.files_scanned < threshold
    {
        return;
    }
    let ms = std::env::var("UNSAFE_REVIEW_INTERNAL_REPO_SIGNAL_TEST_PAUSE_AFTER_SCAN_MS")
        .or_else(|_| std::env::var("UNSAFE_REVIEW_INTERNAL_REPO_SIGNAL_TEST_PAUSE_MS"))
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
        .unwrap_or(5_000);
    std::thread::sleep(Duration::from_millis(ms));
}

#[cfg(not(debug_assertions))]
fn maybe_pause_for_repo_interrupt_after_scan(_status: &RepoScanStatus) {}

#[cfg(debug_assertions)]
fn maybe_exit_for_repo_stub_test() {
    if std::env::var("UNSAFE_REVIEW_INTERNAL_REPO_STUB_TEST_EXIT").is_ok() {
        std::process::exit(3);
    }
}

#[cfg(not(debug_assertions))]
fn maybe_exit_for_repo_stub_test() {}

fn first_pr(options: FirstPrOptions) -> Result<(), String> {
    let mut check = options.check;
    check.policy = PolicyMode::Advisory;
    let provenance = build_provenance(&check);
    let diff = diff_source(&check)?;
    let root = check.root.clone();
    let output = analyze(AnalyzeInput {
        root: root.clone(),
        scope: Scope::Diff,
        diff: diff.clone(),
        mode: AnalysisMode::Draft,
        policy: PolicyMode::Advisory,
        include_unchanged_tests: true,
        max_cards: check.max_cards,
    })?;
    let receipt_audit = audit_witness_receipts(AnalyzeInput {
        root: root.clone(),
        scope: Scope::Diff,
        diff,
        mode: AnalysisMode::Draft,
        policy: PolicyMode::Advisory,
        include_unchanged_tests: true,
        max_cards: check.max_cards,
    })?;
    let policy_report = evaluate_policy_report_from_output(&output)?;
    let manual_candidates = load_manual_candidates(&root)?;

    fs::create_dir_all(&options.out_dir)
        .map_err(|err| format!("create {} failed: {err}", options.out_dir.display()))?;
    let mut comment_plan_artifact = None;
    for (name, renderer) in FIRST_PR_RENDERED_ARTIFACTS {
        // cards.json uses the provenance-aware renderer to emit schema 0.2.
        let raw_rendered = if name == "cards.json" {
            render_json_with_provenance(&output, &provenance)
        } else {
            renderer(&output)
        };
        let rendered = first_pr::render_first_pr_front_door_artifact(
            name,
            raw_rendered,
            &root,
            &manual_candidates,
        );
        if name == "comment-plan.json" {
            comment_plan_artifact = Some(rendered.clone());
        }
        write_artifact(&options.out_dir.join(name), rendered)?;
    }
    write_artifact(
        &options.out_dir.join(RECEIPT_AUDIT_ARTIFACT),
        render_receipt_audit_markdown(&receipt_audit),
    )?;
    write_artifact(
        &options.out_dir.join(POLICY_REPORT_JSON_ARTIFACT),
        render_policy_report_json(&policy_report),
    )?;
    write_artifact(
        &options.out_dir.join(POLICY_REPORT_MARKDOWN_ARTIFACT),
        render_policy_report_markdown(&policy_report),
    )?;
    write_artifact(
        &options.out_dir.join(MANUAL_CANDIDATES_ARTIFACT),
        first_pr::render_manual_candidates_artifact(&root, &manual_candidates),
    )?;
    write_artifact(
        &options.out_dir.join(MANUAL_REPAIR_QUEUE_ARTIFACT),
        first_pr::render_manual_repair_queue_artifact(&root, &manual_candidates),
    )?;
    write_artifact(
        &options.out_dir.join(TOKMD_PACKETS_ARTIFACT),
        first_pr::render_tokmd_packets_artifact(
            &root,
            &manual_candidates,
            comment_plan_artifact.as_deref(),
        ),
    )?;
    write_artifact(
        &options.out_dir.join(REVIEW_KIT_ARTIFACT),
        first_pr::render_review_kit_manifest(
            &output,
            &root,
            &check,
            &manual_candidates,
            &FIRST_PR_ARTIFACTS,
        ),
    )?;
    write_artifact(
        &options.out_dir.join(GATE_MANIFEST_ARTIFACT),
        render_gate_manifest(&output),
    )?;

    first_pr::print_first_pr_report(first_pr::FirstPrReport {
        output: &output,
        out_dir: &options.out_dir,
        root: &root,
        check: &check,
        manual_candidates: &manual_candidates,
        no_changed_gaps_message: NO_CHANGED_GAPS_MESSAGE,
        no_changed_gaps_limitation: NO_CHANGED_GAPS_LIMITATION,
        artifacts: &FIRST_PR_ARTIFACTS,
    });

    Ok(())
}

fn enforce_policy(output: &unsafe_review_core::AnalyzeOutput) -> Result<(), crate::RunFailure> {
    match output.policy {
        PolicyMode::Advisory => Ok(()),
        PolicyMode::NoNewDebt => {
            // SPEC-0030: fail iff `new` OR `worsened` movement is non-empty.
            // Inherited debt (baseline-known cards) must NOT fail the gate.
            // This replaces the previous zero-debt check that blocked brownfield adoption.
            let new = output.summary.new_gaps;
            let worsened = output.summary.worsened_gaps;
            if new == 0 && worsened == 0 {
                Ok(())
            } else {
                Err(crate::RunFailure::PolicyViolation(format!(
                    "no-new-debt policy: {} new gap(s), {} worsened gap(s)",
                    new, worsened
                )))
            }
        }
        // PolicyMode::Blocking is not a policy violation — the tool did not
        // complete a review because the mode is unimplemented.
        PolicyMode::Blocking => Err(crate::RunFailure::Tool(
            "blocking policy is not implemented".to_string(),
        )),
    }
}

fn write_artifact(path: &Path, rendered: String) -> Result<(), String> {
    ensure_parent_dir(path)?;
    fs::write(path, rendered).map_err(|err| format!("write {} failed: {err}", path.display()))
}

fn diff_source(options: &CheckOptions) -> Result<DiffSource, String> {
    if let Some(diff) = &options.diff {
        return match diff {
            DiffInput::File(path) => Ok(DiffSource::File(resolve_diff_path(&options.root, path))),
            DiffInput::Stdin => read_stdin_diff(),
        };
    }
    if let Some(base) = &options.base {
        let output = ProcessCommand::new("git")
            .arg("diff")
            .arg(format!("{base}...HEAD"))
            .current_dir(&options.root)
            .output()
            .map_err(|err| format!("failed to run git diff: {err}"))?;
        if !output.status.success() {
            return Err(format!(
                "git diff failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        return Ok(DiffSource::Text(
            String::from_utf8_lossy(&output.stdout).into_owned(),
        ));
    }
    Ok(DiffSource::NoneRepoScan)
}

fn read_stdin_diff() -> Result<DiffSource, String> {
    let mut text = String::new();
    io::stdin()
        .read_to_string(&mut text)
        .map_err(|err| format!("read diff from stdin failed: {err}"))?;
    Ok(DiffSource::Text(text))
}

/// Build a [`Provenance`] block from CLI `CheckOptions`.
///
/// This is the seam between the CLI layer (where argv, base ref, and diff path are
/// known) and the core JSON renderer.  Git queries use the same `ProcessCommand`
/// pattern as the existing base-diff computation in `diff_source`.  Resolution
/// failures are silent — "when available" semantics per the brief.
fn build_provenance(options: &CheckOptions) -> Provenance {
    let mut prov = Provenance::new_now();

    // Resolved absolute root.
    prov.root_abs = options
        .root
        .canonicalize()
        .ok()
        .map(|p| p.to_string_lossy().replace('\\', "/"));

    // --base mode: resolve base and head SHAs via `git rev-parse`.
    if let Some(base) = &options.base {
        prov.base_sha = git_rev_parse(&options.root, base);
        prov.head_sha = git_rev_parse(&options.root, "HEAD");
    }

    // --diff <file> mode: record path + SHA-256 of content.
    if let Some(DiffInput::File(diff_path)) = &options.diff {
        let resolved = resolve_diff_path(&options.root, diff_path);
        prov.diff_path = Some(resolved.to_string_lossy().replace('\\', "/"));
        if let Ok(bytes) = fs::read(&resolved) {
            prov.diff_sha256 = Some(unsafe_review_core::sha256_hex_of(&bytes));
        }
    }

    // Dirty-worktree marker when git is available.
    prov.dirty_worktree = git_dirty_worktree(&options.root);

    prov
}

/// Run `git rev-parse <ref>` in `root` and return the trimmed stdout on success.
fn git_rev_parse(root: &Path, reference: &str) -> Option<String> {
    let output = ProcessCommand::new("git")
        .arg("-C")
        .arg(root)
        .arg("rev-parse")
        .arg("--verify")
        .arg(reference)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if sha.is_empty() { None } else { Some(sha) }
}

/// Return `Some(true)` when `git status --porcelain` is non-empty (dirty),
/// `Some(false)` when clean, or `None` when git is unavailable / not a repo.
fn git_dirty_worktree(root: &Path) -> Option<bool> {
    let output = ProcessCommand::new("git")
        .arg("-C")
        .arg(root)
        .arg("status")
        .arg("--porcelain")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(!output.stdout.is_empty())
}

fn resolve_diff_path(root: &Path, path: &Path) -> PathBuf {
    if path.exists() || path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn ensure_parent_dir(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create {} failed: {err}", parent.display()))?;
    }
    Ok(())
}

fn render_with_format(output: &unsafe_review_core::AnalyzeOutput, format: &Format) -> String {
    render_with_format_and_provenance(output, format, None)
}

fn render_with_format_and_provenance(
    output: &unsafe_review_core::AnalyzeOutput,
    format: &Format,
    provenance: Option<&Provenance>,
) -> String {
    match format {
        Format::Human => render_human(output),
        Format::Json => {
            if let Some(prov) = provenance {
                render_json_with_provenance(output, prov)
            } else {
                render_json(output)
            }
        }
        Format::Markdown => render_markdown(output),
        Format::PrSummary => render_pr_summary(output),
        Format::GithubSummary => render_github_summary(output),
        Format::Sarif => render_sarif(output),
        Format::CommentPlan => render_comment_plan(output),
        Format::Lsp => render_lsp(output),
        Format::WitnessPlan => render_witness_plan(output),
    }
}

fn doctor(root: &Path) -> Result<(), String> {
    if !root.is_dir() {
        return Err(format!("root {} is not a directory", root.display()));
    }
    let git_available = tool_available("git");
    let git_repo = git_available && git_root_status(root).is_some();
    let base_ref_available = git_repo && git_ref_available(root, "origin/main");
    let cargo_metadata_available = cargo_metadata_available(root);
    let artifact_dir = root.join("target").join("unsafe-review");
    let artifact_dir_writable = artifact_dir_writable(root);

    println!("unsafe-review doctor");
    println!("workspace root: {}", root.display());
    println!("git command: {}", yes_no(git_available));
    println!("git repository: {}", yes_no(git_repo));
    println!("base ref origin/main: {}", yes_no(base_ref_available));
    println!("cargo metadata: {}", yes_no(cargo_metadata_available));
    println!(
        "artifact dir {}: {}",
        artifact_dir.display(),
        writable_status(artifact_dir_writable)
    );
    println!();
    println!("Witness tool signals");
    println!("miri: {}", yes_no(cargo_subcommand_available("miri")));
    println!(
        "cargo-careful: {}",
        yes_no(cargo_subcommand_available("careful") || tool_available("cargo-careful"))
    );
    println!("sanitizers: configure externally with the appropriate Rust toolchain and RUSTFLAGS");
    println!(
        "loom: {}",
        cargo_manifest_hint(root, "loom")
            .unwrap_or("no Cargo.toml dependency hint detected".to_string())
    );
    println!(
        "shuttle: {}",
        cargo_manifest_hint(root, "shuttle")
            .unwrap_or("no Cargo.toml dependency hint detected".to_string())
    );
    println!("kani: {}", yes_no(tool_available("kani")));
    println!("crux: {}", yes_no(tool_available("crux")));
    println!();
    println!("policy: advisory by default");
    println!("witness execution: not run by doctor or by default");
    println!("trust boundary: {FIRST_RUN_TRUST_BOUNDARY}");
    Ok(())
}

fn tool_available(name: &str) -> bool {
    ProcessCommand::new(name).arg("--version").output().is_ok()
}

fn cargo_subcommand_available(subcommand: &str) -> bool {
    ProcessCommand::new("cargo")
        .arg(subcommand)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn cargo_metadata_available(root: &Path) -> bool {
    ProcessCommand::new("cargo")
        .arg("metadata")
        .arg("--no-deps")
        .arg("--format-version")
        .arg("1")
        .current_dir(root)
        .output()
        .is_ok_and(|output| output.status.success())
}

fn artifact_dir_writable(root: &Path) -> bool {
    let target_dir = root.join("target");
    let artifact_dir = target_dir.join("unsafe-review");
    let target_existed = target_dir.exists();
    let artifact_existed = artifact_dir.exists();
    if fs::create_dir_all(&artifact_dir).is_err() {
        return false;
    }
    let probe = artifact_dir.join(format!(".doctor-write-check-{}", std::process::id()));
    let wrote = fs::write(&probe, b"ok")
        .and_then(|_| fs::remove_file(&probe))
        .is_ok();
    if !artifact_existed {
        let _ = fs::remove_dir(&artifact_dir);
    }
    if !target_existed {
        let _ = fs::remove_dir(&target_dir);
    }
    wrote
}

fn writable_status(writable: bool) -> &'static str {
    if writable { "writable" } else { "not writable" }
}

fn git_root_status(root: &Path) -> Option<String> {
    let output = ProcessCommand::new("git")
        .arg("-C")
        .arg(root)
        .arg("rev-parse")
        .arg("--show-toplevel")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!text.is_empty()).then_some(text)
}

fn git_ref_available(root: &Path, reference: &str) -> bool {
    ProcessCommand::new("git")
        .arg("-C")
        .arg(root)
        .arg("rev-parse")
        .arg("--verify")
        .arg(reference)
        .output()
        .is_ok_and(|output| output.status.success())
}

fn cargo_manifest_hint(root: &Path, name: &str) -> Option<String> {
    let text = fs::read_to_string(root.join("Cargo.toml")).ok()?;
    if text.contains(name) {
        Some("Cargo.toml dependency hint detected".to_string())
    } else {
        None
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn badges(root: &Path, out: &Path) -> Result<(), String> {
    fs::create_dir_all(out).map_err(|err| format!("create {} failed: {err}", out.display()))?;
    let output = analyze(AnalyzeInput {
        root: root.to_path_buf(),
        scope: Scope::Repo,
        diff: DiffSource::NoneRepoScan,
        mode: AnalysisMode::Repo,
        policy: PolicyMode::Advisory,
        include_unchanged_tests: true,
        max_cards: None,
    })?;
    let (main, plus) = render_badge_jsons(&output);
    fs::write(out.join("unsafe-review.json"), main)
        .map_err(|err| format!("write badge failed: {err}"))?;
    fs::write(out.join("unsafe-review-plus.json"), plus)
        .map_err(|err| format!("write badge failed: {err}"))?;
    println!("wrote:");
    println!("  {}", out.join("unsafe-review.json").display());
    println!("  {}", out.join("unsafe-review-plus.json").display());
    println!();
    println!("next:");
    println!("  git add {}", out.display());
    println!("  add Shields endpoint badges for your own OWNER/REPO/BRANCH");
    println!();
    println!("trust boundary:");
    println!(
        "  badge JSON counts unsafe-review gaps; it is not safety, UB-free, or Miri-clean status."
    );
    Ok(())
}

fn explain(root: &Path, id: &str, format: Format) -> Result<(), String> {
    let output = card_lookup::analyze_repo_cards(root)?;
    let id = CardId(id.to_string());
    let detail = match card_lookup::explain_text(&output, &id) {
        Ok(detail) => detail,
        Err(_) => card_lookup::manual_candidate_explain(root, &id.0)?
            .ok_or_else(|| format!("card `{id}` not found"))?,
    };
    match format {
        Format::Json => {
            let packet = match card_lookup::context_packet(&output, &id) {
                Ok(packet) => packet,
                Err(_) => card_lookup::manual_candidate_context(root, &id.0)?
                    .ok_or_else(|| format!("card `{id}` not found"))?,
            };
            println!("{packet}");
        }
        _ => println!("{detail}"),
    }
    Ok(())
}

fn context(root: &Path, query: ContextQuery) -> Result<(), String> {
    match query {
        ContextQuery::CardId(id) => {
            let output = card_lookup::analyze_repo_cards(root)?;
            let card_id = CardId(id.clone());
            let packet = match card_lookup::context_packet(&output, &card_id) {
                Ok(packet) => packet,
                Err(_) => card_lookup::manual_candidate_context(root, &id)?
                    .ok_or_else(|| format!("card `{id}` not found"))?,
            };
            println!("{packet}");
            Ok(())
        }
        ContextQuery::FileRange {
            file,
            line_start,
            line_end,
            changed_only,
        } => {
            let output = card_lookup::analyze_repo_cards(root)?;
            let envelope =
                collect_context_range(&output, root, &file, line_start, line_end, changed_only);
            println!("{envelope}");
            Ok(())
        }
    }
}

fn candidate(command: CandidateCommand) -> Result<(), String> {
    match command {
        CandidateCommand::New(options) => candidate_new(options),
        CandidateCommand::Import(options) => candidate_import(options),
        CandidateCommand::Lint(options) => candidate_lint(options),
        CandidateCommand::List(options) => candidate_list(options),
        CandidateCommand::WitnessPlan(options) => candidate_witness_plan(options),
    }
}

fn candidate_new(options: CandidateNewOptions) -> Result<(), String> {
    let candidate = new_manual_candidate_skeleton(&options.class, &options.id)?;
    let rendered = candidate.to_pretty_json()?;
    if let Some(out) = options.out {
        ensure_parent_dir(&out)?;
        fs::write(&out, rendered).map_err(|err| {
            format!(
                "write manual candidate skeleton {} failed: {err}",
                out.display()
            )
        })?;
        println!("wrote manual candidate skeleton: {}", out.display());
        println!("id: {}", candidate.id);
        println!("stable-byte class: {}", options.class);
        println!("source: manual");
        println!("manual_candidate: true");
        println!(
            "next: replace the TODO placeholders, then run `unsafe-review candidate lint {}` before `candidate import`.",
            out.display()
        );
        println!(
            "boundary: this skeleton is an authoring aid only; it is not analyzer discovery, not witness execution, not proof, and not policy gating."
        );
        println!("trust boundary: {}", candidate.trust_boundary);
    } else {
        print!("{rendered}");
    }
    Ok(())
}

fn candidate_lint(options: CandidateLintOptions) -> Result<(), String> {
    let text = fs::read_to_string(&options.input).map_err(|err| {
        format!(
            "read manual candidate {} failed: {err}",
            options.input.display()
        )
    })?;
    let problems = lint_manual_candidate_text(&text);
    if problems.is_empty() {
        println!("candidate lint: ok");
        println!(
            "checked: manual-candidate/v1 schema, cross-field consistency, and TODO markers; nothing was imported or written."
        );
        println!(
            "boundary: lint is advisory authoring validation only; it is not analyzer discovery, not witness execution, not proof of memory safety, not UB-free status, not Miri-clean status, not a site-execution claim, and not policy gating."
        );
        return Ok(());
    }
    let mut message = format!(
        "candidate lint: {} problem(s) in {}",
        problems.len(),
        options.input.display()
    );
    for problem in &problems {
        message.push_str(&format!("\n- {problem}"));
    }
    message.push_str(
        "\nlint reports the first schema or cross-field error plus all TODO markers; nothing was imported or written.",
    );
    Err(message)
}

fn candidate_import(options: CandidateImportOptions) -> Result<(), String> {
    let candidate = read_manual_candidate(&options.input)?;
    let rendered = candidate.to_pretty_json()?;
    if let Some(out) = options.out {
        ensure_parent_dir(&out)?;
        fs::write(&out, rendered)
            .map_err(|err| format!("write manual candidate {} failed: {err}", out.display()))?;
        println!("wrote manual candidate: {}", out.display());
        println!("id: {}", candidate.id);
        println!("source: manual");
        println!("manual_candidate: true");
        println!("trust boundary: {}", candidate.trust_boundary);
    } else {
        print!("{rendered}");
    }
    Ok(())
}

fn candidate_list(options: CandidateListOptions) -> Result<(), String> {
    let candidates = load_manual_candidates(&options.root)?;
    let rendered = match options.format {
        Format::Json => render_candidate_list_json(&options.root, &candidates)?,
        Format::Markdown => render_candidate_list_markdown(&options.root, &candidates),
        _ => return Err("candidate list only supports json or markdown output".to_string()),
    };
    if let Some(out) = options.out {
        write_artifact(&out, rendered)?;
    } else {
        print!("{rendered}");
    }
    Ok(())
}

fn render_candidate_list_json(
    root: &Path,
    candidates: &[unsafe_review_core::ManualCandidate],
) -> Result<String, String> {
    let evidence_refs = candidates
        .iter()
        .map(|candidate| candidate.evidence.len())
        .sum::<usize>();
    let operation_families = first_pr::manual_candidate_operation_family_counts(candidates);
    let evidence_kinds = first_pr::manual_candidate_evidence_kind_counts(candidates);
    let value = serde_json::json!({
        "schema_version": "manual-candidates/v1",
        "tool": "unsafe-review",
        "tool_version": env!("CARGO_PKG_VERSION"),
        "mode": "manual_candidate_index",
        "source": "candidate_list",
        "root": root.display().to_string(),
        "summary": {
            "manual_candidates": candidates.len(),
            "external_evidence_refs": evidence_refs,
            "operation_families": operation_families,
            "evidence_kinds": evidence_kinds,
            "analyzer_discovered": 0,
        },
        "candidates": candidates
            .iter()
            .map(|candidate| manual_candidate_list_entry(root, candidate))
            .collect::<Vec<_>>(),
        "reviewcard_artifact_relationship": manual_candidate_reviewcard_relationship(),
        "reviewcard_artifact_applicability": manual_candidate_reviewcard_applicability(),
        "trust_boundary": manual_candidate_list_trust_boundary(),
    });
    let mut rendered = serde_json::to_string_pretty(&value)
        .map_err(|err| format!("render manual candidate list JSON failed: {err}"))?;
    rendered.push('\n');
    Ok(rendered)
}

fn manual_candidate_list_entry(
    root: &Path,
    candidate: &unsafe_review_core::ManualCandidate,
) -> serde_json::Value {
    let mut value = serde_json::to_value(candidate).unwrap_or_else(|_| serde_json::json!({}));
    if let Some(object) = value.as_object_mut() {
        object.insert("source".to_string(), serde_json::json!("manual"));
        object.insert("manual_candidate".to_string(), serde_json::json!(true));
        object.insert("analyzer_discovered".to_string(), serde_json::json!(false));
        object.insert(
            "location_text".to_string(),
            serde_json::json!(manual_candidate_location_text(candidate)),
        );
        object.insert(
            "explain_command".to_string(),
            serde_json::json!(candidate_explain_command(root, &candidate.id)),
        );
        object.insert(
            "context_json_command".to_string(),
            serde_json::json!(candidate_context_command(root, &candidate.id)),
        );
        object.insert(
            "witness_plan_command".to_string(),
            serde_json::json!(candidate_witness_plan_command(root, &candidate.id)),
        );
        object.insert(
            "implementer_handoff".to_string(),
            manual_candidate_implementer_handoff(candidate),
        );
    }
    value
}

fn render_candidate_list_markdown(
    root: &Path,
    candidates: &[unsafe_review_core::ManualCandidate],
) -> String {
    let evidence_refs = candidates
        .iter()
        .map(|candidate| candidate.evidence.len())
        .sum::<usize>();
    let operation_families = first_pr::manual_candidate_operation_family_counts(candidates);
    let evidence_kinds = first_pr::manual_candidate_evidence_kind_counts(candidates);
    let mut out = String::new();
    out.push_str("# unsafe-review manual candidate list\n\n");
    out.push_str("This is a manual/advisory candidate ledger. It lists imported `.unsafe-review/candidates/*.json` artifacts and does not make them analyzer-discovered ReviewCards.\n\n");
    out.push_str("## Summary\n\n");
    out.push_str(&format!("- Root: `{}`\n", root.display()));
    out.push_str(&format!("- Manual candidates: `{}`\n", candidates.len()));
    out.push_str(&format!("- External evidence refs: `{evidence_refs}`\n"));
    out.push_str(&format!(
        "- Operation families: `{}`\n",
        first_pr::render_count_map(&operation_families)
    ));
    out.push_str(&format!(
        "- Evidence kinds: `{}`\n",
        first_pr::render_count_map(&evidence_kinds)
    ));
    out.push_str("- Analyzer-discovered: `0`\n\n");
    if candidates.is_empty() {
        out.push_str("No imported manual candidates found.\n\n");
    } else {
        out.push_str("## Candidates\n\n");
        for candidate in candidates {
            out.push_str(&format!("### `{}`\n\n", candidate.id));
            out.push_str(&format!("- Title: {}\n", candidate.title));
            out.push_str(&format!(
                "- Location: `{}`\n",
                manual_candidate_location_text(candidate)
            ));
            out.push_str(&format!(
                "- Operation family: `{}`\n",
                candidate.operation_family
            ));
            out.push_str("- Source: `manual`\n");
            out.push_str("- Manual candidate: `true`\n");
            out.push_str("- Analyzer-discovered: `false`\n");
            out.push_str(&format!(
                "- Evidence refs: `{}`\n",
                candidate.evidence.len()
            ));
            out.push_str("#### Implementer Handoff\n\n");
            append_manual_candidate_list_handoff_markdown(&mut out, root, candidate);
        }
    }
    out.push_str("## ReviewCard Artifact Relationship\n\n");
    out.push_str("- `cards.json`: ReviewCard-only analyzer output; manual candidates are listed only by manual-candidate ledger surfaces.\n");
    out.push_str("- `comment-plan.json`: ReviewCard-only comment planning; manual candidates are not selected for automatic comment plans.\n");
    out.push_str("- `repair-queue.json`: ReviewCard-only repair queue; manual candidates are not automatic repair tasks.\n");
    out.push_str("- `policy-report`: ReviewCard-only policy simulation; manual candidates are not policy gating inputs.\n\n");
    out.push_str("## Trust Boundary\n\n");
    out.push_str(manual_candidate_list_trust_boundary());
    out.push('\n');
    out
}

fn append_manual_candidate_list_handoff_markdown(
    out: &mut String,
    root: &Path,
    candidate: &unsafe_review_core::ManualCandidate,
) {
    let handoff = manual_candidate_implementer_handoff(candidate);
    out.push_str(&format!(
        "- Inspect: `{}`\n",
        manual_candidate_location_text(candidate)
    ));
    out.push_str(&format!(
        "- Route: `{}` -> `{}`\n",
        candidate.safe_caller, candidate.unsafe_operation
    ));
    out.push_str(&format!("- Invariant at risk: {}\n", candidate.invariant));
    if let Some(evidence) = handoff
        .get("external_evidence")
        .and_then(serde_json::Value::as_array)
    {
        if evidence.is_empty() {
            out.push_str("- Evidence packet: no external evidence refs yet.\n");
        } else {
            out.push_str(&format!(
                "- Evidence packet: `{}` external reference(s)\n",
                evidence.len()
            ));
            for item in evidence {
                let kind = json_string_field(item, "kind").unwrap_or("other");
                out.push_str(&format!("  - `{kind}`"));
                if let Some(path) = json_string_field(item, "path") {
                    out.push_str(&format!(" at `{path}`"));
                }
                if let Some(summary) = json_string_field(item, "summary") {
                    out.push_str(&format!(": {summary}"));
                }
                out.push('\n');
                if let Some(command) = json_string_field(item, "command") {
                    out.push_str(&format!("    - Command: `{command}`\n"));
                }
                if let Some(limitation) = json_string_field(item, "limitation") {
                    out.push_str(&format!("    - Limitation: {limitation}\n"));
                }
            }
        }
    }
    append_json_string_list_markdown(&handoff, "fix_options", "Fix options", out);
    append_json_string_list_markdown(&handoff, "test_targets", "Test targets", out);
    append_json_string_list_markdown(&handoff, "do_not_touch", "Do not touch", out);
    append_json_string_list_markdown(&handoff, "suggested_next_steps", "Next steps", out);
    append_json_string_list_markdown(&handoff, "non_goals", "Non-goals", out);
    let stop_condition = json_string_field(&handoff, "stop_condition")
        .unwrap_or("stop before source edits if the route no longer matches this manual candidate");
    out.push_str(&format!("- Stop line: {stop_condition}.\n"));
    out.push_str(&format!(
        "- Explain: `{}`\n",
        candidate_explain_command(root, &candidate.id)
    ));
    out.push_str(&format!(
        "- Context: `{}`\n",
        candidate_context_command(root, &candidate.id)
    ));
    out.push_str(&format!(
        "- Witness plan: `{}`\n\n",
        candidate_witness_plan_command(root, &candidate.id)
    ));
}

fn append_json_string_list_markdown(
    value: &serde_json::Value,
    field: &str,
    label: &str,
    out: &mut String,
) {
    let Some(items) = value.get(field).and_then(serde_json::Value::as_array) else {
        return;
    };
    if items.is_empty() {
        return;
    }
    out.push_str(&format!("- {label}:\n"));
    for item in items {
        if let Some(text) = item.as_str().filter(|text| !text.trim().is_empty()) {
            out.push_str(&format!("  - {text}\n"));
        }
    }
}

fn json_string_field<'a>(value: &'a serde_json::Value, field: &str) -> Option<&'a str> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .filter(|text| !text.trim().is_empty())
}

fn manual_candidate_reviewcard_relationship() -> serde_json::Value {
    serde_json::json!({
        "cards.json": "ReviewCard-only analyzer output; manual candidates are listed only by manual-candidate ledger surfaces.",
        "cards.sarif": "ReviewCard-only analyzer output; manual candidates are not emitted as SARIF analyzer results.",
        "comment-plan.json": "ReviewCard-only comment planning; manual candidates are not selected for automatic comment plans.",
        "lsp.json": "ReviewCard-only saved editor projection; manual candidates are not emitted as analyzer diagnostics.",
        "repair-queue.json": "ReviewCard-only repair queue; manual candidates are not automatic repair tasks.",
        "receipt-audit.md": "Receipts may match manual candidate IDs as manual/advisory targets without importing them as ReviewCard witness evidence.",
        "policy-report.json": "ReviewCard-only policy simulation; manual candidates are not policy gating inputs.",
        "policy-report.md": "ReviewCard-only policy simulation; manual candidates are not policy gating inputs."
    })
}

fn manual_candidate_reviewcard_applicability() -> serde_json::Value {
    serde_json::json!({
        "cards.json": manual_candidate_reviewcard_applicability_entry(
            "reviewcard_only",
            "Manual candidates stay in manual-candidate ledger surfaces and are not emitted as analyzer ReviewCards."
        ),
        "cards.sarif": manual_candidate_reviewcard_applicability_entry(
            "reviewcard_only",
            "Manual candidates are not emitted as SARIF analyzer results."
        ),
        "comment-plan.json": manual_candidate_reviewcard_applicability_entry(
            "reviewcard_only",
            "Manual candidates are not selected for automatic comment plans."
        ),
        "lsp.json": manual_candidate_reviewcard_applicability_entry(
            "reviewcard_only",
            "Manual candidates are not emitted as saved editor diagnostics."
        ),
        "repair-queue.json": manual_candidate_reviewcard_applicability_entry(
            "reviewcard_only",
            "Manual candidates are not automatic repair tasks."
        ),
        "policy-report.json": manual_candidate_reviewcard_applicability_entry(
            "reviewcard_only",
            "Manual candidates are not policy gating inputs for the JSON policy report."
        ),
        "policy-report.md": manual_candidate_reviewcard_applicability_entry(
            "reviewcard_only",
            "Manual candidates are not policy gating inputs for the Markdown policy report."
        )
    })
}

fn manual_candidate_reviewcard_applicability_entry(
    decision: &str,
    reason: &str,
) -> serde_json::Value {
    serde_json::json!({
        "decision": decision,
        "applies_to_manual_candidates": false,
        "manual_candidate_markers_allowed": false,
        "reason": reason,
    })
}

fn manual_candidate_list_trust_boundary() -> &'static str {
    "Manual/advisory static unsafe contract review candidate ledger only; candidates are not analyzer-discovered ReviewCards, not a proof of UB, not a proof of memory safety, not UB-free status, not Miri-clean status, not a site-execution claim unless a matching witness receipt says so, not repository safety, and not policy gating. unsafe-review did not run witnesses, post comments, edit source, run an agent, or enforce blocking policy."
}

fn manual_candidate_location_text(candidate: &unsafe_review_core::ManualCandidate) -> String {
    format!(
        "{}:{}",
        candidate.location.file.display(),
        candidate.location.line
    )
}

fn candidate_explain_command(root: &Path, candidate_id: &str) -> String {
    format!(
        "unsafe-review explain --root {} {}",
        shell_arg(&root.display().to_string()),
        shell_arg(candidate_id)
    )
}

fn candidate_context_command(root: &Path, candidate_id: &str) -> String {
    format!(
        "unsafe-review context --root {} {} --json",
        shell_arg(&root.display().to_string()),
        shell_arg(candidate_id)
    )
}

fn candidate_witness_plan_command(root: &Path, candidate_id: &str) -> String {
    format!(
        "unsafe-review candidate witness-plan --root {} {}",
        shell_arg(&root.display().to_string()),
        shell_arg(candidate_id)
    )
}

fn shell_arg(value: &str) -> String {
    if value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-'))
    {
        value.to_string()
    } else {
        format!("\"{}\"", value.replace('"', "\\\""))
    }
}

fn candidate_witness_plan(options: CandidateWitnessPlanOptions) -> Result<(), String> {
    let candidate = unsafe_review_core::load_manual_candidate(&options.root, &options.id)?
        .ok_or_else(|| format!("manual candidate `{}` not found", options.id))?;
    let rendered = render_manual_candidate_witness_plan(&candidate);
    if let Some(out) = options.out {
        ensure_parent_dir(&out)?;
        fs::write(&out, rendered).map_err(|err| {
            format!(
                "write manual candidate witness plan {} failed: {err}",
                out.display()
            )
        })?;
    } else {
        print!("{rendered}");
    }
    Ok(())
}

fn receipt_template(options: ReceiptTemplateOptions) -> Result<(), String> {
    let command_hash = options.command.as_deref().map(WitnessReceipt::command_hash);
    let receipt = WitnessReceipt {
        schema_version: WITNESS_RECEIPT_SCHEMA_VERSION.to_string(),
        card_id: options.card_id,
        tool: options.tool,
        strength: options.strength,
        author: Some(options.author),
        recorded_at: Some(options.recorded_at),
        expires_at: Some(options.expires_at),
        summary: options.summary,
        command: options.command,
        command_hash,
        limitations: if options.limitations.is_empty() {
            None
        } else {
            Some(options.limitations)
        },
        // Templates never claim a run happened, so no verdict is emitted;
        // authors may add one after an actual run.
        verdict: None,
    };
    receipt.validate()?;
    let rendered = receipt.to_pretty_json()?;
    if let Some(path) = options.out {
        ensure_parent_dir(&path)?;
        fs::write(&path, rendered)
            .map_err(|err| format!("write {} failed: {err}", path.display()))?;
    } else {
        print!("{rendered}");
    }
    Ok(())
}

fn receipt_validate(root: &Path) -> Result<(), String> {
    let count = validate_witness_receipts(root.to_path_buf())?;
    println!("witness receipts: {count} valid");
    Ok(())
}

fn receipt_audit(options: CheckOptions) -> Result<(), String> {
    let scope = if options.base.is_some() || options.diff.is_some() {
        Scope::Diff
    } else {
        Scope::Repo
    };
    let mode = match &scope {
        Scope::Diff => AnalysisMode::Draft,
        Scope::Repo => AnalysisMode::Repo,
    };
    let diff = diff_source(&options)?;
    let report = audit_witness_receipts(AnalyzeInput {
        root: options.root,
        scope,
        diff,
        mode,
        policy: PolicyMode::Advisory,
        include_unchanged_tests: true,
        max_cards: options.max_cards,
    })?;
    let rendered = match options.format {
        Format::Json => render_receipt_audit_json(&report),
        Format::Markdown => render_receipt_audit_markdown(&report),
        _ => return Err("receipt audit only supports json or markdown output".to_string()),
    };
    if let Some(path) = options.out {
        ensure_parent_dir(&path)?;
        fs::write(&path, rendered)
            .map_err(|err| format!("write {} failed: {err}", path.display()))?;
    } else {
        println!("{rendered}");
    }
    Ok(())
}

fn outcome(options: OutcomeOptions) -> Result<(), String> {
    let before = fs::read_to_string(&options.before)
        .map_err(|err| format!("read {} failed: {err}", options.before.display()))?;
    let after = fs::read_to_string(&options.after)
        .map_err(|err| format!("read {} failed: {err}", options.after.display()))?;
    let report = compare_outcome_json(&before, &after)?;
    let rendered = match options.format {
        Format::Json => render_outcome_json(&report),
        Format::Markdown => render_outcome_markdown(&report),
        _ => return Err("outcome only supports json or markdown output".to_string()),
    };
    if let Some(path) = options.out {
        ensure_parent_dir(&path)?;
        fs::write(&path, rendered)
            .map_err(|err| format!("write {} failed: {err}", path.display()))?;
    } else {
        println!("{rendered}");
    }
    Ok(())
}

fn policy_report(options: CheckOptions) -> Result<(), String> {
    let scope = if options.base.is_some() || options.diff.is_some() {
        Scope::Diff
    } else {
        Scope::Repo
    };
    let mode = match &scope {
        Scope::Diff => AnalysisMode::Draft,
        Scope::Repo => AnalysisMode::Repo,
    };
    let diff = diff_source(&options)?;
    let report = evaluate_policy_report(AnalyzeInput {
        root: options.root,
        scope,
        diff,
        mode,
        policy: PolicyMode::Advisory,
        include_unchanged_tests: true,
        max_cards: options.max_cards,
    })?;
    let rendered = match options.format {
        Format::Json => render_policy_report_json(&report),
        Format::Markdown => render_policy_report_markdown(&report),
        _ => return Err("policy report only supports json or markdown output".to_string()),
    };
    if let Some(path) = options.out {
        ensure_parent_dir(&path)?;
        fs::write(&path, rendered)
            .map_err(|err| format!("write {} failed: {err}", path.display()))?;
    } else {
        println!("{rendered}");
    }
    Ok(())
}

fn receipt_import_miri(options: SavedOutputReceiptOptions) -> Result<(), String> {
    let output = fs::read_to_string(&options.log)
        .map_err(|err| format!("read {} failed: {err}", options.log.display()))?;
    let receipt = WitnessReceipt::from_miri_output(MiriReceiptInput {
        card_id: options.card_id,
        output,
        author: options.author,
        recorded_at: options.recorded_at,
        expires_at: options.expires_at,
        command: options.command,
        limitations: options.limitations,
    })?;
    let rendered = receipt.to_pretty_json()?;
    if let Some(path) = options.out {
        ensure_parent_dir(&path)?;
        fs::write(&path, rendered)
            .map_err(|err| format!("write {} failed: {err}", path.display()))?;
    } else {
        print!("{rendered}");
    }
    Ok(())
}

fn receipt_import_careful(options: SavedOutputReceiptOptions) -> Result<(), String> {
    let output = fs::read_to_string(&options.log)
        .map_err(|err| format!("read {} failed: {err}", options.log.display()))?;
    let receipt = WitnessReceipt::from_cargo_careful_output(CargoCarefulReceiptInput {
        card_id: options.card_id,
        output,
        author: options.author,
        recorded_at: options.recorded_at,
        expires_at: options.expires_at,
        command: options.command,
        limitations: options.limitations,
    })?;
    let rendered = receipt.to_pretty_json()?;
    if let Some(path) = options.out {
        ensure_parent_dir(&path)?;
        fs::write(&path, rendered)
            .map_err(|err| format!("write {} failed: {err}", path.display()))?;
    } else {
        print!("{rendered}");
    }
    Ok(())
}

fn receipt_import_sanitizer(options: SavedOutputReceiptOptions) -> Result<(), String> {
    let output = fs::read_to_string(&options.log)
        .map_err(|err| format!("read {} failed: {err}", options.log.display()))?;
    let receipt = WitnessReceipt::from_sanitizer_output(SanitizerReceiptInput {
        card_id: options.card_id,
        tool: options
            .tool
            .ok_or_else(|| "missing value for --tool".to_string())?,
        output,
        author: options.author,
        recorded_at: options.recorded_at,
        expires_at: options.expires_at,
        command: options.command,
        limitations: options.limitations,
        allow_runtime: options.allow_runtime,
    })?;
    let rendered = receipt.to_pretty_json()?;
    if let Some(path) = options.out {
        ensure_parent_dir(&path)?;
        fs::write(&path, rendered)
            .map_err(|err| format!("write {} failed: {err}", path.display()))?;
    } else {
        print!("{rendered}");
    }
    Ok(())
}

fn receipt_import_concurrency(options: SavedOutputReceiptOptions) -> Result<(), String> {
    let output = fs::read_to_string(&options.log)
        .map_err(|err| format!("read {} failed: {err}", options.log.display()))?;
    let receipt = WitnessReceipt::from_concurrency_output(ConcurrencyReceiptInput {
        card_id: options.card_id,
        tool: options
            .tool
            .ok_or_else(|| "missing value for --tool".to_string())?,
        output,
        author: options.author,
        recorded_at: options.recorded_at,
        expires_at: options.expires_at,
        command: options.command,
        limitations: options.limitations,
    })?;
    let rendered = receipt.to_pretty_json()?;
    if let Some(path) = options.out {
        ensure_parent_dir(&path)?;
        fs::write(&path, rendered)
            .map_err(|err| format!("write {} failed: {err}", path.display()))?;
    } else {
        print!("{rendered}");
    }
    Ok(())
}

fn receipt_import_proof(options: SavedOutputReceiptOptions) -> Result<(), String> {
    let output = fs::read_to_string(&options.log)
        .map_err(|err| format!("read {} failed: {err}", options.log.display()))?;
    let receipt = WitnessReceipt::from_proof_output(ProofReceiptInput {
        card_id: options.card_id,
        tool: options
            .tool
            .ok_or_else(|| "missing value for --tool".to_string())?,
        output,
        author: options.author,
        recorded_at: options.recorded_at,
        expires_at: options.expires_at,
        command: options.command,
        limitations: options.limitations,
    })?;
    let rendered = receipt.to_pretty_json()?;
    if let Some(path) = options.out {
        ensure_parent_dir(&path)?;
        fs::write(&path, rendered)
            .map_err(|err| format!("write {} failed: {err}", path.display()))?;
    } else {
        print!("{rendered}");
    }
    Ok(())
}

fn run_baseline(command: BaselineCommand) -> Result<(), String> {
    match command {
        BaselineCommand::Init(options) => run_baseline_init(options),
        BaselineCommand::Add(options) => run_baseline_add(options),
        BaselineCommand::Help => {
            print_baseline_help();
            Ok(())
        }
    }
}

fn run_baseline_init(options: BaselineInitOptions) -> Result<(), String> {
    let result = baseline_init(
        &options.root,
        options.out.as_deref(),
        options.review_after.as_deref(),
    )?;
    println!("baseline init: ok");
    println!("captured: {} open actionable card(s)", result.captured);
    println!("ledger: {}", result.ledger_path.display());
    println!("snapshot: {}", result.snapshot_path.display());
    if result.ledger_existed {
        println!(
            "note: ledger already existed; merged existing entries (new entries added, unchanged entries kept)."
        );
    } else {
        println!("note: new ledger created.");
    }
    println!();
    println!("next:");
    println!(
        "  git add {} {}",
        result.ledger_path.display(),
        result.snapshot_path.display()
    );
    println!("  git commit -m 'baseline: record pre-existing debt floor'");
    println!("  # from now on:");
    println!(
        "  unsafe-review check --policy no-new-debt   # fails only when the diff adds or worsens debt"
    );
    println!();
    println!(
        "trust boundary: baseline entries are debt records, not safety records. A baseline init pass means only that the open actionable gaps were recorded as pre-existing; it does not prove memory safety, UB-free status, Miri-clean status, or that any unsafe site executed safely."
    );
    Ok(())
}

fn run_baseline_add(options: BaselineAddOptions) -> Result<(), String> {
    baseline_add(
        &options.root,
        &options.card_id,
        &options.owner,
        &options.reason,
        &options.evidence,
        options.review_after.as_deref(),
        options.out.as_deref(),
    )?;
    println!("baseline add: ok");
    println!("card: {}", options.card_id);
    println!("owner: {}", options.owner);
    println!(
        "trust boundary: baseline entries are debt records, not safety records. Adding a card to the baseline records that the gap pre-existed; it does not prove memory safety, UB-free status, Miri-clean status, or that the unsafe site executed safely."
    );
    Ok(())
}

fn print_baseline_help() {
    println!("unsafe-review baseline: record pre-existing debt as the coverage floor (SPEC-0030)");
    println!();
    println!("Usage:");
    println!(
        "  unsafe-review baseline init [--root .] [--out policy/unsafe-review-baseline.toml] [--review-after YYYY-MM-DD]"
    );
    println!(
        "  unsafe-review baseline add --card-id <UR-...-cN> --owner <name> --reason <text> --evidence <text> [--root .] [--review-after YYYY-MM-DD] [--out policy/unsafe-review-baseline.toml]"
    );
    println!();
    println!("What baseline does:");
    println!(
        "- `init` scans the repo for open actionable cards and records each as a baseline ledger entry with its current coverage state in the snapshot."
    );
    println!(
        "- `add` adds or updates a single ledger entry and its snapshot state without rescanning the entire ledger."
    );
    println!(
        "- The baseline ledger is `policy/unsafe-review-baseline.toml`; the snapshot is `policy/unsafe-review-baseline-snapshot.toml`."
    );
    println!();
    println!("Brownfield onboarding:");
    println!("  unsafe-review baseline init");
    println!(
        "  git add policy/unsafe-review-baseline.toml policy/unsafe-review-baseline-snapshot.toml"
    );
    println!("  git commit -m 'baseline: record pre-existing debt floor'");
    println!("  # from now on:");
    println!("  unsafe-review check --policy no-new-debt");
    println!();
    println!("Trust boundary:");
    println!(
        "- Baseline entries are debt records, not safety records. A baseline init pass means only that the open actionable gaps were recorded as pre-existing debt."
    );
    println!(
        "- Adding a card to the baseline does not prove memory safety, UB-free status, Miri-clean status, or that any unsafe site executed safely."
    );
    println!(
        "- unsafe-review does not execute witnesses, post comments, edit source, run an agent, or enforce blocking policy by default."
    );
}

fn print_help() {
    println!("unsafe-review: cheap unsafe contract review for Rust");
    println!();
    println!("Commands:");
    println!(
        "  check   [--root .] [--base origin/main | --diff file|-] [--format human|json|markdown|pr-summary|github-summary|sarif|comment-plan|lsp|witness-plan] [--policy advisory|no-new-debt] [--out file]"
    );
    println!(
        "  repo    [--root .] [--include glob] [--exclude glob] [--list-files|--dry-run] [--progress] [--timeout-seconds N] [--respect-gitignore|--no-respect-gitignore] [--large-repo-ignores|--no-large-repo-ignores] [--max-files N] [--format human|json|markdown|pr-summary|github-summary|sarif|comment-plan|lsp|witness-plan] [--policy advisory|no-new-debt] [--out file] [--max-cards N]"
    );
    println!(
        "  first-pr [--root .] [--base origin/main|--diff file|-] [--out-dir target/unsafe-review] [--max-cards N]"
    );
    println!("  review  alias for first-pr");
    println!("  pilot   [--root .] [--base origin/main] [--max-cards 5]");
    println!("  badges  [--root .] [--out badges]");
    println!("  explain [--root .] [--json|--format json] <card-id>");
    println!("  context [--root .] [--json|--format json] <card-id>");
    println!("  context [--root .] --file <path> --lines Y-Z [--changed-only] --json");
    println!("  candidate new --class <stable-byte-class> [--id R4R2-S000-TODO] [--out file]");
    println!(
        "  candidate import <manual-candidate.json> [--out .unsafe-review/candidates/<id>.json]"
    );
    println!("  candidate lint <manual-candidate.json>");
    println!("  candidate list [--root .] [--format json|markdown] [--out file]");
    println!("  candidate witness-plan [--root .] <candidate-id> [--out file]");
    println!(
        "  baseline init [--root .] [--out policy/unsafe-review-baseline.toml] [--review-after YYYY-MM-DD]"
    );
    println!(
        "  baseline add --card-id <UR-...-cN> --owner <name> --reason <text> --evidence <text> [--root .] [--review-after YYYY-MM-DD] [--out policy/unsafe-review-baseline.toml]"
    );
    println!(
        "  confirm <card-id> --dry-run|--allow-heavy [--author <owner>] [--root .] [--base origin/main|--diff file] [--expires-at <date>] [--timeout-seconds 600] [--command <override>] [--out file]  (executes the routed witness command only with --allow-heavy; never default; --dry-run previews without executing)"
    );
    println!("  support");
    println!(
        "  outcome --before <cards.json> --after <cards.json> [--format json|markdown] [--out file]"
    );
    println!(
        "  policy report [--root .] [--base origin/main|--diff file] [--format json|markdown] [--out file] [--max-cards N]"
    );
    println!(
        "  receipt template <card-id> --tool <lane> --strength configured|ran|test_targeted|site_reached|reviewed --author <owner> --recorded-at <utc> --expires-at <date> [--summary text] [--command text] [--limitation text] [--out file]"
    );
    println!(
        "  receipt import-miri <card-id> --log <file> --author <owner> --recorded-at <utc> --expires-at <date> --command <cmd> [--limitation text] [--out file]"
    );
    println!(
        "  receipt import-careful <card-id> --log <file> --author <owner> --recorded-at <utc> --expires-at <date> --command <cmd> [--limitation text] [--out file]"
    );
    println!(
        "  receipt import-sanitizer <card-id> --tool asan|msan|tsan|lsan --log <file> --author <owner> --recorded-at <utc> --expires-at <date> --command <cmd> [--allow-runtime] [--limitation text] [--out file]"
    );
    println!(
        "  receipt import-concurrency <card-id> --tool loom|shuttle --log <file> --author <owner> --recorded-at <utc> --expires-at <date> --command <cmd> [--limitation text] [--out file]"
    );
    println!(
        "  receipt import-proof <card-id> --tool kani|crux --log <file> --author <owner> --recorded-at <utc> --expires-at <date> --command <cmd> [--limitation text] [--out file]"
    );
    println!("  receipt validate [--root .]");
    println!(
        "  receipt audit [--root .] [--base origin/main|--diff file] [--format json|markdown] [--out file] [--max-cards N]"
    );
    println!("  doctor  [--root .]");
    println!();
    println!("Flags may be passed as `--flag value` or `--flag=value`.");
    println!();
    println!("Exit codes:");
    println!("  0  ran to completion: clean, or advisory findings (advisory policy default)");
    println!("  1  ran to completion: no-new-debt policy found new or worsened coverage gaps");
    println!("  2  tool did not complete a review: usage, input/IO, or internal error");
    println!();
    println!("Trust boundary: {FIRST_RUN_TRUST_BOUNDARY}");
}

fn print_repo_help() {
    println!("unsafe-review repo: advisory unsafe contract review for a whole Rust repo");
    println!();
    println!("Usage:");
    println!(
        "  unsafe-review repo [--root .] [--include glob] [--exclude glob] [--list-files|--dry-run] [--progress] [--timeout-seconds N] [--respect-gitignore|--no-respect-gitignore] [--large-repo-ignores|--no-large-repo-ignores] [--max-files N] [--format human|json|markdown|pr-summary|github-summary|sarif|comment-plan|lsp|witness-plan] [--policy advisory|no-new-debt] [--out file] [--max-cards N]"
    );
    println!();
    println!("What repo scans today:");
    println!("- Discovers *.rs files under --root, defaulting to the current directory.");
    println!(
        "- Discovery respects gitignore files by default and skips .git, .github, .unsafe-review*, target, node_modules, vendor, build, dist, and generated directories."
    );
    println!(
        "- Discovery also skips large-repo directories (node_modules, vendor, build, dist, generated) by default; use --no-large-repo-ignores to include them."
    );
    println!(
        "- Repo mode scans the selected Rust files; --base and --diff are accepted by the shared parser but do not make repo a diff-only scan."
    );
    println!("- Use check or first-pr when you want changed-file review from --base or --diff.");
    println!();
    println!("Options:");
    println!("- --root <dir> chooses the repository or subdirectory to scan.");
    println!("- --include <glob> adds a root-relative Rust file include filter.");
    println!("- --exclude <glob> removes root-relative Rust files from the selection.");
    println!(
        "- --respect-gitignore is the default; --no-respect-gitignore includes ignored Rust files."
    );
    println!(
        "- --large-repo-ignores is the default; --no-large-repo-ignores disables the large-repo directory skip set."
    );
    println!(
        "- --list-files prints selected Rust files and exits without analysis; --dry-run is an alias."
    );
    println!("- With --list-files, --format supports human, json, or markdown output.");
    println!("- --progress prints scan-status heartbeats to stderr during analysis.");
    println!(
        "- --timeout-seconds <N> stops analysis after roughly N seconds at repo event boundaries."
    );
    println!("- --max-files <N> truncates the selected file list before analysis.");
    println!(
        "- --format <name> chooses human, json, markdown, pr-summary, github-summary, sarif, comment-plan, lsp, or witness-plan output."
    );
    println!(
        "- --policy advisory is the default; --policy no-new-debt exits 1 for new or worsened coverage gaps."
    );
    println!("- --out <file> writes the rendered report to a file instead of stdout.");
    println!("- --max-cards <N> stops after N cards are collected; it does not limit discovery.");
    println!();
    println!("Large-repo guidance:");
    println!(
        "- Prefer scoped roots or include/exclude filters such as --include 'src/**/*.rs' and --exclude '**/generated/**'."
    );
    println!(
        "- Use --list-files or --dry-run before a large scan to confirm the selected Rust files."
    );
    println!(
        "- Use --timeout-seconds with --out on long scans to keep an incomplete status sidecar and any completed-file partial report."
    );
    println!();
    println!("Output and cancellation:");
    println!(
        "- --out renders to <out>.partial, then renames it to <out> only after a successful render."
    );
    println!(
        "- <out>.status.json records scan scope, phase, elapsed time, discovered/scanned/remaining files, cards found, last path, completion, normal errors, Unix interruption signals, and operator next-step diagnostics."
    );
    println!(
        "- On normal errors, incomplete status is kept; if a rendered partial report exists, it is left at <out>.partial."
    );
    println!(
        "- On Unix SIGTERM/SIGINT, repo records phase=terminated and the signal in <out>.status.json when --out is set; after completed files it also keeps the latest partial report at <out>.partial."
    );
    println!("- Without --out, Unix SIGTERM/SIGINT prints an interruption diagnostic to stderr.");
    println!();
    println!("Trust boundary:");
    println!("- ReviewCards are advisory static findings: {FIRST_RUN_TRUST_BOUNDARY}");
    println!(
        "- unsafe-review does not execute witnesses, post comments, edit source, or enforce blocking policy by default."
    );
}

fn print_candidate_help() {
    println!("unsafe-review candidate: import and project manual advisory candidates");
    println!();
    println!("Usage:");
    println!(
        "  unsafe-review candidate new --class <stable-byte-class> [--id R4R2-S000-TODO] [--out file]"
    );
    println!(
        "  unsafe-review candidate import <manual-candidate.json> [--out .unsafe-review/candidates/<id>.json]"
    );
    println!("  unsafe-review candidate lint <manual-candidate.json>");
    println!("  unsafe-review candidate list [--root .] [--format json|markdown] [--out file]");
    println!("  unsafe-review candidate witness-plan [--root .] <candidate-id> [--out file]");
    println!();
    println!("What manual candidates are:");
    println!(
        "- Manual candidates are externally discovered advisory unsafe-review artifacts supplied by a reviewer."
    );
    println!(
        "- They use schema_version `manual-candidate/v1` and preserve source `manual`, manual_candidate `true`, and analyzer_discovered `false`."
    );
    println!(
        "- They can carry a title, file:line location, operation family, unsafe operation, invariant, safe caller route, evidence references, optional fix/test/do-not-touch guidance, and a trust boundary."
    );
    println!();
    println!("Commands:");
    println!(
        "- new emits a schema-correct manual-candidate skeleton for one stable-byte class with TODO placeholder text in free-text fields; cross-field consistency (class, proof mode, fix boundary, PR aperture) is pre-filled so only authoring content remains."
    );
    println!(
        "- new accepts --class with one of: stable-byte-source-getter-reentry, stable-byte-source-rab-async, stable-byte-source-sab-race, stable-byte-source-helper-dependent, stable-byte-source-pathlike-live-view, stable-byte-source-native-ffi-read."
    );
    println!(
        "- import reads a manual candidate JSON file, validates it, and writes a canonical artifact."
    );
    println!(
        "- lint validates a manual candidate file with the same schema and cross-field checks as import, without importing or writing anything, and also flags remaining TODO placeholder markers; it reports the first schema error plus all TODO markers."
    );
    println!(
        "- lint exits 0 with `candidate lint: ok` when clean and exits 2 listing the problems otherwise."
    );
    println!(
        "- list reports imported manual candidates from .unsafe-review/candidates without adding them to ReviewCard-only outputs."
    );
    println!(
        "- witness-plan renders the candidate's advisory witness-plan projection by candidate ID."
    );
    println!();
    println!("Authoring flow:");
    println!(
        "  unsafe-review candidate new --class stable-byte-source-getter-reentry > draft.json"
    );
    println!("  # edit draft.json and replace every TODO placeholder");
    println!("  unsafe-review candidate lint draft.json");
    println!(
        "  unsafe-review candidate import draft.json --out .unsafe-review/candidates/<id>.json"
    );
    println!();
    println!("After import:");
    println!(
        "- explain and context can load the manual candidate by ID when no analyzer ReviewCard has that ID."
    );
    println!(
        "- first-pr writes a separate manual-candidates.json handoff with optional guidance fields; cards.json, SARIF, comment-plan, LSP, repair-queue, and policy-report artifacts stay ReviewCard-only."
    );
    println!(
        "- receipts may audit against manual candidate IDs as external evidence for that manual target, not as imported ReviewCard witness evidence."
    );
    println!();
    println!("Trust boundary:");
    println!(
        "- Manual candidates are not analyzer-discovered findings, not proof of memory safety, not UB-free status, not Miri-clean status, not a site-execution claim unless a matching witness receipt says so, and not policy gating."
    );
    println!(
        "- candidate new and candidate lint are authoring aids only: manual candidates remain manual/advisory with source `manual`, manual_candidate `true`, and analyzer_discovered `false`; a passing lint is not analyzer discovery, not witness execution, and not proof."
    );
    println!(
        "- unsafe-review does not execute witnesses, post comments, edit source, run an agent, or enforce blocking policy by default."
    );
}

#[cfg(test)]
mod tests {
    use super::{
        RepoScanScopeMetadata, render_repo_scan_incomplete_status, resolve_diff_path,
        writable_status, yes_no,
    };
    use std::path::{Path, PathBuf};
    use unsafe_review_core::{DiscoveryOptions, RepoStopReason};

    fn test_scan_scope() -> RepoScanScopeMetadata {
        RepoScanScopeMetadata::new(Path::new("/tmp/repo"), &DiscoveryOptions::repo_defaults())
    }

    #[test]
    fn incomplete_status_labels_timeout_distinctly_from_error() -> Result<(), String> {
        let scope = test_scan_scope();

        // The shared record_incomplete path serves timeout AND non-timeout
        // (analysis/write) errors. The stop_reason must be accurate for each.
        let timeout_json = render_repo_scan_incomplete_status(
            None,
            "repo scan timed out after 1s",
            None,
            &scope,
            RepoStopReason::Timeout,
        )?;
        let timeout: serde_json::Value = serde_json::from_str(&timeout_json)
            .map_err(|err| format!("parse timeout status failed: {err}"))?;
        assert_eq!(timeout["phase"], "failed");
        assert_eq!(timeout["partial"], true);
        assert_eq!(timeout["stop_reason"], "timeout");
        assert_eq!(
            timeout["operator"]["downstream_consumable"], false,
            "a timed-out scan is not downstream-consumable"
        );

        let error_json = render_repo_scan_incomplete_status(
            None,
            "rename partial repo report failed: is a directory",
            None,
            &scope,
            RepoStopReason::Error,
        )?;
        let error: serde_json::Value = serde_json::from_str(&error_json)
            .map_err(|err| format!("parse error status failed: {err}"))?;
        assert_eq!(error["phase"], "failed");
        assert_eq!(error["partial"], true);
        assert_eq!(
            error["stop_reason"], "error",
            "a non-timeout incomplete scan must not be mislabeled as a timeout"
        );
        assert_eq!(
            error["operator"]["downstream_consumable"], false,
            "a failed scan is not downstream-consumable"
        );

        Ok(())
    }

    #[test]
    fn resolve_diff_path_joins_relative_path_to_root() {
        let root = Path::new("/workspace/project");
        let diff = Path::new("fixtures/example.diff");

        let resolved = resolve_diff_path(root, diff);

        assert_eq!(
            resolved,
            PathBuf::from("/workspace/project/fixtures/example.diff")
        );
    }

    #[test]
    fn resolve_diff_path_preserves_absolute_paths() {
        let root = Path::new("/workspace/project");
        let diff = Path::new("/tmp/patch.diff");

        let resolved = resolve_diff_path(root, diff);

        assert_eq!(resolved, PathBuf::from("/tmp/patch.diff"));
    }

    #[test]
    fn yes_no_and_writable_status_report_expected_labels() {
        assert_eq!(yes_no(true), "yes");
        assert_eq!(yes_no(false), "no");
        assert_eq!(writable_status(true), "writable");
        assert_eq!(writable_status(false), "not writable");
    }
}
