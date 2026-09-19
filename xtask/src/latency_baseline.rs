//! Standalone `latency-baseline` command: record phase-latency baselines
//! for the CLI review paths (#2309 PR1).
//!
//! The matrix is fixed and small: two fixture `check` rows (S/M by card
//! count), one `first-pr` bundle row, one pinned-repo `repo` row, and one
//! scripted LSP save-loop row. Each row runs N times against a prebuilt
//! binary. Run 0 is labeled `first_process_run` and the rest
//! `repeat_process_run`: every sample starts a new process, no OS cache is
//! dropped, no same-process analyzer state is preserved, and there is no
//! persistent analysis cache, so the labels describe repetition, never a
//! proven cache state.
//!
//! Every CLI run receipt is validated before it enters the aggregate
//! (schema version, command, scope, identity, outcome, exact phase order,
//! numeric fields, total consistency). A wrong-shaped receipt fails the row
//! instead of recording a silent `null`.
//!
//! The repo row records the exact analyzed commit of the pinned directory;
//! the row is skipped with a recorded reason when the directory is absent
//! or its commit is unavailable. Staged-diff baselines wait for #2308 input
//! identity; file-range waits for a file input that does not exist yet.
//!
//! Diagnostic only — not a coverage claim, proof, UB-free, Miri-clean,
//! site-execution, or performance guarantee.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::lsp_smoke;

const DEFAULT_OUT: &str = "docs/latency/baseline.json";
const DEFAULT_RUNS: usize = 3;
const RECEIPT_SCHEMA: &str = "1.0";
const LSP_TIMEOUT: Duration = Duration::from_secs(120);

const FIXTURE_S: &str = "fixtures/ffi_return_value_checked_guard";
const FIXTURE_M: &str = "fixtures/ffi_argument_nullable_no_invented_obligation";
const FIXTURE_LSP: &str = "fixtures/raw_pointer_alignment";
const MEMCHR_WORK_DIR: &str = "target/dogfood-work/memchr";

/// Probe appended to the task-owned LSP fixture copy. It adds a second
/// intentionally unguarded unsafe site, so a genuine reanalysis of the new
/// saved bytes must publish a diagnostics set that strictly grows the
/// baseline set. Appending never shifts the original site's lines, so the
/// baseline diagnostics must appear verbatim in the fresh publication.
const LSP_PROBE: &str = r#"
pub fn write_header(bytes: &mut [u8], header: Header) {
    assert!(bytes.len() >= core::mem::size_of::<Header>());
    let ptr = bytes.as_mut_ptr();
    // latency-baseline probe: second intentionally unguarded site.
    unsafe { ptr.cast::<Header>().write(header) }
}
"#;

pub(crate) struct LatencyBaselineArgs {
    pub(crate) out: PathBuf,
    pub(crate) runs: usize,
    pub(crate) skip_lsp: bool,
    pub(crate) skip_repo: bool,
}

impl LatencyBaselineArgs {
    pub(crate) fn parse(args: &[String]) -> Result<Self, String> {
        let mut out = PathBuf::from(DEFAULT_OUT);
        let mut runs = DEFAULT_RUNS;
        let mut skip_lsp = false;
        let mut skip_repo = false;
        // `args` is the full argv; element 0 is the binary and element 1
        // is the subcommand, matching the DogfoodExec parse convention.
        let mut idx = 2usize;
        while idx < args.len() {
            match args[idx].as_str() {
                "--out" => {
                    idx += 1;
                    out = PathBuf::from(
                        args.get(idx)
                            .map(String::as_str)
                            .ok_or("--out requires a value")?,
                    );
                }
                arg if arg.starts_with("--out=") => {
                    out = PathBuf::from(
                        arg.split_once('=')
                            .map(|(_, v)| v)
                            .ok_or("--out requires a value")?,
                    );
                }
                "--runs" => {
                    idx += 1;
                    runs = parse_runs(args.get(idx).map(String::as_str))?;
                }
                arg if arg.starts_with("--runs=") => {
                    runs = parse_runs(arg.split_once('=').map(|(_, v)| v))?;
                }
                "--skip-lsp" => skip_lsp = true,
                "--skip-repo" => skip_repo = true,
                other => return Err(format!("unknown latency-baseline argument `{other}`")),
            }
            idx += 1;
        }
        Ok(Self {
            out,
            runs,
            skip_lsp,
            skip_repo,
        })
    }
}

fn parse_runs(raw: Option<&str>) -> Result<usize, String> {
    let runs = raw
        .ok_or("--runs requires a value")?
        .parse::<usize>()
        .map_err(|err| format!("invalid --runs: {err}"))?;
    if runs == 0 {
        return Err("--runs must be greater than 0".to_string());
    }
    Ok(runs)
}

pub(crate) fn run(workspace_root: &Path, args: &LatencyBaselineArgs) -> Result<(), String> {
    build_binaries(workspace_root)?;
    let review_bin = workspace_root.join("target/debug/unsafe-review");
    let lsp_bin = workspace_root.join("target/debug/cargo-unsafe-review");
    let work_dir = workspace_root.join("target/latency-work");
    fs::create_dir_all(&work_dir)
        .map_err(|err| format!("create {} failed: {err}", work_dir.display()))?;

    let mut rows = Vec::new();
    rows.push(cli_matrix_row(
        workspace_root,
        &review_bin,
        &work_dir,
        "fixture-s-check",
        "check",
        "diff",
        &check_fixture_args(workspace_root, FIXTURE_S),
        args.runs,
    )?);
    rows.push(cli_matrix_row(
        workspace_root,
        &review_bin,
        &work_dir,
        "fixture-m-check",
        "check",
        "diff",
        &check_fixture_args(workspace_root, FIXTURE_M),
        args.runs,
    )?);
    rows.push(cli_matrix_row_with_meta(
        workspace_root,
        &review_bin,
        &work_dir,
        "fixture-m-pr-bundle",
        "pr",
        "first-pr",
        "diff",
        &pr_fixture_args(workspace_root, FIXTURE_M),
        args.runs,
        json!({}),
    )?);
    if args.skip_repo {
        rows.push(skipped_row("repo-pinned-memchr", "skipped by --skip-repo"));
    } else {
        let memchr = workspace_root.join(MEMCHR_WORK_DIR);
        if memchr.is_dir() {
            match pinned_commit(&memchr) {
                Ok(commit) => rows.push(cli_matrix_row_with_meta(
                    workspace_root,
                    &review_bin,
                    &work_dir,
                    "repo-pinned-memchr",
                    "repo",
                    "repo",
                    "repo",
                    &[
                        "--root".to_string(),
                        memchr.display().to_string(),
                        "--format".to_string(),
                        "json".to_string(),
                        "--out".to_string(),
                        work_dir
                            .join("repo-pinned-memchr.json")
                            .display()
                            .to_string(),
                    ],
                    args.runs,
                    json!({"analyzed_commit": commit}),
                )?),
                Err(reason) => rows.push(skipped_row("repo-pinned-memchr", &reason)),
            }
        } else {
            rows.push(skipped_row(
                "repo-pinned-memchr",
                &format!(
                    "pinned work dir {} absent (clone target/dogfood-work/memchr first)",
                    memchr.display()
                ),
            ));
        }
    }
    if args.skip_lsp {
        rows.push(skipped_row("lsp-save-loop", "skipped by --skip-lsp"));
    } else {
        rows.push(lsp_save_loop_row(
            workspace_root,
            &lsp_bin,
            &work_dir,
            args.runs,
        )?);
    }

    let receipt = json!({
        "schema_version": RECEIPT_SCHEMA,
        "tool": "unsafe-review latency-baseline",
        "tool_version": cli_crate_version(workspace_root)?,
        "binary_profile": "debug",
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "runs_per_row": args.runs,
        "run_labels": "first_process_run is run 0 of each row in this invocation; repeat_process_run is a later run. Binaries are prebuilt once before the matrix. The labels describe repetition only: no cache is dropped, no same-process state is preserved, and there is no persistent analysis cache, so no cache state is claimed.",
        "trust_boundary": "Phase-latency baseline only; wall-clock times are machine- and load-dependent diagnostics, not performance guarantees, coverage claims, proofs, or UB-free/Miri-clean status. Deterministic counts (cards, bytes, sites) are comparable across machines; timings are not.",
        "pending_inputs": ["staged-diff (waits for #2308 input identity)", "file-range (no file input exists yet)"],
        "rows": rows,
        "proposed_budgets": {},
    });
    let out_abs = if args.out.is_absolute() {
        args.out.clone()
    } else {
        workspace_root.join(&args.out)
    };
    if let Some(parent) = out_abs.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create {} failed: {err}", parent.display()))?;
    }
    let receipt_text = serde_json::to_string_pretty(&receipt)
        .map_err(|err| format!("serialize baseline receipt failed: {err}"))?;
    fs::write(&out_abs, receipt_text)
        .map_err(|err| format!("write {} failed: {err}", out_abs.display()))?;
    println!("latency-baseline: ok ({})", out_abs.display());
    Ok(())
}

fn build_binaries(workspace_root: &Path) -> Result<(), String> {
    let status = Command::new("cargo")
        .args([
            "build",
            "--locked",
            "-p",
            "unsafe-review",
            "-p",
            "unsafe-review-cli",
        ])
        .current_dir(workspace_root)
        .status()
        .map_err(|err| format!("failed to build review binaries: {err}"))?;
    if !status.success() {
        return Err("building review binaries failed".to_string());
    }
    Ok(())
}

fn cli_crate_version(workspace_root: &Path) -> Result<String, String> {
    let manifest = fs::read_to_string(workspace_root.join("crates/unsafe-review-cli/Cargo.toml"))
        .map_err(|err| format!("read CLI manifest failed: {err}"))?;
    for line in manifest.lines() {
        if let Some(version) = line.strip_prefix("version = \"")
            && let Some(version) = version.strip_suffix('"')
        {
            return Ok(version.to_string());
        }
    }
    Err("CLI manifest has no version line".to_string())
}

/// Exact commit of a pinned external directory, or a skip reason when the
/// directory is not a readable git checkout. A row that cannot name its
/// input commit is skipped, never recorded against an assumed input.
///
/// The commit lookup is bound to the directory itself: `git -C <dir>`
/// walks up past a non-repository, so the toplevel it reports must
/// canonicalize to `dir`. Otherwise an empty or failed clone would record
/// the outer checkout's commit as the pinned input's.
fn pinned_commit(dir: &Path) -> Result<String, String> {
    let toplevel = Command::new("git")
        .args([
            "-C",
            &dir.display().to_string(),
            "rev-parse",
            "--show-toplevel",
        ])
        .output()
        .map_err(|err| format!("pinned dir {} is not a git checkout: {err}", dir.display()))?;
    if !toplevel.status.success() {
        return Err(format!(
            "pinned dir {} is not inside a git work tree",
            dir.display()
        ));
    }
    let reported = String::from_utf8_lossy(&toplevel.stdout).trim().to_string();
    let canonical_dir = dir
        .canonicalize()
        .map_err(|err| format!("pinned dir {} is unreadable: {err}", dir.display()))?;
    let canonical_top = PathBuf::from(&reported).canonicalize().map_err(|err| {
        format!(
            "pinned dir {} reports an unreadable toplevel: {err}",
            dir.display()
        )
    })?;
    if canonical_top != canonical_dir {
        return Err(format!(
            "pinned dir {} is not itself a repository (toplevel is {})",
            dir.display(),
            canonical_top.display()
        ));
    }
    let output = Command::new("git")
        .args(["-C", &dir.display().to_string(), "rev-parse", "HEAD"])
        .output()
        .map_err(|err| format!("pinned dir {} has no readable HEAD: {err}", dir.display()))?;
    if !output.status.success() {
        return Err(format!("pinned dir {} has no readable HEAD", dir.display()));
    }
    let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if sha.len() == 40 && sha.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(sha)
    } else {
        Err(format!(
            "pinned dir {} returned a malformed HEAD",
            dir.display()
        ))
    }
}

fn check_fixture_args(workspace_root: &Path, fixture: &str) -> Vec<String> {
    let root = workspace_root.join(fixture);
    vec![
        "--root".to_string(),
        root.display().to_string(),
        "--diff".to_string(),
        root.join("change.diff").display().to_string(),
        "--format".to_string(),
        "json".to_string(),
        "--out".to_string(),
        workspace_root
            .join("target/latency-work/cards.json")
            .display()
            .to_string(),
    ]
}

fn pr_fixture_args(workspace_root: &Path, fixture: &str) -> Vec<String> {
    let root = workspace_root.join(fixture);
    vec![
        "--root".to_string(),
        root.display().to_string(),
        "--diff".to_string(),
        root.join("change.diff").display().to_string(),
        "--out-dir".to_string(),
        workspace_root
            .join("target/latency-work/pr-bundle")
            .display()
            .to_string(),
    ]
}

fn expected_phases(command: &str) -> &'static [&'static str] {
    match command {
        "first-pr" => &[
            "input_resolution",
            "analyze",
            "receipt_audit",
            "policy_eval",
            "projections",
            "artifact_writes",
        ],
        _ => &[
            "input_resolution",
            "analyze",
            "projections",
            "artifact_writes",
            "policy_eval",
        ],
    }
}

/// Validate a CLI latency receipt before it enters the aggregate. Every
/// field the baseline compares must be present and well-shaped; anything
/// else fails the row instead of recording a silent `null`.
fn validate_receipt(
    id: &str,
    run: usize,
    command: &str,
    scope: &str,
    receipt: &Value,
) -> Result<(), String> {
    let fail = |what: &str| format!("row {id} run {run} receipt rejected: {what}");
    if receipt["schema_version"].as_str() != Some(RECEIPT_SCHEMA) {
        return Err(fail("schema_version is not 1.0"));
    }
    if receipt["command"].as_str() != Some(command) {
        return Err(fail("command does not match the row"));
    }
    if receipt["scope"].as_str() != Some(scope) {
        return Err(fail("scope does not match the row"));
    }
    if receipt["input_identity"].as_str().is_none_or(str::is_empty) {
        return Err(fail("input_identity is missing or empty"));
    }
    if receipt["options_digest"].as_str().is_none_or(str::is_empty) {
        return Err(fail("options_digest is missing or empty"));
    }
    if receipt["outcome"]["analysis"].as_str() != Some("complete") {
        return Err(fail("outcome.analysis is not complete"));
    }
    if !matches!(
        receipt["outcome"]["policy"].as_str(),
        Some("pass" | "fail" | "not_evaluated")
    ) {
        return Err(fail("outcome.policy is not a known value"));
    }
    let phases = receipt["phases"]
        .as_array()
        .ok_or_else(|| fail("phases is not an array"))?;
    let expected = expected_phases(command);
    if phases.len() != expected.len() {
        return Err(fail("phases do not match the command's phase order"));
    }
    let mut total = 0u64;
    for (phase, name) in phases.iter().zip(expected.iter()) {
        if phase["name"].as_str() != Some(name) {
            return Err(fail("phases do not match the command's phase order"));
        }
        total += phase["elapsed_ms"]
            .as_u64()
            .ok_or_else(|| fail("a phase elapsed_ms is not numeric"))?;
    }
    if receipt["total_ms"].as_u64() != Some(total) {
        return Err(fail("total_ms does not equal the phase sum"));
    }
    if receipt["cards"].as_u64().is_none() {
        return Err(fail("cards is not numeric"));
    }
    if receipt["output_bytes_total"].as_u64().is_none() {
        return Err(fail("output_bytes_total is not numeric"));
    }
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "matrix rows need identity, scope, args, and meta at one call site"
)]
fn cli_matrix_row(
    workspace_root: &Path,
    binary: &Path,
    work_dir: &Path,
    id: &str,
    command: &str,
    scope: &str,
    tail: &[String],
    runs: usize,
) -> Result<Value, String> {
    cli_matrix_row_with_meta(
        workspace_root,
        binary,
        work_dir,
        id,
        command,
        command,
        scope,
        tail,
        runs,
        json!({}),
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "matrix rows need identity, scope, args, and meta at one call site"
)]
fn cli_matrix_row_with_meta(
    workspace_root: &Path,
    binary: &Path,
    work_dir: &Path,
    id: &str,
    command: &str,
    receipt_command: &str,
    scope: &str,
    tail: &[String],
    runs: usize,
    meta: Value,
) -> Result<Value, String> {
    let mut run_values = Vec::new();
    for run in 0..runs {
        let latency_path = work_dir.join(format!("{id}-run{run}.latency.json"));
        let mut argv = vec![command.to_string()];
        argv.extend(tail.iter().cloned());
        argv.push("--latency-out".to_string());
        argv.push(latency_path.display().to_string());
        let started = Instant::now();
        let status = Command::new(binary)
            .args(&argv)
            .current_dir(workspace_root)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|err| format!("row {id} run {run} failed to start: {err}"))?;
        let wall_ms = started.elapsed().as_millis() as u64;
        if !status.success() {
            return Err(format!("row {id} run {run} exited with status {status}"));
        }
        let receipt_text = fs::read_to_string(&latency_path)
            .map_err(|err| format!("row {id} run {run} has no latency receipt: {err}"))?;
        let receipt: Value = serde_json::from_str(&receipt_text)
            .map_err(|err| format!("row {id} run {run} receipt is not JSON: {err}"))?;
        validate_receipt(id, run, receipt_command, scope, &receipt)?;
        let total_ms = receipt["total_ms"].as_u64().unwrap_or(u64::MAX);
        if wall_ms < total_ms {
            return Err(format!(
                "row {id} run {run} receipt rejected: wall clock {wall_ms}ms is below the phase sum {total_ms}ms"
            ));
        }
        run_values.push(json!({
            "run": run,
            "cache": if run == 0 { "first_process_run" } else { "repeat_process_run" },
            "wall_ms": wall_ms,
            "phases": receipt["phases"],
            "total_ms": receipt["total_ms"],
            "cards": receipt["cards"],
            "output_bytes_total": receipt["output_bytes_total"],
            "command": receipt["command"],
            "scope": receipt["scope"],
            "input_identity": receipt["input_identity"],
            "options_digest": receipt["options_digest"],
            "outcome": receipt["outcome"],
            "tool_commit": receipt["tool_commit"],
            "tool_binary_digest": receipt["tool_binary_digest"],
            "build_rustc": receipt["build_rustc"],
            "dirty_worktree": receipt["dirty_worktree"],
        }));
    }
    Ok(json!({
        "id": id,
        "kind": "cli",
        "meta": meta,
        "skipped": Value::Null,
        "runs": run_values,
    }))
}

fn skipped_row(id: &str, reason: &str) -> Value {
    skipped_row_with_kind(
        id,
        if id == "lsp-save-loop" { "lsp" } else { "cli" },
        reason,
    )
}

fn skipped_row_with_kind(id: &str, kind: &str, reason: &str) -> Value {
    json!({
        "id": id,
        "kind": kind,
        "meta": {},
        "skipped": reason,
        "runs": [],
    })
}

fn copy_dir(source: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|err| format!("create {} failed: {err}", dest.display()))?;
    let entries =
        fs::read_dir(source).map_err(|err| format!("read {} failed: {err}", source.display()))?;
    for entry in entries {
        let entry = entry.map_err(|err| format!("read entry failed: {err}"))?;
        let target = dest.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(|err| format!("stat entry failed: {err}"))?;
        if file_type.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), &target).map_err(|err| format!("copy entry failed: {err}"))?;
        }
    }
    Ok(())
}

fn lsp_save_loop_row(
    workspace_root: &Path,
    binary: &Path,
    work_dir: &Path,
    runs: usize,
) -> Result<Value, String> {
    let fixture_root = workspace_root.join(FIXTURE_LSP);
    if !fixture_root.is_dir() {
        return Ok(json!({
            "id": "lsp-save-loop",
            "kind": "lsp",
            "meta": {},
            "skipped": format!("fixture {} absent", fixture_root.display()),
            "runs": [],
        }));
    }
    let mut run_values = Vec::new();
    for run in 0..runs {
        let task_root = work_dir.join(format!("lsp-{run}"));
        if task_root.exists() {
            fs::remove_dir_all(&task_root)
                .map_err(|err| format!("clean {} failed: {err}", task_root.display()))?;
        }
        copy_dir(&fixture_root, &task_root)?;
        let sample = lsp_save_roundtrip(
            workspace_root,
            binary,
            &task_root,
            &fixture_root.join("src/lib.rs"),
        )?;
        run_values.push(json!({
            "run": run,
            "cache": if run == 0 { "first_process_run" } else { "repeat_process_run" },
            "init_to_diagnostics_ms": sample.init_ms,
            "stale_mark_ms": sample.stale_ms,
            "save_refresh_ms": sample.refresh_ms,
            "saved_source_sha256": sample.saved_sha256,
            "baseline_diagnostics": sample.baseline,
            "fresh_diagnostics": sample.fresh,
        }));
    }
    Ok(json!({
        "id": "lsp-save-loop",
        "kind": "lsp",
        "meta": {},
        "skipped": Value::Null,
        "runs": run_values,
    }))
}

struct LspSample {
    init_ms: u64,
    stale_ms: u64,
    refresh_ms: u64,
    saved_sha256: String,
    baseline: Value,
    fresh: Value,
}

fn lsp_save_roundtrip(
    workspace_root: &Path,
    binary: &Path,
    task_root: &Path,
    template_source: &Path,
) -> Result<LspSample, String> {
    let mut child = Command::new(binary)
        .arg("lsp")
        .current_dir(workspace_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| format!("failed to start live LSP server: {err}"))?;
    let outcome = lsp_roundtrip_inner(&mut child, task_root, template_source);
    let _ = child.kill();
    let _ = child.wait();
    outcome
}

/// One save-loop sample against a task-owned fixture copy.
///
/// The protocol binds every wait to the target URI and document version,
/// and separates the two post-edit events the server produces:
/// `didChange` marks diagnostics stale (an empty publication at the new
/// version), and `didSave` re-analyzes the physically written saved bytes
/// (a fresh publication at the same version). The fresh oracle additionally
/// requires a diagnostics set that strictly grows the baseline set, which
/// only a genuine reanalysis of the edited saved source can produce: the
/// edit appends a second unguarded site without shifting the original
/// site's lines, so every baseline diagnostic must appear verbatim.
/// A prior-generation publication is never accepted as completion.
fn lsp_roundtrip_inner(
    child: &mut Child,
    task_root: &Path,
    template_source: &Path,
) -> Result<LspSample, String> {
    let fixture_uri = lsp_smoke::file_uri(task_root)?;
    let source_path = task_root.join("src/lib.rs");
    let source_uri = lsp_smoke::file_uri(&source_path)?;
    let source_uri_text = source_uri.to_string();
    let template_text = fs::read_to_string(template_source)
        .map_err(|err| format!("read {} failed: {err}", template_source.display()))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "live LSP server stdin was not piped".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "live LSP server stdout was not piped".to_string())?;
    let (messages_tx, messages_rx) = mpsc::channel();
    thread::spawn(move || {
        let mut reader = std::io::BufReader::new(stdout);
        loop {
            let message = lsp_smoke::read_message(&mut reader);
            let done = message.is_err();
            if messages_tx.send(message).is_err() || done {
                break;
            }
        }
    });

    lsp_smoke::write_message(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "processId": null,
                "rootUri": fixture_uri,
                "capabilities": {},
                "workspaceFolders": [{
                    "uri": fixture_uri,
                    "name": "latency-baseline-fixture"
                }],
            },
        }),
    )?;
    lsp_smoke::wait_for_id(&messages_rx, 1)?;
    lsp_smoke::write_message(
        &mut stdin,
        &json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}),
    )?;

    // The server refreshes on `initialized` (not on open: refreshOnOpen
    // defaults to false), so the first publication is initialization
    // output. It carries no document version yet; bind on URI only.
    let initialized = Instant::now();
    let baseline = wait_publish(&messages_rx, &source_uri_text, None)?;
    let init_ms = initialized.elapsed().as_millis() as u64;

    lsp_smoke::write_message(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": source_uri,
                    "languageId": "rust",
                    "version": 1,
                    "text": template_text,
                },
            },
        }),
    )?;

    // Physically write the edited bytes before notifying: the product is
    // saved-source-first, so the save must re-analyze bytes on disk, not
    // overlay bytes alone.
    let edited_text = format!("{template_text}{LSP_PROBE}");
    fs::write(&source_path, &edited_text)
        .map_err(|err| format!("write {} failed: {err}", source_path.display()))?;
    let saved_sha256 = sha256_hex(edited_text.as_bytes());

    let changed = Instant::now();
    lsp_smoke::write_message(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": {"uri": source_uri, "version": 2},
                "contentChanges": [{"text": edited_text}],
            },
        }),
    )?;
    // didChange marks diagnostics stale: expect the empty publication at
    // the new version, and measure the stale transition as its own event.
    wait_publish(&messages_rx, &source_uri_text, Some((2, true)))?;
    let stale_ms = changed.elapsed().as_millis() as u64;

    let saved = Instant::now();
    lsp_smoke::write_message(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didSave",
            "params": {"textDocument": {"uri": source_uri}},
        }),
    )?;
    // didSave re-analyzes the saved bytes: expect a non-stale publication
    // at the new version whose diagnostics strictly grow the baseline.
    let fresh = wait_publish(&messages_rx, &source_uri_text, Some((2, false)))?;
    let refresh_ms = saved.elapsed().as_millis() as u64;
    require_superset(&baseline, &fresh)?;

    Ok(LspSample {
        init_ms,
        stale_ms,
        refresh_ms,
        saved_sha256,
        baseline,
        fresh,
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Wait for the next `publishDiagnostics` bound to `uri`, optionally to a
/// document `version` and a stale (empty-diagnostics) or fresh
/// (non-empty-diagnostics) shape. Publications for other URIs, other
/// versions, or the wrong shape are skipped, never accepted: a
/// prior-generation or stale publication must not pose as the awaited
/// event.
fn wait_publish(
    messages: &Receiver<Result<Value, String>>,
    uri: &str,
    version_shape: Option<(i32, bool)>,
) -> Result<Value, String> {
    let deadline = Instant::now() + LSP_TIMEOUT;
    loop {
        let message = lsp_smoke::receive_until(messages, deadline)?;
        if message.get("method").and_then(Value::as_str) != Some("textDocument/publishDiagnostics")
        {
            continue;
        }
        let params = &message["params"];
        if params["uri"].as_str() != Some(uri) {
            continue;
        }
        if let Some((version, stale)) = version_shape {
            if params["version"].as_i64() != Some(version as i64) {
                continue;
            }
            let diagnostics = params["diagnostics"].as_array();
            match (stale, diagnostics) {
                (true, Some(diagnostics)) if diagnostics.is_empty() => {}
                (false, Some(diagnostics)) if !diagnostics.is_empty() => {}
                _ => continue,
            }
        }
        let diagnostics = params["diagnostics"].clone();
        if diagnostics.is_null() {
            continue;
        }
        return Ok(diagnostics);
    }
}

/// Require the fresh diagnostics set to strictly grow the baseline set:
/// every baseline diagnostic must appear verbatim, plus at least one more.
/// The probe appends a second site without shifting the original lines, so
/// only a genuine reanalysis of the edited saved source passes.
fn require_superset(baseline: &Value, fresh: &Value) -> Result<(), String> {
    let baseline_items = baseline
        .as_array()
        .ok_or_else(|| "baseline diagnostics are not an array".to_string())?;
    let fresh_items = fresh
        .as_array()
        .ok_or_else(|| "fresh diagnostics are not an array".to_string())?;
    if fresh_items.len() <= baseline_items.len() {
        return Err(format!(
            "fresh diagnostics ({} items) do not grow the baseline set ({} items)",
            fresh_items.len(),
            baseline_items.len()
        ));
    }
    for item in baseline_items {
        if !fresh_items.contains(item) {
            return Err(
                "fresh diagnostics omit a baseline diagnostic; not a reanalysis of the edited source"
                    .to_string(),
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_receipt() -> Value {
        json!({
            "schema_version": "1.0",
            "command": "check",
            "scope": "diff",
            "input_identity": "diff-sha256:abc",
            "options_digest": "sha256:opts",
            "outcome": {"analysis": "complete", "policy": "pass"},
            "phases": [
                {"name": "input_resolution", "elapsed_ms": 10},
                {"name": "analyze", "elapsed_ms": 5},
                {"name": "projections", "elapsed_ms": 1},
                {"name": "artifact_writes", "elapsed_ms": 0},
                {"name": "policy_eval", "elapsed_ms": 0},
            ],
            "total_ms": 16,
            "cards": 2,
            "output_bytes_total": 100,
        })
    }

    fn expect_rejected(result: Result<(), String>, case: &str) -> Result<(), String> {
        match result {
            Ok(()) => Err(format!("expected rejection, accepted: {case}")),
            Err(_) => Ok(()),
        }
    }

    #[test]
    fn valid_check_receipt_passes_validation() -> Result<(), String> {
        validate_receipt("row", 0, "check", "diff", &check_receipt())?;
        Ok(())
    }

    #[test]
    fn wrong_command_or_scope_fails_closed() -> Result<(), String> {
        expect_rejected(
            validate_receipt("row", 0, "repo", "diff", &check_receipt()),
            "wrong command",
        )?;
        expect_rejected(
            validate_receipt("row", 0, "check", "repo", &check_receipt()),
            "wrong scope",
        )?;
        Ok(())
    }

    #[test]
    fn incomplete_outcome_or_mismatched_total_fails_closed() -> Result<(), String> {
        let mut receipt = check_receipt();
        receipt["outcome"]["analysis"] = json!("partial");
        expect_rejected(
            validate_receipt("row", 0, "check", "diff", &receipt),
            "partial outcome",
        )?;
        let mut receipt = check_receipt();
        receipt["total_ms"] = json!(15);
        expect_rejected(
            validate_receipt("row", 0, "check", "diff", &receipt),
            "mismatched total",
        )?;
        let mut receipt = check_receipt();
        receipt["phases"][1]["name"] = json!("artifact_writes");
        expect_rejected(
            validate_receipt("row", 0, "check", "diff", &receipt),
            "wrong phase order",
        )?;
        Ok(())
    }

    #[test]
    fn first_pr_receipt_needs_six_phases_in_order() -> Result<(), String> {
        let mut receipt = check_receipt();
        receipt["command"] = json!("first-pr");
        expect_rejected(
            validate_receipt("row", 0, "first-pr", "diff", &receipt),
            "five phases for first-pr",
        )?;
        receipt["phases"] = json!([
            {"name": "input_resolution", "elapsed_ms": 10},
            {"name": "analyze", "elapsed_ms": 5},
            {"name": "receipt_audit", "elapsed_ms": 2},
            {"name": "policy_eval", "elapsed_ms": 0},
            {"name": "projections", "elapsed_ms": 1},
            {"name": "artifact_writes", "elapsed_ms": 3},
        ]);
        receipt["total_ms"] = json!(21);
        validate_receipt("row", 0, "first-pr", "diff", &receipt)?;
        Ok(())
    }

    #[test]
    fn superset_oracle_accepts_growth_and_rejects_replacement() -> Result<(), String> {
        let baseline = json!([{"message": "guard missing", "range": "r1"}]);
        let grown = json!([
            {"message": "guard missing", "range": "r1"},
            {"message": "second site", "range": "r2"},
        ]);
        require_superset(&baseline, &grown)?;
        let replaced = json!([
            {"message": "guard missing", "range": "r1-changed"},
            {"message": "second site", "range": "r2"},
        ]);
        expect_rejected(require_superset(&baseline, &replaced), "replaced set")?;
        let same = json!([{"message": "guard missing", "range": "r1"}]);
        expect_rejected(require_superset(&baseline, &same), "unchanged set")?;
        let empty = json!([]);
        expect_rejected(require_superset(&empty, &empty), "empty set")?;
        Ok(())
    }

    #[test]
    fn skipped_lsp_row_keeps_lsp_kind() {
        let skipped = skipped_row("lsp-save-loop", "no fixture");
        assert_eq!(skipped["kind"], "lsp");
        assert_eq!(skipped_row("repo-pinned-memchr", "absent")["kind"], "cli");
    }

    #[test]
    fn pinned_commit_rejects_non_repositories() -> Result<(), String> {
        let dir = std::env::temp_dir().join(format!("latency-norepo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|err| format!("test setup failed: {err}"))?;
        let rejected = pinned_commit(&dir);
        assert!(
            rejected.is_err(),
            "a plain directory must not yield a commit"
        );
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn sha256_matches_nist_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
