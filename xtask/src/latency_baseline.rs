//! Standalone `latency-baseline` command: record cold/warm phase-latency
//! baselines for the CLI review paths (#2309 PR1).
//!
//! The matrix is fixed and small: two fixture `check` rows (S/M by card
//! count), one `first-pr` bundle row, one pinned-repo `repo` row, and one
//! scripted LSP save-loop row. Each row runs N times against a prebuilt
//! binary; run 0 is reported cold and the rest warm (no cache dropping is
//! performed — the definition is recorded in the receipt). The tool's own
//! `--latency-out` receipts supply the phase breakdowns; this runner adds
//! wall-clock totals, identities, and the aggregate file.
//!
//! The repo row is skipped with a recorded reason when the pinned work dir
//! is absent (e.g. offline); a skip is data, never a failure. Staged-diff
//! baselines wait for #2308 input identity; file-range waits for a file
//! input that does not exist yet — both are documented skips, not silent
//! omissions.
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
                    out = PathBuf::from(args.get(idx).ok_or("--out requires a value")?.to_string());
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
        &check_fixture_args(workspace_root, FIXTURE_S),
        args.runs,
    )?);
    rows.push(cli_matrix_row(
        workspace_root,
        &review_bin,
        &work_dir,
        "fixture-m-check",
        "check",
        &check_fixture_args(workspace_root, FIXTURE_M),
        args.runs,
    )?);
    rows.push(cli_matrix_row(
        workspace_root,
        &review_bin,
        &work_dir,
        "fixture-m-pr-bundle",
        "pr",
        &pr_fixture_args(workspace_root, FIXTURE_M),
        args.runs,
    )?);
    if args.skip_repo {
        rows.push(skipped_row("repo-pinned-memchr", "skipped by --skip-repo"));
    } else {
        let memchr = workspace_root.join(MEMCHR_WORK_DIR);
        if memchr.is_dir() {
            rows.push(cli_matrix_row(
                workspace_root,
                &review_bin,
                &work_dir,
                "repo-pinned-memchr",
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
            )?);
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
        rows.push(lsp_save_loop_row(workspace_root, &lsp_bin, args.runs)?);
    }

    let receipt = json!({
        "schema_version": RECEIPT_SCHEMA,
        "tool": "unsafe-review latency-baseline",
        "tool_version": cli_crate_version(workspace_root)?,
        "binary_profile": "debug",
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "runs_per_row": args.runs,
        "cold_definition": "run 0 of each row in this invocation; binaries prebuilt once before the matrix; no cache dropping performed",
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

fn cli_matrix_row(
    workspace_root: &Path,
    binary: &Path,
    work_dir: &Path,
    id: &str,
    command: &str,
    tail: &[String],
    runs: usize,
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
        run_values.push(json!({
            "run": run,
            "cache": if run == 0 { "cold" } else { "warm" },
            "wall_ms": wall_ms,
            "phases": receipt["phases"],
            "total_ms": receipt["total_ms"],
            "cards": receipt["cards"],
            "output_bytes_total": receipt["output_bytes_total"],
        }));
    }
    Ok(json!({
        "id": id,
        "kind": "cli",
        "skipped": Value::Null,
        "runs": run_values,
    }))
}

fn skipped_row(id: &str, reason: &str) -> Value {
    json!({
        "id": id,
        "kind": "cli",
        "skipped": reason,
        "runs": [],
    })
}

fn lsp_save_loop_row(workspace_root: &Path, binary: &Path, runs: usize) -> Result<Value, String> {
    let fixture_root = workspace_root.join(FIXTURE_LSP);
    if !fixture_root.is_dir() {
        return Ok(skipped_row(
            "lsp-save-loop",
            &format!("fixture {} absent", fixture_root.display()),
        ));
    }
    let mut run_values = Vec::new();
    for run in 0..runs {
        let (open_ms, edit_save_ms, diagnostics) =
            lsp_save_roundtrip(workspace_root, binary, &fixture_root)?;
        run_values.push(json!({
            "run": run,
            "cache": if run == 0 { "cold" } else { "warm" },
            "open_to_diagnostics_ms": open_ms,
            "edit_save_to_diagnostics_ms": edit_save_ms,
            "diagnostics": diagnostics,
        }));
    }
    Ok(json!({
        "id": "lsp-save-loop",
        "kind": "lsp",
        "skipped": Value::Null,
        "runs": run_values,
    }))
}

fn lsp_save_roundtrip(
    workspace_root: &Path,
    binary: &Path,
    fixture_root: &Path,
) -> Result<(u64, u64, u64), String> {
    let mut child = Command::new(binary)
        .arg("lsp")
        .current_dir(workspace_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| format!("failed to start live LSP server: {err}"))?;
    let outcome = lsp_roundtrip_inner(&mut child, fixture_root);
    let _ = child.kill();
    let _ = child.wait();
    outcome
}

fn lsp_roundtrip_inner(child: &mut Child, fixture_root: &Path) -> Result<(u64, u64, u64), String> {
    let fixture_uri = lsp_smoke::file_uri(fixture_root)?;
    let source_path = fixture_root.join("src/lib.rs");
    let source_uri = lsp_smoke::file_uri(&source_path)?;
    let source_text = fs::read_to_string(&source_path)
        .map_err(|err| format!("read {} failed: {err}", source_path.display()))?;
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

    let opened = Instant::now();
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
                    "text": source_text,
                },
            },
        }),
    )?;
    let first = wait_diagnostics(&messages_rx)?;
    let open_ms = opened.elapsed().as_millis() as u64;

    // The edit changes the overlay bytes, so the subsequent save must
    // re-analyze; attribution runs from the edit send to the next
    // publishDiagnostics, whichever notification triggers it.
    let edited = Instant::now();
    lsp_smoke::write_message(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": {"uri": source_uri, "version": 2},
                "contentChanges": [{"text": format!("{source_text}\n// latency-baseline save probe\n")}],
            },
        }),
    )?;
    lsp_smoke::write_message(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didSave",
            "params": {"textDocument": {"uri": source_uri}},
        }),
    )?;
    let second = wait_diagnostics(&messages_rx)?;
    let edit_save_ms = edited.elapsed().as_millis() as u64;
    Ok((open_ms, edit_save_ms, first.max(second)))
}

fn wait_diagnostics(messages: &Receiver<Result<Value, String>>) -> Result<u64, String> {
    let deadline = Instant::now() + LSP_TIMEOUT;
    loop {
        let message = lsp_smoke::receive_until(messages, deadline)?;
        if message.get("method").and_then(Value::as_str) == Some("textDocument/publishDiagnostics")
        {
            return message["params"]["diagnostics"]
                .as_array()
                .map(|diagnostics| diagnostics.len() as u64)
                .ok_or_else(|| "publishDiagnostics has no diagnostics array".to_string());
        }
    }
}
