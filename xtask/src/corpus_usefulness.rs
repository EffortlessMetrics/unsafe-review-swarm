#![allow(
    clippy::module_name_repetitions,
    reason = "module name is clear in context"
)]

//! `corpus-usefulness` xtask subcommand.
//!
//! Aggregates SPEC-0038 `usefulness-telemetry.json` across a curated,
//! documented representative subset of local `fixtures/` and emits a
//! `corpus-usefulness-rollup.json` artifact.
//!
//! This subcommand is OFF the per-PR critical path.  Only a fast schema check
//! on the committed sample belongs in `check-pr`.
//!
//! Trust boundary: diagnostic usefulness/noise characterisation only — NOT
//! calibrated precision/recall, NOT accuracy %, NOT a proof/UB-free/Miri-clean/
//! site-execution claim, NOT a gate/SLA.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use serde_json::json;

// ── constants ──────────────────────────────────────────────────────────────

const DEFAULT_OUT: &str = "target/corpus-usefulness/corpus-usefulness-rollup.json";
const SCHEMA_VERSION: &str = "corpus-usefulness-rollup/v1";
const TRUST_BOUNDARY: &str = "Corpus usefulness rollup is diagnostic noise/usefulness \
characterisation only — not calibrated precision or recall, not a coverage claim, \
not proof of memory safety, not UB-free, not Miri-clean, not site-execution, \
not a gate. Aggregated from SPEC-0038 usefulness-telemetry.json projected from ReviewCard \
truth objects across the listed fixture subset.";

/// The curated representative subset.  Each entry has a fixture directory name
/// (relative to `fixtures/`) and a short rationale documenting why it was
/// chosen for the subset (noise shape it exercises).
///
/// Selection criteria (per brief):
/// - negative controls (no-cards expected): `safe_code_no_cards`
/// - single-gap positive: `raw_pointer_alignment`
/// - multi-gap / multi-obligation: `vec_from_raw_parts`
/// - witnessed / receipt attached: `raw_pointer_alignment_receipted`
/// - FFI boundary: `ffi_missing_boundary_contract`
/// - human-review-only: `inline_asm_human_review`
/// - guard not matching (false-positive controls): `raw_pointer_alignment_closed_branch_not_guard`
/// - capped / large diff edge: `copy_nonoverlapping`
/// - alternate operation families: `box_from_raw`, `drop_in_place_deallocation`, `transmute_bool_disjunct_return_guard`
/// - agent-ready indicators: `atomic_pointer_state_swap`
/// - documented private unsafe fn: `documented_private_unsafe_fn`
/// - adjacent-unchanged (no-card control): `adjacent_unchanged_unsafe_fn_no_card`
/// - vec with ManuallyDrop origin: `vec_from_raw_parts_manuallydrop_origin`
/// - FFI sanitizer-route: `ffi_sanitizer_route`
/// - copy_nonoverlapping with full guards: `copy_nonoverlapping_slice_range_guard`
pub(crate) const SUBSET: &[(&str, &str)] = &[
    (
        "safe_code_no_cards",
        "negative control: safe code produces zero cards",
    ),
    (
        "adjacent_unchanged_unsafe_fn_no_card",
        "negative control: unchanged unsafe fn outside diff produces no card",
    ),
    (
        "raw_pointer_alignment",
        "positive single-gap: raw pointer missing alignment guard",
    ),
    (
        "raw_pointer_alignment_receipted",
        "witnessed/receipted: same operation with witness receipt attached",
    ),
    (
        "raw_pointer_alignment_closed_branch_not_guard",
        "false-positive control: closed branch is not a guard",
    ),
    (
        "vec_from_raw_parts",
        "multi-gap positive: Vec::from_raw_parts with multiple obligation gaps",
    ),
    (
        "vec_from_raw_parts_manuallydrop_origin",
        "multi-gap variant: ManuallyDrop pointer origin shape",
    ),
    (
        "copy_nonoverlapping",
        "capped / multi-pointer operation: ptr::copy_nonoverlapping",
    ),
    (
        "copy_nonoverlapping_slice_range_guard",
        "multi-pointer with full slice-range guard discharged",
    ),
    (
        "box_from_raw",
        "alternate operation family: Box::from_raw ownership obligation",
    ),
    (
        "drop_in_place_deallocation",
        "alternate operation family: ptr::drop_in_place deallocation hazard",
    ),
    (
        "transmute_bool_disjunct_return_guard",
        "transmute family with disjunctive early-return guard",
    ),
    (
        "inline_asm_human_review",
        "human-review-only: inline assembly requires human reviewer",
    ),
    (
        "ffi_missing_boundary_contract",
        "FFI boundary: missing boundary contract card",
    ),
    (
        "ffi_sanitizer_route",
        "FFI sanitizer-route: sanitizer witness route exists",
    ),
    (
        "atomic_pointer_state_swap",
        "atomic pointer operation: agent-readiness shape",
    ),
    (
        "documented_private_unsafe_fn",
        "documented private unsafe fn: contract coverage shape",
    ),
    (
        "raw_pointer_deref_brownfield_inherited",
        "brownfield/inherited debt: pre-existing baselined gap shows inherited_cards=1 with new_gaps=0 for a safe-only diff",
    ),
];

// ── public API ─────────────────────────────────────────────────────────────

/// Run the corpus-usefulness aggregation across the curated subset.
pub(crate) fn run(out: Option<&Path>) -> Result<(), String> {
    let out_path = out
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from(DEFAULT_OUT));

    // Build the binary once up front so per-fixture elapsed_ms reflects actual
    // scan time, not compile time.  Without this the first `cargo run` invocation
    // absorbs the full incremental compilation and inflates the elapsed range.
    let binary = build_binary()?;
    println!("corpus-usefulness: binary at {}", binary.display());

    let mut per_run: Vec<RunResult> = Vec::new();

    for (fixture_name, rationale) in SUBSET {
        let result = run_fixture(fixture_name, rationale, &binary)?;
        per_run.push(result);
    }

    let rollup = build_rollup(&per_run);

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create_dir_all {} failed: {err}", parent.display()))?;
    }

    let text = serde_json::to_string_pretty(&rollup)
        .map_err(|err| format!("serialise rollup failed: {err}"))?;

    fs::write(&out_path, &text)
        .map_err(|err| format!("write {} failed: {err}", out_path.display()))?;

    let completed = per_run.iter().filter(|r| r.status == "completed").count();
    let failed = per_run.iter().filter(|r| r.status == "failed").count();
    println!(
        "corpus-usefulness: wrote {} ({} completed, {} failed of {} fixtures)",
        out_path.display(),
        completed,
        failed,
        SUBSET.len(),
    );

    Ok(())
}

/// Validate a corpus-usefulness-rollup.json against the schema contract.
pub(crate) fn check_schema(path: &Path) -> Result<(), String> {
    let text =
        fs::read_to_string(path).map_err(|err| format!("read {} failed: {err}", path.display()))?;

    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|err| format!("{} is not valid JSON: {err}", path.display()))?;

    let obj = value
        .as_object()
        .ok_or_else(|| format!("{} must be a JSON object", path.display()))?;

    // schema_version
    match obj.get("schema_version") {
        Some(serde_json::Value::String(s)) if s == SCHEMA_VERSION => {}
        Some(serde_json::Value::String(s)) => {
            return Err(format!(
                "{}: schema_version must be \"{SCHEMA_VERSION}\", got \"{s}\"",
                path.display()
            ));
        }
        Some(_) => {
            return Err(format!(
                "{}: schema_version must be a string",
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

    // generated_at
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

    // trust_boundary — must contain "not calibrated" and must not positive-claim forbidden phrases
    match obj.get("trust_boundary") {
        Some(serde_json::Value::String(s)) => {
            if !s.contains("not calibrated") {
                return Err(format!(
                    "{}: trust_boundary must contain 'not calibrated'",
                    path.display()
                ));
            }
            if !s.contains("not a gate") {
                return Err(format!(
                    "{}: trust_boundary must contain 'not a gate'",
                    path.display()
                ));
            }
            for forbidden in ["UB-free", "Miri-clean", "site-execution", "proof"] {
                // Positive claims are forbidden — but the word may appear as part of
                // "not UB-free" which is fine.  We check for bare positive form only.
                if s.contains(forbidden) && !s.contains(&format!("not {forbidden}")) {
                    return Err(format!(
                        "{}: trust_boundary must not make a positive {forbidden} claim",
                        path.display()
                    ));
                }
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

    // fixture_subset must be a non-empty array
    let subset = match obj.get("fixture_subset") {
        Some(serde_json::Value::Array(arr)) if !arr.is_empty() => arr,
        Some(serde_json::Value::Array(_)) => {
            return Err(format!(
                "{}: fixture_subset must not be empty",
                path.display()
            ));
        }
        Some(_) => {
            return Err(format!(
                "{}: fixture_subset must be an array",
                path.display()
            ));
        }
        None => {
            return Err(format!(
                "{}: missing required field fixture_subset",
                path.display()
            ));
        }
    };

    // Each fixture_subset entry must have fixture and rationale strings
    for (idx, entry) in subset.iter().enumerate() {
        let entry_obj = entry.as_object().ok_or_else(|| {
            format!(
                "{}: fixture_subset[{idx}] must be an object",
                path.display()
            )
        })?;
        for field in ["fixture", "rationale"] {
            match entry_obj.get(field) {
                Some(serde_json::Value::String(s)) if !s.is_empty() => {}
                Some(_) => {
                    return Err(format!(
                        "{}: fixture_subset[{idx}].{field} must be a non-empty string",
                        path.display()
                    ));
                }
                None => {
                    return Err(format!(
                        "{}: fixture_subset[{idx}] missing required field {field}",
                        path.display()
                    ));
                }
            }
        }
    }

    // corpus_totals must be an object with required numeric fields
    let totals = match obj.get("corpus_totals") {
        Some(serde_json::Value::Object(o)) => o,
        Some(_) => {
            return Err(format!(
                "{}: corpus_totals must be an object",
                path.display()
            ));
        }
        None => {
            return Err(format!(
                "{}: missing required field corpus_totals",
                path.display()
            ));
        }
    };
    for field in ["fixtures_run", "fixtures_completed", "fixtures_failed"] {
        match totals.get(field) {
            Some(serde_json::Value::Number(_)) => {}
            Some(_) => {
                return Err(format!(
                    "{}: corpus_totals.{field} must be a number",
                    path.display()
                ));
            }
            None => {
                return Err(format!(
                    "{}: corpus_totals missing required field {field}",
                    path.display()
                ));
            }
        }
    }

    // card_inventory must be an object
    match obj.get("card_inventory") {
        Some(serde_json::Value::Object(_)) => {}
        Some(_) => {
            return Err(format!(
                "{}: card_inventory must be an object",
                path.display()
            ));
        }
        None => {
            return Err(format!(
                "{}: missing required field card_inventory",
                path.display()
            ));
        }
    }

    // agent_readiness must be an object
    match obj.get("agent_readiness") {
        Some(serde_json::Value::Object(_)) => {}
        Some(_) => {
            return Err(format!(
                "{}: agent_readiness must be an object",
                path.display()
            ));
        }
        None => {
            return Err(format!(
                "{}: missing required field agent_readiness",
                path.display()
            ));
        }
    }

    // scan_cost_range must be an object with elapsed_ms_min/median/max
    let scan_cost = match obj.get("scan_cost_range") {
        Some(serde_json::Value::Object(o)) => o,
        Some(_) => {
            return Err(format!(
                "{}: scan_cost_range must be an object",
                path.display()
            ));
        }
        None => {
            return Err(format!(
                "{}: missing required field scan_cost_range",
                path.display()
            ));
        }
    };
    for field in ["elapsed_ms_min", "elapsed_ms_median", "elapsed_ms_max"] {
        match scan_cost.get(field) {
            Some(serde_json::Value::Number(_)) | Some(serde_json::Value::Null) => {}
            Some(_) => {
                return Err(format!(
                    "{}: scan_cost_range.{field} must be a number or null",
                    path.display()
                ));
            }
            None => {
                return Err(format!(
                    "{}: scan_cost_range missing required field {field}",
                    path.display()
                ));
            }
        }
    }

    // human_summary must be a non-empty string
    match obj.get("human_summary") {
        Some(serde_json::Value::String(s)) if !s.is_empty() => {}
        Some(_) => {
            return Err(format!(
                "{}: human_summary must be a non-empty string",
                path.display()
            ));
        }
        None => {
            return Err(format!(
                "{}: missing required field human_summary",
                path.display()
            ));
        }
    }

    println!(
        "check-corpus-usefulness-schema: ok ({} subset entries in {})",
        subset.len(),
        path.display()
    );

    Ok(())
}

// ── internals ──────────────────────────────────────────────────────────────

struct RunResult {
    fixture: String,
    rationale: String,
    status: String,
    /// Parsed telemetry JSON, or None if the run failed or the file was absent.
    telemetry: Option<serde_json::Value>,
    elapsed_ms: u64,
}

/// Build `unsafe-review` and return the path to the compiled binary.
///
/// Using `cargo build` once and then invoking the binary directly keeps
/// per-fixture `elapsed_ms` honest — it measures scan time only, not
/// incremental compilation.
fn build_binary() -> Result<PathBuf, String> {
    let status = Command::new("cargo")
        .args(["build", "--locked", "-p", "unsafe-review"])
        .status()
        .map_err(|err| format!("cargo build spawn failed: {err}"))?;

    if !status.success() {
        return Err("cargo build --locked -p unsafe-review failed".to_string());
    }

    // Resolve the target directory from cargo metadata so the path is correct
    // regardless of any workspace-level target-dir override.
    let meta_out = Command::new("cargo")
        .args(["metadata", "--locked", "--no-deps", "--format-version=1"])
        .output()
        .map_err(|err| format!("cargo metadata spawn failed: {err}"))?;

    if !meta_out.status.success() {
        return Err("cargo metadata failed".to_string());
    }

    let meta_text =
        String::from_utf8(meta_out.stdout).map_err(|err| format!("cargo metadata utf8: {err}"))?;

    let meta: serde_json::Value = serde_json::from_str(&meta_text)
        .map_err(|err| format!("cargo metadata json parse failed: {err}"))?;

    let target_dir = meta
        .get("target_directory")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "cargo metadata: missing target_directory".to_string())?;

    // Platform-aware binary name.
    let bin_name = if cfg!(windows) {
        "unsafe-review.exe"
    } else {
        "unsafe-review"
    };

    let bin_path = PathBuf::from(target_dir).join("debug").join(bin_name);

    if !bin_path.exists() {
        return Err(format!("built binary not found at {}", bin_path.display()));
    }

    Ok(bin_path)
}

fn run_fixture(fixture_name: &str, rationale: &str, binary: &Path) -> Result<RunResult, String> {
    let root = format!("fixtures/{fixture_name}");
    let diff = format!("fixtures/{fixture_name}/change.diff");

    // We must verify the fixture exists before trying to run it.
    if !Path::new(&root).is_dir() {
        return Ok(RunResult {
            fixture: fixture_name.to_string(),
            rationale: rationale.to_string(),
            status: "skipped_missing".to_string(),
            telemetry: None,
            elapsed_ms: 0,
        });
    }

    // Emit artifacts into a per-fixture temp directory under target/.
    let safe_name = sanitize_id(fixture_name);
    let out_dir = format!("target/corpus-usefulness/runs/{safe_name}");
    fs::create_dir_all(&out_dir)
        .map_err(|err| format!("create_dir_all {out_dir} failed: {err}"))?;

    // Invoke the pre-built binary directly — elapsed_ms must reflect scan time,
    // not compile time.  build_binary() already ensured it is up to date.
    let start = Instant::now();
    let result = Command::new(binary)
        .args([
            "first-pr",
            "--root",
            &root,
            "--diff",
            &diff,
            "--out-dir",
            &out_dir,
        ])
        .output();
    let elapsed_ms = start.elapsed().as_millis() as u64;

    let status = match result {
        Err(err) => {
            println!("corpus-usefulness: spawn failed for {fixture_name}: {err}");
            "failed"
        }
        Ok(ref output) if !output.status.success() => "failed",
        Ok(_) => "completed",
    };

    let telemetry = if status == "completed" {
        let telemetry_path = format!("{out_dir}/usefulness-telemetry.json");
        read_telemetry(&telemetry_path)
    } else {
        None
    };

    Ok(RunResult {
        fixture: fixture_name.to_string(),
        rationale: rationale.to_string(),
        status: status.to_string(),
        telemetry,
        elapsed_ms,
    })
}

fn read_telemetry(path: &str) -> Option<serde_json::Value> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// Aggregate all per-run telemetry into the corpus rollup JSON value.
fn build_rollup(runs: &[RunResult]) -> serde_json::Value {
    let fixtures_run = runs.len() as u64;
    let fixtures_completed = runs.iter().filter(|r| r.status == "completed").count() as u64;
    let fixtures_failed = runs
        .iter()
        .filter(|r| r.status == "failed" || r.status == "skipped_missing")
        .count() as u64;

    // ── aggregate card_inventory ───────────────────────────────────────────
    let mut total_cards: u64 = 0;
    let mut actionable_cards: u64 = 0;
    let mut new_cards: u64 = 0;
    let mut worsened_cards: u64 = 0;
    let mut resolved_cards: u64 = 0;
    let mut inherited_cards: u64 = 0;

    // ── aggregate coverage_slots ──────────────────────────────────────────
    let mut contract_missing: u64 = 0;
    let mut contract_weak: u64 = 0;
    let mut guard_missing: u64 = 0;
    let mut guard_weak: u64 = 0;
    let mut test_reach_missing: u64 = 0;
    let mut test_reach_weak: u64 = 0;
    let mut witness_receipt_missing: u64 = 0;

    // ── aggregate agent_readiness ─────────────────────────────────────────
    let mut ar_ready: u64 = 0;
    let mut ar_requires_witness_receipt: u64 = 0;
    let mut ar_needs_human: u64 = 0;
    let mut ar_unsupported: u64 = 0;

    // ── aggregate not_selected_reason_histogram ───────────────────────────
    let mut not_selected_reason: BTreeMap<String, u64> = BTreeMap::new();
    // ── aggregate not_selected_class_histogram ────────────────────────────
    let mut not_selected_class: BTreeMap<String, u64> = BTreeMap::new();
    // ── aggregate unfulfilled_obligation_count ────────────────────────────
    let mut unfulfilled_obligation_total: u64 = 0;

    // ── scan_cost samples ─────────────────────────────────────────────────
    let mut elapsed_samples: Vec<u64> = Vec::new();
    let mut output_bytes_samples: Vec<u64> = Vec::new();

    for run in runs {
        // Always record the CLI-measured elapsed time for all completed runs,
        // regardless of whether telemetry has a scan_cost field.
        if run.status == "completed" {
            elapsed_samples.push(run.elapsed_ms);
        }

        let Some(tel) = &run.telemetry else {
            continue;
        };

        // card_inventory
        if let Some(ci) = tel.get("card_inventory").and_then(|v| v.as_object()) {
            total_cards += obj_u64(ci, "total_cards");
            actionable_cards += obj_u64(ci, "actionable_cards");
            new_cards += obj_u64(ci, "new_cards");
            worsened_cards += obj_u64(ci, "worsened_cards");
            resolved_cards += obj_u64(ci, "resolved_cards");
            inherited_cards += obj_u64(ci, "inherited_cards");
        }

        // coverage_slots
        if let Some(cs) = tel.get("coverage_slots").and_then(|v| v.as_object()) {
            contract_missing += obj_u64(cs, "contract_missing");
            contract_weak += obj_u64(cs, "contract_weak");
            guard_missing += obj_u64(cs, "guard_missing");
            guard_weak += obj_u64(cs, "guard_weak");
            test_reach_missing += obj_u64(cs, "test_reach_missing");
            test_reach_weak += obj_u64(cs, "test_reach_weak");
            witness_receipt_missing += obj_u64(cs, "witness_receipt_missing");
        }

        // agent_readiness
        if let Some(ar) = tel.get("agent_readiness").and_then(|v| v.as_object()) {
            ar_ready += obj_u64(ar, "ready");
            ar_requires_witness_receipt += obj_u64(ar, "requires_witness_receipt");
            ar_needs_human += obj_u64(ar, "needs_human");
            ar_unsupported += obj_u64(ar, "unsupported");
        }

        // comment_selection histograms
        if let Some(cs) = tel.get("comment_selection").and_then(|v| v.as_object()) {
            if let Some(hist) = cs
                .get("not_selected_reason_histogram")
                .and_then(|v| v.as_object())
            {
                for (k, v) in hist {
                    let count = v.as_u64().unwrap_or(0);
                    *not_selected_reason.entry(k.clone()).or_insert(0) += count;
                }
            }
            if let Some(hist) = cs
                .get("not_selected_class_histogram")
                .and_then(|v| v.as_object())
            {
                for (k, v) in hist {
                    let count = v.as_u64().unwrap_or(0);
                    *not_selected_class.entry(k.clone()).or_insert(0) += count;
                }
            }
        }

        // unfulfilled_obligation_count
        if let Some(v) = tel
            .get("unfulfilled_obligation_count")
            .and_then(|v| v.as_u64())
        {
            unfulfilled_obligation_total += v;
        }

        // scan_cost (from telemetry artifact itself)
        if let Some(sc) = tel.get("scan_cost").and_then(|v| v.as_object())
            && let Some(b) = sc.get("output_bytes_total").and_then(|v| v.as_u64())
        {
            output_bytes_samples.push(b);
        }
    }

    // ── per-fixture list for the rollup ───────────────────────────────────
    let fixture_subset: Vec<serde_json::Value> = runs
        .iter()
        .map(|r| {
            json!({
                "fixture": r.fixture,
                "rationale": r.rationale,
                "status": r.status,
                "elapsed_ms": r.elapsed_ms,
            })
        })
        .collect();

    // ── scan_cost_range ───────────────────────────────────────────────────
    let scan_cost_range = build_scan_cost_range(&elapsed_samples, &output_bytes_samples);

    // ── human summary line ────────────────────────────────────────────────
    let human_summary = format!(
        "{fixtures_completed}/{fixtures_run} fixtures completed; \
{total_cards} total cards ({actionable_cards} actionable); \
agent_readiness: {ar_ready} ready / {ar_requires_witness_receipt} requires_witness_receipt / \
{ar_needs_human} needs_human / {ar_unsupported} unsupported; \
unfulfilled_obligations: {unfulfilled_obligation_total}; \
elapsed ms min/median/max: {}/{}/{}",
        scan_cost_range
            .get("elapsed_ms_min")
            .and_then(|v| v.as_u64())
            .map(|v| v.to_string())
            .unwrap_or_else(|| "n/a".to_string()),
        scan_cost_range
            .get("elapsed_ms_median")
            .and_then(|v| v.as_u64())
            .map(|v| v.to_string())
            .unwrap_or_else(|| "n/a".to_string()),
        scan_cost_range
            .get("elapsed_ms_max")
            .and_then(|v| v.as_u64())
            .map(|v| v.to_string())
            .unwrap_or_else(|| "n/a".to_string()),
    );

    let now = now_utc_iso8601();

    json!({
        "schema_version": SCHEMA_VERSION,
        "generated_at": now,
        "trust_boundary": TRUST_BOUNDARY,
        "fixture_subset": fixture_subset,
        "corpus_totals": {
            "fixtures_run": fixtures_run,
            "fixtures_completed": fixtures_completed,
            "fixtures_failed": fixtures_failed,
        },
        "card_inventory": {
            "total_cards": total_cards,
            "actionable_cards": actionable_cards,
            "new_cards": new_cards,
            "worsened_cards": worsened_cards,
            "resolved_cards": resolved_cards,
            "inherited_cards": inherited_cards,
        },
        "coverage_slots": {
            "contract_missing": contract_missing,
            "contract_weak": contract_weak,
            "guard_missing": guard_missing,
            "guard_weak": guard_weak,
            "test_reach_missing": test_reach_missing,
            "test_reach_weak": test_reach_weak,
            "witness_receipt_missing": witness_receipt_missing,
        },
        "agent_readiness": {
            "ready": ar_ready,
            "requires_witness_receipt": ar_requires_witness_receipt,
            "needs_human": ar_needs_human,
            "unsupported": ar_unsupported,
        },
        "not_selected_reason_histogram": not_selected_reason,
        "not_selected_class_histogram": not_selected_class,
        "unfulfilled_obligation_count": unfulfilled_obligation_total,
        "scan_cost_range": scan_cost_range,
        "human_summary": human_summary,
    })
}

fn build_scan_cost_range(
    elapsed_ms: &[u64],
    output_bytes: &[u64],
) -> serde_json::Map<String, serde_json::Value> {
    let mut map = serde_json::Map::new();

    if elapsed_ms.is_empty() {
        map.insert("elapsed_ms_min".to_string(), serde_json::Value::Null);
        map.insert("elapsed_ms_median".to_string(), serde_json::Value::Null);
        map.insert("elapsed_ms_max".to_string(), serde_json::Value::Null);
    } else {
        let mut sorted = elapsed_ms.to_vec();
        sorted.sort_unstable();
        let min = sorted[0];
        let max = *sorted.last().unwrap_or(&0);
        let median = sorted[sorted.len() / 2];
        map.insert(
            "elapsed_ms_min".to_string(),
            serde_json::Value::Number(min.into()),
        );
        map.insert(
            "elapsed_ms_median".to_string(),
            serde_json::Value::Number(median.into()),
        );
        map.insert(
            "elapsed_ms_max".to_string(),
            serde_json::Value::Number(max.into()),
        );
    }

    if output_bytes.is_empty() {
        map.insert("output_bytes_min".to_string(), serde_json::Value::Null);
        map.insert("output_bytes_median".to_string(), serde_json::Value::Null);
        map.insert("output_bytes_max".to_string(), serde_json::Value::Null);
    } else {
        let mut sorted = output_bytes.to_vec();
        sorted.sort_unstable();
        let min = sorted[0];
        let max = *sorted.last().unwrap_or(&0);
        let median = sorted[sorted.len() / 2];
        map.insert(
            "output_bytes_min".to_string(),
            serde_json::Value::Number(min.into()),
        );
        map.insert(
            "output_bytes_median".to_string(),
            serde_json::Value::Number(median.into()),
        );
        map.insert(
            "output_bytes_max".to_string(),
            serde_json::Value::Number(max.into()),
        );
    }

    map
}

fn obj_u64(obj: &serde_json::Map<String, serde_json::Value>, key: &str) -> u64 {
    obj.get(key).and_then(|v| v.as_u64()).unwrap_or(0)
}

fn sanitize_id(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Minimal ISO-8601 UTC timestamp (seconds precision).
fn now_utc_iso8601() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let sec = secs % 60;
    let min = (secs / 60) % 60;
    let hour = (secs / 3600) % 24;
    let days = secs / 86400;
    let (year, month, day) = days_to_ymd(days);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}Z")
}

fn days_to_ymd(days: u64) -> (u32, u32, u32) {
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

// ── tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    /// Create a unique temp path to avoid test-parallel collisions.
    fn unique_temp_path(prefix: &str) -> Result<std::path::PathBuf, String> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system clock error: {err}"))?
            .as_nanos();
        Ok(std::env::temp_dir().join(format!("{prefix}-{nanos}.json")))
    }

    #[test]
    fn schema_check_accepts_valid_sample() -> Result<(), String> {
        let rollup = serde_json::json!({
            "schema_version": SCHEMA_VERSION,
            "generated_at": "2026-06-12T00:00:00Z",
            "trust_boundary": TRUST_BOUNDARY,
            "fixture_subset": [
                {
                    "fixture": "safe_code_no_cards",
                    "rationale": "negative control: safe code",
                    "status": "completed",
                    "elapsed_ms": 100u64
                }
            ],
            "corpus_totals": {
                "fixtures_run": 1u64,
                "fixtures_completed": 1u64,
                "fixtures_failed": 0u64,
            },
            "card_inventory": {
                "total_cards": 0u64,
                "actionable_cards": 0u64,
                "new_cards": 0u64,
                "worsened_cards": 0u64,
                "resolved_cards": 0u64,
                "inherited_cards": 0u64,
            },
            "coverage_slots": {
                "contract_missing": 0u64,
                "contract_weak": 0u64,
                "guard_missing": 0u64,
                "guard_weak": 0u64,
                "test_reach_missing": 0u64,
                "test_reach_weak": 0u64,
                "witness_receipt_missing": 0u64,
            },
            "agent_readiness": {
                "ready": 0u64,
                "requires_witness_receipt": 0u64,
                "needs_human": 0u64,
                "unsupported": 0u64,
            },
            "not_selected_reason_histogram": {},
            "not_selected_class_histogram": {},
            "unfulfilled_obligation_count": 0u64,
            "scan_cost_range": {
                "elapsed_ms_min": serde_json::Value::Null,
                "elapsed_ms_median": serde_json::Value::Null,
                "elapsed_ms_max": serde_json::Value::Null,
            },
            "human_summary": "1/1 fixtures completed; 0 total cards (0 actionable)",
        });
        let sample = serde_json::to_string_pretty(&rollup)
            .map_err(|err| format!("test sample serialisation failed: {err}"))?;

        let path = unique_temp_path("corpus-usefulness-test-valid")?;
        fs::write(&path, &sample).map_err(|err| format!("write test sample failed: {err}"))?;
        let result = check_schema(&path);
        let _ = fs::remove_file(&path);
        result
    }

    #[test]
    fn schema_check_rejects_missing_schema_version() -> Result<(), String> {
        let sample = r#"{"generated_at":"2026-06-12T00:00:00Z","trust_boundary":"not calibrated not a gate"}"#;
        let path = unique_temp_path("corpus-usefulness-test-no-sv")?;
        fs::write(&path, sample).map_err(|err| format!("write test sample failed: {err}"))?;
        let result = check_schema(&path);
        let _ = fs::remove_file(&path);
        if result.is_err() {
            Ok(())
        } else {
            Err("schema check should have rejected missing schema_version".to_string())
        }
    }

    #[test]
    fn schema_check_rejects_missing_trust_boundary_phrases() -> Result<(), String> {
        let rollup = serde_json::json!({
            "schema_version": SCHEMA_VERSION,
            "generated_at": "2026-06-12T00:00:00Z",
            "trust_boundary": "incomplete boundary",
            "fixture_subset": [{"fixture": "x", "rationale": "y", "status": "ok", "elapsed_ms": 1u64}],
            "corpus_totals": {"fixtures_run": 1u64, "fixtures_completed": 1u64, "fixtures_failed": 0u64},
            "card_inventory": {
                "total_cards": 0u64, "actionable_cards": 0u64, "new_cards": 0u64,
                "worsened_cards": 0u64, "resolved_cards": 0u64, "inherited_cards": 0u64,
            },
            "agent_readiness": {
                "ready": 0u64, "requires_witness_receipt": 0u64,
                "needs_human": 0u64, "unsupported": 0u64,
            },
            "scan_cost_range": {
                "elapsed_ms_min": serde_json::Value::Null,
                "elapsed_ms_median": serde_json::Value::Null,
                "elapsed_ms_max": serde_json::Value::Null,
            },
            "human_summary": "ok",
        });
        let sample = serde_json::to_string_pretty(&rollup)
            .map_err(|err| format!("serialise failed: {err}"))?;
        let path = unique_temp_path("corpus-usefulness-test-bad-tb")?;
        fs::write(&path, &sample).map_err(|err| format!("write test sample failed: {err}"))?;
        let result = check_schema(&path);
        let _ = fs::remove_file(&path);
        if result.is_err() {
            Ok(())
        } else {
            Err(
                "schema check should have rejected trust_boundary missing 'not calibrated'"
                    .to_string(),
            )
        }
    }

    #[test]
    fn subset_list_is_non_empty_and_documented() -> Result<(), String> {
        if SUBSET.len() < 15 {
            return Err(format!(
                "subset must have at least 15 entries per brief, got {}",
                SUBSET.len()
            ));
        }
        for (name, rationale) in SUBSET {
            if name.is_empty() {
                return Err("fixture name must not be empty".to_string());
            }
            if rationale.is_empty() {
                return Err(format!("rationale for {name} must not be empty"));
            }
        }
        Ok(())
    }

    #[test]
    fn rollup_build_aggregates_correctly() -> Result<(), String> {
        let tel = serde_json::json!({
            "schema_version": "usefulness-telemetry/v1",
            "card_inventory": {
                "total_cards": 2u64,
                "actionable_cards": 1u64,
                "new_cards": 2u64,
                "worsened_cards": 0u64,
                "resolved_cards": 0u64,
                "inherited_cards": 0u64,
            },
            "coverage_slots": {
                "contract_missing": 1u64,
                "contract_weak": 0u64,
                "guard_missing": 2u64,
                "guard_weak": 0u64,
                "test_reach_missing": 2u64,
                "test_reach_weak": 0u64,
                "witness_receipt_missing": 2u64,
            },
            "agent_readiness": {
                "ready": 0u64,
                "requires_witness_receipt": 0u64,
                "needs_human": 0u64,
                "unsupported": 2u64,
            },
            "comment_selection": {
                "selected_count": 1u64,
                "not_selected_count": 1u64,
                "not_selected_reason_histogram": {"lower_relevance": 1u64},
                "not_selected_class_histogram": {"lower_relevance/raw_ptr_deref": 1u64},
            },
            "unfulfilled_obligation_count": 5u64,
            "scan_cost": {
                "elapsed_ms": 200u64,
                "output_bytes_total": 4096u64,
            },
            "trust_boundary": "test",
        });

        let runs = vec![RunResult {
            fixture: "raw_pointer_alignment".to_string(),
            rationale: "positive single-gap".to_string(),
            status: "completed".to_string(),
            telemetry: Some(tel),
            elapsed_ms: 350,
        }];

        let rollup = build_rollup(&runs);

        if rollup["card_inventory"]["total_cards"].as_u64() != Some(2) {
            return Err(format!(
                "expected total_cards=2, got {:?}",
                rollup["card_inventory"]["total_cards"]
            ));
        }
        if rollup["agent_readiness"]["unsupported"].as_u64() != Some(2) {
            return Err(format!(
                "expected unsupported=2, got {:?}",
                rollup["agent_readiness"]["unsupported"]
            ));
        }
        if rollup["unfulfilled_obligation_count"].as_u64() != Some(5) {
            return Err(format!(
                "expected unfulfilled_obligation_count=5, got {:?}",
                rollup["unfulfilled_obligation_count"]
            ));
        }
        if rollup["not_selected_reason_histogram"]["lower_relevance"].as_u64() != Some(1) {
            return Err(format!(
                "expected not_selected_reason_histogram.lower_relevance=1, got {:?}",
                rollup["not_selected_reason_histogram"]["lower_relevance"]
            ));
        }
        // elapsed_ms samples should reflect the CLI-measured 350ms
        if rollup["scan_cost_range"]["elapsed_ms_min"].as_u64() != Some(350) {
            return Err(format!(
                "expected elapsed_ms_min=350, got {:?}",
                rollup["scan_cost_range"]["elapsed_ms_min"]
            ));
        }
        Ok(())
    }
}
