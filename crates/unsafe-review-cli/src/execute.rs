use crate::command::{
    CandidateCommand, CandidateImportOptions, CandidateListOptions, CandidateWitnessPlanOptions,
    CheckOptions, Command, DiffInput, FirstPrOptions, Format, OutcomeOptions,
    ReceiptTemplateOptions, RepoOptions, SavedOutputReceiptOptions,
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
    ProofReceiptInput, RepoScanEvent, RepoScanPhase, RepoScanStatus, SanitizerReceiptInput, Scope,
    WITNESS_RECEIPT_SCHEMA_VERSION, WitnessReceipt, analyze, analyze_with_discovery,
    analyze_with_discovery_and_repo_events, audit_witness_receipts, compare_outcome_json,
    discover_repo_files, evaluate_policy_report, load_manual_candidates, read_manual_candidate,
    render_badge_jsons, render_comment_plan, render_github_summary, render_human, render_json,
    render_lsp, render_manual_candidate_witness_plan, render_markdown, render_outcome_json,
    render_outcome_markdown, render_policy_report_json, render_policy_report_markdown,
    render_pr_summary, render_receipt_audit_json, render_receipt_audit_markdown,
    render_repair_queue, render_sarif, render_witness_plan, validate_witness_receipts,
};

mod card_lookup;
mod first_pr;

const NO_CHANGED_GAPS_MESSAGE: &str = "No changed unsafe-review gaps were found.";
const NO_CHANGED_GAPS_LIMITATION: &str =
    "This does not prove the repo safe, UB-free, Miri-clean, or that any unsafe site executed.";
type FirstPrRenderer = fn(&AnalyzeOutput) -> String;

const REVIEW_KIT_ARTIFACT: &str = "review-kit.json";
const RECEIPT_AUDIT_ARTIFACT: &str = "receipt-audit.md";
const MANUAL_CANDIDATES_ARTIFACT: &str = "manual-candidates.json";
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
const FIRST_PR_ARTIFACTS: [&str; 11] = [
    REVIEW_KIT_ARTIFACT,
    "cards.json",
    "pr-summary.md",
    "github-summary.md",
    "cards.sarif",
    "comment-plan.json",
    "witness-plan.md",
    RECEIPT_AUDIT_ARTIFACT,
    MANUAL_CANDIDATES_ARTIFACT,
    "lsp.json",
    "repair-queue.json",
];

pub(crate) fn execute(command: Command) -> Result<(), String> {
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
        Command::Version => {
            println!("unsafe-review {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Command::Support => {
            print_support();
            Ok(())
        }
        Command::Doctor { root } => doctor(&root),
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
        Command::FirstPr(options) => first_pr(options),
        Command::Badges { root, out } => badges(&root, &out),
        Command::Explain { root, id, format } => explain(&root, &id, format),
        Command::Context { root, id } => context(&root, &id),
        Command::Candidate(command) => candidate(command),
        Command::ReceiptTemplate(options) => receipt_template(options),
        Command::ReceiptValidate { root } => receipt_validate(&root),
        Command::ReceiptAudit(options) => receipt_audit(options),
        Command::ReceiptImportMiri(options) => receipt_import_miri(options),
        Command::ReceiptImportCareful(options) => receipt_import_careful(options),
        Command::ReceiptImportSanitizer(options) => receipt_import_sanitizer(options),
        Command::ReceiptImportConcurrency(options) => receipt_import_concurrency(options),
        Command::ReceiptImportProof(options) => receipt_import_proof(options),
        Command::Outcome(options) => outcome(options),
        Command::PolicyReport(options) => policy_report(options),
        Command::Lsp => crate::lsp::serve(),
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
    println!("- not a site-execution claim unless a matching receipt says so.");
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
) -> Result<(), String> {
    let diff = diff_source(&options)?;
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
    )?;
    let rendered = render_with_format(&output, &options.format);
    if let Some(path) = options.out {
        ensure_parent_dir(&path)?;
        fs::write(&path, rendered)
            .map_err(|err| format!("write {} failed: {err}", path.display()))?;
    } else {
        println!("{rendered}");
    }
    enforce_policy(&output)?;
    Ok(())
}

fn repo(options: RepoOptions) -> Result<(), String> {
    if options.list_files {
        return repo_list_files(options);
    }
    run_repo_check(options)
}

fn run_repo_check(options: RepoOptions) -> Result<(), String> {
    let check = options.check;
    let diff = diff_source(&check)?;
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
    )?;
    maybe_pause_for_repo_interrupt_test();
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
            return Err(repo_incomplete_error(
                &mut reporter,
                &err,
                partial_path.as_deref(),
            ));
        }
    };
    let rendered = render_with_format(&output, &check.format);
    if let Some(path) = report_path {
        let partial = repo_partial_path(&path);
        if let Err(err) = write_repo_report(&path, &partial, rendered) {
            return Err(repo_incomplete_error(&mut reporter, &err, Some(&partial)));
        }
    } else {
        println!("{rendered}");
    }
    enforce_policy(&output)?;
    Ok(())
}

fn repo_list_files(options: RepoOptions) -> Result<(), String> {
    let root = options.check.root.clone();
    let files = discover_repo_files(root.clone(), options.discovery)?;
    let rendered = render_repo_file_list(&root, &files);
    if let Some(path) = options.check.out {
        ensure_parent_dir(&path)?;
        fs::write(&path, rendered)
            .map_err(|err| format!("write {} failed: {err}", path.display()))?;
    } else {
        print!("{rendered}");
    }
    Ok(())
}

fn render_repo_file_list(root: &Path, files: &[PathBuf]) -> String {
    let mut rendered = format!(
        "unsafe-review repo file list\nroot: {}\nfiles: {}\n",
        root.display(),
        files.len()
    );
    for file in files {
        rendered.push_str(&file.display().to_string());
        rendered.push('\n');
    }
    rendered
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
        fs::write(
            &path,
            render_repo_scan_incomplete_status(
                last_status.as_ref(),
                error,
                partial_path.filter(|path| path.exists()),
                &self.scan_scope,
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
    let value = serde_json::json!({
        "schema_version": status.schema_version.as_str(),
        "phase": status.phase.as_str(),
        "scan_scope": repo_scan_scope_json(scan_scope),
        "elapsed_ms": status.elapsed_ms,
        "files_discovered": status.files_discovered,
        "files_scanned": status.files_scanned,
        "files_remaining": files_remaining(status),
        "cards_found": status.cards_found,
        "last_path": status.last_path.as_ref().map(|path| path.display().to_string()),
        "completed": status.completed,
        "error": null,
        "signal": null,
        "partial_path": null,
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
            .map(|path| path.display().to_string()),
        "completed": false,
        "error": error,
        "signal": null,
        "partial_path": partial_path.map(|path| path.display().to_string()),
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
            .map(|path| path.display().to_string()),
        "completed": false,
        "error": format!("repo scan interrupted by {signal_name}"),
        "signal": signal_name,
        "partial_path": partial_path.map(|path| path.display().to_string()),
    });
    let mut rendered = serde_json::to_string_pretty(&value)
        .map_err(|err| format!("render repo status JSON failed: {err}"))?;
    rendered.push('\n');
    Ok(rendered)
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
        .map(|path| path.display().to_string())
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

fn first_pr(options: FirstPrOptions) -> Result<(), String> {
    let mut check = options.check;
    check.policy = PolicyMode::Advisory;
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
    let manual_candidates = load_manual_candidates(&root)?;

    fs::create_dir_all(&options.out_dir)
        .map_err(|err| format!("create {} failed: {err}", options.out_dir.display()))?;
    for (name, renderer) in FIRST_PR_RENDERED_ARTIFACTS {
        write_artifact(&options.out_dir.join(name), renderer(&output))?;
    }
    write_artifact(
        &options.out_dir.join(RECEIPT_AUDIT_ARTIFACT),
        render_receipt_audit_markdown(&receipt_audit),
    )?;
    write_artifact(
        &options.out_dir.join(MANUAL_CANDIDATES_ARTIFACT),
        first_pr::render_manual_candidates_artifact(&manual_candidates),
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

fn enforce_policy(output: &unsafe_review_core::AnalyzeOutput) -> Result<(), String> {
    match output.policy {
        PolicyMode::Advisory => Ok(()),
        PolicyMode::NoNewDebt => {
            if output.summary.open_actionable_gaps == 0 {
                Ok(())
            } else {
                Err(format!(
                    "no-new-debt policy found {} open actionable gap(s)",
                    output.summary.open_actionable_gaps
                ))
            }
        }
        PolicyMode::Blocking => Err("blocking policy is not implemented".to_string()),
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
    match format {
        Format::Human => render_human(output),
        Format::Json => render_json(output),
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
    println!(
        "trust boundary: static unsafe contract review only; not memory-safety proof, not UB-free status, and no witness execution"
    );
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

fn context(root: &Path, id: &str) -> Result<(), String> {
    let output = card_lookup::analyze_repo_cards(root)?;
    let id = CardId(id.to_string());
    let packet = match card_lookup::context_packet(&output, &id) {
        Ok(packet) => packet,
        Err(_) => card_lookup::manual_candidate_context(root, &id.0)?
            .ok_or_else(|| format!("card `{id}` not found"))?,
    };
    println!("{packet}");
    Ok(())
}

fn candidate(command: CandidateCommand) -> Result<(), String> {
    match command {
        CandidateCommand::Import(options) => candidate_import(options),
        CandidateCommand::List(options) => candidate_list(options),
        CandidateCommand::WitnessPlan(options) => candidate_witness_plan(options),
    }
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
            "analyzer_discovered": 0,
        },
        "candidates": candidates
            .iter()
            .map(|candidate| manual_candidate_list_entry(root, candidate))
            .collect::<Vec<_>>(),
        "reviewcard_artifact_relationship": manual_candidate_reviewcard_relationship(),
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
    let mut out = String::new();
    out.push_str("# unsafe-review manual candidate list\n\n");
    out.push_str("This is a manual/advisory candidate ledger. It lists imported `.unsafe-review/candidates/*.json` artifacts and does not make them analyzer-discovered ReviewCards.\n\n");
    out.push_str("## Summary\n\n");
    out.push_str(&format!("- Root: `{}`\n", root.display()));
    out.push_str(&format!("- Manual candidates: `{}`\n", candidates.len()));
    out.push_str(&format!("- External evidence refs: `{evidence_refs}`\n"));
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
            out.push_str(&format!("- Source: `manual`\n"));
            out.push_str("- Manual candidate: `true`\n");
            out.push_str("- Analyzer-discovered: `false`\n");
            out.push_str(&format!(
                "- Evidence refs: `{}`\n",
                candidate.evidence.len()
            ));
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

fn manual_candidate_reviewcard_relationship() -> serde_json::Value {
    serde_json::json!({
        "cards.json": "ReviewCard-only analyzer output; manual candidates are listed only by manual-candidate ledger surfaces.",
        "cards.sarif": "ReviewCard-only analyzer output; manual candidates are not emitted as SARIF analyzer results.",
        "comment-plan.json": "ReviewCard-only comment planning; manual candidates are not selected for automatic comment plans.",
        "lsp.json": "ReviewCard-only saved editor projection; manual candidates are not emitted as analyzer diagnostics.",
        "repair-queue.json": "ReviewCard-only repair queue; manual candidates are not automatic repair tasks.",
        "receipt-audit.md": "Receipts may match manual candidate IDs as manual/advisory targets without importing them as ReviewCard witness evidence.",
        "policy-report": "ReviewCard-only policy simulation; manual candidates are not policy gating inputs."
    })
}

fn manual_candidate_list_trust_boundary() -> &'static str {
    "Manual/advisory static unsafe contract review candidate ledger only; candidates are not analyzer-discovered ReviewCards, not a proof of UB, not a proof of memory safety, not UB-free status, not a Miri result, not Miri-clean status, not site-execution proof, not repository safety, and not policy gating. unsafe-review did not run witnesses, post comments, edit source, run an agent, or enforce blocking policy."
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

fn print_help() {
    println!("unsafe-review: cheap unsafe contract review for Rust");
    println!();
    println!("Commands:");
    println!(
        "  check   [--root .] [--base origin/main | --diff file|-] [--format human|json|markdown|pr-summary|github-summary|sarif|comment-plan|lsp|witness-plan] [--policy advisory|no-new-debt] [--out file]"
    );
    println!(
        "  repo    [--root .] [--include glob] [--exclude glob] [--list-files] [--progress] [--timeout-seconds N] [--respect-gitignore|--no-respect-gitignore] [--max-files N] [--format human|json|markdown|pr-summary|github-summary|sarif|comment-plan|lsp|witness-plan] [--policy advisory|no-new-debt] [--out file] [--max-cards N]"
    );
    println!(
        "  first-pr [--root .] [--base origin/main|--diff file|-] [--out-dir target/unsafe-review] [--max-cards N]"
    );
    println!("  review  alias for first-pr");
    println!("  pilot   [--root .] [--base origin/main] [--max-cards 5]");
    println!("  badges  [--root .] [--out badges]");
    println!("  explain [--root .] [--json|--format json] <card-id>");
    println!("  context [--root .] [--json|--format json] <card-id>");
    println!(
        "  candidate import <manual-candidate.json> [--out .unsafe-review/candidates/<id>.json]"
    );
    println!("  candidate list [--root .] [--format json|markdown] [--out file]");
    println!("  candidate witness-plan [--root .] <candidate-id> [--out file]");
    println!("  support");
    println!(
        "  outcome --before <cards.json> --after <cards.json> [--format json|markdown] [--out file]"
    );
    println!(
        "  policy report [--root .] [--base origin/main|--diff file] [--format json|markdown] [--out file] [--max-cards N]"
    );
    println!(
        "  receipt template <card-id> --tool <lane> --strength <level> --author <owner> --recorded-at <utc> --expires-at <date> [--summary text] [--command text] [--limitation text] [--out file]"
    );
    println!(
        "  receipt import-miri <card-id> --log <file> --author <owner> --recorded-at <utc> --expires-at <date> --command <cmd> [--limitation text] [--out file]"
    );
    println!(
        "  receipt import-careful <card-id> --log <file> --author <owner> --recorded-at <utc> --expires-at <date> --command <cmd> [--limitation text] [--out file]"
    );
    println!(
        "  receipt import-sanitizer <card-id> --tool asan|msan|tsan|lsan --log <file> --author <owner> --recorded-at <utc> --expires-at <date> --command <cmd> [--limitation text] [--out file]"
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
    println!(
        "Trust boundary: static unsafe contract review only; not memory-safety proof, not UB-free status, and not Miri-clean status."
    );
}

fn print_repo_help() {
    println!("unsafe-review repo: advisory unsafe contract review for a whole Rust repo");
    println!();
    println!("Usage:");
    println!(
        "  unsafe-review repo [--root .] [--include glob] [--exclude glob] [--list-files] [--progress] [--timeout-seconds N] [--respect-gitignore|--no-respect-gitignore] [--max-files N] [--format human|json|markdown|pr-summary|github-summary|sarif|comment-plan|lsp|witness-plan] [--policy advisory|no-new-debt] [--out file] [--max-cards N]"
    );
    println!();
    println!("What repo scans today:");
    println!("- Discovers *.rs files under --root, defaulting to the current directory.");
    println!(
        "- Discovery respects gitignore files by default and skips .git, .github, .unsafe-review*, target, node_modules, vendor, build, dist, and generated directories."
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
    println!("- --list-files prints selected Rust files and exits without analysis.");
    println!("- --progress prints scan-status heartbeats to stderr during analysis.");
    println!(
        "- --timeout-seconds <N> stops analysis after roughly N seconds at repo event boundaries."
    );
    println!("- --max-files <N> truncates the selected file list before analysis.");
    println!(
        "- --format <name> chooses human, json, markdown, pr-summary, github-summary, sarif, comment-plan, lsp, or witness-plan output."
    );
    println!(
        "- --policy advisory is the default; --policy no-new-debt exits nonzero for open actionable gaps."
    );
    println!("- --out <file> writes the rendered report to a file instead of stdout.");
    println!("- --max-cards <N> stops after N cards are collected; it does not limit discovery.");
    println!();
    println!("Large-repo guidance:");
    println!(
        "- Prefer scoped roots or include/exclude filters such as --include 'src/**/*.rs' and --exclude '**/generated/**'."
    );
    println!("- Use --list-files before a large scan to confirm the selected Rust files.");
    println!(
        "- Use --timeout-seconds with --out on long scans to keep an incomplete status sidecar and any completed-file partial report."
    );
    println!();
    println!("Output and cancellation:");
    println!(
        "- --out renders to <out>.partial, then renames it to <out> only after a successful render."
    );
    println!(
        "- <out>.status.json records scan scope, phase, elapsed time, discovered/scanned/remaining files, cards found, last path, completion, normal errors, and Unix interruption signals."
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
    println!(
        "- ReviewCards are advisory static findings, not memory-safety proof, not UB-free status, and not Miri-clean status."
    );
    println!(
        "- unsafe-review does not execute witnesses, post comments, edit source, or enforce blocking policy by default."
    );
}

fn print_candidate_help() {
    println!("unsafe-review candidate: import and project manual advisory candidates");
    println!();
    println!("Usage:");
    println!(
        "  unsafe-review candidate import <manual-candidate.json> [--out .unsafe-review/candidates/<id>.json]"
    );
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
        "- They can carry a title, file:line location, operation family, unsafe operation, invariant, safe caller route, evidence references, and a trust boundary."
    );
    println!();
    println!("Commands:");
    println!(
        "- import reads a manual candidate JSON file, validates it, and writes a canonical artifact."
    );
    println!(
        "- list reports imported manual candidates from .unsafe-review/candidates without adding them to ReviewCard-only outputs."
    );
    println!(
        "- witness-plan renders the candidate's advisory witness-plan projection by candidate ID."
    );
    println!();
    println!("After import:");
    println!(
        "- explain and context can load the manual candidate by ID when no analyzer ReviewCard has that ID."
    );
    println!(
        "- first-pr writes a separate manual-candidates.json handoff; cards.json, SARIF, comment-plan, LSP, repair-queue, and policy-report stay ReviewCard-only."
    );
    println!(
        "- receipts may audit against manual candidate IDs as external evidence for that manual target, not as imported ReviewCard witness evidence."
    );
    println!();
    println!("Trust boundary:");
    println!(
        "- Manual candidates are not analyzer-discovered findings, not proof of memory safety, not UB-free status, not Miri-clean status, not site-execution proof, and not policy gating."
    );
    println!(
        "- unsafe-review does not execute witnesses, post comments, edit source, run an agent, or enforce blocking policy by default."
    );
}

#[cfg(test)]
mod tests {
    use super::{resolve_diff_path, writable_status, yes_no};
    use std::path::{Path, PathBuf};

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
