//! `check-real-pr-corpus` gate: validates the `policy/pr-corpus.toml` ledger by
//! running `unsafe-review check` and `unsafe-review first-pr` against each
//! declared synthetic-fixture case and asserting the observed movement counts
//! and comment-plan selection counts match the declared expectations.
//!
//! This gate is deterministic (integer count assertions over pinned fixture
//! inputs) and is safe to include in `check-pr`. Assertions are order-independent
//! integer counts, never byte-golden diffs or ordered lists.
//!
//! Trust boundary: movement counts are advisory diagnostics, not memory-safety
//! proof, UB-free claims, Miri-clean claims, site-execution results, or
//! calibrated precision/recall measurements.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const PR_CORPUS_LEDGER: &str = "policy/pr-corpus.toml";

/// A parsed `[[pr]]` case from the ledger.
struct PrCase {
    id: String,
    root: String,
    diff: String,
    expected: ExpectedCounts,
    no_new_debt_exit_code: Option<i32>,
}

/// Expected movement + comment-plan counts for one case.
struct ExpectedCounts {
    new_gaps: u64,
    worsened_gaps: u64,
    improved_gaps: u64,
    resolved_gaps: u64,
    inherited_gaps: u64,
    selected_count: u64,
    not_selected_count: u64,
}

/// Run `check-real-pr-corpus`: parse the ledger, invoke the tool for each case,
/// and assert each observed count matches the declared expected value.
pub(crate) fn check() -> Result<(), String> {
    let ledger = parse_ledger(Path::new(PR_CORPUS_LEDGER))?;
    if ledger.is_empty() {
        return Err(format!(
            "{PR_CORPUS_LEDGER} must declare at least one [[pr]] case"
        ));
    }

    let mut case_count = 0usize;
    let mut errors: Vec<String> = Vec::new();

    for case in &ledger {
        case_count += 1;
        if let Err(err) = run_case(case) {
            errors.push(err);
        }
    }

    if !errors.is_empty() {
        return Err(format!(
            "check-real-pr-corpus: {} case(s) failed:\n{}",
            errors.len(),
            errors.join("\n")
        ));
    }

    println!("check-real-pr-corpus: ok ({case_count} case(s) passed)");
    Ok(())
}

/// Run one `[[pr]]` case: advisory check + comment-plan validation + optional
/// no-new-debt exit-code check.
fn run_case(case: &PrCase) -> Result<(), String> {
    let id = &case.id;
    let root = Path::new(&case.root);
    let diff = root.join(&case.diff);

    // --- Advisory check: capture JSON stdout and assert movement counts ---
    let check_stdout = run_unsafe_review_capture([
        os("check"),
        os("--root"),
        root.as_os_str().to_os_string(),
        os("--diff"),
        diff.as_os_str().to_os_string(),
        os("--format"),
        os("json"),
    ])
    .map_err(|err| format!("check-real-pr-corpus case `{id}`: advisory check failed: {err}"))?;

    let check_json: serde_json::Value = serde_json::from_str(&check_stdout).map_err(|err| {
        format!("check-real-pr-corpus case `{id}`: advisory check JSON parse failed: {err}")
    })?;

    assert_movement_counts(id, &check_json, &case.expected)?;

    // --- Comment-plan: run first-pr into a target/ temp dir ---
    let out_dir_rel = format!("target/unsafe-review-pr-corpus-{id}");
    let out_dir = PathBuf::from(&out_dir_rel);

    // Clean the temp dir before use (safe: it is always under target/).
    if out_dir.exists() {
        fs::remove_dir_all(&out_dir).map_err(|err| {
            format!(
                "check-real-pr-corpus case `{id}`: remove {} failed: {err}",
                out_dir.display()
            )
        })?;
    }
    fs::create_dir_all(&out_dir).map_err(|err| {
        format!(
            "check-real-pr-corpus case `{id}`: create {} failed: {err}",
            out_dir.display()
        )
    })?;

    run_unsafe_review_silent([
        os("first-pr"),
        os("--root"),
        root.as_os_str().to_os_string(),
        os("--diff"),
        diff.as_os_str().to_os_string(),
        os("--out-dir"),
        out_dir.as_os_str().to_os_string(),
    ])
    .map_err(|err| format!("check-real-pr-corpus case `{id}`: first-pr failed: {err}"))?;

    let comment_plan_path = out_dir.join("comment-plan.json");
    let comment_plan_text = fs::read_to_string(&comment_plan_path).map_err(|err| {
        format!(
            "check-real-pr-corpus case `{id}`: read {} failed: {err}",
            comment_plan_path.display()
        )
    })?;
    let comment_plan: serde_json::Value =
        serde_json::from_str(&comment_plan_text).map_err(|err| {
            format!("check-real-pr-corpus case `{id}`: comment-plan.json parse failed: {err}")
        })?;

    assert_comment_plan_counts(id, &comment_plan, &case.expected)?;

    // Clean up the temp dir after a successful assertion.
    let _ = fs::remove_dir_all(&out_dir);

    // --- Optional no-new-debt exit-code check ---
    if let Some(expected_exit) = case.no_new_debt_exit_code {
        let actual_exit = run_unsafe_review_exit_code([
            os("check"),
            os("--root"),
            root.as_os_str().to_os_string(),
            os("--diff"),
            diff.as_os_str().to_os_string(),
            os("--format"),
            os("json"),
            os("--policy"),
            os("no-new-debt"),
        ])
        .map_err(|err| {
            format!("check-real-pr-corpus case `{id}`: no-new-debt check failed to run: {err}")
        })?;

        if actual_exit != expected_exit {
            return Err(format!(
                "check-real-pr-corpus case `{id}`: no_new_debt_exit_code mismatch: \
                 expected={expected_exit} actual={actual_exit}"
            ));
        }
    }

    Ok(())
}

/// Assert summary movement counts in the advisory check JSON output.
fn assert_movement_counts(
    id: &str,
    check_json: &serde_json::Value,
    expected: &ExpectedCounts,
) -> Result<(), String> {
    let fields: &[(&str, u64)] = &[
        ("new_gaps", expected.new_gaps),
        ("worsened_gaps", expected.worsened_gaps),
        ("improved_gaps", expected.improved_gaps),
        ("resolved_gaps", expected.resolved_gaps),
        ("inherited_gaps", expected.inherited_gaps),
    ];

    for (field, exp) in fields {
        let pointer = format!("/summary/{field}");
        let actual = check_json
            .pointer(&pointer)
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| {
                format!("check-real-pr-corpus case `{id}`: advisory check JSON missing `{pointer}`")
            })?;
        if actual != *exp {
            return Err(format!(
                "check-real-pr-corpus case `{id}`: summary.{field} mismatch: \
                 expected={exp} actual={actual}"
            ));
        }
    }
    Ok(())
}

/// Assert comment-plan summary counts.
fn assert_comment_plan_counts(
    id: &str,
    comment_plan: &serde_json::Value,
    expected: &ExpectedCounts,
) -> Result<(), String> {
    let fields: &[(&str, u64)] = &[
        ("selected_count", expected.selected_count),
        ("not_selected_count", expected.not_selected_count),
    ];

    for (field, exp) in fields {
        let pointer = format!("/summary/{field}");
        let actual = comment_plan
            .pointer(&pointer)
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| {
                format!("check-real-pr-corpus case `{id}`: comment-plan.json missing `{pointer}`")
            })?;
        if actual != *exp {
            return Err(format!(
                "check-real-pr-corpus case `{id}`: comment-plan summary.{field} mismatch: \
                 expected={exp} actual={actual}"
            ));
        }
    }
    Ok(())
}

/// Parse the `policy/pr-corpus.toml` ledger and return all `[[pr]]` cases.
fn parse_ledger(path: &Path) -> Result<Vec<PrCase>, String> {
    let text =
        fs::read_to_string(path).map_err(|err| format!("read {} failed: {err}", path.display()))?;
    let doc: toml::Value = text
        .parse::<toml::Table>()
        .map(toml::Value::Table)
        .map_err(|err| format!("{} is not valid TOML: {err}", path.display()))?;

    // Validate schema_version.
    let _schema = doc
        .get("schema_version")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("{} missing string key `schema_version`", path.display()))?;

    let ledger_path = path.display().to_string();

    let entries = match doc.get("pr") {
        Some(toml::Value::Array(arr)) => arr,
        Some(_) => {
            return Err(format!("{ledger_path} `pr` must be an array of tables"));
        }
        None => return Ok(Vec::new()),
    };

    let mut cases = Vec::new();
    for (idx, entry) in entries.iter().enumerate() {
        let table = entry
            .as_table()
            .ok_or_else(|| format!("{ledger_path} pr[{idx}] must be a table"))?;

        let id = table
            .get("id")
            .and_then(toml::Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| format!("{ledger_path} pr[{idx}] missing non-empty `id`"))?
            .to_string();

        // Validate kind == "synthetic-fixture" (only kind supported in this gate).
        let kind = table
            .get("kind")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| format!("{ledger_path} pr[{idx}] ({id}) missing `kind`"))?;
        if kind != "synthetic-fixture" {
            return Err(format!(
                "{ledger_path} pr[{idx}] ({id}) kind `{kind}` is not supported \
                 by check-real-pr-corpus; only `synthetic-fixture` is allowed"
            ));
        }

        let root = table
            .get("root")
            .and_then(toml::Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| format!("{ledger_path} pr[{idx}] ({id}) missing non-empty `root`"))?
            .to_string();

        let diff = table
            .get("diff")
            .and_then(toml::Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| format!("{ledger_path} pr[{idx}] ({id}) missing non-empty `diff`"))?
            .to_string();

        let expected_table = table
            .get("expected")
            .and_then(toml::Value::as_table)
            .ok_or_else(|| {
                format!("{ledger_path} pr[{idx}] ({id}) missing `[pr.expected]` table")
            })?;

        let expected = parse_expected_counts(&id, &ledger_path, idx, expected_table)?;

        let no_new_debt_exit_code = table
            .get("no_new_debt_exit_code")
            .and_then(toml::Value::as_integer)
            .map(|v| v as i32);

        cases.push(PrCase {
            id,
            root,
            diff,
            expected,
            no_new_debt_exit_code,
        });
    }

    Ok(cases)
}

/// Parse an `[pr.expected]` table into `ExpectedCounts`.
fn parse_expected_counts(
    id: &str,
    ledger_path: &str,
    idx: usize,
    table: &toml::map::Map<String, toml::Value>,
) -> Result<ExpectedCounts, String> {
    let fields: &[&str] = &[
        "new_gaps",
        "worsened_gaps",
        "improved_gaps",
        "resolved_gaps",
        "inherited_gaps",
        "selected_count",
        "not_selected_count",
    ];

    let mut values = [0u64; 7];
    for (i, field) in fields.iter().enumerate() {
        let v = table
            .get(*field)
            .and_then(toml::Value::as_integer)
            .ok_or_else(|| {
                format!("{ledger_path} pr[{idx}] ({id}) [expected] missing integer `{field}`")
            })?;
        if v < 0 {
            return Err(format!(
                "{ledger_path} pr[{idx}] ({id}) [expected] `{field}` must be non-negative"
            ));
        }
        values[i] = v as u64;
    }

    Ok(ExpectedCounts {
        new_gaps: values[0],
        worsened_gaps: values[1],
        improved_gaps: values[2],
        resolved_gaps: values[3],
        inherited_gaps: values[4],
        selected_count: values[5],
        not_selected_count: values[6],
    })
}

/// Run `cargo run --locked -p unsafe-review -- <args>` and capture stdout.
/// Exits with an error if the process exits with code 2 (tool error).
/// Exit codes 0 and 1 are both acceptable (advisory / policy violation).
fn run_unsafe_review_capture(args: impl IntoIterator<Item = OsString>) -> Result<String, String> {
    let args: Vec<OsString> = args.into_iter().collect();
    let display = args
        .iter()
        .map(|a| a.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");

    let output = Command::new("cargo")
        .args(["run", "--locked", "-p", "unsafe-review", "--"])
        .args(&args)
        .output()
        .map_err(|err| format!("failed to spawn unsafe-review {display}: {err}"))?;

    // Exit 2 means the tool itself failed (bad args, IO error, etc.).
    if output.status.code() == Some(2) {
        return Err(format!(
            "unsafe-review {display} exited with code 2 (tool error):\nstderr:\n{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Run `cargo run --locked -p unsafe-review -- <args>`, discarding stdout/stderr.
/// Exits with an error if the process exits with code 2.
fn run_unsafe_review_silent(args: impl IntoIterator<Item = OsString>) -> Result<(), String> {
    let args: Vec<OsString> = args.into_iter().collect();
    let display = args
        .iter()
        .map(|a| a.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");

    let output = Command::new("cargo")
        .args(["run", "--locked", "-p", "unsafe-review", "--"])
        .args(&args)
        .output()
        .map_err(|err| format!("failed to spawn unsafe-review {display}: {err}"))?;

    if output.status.code() == Some(2) {
        return Err(format!(
            "unsafe-review {display} exited with code 2 (tool error):\nstderr:\n{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(())
}

/// Run `cargo run --locked -p unsafe-review -- <args>` and return the exit code.
/// Returns `Err` only if the process could not be spawned.
fn run_unsafe_review_exit_code(args: impl IntoIterator<Item = OsString>) -> Result<i32, String> {
    let args: Vec<OsString> = args.into_iter().collect();
    let display = args
        .iter()
        .map(|a| a.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");

    let status = Command::new("cargo")
        .args(["run", "--locked", "-p", "unsafe-review", "--"])
        .args(&args)
        .status()
        .map_err(|err| format!("failed to spawn unsafe-review {display}: {err}"))?;

    Ok(status.code().unwrap_or(-1))
}

fn os(s: &str) -> OsString {
    OsString::from(s)
}
