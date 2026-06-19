//! `check-evidence-loss-challenges` gate: validates controlled evidence-loss
//! transformations against deterministic fixture inputs.
//!
//! Challenge cases are generated under `target/evidence-loss-challenges/`, then
//! scanned with `unsafe-review check` and `unsafe-review first-pr`. Assertions
//! are count and field invariants, not byte-golden diffs.
//!
//! Trust boundary: evidence-loss challenges show that known review-evidence
//! losses are detected on bounded inputs. They are not recall measurements,
//! memory-safety proof, UB-free claims, Miri-clean claims, site-execution
//! results, or policy-readiness claims.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Component, Path};
use std::process::Command;

const LEDGER: &str = "policy/evidence-loss-challenges.toml";
const SCHEMA_VERSION: &str = "1.0";
const WORK_DIR: &str = "target/evidence-loss-challenges";

const REMOVE_SAFETY_DOC_REPLACEMENT: &str =
    "/// Caller must ensure the pointer is valid and properly aligned.\n";
const REMOVE_SAFETY_COMMENT_BLOCK: &str = "    // SAFETY: caller guarantees the invariants documented in the function-level\n    // # Safety section.\n";
const REMOVE_SAFETY_DIFF: &str = "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -5,17 +5,6 @@\n /// Read config from a raw pointer.\n ///\n-/// # Safety\n-///\n-/// The caller must ensure `ptr` is non-null, properly aligned for `Config`,\n-/// points to an initialized `Config` value that remains live for this call,\n-/// and that the access stays within one allocation.\n-///\n-/// This PR adds the safety contract above; the underlying unsafe dereference\n-/// is unchanged.  The coverage slot for contract moves from `missing` to\n-/// `present` -- an evidence improvement, NOT a safety proof or resolution.\n-/// The unsafe site remains open and advisory.\n+/// Caller must ensure the pointer is valid and properly aligned.\n pub fn read_config(ptr: *const Config) -> Config {\n-    // SAFETY: caller guarantees the invariants documented in the function-level\n-    // # Safety section.\n     unsafe { *ptr }\n }\n";

struct ChallengeCase {
    id: String,
    source_fixture: String,
    diff: String,
    transformation: String,
    baseline_contract_coverage: Option<String>,
    expected: Expected,
    no_new_debt_exit_code: Option<i32>,
}

struct Expected {
    cards: u64,
    new_gaps: u64,
    worsened_gaps: u64,
    improved_gaps: u64,
    resolved_gaps: u64,
    inherited_gaps: u64,
    selected_count: u64,
    not_selected_count: u64,
    operation_family: Option<String>,
    class: Option<String>,
    baseline_state: Option<String>,
    outcome_movement: Option<String>,
}

pub(crate) fn check() -> Result<(), String> {
    let cases = parse_ledger(Path::new(LEDGER))?;
    if cases.is_empty() {
        return Err(format!(
            "{LEDGER} must declare at least one [[challenge]] case"
        ));
    }

    let mut errors = Vec::new();
    for case in &cases {
        if let Err(err) = run_case(case) {
            errors.push(err);
        }
    }

    if !errors.is_empty() {
        return Err(format!(
            "check-evidence-loss-challenges: {} case(s) failed:\n{}",
            errors.len(),
            errors.join("\n")
        ));
    }

    println!(
        "check-evidence-loss-challenges: ok ({} case(s) passed)",
        cases.len()
    );
    Ok(())
}

fn run_case(case: &ChallengeCase) -> Result<(), String> {
    let work_root = Path::new(WORK_DIR).join(&case.id);
    let root = work_root.join("root");
    if work_root.exists() {
        fs::remove_dir_all(&work_root).map_err(|err| {
            format!(
                "check-evidence-loss-challenges case `{}`: remove {} failed: {err}",
                case.id,
                work_root.display()
            )
        })?;
    }
    copy_dir_all(Path::new(&case.source_fixture), &root)?;
    apply_transformation(case, &root)?;

    let diff = root.join(&case.diff);
    let check_stdout = run_unsafe_review_capture([
        os("check"),
        os("--root"),
        root.as_os_str().to_os_string(),
        os("--diff"),
        diff.as_os_str().to_os_string(),
        os("--format"),
        os("json"),
    ])
    .map_err(|err| {
        format!(
            "check-evidence-loss-challenges case `{}`: advisory check failed: {err}",
            case.id
        )
    })?;
    let check_json: serde_json::Value = serde_json::from_str(&check_stdout).map_err(|err| {
        format!(
            "check-evidence-loss-challenges case `{}`: advisory check JSON parse failed: {err}",
            case.id
        )
    })?;
    assert_check_json(case, &check_json)?;

    let out_dir = work_root.join("first-pr");
    fs::create_dir_all(&out_dir).map_err(|err| {
        format!(
            "check-evidence-loss-challenges case `{}`: create {} failed: {err}",
            case.id,
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
    .map_err(|err| {
        format!(
            "check-evidence-loss-challenges case `{}`: first-pr failed: {err}",
            case.id
        )
    })?;
    let comment_plan_path = out_dir.join("comment-plan.json");
    let comment_plan_text = fs::read_to_string(&comment_plan_path).map_err(|err| {
        format!(
            "check-evidence-loss-challenges case `{}`: read {} failed: {err}",
            case.id,
            comment_plan_path.display()
        )
    })?;
    let comment_plan: serde_json::Value =
        serde_json::from_str(&comment_plan_text).map_err(|err| {
            format!(
                "check-evidence-loss-challenges case `{}`: comment-plan JSON parse failed: {err}",
                case.id
            )
        })?;
    assert_comment_plan(case, &comment_plan)?;

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
            format!(
                "check-evidence-loss-challenges case `{}`: no-new-debt check failed to run: {err}",
                case.id
            )
        })?;

        if actual_exit != expected_exit {
            return Err(format!(
                "check-evidence-loss-challenges case `{}`: no_new_debt_exit_code mismatch: expected={expected_exit} actual={actual_exit}",
                case.id
            ));
        }
    }

    let _ = fs::remove_dir_all(&work_root);
    Ok(())
}

fn apply_transformation(case: &ChallengeCase, root: &Path) -> Result<(), String> {
    if case.transformation != "remove-safety-section" {
        return Err(format!(
            "{LEDGER} challenge `{}` uses unsupported transformation `{}`",
            case.id, case.transformation
        ));
    }

    let src = root.join("src").join("lib.rs");
    let text =
        fs::read_to_string(&src).map_err(|err| format!("read {} failed: {err}", src.display()))?;
    let text = normalize_lf(&text);
    let text = remove_safety_doc_section(
        &text,
        REMOVE_SAFETY_DOC_REPLACEMENT,
        &format!("{} src/lib.rs safety doc block", case.id),
    )?;
    let text = replace_once(
        &text,
        REMOVE_SAFETY_COMMENT_BLOCK,
        "",
        &format!("{} src/lib.rs SAFETY comment block", case.id),
    )?;
    fs::write(&src, text).map_err(|err| format!("write {} failed: {err}", src.display()))?;

    if let Some(contract_coverage) = &case.baseline_contract_coverage {
        let snapshot = root
            .join("policy")
            .join("unsafe-review-baseline-snapshot.toml");
        let snapshot_text = fs::read_to_string(&snapshot)
            .map_err(|err| format!("read {} failed: {err}", snapshot.display()))?;
        let replacement = format!("contract_coverage = \"{contract_coverage}\"");
        let snapshot_text = replace_once(
            &snapshot_text,
            "contract_coverage = \"missing\"",
            &replacement,
            &format!("{} baseline contract coverage", case.id),
        )?;
        fs::write(&snapshot, snapshot_text)
            .map_err(|err| format!("write {} failed: {err}", snapshot.display()))?;
    }

    let diff_path = root.join(&case.diff);
    fs::write(&diff_path, REMOVE_SAFETY_DIFF)
        .map_err(|err| format!("write {} failed: {err}", diff_path.display()))?;
    Ok(())
}

fn assert_check_json(case: &ChallengeCase, value: &serde_json::Value) -> Result<(), String> {
    let fields = [
        ("cards", case.expected.cards),
        ("new_gaps", case.expected.new_gaps),
        ("worsened_gaps", case.expected.worsened_gaps),
        ("improved_gaps", case.expected.improved_gaps),
        ("resolved_gaps", case.expected.resolved_gaps),
        ("inherited_gaps", case.expected.inherited_gaps),
    ];
    for (field, expected) in fields {
        let pointer = format!("/summary/{field}");
        let actual = value
            .pointer(&pointer)
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| {
                format!(
                    "check-evidence-loss-challenges case `{}`: missing `{pointer}`",
                    case.id
                )
            })?;
        if actual != expected {
            return Err(format!(
                "check-evidence-loss-challenges case `{}`: summary.{field} mismatch: expected={expected} actual={actual}",
                case.id
            ));
        }
    }

    let cards = value
        .get("cards")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            format!(
                "check-evidence-loss-challenges case `{}`: missing cards array",
                case.id
            )
        })?;
    if let Some(expected_family) = &case.expected.operation_family {
        assert_first_card_str(case, cards, "operation_family", expected_family)?;
    }
    if let Some(expected_class) = &case.expected.class {
        assert_first_card_str(case, cards, "class", expected_class)?;
    }
    if let Some(expected_baseline_state) = &case.expected.baseline_state {
        assert_first_card_coverage_str(case, cards, "baseline_state", expected_baseline_state)?;
    }
    if let Some(expected_outcome_movement) = &case.expected.outcome_movement {
        assert_first_card_coverage_str(case, cards, "outcome_movement", expected_outcome_movement)?;
    }
    Ok(())
}

fn assert_comment_plan(case: &ChallengeCase, value: &serde_json::Value) -> Result<(), String> {
    for (field, expected) in [
        ("selected_count", case.expected.selected_count),
        ("not_selected_count", case.expected.not_selected_count),
    ] {
        let pointer = format!("/summary/{field}");
        let actual = value
            .pointer(&pointer)
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| {
                format!(
                    "check-evidence-loss-challenges case `{}`: comment-plan missing `{pointer}`",
                    case.id
                )
            })?;
        if actual != expected {
            return Err(format!(
                "check-evidence-loss-challenges case `{}`: comment-plan summary.{field} mismatch: expected={expected} actual={actual}",
                case.id
            ));
        }
    }
    Ok(())
}

fn assert_first_card_str(
    case: &ChallengeCase,
    cards: &[serde_json::Value],
    field: &str,
    expected: &str,
) -> Result<(), String> {
    let actual = cards
        .first()
        .and_then(|card| card.get(field))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            format!(
                "check-evidence-loss-challenges case `{}`: first card missing `{field}`",
                case.id
            )
        })?;
    if actual != expected {
        return Err(format!(
            "check-evidence-loss-challenges case `{}`: first card `{field}` mismatch: expected={expected} actual={actual}",
            case.id
        ));
    }
    Ok(())
}

fn assert_first_card_coverage_str(
    case: &ChallengeCase,
    cards: &[serde_json::Value],
    field: &str,
    expected: &str,
) -> Result<(), String> {
    let actual = cards
        .first()
        .and_then(|card| card.get("coverage"))
        .and_then(|coverage| coverage.get(field))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            format!(
                "check-evidence-loss-challenges case `{}`: first card coverage missing `{field}`",
                case.id
            )
        })?;
    if actual != expected {
        return Err(format!(
            "check-evidence-loss-challenges case `{}`: first card coverage `{field}` mismatch: expected={expected} actual={actual}",
            case.id
        ));
    }
    Ok(())
}

fn parse_ledger(path: &Path) -> Result<Vec<ChallengeCase>, String> {
    let text =
        fs::read_to_string(path).map_err(|err| format!("read {} failed: {err}", path.display()))?;
    let doc: toml::Value = text
        .parse::<toml::Table>()
        .map(toml::Value::Table)
        .map_err(|err| format!("{} is not valid TOML: {err}", path.display()))?;
    let schema = doc
        .get("schema_version")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("{} missing string `schema_version`", path.display()))?;
    if schema != SCHEMA_VERSION {
        return Err(format!(
            "{} unsupported schema_version `{schema}`; expected `{SCHEMA_VERSION}`",
            path.display()
        ));
    }

    let ledger_path = path.display().to_string();
    let entries = match doc.get("challenge") {
        Some(toml::Value::Array(arr)) => arr,
        Some(_) => {
            return Err(format!(
                "{ledger_path} `challenge` must be an array of tables"
            ));
        }
        None => return Ok(Vec::new()),
    };

    let mut cases = Vec::new();
    for (idx, entry) in entries.iter().enumerate() {
        let table = entry
            .as_table()
            .ok_or_else(|| format!("{ledger_path} challenge[{idx}] must be a table"))?;
        let id = required_str(table, "id", &ledger_path, idx)?;
        validate_id(&id)
            .map_err(|err| format!("{ledger_path} challenge[{idx}] id `{id}` is invalid: {err}"))?;
        let kind = required_str(table, "kind", &ledger_path, idx)?;
        if kind != "fixture-transform" {
            return Err(format!(
                "{ledger_path} challenge[{idx}] ({id}) kind `{kind}` is not supported"
            ));
        }
        let source_fixture = required_str(table, "source_fixture", &ledger_path, idx)?;
        if !source_fixture.starts_with("fixtures/") {
            return Err(format!(
                "{ledger_path} challenge[{idx}] ({id}) source_fixture must live under fixtures/"
            ));
        }
        validate_relative_path(
            &source_fixture,
            &format!("{ledger_path} challenge[{idx}] ({id}) source_fixture"),
        )?;
        let diff = required_str(table, "diff", &ledger_path, idx)?;
        validate_relative_path(
            &diff,
            &format!("{ledger_path} challenge[{idx}] ({id}) diff"),
        )?;
        let transformation = required_str(table, "transformation", &ledger_path, idx)?;
        let baseline_contract_coverage = table
            .get("baseline_contract_coverage")
            .and_then(toml::Value::as_str)
            .map(str::to_string);
        let expected_table = table
            .get("expected")
            .and_then(toml::Value::as_table)
            .ok_or_else(|| {
                format!("{ledger_path} challenge[{idx}] ({id}) missing `[challenge.expected]`")
            })?;
        let expected = parse_expected(&id, &ledger_path, idx, expected_table)?;
        let no_new_debt_exit_code = table
            .get("no_new_debt_exit_code")
            .and_then(toml::Value::as_integer)
            .map(|v| {
                i32::try_from(v).map_err(|err| {
                    format!(
                        "{ledger_path} challenge[{idx}] ({id}) `no_new_debt_exit_code` value `{v}` is outside the i32 range: {err}"
                    )
                })
            })
            .transpose()?;

        cases.push(ChallengeCase {
            id,
            source_fixture,
            diff,
            transformation,
            baseline_contract_coverage,
            expected,
            no_new_debt_exit_code,
        });
    }
    Ok(cases)
}

fn parse_expected(
    id: &str,
    ledger_path: &str,
    idx: usize,
    table: &toml::map::Map<String, toml::Value>,
) -> Result<Expected, String> {
    Ok(Expected {
        cards: required_u64(table, "cards", id, ledger_path, idx)?,
        new_gaps: required_u64(table, "new_gaps", id, ledger_path, idx)?,
        worsened_gaps: required_u64(table, "worsened_gaps", id, ledger_path, idx)?,
        improved_gaps: required_u64(table, "improved_gaps", id, ledger_path, idx)?,
        resolved_gaps: required_u64(table, "resolved_gaps", id, ledger_path, idx)?,
        inherited_gaps: required_u64(table, "inherited_gaps", id, ledger_path, idx)?,
        selected_count: required_u64(table, "selected_count", id, ledger_path, idx)?,
        not_selected_count: required_u64(table, "not_selected_count", id, ledger_path, idx)?,
        operation_family: optional_str(table, "operation_family"),
        class: optional_str(table, "class"),
        baseline_state: optional_str(table, "baseline_state"),
        outcome_movement: optional_str(table, "outcome_movement"),
    })
}

fn optional_str(table: &toml::map::Map<String, toml::Value>, key: &str) -> Option<String> {
    table
        .get(key)
        .and_then(toml::Value::as_str)
        .map(str::to_string)
}

fn required_str(
    table: &toml::map::Map<String, toml::Value>,
    key: &str,
    ledger_path: &str,
    idx: usize,
) -> Result<String, String> {
    table
        .get(key)
        .and_then(toml::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("{ledger_path} challenge[{idx}] missing non-empty `{key}`"))
}

fn required_u64(
    table: &toml::map::Map<String, toml::Value>,
    key: &str,
    id: &str,
    ledger_path: &str,
    idx: usize,
) -> Result<u64, String> {
    let value = table
        .get(key)
        .and_then(toml::Value::as_integer)
        .ok_or_else(|| {
            format!("{ledger_path} challenge[{idx}] ({id}) [expected] missing integer `{key}`")
        })?;
    if value < 0 {
        return Err(format!(
            "{ledger_path} challenge[{idx}] ({id}) [expected] `{key}` must be non-negative"
        ));
    }
    Ok(value as u64)
}

fn validate_id(id: &str) -> Result<(), &'static str> {
    if id.is_empty() {
        return Err("id must not be empty");
    }
    if id.starts_with('-') || id.ends_with('-') || id.split('-').any(str::is_empty) {
        return Err("id must use non-empty hyphen-separated segments");
    }
    if !id
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
    {
        return Err("id must contain only lowercase ASCII letters, digits, and `-`");
    }
    Ok(())
}

fn validate_relative_path(value: &str, context: &str) -> Result<(), String> {
    let path = Path::new(value);
    if path.is_absolute() {
        return Err(format!("{context} must be relative, got `{value}`"));
    }
    for component in path.components() {
        match component {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!(
                    "{context} must stay inside the generated challenge root, got `{value}`"
                ));
            }
            Component::CurDir | Component::Normal(_) => {}
        }
    }
    Ok(())
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|err| format!("create {} failed: {err}", dst.display()))?;
    for entry in fs::read_dir(src).map_err(|err| format!("read {} failed: {err}", src.display()))? {
        let entry = entry.map_err(|err| format!("read {} entry failed: {err}", src.display()))?;
        let ty = entry
            .file_type()
            .map_err(|err| format!("file type {} failed: {err}", entry.path().display()))?;
        let target = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else if ty.is_file() {
            fs::copy(entry.path(), &target).map_err(|err| {
                format!(
                    "copy {} to {} failed: {err}",
                    entry.path().display(),
                    target.display()
                )
            })?;
        } else if ty.is_symlink() {
            return Err(format!(
                "fixture setup refuses to copy symlink {} to {}",
                entry.path().display(),
                target.display()
            ));
        }
    }
    Ok(())
}

fn replace_once(text: &str, from: &str, to: &str, context: &str) -> Result<String, String> {
    let Some(index) = text.find(from) else {
        return Err(format!("{context}: expected text block was not found"));
    };
    let mut out = String::with_capacity(text.len() - from.len() + to.len());
    out.push_str(&text[..index]);
    out.push_str(to);
    out.push_str(&text[index + from.len()..]);
    Ok(out)
}

fn remove_safety_doc_section(
    text: &str,
    replacement: &str,
    context: &str,
) -> Result<String, String> {
    let start = text
        .find("/// # Safety\n")
        .ok_or_else(|| format!("{context}: expected `# Safety` doc section was not found"))?;
    let section = &text[start..];
    let end_marker = "/// The unsafe site remains open and advisory.\n";
    let end = section
        .find(end_marker)
        .map(|idx| start + idx + end_marker.len())
        .ok_or_else(|| format!("{context}: expected safety doc section end was not found"))?;
    let mut out = String::with_capacity(text.len() - (end - start) + replacement.len());
    out.push_str(&text[..start]);
    out.push_str(replacement);
    out.push_str(&text[end..]);
    Ok(out)
}

fn normalize_lf(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn run_unsafe_review_capture(args: impl IntoIterator<Item = OsString>) -> Result<String, String> {
    let output = run_unsafe_review(args)?;
    let exit_code = output.status.code().unwrap_or(-1);
    if !matches!(exit_code, 0 | 1) {
        return Err(format!(
            "unsafe-review exited with {exit_code}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn run_unsafe_review_silent(args: impl IntoIterator<Item = OsString>) -> Result<(), String> {
    let output = run_unsafe_review(args)?;
    if !output.status.success() {
        return Err(format!(
            "unsafe-review exited with {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

fn run_unsafe_review_exit_code(args: impl IntoIterator<Item = OsString>) -> Result<i32, String> {
    let output = run_unsafe_review(args)?;
    Ok(output.status.code().unwrap_or(-1))
}

fn run_unsafe_review(
    args: impl IntoIterator<Item = OsString>,
) -> Result<std::process::Output, String> {
    let args: Vec<OsString> = args.into_iter().collect();
    Command::new(cargo_program())
        .args(["run", "--locked", "-p", "unsafe-review", "--"])
        .args(&args)
        .output()
        .map_err(|err| format!("failed to spawn unsafe-review: {err}"))
}

fn os(value: &str) -> OsString {
    OsString::from(value)
}

fn cargo_program() -> OsString {
    std::env::var_os("CARGO").unwrap_or_else(|| OsStr::new("cargo").to_os_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replace_once_rejects_missing_block() {
        let err = replace_once("abc", "missing", "", "test")
            .err()
            .unwrap_or_default();
        assert!(err.contains("expected text block"));
    }

    #[test]
    fn normalize_lf_converts_windows_newlines() {
        assert_eq!(normalize_lf("a\r\nb\rc\n"), "a\nb\nc\n");
    }

    #[test]
    fn relative_path_validation_rejects_parent_escape() {
        let err = validate_relative_path("../change.diff", "test")
            .err()
            .unwrap_or_default();
        assert!(err.contains("generated challenge root"), "{err}");
    }
}
