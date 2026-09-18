use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use unsafe_review_core::{
    AnalysisMode, AnalyzeInput, CargoCarefulReceiptInput, ConcurrencyReceiptInput,
    ExecutedReceiptInput, MiriReceiptInput, PolicyMode, ProofReceiptInput, ReviewCard,
    SanitizerReceiptInput, Scope, SubjectBinding, TerminalStatus, WitnessKind, WitnessReceipt,
    WitnessRoute, analyze,
};

use crate::command::{CheckOptions, ConfirmOptions};

use super::{FIRST_RUN_TRUST_BOUNDARY, card_lookup, diff_source, ensure_parent_dir};

const CONFIRM_LIMITATION: &str = "executed via unsafe-review confirm --allow-heavy; single local run, not site-execution proof for other configurations";
const DEFAULT_EXPIRES_DAYS: i64 = 30;
const POLL_INTERVAL_MS: u64 = 50;

/// Identifies whether the command to be executed came from the analyzer's
/// automatically-derived witness route, or was overridden by the user with
/// `--command`.  Printed before execution so a reviewer can see what they are
/// about to trust.
#[derive(Clone, Copy, Debug)]
enum CommandSource {
    /// Command was derived by the analyzer from the card's witness route.
    AnalyzerRoute,
    /// Command was supplied by the user with `--command` (author-controlled).
    CommandOverride,
}

impl CommandSource {
    fn label(self) -> &'static str {
        match self {
            Self::AnalyzerRoute => "analyzer-derived route",
            Self::CommandOverride => "--command override (author-controlled)",
        }
    }
}

pub(super) fn run(options: ConfirmOptions) -> Result<(), String> {
    let (card, scope, tool_version) = resolve_card(&options)?;
    let (kind, routed_command) = select_route(&card.id.0, &card.routes)?;
    let lane = confirm_lane(kind, &card.id.0)?;
    let command_source = if options.command.is_some() {
        CommandSource::CommandOverride
    } else {
        CommandSource::AnalyzerRoute
    };
    let command_text = options.command.clone().unwrap_or(routed_command);
    let invocation = parse_command_line(&command_text)?;
    if options.dry_run {
        print_dry_run(
            &options,
            &card,
            kind,
            lane,
            &command_text,
            command_source,
            &invocation,
        );
        return Ok(());
    }
    reject_diff_execution(&options)?;
    println!("command provenance: {}", command_source.label());
    println!("parsed program: {}", invocation.program);
    println!("parsed argv: {}", invocation.describe_argv());
    if !invocation.env.is_empty() {
        println!("parsed env: {}", invocation.describe_env());
    }
    let execution = execute_with_timeout(
        &invocation.env,
        &invocation.exec_argv(),
        &options.root,
        Duration::from_secs(options.timeout_seconds),
    )?;
    if execution.timed_out {
        let log_note = write_raw_output_log(&options.root, &card.id.0, &execution.output)?;
        return Err(format!(
            "confirm execution timed out after {}s; the child process was killed and no receipt was written{log_note}",
            options.timeout_seconds
        ));
    }
    let recorded_at = current_utc_timestamp()?;
    let expires_at = match &options.expires_at {
        Some(value) => value.clone(),
        None => default_expires_at()?,
    };
    let binding = subject_binding(
        &options.root,
        &card,
        &scope,
        &tool_version,
        &execution.output,
        execution.terminal,
        &invocation,
    );
    let receipt = match build_receipt(
        lane,
        ReceiptFields {
            card_id: card.id.0.clone(),
            output: execution.output.clone(),
            author: options.author.clone(),
            recorded_at,
            expires_at,
            command: command_text.clone(),
            terminal: execution.terminal,
            binding: Some(binding),
        },
    ) {
        Ok(receipt) => receipt,
        Err(err) => {
            let log_note = write_raw_output_log(&options.root, &card.id.0, &execution.output)?;
            return Err(format!(
                "execution completed but the output did not classify as {} evidence ({err}){log_note}; author a receipt manually if appropriate; no receipt was written",
                lane.tool_name()
            ));
        }
    };
    let receipt_path = receipt_output_path(&options, &receipt);
    ensure_parent_dir(&receipt_path)?;
    fs::write(&receipt_path, receipt.to_pretty_json()?)
        .map_err(|err| format!("write {} failed: {err}", receipt_path.display()))?;
    println!("unsafe-review confirm");
    println!("card: {}", card.id.0);
    println!("route: {}", kind.as_str());
    println!("command: {command_text}");
    println!("exit: {}", describe_terminal(execution.terminal));
    if let Some(subject) = &receipt.subject {
        println!("subject: {}", subject.subject_digest);
    }
    println!("tool: {}", receipt.tool);
    println!("strength recorded: {}", receipt.strength);
    println!("receipt: {}", receipt_path.display());
    println!();
    println!(
        "next: re-run check or first-pr to import this receipt; the card upgrades only through the saved receipt."
    );
    println!("trust boundary: {FIRST_RUN_TRUST_BOUNDARY}");
    Ok(())
}

fn print_dry_run(
    options: &ConfirmOptions,
    card: &ReviewCard,
    kind: WitnessKind,
    lane: ConfirmLane,
    command_text: &str,
    command_source: CommandSource,
    invocation: &Invocation,
) {
    println!("unsafe-review confirm (dry run)");
    println!("card: {}", card.id.0);
    println!("operation family: {}", card.operation.family.as_str());
    println!("route: {}", kind.as_str());
    println!("command: {command_text}");
    println!("command provenance: {}", command_source.label());
    println!("parsed program: {}", invocation.program);
    println!("parsed argv: {}", invocation.describe_argv());
    if !invocation.env.is_empty() {
        println!("parsed env: {}", invocation.describe_env());
    }
    println!("working directory: {}", options.root.display());
    println!("timeout: {}s", options.timeout_seconds);
    println!(
        "expected evidence: a `{}` witness receipt classified by the explicit executed-output constructor",
        lane.tool_name(),
    );
    println!();
    println!(
        "limits: dry run only; nothing was executed; unsafe-review never executes witnesses by default."
    );
    println!("trust boundary: {FIRST_RUN_TRUST_BOUNDARY}");
}

/// Best-effort source revision probe. Returns `None` outside a git checkout
/// or when git cannot answer; only the revision string and dirty bit travel
/// into the receipt, never file contents or paths.
fn git_head(root: &Path) -> Option<String> {
    let run = execute_with_timeout(
        &[],
        &[
            "git".to_string(),
            "-C".to_string(),
            root.to_string_lossy().into_owned(),
            "rev-parse".to_string(),
            "HEAD".to_string(),
        ],
        root,
        Duration::from_secs(10),
    )
    .ok()?;
    if run.timed_out || run.terminal.exit_code != Some(0) {
        return None;
    }
    let head = run.output.trim().to_string();
    (!head.is_empty()).then_some(head)
}

fn git_dirty(root: &Path) -> Option<bool> {
    let run = execute_with_timeout(
        &[],
        &[
            "git".to_string(),
            "-C".to_string(),
            root.to_string_lossy().into_owned(),
            "status".to_string(),
            "--porcelain=v1".to_string(),
        ],
        root,
        Duration::from_secs(10),
    )
    .ok()?;
    if run.timed_out || run.terminal.exit_code != Some(0) {
        return None;
    }
    Some(!run.output.trim().is_empty())
}

fn subject_binding(
    root: &Path,
    card: &ReviewCard,
    scope: &str,
    tool_version: &str,
    output: &str,
    terminal: TerminalStatus,
    invocation: &Invocation,
) -> SubjectBinding {
    let owner = card.site.owner.as_deref().unwrap_or("");
    SubjectBinding {
        subject_digest: SubjectBinding::digest_subject(&[
            &card.id.0,
            card.operation.family.as_str(),
            owner,
            &card.site.location.file.to_string_lossy(),
            &card.site.snippet,
        ]),
        invocation_digest: Some(SubjectBinding::digest_invocation(
            &invocation.program,
            &invocation.args,
            &invocation.env,
        )),
        scope: Some(scope.to_string()),
        head_commit: git_head(root),
        repo_dirty: git_dirty(root),
        workdir: Some(".".to_string()),
        output_digest: Some(SubjectBinding::digest_output(output)),
        captured_complete: Some(terminal.captured_complete),
        tool_version: Some(tool_version.to_string()),
    }
}

/// A card resolved from a saved `--diff` patch cannot be executed: the
/// witness would run in `--root`, but nothing establishes that checkout is
/// the diff's tree, so the receipt would bind execution evidence to a
/// subject it never observed. Check out the diff's tree and confirm without
/// `--diff` instead. Dry-run previews never execute and stay allowed.
fn reject_diff_execution(options: &ConfirmOptions) -> Result<(), String> {
    if options.diff.is_some() {
        return Err("cannot execute a witness for a card resolved from a saved --diff patch: the execution tree cannot be shown to be the reviewed tree; check out the diff and confirm without --diff".to_string());
    }
    Ok(())
}

fn resolve_card(options: &ConfirmOptions) -> Result<(ReviewCard, String, String), String> {
    let output = if options.base.is_some() || options.diff.is_some() {
        let check = CheckOptions {
            root: options.root.clone(),
            base: options.base.clone(),
            diff: options.diff.clone(),
            ..CheckOptions::default()
        };
        let diff = diff_source(&check)?;
        analyze(AnalyzeInput {
            root: options.root.clone(),
            scope: Scope::Diff,
            diff,
            mode: AnalysisMode::Draft,
            policy: PolicyMode::Advisory,
            include_unchanged_tests: true,
            max_cards: None,
        })?
    } else {
        card_lookup::analyze_repo_cards(&options.root)?
    };
    if let Some(card) = output
        .cards
        .iter()
        .find(|card| card.id.0 == options.card_id)
    {
        return Ok((
            card.clone(),
            output.analysis_identity.scope.clone(),
            output.analysis_identity.tool_version.clone(),
        ));
    }
    if card_lookup::manual_candidate_explain(&options.root, &options.card_id)?.is_some() {
        return Err(format!(
            "card `{}` is a manual candidate; confirm executes analyzer ReviewCard witness routes only. Follow `unsafe-review candidate witness-plan` and import a receipt manually.",
            options.card_id
        ));
    }
    Err(format!("card `{}` not found", options.card_id))
}

fn select_route(card_id: &str, routes: &[WitnessRoute]) -> Result<(WitnessKind, String), String> {
    routes
        .iter()
        .find_map(|route| {
            route
                .command
                .clone()
                .map(|command| (route.kind, command))
        })
        .ok_or_else(|| {
            format!(
                "card `{card_id}` has no routed witness command to execute; nothing was executed. Follow the human-deep-review route in witness-plan.md and record a receipt manually."
            )
        })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConfirmLane {
    Miri,
    CargoCareful,
    Sanitizer(&'static str),
    Concurrency(&'static str),
    Proof(&'static str),
}

impl ConfirmLane {
    fn tool_name(self) -> &'static str {
        match self {
            Self::Miri => "miri",
            Self::CargoCareful => "cargo-careful",
            Self::Sanitizer(tool) | Self::Concurrency(tool) | Self::Proof(tool) => tool,
        }
    }
}

fn confirm_lane(kind: WitnessKind, card_id: &str) -> Result<ConfirmLane, String> {
    match kind {
        WitnessKind::Miri => Ok(ConfirmLane::Miri),
        WitnessKind::CargoCareful => Ok(ConfirmLane::CargoCareful),
        WitnessKind::AddressSanitizer => Ok(ConfirmLane::Sanitizer("asan")),
        WitnessKind::MemorySanitizer => Ok(ConfirmLane::Sanitizer("msan")),
        WitnessKind::ThreadSanitizer => Ok(ConfirmLane::Sanitizer("tsan")),
        WitnessKind::LeakSanitizer => Ok(ConfirmLane::Sanitizer("lsan")),
        WitnessKind::Loom => Ok(ConfirmLane::Concurrency("loom")),
        WitnessKind::Shuttle => Ok(ConfirmLane::Concurrency("shuttle")),
        WitnessKind::Kani => Ok(ConfirmLane::Proof("kani")),
        WitnessKind::Crux => Ok(ConfirmLane::Proof("crux")),
        WitnessKind::HumanDeepReview => Err(format!(
            "card `{card_id}` routes to `human-deep-review`; confirm cannot execute a human review and nothing was executed. Perform the deep review manually and record a `human-deep-review` receipt with `receipt template`."
        )),
        WitnessKind::Unsupported => Err(format!(
            "card `{card_id}` routes to an unsupported witness kind; nothing was executed. Follow the human-deep-review route in witness-plan.md and record a receipt manually."
        )),
    }
}

struct ReceiptFields {
    card_id: String,
    output: String,
    author: String,
    recorded_at: String,
    expires_at: String,
    command: String,
    terminal: TerminalStatus,
    binding: Option<SubjectBinding>,
}

fn build_receipt(lane: ConfirmLane, fields: ReceiptFields) -> Result<WitnessReceipt, String> {
    let limitations = vec![CONFIRM_LIMITATION.to_string()];
    let input = match lane {
        ConfirmLane::Miri => ExecutedReceiptInput::Miri(MiriReceiptInput {
            card_id: fields.card_id,
            output: fields.output,
            author: fields.author,
            recorded_at: fields.recorded_at,
            expires_at: fields.expires_at,
            command: fields.command,
            limitations,
            terminal_status: Some(fields.terminal),
            subject: fields.binding.clone(),
        }),
        ConfirmLane::CargoCareful => ExecutedReceiptInput::CargoCareful(CargoCarefulReceiptInput {
            card_id: fields.card_id,
            output: fields.output,
            author: fields.author,
            recorded_at: fields.recorded_at,
            expires_at: fields.expires_at,
            command: fields.command,
            limitations,
            terminal_status: Some(fields.terminal),
            subject: fields.binding.clone(),
        }),
        ConfirmLane::Sanitizer(tool) => ExecutedReceiptInput::Sanitizer(SanitizerReceiptInput {
            card_id: fields.card_id,
            tool: tool.to_string(),
            output: fields.output,
            author: fields.author,
            recorded_at: fields.recorded_at,
            expires_at: fields.expires_at,
            command: fields.command,
            limitations,
            terminal_status: Some(fields.terminal),
            subject: fields.binding.clone(),
            allow_runtime: false,
        }),
        ConfirmLane::Concurrency(tool) => {
            ExecutedReceiptInput::Concurrency(ConcurrencyReceiptInput {
                card_id: fields.card_id,
                tool: tool.to_string(),
                output: fields.output,
                author: fields.author,
                recorded_at: fields.recorded_at,
                expires_at: fields.expires_at,
                command: fields.command,
                limitations,
                terminal_status: Some(fields.terminal),
                subject: fields.binding.clone(),
            })
        }
        ConfirmLane::Proof(tool) => ExecutedReceiptInput::Proof(ProofReceiptInput {
            card_id: fields.card_id,
            tool: tool.to_string(),
            output: fields.output,
            author: fields.author,
            recorded_at: fields.recorded_at,
            expires_at: fields.expires_at,
            command: fields.command,
            limitations,
            terminal_status: Some(fields.terminal),
            subject: fields.binding.clone(),
        }),
    };
    WitnessReceipt::from_executed_output(input)
}

type EnvAssignments = Vec<(String, String)>;

/// One parsed witness invocation: program, arguments, environment
/// overrides, and provenance. This single value feeds the dry-run preview,
/// the process spawner, and the receipt identity, so the previewed command
/// is the executed command.
///
/// Grammar (no shell involved): whitespace separates words; single quotes
/// preserve bytes literally; double quotes preserve literally except that
/// backslash escapes only `"` and `\` (so Windows paths survive); a
/// backslash outside quotes escapes the next character; shell
/// metacharacters (`| & ; < > ( ) $` and backtick) are rejected explicitly
/// instead of being reinterpreted.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Invocation {
    program: String,
    args: Vec<String>,
    env: EnvAssignments,
}

impl Invocation {
    fn exec_argv(&self) -> Vec<String> {
        let mut argv = Vec::with_capacity(self.args.len() + 1);
        argv.push(self.program.clone());
        argv.extend(self.args.iter().cloned());
        argv
    }

    fn describe_argv(&self) -> String {
        format!("{:?}", self.exec_argv())
    }

    fn describe_env(&self) -> String {
        // Keys only: values may carry secrets from user-authored witness
        // commands, and this string is printed to the terminal (plus any
        // CI log capturing stdout).
        let keys: Vec<&str> = self.env.iter().map(|(key, _)| key.as_str()).collect();
        format!("{keys:?} (values redacted)")
    }
}

fn parse_command_line(command: &str) -> Result<Invocation, String> {
    let words = split_command_words(command)?;
    let mut env = Vec::new();
    let mut words = words.into_iter();
    let mut program = None;
    for word in words.by_ref() {
        if program.is_none()
            && let Some(assignment) = env_assignment(&word)
        {
            env.push(assignment);
            continue;
        }
        program = Some(word);
        break;
    }
    let Some(program) = program else {
        return Err(
            "witness command has no executable to run after environment assignments".to_string(),
        );
    };
    Ok(Invocation {
        program,
        args: words.collect(),
        env,
    })
}

fn split_command_words(command: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut started = false;
    let mut chars = command.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            ch if ch.is_whitespace() => {
                if started {
                    words.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            '\'' => {
                started = true;
                loop {
                    match chars.next() {
                        None => {
                            return Err(
                                "witness command has an unterminated single quote".to_string()
                            );
                        }
                        Some('\'') => break,
                        Some(next) => current.push(next),
                    }
                }
            }
            '"' => {
                started = true;
                loop {
                    match chars.next() {
                        None => {
                            return Err(
                                "witness command has an unterminated double quote".to_string()
                            );
                        }
                        Some('"') => break,
                        // Inside double quotes only `"` and `\` are escapable,
                        // so Windows paths like `"C:\tools\runner"` survive
                        // unchanged. Every other backslash stays literal.
                        Some('\\') => match chars.peek() {
                            Some('"') | Some('\\') => {
                                current.push(chars.next().unwrap_or('\\'));
                            }
                            _ => current.push('\\'),
                        },
                        Some(next) => current.push(next),
                    }
                }
            }
            '\\' => {
                let Some(escaped) = chars.next() else {
                    return Err("witness command has a trailing backslash".to_string());
                };
                started = true;
                current.push(escaped);
            }
            '|' | '&' | ';' | '<' | '>' | '(' | ')' | '$' | '`' => {
                return Err(format!(
                    "witness command contains unsupported shell syntax `{ch}`; quote it to pass it literally or run the pipeline manually and import a receipt"
                ));
            }
            _ => {
                started = true;
                current.push(ch);
            }
        }
    }
    if started {
        words.push(current);
    }
    Ok(words)
}

fn env_assignment(token: &str) -> Option<(String, String)> {
    let (name, value) = token.split_once('=')?;
    let mut chars = name.chars();
    let first = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return None;
    }
    Some((name.to_string(), value.to_string()))
}

fn describe_terminal(terminal: TerminalStatus) -> String {
    if terminal.signaled {
        return "signaled".to_string();
    }
    match terminal.exit_code {
        Some(code) => format!("exit {code}"),
        None => "exit unknown".to_string(),
    }
}

struct CommandRun {
    output: String,
    timed_out: bool,
    terminal: TerminalStatus,
}

fn execute_with_timeout(
    envs: &[(String, String)],
    argv: &[String],
    cwd: &Path,
    timeout: Duration,
) -> Result<CommandRun, String> {
    let Some((program, args)) = argv.split_first() else {
        return Err("witness command has no executable to run".to_string());
    };
    let mut command = ProcessCommand::new(program);
    command
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (name, value) in envs {
        command.env(name, value);
    }
    let mut child = command.spawn().map_err(|err| {
        format!(
            "confirm failed to spawn `{program}`: {err}; nothing was executed and no receipt was written"
        )
    })?;
    let stdout_reader = spawn_pipe_reader(child.stdout.take());
    let stderr_reader = spawn_pipe_reader(child.stderr.take());
    let started = Instant::now();
    let mut timed_out = false;
    let mut exit_code: Option<i32> = None;
    let mut signaled = false;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                exit_code = status.code();
                signaled = exit_signaled(&status);
                break;
            }
            Ok(None) => {
                if started.elapsed() >= timeout {
                    timed_out = true;
                    let _ = child.kill();
                    let _ = child.wait();
                    break;
                }
                thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
            }
            Err(err) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("confirm failed to wait for `{program}`: {err}"));
            }
        }
    }
    // Joining here can stall past the timeout when the killed child left
    // grandchildren holding the pipes: read_to_end only sees EOF once every
    // writer exits. A timed-out run therefore finishes promptly only when
    // the child has no surviving offspring. Killing the process group
    // instead is future work, not this slice.
    let mut output = join_pipe_reader(stdout_reader)?;
    output.push_str(&join_pipe_reader(stderr_reader)?);
    let terminal = TerminalStatus {
        exit_code,
        signaled,
        captured_complete: !timed_out,
    };
    Ok(CommandRun {
        output,
        timed_out,
        terminal,
    })
}

#[cfg(unix)]
fn exit_signaled(status: &std::process::ExitStatus) -> bool {
    use std::os::unix::process::ExitStatusExt;
    status.signal().is_some()
}

#[cfg(not(unix))]
fn exit_signaled(_status: &std::process::ExitStatus) -> bool {
    false
}

fn spawn_pipe_reader<R: Read + Send + 'static>(
    pipe: Option<R>,
) -> Option<thread::JoinHandle<Vec<u8>>> {
    pipe.map(|mut pipe| {
        thread::spawn(move || {
            let mut buffer = Vec::new();
            let _ = pipe.read_to_end(&mut buffer);
            buffer
        })
    })
}

fn join_pipe_reader(handle: Option<thread::JoinHandle<Vec<u8>>>) -> Result<String, String> {
    let Some(handle) = handle else {
        return Ok(String::new());
    };
    handle
        .join()
        .map(|buffer| String::from_utf8_lossy(&buffer).into_owned())
        .map_err(|_panic| "confirm output reader thread panicked".to_string())
}

fn write_raw_output_log(root: &Path, card_id: &str, output: &str) -> Result<String, String> {
    if output.trim().is_empty() {
        return Ok("; no command output was captured".to_string());
    }
    let path = confirm_log_path(root, card_id);
    ensure_parent_dir(&path)?;
    fs::write(&path, output).map_err(|err| format!("write {} failed: {err}", path.display()))?;
    Ok(format!("; raw output saved at {}", path.display()))
}

fn confirm_log_path(root: &Path, card_id: &str) -> PathBuf {
    let prefix = card_id
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
        .take(48)
        .collect::<String>();
    let prefix = if prefix.is_empty() {
        "card".to_string()
    } else {
        prefix
    };
    root.join("target")
        .join("unsafe-review-confirm")
        .join(format!("{prefix}-output.log"))
}

fn receipt_output_path(options: &ConfirmOptions, receipt: &WitnessReceipt) -> PathBuf {
    if let Some(out) = &options.out {
        return out.clone();
    }
    let hash = WitnessReceipt::command_hash(&receipt.card_id);
    options
        .root
        .join(".unsafe-review")
        .join("receipts")
        .join(format!("confirm-{}-{hash}.json", receipt.tool))
}

fn current_utc_timestamp() -> Result<String, String> {
    let secs = unix_seconds()?;
    let (year, month, day) = civil_from_days((secs / 86_400) as i64);
    let rem = secs % 86_400;
    Ok(format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3_600,
        (rem % 3_600) / 60,
        rem % 60
    ))
}

fn default_expires_at() -> Result<String, String> {
    Ok(expires_after_days((unix_seconds()? / 86_400) as i64))
}

fn expires_after_days(today_days_since_epoch: i64) -> String {
    date_for_days(today_days_since_epoch + DEFAULT_EXPIRES_DAYS)
}

fn date_for_days(days_since_epoch: i64) -> String {
    let (year, month, day) = civil_from_days(days_since_epoch);
    format!("{year:04}-{month:02}-{day:02}")
}

fn unix_seconds() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|err| format!("system clock before UNIX_EPOCH: {err}"))
}

// Mirrors the civil-date conversion used by receipt auditing in
// `unsafe-review-core/src/analysis/receipts.rs`.
fn civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + i64::from(month <= 2);
    (year as i32, month as u32, day as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_command_line_splits_env_assignments_and_argv() -> Result<(), String> {
        let invocation =
            parse_command_line("RUSTFLAGS='-Z sanitizer=address' cargo +nightly test read_header")?;

        assert_eq!(
            invocation.env,
            vec![("RUSTFLAGS".to_string(), "-Z sanitizer=address".to_string())]
        );
        assert_eq!(invocation.program, "cargo".to_string());
        assert_eq!(invocation.args, vec!["+nightly", "test", "read_header"]);
        assert_eq!(
            invocation.exec_argv(),
            vec!["cargo", "+nightly", "test", "read_header"]
        );
        Ok(())
    }

    #[test]
    fn parse_command_line_accepts_multiple_unquoted_env_assignments() -> Result<(), String> {
        let invocation = parse_command_line(
            "MIRIFLAGS=-Zmiri-strict-provenance RUST_BACKTRACE=1 cargo +nightly miri test read_header",
        )?;

        assert_eq!(
            invocation.env,
            vec![
                (
                    "MIRIFLAGS".to_string(),
                    "-Zmiri-strict-provenance".to_string()
                ),
                ("RUST_BACKTRACE".to_string(), "1".to_string()),
            ]
        );
        assert_eq!(invocation.program, "cargo".to_string());
        assert_eq!(
            invocation.args,
            vec!["+nightly", "miri", "test", "read_header"]
        );
        Ok(())
    }

    #[test]
    fn parse_command_line_does_not_treat_flag_equals_values_as_env_assignments()
    -> Result<(), String> {
        let invocation = parse_command_line("cargo kani --harness=byte_to_bool_harness")?;

        assert!(invocation.env.is_empty());
        assert_eq!(invocation.program, "cargo".to_string());
        assert_eq!(
            invocation.args,
            vec!["kani", "--harness=byte_to_bool_harness"]
        );
        Ok(())
    }

    #[test]
    fn parse_command_line_rejects_unterminated_quotes_and_empty_commands() {
        let unterminated = parse_command_line("RUSTFLAGS='-Z sanitizer=address cargo test");
        assert_eq!(
            unterminated,
            Err("witness command has an unterminated single quote".to_string())
        );

        let env_only = parse_command_line("RUST_BACKTRACE=1");
        assert_eq!(
            env_only,
            Err(
                "witness command has no executable to run after environment assignments"
                    .to_string()
            )
        );
    }

    #[test]
    fn parse_command_line_preserves_quoted_paths_and_empty_arguments() -> Result<(), String> {
        let invocation = parse_command_line(r#""/opt/my tools/runner" --filter "a b" ''"#)?;

        assert_eq!(invocation.program, "/opt/my tools/runner".to_string());
        assert_eq!(invocation.args, vec!["--filter", "a b", ""]);
        assert_eq!(
            invocation.exec_argv(),
            vec!["/opt/my tools/runner", "--filter", "a b", ""]
        );
        Ok(())
    }

    #[test]
    fn parse_command_line_supports_escapes_and_double_quotes() -> Result<(), String> {
        let invocation = parse_command_line(r#"prog a\ b "c\"d" e"#)?;

        assert_eq!(invocation.program, "prog".to_string());
        assert_eq!(invocation.args, vec!["a b", "c\"d", "e"]);
        Ok(())
    }

    #[test]
    fn parse_command_line_rejects_shell_syntax_explicitly() {
        for command in [
            "cargo test foo | grep bar",
            "cargo test foo; cargo test bar",
            "cargo test foo && cargo test bar",
            "cargo test > out.txt",
            "echo $(uname)",
            "echo `uname`",
            "FOO=$HOME cargo test",
        ] {
            let err = parse_command_line(command).err().unwrap_or_default();
            assert!(
                err.contains("unsupported shell syntax"),
                "command `{command}` must fail explicitly, got `{err}`"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn quoted_program_path_with_spaces_reaches_child_intact() -> Result<(), String> {
        use std::os::unix::fs::PermissionsExt;
        let dir =
            std::env::temp_dir().join(format!("unsafe-review spaced dir {}", std::process::id()));
        fs::create_dir_all(&dir).map_err(|err| format!("create spaced temp dir failed: {err}"))?;
        let program = dir.join("echo child");
        fs::write(&program, "#!/bin/sh\nprintf 'got:[%s]\\n' \"$@\"\n")
            .map_err(|err| format!("write probe child failed: {err}"))?;
        fs::set_permissions(&program, fs::Permissions::from_mode(0o755))
            .map_err(|err| format!("chmod probe child failed: {err}"))?;

        let command = format!("\"{}\" 'a b' c", program.display());
        let invocation = parse_command_line(&command)?;
        assert_eq!(invocation.program, program.to_string_lossy());
        assert_eq!(invocation.args, vec!["a b".to_string(), "c".to_string()]);

        let run = execute_with_timeout(
            &invocation.env,
            &invocation.exec_argv(),
            Path::new("."),
            Duration::from_secs(30),
        )?;
        let _ = fs::remove_dir_all(&dir);

        assert!(!run.timed_out);
        assert_eq!(run.terminal.exit_code, Some(0));
        assert!(run.output.contains("got:[a b]"), "output: {}", run.output);
        assert!(run.output.contains("got:[c]"), "output: {}", run.output);
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn spaced_argument_reaches_windows_child_intact() -> Result<(), String> {
        let invocation = parse_command_line("cmd /C echo \"hello world\"")?;
        assert_eq!(invocation.program, "cmd".to_string());
        assert_eq!(invocation.args, vec!["/C", "echo", "hello world"]);

        let run = execute_with_timeout(
            &invocation.env,
            &invocation.exec_argv(),
            Path::new("."),
            Duration::from_secs(30),
        )?;

        assert!(!run.timed_out);
        assert_eq!(run.terminal.exit_code, Some(0));
        assert!(run.output.contains("hello world"), "output: {}", run.output);
        Ok(())
    }

    #[test]
    fn parse_command_line_rejects_trailing_backslash() {
        let err = parse_command_line("cargo test foo\\")
            .err()
            .unwrap_or_default();
        assert!(err.contains("trailing backslash"), "got `{err}`");
    }

    #[cfg(unix)]
    #[test]
    fn execute_with_timeout_kills_and_reaps_the_timed_out_child() -> Result<(), String> {
        let started = Instant::now();
        let run = execute_with_timeout(
            &[],
            &[
                "sh".to_string(),
                "-c".to_string(),
                "exec sleep 30".to_string(),
            ],
            Path::new("."),
            Duration::from_millis(50),
        )?;

        assert!(run.timed_out);
        assert!(run.output.is_empty());
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "timed-out child should be killed and reaped promptly"
        );
        Ok(())
    }

    #[test]
    fn confirm_lane_maps_route_kinds_to_receipt_constructors() -> Result<(), String> {
        let card_id = "UR-fixture-c1";
        assert_eq!(confirm_lane(WitnessKind::Miri, card_id)?, ConfirmLane::Miri);
        assert_eq!(
            confirm_lane(WitnessKind::CargoCareful, card_id)?,
            ConfirmLane::CargoCareful
        );
        assert_eq!(
            confirm_lane(WitnessKind::AddressSanitizer, card_id)?,
            ConfirmLane::Sanitizer("asan")
        );
        assert_eq!(
            confirm_lane(WitnessKind::MemorySanitizer, card_id)?,
            ConfirmLane::Sanitizer("msan")
        );
        assert_eq!(
            confirm_lane(WitnessKind::ThreadSanitizer, card_id)?,
            ConfirmLane::Sanitizer("tsan")
        );
        assert_eq!(
            confirm_lane(WitnessKind::LeakSanitizer, card_id)?,
            ConfirmLane::Sanitizer("lsan")
        );
        assert_eq!(
            confirm_lane(WitnessKind::Loom, card_id)?,
            ConfirmLane::Concurrency("loom")
        );
        assert_eq!(
            confirm_lane(WitnessKind::Shuttle, card_id)?,
            ConfirmLane::Concurrency("shuttle")
        );
        assert_eq!(
            confirm_lane(WitnessKind::Kani, card_id)?,
            ConfirmLane::Proof("kani")
        );
        assert_eq!(
            confirm_lane(WitnessKind::Crux, card_id)?,
            ConfirmLane::Proof("crux")
        );
        Ok(())
    }

    #[test]
    fn confirm_lane_refuses_human_deep_review_and_unsupported_kinds() {
        let human = confirm_lane(WitnessKind::HumanDeepReview, "UR-fixture-c1");
        let err = human.err().unwrap_or_default();
        assert!(err.contains("human-deep-review"));
        assert!(err.contains("nothing was executed"));

        let unsupported = confirm_lane(WitnessKind::Unsupported, "UR-fixture-c1");
        let err = unsupported.err().unwrap_or_default();
        assert!(err.contains("unsupported witness kind"));
        assert!(err.contains("nothing was executed"));
    }

    #[test]
    fn select_route_picks_first_route_with_a_command() -> Result<(), String> {
        let routes = vec![
            WitnessRoute {
                kind: WitnessKind::HumanDeepReview,
                reason: "manual review".to_string(),
                command: None,
                required: false,
            },
            WitnessRoute {
                kind: WitnessKind::Miri,
                reason: "pure-Rust hazard".to_string(),
                command: Some("cargo +nightly miri test read_header".to_string()),
                required: false,
            },
        ];

        let (kind, command) = select_route("UR-fixture-c1", &routes)?;
        assert_eq!(kind, WitnessKind::Miri);
        assert_eq!(command, "cargo +nightly miri test read_header");
        Ok(())
    }

    #[test]
    fn select_route_reports_missing_routed_command_honestly() {
        let routes = vec![WitnessRoute {
            kind: WitnessKind::HumanDeepReview,
            reason: "manual review".to_string(),
            command: None,
            required: false,
        }];

        let err = select_route("UR-fixture-c1", &routes)
            .err()
            .unwrap_or_default();
        assert!(err.contains("no routed witness command"));
        assert!(err.contains("witness-plan.md"));
        assert!(err.contains("nothing was executed"));
    }

    #[test]
    fn default_expiry_is_thirty_days_after_today() {
        assert_eq!(expires_after_days(0), "1970-01-31");
        assert_eq!(date_for_days(0), "1970-01-01");
        assert_eq!(date_for_days(31), "1970-02-01");
        // 2026-06-06 is day 20_610 since the epoch; thirty days later is 2026-07-06.
        assert_eq!(date_for_days(20_610), "2026-06-06");
        assert_eq!(expires_after_days(20_610), "2026-07-06");
    }

    #[test]
    fn build_receipt_records_confirm_limitation_for_miri_lane() -> Result<(), String> {
        let receipt = build_receipt(
            ConfirmLane::Miri,
            ReceiptFields {
                card_id:
                    "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                        .to_string(),
                output: "running 1 test\ntest read_header ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; finished in 0.01s\n"
                    .to_string(),
                author: "core/fixtures".to_string(),
                recorded_at: "2026-06-06T00:00:00Z".to_string(),
                expires_at: "2026-07-06".to_string(),
                command: "cargo +nightly miri test read_header".to_string(),
                terminal: TerminalStatus::exited(0),
                binding: None,
            },
        )?;

        assert_eq!(receipt.tool, "miri");
        assert_eq!(receipt.strength, "ran");
        let limitations = receipt.limitations.unwrap_or_default();
        assert!(limitations.iter().any(|item| item == CONFIRM_LIMITATION));
        assert!(
            limitations
                .iter()
                .any(|item| item == "executed-output adapter; unsafe-review ran Miri")
        );
        assert!(limitations.iter().all(|item| !item.contains("did not run")));
        Ok(())
    }

    #[test]
    fn build_receipt_rejects_unclassified_output_without_fabricating() {
        let result = build_receipt(
            ConfirmLane::Miri,
            ReceiptFields {
                card_id:
                    "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                        .to_string(),
                output: "warning: nothing ran\n".to_string(),
                author: "core/fixtures".to_string(),
                recorded_at: "2026-06-06T00:00:00Z".to_string(),
                expires_at: "2026-07-06".to_string(),
                command: "cargo +nightly miri test read_header".to_string(),
                terminal: TerminalStatus::exited(0),
                binding: None,
            },
        );

        assert!(result.err().unwrap_or_default().contains("test result: ok"));
    }

    #[test]
    fn describe_terminal_reports_exit_and_signal() {
        assert_eq!(
            describe_terminal(TerminalStatus::exited(0)),
            "exit 0".to_string()
        );
        assert_eq!(
            describe_terminal(TerminalStatus::exited(7)),
            "exit 7".to_string()
        );
        assert_eq!(
            describe_terminal(TerminalStatus::signaled()),
            "signaled".to_string()
        );
        assert_eq!(
            describe_terminal(TerminalStatus::unknown()),
            "exit unknown".to_string()
        );
    }

    #[cfg(unix)]
    #[test]
    fn execute_with_timeout_retains_nonzero_exit_behind_success_output() -> Result<(), String> {
        let run = execute_with_timeout(
            &[],
            &[
                "sh".to_string(),
                "-c".to_string(),
                "printf 'test result: ok. 1 passed; 0 failed;\\n'; exit 7".to_string(),
            ],
            Path::new("."),
            Duration::from_secs(30),
        )?;

        assert!(!run.timed_out);
        assert!(run.output.contains("test result: ok"));
        assert_eq!(run.terminal.exit_code, Some(7));
        assert!(!run.terminal.signaled);
        assert!(run.terminal.captured_complete);
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn execute_with_timeout_retains_nonzero_exit_behind_success_output() -> Result<(), String> {
        let run = execute_with_timeout(
            &[],
            &[
                "cmd".to_string(),
                "/C".to_string(),
                "echo test result: ok & exit 7".to_string(),
            ],
            Path::new("."),
            Duration::from_secs(30),
        )?;

        assert!(!run.timed_out);
        assert!(run.output.contains("test result: ok"));
        assert_eq!(run.terminal.exit_code, Some(7));
        assert!(!run.terminal.signaled);
        assert!(run.terminal.captured_complete);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn execute_with_timeout_retains_signal_termination() -> Result<(), String> {
        let run = execute_with_timeout(
            &[],
            &[
                "sh".to_string(),
                "-c".to_string(),
                "kill -TERM $$".to_string(),
            ],
            Path::new("."),
            Duration::from_secs(30),
        )?;

        assert!(!run.timed_out);
        assert!(run.terminal.signaled);
        assert!(run.terminal.captured_complete);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn execute_with_timeout_marks_killed_capture_incomplete() -> Result<(), String> {
        let run = execute_with_timeout(
            &[],
            &[
                "sh".to_string(),
                "-c".to_string(),
                "exec sleep 30".to_string(),
            ],
            Path::new("."),
            Duration::from_secs(1),
        )?;

        assert!(run.timed_out);
        assert!(!run.terminal.captured_complete);
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn execute_with_timeout_marks_killed_capture_incomplete() -> Result<(), String> {
        let run = execute_with_timeout(
            &[],
            &[
                "cmd".to_string(),
                "/C".to_string(),
                "timeout /T 30 /NOBREAK > NUL".to_string(),
            ],
            Path::new("."),
            Duration::from_secs(1),
        )?;

        assert!(run.timed_out);
        assert!(!run.terminal.captured_complete);
        Ok(())
    }

    #[test]
    fn build_receipt_downgrades_success_output_on_nonzero_exit() -> Result<(), String> {
        let receipt = build_receipt(
            ConfirmLane::Miri,
            ReceiptFields {
                card_id:
                    "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                        .to_string(),
                output: "running 1 test\ntest read_header ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; finished in 0.01s\n"
                    .to_string(),
                author: "core/fixtures".to_string(),
                recorded_at: "2026-06-06T00:00:00Z".to_string(),
                expires_at: "2026-07-06".to_string(),
                command: "cargo +nightly miri test read_header".to_string(),
                terminal: TerminalStatus::exited(7),
                binding: None,
            },
        )?;

        assert_eq!(receipt.verdict.as_deref(), Some("inconclusive"));
        assert_eq!(receipt.exit_code, Some(7));
        Ok(())
    }

    #[test]
    fn subject_digest_is_deterministic_and_sensitive() {
        let parts = ["UR-test-c1", "miri", "owner", "src/lib.rs", "snippet"];
        assert_eq!(
            SubjectBinding::digest_subject(&parts),
            SubjectBinding::digest_subject(&parts)
        );
        assert_ne!(
            SubjectBinding::digest_subject(&parts),
            SubjectBinding::digest_subject(&[
                "UR-test-c2",
                "miri",
                "owner",
                "src/lib.rs",
                "snippet"
            ])
        );
    }

    #[test]
    fn git_probe_reports_revision_and_dirtiness() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let head = git_head(root);
        let dirty = git_dirty(root);
        if let (Some(head), Some(_)) = (head, dirty) {
            assert_eq!(head.len(), 40, "head: {head}");
            assert!(head.chars().all(|ch| ch.is_ascii_hexdigit()));
        }
    }

    #[test]
    fn build_receipt_preserves_subject_binding() -> Result<(), String> {
        let binding = SubjectBinding {
            subject_digest: "digest".to_string(),
            invocation_digest: Some("invocation".to_string()),
            scope: Some("repo".to_string()),
            head_commit: Some("abc123".to_string()),
            repo_dirty: Some(false),
            workdir: Some(".".to_string()),
            output_digest: Some("output".to_string()),
            captured_complete: Some(true),
            tool_version: Some("0.5.0".to_string()),
        };
        let receipt = build_receipt(
            ConfirmLane::Miri,
            ReceiptFields {
                card_id:
                    "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                        .to_string(),
                output: "running 1 test\ntest read_header ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; finished in 0.01s\n"
                    .to_string(),
                author: "core/fixtures".to_string(),
                recorded_at: "2026-06-06T00:00:00Z".to_string(),
                expires_at: "2026-07-06".to_string(),
                command: "cargo +nightly miri test read_header".to_string(),
                terminal: TerminalStatus::exited(0),
                binding: Some(binding.clone()),
            },
        )?;

        assert_eq!(receipt.subject, Some(binding));
        Ok(())
    }

    #[test]
    fn confirm_log_path_uses_sanitized_card_id_prefix() {
        let path = confirm_log_path(
            Path::new("fixtures/raw_pointer_alignment"),
            "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1",
        );
        let rendered = path.to_string_lossy().replace('\\', "/");
        assert!(
            rendered.starts_with("fixtures/raw_pointer_alignment/target/unsafe-review-confirm/")
        );
        assert!(rendered.ends_with("-output.log"));
        assert!(rendered.contains("UR-crate-src-lib-rs-owner"));
    }

    const DRY_RUN_FIXTURE_ROOT: &str = "../../fixtures/copy_nonoverlapping";
    const DRY_RUN_CARD_ID: &str = "UR-copy-nonoverlapping-src-lib-rs-copy-bytes-operation-copy_nonoverlapping-copy-nonoverlapping-a02b5acd2c90-pointer_validity-c1";

    fn dry_run_options(command: &str) -> ConfirmOptions {
        ConfirmOptions {
            card_id: DRY_RUN_CARD_ID.to_string(),
            root: PathBuf::from(DRY_RUN_FIXTURE_ROOT),
            base: None,
            diff: None,
            dry_run: true,
            author: String::new(),
            expires_at: None,
            timeout_seconds: 600,
            command: Some(command.to_string()),
            out: None,
        }
    }

    fn assert_no_confirm_sidecars() {
        let root = Path::new(DRY_RUN_FIXTURE_ROOT);
        assert!(
            !root.join(".unsafe-review").exists(),
            "dry run must not write receipts"
        );
        assert!(
            !root.join("target").exists(),
            "dry run must not write raw output logs"
        );
    }

    #[test]
    fn dry_run_rejects_unterminated_quote_override() {
        let options = dry_run_options("RUSTFLAGS='-Z sanitizer=address cargo test");
        let err = run(options).err().unwrap_or_default();
        assert!(
            err.contains("unterminated single quote"),
            "dry run must surface the parse failure, got `{err}`"
        );
        assert_no_confirm_sidecars();
    }

    #[test]
    fn dry_run_rejects_unsupported_shell_syntax_override() {
        let options = dry_run_options("cargo test foo | grep bar");
        let err = run(options).err().unwrap_or_default();
        assert!(
            err.contains("unsupported shell syntax"),
            "dry run must surface the parse failure, got `{err}`"
        );
        assert_no_confirm_sidecars();
    }

    #[test]
    #[cfg(unix)]
    fn dry_run_spawns_no_child_process() -> Result<(), String> {
        let probe = std::env::temp_dir().join("unsafe-review-confirm-dry-run-probe");
        let _ = std::fs::remove_file(&probe);
        run(dry_run_options(&format!("touch {}", probe.display())))?;
        assert!(
            !probe.exists(),
            "dry run must preview without executing the parsed invocation"
        );
        let _ = std::fs::remove_file(&probe);
        assert_no_confirm_sidecars();
        Ok(())
    }

    #[test]
    fn diff_resolved_cards_cannot_execute() -> Result<(), String> {
        let options = ConfirmOptions {
            diff: Some(crate::command::DiffInput::Stdin),
            ..ConfirmOptions::default()
        };
        assert!(reject_diff_execution(&options).is_err());
        let options = ConfirmOptions::default();
        reject_diff_execution(&options)?;
        Ok(())
    }

    #[test]
    fn describe_env_redacts_assignment_values() -> Result<(), String> {
        let invocation = parse_command_line("AWS_SECRET_ACCESS_KEY=hunter2 cargo test")?;
        let shown = invocation.describe_env();
        assert!(shown.contains("AWS_SECRET_ACCESS_KEY"));
        assert!(!shown.contains("hunter2"));
        Ok(())
    }

    #[test]
    fn git_probes_return_none_outside_a_repository() -> Result<(), String> {
        let root = std::env::temp_dir().join(format!(
            "unsafe-review-confirm-no-repo-{}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        }
        fs::create_dir_all(&root).map_err(|err| format!("create temp dir failed: {err}"))?;

        assert_eq!(git_head(&root), None);
        assert_eq!(git_dirty(&root), None);

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        Ok(())
    }
}
