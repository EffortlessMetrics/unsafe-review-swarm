use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use unsafe_review_core::{
    AnalysisMode, AnalyzeInput, CargoCarefulReceiptInput, ConcurrencyReceiptInput,
    MiriReceiptInput, PolicyMode, ProofReceiptInput, ReviewCard, SanitizerReceiptInput, Scope,
    WitnessKind, WitnessReceipt, WitnessRoute, analyze,
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
    let card = resolve_card(&options)?;
    let (kind, routed_command) = select_route(&card.id.0, &card.routes)?;
    let lane = confirm_lane(kind, &card.id.0)?;
    let command_source = if options.command.is_some() {
        CommandSource::CommandOverride
    } else {
        CommandSource::AnalyzerRoute
    };
    let command_text = options.command.clone().unwrap_or(routed_command);
    if options.dry_run {
        print_dry_run(&options, &card, kind, lane, &command_text, command_source);
        return Ok(());
    }
    println!("command provenance: {}", command_source.label());
    let (envs, argv) = parse_command_line(&command_text)?;
    let execution = execute_with_timeout(
        &envs,
        &argv,
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
    let receipt = match build_receipt(
        lane,
        ReceiptFields {
            card_id: card.id.0.clone(),
            output: execution.output.clone(),
            author: options.author.clone(),
            recorded_at,
            expires_at,
            command: command_text.clone(),
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
) {
    println!("unsafe-review confirm (dry run)");
    println!("card: {}", card.id.0);
    println!("operation family: {}", card.operation.family.as_str());
    println!("route: {}", kind.as_str());
    println!("command: {command_text}");
    println!("command provenance: {}", command_source.label());
    println!("working directory: {}", options.root.display());
    println!("timeout: {}s", options.timeout_seconds);
    println!(
        "expected evidence: a `{}` witness receipt classified by the existing saved-output `{}` import constructor",
        lane.tool_name(),
        lane.tool_name()
    );
    println!();
    println!(
        "limits: dry run only; nothing was executed; unsafe-review never executes witnesses by default."
    );
    println!("trust boundary: {FIRST_RUN_TRUST_BOUNDARY}");
}

fn resolve_card(options: &ConfirmOptions) -> Result<ReviewCard, String> {
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
        return Ok(card.clone());
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
}

fn build_receipt(lane: ConfirmLane, fields: ReceiptFields) -> Result<WitnessReceipt, String> {
    let limitations = vec![CONFIRM_LIMITATION.to_string()];
    match lane {
        ConfirmLane::Miri => WitnessReceipt::from_miri_output(MiriReceiptInput {
            card_id: fields.card_id,
            output: fields.output,
            author: fields.author,
            recorded_at: fields.recorded_at,
            expires_at: fields.expires_at,
            command: fields.command,
            limitations,
        }),
        ConfirmLane::CargoCareful => {
            WitnessReceipt::from_cargo_careful_output(CargoCarefulReceiptInput {
                card_id: fields.card_id,
                output: fields.output,
                author: fields.author,
                recorded_at: fields.recorded_at,
                expires_at: fields.expires_at,
                command: fields.command,
                limitations,
            })
        }
        ConfirmLane::Sanitizer(tool) => {
            WitnessReceipt::from_sanitizer_output(SanitizerReceiptInput {
                card_id: fields.card_id,
                tool: tool.to_string(),
                output: fields.output,
                author: fields.author,
                recorded_at: fields.recorded_at,
                expires_at: fields.expires_at,
                command: fields.command,
                limitations,
                allow_runtime: false,
            })
        }
        ConfirmLane::Concurrency(tool) => {
            WitnessReceipt::from_concurrency_output(ConcurrencyReceiptInput {
                card_id: fields.card_id,
                tool: tool.to_string(),
                output: fields.output,
                author: fields.author,
                recorded_at: fields.recorded_at,
                expires_at: fields.expires_at,
                command: fields.command,
                limitations,
            })
        }
        ConfirmLane::Proof(tool) => WitnessReceipt::from_proof_output(ProofReceiptInput {
            card_id: fields.card_id,
            tool: tool.to_string(),
            output: fields.output,
            author: fields.author,
            recorded_at: fields.recorded_at,
            expires_at: fields.expires_at,
            command: fields.command,
            limitations,
        }),
    }
}

type EnvAssignments = Vec<(String, String)>;

fn parse_command_line(command: &str) -> Result<(EnvAssignments, Vec<String>), String> {
    let mut env = Vec::new();
    let mut rest = command.trim_start();
    while let Some((token, remainder)) = next_token(rest)? {
        let Some(assignment) = env_assignment(&token) else {
            break;
        };
        env.push(assignment);
        rest = remainder.trim_start();
    }
    let argv = rest
        .split_whitespace()
        .map(str::to_string)
        .collect::<Vec<_>>();
    if argv.is_empty() {
        return Err(
            "witness command has no executable to run after environment assignments".to_string(),
        );
    }
    Ok((env, argv))
}

fn next_token(text: &str) -> Result<Option<(String, &str)>, String> {
    let text = text.trim_start();
    if text.is_empty() {
        return Ok(None);
    }
    let mut in_quote = false;
    for (idx, ch) in text.char_indices() {
        match ch {
            '\'' => in_quote = !in_quote,
            ch if ch.is_whitespace() && !in_quote => {
                return Ok(Some((text[..idx].to_string(), &text[idx..])));
            }
            _ => {}
        }
    }
    if in_quote {
        return Err("witness command has an unterminated single quote".to_string());
    }
    Ok(Some((text.to_string(), "")))
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
    Some((name.to_string(), strip_single_quotes(value)))
}

fn strip_single_quotes(value: &str) -> String {
    value
        .strip_prefix('\'')
        .and_then(|stripped| stripped.strip_suffix('\''))
        .map_or_else(|| value.to_string(), str::to_string)
}

struct CommandRun {
    output: String,
    timed_out: bool,
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
    loop {
        match child.try_wait() {
            Ok(Some(_status)) => break,
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
    let mut output = join_pipe_reader(stdout_reader)?;
    output.push_str(&join_pipe_reader(stderr_reader)?);
    Ok(CommandRun { output, timed_out })
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
        let (envs, argv) =
            parse_command_line("RUSTFLAGS='-Z sanitizer=address' cargo +nightly test read_header")?;

        assert_eq!(
            envs,
            vec![("RUSTFLAGS".to_string(), "-Z sanitizer=address".to_string())]
        );
        assert_eq!(argv, vec!["cargo", "+nightly", "test", "read_header"]);
        Ok(())
    }

    #[test]
    fn parse_command_line_accepts_multiple_unquoted_env_assignments() -> Result<(), String> {
        let (envs, argv) = parse_command_line(
            "MIRIFLAGS=-Zmiri-strict-provenance RUST_BACKTRACE=1 cargo +nightly miri test read_header",
        )?;

        assert_eq!(
            envs,
            vec![
                (
                    "MIRIFLAGS".to_string(),
                    "-Zmiri-strict-provenance".to_string()
                ),
                ("RUST_BACKTRACE".to_string(), "1".to_string()),
            ]
        );
        assert_eq!(
            argv,
            vec!["cargo", "+nightly", "miri", "test", "read_header"]
        );
        Ok(())
    }

    #[test]
    fn parse_command_line_does_not_treat_flag_equals_values_as_env_assignments()
    -> Result<(), String> {
        let (envs, argv) = parse_command_line("cargo kani --harness=byte_to_bool_harness")?;

        assert!(envs.is_empty());
        assert_eq!(
            argv,
            vec!["cargo", "kani", "--harness=byte_to_bool_harness"]
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
            },
        )?;

        assert_eq!(receipt.tool, "miri");
        assert_eq!(receipt.strength, "ran");
        let limitations = receipt.limitations.unwrap_or_default();
        assert!(limitations.iter().any(|item| item == CONFIRM_LIMITATION));
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
            },
        );

        assert!(result.err().unwrap_or_default().contains("test result: ok"));
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
}
