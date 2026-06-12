#![allow(
    clippy::module_name_repetitions,
    reason = "module name is clear in context"
)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use serde_json::json;

const DEFAULT_OUT: &str = "target/corpus-backstop/resource-report.json";
const SCHEMA_VERSION: &str = "0.1";
const TRUST_BOUNDARY: &str = "Corpus backstop report is diagnostic triage input only, not a coverage, precision, recall, memory-safety, UB-free, Miri-clean, site-execution, or performance SLA claim; not a gate.";
const CORPUS_SOURCE: &str = "docs/dogfood/corpus.toml";

/// Run the corpus backstop: iterate fixture-control targets and emit a resource-report.json.
pub(crate) fn run(out: Option<&Path>) -> Result<(), String> {
    let out_path = out
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from(DEFAULT_OUT));

    let targets = load_fixture_control_targets()?;

    let mut runs = Vec::new();
    let mut completed: u64 = 0;
    let mut failed: u64 = 0;

    for target in &targets {
        let run = execute_target(target);
        match run.status.as_str() {
            "completed" => completed += 1,
            _ => failed += 1,
        }
        runs.push(run);
    }

    let elapsed_ms_total: u64 = runs.iter().map(|r| r.elapsed_ms).sum();
    let output_bytes_total: u64 = runs.iter().map(|r| r.output_bytes).sum();
    let card_count_total: u64 = runs.iter().map(|r| r.card_count).sum();
    let target_count = targets.len() as u64;

    let runs_json: Vec<serde_json::Value> = runs
        .iter()
        .map(|r| {
            json!({
                "id": r.id,
                "kind": r.kind,
                "elapsed_ms": r.elapsed_ms,
                "output_bytes": r.output_bytes,
                "files_discovered": r.files_discovered,
                "files_scanned": r.files_scanned,
                "files_skipped": r.files_skipped,
                "card_count": r.card_count,
                "status": r.status,
            })
        })
        .collect();

    let now = now_utc_iso8601();

    let report = json!({
        "schema_version": SCHEMA_VERSION,
        "generated_at": now,
        "corpus_source": CORPUS_SOURCE,
        "run_summary": {
            "target_count": target_count,
            "completed": completed,
            "failed": failed,
            "skipped": 0u64,
        },
        "runs": runs_json,
        "totals": {
            "elapsed_ms_total": elapsed_ms_total,
            "output_bytes_total": output_bytes_total,
            "card_count_total": card_count_total,
        },
        "peak_rss_bytes": serde_json::Value::Null,
        "peak_rss_source": serde_json::Value::Null,
        "trust_boundary": TRUST_BOUNDARY,
    });

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create {} failed: {err}", parent.display()))?;
    }

    let report_text = serde_json::to_string_pretty(&report)
        .map_err(|err| format!("failed to serialize resource-report: {err}"))?;

    fs::write(&out_path, &report_text)
        .map_err(|err| format!("write {} failed: {err}", out_path.display()))?;

    println!(
        "corpus-backstop: wrote {} ({} completed, {} failed)",
        out_path.display(),
        completed,
        failed
    );

    Ok(())
}

/// Validate a resource-report.json file against the schema contract.
pub(crate) fn check_schema(path: &Path) -> Result<(), String> {
    let text =
        fs::read_to_string(path).map_err(|err| format!("read {} failed: {err}", path.display()))?;

    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|err| format!("{} is not valid JSON: {err}", path.display()))?;

    let obj = value
        .as_object()
        .ok_or_else(|| format!("{} must be a JSON object", path.display()))?;

    // schema_version must be a non-empty string
    match obj.get("schema_version") {
        Some(serde_json::Value::String(s)) if !s.is_empty() => {}
        Some(_) => {
            return Err(format!(
                "{}: schema_version must be a non-empty string",
                path.display()
            ));
        }
        None => {
            return Err(format!(
                "{}: missing required field schema_version",
                path.display()
            ));
        }
    }

    // generated_at must be a non-empty string
    match obj.get("generated_at") {
        Some(serde_json::Value::String(s)) if !s.is_empty() => {}
        Some(_) => {
            return Err(format!(
                "{}: generated_at must be a non-empty string",
                path.display()
            ));
        }
        None => {
            return Err(format!(
                "{}: missing required field generated_at",
                path.display()
            ));
        }
    }

    // runs must be an array
    let runs = match obj.get("runs") {
        Some(serde_json::Value::Array(arr)) => arr,
        Some(_) => return Err(format!("{}: runs must be an array", path.display())),
        None => return Err(format!("{}: missing required field runs", path.display())),
    };

    // trust_boundary must contain required phrases
    match obj.get("trust_boundary") {
        Some(serde_json::Value::String(s)) => {
            if !s.contains("diagnostic triage input only") {
                return Err(format!(
                    "{}: trust_boundary must contain 'diagnostic triage input only'",
                    path.display()
                ));
            }
            if !s.contains("not a gate") {
                return Err(format!(
                    "{}: trust_boundary must contain 'not a gate'",
                    path.display()
                ));
            }
        }
        Some(_) => {
            return Err(format!(
                "{}: trust_boundary must be a string",
                path.display()
            ));
        }
        None => {
            return Err(format!(
                "{}: missing required field trust_boundary",
                path.display()
            ));
        }
    }

    // peak_rss_bytes must be null or a positive number
    match obj.get("peak_rss_bytes") {
        Some(serde_json::Value::Null) => {}
        Some(serde_json::Value::Number(n)) => {
            let v = n.as_f64().unwrap_or(0.0);
            if v <= 0.0 {
                return Err(format!(
                    "{}: peak_rss_bytes must be null or a positive number",
                    path.display()
                ));
            }
        }
        Some(_) => {
            return Err(format!(
                "{}: peak_rss_bytes must be null or a number",
                path.display()
            ));
        }
        None => {
            return Err(format!(
                "{}: missing required field peak_rss_bytes",
                path.display()
            ));
        }
    }

    // Each run must have: id (string), kind (string), elapsed_ms (number), status (string)
    for (idx, run) in runs.iter().enumerate() {
        let run_obj = run
            .as_object()
            .ok_or_else(|| format!("{}: runs[{idx}] must be an object", path.display()))?;

        for field in ["id", "kind", "status"] {
            match run_obj.get(field) {
                Some(serde_json::Value::String(_)) => {}
                Some(_) => {
                    return Err(format!(
                        "{}: runs[{idx}].{field} must be a string",
                        path.display()
                    ));
                }
                None => {
                    return Err(format!(
                        "{}: runs[{idx}] missing required field {field}",
                        path.display()
                    ));
                }
            }
        }

        match run_obj.get("elapsed_ms") {
            Some(serde_json::Value::Number(_)) => {}
            Some(_) => {
                return Err(format!(
                    "{}: runs[{idx}].elapsed_ms must be a number",
                    path.display()
                ));
            }
            None => {
                return Err(format!(
                    "{}: runs[{idx}] missing required field elapsed_ms",
                    path.display()
                ));
            }
        }
    }

    println!(
        "check-corpus-backstop-schema: ok ({} runs validated in {})",
        runs.len(),
        path.display()
    );

    Ok(())
}

/// A single run result.
struct RunResult {
    id: String,
    kind: String,
    elapsed_ms: u64,
    output_bytes: u64,
    files_discovered: u64,
    files_scanned: u64,
    files_skipped: u64,
    card_count: u64,
    status: String,
}

/// Execute one fixture-control target and return a RunResult.
fn execute_target(target: &CorpusTarget) -> RunResult {
    let tmp_out = format!(
        "target/corpus-backstop/run-{}.json",
        sanitize_id(&target.id)
    );

    let start = Instant::now();
    let run_result = Command::new("cargo")
        .args([
            "run",
            "--locked",
            "-p",
            "unsafe-review",
            "--",
            "check",
            "--root",
            &target.root,
            "--diff",
            &target.diff,
            "--format",
            "json",
            "--out",
            &tmp_out,
        ])
        .output();
    let elapsed_ms = start.elapsed().as_millis() as u64;

    let (status, output_bytes, files_discovered, files_scanned, files_skipped, card_count) =
        match run_result {
            Err(_) => ("failed".to_string(), 0u64, 0u64, 0u64, 0u64, 0u64),
            Ok(output) if !output.status.success() => {
                ("failed".to_string(), 0u64, 0u64, 0u64, 0u64, 0u64)
            }
            Ok(_) => {
                let (ob, fd, fs, fsk, cc) = read_output_metrics(&tmp_out);
                ("completed".to_string(), ob, fd, fs, fsk, cc)
            }
        };

    RunResult {
        id: target.id.clone(),
        kind: target.kind.clone(),
        elapsed_ms,
        output_bytes,
        files_discovered,
        files_scanned,
        files_skipped,
        card_count,
        status,
    }
}

/// Read output metrics from the generated JSON artifact if available.
fn read_output_metrics(path: &str) -> (u64, u64, u64, u64, u64) {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return (0, 0, 0, 0, 0),
    };
    let output_bytes = text.len() as u64;
    let value: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return (output_bytes, 0, 0, 0, 0),
    };

    let scan_status = value
        .get("scan_status")
        .or_else(|| value.get("repo_scan_status"));

    let files_discovered = scan_status
        .and_then(|s| s.get("files_discovered"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let files_scanned = scan_status
        .and_then(|s| s.get("files_scanned"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let files_skipped = scan_status
        .and_then(|s| s.get("files_skipped"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let card_count = value
        .get("cards")
        .and_then(|c| c.as_array())
        .map(|arr| arr.len() as u64)
        .or_else(|| {
            value
                .get("summary")
                .and_then(|s| s.get("total_cards"))
                .and_then(|v| v.as_u64())
        })
        .unwrap_or(0);

    (
        output_bytes,
        files_discovered,
        files_scanned,
        files_skipped,
        card_count,
    )
}

/// A parsed fixture-control target from corpus.toml.
struct CorpusTarget {
    id: String,
    kind: String,
    root: String,
    diff: String,
}

/// Load fixture-control targets with status = "active" from corpus.toml.
fn load_fixture_control_targets() -> Result<Vec<CorpusTarget>, String> {
    let manifest_path = Path::new(CORPUS_SOURCE);
    let text = fs::read_to_string(manifest_path)
        .map_err(|err| format!("read {CORPUS_SOURCE} failed: {err}"))?;

    let value: toml::Value =
        toml::from_str(&text).map_err(|err| format!("parse {CORPUS_SOURCE} failed: {err}"))?;

    let targets_arr = match value.get("targets") {
        Some(toml::Value::Array(arr)) => arr,
        _ => return Err(format!("{CORPUS_SOURCE}: missing [[targets]] array")),
    };

    let mut result = Vec::new();
    for (idx, entry) in targets_arr.iter().enumerate() {
        let table = entry
            .as_table()
            .ok_or_else(|| format!("{CORPUS_SOURCE}: targets[{idx}] is not a table"))?;

        let kind = table
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let status = table.get("status").and_then(|v| v.as_str()).unwrap_or("");

        if kind != "fixture-control" || status != "active" {
            continue;
        }

        let id = table
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("{CORPUS_SOURCE}: targets[{idx}] missing id"))?
            .to_string();

        let root = table
            .get("root")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("{CORPUS_SOURCE}: targets[{idx}] ({id}) missing root"))?
            .to_string();

        let diff = table
            .get("diff")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("{CORPUS_SOURCE}: targets[{idx}] ({id}) missing diff"))?
            .to_string();

        result.push(CorpusTarget {
            id,
            kind,
            root,
            diff,
        });
    }

    Ok(result)
}

/// Sanitize a target id for use in a filename: keep alphanumeric, dash, underscore.
fn sanitize_id(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Return a minimal ISO-8601 UTC timestamp string (seconds precision).
fn now_utc_iso8601() -> String {
    // Use SystemTime for a timestamp without external deps.
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Format as YYYY-MM-DDTHH:MM:SSZ
    let s = secs;
    let sec = s % 60;
    let min = (s / 60) % 60;
    let hour = (s / 3600) % 24;
    let days = s / 86400;
    // Compute year/month/day from days since epoch (1970-01-01).
    let (year, month, day) = days_to_ymd(days);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}Z")
}

/// Convert days since Unix epoch to (year, month, day).
fn days_to_ymd(days: u64) -> (u32, u32, u32) {
    // Gregorian calendar computation.
    let mut y = 1970u32;
    let mut remaining = days;

    loop {
        let days_in_y = if is_leap(y) { 366 } else { 365 };
        if remaining < days_in_y {
            break;
        }
        remaining -= days_in_y;
        y += 1;
    }

    let month_days: [u32; 12] = if is_leap(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut m = 1u32;
    for &md in &month_days {
        if remaining < md as u64 {
            break;
        }
        remaining -= md as u64;
        m += 1;
    }

    (y, m, remaining as u32 + 1)
}

fn is_leap(year: u32) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}
