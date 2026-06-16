//! Standalone `dogfood-exec` command: clone pinned repo-snapshot corpus targets, run
//! `unsafe-review repo`, and emit per-target diagnostics.
//!
//! This command is NOT in the `check-pr` bundle because it requires network access
//! and cloning real repositories (zerocopy alone scanned in ~282 s). Run it manually
//! or on a release/nightly cadence.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

const CORPUS_SOURCE: &str = "docs/dogfood/corpus.toml";
const DEFAULT_WORK_DIR: &str = "target/dogfood-work";
const DEFAULT_MAX_CARDS: u32 = 50;
/// Default per-target timeout in seconds.  Chosen large enough that a real
/// zerocopy scan (~282 s) plus a generous git-fetch never spuriously times out.
const DEFAULT_TIMEOUT_SECS: u64 = 900;
/// Poll interval for `try_wait` inside `run_with_timeout`.
const TIMEOUT_POLL_INTERVAL: Duration = Duration::from_millis(200);

/// Flags parsed from the command line for `dogfood-exec`.
pub(crate) struct DogfoodExecArgs {
    /// Only run the target with this id (None = run all repo-snapshot targets).
    pub(crate) target: Option<String>,
    /// Base directory for per-target clone work dirs.
    pub(crate) work_dir: PathBuf,
    /// `--max-cards N` forwarded to `unsafe-review repo`.
    pub(crate) max_cards: u32,
    /// Exit non-zero if any target produces `run_failed` or `schema_failed`.
    pub(crate) strict: bool,
    /// Remove the per-target work dir before cloning.
    pub(crate) clean: bool,
    /// Per-target wall-clock timeout in seconds.  A single value covers both
    /// git operations and the unsafe-review scan.  Default: 900 s.
    pub(crate) timeout_secs: u64,
}

impl DogfoodExecArgs {
    /// Parse `dogfood-exec [--target <id>] [--work-dir <path>] [--max-cards <N>]
    ///                      [--strict] [--clean] [--timeout <secs>]`
    /// from the xtask args slice.
    ///
    /// `args[0]` is the xtask binary name and `args[1]` is "dogfood-exec".
    pub(crate) fn parse(args: &[String]) -> Result<Self, String> {
        let mut target: Option<String> = None;
        let mut work_dir: Option<PathBuf> = None;
        let mut max_cards: u32 = DEFAULT_MAX_CARDS;
        let mut strict = false;
        let mut clean = false;
        let mut timeout_secs: u64 = DEFAULT_TIMEOUT_SECS;

        let mut i = 2usize;
        while i < args.len() {
            match args[i].as_str() {
                "--target" => {
                    i += 1;
                    let val = args.get(i).ok_or("--target requires a value")?;
                    target = Some(val.clone());
                }
                "--work-dir" => {
                    i += 1;
                    let val = args.get(i).ok_or("--work-dir requires a value")?;
                    work_dir = Some(PathBuf::from(val));
                }
                "--max-cards" => {
                    i += 1;
                    let val = args.get(i).ok_or("--max-cards requires a value")?;
                    max_cards = val
                        .parse::<u32>()
                        .map_err(|e| format!("--max-cards: {e}"))?;
                }
                "--strict" => {
                    strict = true;
                }
                "--clean" => {
                    clean = true;
                }
                "--timeout" => {
                    i += 1;
                    let val = args.get(i).ok_or("--timeout requires a value")?;
                    timeout_secs = val.parse::<u64>().map_err(|e| format!("--timeout: {e}"))?;
                    if timeout_secs == 0 {
                        return Err("--timeout must be greater than 0".to_string());
                    }
                }
                other => {
                    return Err(format!("`dogfood-exec` does not accept argument `{other}`"));
                }
            }
            i += 1;
        }

        Ok(Self {
            target,
            work_dir: work_dir.unwrap_or_else(|| PathBuf::from(DEFAULT_WORK_DIR)),
            max_cards,
            strict,
            clean,
            timeout_secs,
        })
    }
}

/// A parsed repo-snapshot target from corpus.toml.
pub(crate) struct TargetSpec {
    pub(crate) id: String,
    pub(crate) repository: String,
    pub(crate) commit: String,
}

/// Diagnostics recorded after a successful scan.
pub(crate) struct ScanDiagnostics {
    /// Number of cards reported in `summary.cards`.
    pub(crate) summary_card_count: u64,
    /// Length of the `cards` array.
    pub(crate) cards_array_len: u64,
    /// Percentage of cards with `operation_family == "unknown"` (0–100).
    pub(crate) unknown_pct: f64,
    /// Percentage of cards with `operation_family == "target_feature"` (0–100).
    pub(crate) target_feature_pct: f64,
    /// `summary.unsafe_sites` value (0 if absent).
    pub(crate) unsafe_sites: u64,
    /// `summary.rust_files` value (0 if absent).
    pub(crate) rust_files: u64,
}

/// Per-target run outcome.
pub(crate) enum TargetStatus {
    Ok(ScanDiagnostics),
    CloneFailed(String),
    RunFailed(i32),
    SchemaFailed(String),
    /// A subprocess exceeded the per-target wall-clock deadline.  The label
    /// names the operation that timed out (e.g. "git fetch", "unsafe-review").
    Timeout(String),
}

/// Result for one target.
pub(crate) struct TargetResult {
    pub(crate) id: String,
    pub(crate) repository: String,
    pub(crate) commit: String,
    pub(crate) status: TargetStatus,
}

/// Select only `kind == "repo-snapshot"` targets from a parsed corpus.toml.
///
/// This is a pure, IO-free function — testable without network.
pub(crate) fn select_repo_snapshot_targets(
    corpus: &toml::Value,
) -> Result<Vec<TargetSpec>, String> {
    let targets_arr = match corpus.get("targets") {
        Some(toml::Value::Array(arr)) => arr,
        _ => return Err(format!("{CORPUS_SOURCE}: missing [[targets]] array")),
    };

    let mut result = Vec::new();
    for (idx, entry) in targets_arr.iter().enumerate() {
        let table = entry
            .as_table()
            .ok_or_else(|| format!("{CORPUS_SOURCE}: targets[{idx}] is not a table"))?;

        let kind = table.get("kind").and_then(|v| v.as_str()).unwrap_or("");

        if kind != "repo-snapshot" {
            continue;
        }

        let id = table
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("{CORPUS_SOURCE}: targets[{idx}] missing id"))?
            .to_string();

        let repository = table
            .get("repository")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("{CORPUS_SOURCE}: targets[{idx}] ({id}) missing repository"))?
            .to_string();

        let commit = table
            .get("commit")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("{CORPUS_SOURCE}: targets[{idx}] ({id}) missing commit"))?
            .to_string();

        result.push(TargetSpec {
            id,
            repository,
            commit,
        });
    }

    Ok(result)
}

/// Evaluate a parsed scan JSON value and return diagnostics.
///
/// Returns `Err` if the required invariants (`cards` array and `summary` object) are absent.
/// This is a pure, IO-free function — testable without network.
pub(crate) fn evaluate_scan_json(value: &serde_json::Value) -> Result<ScanDiagnostics, String> {
    // `cards` must be an array.
    let cards_arr = value
        .get("cards")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "scan JSON missing required `cards` array".to_string())?;

    // `summary` must be an object.
    let summary = value
        .get("summary")
        .and_then(|v| v.as_object())
        .ok_or_else(|| "scan JSON missing required `summary` object".to_string())?;

    let cards_array_len = cards_arr.len() as u64;

    // Count operation_family occurrences.
    let mut unknown_count = 0u64;
    let mut target_feature_count = 0u64;
    for card in cards_arr {
        match card.get("operation_family").and_then(|v| v.as_str()) {
            Some("unknown") => unknown_count += 1,
            Some("target_feature") => target_feature_count += 1,
            _ => {}
        }
    }

    let (unknown_pct, target_feature_pct) = if cards_array_len > 0 {
        (
            (unknown_count as f64 / cards_array_len as f64) * 100.0,
            (target_feature_count as f64 / cards_array_len as f64) * 100.0,
        )
    } else {
        (0.0, 0.0)
    };

    let summary_card_count = summary.get("cards").and_then(|v| v.as_u64()).unwrap_or(0);

    let unsafe_sites = summary
        .get("unsafe_sites")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let rust_files = summary
        .get("rust_files")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    Ok(ScanDiagnostics {
        summary_card_count,
        cards_array_len,
        unknown_pct,
        target_feature_pct,
        unsafe_sites,
        rust_files,
    })
}

/// Run the dogfood-exec command end-to-end.
pub(crate) fn run(args: &DogfoodExecArgs) -> Result<(), String> {
    // Load and parse corpus.toml.
    let corpus_text = fs::read_to_string(Path::new(CORPUS_SOURCE))
        .map_err(|err| format!("read {CORPUS_SOURCE} failed: {err}"))?;
    let corpus: toml::Value = toml::from_str(&corpus_text)
        .map_err(|err| format!("parse {CORPUS_SOURCE} failed: {err}"))?;

    let all_targets = select_repo_snapshot_targets(&corpus)?;

    // Filter by --target if specified.
    let targets: Vec<TargetSpec> = if let Some(ref filter_id) = args.target {
        let filtered: Vec<TargetSpec> = all_targets
            .into_iter()
            .filter(|t| &t.id == filter_id)
            .collect();
        if filtered.is_empty() {
            return Err(format!(
                "no repo-snapshot target with id `{filter_id}` in {CORPUS_SOURCE}"
            ));
        }
        filtered
    } else {
        all_targets
    };

    let mut results: Vec<TargetResult> = Vec::new();

    for target in &targets {
        let result = run_target(target, args);
        results.push(result);
    }

    // Emit per-target summary.
    let mut ok_count = 0u32;
    let mut failed_count = 0u32;
    let mut has_hard_failure = false;

    println!("dogfood-exec results:");
    println!(
        "{:<30} {:<45} {:<15} diagnostics",
        "id", "repository@commit", "status"
    );

    for r in &results {
        let commit_short = &r.commit[..r.commit.len().min(12)];
        let repo_at_commit = format!("{}@{}", r.repository, commit_short);

        match &r.status {
            TargetStatus::Ok(diag) => {
                ok_count += 1;
                println!(
                    "{:<30} {:<45} {:<15} cards={} (summary={}) unsafe_sites={} rust_files={} unknown={:.1}% target_feature={:.1}%",
                    r.id,
                    repo_at_commit,
                    "ok",
                    diag.cards_array_len,
                    diag.summary_card_count,
                    diag.unsafe_sites,
                    diag.rust_files,
                    diag.unknown_pct,
                    diag.target_feature_pct,
                );
            }
            TargetStatus::CloneFailed(err) => {
                failed_count += 1;
                println!(
                    "{:<30} {:<45} {:<15} clone_failed: {}",
                    r.id, repo_at_commit, "clone_failed", err,
                );
                // clone_failed is NEVER a hard failure even under --strict
            }
            TargetStatus::RunFailed(code) => {
                failed_count += 1;
                has_hard_failure = true;
                println!(
                    "{:<30} {:<45} {:<15} exit_code={}",
                    r.id, repo_at_commit, "run_failed", code,
                );
            }
            TargetStatus::SchemaFailed(err) => {
                failed_count += 1;
                has_hard_failure = true;
                println!(
                    "{:<30} {:<45} {:<15} {}",
                    r.id, repo_at_commit, "schema_failed", err,
                );
            }
            TargetStatus::Timeout(label) => {
                failed_count += 1;
                // timeout is a hard failure under --strict (like run_failed)
                has_hard_failure = true;
                println!(
                    "{:<30} {:<45} {:<15} {}",
                    r.id, repo_at_commit, "timeout", label,
                );
            }
        }
    }

    println!(
        "\ndogfood-exec: {} ok / {} failed (total {})",
        ok_count,
        failed_count,
        results.len(),
    );

    if args.strict && has_hard_failure {
        return Err(
            "dogfood-exec --strict: one or more targets produced run_failed, schema_failed, or timeout"
                .to_string(),
        );
    }

    Ok(())
}

/// Clone, scan, and evaluate a single repo-snapshot target.
fn run_target(target: &TargetSpec, args: &DogfoodExecArgs) -> TargetResult {
    let work_dir = args.work_dir.join(&target.id);
    let artifact_path = args
        .work_dir
        .join(format!("{}.unsafe-review.json", target.id));

    // Establish a single per-target deadline shared by git operations and the scan.
    let deadline = Instant::now() + Duration::from_secs(args.timeout_secs);

    // --clean: remove the work dir before cloning.
    if args.clean {
        let clean_result = remove_dir_if_exists(&work_dir);
        if let Err(err) = clean_result {
            return TargetResult {
                id: target.id.clone(),
                repository: target.repository.clone(),
                commit: target.commit.clone(),
                status: TargetStatus::CloneFailed(format!(
                    "failed to clean work dir {}: {err}",
                    work_dir.display()
                )),
            };
        }
    }

    // Step 1: clone at pinned commit.
    match clone_at_commit(&work_dir, &target.repository, &target.commit, deadline) {
        Ok(()) => {}
        Err(err) if err.contains("timed out") => {
            return TargetResult {
                id: target.id.clone(),
                repository: target.repository.clone(),
                commit: target.commit.clone(),
                status: TargetStatus::Timeout(err),
            };
        }
        Err(err) => {
            return TargetResult {
                id: target.id.clone(),
                repository: target.repository.clone(),
                commit: target.commit.clone(),
                status: TargetStatus::CloneFailed(err),
            };
        }
    }

    // Step 2: run unsafe-review repo.
    let scan_run = match run_unsafe_review(&work_dir, &artifact_path, args.max_cards, deadline) {
        Ok(run) => run,
        Err(err) => {
            eprintln!("dogfood-exec: spawn error for {}: {err}", target.id);
            return TargetResult {
                id: target.id.clone(),
                repository: target.repository.clone(),
                commit: target.commit.clone(),
                // Treat spawn failure as exit code -1.
                status: TargetStatus::RunFailed(-1),
            };
        }
    };

    let exit_code = match scan_run {
        ScanRun::TimedOut => {
            return TargetResult {
                id: target.id.clone(),
                repository: target.repository.clone(),
                commit: target.commit.clone(),
                status: TargetStatus::Timeout("unsafe-review scan timed out".to_string()),
            };
        }
        ScanRun::Exited(code) => code,
    };

    // Exit code 0 (clean/advisory) or 1 (policy violation) are both acceptable.
    // Exit code 2 is a tool error.
    if exit_code == 2 {
        return TargetResult {
            id: target.id.clone(),
            repository: target.repository.clone(),
            commit: target.commit.clone(),
            status: TargetStatus::RunFailed(exit_code),
        };
    }

    // Step 3: read and evaluate the JSON artifact.
    let text = match fs::read_to_string(&artifact_path) {
        Ok(t) => t,
        Err(err) => {
            return TargetResult {
                id: target.id.clone(),
                repository: target.repository.clone(),
                commit: target.commit.clone(),
                status: TargetStatus::SchemaFailed(format!(
                    "could not read artifact {}: {err}",
                    artifact_path.display()
                )),
            };
        }
    };

    let value: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(err) => {
            return TargetResult {
                id: target.id.clone(),
                repository: target.repository.clone(),
                commit: target.commit.clone(),
                status: TargetStatus::SchemaFailed(format!("artifact is not valid JSON: {err}")),
            };
        }
    };

    match evaluate_scan_json(&value) {
        Ok(diag) => TargetResult {
            id: target.id.clone(),
            repository: target.repository.clone(),
            commit: target.commit.clone(),
            status: TargetStatus::Ok(diag),
        },
        Err(err) => TargetResult {
            id: target.id.clone(),
            repository: target.repository.clone(),
            commit: target.commit.clone(),
            status: TargetStatus::SchemaFailed(err),
        },
    }
}

/// Clone the given GitHub repository at the given commit SHA into `work_dir`.
///
/// Strategy:
///   1. `git init <work_dir>`
///   2. `git -C <work_dir> fetch --depth 1 https://github.com/<repo> <commit>`
///   3. `git -C <work_dir> checkout FETCH_HEAD`
///
/// If step 2 fails (some hosts reject fetch-by-SHA), falls back to:
///   2b. `git clone https://github.com/<repo> <work_dir>`
///   3b. `git -C <work_dir> checkout <commit>`
///
/// If `work_dir` already exists and is non-empty, the clone steps are skipped
/// (the caller is responsible for --clean if a fresh clone is wanted).
///
/// Every git subprocess is bounded by `deadline`.  A timeout error propagates
/// as an `Err` containing "timed out" so the caller can record it as
/// `TargetStatus::Timeout`.
fn clone_at_commit(
    work_dir: &Path,
    repository: &str,
    commit: &str,
    deadline: Instant,
) -> Result<(), String> {
    // If the work dir already exists and has a .git directory, skip cloning.
    if work_dir.join(".git").exists() {
        return Ok(());
    }

    let url = format!("https://github.com/{repository}");
    let work_dir_str = work_dir
        .to_str()
        .ok_or_else(|| format!("work dir path is not valid UTF-8: {}", work_dir.display()))?;

    // git init
    run_git(&["init", work_dir_str], "git init", deadline)?;

    // Attempt shallow fetch by SHA.
    let fetch_result = run_git(
        &["-C", work_dir_str, "fetch", "--depth", "1", &url, commit],
        "git fetch --depth 1",
        deadline,
    );

    if fetch_result.is_ok() {
        // Checkout FETCH_HEAD.
        run_git(
            &["-C", work_dir_str, "checkout", "FETCH_HEAD"],
            "git checkout FETCH_HEAD",
            deadline,
        )
    } else {
        // Fallback: full clone then checkout.
        // Remove the half-initialised directory first so `git clone` can write into it.
        remove_dir_if_exists(work_dir)
            .map_err(|e| format!("cleanup before fallback clone: {e}"))?;

        run_git(&["clone", &url, work_dir_str], "git clone", deadline)?;
        run_git(
            &["-C", work_dir_str, "checkout", commit],
            "git checkout <commit>",
            deadline,
        )
    }
}

/// Run a `git` command with a wall-clock deadline, returning `Ok(())` on
/// success, `Err` on failure, or a timeout error if the deadline elapses.
///
/// Stdout and stderr are inherited so git progress is visible.
fn run_git(args: &[&str], label: &str, deadline: Instant) -> Result<(), String> {
    let mut cmd = Command::new("git");
    cmd.args(args);
    match run_with_timeout(cmd, deadline, label)? {
        TimedRun::Exited(0) => Ok(()),
        TimedRun::Exited(code) => Err(format!("{label} failed (exit {code})")),
        TimedRun::TimedOut => Err(format!("{label} timed out")),
    }
}

/// Outcome returned by [`run_unsafe_review`].
enum ScanRun {
    /// The scan completed; carries the exit code.
    Exited(i32),
    /// The scan was killed because the deadline elapsed.
    TimedOut,
}

/// Run `cargo run --locked -p unsafe-review -- repo --root <work_dir> --format json
/// --max-cards <N> --out <artifact_path>` with a wall-clock deadline.
///
/// Returns `ScanRun::Exited(code)` when the process exits, or
/// `ScanRun::TimedOut` if the deadline elapses (the child is killed and reaped
/// before returning).  `Err` is returned only if the process could not be
/// spawned or if kill/wait after timeout encounters an OS error.
fn run_unsafe_review(
    work_dir: &Path,
    artifact_path: &Path,
    max_cards: u32,
    deadline: Instant,
) -> Result<ScanRun, String> {
    let work_dir_str = work_dir
        .to_str()
        .ok_or_else(|| format!("work dir path is not valid UTF-8: {}", work_dir.display()))?;
    let artifact_str = artifact_path.to_str().ok_or_else(|| {
        format!(
            "artifact path is not valid UTF-8: {}",
            artifact_path.display()
        )
    })?;
    let max_cards_str = max_cards.to_string();

    let mut cmd = Command::new("cargo");
    cmd.args([
        "run",
        "--locked",
        "-p",
        "unsafe-review",
        "--",
        "repo",
        "--root",
        work_dir_str,
        "--format",
        "json",
        "--max-cards",
        &max_cards_str,
        "--out",
        artifact_str,
    ]);
    let result = run_with_timeout(cmd, deadline, "cargo run unsafe-review")?;

    Ok(match result {
        TimedRun::Exited(code) => ScanRun::Exited(code),
        TimedRun::TimedOut => ScanRun::TimedOut,
    })
}

/// Remove `path` if it exists; ignore NotFound; propagate other errors.
fn remove_dir_if_exists(path: &Path) -> Result<(), String> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(format!("remove {} failed: {err}", path.display())),
    }
}

/// Outcome of [`run_with_timeout`].
enum TimedRun {
    /// The process exited within the deadline; carries the exit code.
    Exited(i32),
    /// The deadline elapsed; the process was killed.
    TimedOut,
}

/// Spawn `cmd`, then poll [`std::process::Child::try_wait`] every
/// [`TIMEOUT_POLL_INTERVAL`] until either the child exits or `deadline` passes.
/// On timeout the child is killed and reaped before returning [`TimedRun::TimedOut`].
///
/// Stdout and stderr are inherited (i.e. the child writes directly to the
/// terminal) so that progress is visible for long-running operations.  On the
/// timeout path we kill-then-wait so the child's resources are reclaimed before
/// we return — no zombie processes.
///
/// Returns `Err` only if the process could not be spawned or if kill/wait after
/// timeout encounters an OS error.
fn run_with_timeout(mut cmd: Command, deadline: Instant, label: &str) -> Result<TimedRun, String> {
    let mut child = cmd
        .spawn()
        .map_err(|err| format!("{label} failed to spawn: {err}"))?;

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return Ok(TimedRun::Exited(status.code().unwrap_or(-1)));
            }
            Ok(None) => {
                // Child still running — check deadline before sleeping.
                if Instant::now() >= deadline {
                    // Best-effort kill; propagate OS errors so the caller can
                    // surface them rather than silently swallowing them.
                    child
                        .kill()
                        .map_err(|err| format!("{label} kill() failed: {err}"))?;
                    // Reap to avoid a zombie.
                    child
                        .wait()
                        .map_err(|err| format!("{label} wait() after kill failed: {err}"))?;
                    return Ok(TimedRun::TimedOut);
                }
                std::thread::sleep(TIMEOUT_POLL_INTERVAL);
            }
            Err(err) => {
                return Err(format!("{label} try_wait() failed: {err}"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Valid scan JSON with a mix of operation families produces correct diagnostics.
    #[test]
    fn dogfood_exec_evaluate_scan_json_valid() -> Result<(), String> {
        let value = serde_json::json!({
            "schema_version": "0.1",
            "summary": {
                "cards": 3u64,
                "unsafe_sites": 10u64,
                "rust_files": 5u64
            },
            "cards": [
                {"operation_family": "unknown"},
                {"operation_family": "target_feature"},
                {"operation_family": "raw_pointer_read"},
            ]
        });

        let diag = evaluate_scan_json(&value)?;

        assert_eq!(diag.cards_array_len, 3);
        assert_eq!(diag.summary_card_count, 3);
        assert_eq!(diag.unsafe_sites, 10);
        assert_eq!(diag.rust_files, 5);
        // 1 out of 3 → 33.33...%
        assert!(
            (diag.unknown_pct - 33.333).abs() < 0.01,
            "unknown_pct={}",
            diag.unknown_pct
        );
        assert!(
            (diag.target_feature_pct - 33.333).abs() < 0.01,
            "target_feature_pct={}",
            diag.target_feature_pct
        );
        Ok(())
    }

    /// Empty cards array still produces 0.0 percentages without dividing by zero.
    #[test]
    fn dogfood_exec_evaluate_scan_json_empty_cards() -> Result<(), String> {
        let value = serde_json::json!({
            "summary": {
                "cards": 0u64,
                "unsafe_sites": 0u64,
                "rust_files": 2u64
            },
            "cards": []
        });

        let diag = evaluate_scan_json(&value)?;

        assert_eq!(diag.cards_array_len, 0);
        assert_eq!(diag.unknown_pct, 0.0);
        assert_eq!(diag.target_feature_pct, 0.0);
        Ok(())
    }

    /// Missing `cards` key returns an error.
    #[test]
    fn dogfood_exec_evaluate_scan_json_missing_cards_is_err() {
        let value = serde_json::json!({
            "summary": { "cards": 0u64 }
        });

        let result = evaluate_scan_json(&value);
        assert!(result.is_err(), "expected Err for missing cards");
        let err = result.err().unwrap_or_default();
        assert!(
            err.contains("cards"),
            "error message should mention 'cards': {err}"
        );
    }

    /// Missing `summary` key returns an error.
    #[test]
    fn dogfood_exec_evaluate_scan_json_missing_summary_is_err() {
        let value = serde_json::json!({
            "cards": [{"operation_family": "unknown"}]
        });

        let result = evaluate_scan_json(&value);
        assert!(result.is_err(), "expected Err for missing summary");
        let err = result.err().unwrap_or_default();
        assert!(
            err.contains("summary"),
            "error message should mention 'summary': {err}"
        );
    }

    /// A corpus with mixed kinds returns only repo-snapshot targets.
    #[test]
    fn dogfood_exec_select_repo_snapshot_filters_by_kind() -> Result<(), String> {
        let corpus: toml::Value = r#"
schema_version = "0.1"

[[targets]]
id = "fixture-one"
crate = "foo"
kind = "fixture-control"
status = "active"
purpose = "test fixture control"
command = "cargo run"
artifact_status = "local_untracked"
artifacts = []

[[targets]]
id = "pr-one"
crate = "bar"
repository = "owner/bar"
kind = "pr-diff"
status = "active"
purpose = "test pr diff target"
command = "cargo run"
artifact_status = "local_untracked"
artifacts = []

[[targets]]
id = "repo-one"
crate = "baz"
repository = "owner/baz"
kind = "repo-snapshot"
status = "active"
commit = "abcdef1234567890abcdef1234567890abcdef12"
root = "target/dogfood-work/baz"
purpose = "test repo snapshot target"
command = "cargo run"
artifact_status = "local_untracked"
artifacts = []

[[targets]]
id = "repo-two"
crate = "qux"
repository = "owner/qux"
kind = "repo-snapshot"
status = "active"
commit = "0011223344556677889900112233445566778899"
root = "target/dogfood-work/qux"
purpose = "another repo snapshot"
command = "cargo run"
artifact_status = "local_untracked"
artifacts = []
"#
        .parse::<toml::Table>()
        .map_err(|e| e.to_string())
        .map(toml::Value::Table)?;

        let targets = select_repo_snapshot_targets(&corpus)?;

        assert_eq!(targets.len(), 2, "expected exactly 2 repo-snapshot targets");
        assert_eq!(targets[0].id, "repo-one");
        assert_eq!(targets[0].repository, "owner/baz");
        assert_eq!(
            targets[0].commit,
            "abcdef1234567890abcdef1234567890abcdef12"
        );
        assert_eq!(targets[1].id, "repo-two");
        Ok(())
    }

    /// A corpus with no targets array returns an error.
    #[test]
    fn dogfood_exec_select_repo_snapshot_missing_targets_array_is_err() {
        let corpus: toml::Value = "schema_version = \"0.1\""
            .parse::<toml::Table>()
            .map(toml::Value::Table)
            .unwrap_or(toml::Value::Table(toml::map::Map::new()));

        let result = select_repo_snapshot_targets(&corpus);
        assert!(result.is_err(), "expected Err for missing targets array");
    }

    /// Arg parser accepts all flags correctly.
    #[test]
    fn dogfood_exec_args_parse_all_flags() -> Result<(), String> {
        let args: Vec<String> = vec![
            "xtask".to_string(),
            "dogfood-exec".to_string(),
            "--target".to_string(),
            "smallvec-capped".to_string(),
            "--work-dir".to_string(),
            "target/my-work".to_string(),
            "--max-cards".to_string(),
            "20".to_string(),
            "--strict".to_string(),
            "--clean".to_string(),
        ];

        let parsed = DogfoodExecArgs::parse(&args)?;

        assert_eq!(parsed.target.as_deref(), Some("smallvec-capped"));
        assert_eq!(parsed.work_dir, PathBuf::from("target/my-work"));
        assert_eq!(parsed.max_cards, 20);
        assert!(parsed.strict);
        assert!(parsed.clean);
        Ok(())
    }

    /// Arg parser uses defaults when no flags are given.
    #[test]
    fn dogfood_exec_args_parse_defaults() -> Result<(), String> {
        let args: Vec<String> = vec!["xtask".to_string(), "dogfood-exec".to_string()];

        let parsed = DogfoodExecArgs::parse(&args)?;

        assert!(parsed.target.is_none());
        assert_eq!(parsed.work_dir, PathBuf::from(DEFAULT_WORK_DIR));
        assert_eq!(parsed.max_cards, DEFAULT_MAX_CARDS);
        assert!(!parsed.strict);
        assert!(!parsed.clean);
        // Default timeout is the documented constant.
        assert_eq!(parsed.timeout_secs, DEFAULT_TIMEOUT_SECS);
        Ok(())
    }

    /// Unknown flag returns an error.
    #[test]
    fn dogfood_exec_args_parse_unknown_flag_is_err() {
        let args: Vec<String> = vec![
            "xtask".to_string(),
            "dogfood-exec".to_string(),
            "--unknown".to_string(),
        ];

        let result = DogfoodExecArgs::parse(&args);
        assert!(result.is_err(), "expected Err for unknown flag");
        let err = result.err().unwrap_or_default();
        assert!(err.contains("--unknown"), "{err}");
    }

    /// `--timeout` accepts a valid positive integer.
    #[test]
    fn dogfood_exec_args_parse_timeout_valid() -> Result<(), String> {
        let args: Vec<String> = vec![
            "xtask".to_string(),
            "dogfood-exec".to_string(),
            "--timeout".to_string(),
            "600".to_string(),
        ];

        let parsed = DogfoodExecArgs::parse(&args)?;
        assert_eq!(parsed.timeout_secs, 600);
        Ok(())
    }

    /// `--timeout 0` is rejected (must be > 0).
    #[test]
    fn dogfood_exec_args_parse_timeout_zero_is_err() {
        let args: Vec<String> = vec![
            "xtask".to_string(),
            "dogfood-exec".to_string(),
            "--timeout".to_string(),
            "0".to_string(),
        ];

        let result = DogfoodExecArgs::parse(&args);
        assert!(result.is_err(), "expected Err for --timeout 0");
        let err = result.err().unwrap_or_default();
        assert!(
            err.contains("greater than 0"),
            "error should mention zero bound: {err}"
        );
    }

    /// `--timeout` with a non-numeric value returns a parse error.
    #[test]
    fn dogfood_exec_args_parse_timeout_non_numeric_is_err() {
        let args: Vec<String> = vec![
            "xtask".to_string(),
            "dogfood-exec".to_string(),
            "--timeout".to_string(),
            "abc".to_string(),
        ];

        let result = DogfoodExecArgs::parse(&args);
        assert!(result.is_err(), "expected Err for non-numeric --timeout");
        let err = result.err().unwrap_or_default();
        assert!(
            err.contains("--timeout"),
            "error should mention --timeout: {err}"
        );
    }

    /// `--timeout` without a following value returns an error.
    #[test]
    fn dogfood_exec_args_parse_timeout_missing_value_is_err() {
        let args: Vec<String> = vec![
            "xtask".to_string(),
            "dogfood-exec".to_string(),
            "--timeout".to_string(),
        ];

        let result = DogfoodExecArgs::parse(&args);
        assert!(result.is_err(), "expected Err for missing --timeout value");
        let err = result.err().unwrap_or_default();
        assert!(
            err.contains("requires a value"),
            "error should say 'requires a value': {err}"
        );
    }

    /// `--timeout` combined with other flags is parsed correctly.
    #[test]
    fn dogfood_exec_args_parse_timeout_with_other_flags() -> Result<(), String> {
        let args: Vec<String> = vec![
            "xtask".to_string(),
            "dogfood-exec".to_string(),
            "--target".to_string(),
            "zerocopy-stable".to_string(),
            "--timeout".to_string(),
            "300".to_string(),
            "--strict".to_string(),
        ];

        let parsed = DogfoodExecArgs::parse(&args)?;
        assert_eq!(parsed.target.as_deref(), Some("zerocopy-stable"));
        assert_eq!(parsed.timeout_secs, 300);
        assert!(parsed.strict);
        Ok(())
    }
}
