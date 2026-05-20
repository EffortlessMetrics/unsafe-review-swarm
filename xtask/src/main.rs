#![forbid(unsafe_code)]
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const REQUIRED_DOCS: &[&str] = &[
    "README.md",
    "docs/MISSION.md",
    "docs/ROADMAP.md",
    "docs/specs/README.md",
    "docs/status/SUPPORT_SUMMARY.md",
    "docs/status/SUPPORT_TIERS.md",
];
const FRONT_DOOR_MARKDOWN_DOCS: &[&str] = &[
    "README.md",
    "docs/README.md",
    "docs/FIRST_USE.md",
    "docs/CLI.md",
    "crates/unsafe-review/README.md",
    "crates/unsafe-review-cli/README.md",
    "crates/unsafe-review-core/README.md",
];

const POLICY_FILES: &[&str] = &[
    "policy/unsafe-review.toml",
    "policy/unsafe-review-baseline.toml",
    "policy/unsafe-review-suppressions.toml",
    "policy/clippy-lints.toml",
    "policy/clippy-exceptions.toml",
    "policy/no-panic-allowlist.toml",
    "policy/non-rust-allowlist.toml",
    "policy/generated-allowlist.toml",
    "policy/executable-allowlist.toml",
    "policy/workflow-allowlist.toml",
    "policy/process-allowlist.toml",
    "policy/network-allowlist.toml",
];
const WORKFLOW_ALLOWLIST: &str = "policy/workflow-allowlist.toml";
const WORKFLOW_DIR: &str = ".github/workflows";

const FIXTURE_REQUIRED_FILES: &[&str] = &["Cargo.toml", "change.diff", "src/lib.rs"];

const FIXTURE_EXPECTED_CARDS_EXCEPTIONS: &[&str] = &[
    "duplicate_raw_pointer_reads",
    "raw_pointer_alignment_line_drift",
];

const FIXTURE_PACKAGE_PREFIX_EXCEPTIONS: &[(&str, &str)] =
    &[("raw_pointer_alignment_line_drift", "raw-pointer-alignment")];

const CALIBRATION_REQUIRED_KINDS: &[&str] = &["positive", "negative", "false_positive_control"];
const CALIBRATION_CASE_FIELDS: &[&str] = &[
    "fixture",
    "kind",
    "claim",
    "support_tier",
    "expected_cards",
    "expected_class",
    "expected_operation_family",
    "expected_hazard",
];
const OPERATION_FAMILY_REGISTRY: &str =
    "docs/specs/appendices/UNSAFE-REVIEW-SPEC-0005-appendix-operation-family-registry.md";
const OPERATION_FAMILY_REGISTRY_COLUMNS: usize = 9;
const OPERATION_FAMILY_REGISTRY_HEADER: &[&str] = &[
    "operation_family",
    "detected syntax shapes",
    "hazards",
    "not hazards",
    "obligation / evidence keys",
    "witness route",
    "fixture proof",
    "known false-positive controls",
    "known limits",
];
const OPERATION_FAMILY_REGISTRY_REQUIRED_TEXT_COLUMNS: &[(usize, &str)] = &[
    (1, "detected syntax shapes"),
    (7, "known false-positive controls"),
    (8, "known limits"),
];
const OPERATION_FAMILY_REGISTRY_OBLIGATION_KEYS_COLUMN: (usize, &str) =
    (4, "obligation / evidence keys");
const OPERATION_FAMILY_SOURCE: &str = "crates/unsafe-review-core/src/domain/operation.rs";
const SAFETY_OBLIGATION_SOURCE: &str = "crates/unsafe-review-core/src/analysis/obligations.rs";
const HAZARD_KIND_SOURCE: &str = "crates/unsafe-review-core/src/domain/hazard.rs";
const WITNESS_KIND_SOURCE: &str = "crates/unsafe-review-core/src/domain/witness.rs";
const ZERO_CARD_EXPECTATION_FIELDS: &[&str] = &[
    "expected_class",
    "expected_operation_family",
    "expected_hazard",
];

const SUPPORT_TIERS_DOC: &str = "docs/status/SUPPORT_TIERS.md";
const SUPPORT_SUMMARY_DOC: &str = "docs/status/SUPPORT_SUMMARY.md";
const KNOWN_SUPPORT_TIERS: &[&str] = &["scaffold", "experimental", "planned", "deferred"];
const SUPPORT_PROOF_TERMS: &[&str] = &[
    "test",
    "tests",
    "fixture",
    "fixtures",
    "golden",
    "goldens",
    "e2e",
    "xtask",
    "workflow",
    "handoff",
    "dogfood",
    "parser",
    "renderer",
    "manifest",
    "serde",
    "round-trip",
    "adr",
];
const KNOWN_SUPPORT_SUMMARY_POSTURES: &[&str] = &["Experimental", "Deferred or planned"];
const SUPPORT_SUMMARY_REQUIRED_PHRASES: &[&str] = &[
    "memory-safety proof",
    "UB-free claim",
    "Miri-clean claim",
    "site-execution proof",
    "calibrated policy gate",
    "SUPPORT_TIERS.md",
];
const DOGFOOD_MANIFEST: &str = "docs/dogfood/corpus.toml";
const DOGFOOD_INDEX: &str = "docs/dogfood/index.json";
const DOGFOOD_TARGET_KINDS: &[&str] = &["repo-snapshot", "pr-diff"];
const DOGFOOD_TARGET_STATUSES: &[&str] = &["active", "parked", "retired"];
const DOGFOOD_ARTIFACT_STATUSES: &[&str] = &["checked_in", "local_untracked", "remote_manual"];
const MAX_COMMENT_PLAN_COMMENTS: usize = 3;
const FUZZ_REQUIRED_FILES: &[&str] = &[
    "docs/FUZZING.md",
    "fuzz/.gitignore",
    "fuzz/Cargo.lock",
    "fuzz/Cargo.toml",
    "fuzz/corpus/analyze/basic",
    "fuzz/fuzz_targets/analyze.rs",
];
const PUBLIC_BADGE_ENDPOINTS: &[(&str, &str)] = &[
    ("badges/unsafe-review.json", "unsafe-review"),
    ("badges/unsafe-review-plus.json", "unsafe-review+"),
];

fn main() {
    if let Err(err) = run(std::env::args().collect()) {
        eprintln!("xtask: {err}");
        std::process::exit(2);
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    let root = workspace_root()?;
    std::env::set_current_dir(&root)
        .map_err(|err| format!("failed to enter workspace root {}: {err}", root.display()))?;

    match args.get(1).map(|arg| arg.as_str()) {
        None | Some("help") | Some("--help") => {
            println!(
                "xtask commands: check-pr, check-docs, check-policy, check-support-tiers, check-fixtures, check-calibration, check-dogfood, check-fuzz, check-advisory-artifacts <dir>, check-first-pr-artifacts <dir>"
            );
            Ok(())
        }
        Some("check-pr") => {
            require_no_extra_args(&args, "check-pr")?;
            check_docs()?;
            check_policy()?;
            check_support_tiers()?;
            check_fixtures()?;
            check_calibration()?;
            check_dogfood()?;
            check_manual_fuzz_harness()?;
            check_tracked_generated_artifacts()?;
            println!("check-pr: ok");
            Ok(())
        }
        Some("check-docs") => {
            require_no_extra_args(&args, "check-docs")?;
            check_docs()
        }
        Some("check-policy") => {
            require_no_extra_args(&args, "check-policy")?;
            check_policy()
        }
        Some("check-support-tiers") => {
            require_no_extra_args(&args, "check-support-tiers")?;
            check_support_tiers()
        }
        Some("check-fixtures") => {
            require_no_extra_args(&args, "check-fixtures")?;
            check_fixtures()
        }
        Some("check-calibration") => {
            require_no_extra_args(&args, "check-calibration")?;
            check_calibration()
        }
        Some("check-dogfood") => {
            require_no_extra_args(&args, "check-dogfood")?;
            check_dogfood()
        }
        Some("check-fuzz") => {
            require_no_extra_args(&args, "check-fuzz")?;
            check_manual_fuzz_harness()
        }
        Some("check-advisory-artifacts") => {
            let Some(dir) = args.get(2) else {
                return Err("usage: cargo xtask check-advisory-artifacts <dir>".to_string());
            };
            require_max_args(&args, "check-advisory-artifacts", 3)?;
            check_advisory_artifacts(Path::new(dir))
        }
        Some("check-first-pr-artifacts") => {
            let Some(dir) = args.get(2) else {
                return Err("usage: cargo xtask check-first-pr-artifacts <dir>".to_string());
            };
            require_max_args(&args, "check-first-pr-artifacts", 3)?;
            check_first_pr_artifacts(Path::new(dir))
        }
        Some(other) => Err(format!("unknown xtask command `{other}`")),
    }
}

fn workspace_root() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "failed to resolve workspace root from xtask manifest path".to_string())
}

fn require_no_extra_args(args: &[String], command: &str) -> Result<(), String> {
    require_max_args(args, command, 2)
}

fn require_max_args(args: &[String], command: &str, max_len: usize) -> Result<(), String> {
    if args.len() <= max_len {
        return Ok(());
    }
    Err(format!(
        "`{command}` does not accept extra arguments: {}",
        args[max_len..].join(" ")
    ))
}

fn check_docs() -> Result<(), String> {
    for path in REQUIRED_DOCS {
        require_file(path)?;
    }
    for path in FRONT_DOOR_MARKDOWN_DOCS {
        check_markdown_local_links(path)?;
    }
    check_public_badge_endpoints()?;
    check_docs_map_paths("docs/README.md")?;
    check_index(
        Path::new("docs/specs"),
        Path::new("docs/specs/README.md"),
        "UNSAFE-REVIEW-SPEC-",
    )?;
    check_index(
        Path::new("docs/adr"),
        Path::new("docs/adr/README.md"),
        "UNSAFE-REVIEW-ADR-",
    )?;
    check_index(
        Path::new("docs/proposals"),
        Path::new("docs/proposals/README.md"),
        "UNSAFE-REVIEW-PROP-",
    )?;
    check_handoff_index(
        Path::new("docs/handoffs"),
        Path::new("docs/handoffs/README.md"),
    )?;
    check_no_windows_paths(&[
        Path::new("README.md"),
        Path::new("MANIFEST.md"),
        Path::new("docs"),
        Path::new("plans"),
        Path::new(".unsafe-review"),
        Path::new("policy"),
    ])?;
    println!("check-docs: ok");
    Ok(())
}

fn check_policy() -> Result<(), String> {
    for path in POLICY_FILES {
        let value = parse_toml_file(Path::new(path))?;
        require_toml_string(&value, "schema_version", path)?;
    }
    check_workflow_allowlist(Path::new(WORKFLOW_ALLOWLIST), Path::new(WORKFLOW_DIR))?;
    check_unsafe_review_ledger(
        Path::new("policy/unsafe-review-baseline.toml"),
        LedgerKind::Baseline,
    )?;
    check_unsafe_review_ledger(
        Path::new("policy/unsafe-review-suppressions.toml"),
        LedgerKind::Suppression,
    )?;
    parse_toml_file(Path::new(".unsafe-review/goals/active.toml"))?;
    println!("check-policy: ok");
    Ok(())
}

#[derive(Debug)]
struct WorkflowPolicyEntry {
    path: String,
    permissions: String,
    actions: BTreeSet<String>,
}

fn check_workflow_allowlist(allowlist: &Path, workflow_dir: &Path) -> Result<(), String> {
    let policies = workflow_policy_entries(allowlist)?;
    let mut by_path = BTreeMap::new();
    for entry in policies {
        let workflow_path = workspace_path(&entry.path);
        if !workflow_path.is_file() {
            return Err(format!(
                "{} lists missing workflow `{}`",
                allowlist.display(),
                entry.path
            ));
        }
        let text = read_to_string(&workflow_path)?;
        check_workflow_text_against_policy(&entry.path, &text, &entry)?;
        if by_path.insert(entry.path.clone(), entry).is_some() {
            return Err(format!(
                "{} contains duplicate workflow entry",
                allowlist.display()
            ));
        }
    }

    for workflow in workflow_files(workflow_dir)? {
        if !by_path.contains_key(&workflow) {
            return Err(format!(
                "{} is missing workflow allowlist entry for `{workflow}`",
                allowlist.display()
            ));
        }
    }

    Ok(())
}

fn workflow_policy_entries(allowlist: &Path) -> Result<Vec<WorkflowPolicyEntry>, String> {
    let value = parse_toml_file(allowlist)?;
    let path_display = allowlist.display().to_string();
    let entries = value
        .get("workflow")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{path_display} must contain [[workflow]] entries"))?;
    if entries.is_empty() {
        return Err(format!(
            "{path_display} must contain at least one workflow entry"
        ));
    }

    let mut out = Vec::new();
    for (idx, entry) in entries.iter().enumerate() {
        let path = required_toml_string(entry, "path", &format!("{path_display} workflow[{idx}]"))?
            .to_string();
        let permissions = required_toml_string(
            entry,
            "permissions",
            &format!("{path_display} workflow[{idx}]"),
        )?
        .to_string();
        let reason =
            required_toml_string(entry, "reason", &format!("{path_display} workflow[{idx}]"))?;
        if reason.len() < 16 {
            return Err(format!(
                "{path_display} workflow[{idx}] reason is too terse"
            ));
        }
        let review_after = required_toml_string(
            entry,
            "review_after",
            &format!("{path_display} workflow[{idx}]"),
        )?;
        if !looks_like_iso_date(review_after) {
            return Err(format!(
                "{path_display} workflow[{idx}] review_after must use YYYY-MM-DD"
            ));
        }
        let actions = entry
            .get("actions")
            .and_then(toml::Value::as_array)
            .ok_or_else(|| format!("{path_display} workflow[{idx}] is missing actions array"))?;
        let mut action_set = BTreeSet::new();
        for (action_idx, action) in actions.iter().enumerate() {
            let Some(action) = action.as_str() else {
                return Err(format!(
                    "{path_display} workflow[{idx}] actions[{action_idx}] must be a string"
                ));
            };
            if action.trim().is_empty() {
                return Err(format!(
                    "{path_display} workflow[{idx}] actions[{action_idx}] is empty"
                ));
            }
            action_set.insert(action.to_string());
        }
        if action_set.is_empty() {
            return Err(format!(
                "{path_display} workflow[{idx}] must list at least one action"
            ));
        }
        out.push(WorkflowPolicyEntry {
            path,
            permissions,
            actions: action_set,
        });
    }
    Ok(out)
}

fn check_workflow_text_against_policy(
    path: &str,
    text: &str,
    policy: &WorkflowPolicyEntry,
) -> Result<(), String> {
    if !workflow_declares_permission(text, &policy.permissions) {
        return Err(format!(
            "{path} must declare workflow permission `{}`",
            policy.permissions
        ));
    }

    let used_actions = workflow_used_actions(text);
    for action in &used_actions {
        if !policy.actions.contains(action) {
            return Err(format!(
                "{path} uses action `{action}` that is not listed in {WORKFLOW_ALLOWLIST}"
            ));
        }
    }
    for action in &policy.actions {
        if !used_actions.contains(action) {
            return Err(format!(
                "{WORKFLOW_ALLOWLIST} lists action `{action}` for {path}, but the workflow does not use it"
            ));
        }
    }
    Ok(())
}

fn workflow_declares_permission(text: &str, permission: &str) -> bool {
    text.lines().any(|line| line.trim() == "permissions:")
        && text.lines().any(|line| line.trim() == permission)
}

fn workflow_used_actions(text: &str) -> BTreeSet<String> {
    let mut actions = BTreeSet::new();
    for line in text.lines() {
        let trimmed = line.trim();
        let trimmed = trimmed.strip_prefix("- ").unwrap_or(trimmed);
        let Some(raw_action) = trimmed.strip_prefix("uses:") else {
            continue;
        };
        let action = raw_action
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim();
        if !action.is_empty() {
            actions.insert(action.to_string());
        }
    }
    actions
}

fn workflow_files(workflow_dir: &Path) -> Result<BTreeSet<String>, String> {
    let dir = workspace_path(&workflow_dir.display().to_string());
    let entries =
        fs::read_dir(&dir).map_err(|err| format!("read {} failed: {err}", dir.display()))?;
    let mut files = BTreeSet::new();
    for entry in entries {
        let entry = entry.map_err(|err| format!("read_dir entry failed: {err}"))?;
        let path = entry.path();
        let extension = path.extension().and_then(std::ffi::OsStr::to_str);
        if !matches!(extension, Some("yml" | "yaml")) {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(std::ffi::OsStr::to_str) else {
            return Err(format!("non-UTF-8 workflow file name: {}", path.display()));
        };
        files.insert(format!("{WORKFLOW_DIR}/{file_name}"));
    }
    if files.is_empty() {
        return Err(format!("{} contains no workflow files", dir.display()));
    }
    Ok(files)
}

#[derive(Clone, Copy)]
enum LedgerKind {
    Baseline,
    Suppression,
}

impl LedgerKind {
    fn name(self) -> &'static str {
        match self {
            Self::Baseline => "baseline",
            Self::Suppression => "suppression",
        }
    }
}

fn check_unsafe_review_ledger(path: &Path, kind: LedgerKind) -> Result<(), String> {
    let value = parse_toml_file(path)?;
    let path_display = path.display().to_string();
    let status = value
        .get("status")
        .and_then(toml::Value::as_str)
        .unwrap_or("active");
    let entries = value
        .get("entries")
        .and_then(toml::Value::as_array)
        .map_or(&[][..], Vec::as_slice);

    if status == "empty" {
        if entries.is_empty() {
            return Ok(());
        }
        return Err(format!(
            "{path_display} status is empty but contains entries"
        ));
    }

    for (idx, entry) in entries.iter().enumerate() {
        let Some(entry) = entry.as_table() else {
            return Err(format!(
                "{path_display} entries[{idx}] must be a TOML table"
            ));
        };
        for key in ["card_id", "owner", "reason", "evidence"] {
            require_ledger_entry_string(entry, key, &path_display, idx)?;
        }
        let has_review_after = ledger_entry_date(entry, "review_after", &path_display, idx)?;
        let has_expires = ledger_entry_date(entry, "expires", &path_display, idx)?;
        match kind {
            LedgerKind::Baseline if !has_review_after => {
                return Err(format!(
                    "{path_display} entries[{idx}] baseline entry is missing review_after"
                ));
            }
            LedgerKind::Suppression if !has_review_after && !has_expires => {
                return Err(format!(
                    "{path_display} entries[{idx}] suppression entry must set review_after or expires"
                ));
            }
            _ => {}
        }
        let card_id = entry
            .get("card_id")
            .and_then(toml::Value::as_str)
            .unwrap_or_default();
        if !looks_like_counted_card_id(card_id) {
            return Err(format!(
                "{path_display} entries[{idx}] {} card_id must be an exact counted UR-* identity ending in -cN",
                kind.name()
            ));
        }
    }

    Ok(())
}

fn require_ledger_entry_string(
    entry: &toml::map::Map<String, toml::Value>,
    key: &str,
    path: &str,
    idx: usize,
) -> Result<(), String> {
    let Some(value) = entry.get(key).and_then(toml::Value::as_str) else {
        return Err(format!("{path} entries[{idx}] is missing string `{key}`"));
    };
    if value.trim().is_empty() {
        Err(format!("{path} entries[{idx}] string `{key}` is empty"))
    } else {
        Ok(())
    }
}

fn ledger_entry_date(
    entry: &toml::map::Map<String, toml::Value>,
    key: &str,
    path: &str,
    idx: usize,
) -> Result<bool, String> {
    let Some(value) = entry.get(key) else {
        return Ok(false);
    };
    let Some(value) = value.as_str() else {
        return Err(format!("{path} entries[{idx}] `{key}` must be a string"));
    };
    if !looks_like_iso_date(value) {
        return Err(format!("{path} entries[{idx}] `{key}` must use YYYY-MM-DD"));
    }
    Ok(true)
}

fn looks_like_iso_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[0..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'-'
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[7] == b'-'
        && bytes[8..10].iter().all(u8::is_ascii_digit)
}

fn looks_like_counted_card_id(value: &str) -> bool {
    let Some((prefix, count)) = value.rsplit_once("-c") else {
        return false;
    };
    value.starts_with("UR-")
        && !prefix.is_empty()
        && !count.is_empty()
        && count.bytes().all(|byte| byte.is_ascii_digit())
}

fn check_support_tiers() -> Result<(), String> {
    let path = SUPPORT_TIERS_DOC;
    let text = read_to_string(Path::new(path))?;
    check_support_tiers_text(path, &text)?;
    check_support_summary()?;
    println!("check-support-tiers: ok");
    Ok(())
}

fn check_support_tiers_text(path: &str, text: &str) -> Result<(), String> {
    let mut rows = 0usize;
    for (line_no, line) in text.lines().enumerate() {
        let Some(row) = support_tier_row_from_line(line, path, line_no + 1)? else {
            continue;
        };
        rows += 1;
        if !KNOWN_SUPPORT_TIERS.contains(&row.tier) {
            return Err(format!(
                "{path}:{} uses unknown support tier `{}`",
                line_no + 1,
                row.tier
            ));
        }
        if matches!(row.tier, "scaffold" | "experimental")
            && !support_proof_cell_has_evidence_term(row.proof)
        {
            return Err(format!(
                "{path}:{} proof for `{}` must name concrete evidence such as tests, fixtures, dogfood, workflows, or an ADR",
                line_no + 1,
                row.capability
            ));
        }
    }
    if rows == 0 {
        return Err(format!("{path} has no support-tier rows"));
    }
    Ok(())
}

fn check_support_summary() -> Result<(), String> {
    let path = SUPPORT_SUMMARY_DOC;
    let text = read_to_string(Path::new(path))?;
    check_support_summary_text(path, &text)
}

fn check_support_summary_text(path: &str, text: &str) -> Result<(), String> {
    for phrase in SUPPORT_SUMMARY_REQUIRED_PHRASES {
        if !text.contains(phrase) {
            return Err(format!(
                "{path} must include trust-boundary phrase `{phrase}`"
            ));
        }
    }

    let mut rows = 0usize;
    for (line_no, line) in text.lines().enumerate() {
        let Some(posture) = support_summary_posture_from_row(line) else {
            continue;
        };
        rows += 1;
        if !KNOWN_SUPPORT_SUMMARY_POSTURES.contains(&posture) {
            return Err(format!(
                "{path}:{} uses unknown support summary posture `{posture}`",
                line_no + 1
            ));
        }
    }
    if rows == 0 {
        return Err(format!("{path} has no current-posture rows"));
    }
    Ok(())
}

fn check_fixtures() -> Result<(), String> {
    let dirs = fixture_dirs(Path::new("fixtures"))?;
    if dirs.is_empty() {
        return Err("fixtures directory has no fixture cases".to_string());
    }
    check_fixture_exception_ledgers(&dirs)?;
    for dir in &dirs {
        check_fixture(dir)?;
    }
    println!("check-fixtures: ok ({} fixtures)", dirs.len());
    Ok(())
}

fn check_fixture_exception_ledgers(dirs: &[PathBuf]) -> Result<(), String> {
    let mut fixture_paths = BTreeMap::new();
    for dir in dirs {
        let name = fixture_dir_name(dir)?.to_string();
        fixture_paths.insert(name, dir);
    }

    for fixture in FIXTURE_EXPECTED_CARDS_EXCEPTIONS {
        let Some(dir) = fixture_paths.get(*fixture) else {
            return Err(format!(
                "expected-card exception fixture `{fixture}` does not exist"
            ));
        };
        if dir.join("expected.cards.json").is_file() {
            return Err(format!(
                "expected-card exception fixture `{fixture}` has expected.cards.json"
            ));
        }
    }

    for (fixture, _prefix) in FIXTURE_PACKAGE_PREFIX_EXCEPTIONS {
        if !fixture_paths.contains_key(*fixture) {
            return Err(format!(
                "package-prefix exception fixture `{fixture}` does not exist"
            ));
        }
    }

    Ok(())
}

fn check_calibration() -> Result<(), String> {
    let path = workspace_path("fixtures/calibration.toml");
    let value = parse_toml_file(&path)?;
    require_toml_string(&value, "schema_version", "fixtures/calibration.toml")?;
    let required = value
        .get("required_core_fixtures")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| "fixtures/calibration.toml is missing required_core_fixtures".to_string())?;
    let cases = value
        .get("cases")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| "fixtures/calibration.toml is missing cases".to_string())?;
    if cases.is_empty() {
        return Err("fixtures/calibration.toml has no calibration cases".to_string());
    }

    let mut fixtures = BTreeSet::new();
    let mut kinds = BTreeSet::new();
    let mut operation_families = BTreeSet::new();
    let mut operation_family_fixtures = BTreeMap::new();
    let support_capabilities = support_tier_capabilities()?;
    for (idx, case) in cases.iter().enumerate() {
        let Some(case) = case.as_table() else {
            return Err(format!(
                "fixtures/calibration.toml cases[{idx}] must be a TOML table"
            ));
        };
        check_calibration_case_fields(case, idx)?;
        let fixture = required_case_string(case, "fixture", idx)?;
        let kind = required_case_string(case, "kind", idx)?;
        let claim = required_case_string(case, "claim", idx)?;
        let support_tier = required_case_string(case, "support_tier", idx)?;
        if !support_capabilities.contains(support_tier) {
            return Err(format!(
                "fixtures/calibration.toml cases[{idx}] support_tier `{support_tier}` is not a capability in docs/status/SUPPORT_TIERS.md"
            ));
        }
        if !CALIBRATION_REQUIRED_KINDS.contains(&kind) {
            return Err(format!(
                "fixtures/calibration.toml cases[{idx}] uses unknown kind `{kind}`"
            ));
        }
        if claim.len() < 16 {
            return Err(format!(
                "fixtures/calibration.toml cases[{idx}] claim is too terse"
            ));
        }
        if !fixtures.insert(fixture.to_string()) {
            return Err(format!(
                "fixtures/calibration.toml contains duplicate fixture `{fixture}`"
            ));
        }
        kinds.insert(kind.to_string());
        check_calibration_case(case, fixture, kind, idx)?;
        if let Some(operation_family) =
            optional_case_string(case, "expected_operation_family", idx)?
        {
            operation_families.insert(operation_family.to_string());
            operation_family_fixtures
                .entry(operation_family.to_string())
                .or_insert_with(BTreeSet::new)
                .insert(fixture.to_string());
        }
    }
    check_operation_family_registry_coverage(&operation_families, &operation_family_fixtures)?;

    for kind in CALIBRATION_REQUIRED_KINDS {
        if !kinds.contains(*kind) {
            return Err(format!(
                "fixtures/calibration.toml is missing a `{kind}` calibration case"
            ));
        }
    }

    let mut required_fixtures = BTreeSet::new();
    for (idx, fixture) in required.iter().enumerate() {
        let Some(fixture) = fixture.as_str() else {
            return Err(format!(
                "fixtures/calibration.toml required_core_fixtures[{idx}] must be a string"
            ));
        };
        if !required_fixtures.insert(fixture.to_string()) {
            return Err(format!(
                "fixtures/calibration.toml contains duplicate required core fixture `{fixture}`"
            ));
        }
        if !fixtures.contains(fixture) {
            return Err(format!(
                "fixtures/calibration.toml required core fixture `{fixture}` has no case"
            ));
        }
    }
    for fixture in &fixtures {
        if !required_fixtures.contains(fixture) {
            return Err(format!(
                "fixtures/calibration.toml case fixture `{fixture}` is missing from required_core_fixtures"
            ));
        }
    }

    for dir in fixture_dirs(&workspace_path("fixtures"))? {
        let fixture = fixture_dir_name(&dir)?;
        if FIXTURE_EXPECTED_CARDS_EXCEPTIONS.contains(&fixture) {
            continue;
        }
        if dir.join("expected.cards.json").is_file() && !fixtures.contains(fixture) {
            return Err(format!(
                "fixture `{fixture}` has expected.cards.json but no fixtures/calibration.toml case"
            ));
        }
    }

    println!("check-calibration: ok ({} cases)", cases.len());
    Ok(())
}

fn check_dogfood() -> Result<(), String> {
    let value = parse_toml_file(&workspace_path(DOGFOOD_MANIFEST))?;
    require_toml_string(&value, "schema_version", DOGFOOD_MANIFEST)?;
    require_toml_string(&value, "status", DOGFOOD_MANIFEST)?;
    require_toml_string(&value, "artifact_root", DOGFOOD_MANIFEST)?;
    let boundary = required_toml_string(&value, "trust_boundary", DOGFOOD_MANIFEST)?;
    require_boundary_text(boundary, DOGFOOD_MANIFEST)?;

    let targets = value
        .get("targets")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{DOGFOOD_MANIFEST} is missing targets"))?;
    if targets.is_empty() {
        return Err(format!("{DOGFOOD_MANIFEST} has no dogfood targets"));
    }

    let mut ids = BTreeSet::new();
    let mut repositories = BTreeSet::new();
    let mut snapshot_targets_by_repo = BTreeMap::<String, BTreeSet<String>>::new();
    let mut pr_diff_targets_by_repo = BTreeMap::<String, BTreeSet<String>>::new();
    let mut artifact_status_counts = BTreeMap::new();
    let mut repo_snapshots = 0usize;
    let mut pr_diffs = 0usize;
    for (idx, target) in targets.iter().enumerate() {
        let Some(target) = target.as_table() else {
            return Err(format!(
                "{DOGFOOD_MANIFEST} targets[{idx}] must be a TOML table"
            ));
        };
        let id = required_target_string(target, "id", idx)?;
        if !ids.insert(id.to_string()) {
            return Err(format!(
                "{DOGFOOD_MANIFEST} contains duplicate target id `{id}`"
            ));
        }
        let repository = required_target_string(target, "repository", idx)?;
        if !repository.contains('/') {
            return Err(format!(
                "{DOGFOOD_MANIFEST} targets[{idx}] repository `{repository}` must be owner/repo"
            ));
        }
        repositories.insert(repository.to_string());
        snapshot_targets_by_repo
            .entry(repository.to_string())
            .or_default();
        pr_diff_targets_by_repo
            .entry(repository.to_string())
            .or_default();
        required_target_string(target, "crate", idx)?;
        let kind = required_target_string(target, "kind", idx)?;
        if !DOGFOOD_TARGET_KINDS.contains(&kind) {
            return Err(format!(
                "{DOGFOOD_MANIFEST} targets[{idx}] uses unknown kind `{kind}`"
            ));
        }
        let status = required_target_string(target, "status", idx)?;
        if !DOGFOOD_TARGET_STATUSES.contains(&status) {
            return Err(format!(
                "{DOGFOOD_MANIFEST} targets[{idx}] uses unknown status `{status}`"
            ));
        }
        let purpose = required_target_string(target, "purpose", idx)?;
        if purpose.len() < 24 {
            return Err(format!(
                "{DOGFOOD_MANIFEST} targets[{idx}] purpose is too terse"
            ));
        }
        let command = required_target_string(target, "command", idx)?;
        if !command.contains("unsafe-review") || !command.contains("--format json") {
            return Err(format!(
                "{DOGFOOD_MANIFEST} targets[{idx}] command must run unsafe-review JSON output"
            ));
        }
        let artifact_status = required_target_string(target, "artifact_status", idx)?;
        if !DOGFOOD_ARTIFACT_STATUSES.contains(&artifact_status) {
            return Err(format!(
                "{DOGFOOD_MANIFEST} targets[{idx}] uses unknown artifact_status `{artifact_status}`"
            ));
        }
        *artifact_status_counts
            .entry(artifact_status.to_string())
            .or_insert(0usize) += 1;
        let artifacts = target
            .get("artifacts")
            .and_then(toml::Value::as_array)
            .ok_or_else(|| {
                format!("{DOGFOOD_MANIFEST} targets[{idx}] is missing artifacts array")
            })?;
        if artifacts.is_empty() {
            return Err(format!(
                "{DOGFOOD_MANIFEST} targets[{idx}] artifacts array is empty"
            ));
        }
        for (artifact_idx, artifact) in artifacts.iter().enumerate() {
            let Some(artifact) = artifact.as_str() else {
                return Err(format!(
                    "{DOGFOOD_MANIFEST} targets[{idx}] artifacts[{artifact_idx}] must be a string"
                ));
            };
            check_dogfood_path(artifact, idx, "artifacts")?;
            if artifact_status == "checked_in" && !Path::new(artifact).is_file() {
                return Err(format!(
                    "{DOGFOOD_MANIFEST} targets[{idx}] checked-in artifact missing: {artifact}"
                ));
            }
        }

        match kind {
            "repo-snapshot" => {
                repo_snapshots += 1;
                snapshot_targets_by_repo
                    .entry(repository.to_string())
                    .or_default()
                    .insert(id.to_string());
                let commit = required_target_string(target, "commit", idx)?;
                if commit.len() != 40 || !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                    return Err(format!(
                        "{DOGFOOD_MANIFEST} targets[{idx}] commit must be a full 40-character hex SHA"
                    ));
                }
                let root = required_target_string(target, "root", idx)?;
                check_dogfood_path(root, idx, "root")?;
            }
            "pr-diff" => {
                pr_diffs += 1;
                pr_diff_targets_by_repo
                    .entry(repository.to_string())
                    .or_default()
                    .insert(id.to_string());
                let Some(pr) = target.get("pr").and_then(toml::Value::as_integer) else {
                    return Err(format!(
                        "{DOGFOOD_MANIFEST} targets[{idx}] is missing integer pr"
                    ));
                };
                if pr <= 0 {
                    return Err(format!(
                        "{DOGFOOD_MANIFEST} targets[{idx}] pr must be positive"
                    ));
                }
                let root = required_target_string(target, "root", idx)?;
                check_dogfood_path(root, idx, "root")?;
                let diff = required_target_string(target, "diff", idx)?;
                check_dogfood_path(diff, idx, "diff")?;
            }
            _ => {
                return Err(format!(
                    "{DOGFOOD_MANIFEST} targets[{idx}] uses unsupported kind `{kind}`"
                ));
            }
        }
    }

    if repositories.len() < 5 {
        return Err(format!(
            "{DOGFOOD_MANIFEST} must cover at least 5 real repositories"
        ));
    }
    if repo_snapshots == 0 || pr_diffs == 0 {
        return Err(format!(
            "{DOGFOOD_MANIFEST} must include repo-snapshot and pr-diff targets"
        ));
    }
    check_dogfood_index(DogfoodIndexExpected {
        target_count: targets.len(),
        repository_count: repositories.len(),
        repo_snapshots,
        pr_diffs,
        repositories: &repositories,
        snapshot_targets_by_repo: &snapshot_targets_by_repo,
        pr_diff_targets_by_repo: &pr_diff_targets_by_repo,
        artifact_status_counts: &artifact_status_counts,
    })?;

    println!(
        "check-dogfood: ok ({} targets, {} repositories)",
        targets.len(),
        repositories.len()
    );
    Ok(())
}

struct DogfoodIndexExpected<'a> {
    target_count: usize,
    repository_count: usize,
    repo_snapshots: usize,
    pr_diffs: usize,
    repositories: &'a BTreeSet<String>,
    snapshot_targets_by_repo: &'a BTreeMap<String, BTreeSet<String>>,
    pr_diff_targets_by_repo: &'a BTreeMap<String, BTreeSet<String>>,
    artifact_status_counts: &'a BTreeMap<String, usize>,
}

fn check_dogfood_index(expected: DogfoodIndexExpected<'_>) -> Result<(), String> {
    let index = parse_json_file(&workspace_path(DOGFOOD_INDEX))?;
    require_json_str(&index, "schema_version", "0.1", DOGFOOD_INDEX)?;
    require_json_str(&index, "status", "experimental", DOGFOOD_INDEX)?;
    require_json_str(&index, "manifest", DOGFOOD_MANIFEST, DOGFOOD_INDEX)?;
    let boundary = index
        .get("trust_boundary")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("{DOGFOOD_INDEX} is missing trust_boundary"))?;
    require_boundary_text(boundary, DOGFOOD_INDEX)?;
    require_json_usize_at(
        &index,
        "/summary/repositories",
        expected.repository_count,
        DOGFOOD_INDEX,
    )?;
    require_json_usize_at(
        &index,
        "/summary/targets",
        expected.target_count,
        DOGFOOD_INDEX,
    )?;
    require_json_usize_at(
        &index,
        "/summary/repo_snapshots",
        expected.repo_snapshots,
        DOGFOOD_INDEX,
    )?;
    require_json_usize_at(
        &index,
        "/summary/pr_diffs",
        expected.pr_diffs,
        DOGFOOD_INDEX,
    )?;

    for (status, count) in expected.artifact_status_counts {
        require_json_usize_at(
            &index,
            &format!("/summary/artifact_statuses/{status}"),
            *count,
            DOGFOOD_INDEX,
        )?;
    }

    let repository_rows = json_array_at(&index, "/repositories", DOGFOOD_INDEX)?;
    if repository_rows.len() != expected.repository_count {
        return Err(format!(
            "{DOGFOOD_INDEX} repositories has {}, expected {}",
            repository_rows.len(),
            expected.repository_count
        ));
    }
    let mut seen = BTreeSet::new();
    for (idx, row) in repository_rows.iter().enumerate() {
        let Some(repository) = row.get("repository").and_then(serde_json::Value::as_str) else {
            return Err(format!(
                "{DOGFOOD_INDEX} repositories[{idx}] is missing repository"
            ));
        };
        if !expected.repositories.contains(repository) {
            return Err(format!(
                "{DOGFOOD_INDEX} repositories[{idx}] references unknown repository `{repository}`"
            ));
        }
        if !seen.insert(repository.to_string()) {
            return Err(format!(
                "{DOGFOOD_INDEX} repositories contains duplicate `{repository}`"
            ));
        }
        let expected_snapshots = expected
            .snapshot_targets_by_repo
            .get(repository)
            .ok_or_else(|| format!("{DOGFOOD_MANIFEST} has no target map for `{repository}`"))?;
        require_json_string_set_at(row, "/snapshot_targets", expected_snapshots, DOGFOOD_INDEX)?;
        let expected_pr_diffs = expected
            .pr_diff_targets_by_repo
            .get(repository)
            .ok_or_else(|| format!("{DOGFOOD_MANIFEST} has no target map for `{repository}`"))?;
        require_json_string_set_at(row, "/pr_diff_targets", expected_pr_diffs, DOGFOOD_INDEX)?;
        let Some(summary) = row
            .get("primary_exercise")
            .and_then(serde_json::Value::as_str)
        else {
            return Err(format!(
                "{DOGFOOD_INDEX} repositories[{idx}] is missing primary_exercise"
            ));
        };
        if summary.len() < 24 {
            return Err(format!(
                "{DOGFOOD_INDEX} repositories[{idx}] primary_exercise is too terse"
            ));
        }
    }

    if json_array_at(&index, "/recorded_outcomes", DOGFOOD_INDEX)?.is_empty() {
        return Err(format!(
            "{DOGFOOD_INDEX} recorded_outcomes must document at least one saved outcome"
        ));
    }
    if json_array_at(&index, "/limitations", DOGFOOD_INDEX)?.is_empty() {
        return Err(format!(
            "{DOGFOOD_INDEX} limitations must document current dogfood limits"
        ));
    }

    Ok(())
}

fn check_manual_fuzz_harness() -> Result<(), String> {
    for path in FUZZ_REQUIRED_FILES {
        require_repo_file(path)?;
    }

    let workspace = parse_toml_file(&repo_path("Cargo.toml"))?;
    let excludes = workspace
        .get("workspace")
        .and_then(|workspace| workspace.get("exclude"))
        .and_then(toml::Value::as_array)
        .ok_or_else(|| "Cargo.toml workspace.exclude must list fuzz".to_string())?;
    if !excludes
        .iter()
        .any(|entry| entry.as_str().is_some_and(|entry| entry == "fuzz"))
    {
        return Err("Cargo.toml workspace.exclude must list fuzz".to_string());
    }

    let fuzz_manifest = parse_toml_file(&repo_path("fuzz/Cargo.toml"))?;
    if fuzz_manifest
        .get("package")
        .and_then(|package| package.get("publish"))
        .and_then(toml::Value::as_bool)
        .unwrap_or(true)
    {
        return Err("fuzz/Cargo.toml package.publish must be false".to_string());
    }
    let cargo_fuzz = fuzz_manifest
        .get("package")
        .and_then(|package| package.get("metadata"))
        .and_then(|metadata| metadata.get("cargo-fuzz"))
        .and_then(toml::Value::as_bool)
        .unwrap_or(false);
    if !cargo_fuzz {
        return Err("fuzz/Cargo.toml package.metadata.cargo-fuzz must be true".to_string());
    }
    let bins = fuzz_manifest
        .get("bin")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| "fuzz/Cargo.toml must define an analyze fuzz target".to_string())?;
    let has_analyze_target = bins.iter().any(|bin| {
        bin.get("name").and_then(toml::Value::as_str) == Some("analyze")
            && bin.get("path").and_then(toml::Value::as_str) == Some("fuzz_targets/analyze.rs")
    });
    if !has_analyze_target {
        return Err("fuzz/Cargo.toml must define analyze at fuzz_targets/analyze.rs".to_string());
    }

    let fuzz_docs = read_to_string(&repo_path("docs/FUZZING.md"))?;
    for phrase in [
        "manual `cargo-fuzz` harness",
        "not part of the default PR gate",
        "does not prove soundness",
    ] {
        if !fuzz_docs.contains(phrase) {
            return Err(format!("docs/FUZZING.md must include `{phrase}`"));
        }
    }

    let target = read_to_string(&repo_path("fuzz/fuzz_targets/analyze.rs"))?;
    for phrase in [
        "fuzz_target!",
        "DiffSource::Text",
        "render_json",
        "MAX_SOURCE_BYTES",
        "MAX_DIFF_BYTES",
    ] {
        if !target.contains(phrase) {
            return Err(format!(
                "fuzz/fuzz_targets/analyze.rs must include `{phrase}`"
            ));
        }
    }

    let ignore = read_to_string(&repo_path("fuzz/.gitignore"))?;
    for ignored in ["artifacts/", "target/"] {
        if !ignore.lines().any(|line| line.trim() == ignored) {
            return Err(format!("fuzz/.gitignore must ignore `{ignored}`"));
        }
    }

    println!("check-fuzz: ok");
    Ok(())
}

struct AdvisoryArtifactSummary {
    card_ids: BTreeSet<String>,
    card_count: usize,
}

fn check_advisory_artifacts(dir: &Path) -> Result<(), String> {
    check_advisory_artifact_set(dir)?;
    println!("check-advisory-artifacts: ok ({})", dir.display());
    Ok(())
}

fn check_first_pr_artifacts(dir: &Path) -> Result<(), String> {
    let summary = check_advisory_artifact_set(dir)?;
    check_witness_plan_artifact(dir, summary.card_count)?;
    check_lsp_artifact(dir, &summary.card_ids)?;
    check_first_pr_artifact_overclaims(dir)?;

    println!("check-first-pr-artifacts: ok ({})", dir.display());
    Ok(())
}

fn check_advisory_artifact_set(dir: &Path) -> Result<AdvisoryArtifactSummary, String> {
    if !dir.is_dir() {
        return Err(format!(
            "advisory artifact directory missing: {}",
            dir.display()
        ));
    }

    let cards = parse_json_file(&dir.join("cards.json"))?;
    require_json_str(&cards, "tool", "unsafe-review", "cards.json")?;
    require_json_str(&cards, "policy", "advisory", "cards.json")?;
    require_json_array(&cards, "cards", "cards.json")?;
    let cards_boundary = cards
        .get("trust_boundary")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "cards.json is missing trust_boundary".to_string())?;
    require_boundary_text(cards_boundary, "cards.json")?;
    require_cards_output_shape(&cards)?;
    let card_facts = advisory_card_facts(&cards)?;
    let card_ids = card_facts.keys().cloned().collect::<BTreeSet<_>>();
    let card_count = card_ids.len();
    let summary_cards = json_usize_at(&cards, "/summary/cards", "cards.json")?;
    if summary_cards != card_count {
        return Err(format!(
            "cards.json summary.cards is {summary_cards}, but cards array has {card_count}"
        ));
    }

    let pr_summary_path = dir.join("pr-summary.md");
    let pr_summary = read_to_string(&pr_summary_path)?;
    require_text_contains(
        &pr_summary,
        &format!("- Review cards: {card_count}"),
        &pr_summary_path,
    )?;
    require_text_contains(
        &pr_summary,
        "static unsafe contract review",
        &pr_summary_path,
    )?;
    require_text_contains(
        &pr_summary,
        "not a proof of memory safety",
        &pr_summary_path,
    )?;
    require_text_contains(&pr_summary, "not UB-free status", &pr_summary_path)?;
    require_text_contains(&pr_summary, "not a Miri result", &pr_summary_path)?;
    require_pr_summary_shape(&pr_summary, card_count, &card_ids, &pr_summary_path)?;
    if card_count == 0 {
        require_text_contains(
            &pr_summary,
            "No changed unsafe-review gaps were found.",
            &pr_summary_path,
        )?;
        require_text_contains(&pr_summary, "unsafe site executed", &pr_summary_path)?;
    }

    let sarif = parse_json_file(&dir.join("cards.sarif"))?;
    require_json_str(&sarif, "version", "2.1.0", "cards.sarif")?;
    require_json_array(&sarif, "runs", "cards.sarif")?;
    require_sarif_run_shape(&sarif)?;
    let sarif_results = json_array_at(&sarif, "/runs/0/results", "cards.sarif")?;
    if sarif_results.len() != card_count {
        return Err(format!(
            "cards.sarif has {} result(s), but cards.json has {card_count} card(s)",
            sarif_results.len()
        ));
    }
    let mut sarif_card_ids = BTreeSet::new();
    for result in sarif_results {
        let card_id =
            require_non_empty_json_str_at(result, "/properties/cardId", "cards.sarif result")?;
        if !sarif_card_ids.insert(card_id.to_string()) {
            return Err(format!(
                "cards.sarif contains duplicate result for card id `{card_id}`"
            ));
        }
        let Some(facts) = card_facts.get(card_id) else {
            return Err(format!(
                "cards.sarif result references unknown card id `{card_id}`"
            ));
        };
        require_sarif_result_shape(result)?;
        require_sarif_result_matches_card(result, facts)?;
    }
    if sarif_card_ids != card_ids {
        return Err("cards.sarif result card ids do not exactly match cards.json".to_string());
    }
    let sarif_boundary = sarif
        .pointer("/runs/0/properties/trustBoundary")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "cards.sarif is missing /runs/0/properties/trustBoundary".to_string())?;
    require_boundary_text(sarif_boundary, "cards.sarif")?;

    let comment_plan = parse_json_file(&dir.join("comment-plan.json"))?;
    require_json_str(&comment_plan, "mode", "plan_only", "comment-plan.json")?;
    require_json_str(&comment_plan, "policy", "advisory", "comment-plan.json")?;
    require_json_array(&comment_plan, "comments", "comment-plan.json")?;
    let comments = json_array_at(&comment_plan, "/comments", "comment-plan.json")?;
    if comments.len() > MAX_COMMENT_PLAN_COMMENTS {
        return Err(format!(
            "comment-plan.json has {} planned comment(s), but advisory artifacts allow at most {MAX_COMMENT_PLAN_COMMENTS}",
            comments.len()
        ));
    }
    let mut comment_card_ids = BTreeSet::new();
    for comment in comments {
        let Some(card_id) = comment.get("card_id").and_then(serde_json::Value::as_str) else {
            return Err("comment-plan.json comment is missing card_id".to_string());
        };
        if !comment_card_ids.insert(card_id.to_string()) {
            return Err(format!(
                "comment-plan.json contains duplicate planned comment for card id `{card_id}`"
            ));
        }
        let Some(facts) = card_facts.get(card_id) else {
            return Err(format!(
                "comment-plan.json references unknown card id `{card_id}`"
            ));
        };
        require_non_empty_json_str(comment, "path", "comment-plan.json comment")?;
        require_comment_line(comment)?;
        require_non_empty_json_str(comment, "class", "comment-plan.json comment")?;
        require_non_empty_json_str(comment, "priority", "comment-plan.json comment")?;
        require_non_empty_json_str(comment, "confidence", "comment-plan.json comment")?;
        require_non_empty_json_str(comment, "operation", "comment-plan.json comment")?;
        require_non_empty_json_str(comment, "operation_family", "comment-plan.json comment")?;
        json_array_at(comment, "/witness_routes", "comment-plan.json comment")?;
        json_array_at(comment, "/verify_commands", "comment-plan.json comment")?;
        require_non_empty_json_str(comment, "selection_reason", "comment-plan.json comment")?;
        let body = require_non_empty_json_str(comment, "body", "comment-plan.json comment")?;
        require_comment_body_boundary_text(body, "comment-plan.json comment body")?;
        require_comment_plan_comment_matches_card(comment, facts)?;
    }
    let comment_boundary = comment_plan
        .get("trust_boundary")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "comment-plan.json is missing trust_boundary".to_string())?;
    require_boundary_text(comment_boundary, "comment-plan.json")?;
    if card_count == 0 {
        let no_changed = comment_plan
            .get("no_changed_gaps")
            .ok_or_else(|| "comment-plan.json is missing no_changed_gaps".to_string())?;
        require_json_str(
            no_changed,
            "message",
            "No changed unsafe-review gaps were found.",
            "comment-plan.json no_changed_gaps",
        )?;
        let limitation = require_non_empty_json_str(
            no_changed,
            "limitation",
            "comment-plan.json no_changed_gaps",
        )?;
        if !text_contains_ignore_ascii_case(limitation, "unsafe site executed") {
            return Err(
                "comment-plan.json no_changed_gaps.limitation must mention unsafe site execution"
                    .to_string(),
            );
        }
    }

    Ok(AdvisoryArtifactSummary {
        card_ids,
        card_count,
    })
}

fn check_witness_plan_artifact(dir: &Path, card_count: usize) -> Result<(), String> {
    let path = dir.join("witness-plan.md");
    let text = read_to_string(&path)?;
    require_text_contains(&text, "# unsafe-review witness plan", &path)?;
    require_text_contains(&text, &format!("- Review cards: {card_count}"), &path)?;
    require_text_contains(&text, "does not run Miri", &path)?;
    require_text_contains(&text, "cargo-careful", &path)?;
    require_text_contains(&text, "not a proof of memory safety", &path)?;
    require_text_contains(&text, "not UB-free status", &path)?;
    require_text_contains(&text, "not a Miri result", &path)?;
    if card_count > 0 {
        require_text_contains(&text, "## Route groups", &path)?;
        require_text_contains(&text, "- Route:", &path)?;
        require_text_contains(&text, "What it can show", &path)?;
        require_text_contains(&text, "What it cannot prove", &path)?;
        require_text_contains(&text, "Receipt hint", &path)?;
    } else {
        require_text_contains(&text, "No changed unsafe-review gaps were found.", &path)?;
        require_text_contains(&text, "unsafe site executed", &path)?;
    }
    Ok(())
}

fn check_lsp_artifact(dir: &Path, card_ids: &BTreeSet<String>) -> Result<(), String> {
    let path = dir.join("lsp.json");
    let lsp = parse_json_file(&path)?;
    require_json_str(&lsp, "tool", "unsafe-review", "lsp.json")?;
    require_json_str(&lsp, "mode", "read_only_projection", "lsp.json")?;
    require_json_str(&lsp, "policy", "advisory", "lsp.json")?;
    require_json_array(&lsp, "diagnostics", "lsp.json")?;
    require_json_array(&lsp, "hovers", "lsp.json")?;
    require_json_array(&lsp, "code_actions", "lsp.json")?;
    let boundary = lsp
        .get("trust_boundary")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "lsp.json is missing trust_boundary".to_string())?;
    require_boundary_text(boundary, "lsp.json")?;
    let status_boundary = lsp
        .pointer("/status/trust_boundary")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "lsp.json is missing /status/trust_boundary".to_string())?;
    require_boundary_text(status_boundary, "lsp.json status")?;

    for diagnostic in json_array_at(&lsp, "/diagnostics", "lsp.json")? {
        let Some(card_id) = diagnostic
            .get("card_id")
            .and_then(serde_json::Value::as_str)
        else {
            return Err("lsp.json diagnostic is missing card_id".to_string());
        };
        if !card_ids.contains(card_id) {
            return Err(format!(
                "lsp.json diagnostic references unknown card id `{card_id}`"
            ));
        }
        require_non_empty_json_str(diagnostic, "operation_family", "lsp.json diagnostic")?;
        json_array_at(diagnostic, "/hazards", "lsp.json diagnostic")?;
        json_array_at(diagnostic, "/missing_evidence", "lsp.json diagnostic")?;
        let boundary = diagnostic
            .get("trust_boundary")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "lsp.json diagnostic is missing trust_boundary".to_string())?;
        require_boundary_text(boundary, "lsp.json diagnostic")?;
    }

    for hover in json_array_at(&lsp, "/hovers", "lsp.json")? {
        require_known_card_id(hover, "lsp.json hover", card_ids)?;
        let contents = hover
            .get("contents")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "lsp.json hover is missing contents".to_string())?;
        require_text_contains(contents, "Trust boundary", &path)?;
        let boundary = hover
            .get("trust_boundary")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "lsp.json hover is missing trust_boundary".to_string())?;
        require_boundary_text(boundary, "lsp.json hover")?;
    }

    for action in json_array_at(&lsp, "/code_actions", "lsp.json")? {
        require_known_card_id(action, "lsp.json code_action", card_ids)?;
        let Some(command) = action.get("command").and_then(serde_json::Value::as_str) else {
            return Err("lsp.json code_action is missing command".to_string());
        };
        if command.trim().is_empty() {
            return Err("lsp.json code_action command must not be empty".to_string());
        }
        if action.get("edit").is_some() || action.get("workspace_edit").is_some() {
            return Err("lsp.json code_action must not contain source edits".to_string());
        }
    }
    Ok(())
}

fn require_known_card_id(
    value: &serde_json::Value,
    context: &str,
    card_ids: &BTreeSet<String>,
) -> Result<(), String> {
    let Some(card_id) = value.get("card_id").and_then(serde_json::Value::as_str) else {
        return Err(format!("{context} is missing card_id"));
    };
    if card_ids.contains(card_id) {
        Ok(())
    } else {
        Err(format!("{context} references unknown card id `{card_id}`"))
    }
}

fn check_first_pr_artifact_overclaims(dir: &Path) -> Result<(), String> {
    for name in [
        "pr-summary.md",
        "comment-plan.json",
        "witness-plan.md",
        "lsp.json",
    ] {
        let path = dir.join(name);
        if path.is_file() {
            reject_positive_overclaims(&path, &read_to_string(&path)?)?;
        }
    }
    Ok(())
}

fn reject_positive_overclaims(path: &Path, text: &str) -> Result<(), String> {
    for (line_no, line) in text.lines().enumerate() {
        let lower = line.to_ascii_lowercase();
        for forbidden in ["all clear", "safe to merge", "proved safe", "proven safe"] {
            if lower.contains(forbidden) {
                return Err(format!(
                    "{}:{} must not imply `{forbidden}`",
                    path.display(),
                    line_no + 1
                ));
            }
        }
        if (lower.contains("miri-clean") || lower.contains("miri clean"))
            && !lower.contains("not miri-clean")
            && !lower.contains("not a miri-clean")
            && !lower.contains("not miri clean")
            && !lower.contains("cannot prove")
            && !lower.contains("does not")
        {
            return Err(format!(
                "{}:{} must not imply Miri-clean status",
                path.display(),
                line_no + 1
            ));
        }
        if lower.contains("ub-free")
            && !lower.contains("not ub-free")
            && !lower.contains("not a ub-free")
            && !lower.contains("cannot prove")
            && !lower.contains("does not")
        {
            return Err(format!(
                "{}:{} must not imply UB-free status",
                path.display(),
                line_no + 1
            ));
        }
        if lower.contains("site reached")
            && !lower.contains("not")
            && !lower.contains("cannot prove")
            && !lower.contains("does not")
        {
            return Err(format!(
                "{}:{} must not imply site execution",
                path.display(),
                line_no + 1
            ));
        }
    }
    Ok(())
}

fn check_fixture(dir: &Path) -> Result<(), String> {
    let name = fixture_dir_name(dir)?;
    if !is_snake_case_name(name) {
        return Err(format!(
            "{} must use a lowercase snake_case fixture name",
            dir.display()
        ));
    }

    for relative in FIXTURE_REQUIRED_FILES {
        require_fixture_file(dir, relative)?;
    }

    let cargo = parse_toml_file(&dir.join("Cargo.toml"))?;
    let package_name = cargo
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("{}/Cargo.toml is missing package.name", dir.display()))?;
    let expected_prefix = fixture_package_prefix(name);
    if !package_name.starts_with(&expected_prefix) {
        return Err(format!(
            "{}/Cargo.toml package.name `{package_name}` does not start with `{expected_prefix}`",
            dir.display()
        ));
    }

    let expected_cards = dir.join("expected.cards.json");
    if expected_cards.is_file() {
        let expected_cards = parse_json_file(&expected_cards)?;
        if !expected_cards.is_array() {
            return Err(format!(
                "{}/expected.cards.json must contain a JSON array of cards",
                dir.display()
            ));
        }
    } else if !FIXTURE_EXPECTED_CARDS_EXCEPTIONS.contains(&name) {
        return Err(format!(
            "fixture {} is missing expected.cards.json",
            dir.display()
        ));
    }

    let diff = read_to_string(&dir.join("change.diff"))?;
    if !looks_like_git_diff(&diff) {
        return Err(format!(
            "{}/change.diff does not look like a unified git diff",
            dir.display()
        ));
    }

    Ok(())
}

fn check_calibration_case(
    case: &toml::map::Map<String, toml::Value>,
    fixture: &str,
    kind: &str,
    idx: usize,
) -> Result<(), String> {
    let fixture_dir = workspace_path("fixtures").join(fixture);
    if !fixture_dir.is_dir() {
        return Err(format!(
            "fixtures/calibration.toml cases[{idx}] references missing fixture `{fixture}`"
        ));
    }
    let expected_cards = required_case_usize(case, "expected_cards", idx)?;
    let cards = parse_json_file(&fixture_dir.join("expected.cards.json"))?;
    let Some(cards) = cards.as_array() else {
        return Err(format!(
            "{}/expected.cards.json must contain a JSON array",
            fixture_dir.display()
        ));
    };
    if cards.len() != expected_cards {
        return Err(format!(
            "fixtures/calibration.toml cases[{idx}] expected_cards is {expected_cards}, but {fixture}/expected.cards.json has {} card(s)",
            cards.len()
        ));
    }
    check_calibration_kind_card_count(kind, expected_cards, idx)?;
    if expected_cards == 0 {
        check_zero_card_expectations(case, idx)?;
        return Ok(());
    }
    let expected_class = required_case_string(case, "expected_class", idx)?;
    if !cards
        .iter()
        .any(|card| json_str(card, "class") == Some(expected_class))
    {
        return Err(format!(
            "fixtures/calibration.toml cases[{idx}] expected_class `{expected_class}` was not found in {fixture}/expected.cards.json"
        ));
    }
    if let Some(expected_operation_family) =
        optional_case_string(case, "expected_operation_family", idx)?
        && !cards
            .iter()
            .any(|card| json_str(card, "operation_family") == Some(expected_operation_family))
    {
        return Err(format!(
            "fixtures/calibration.toml cases[{idx}] expected_operation_family `{expected_operation_family}` was not found in {fixture}/expected.cards.json"
        ));
    }
    if let Some(expected_hazard) = optional_case_string(case, "expected_hazard", idx)?
        && !cards
            .iter()
            .any(|card| json_array_contains_str(card, "hazards", expected_hazard))
    {
        return Err(format!(
            "fixtures/calibration.toml cases[{idx}] expected_hazard `{expected_hazard}` was not found in {fixture}/expected.cards.json"
        ));
    }
    Ok(())
}

fn check_calibration_case_fields(
    case: &toml::map::Map<String, toml::Value>,
    idx: usize,
) -> Result<(), String> {
    for field in case.keys() {
        if !CALIBRATION_CASE_FIELDS.contains(&field.as_str()) {
            return Err(format!(
                "fixtures/calibration.toml cases[{idx}] uses unknown field `{field}`"
            ));
        }
    }
    Ok(())
}

fn check_calibration_kind_card_count(
    kind: &str,
    expected_cards: usize,
    idx: usize,
) -> Result<(), String> {
    match (kind, expected_cards) {
        ("positive", 0) => Err(format!(
            "fixtures/calibration.toml cases[{idx}] kind `positive` must expect at least one card"
        )),
        ("negative", 0) => Ok(()),
        ("negative", _) => Err(format!(
            "fixtures/calibration.toml cases[{idx}] kind `negative` must expect zero cards"
        )),
        _ => Ok(()),
    }
}

fn check_operation_family_registry_coverage(
    calibration_families: &BTreeSet<String>,
    calibration_fixtures_by_family: &BTreeMap<String, BTreeSet<String>>,
) -> Result<(), String> {
    check_operation_family_registry_header()?;
    let registry_families = operation_family_registry_rows()?;
    let known_operation_families = operation_family_labels()?;
    let known_obligation_keys = safety_obligation_labels()?;
    let known_hazards = hazard_kind_labels()?;
    let known_witness_routes = witness_kind_labels()?;
    let registry_obligation_keys = operation_family_registry_obligation_keys()?;
    let registry_hazards = operation_family_registry_hazards()?;
    let registry_fixture_proofs = operation_family_registry_fixture_proofs()?;
    let registry_witness_routes = operation_family_registry_witness_routes()?;
    let registry = OperationFamilyRegistryView {
        families: &registry_families,
        obligation_keys: &registry_obligation_keys,
        hazards: &registry_hazards,
        fixture_proofs: &registry_fixture_proofs,
        witness_routes: &registry_witness_routes,
    };
    check_operation_family_registry_coverage_with_registry(
        calibration_families,
        calibration_fixtures_by_family,
        &known_operation_families,
        &known_obligation_keys,
        &known_hazards,
        &known_witness_routes,
        &registry,
    )
}

fn check_operation_family_registry_header() -> Result<(), String> {
    let text = read_to_string(&workspace_path(OPERATION_FAMILY_REGISTRY))?;
    check_operation_family_registry_header_from_text(&text)
}

fn check_operation_family_registry_header_from_text(text: &str) -> Result<(), String> {
    for line in text.lines() {
        let columns = registry_columns(line);
        let Some(first) = columns.first() else {
            continue;
        };
        if *first != "operation_family" {
            continue;
        }
        if columns != OPERATION_FAMILY_REGISTRY_HEADER {
            return Err(format!(
                "{OPERATION_FAMILY_REGISTRY} registry header must be: {}",
                OPERATION_FAMILY_REGISTRY_HEADER.join(" | ")
            ));
        }
        return Ok(());
    }
    Err(format!(
        "{OPERATION_FAMILY_REGISTRY} is missing operation_family registry header"
    ))
}

struct OperationFamilyRegistryView<'a> {
    families: &'a BTreeSet<String>,
    obligation_keys: &'a BTreeMap<String, BTreeSet<String>>,
    hazards: &'a BTreeMap<String, BTreeSet<String>>,
    fixture_proofs: &'a BTreeMap<String, BTreeSet<String>>,
    witness_routes: &'a BTreeMap<String, BTreeSet<String>>,
}

fn check_operation_family_registry_coverage_with_registry(
    calibration_families: &BTreeSet<String>,
    calibration_fixtures_by_family: &BTreeMap<String, BTreeSet<String>>,
    known_operation_families: &BTreeSet<String>,
    known_obligation_keys: &BTreeSet<String>,
    known_hazards: &BTreeSet<String>,
    known_witness_routes: &BTreeSet<String>,
    registry: &OperationFamilyRegistryView<'_>,
) -> Result<(), String> {
    let missing_registry_rows = calibration_families
        .difference(registry.families)
        .cloned()
        .collect::<Vec<_>>();
    if !missing_registry_rows.is_empty() {
        return Err(format!(
            "{OPERATION_FAMILY_REGISTRY} is missing operation_family row(s) for fixture-backed calibration family/families: {}",
            missing_registry_rows.join(", ")
        ));
    }

    let unbacked_registry_rows = registry
        .families
        .difference(calibration_families)
        .cloned()
        .collect::<Vec<_>>();
    if !unbacked_registry_rows.is_empty() {
        return Err(format!(
            "{OPERATION_FAMILY_REGISTRY} contains operation_family row(s) without fixture-backed calibration family/families: {}",
            unbacked_registry_rows.join(", ")
        ));
    }

    let unknown_registry_families = registry
        .families
        .difference(known_operation_families)
        .cloned()
        .collect::<Vec<_>>();
    if !unknown_registry_families.is_empty() {
        return Err(format!(
            "{OPERATION_FAMILY_REGISTRY} cites unknown operation_family row(s) not emitted by OperationFamily::as_str: {}",
            unknown_registry_families.join(", ")
        ));
    }

    for family in registry.families {
        let Some(obligation_keys) = registry.obligation_keys.get(family) else {
            return Err(format!(
                "{OPERATION_FAMILY_REGISTRY} operation_family `{family}` is missing obligation/evidence key metadata"
            ));
        };
        let unknown_obligation_keys = obligation_keys
            .iter()
            .filter(|key| !known_obligation_keys.contains(key.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        if !unknown_obligation_keys.is_empty() {
            return Err(format!(
                "{OPERATION_FAMILY_REGISTRY} operation_family `{family}` cites unknown obligation/evidence key(s): {}",
                unknown_obligation_keys.join(", ")
            ));
        }

        let Some(hazards) = registry.hazards.get(family) else {
            return Err(format!(
                "{OPERATION_FAMILY_REGISTRY} operation_family `{family}` is missing hazard metadata"
            ));
        };
        let unknown_hazards = hazards
            .iter()
            .filter(|hazard| !known_hazards.contains(hazard.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        if !unknown_hazards.is_empty() {
            return Err(format!(
                "{OPERATION_FAMILY_REGISTRY} operation_family `{family}` cites unknown hazard(s): {}",
                unknown_hazards.join(", ")
            ));
        }
    }

    for (family, fixtures) in registry.fixture_proofs {
        let Some(calibration_fixtures) = calibration_fixtures_by_family.get(family) else {
            return Err(format!(
                "{OPERATION_FAMILY_REGISTRY} contains fixture proof for uncalibrated operation_family `{family}`"
            ));
        };
        let unbacked_fixtures = fixtures
            .difference(calibration_fixtures)
            .cloned()
            .collect::<Vec<_>>();
        if !unbacked_fixtures.is_empty() {
            return Err(format!(
                "{OPERATION_FAMILY_REGISTRY} operation_family `{family}` cites fixture proof(s) not calibrated for that family: {}",
                unbacked_fixtures.join(", ")
            ));
        }
    }

    for family in registry.families {
        let Some(routes) = registry.witness_routes.get(family) else {
            return Err(format!(
                "{OPERATION_FAMILY_REGISTRY} operation_family `{family}` is missing witness route metadata"
            ));
        };
        let unknown_routes = routes
            .iter()
            .filter(|route| !known_witness_routes.contains(route.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        if !unknown_routes.is_empty() {
            return Err(format!(
                "{OPERATION_FAMILY_REGISTRY} operation_family `{family}` cites unknown witness route(s): {}",
                unknown_routes.join(", ")
            ));
        }
    }

    Ok(())
}

fn operation_family_registry_rows() -> Result<BTreeSet<String>, String> {
    let text = read_to_string(&workspace_path(OPERATION_FAMILY_REGISTRY))?;
    operation_family_registry_rows_from_text(&text)
}

fn operation_family_registry_rows_from_text(text: &str) -> Result<BTreeSet<String>, String> {
    let mut rows = BTreeSet::new();
    for line in text.lines() {
        let columns = registry_columns(line);
        let Some(first) = columns.first() else {
            continue;
        };
        let Some(family) = first
            .strip_prefix('`')
            .and_then(|value| value.strip_suffix('`'))
        else {
            continue;
        };
        if columns.len() != OPERATION_FAMILY_REGISTRY_COLUMNS {
            return Err(format!(
                "{OPERATION_FAMILY_REGISTRY} operation_family `{family}` must have {OPERATION_FAMILY_REGISTRY_COLUMNS} columns, found {}",
                columns.len()
            ));
        }
        validate_operation_family_registry_required_text(family, &columns)?;
        if !rows.insert(family.to_string()) {
            return Err(format!(
                "{OPERATION_FAMILY_REGISTRY} contains duplicate operation_family row `{family}`"
            ));
        }
    }
    if rows.is_empty() {
        return Err(format!(
            "{OPERATION_FAMILY_REGISTRY} contains no operation_family registry rows"
        ));
    }
    Ok(rows)
}

fn validate_operation_family_registry_required_text(
    family: &str,
    columns: &[&str],
) -> Result<(), String> {
    for (idx, name) in OPERATION_FAMILY_REGISTRY_REQUIRED_TEXT_COLUMNS {
        let Some(value) = columns.get(*idx) else {
            return Err(format!(
                "{OPERATION_FAMILY_REGISTRY} operation_family `{family}` is missing {name} column"
            ));
        };
        if is_placeholder_registry_text(value) {
            return Err(format!(
                "{OPERATION_FAMILY_REGISTRY} operation_family `{family}` {name} column must describe current review behavior, not `{value}`"
            ));
        }
    }
    let (idx, name) = OPERATION_FAMILY_REGISTRY_OBLIGATION_KEYS_COLUMN;
    let Some(value) = columns.get(idx) else {
        return Err(format!(
            "{OPERATION_FAMILY_REGISTRY} operation_family `{family}` is missing {name} column"
        ));
    };
    if family == "unknown" {
        if value.trim() != "unknown" {
            return Err(format!(
                "{OPERATION_FAMILY_REGISTRY} operation_family `unknown` {name} column must stay `unknown`"
            ));
        }
    } else if is_placeholder_registry_text(value) {
        return Err(format!(
            "{OPERATION_FAMILY_REGISTRY} operation_family `{family}` {name} column must name concrete obligation/evidence keys, not `{value}`"
        ));
    } else {
        let invalid_keys = registry_key_tokens(value)
            .into_iter()
            .filter(|key| !is_registry_key_token(key))
            .collect::<Vec<_>>();
        if !invalid_keys.is_empty() {
            return Err(format!(
                "{OPERATION_FAMILY_REGISTRY} operation_family `{family}` {name} column contains invalid key token(s): {}",
                invalid_keys.join(", ")
            ));
        }
    }
    Ok(())
}

fn operation_family_registry_obligation_keys() -> Result<BTreeMap<String, BTreeSet<String>>, String>
{
    let text = read_to_string(&workspace_path(OPERATION_FAMILY_REGISTRY))?;
    operation_family_registry_obligation_keys_from_text(&text)
}

fn operation_family_registry_obligation_keys_from_text(
    text: &str,
) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    let mut keys_by_family = BTreeMap::new();
    for line in text.lines() {
        let columns = registry_columns(line);
        let Some(first) = columns.first() else {
            continue;
        };
        let Some(family) = first
            .strip_prefix('`')
            .and_then(|value| value.strip_suffix('`'))
        else {
            continue;
        };
        let (idx, name) = OPERATION_FAMILY_REGISTRY_OBLIGATION_KEYS_COLUMN;
        let Some(value) = columns.get(idx) else {
            return Err(format!(
                "{OPERATION_FAMILY_REGISTRY} operation_family `{family}` is missing {name} column"
            ));
        };
        let keys = registry_key_tokens(value)
            .into_iter()
            .collect::<BTreeSet<_>>();
        if keys.is_empty() {
            return Err(format!(
                "{OPERATION_FAMILY_REGISTRY} operation_family `{family}` {name} column has no key tokens"
            ));
        }
        if keys_by_family.insert(family.to_string(), keys).is_some() {
            return Err(format!(
                "{OPERATION_FAMILY_REGISTRY} contains duplicate operation_family row `{family}`"
            ));
        }
    }
    if keys_by_family.is_empty() {
        return Err(format!(
            "{OPERATION_FAMILY_REGISTRY} contains no operation_family registry rows"
        ));
    }
    Ok(keys_by_family)
}

fn is_placeholder_registry_text(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "" | "none" | "n/a" | "na" | "todo" | "tbd" | "unknown"
    )
}

fn registry_key_tokens(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn is_registry_key_token(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.first().is_some_and(u8::is_ascii_lowercase)
        && bytes.last().is_some_and(u8::is_ascii_alphanumeric)
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
        && !bytes.windows(2).any(|pair| pair == b"--")
}

fn operation_family_labels() -> Result<BTreeSet<String>, String> {
    let text = read_to_string(&workspace_path(OPERATION_FAMILY_SOURCE))?;
    operation_family_labels_from_text(&text)
}

fn operation_family_labels_from_text(text: &str) -> Result<BTreeSet<String>, String> {
    let Some((_, tail)) = text.split_once("impl OperationFamily") else {
        return Err(format!(
            "{OPERATION_FAMILY_SOURCE} has no OperationFamily implementation"
        ));
    };
    let Some((operation_family_impl, _)) = tail.split_once("pub struct UnsafeOperation") else {
        return Err(format!(
            "{OPERATION_FAMILY_SOURCE} OperationFamily labels must appear before UnsafeOperation"
        ));
    };
    as_str_labels_from_text(operation_family_impl)
        .ok_or_else(|| format!("{OPERATION_FAMILY_SOURCE} has no OperationFamily::as_str labels"))
}

fn safety_obligation_labels() -> Result<BTreeSet<String>, String> {
    let text = read_to_string(&workspace_path(SAFETY_OBLIGATION_SOURCE))?;
    safety_obligation_labels_from_text(&text)
}

fn safety_obligation_labels_from_text(text: &str) -> Result<BTreeSet<String>, String> {
    let mut labels = BTreeSet::new();
    let mut rest = text;
    while let Some((_, suffix)) = rest.split_once("SafetyObligation::new(") {
        let Some(label) = first_quoted_text(suffix) else {
            return Err(format!(
                "{SAFETY_OBLIGATION_SOURCE} has SafetyObligation::new without a string key"
            ));
        };
        labels.insert(label.to_string());
        rest = suffix;
    }
    if labels.is_empty() {
        Err(format!(
            "{SAFETY_OBLIGATION_SOURCE} has no SafetyObligation::new labels"
        ))
    } else {
        Ok(labels)
    }
}

fn first_quoted_text(text: &str) -> Option<&str> {
    let (_, suffix) = text.split_once('"')?;
    let (value, _) = suffix.split_once('"')?;
    Some(value)
}

fn hazard_kind_labels() -> Result<BTreeSet<String>, String> {
    let text = read_to_string(&workspace_path(HAZARD_KIND_SOURCE))?;
    hazard_kind_labels_from_text(&text)
}

fn hazard_kind_labels_from_text(text: &str) -> Result<BTreeSet<String>, String> {
    as_str_labels_from_text(text)
        .ok_or_else(|| format!("{HAZARD_KIND_SOURCE} has no HazardKind::as_str labels"))
}

fn witness_kind_labels() -> Result<BTreeSet<String>, String> {
    let text = read_to_string(&workspace_path(WITNESS_KIND_SOURCE))?;
    witness_kind_labels_from_text(&text)
}

fn witness_kind_labels_from_text(text: &str) -> Result<BTreeSet<String>, String> {
    as_str_labels_from_text(text)
        .ok_or_else(|| format!("{WITNESS_KIND_SOURCE} has no WitnessKind::as_str labels"))
}

fn as_str_labels_from_text(text: &str) -> Option<BTreeSet<String>> {
    let labels = text
        .lines()
        .filter_map(|line| {
            let (_, suffix) = line.split_once("=> \"")?;
            let (label, _) = suffix.split_once('"')?;
            Some(label.to_string())
        })
        .collect::<BTreeSet<_>>();
    (!labels.is_empty()).then_some(labels)
}

fn operation_family_registry_hazards() -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    let text = read_to_string(&workspace_path(OPERATION_FAMILY_REGISTRY))?;
    operation_family_registry_hazards_from_text(&text)
}

fn operation_family_registry_hazards_from_text(
    text: &str,
) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    let mut hazards_by_family = BTreeMap::new();
    for line in text.lines() {
        let columns = registry_columns(line);
        let Some(first) = columns.first() else {
            continue;
        };
        let Some(family) = first
            .strip_prefix('`')
            .and_then(|value| value.strip_suffix('`'))
        else {
            continue;
        };
        let Some(hazard_column) = columns.get(2) else {
            return Err(format!(
                "{OPERATION_FAMILY_REGISTRY} operation_family `{family}` is missing hazard column"
            ));
        };
        let hazards = hazard_tokens(hazard_column);
        if hazards.is_empty() {
            return Err(format!(
                "{OPERATION_FAMILY_REGISTRY} operation_family `{family}` hazard column has no hazard names"
            ));
        }
        if hazards_by_family
            .insert(family.to_string(), hazards)
            .is_some()
        {
            return Err(format!(
                "{OPERATION_FAMILY_REGISTRY} contains duplicate operation_family row `{family}`"
            ));
        }
    }
    if hazards_by_family.is_empty() {
        return Err(format!(
            "{OPERATION_FAMILY_REGISTRY} contains no operation_family registry rows"
        ));
    }
    Ok(hazards_by_family)
}

fn operation_family_registry_fixture_proofs() -> Result<BTreeMap<String, BTreeSet<String>>, String>
{
    let text = read_to_string(&workspace_path(OPERATION_FAMILY_REGISTRY))?;
    operation_family_registry_fixture_proofs_from_text(&text)
}

fn operation_family_registry_fixture_proofs_from_text(
    text: &str,
) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    let mut proofs = BTreeMap::new();
    for line in text.lines() {
        let columns = registry_columns(line);
        let Some(first) = columns.first() else {
            continue;
        };
        let Some(family) = first
            .strip_prefix('`')
            .and_then(|value| value.strip_suffix('`'))
        else {
            continue;
        };
        let Some(fixture_proof) = columns.get(6) else {
            return Err(format!(
                "{OPERATION_FAMILY_REGISTRY} operation_family `{family}` is missing fixture proof column"
            ));
        };
        let fixture_names = backtick_tokens(fixture_proof);
        if fixture_names.is_empty() {
            return Err(format!(
                "{OPERATION_FAMILY_REGISTRY} operation_family `{family}` fixture proof column has no fixture names"
            ));
        }
        if proofs.insert(family.to_string(), fixture_names).is_some() {
            return Err(format!(
                "{OPERATION_FAMILY_REGISTRY} contains duplicate operation_family row `{family}`"
            ));
        }
    }
    if proofs.is_empty() {
        return Err(format!(
            "{OPERATION_FAMILY_REGISTRY} contains no operation_family registry rows"
        ));
    }
    Ok(proofs)
}

fn operation_family_registry_witness_routes() -> Result<BTreeMap<String, BTreeSet<String>>, String>
{
    let text = read_to_string(&workspace_path(OPERATION_FAMILY_REGISTRY))?;
    operation_family_registry_witness_routes_from_text(&text)
}

fn operation_family_registry_witness_routes_from_text(
    text: &str,
) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    let mut routes_by_family = BTreeMap::new();
    for line in text.lines() {
        let columns = registry_columns(line);
        let Some(first) = columns.first() else {
            continue;
        };
        let Some(family) = first
            .strip_prefix('`')
            .and_then(|value| value.strip_suffix('`'))
        else {
            continue;
        };
        let Some(route_column) = columns.get(5) else {
            return Err(format!(
                "{OPERATION_FAMILY_REGISTRY} operation_family `{family}` is missing witness route column"
            ));
        };
        let routes = witness_route_tokens(route_column);
        if routes.is_empty() {
            return Err(format!(
                "{OPERATION_FAMILY_REGISTRY} operation_family `{family}` witness route column has no route names"
            ));
        }
        if routes_by_family
            .insert(family.to_string(), routes)
            .is_some()
        {
            return Err(format!(
                "{OPERATION_FAMILY_REGISTRY} contains duplicate operation_family row `{family}`"
            ));
        }
    }
    if routes_by_family.is_empty() {
        return Err(format!(
            "{OPERATION_FAMILY_REGISTRY} contains no operation_family registry rows"
        ));
    }
    Ok(routes_by_family)
}

fn hazard_tokens(text: &str) -> BTreeSet<String> {
    text.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .map(str::trim)
        .filter(|token| token.chars().any(|ch| ch.is_ascii_alphanumeric()))
        .map(ToString::to_string)
        .collect()
}

fn witness_route_tokens(text: &str) -> BTreeSet<String> {
    text.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '-'))
        .map(str::trim)
        .filter(|token| token.chars().any(|ch| ch.is_ascii_alphanumeric()))
        .map(ToString::to_string)
        .collect()
}

fn registry_columns(line: &str) -> Vec<&str> {
    line.split('|')
        .map(str::trim)
        .filter(|column| !column.is_empty())
        .collect()
}

fn backtick_tokens(text: &str) -> BTreeSet<String> {
    let mut tokens = BTreeSet::new();
    let mut rest = text;
    while let Some(start) = rest.find('`') {
        rest = &rest[start + 1..];
        let Some(end) = rest.find('`') else {
            break;
        };
        let token = &rest[..end];
        let token = token.trim();
        if !token.is_empty() {
            tokens.insert(token.to_string());
        }
        rest = &rest[end + 1..];
    }
    tokens
}

fn check_zero_card_expectations(
    case: &toml::map::Map<String, toml::Value>,
    idx: usize,
) -> Result<(), String> {
    for field in ZERO_CARD_EXPECTATION_FIELDS {
        if case.contains_key(*field) {
            return Err(format!(
                "fixtures/calibration.toml cases[{idx}] has expected_cards = 0 and cannot set `{field}`"
            ));
        }
    }
    Ok(())
}

fn check_index(dir: &Path, readme: &Path, prefix: &str) -> Result<(), String> {
    let index = read_to_string(readme)?;
    let mut ids = BTreeSet::new();
    for path in markdown_files(dir)? {
        if path == readme {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            return Err(format!("non-UTF-8 file name in {}", dir.display()));
        };
        if !name.starts_with(prefix) {
            return Err(format!(
                "{} does not use expected `{prefix}` prefix",
                path.display()
            ));
        }
        let id = name.trim_end_matches(".md");
        if !ids.insert(id.to_string()) {
            return Err(format!("duplicate source-of-truth id `{id}`"));
        }
        if !index.contains(name) {
            return Err(format!(
                "{} is missing from {}",
                path.display(),
                readme.display()
            ));
        }
    }
    Ok(())
}

fn check_handoff_index(dir: &Path, readme: &Path) -> Result<(), String> {
    let index = read_to_string(readme)?;
    let mut files = 0usize;
    for path in markdown_files(dir)? {
        if path == readme {
            continue;
        }
        files += 1;
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            return Err(format!("non-UTF-8 file name in {}", dir.display()));
        };
        if !name.starts_with("20") {
            return Err(format!(
                "{} does not use dated handoff file naming",
                path.display()
            ));
        }
        if !index.contains(name) {
            return Err(format!(
                "{} is missing from {}",
                path.display(),
                readme.display()
            ));
        }
    }
    if files == 0 {
        return Err(format!("{} has no handoff files", dir.display()));
    }
    Ok(())
}

fn check_docs_map_paths(path: &str) -> Result<(), String> {
    let text = read_to_string(Path::new(path))?;
    let mut checked = 0usize;
    for span in markdown_code_spans(&text) {
        let candidate = span.trim();
        if !looks_like_repo_path(candidate) {
            continue;
        }
        checked += 1;
        if !Path::new(candidate).exists() && !repo_path(candidate).exists() {
            return Err(format!("{path} references missing path `{candidate}`"));
        }
    }
    if checked == 0 {
        return Err(format!("{path} has no repository path code spans"));
    }
    Ok(())
}

fn check_markdown_local_links(path: &str) -> Result<(), String> {
    let source = workspace_path(path);
    let text = read_to_string(&source)?;
    for target in markdown_link_targets(&text) {
        let Some(local) = local_markdown_link_target(&target) else {
            continue;
        };
        let resolved = markdown_link_path(&source, local);
        if !resolved.exists() {
            return Err(format!("{path} references missing local link `{target}`"));
        }
    }
    Ok(())
}

fn markdown_link_targets(text: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let mut rest = text;
    while let Some(label_start) = rest.find('[') {
        rest = &rest[label_start + 1..];
        let Some(label_end) = rest.find(']') else {
            break;
        };
        let after_label = &rest[label_end + 1..];
        let Some(after_open) = after_label.strip_prefix('(') else {
            rest = after_label;
            continue;
        };
        let Some(target_end) = after_open.find(')') else {
            break;
        };
        let target = after_open[..target_end].trim();
        if !target.is_empty() {
            targets.push(target.to_string());
        }
        rest = &after_open[target_end + 1..];
    }
    targets
}

fn local_markdown_link_target(target: &str) -> Option<&str> {
    let target = target
        .split_once('#')
        .map_or(target, |(path, _)| path)
        .trim();
    if target.is_empty()
        || target.starts_with('#')
        || target.starts_with("http://")
        || target.starts_with("https://")
        || target.starts_with("mailto:")
        || target.starts_with("file:")
        || target.starts_with("sandbox:")
    {
        return None;
    }
    Some(target)
}

fn markdown_link_path(source: &Path, target: &str) -> PathBuf {
    let target_path = Path::new(target);
    if target_path.is_absolute() {
        return target_path.to_path_buf();
    }
    source.parent().map_or_else(
        || target_path.to_path_buf(),
        |parent| parent.join(target_path),
    )
}

fn markdown_code_spans(text: &str) -> Vec<String> {
    let mut spans = Vec::new();
    let mut current = String::new();
    let mut in_code = false;
    for ch in text.chars() {
        if ch == '`' {
            if in_code {
                spans.push(current.clone());
                current.clear();
            }
            in_code = !in_code;
        } else if in_code {
            current.push(ch);
        }
    }
    spans
}

fn looks_like_repo_path(value: &str) -> bool {
    value.contains('/') || value.ends_with(".md")
}

fn check_no_windows_paths(paths: &[&Path]) -> Result<(), String> {
    for path in paths {
        visit_text(path, &mut |file| {
            let text = read_to_string(file)?;
            for (line_no, line) in text.lines().enumerate() {
                if has_windows_path(line) {
                    return Err(format!(
                        "{}:{} contains a Windows-style path",
                        file.display(),
                        line_no + 1
                    ));
                }
            }
            Ok(())
        })?;
    }
    Ok(())
}

fn check_public_badge_endpoints() -> Result<(), String> {
    let readme = read_to_string(&workspace_path("README.md"))?;
    let endpoint_prefix = "https://img.shields.io/endpoint?url=https%3A%2F%2Fraw.githubusercontent.com%2FEffortlessMetrics%2Funsafe-review%2Fmain%2Fbadges%2F";
    let endpoint_links = readme.matches(endpoint_prefix).count();
    if endpoint_links != PUBLIC_BADGE_ENDPOINTS.len() {
        return Err(format!(
            "README.md has {endpoint_links} public unsafe-review badge endpoint link(s), expected {}",
            PUBLIC_BADGE_ENDPOINTS.len()
        ));
    }

    for (path, label) in PUBLIC_BADGE_ENDPOINTS {
        let endpoint = public_badge_endpoint_url(path);
        if !readme.contains(&endpoint) {
            return Err(format!(
                "README.md is missing public badge endpoint `{endpoint}`"
            ));
        }
        let value = parse_json_file(&workspace_path(path))?;
        let schema = json_u64_at(&value, "/schemaVersion", path)?;
        if schema != 1 {
            return Err(format!("{path} schemaVersion is {schema}, expected 1"));
        }
        require_json_str(&value, "label", label, path)?;
        let message = require_non_empty_json_str(&value, "message", path)?;
        for forbidden in ["safe", "sound", "ub-free", "miri-clean", "proof"] {
            if text_contains_ignore_ascii_case(message, forbidden) {
                return Err(format!(
                    "{path} badge message must not imply `{forbidden}`: {message}"
                ));
            }
        }
        require_non_empty_json_str(&value, "color", path)?;
    }
    Ok(())
}

fn public_badge_endpoint_url(path: &str) -> String {
    let encoded_path = path.replace('/', "%2F");
    format!(
        "https://img.shields.io/endpoint?url=https%3A%2F%2Fraw.githubusercontent.com%2FEffortlessMetrics%2Funsafe-review%2Fmain%2F{encoded_path}"
    )
}

fn check_tracked_generated_artifacts() -> Result<(), String> {
    let output = Command::new("git")
        .args(["ls-files"])
        .output()
        .map_err(|err| format!("failed to run git ls-files: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    for path in String::from_utf8_lossy(&output.stdout).lines() {
        if is_forbidden_generated_path(path) {
            return Err(format!("generated artifact is tracked: {path}"));
        }
    }
    Ok(())
}

fn parse_toml_file(path: &Path) -> Result<toml::Value, String> {
    let text = read_to_string(path)?;
    text.parse::<toml::Table>()
        .map(toml::Value::Table)
        .map_err(|err| format!("{} is not valid TOML: {err}", path.display()))
}

fn parse_json_file(path: &Path) -> Result<serde_json::Value, String> {
    let text = read_to_string(path)?;
    serde_json::from_str(&text)
        .map_err(|err| format!("{} is not valid JSON: {err}", path.display()))
}

#[derive(Debug)]
struct AdvisoryCardFacts {
    class: String,
    priority: String,
    confidence: String,
    operation_family: String,
    file: String,
    line: u64,
    column: u64,
}

fn advisory_card_facts(
    cards: &serde_json::Value,
) -> Result<BTreeMap<String, AdvisoryCardFacts>, String> {
    let mut facts = BTreeMap::new();
    for card in json_array_at(cards, "/cards", "cards.json")? {
        let Some(id) = card.get("id").and_then(serde_json::Value::as_str) else {
            return Err("cards.json card is missing id".to_string());
        };
        let card_facts = AdvisoryCardFacts {
            class: require_non_empty_json_str(card, "class", "cards.json card")?.to_string(),
            priority: require_non_empty_json_str(card, "priority", "cards.json card")?.to_string(),
            confidence: require_non_empty_json_str(card, "confidence", "cards.json card")?
                .to_string(),
            operation_family: require_non_empty_json_str(
                card,
                "operation_family",
                "cards.json card",
            )?
            .to_string(),
            file: require_non_empty_json_str_at(card, "/site/file", "cards.json card")?.to_string(),
            line: json_u64_at(card, "/site/line", "cards.json card")?,
            column: json_u64_at(card, "/site/column", "cards.json card")?,
        };
        if facts.insert(id.to_string(), card_facts).is_some() {
            return Err(format!("cards.json contains duplicate card id `{id}`"));
        }
    }
    Ok(facts)
}

fn require_cards_output_shape(cards: &serde_json::Value) -> Result<(), String> {
    require_non_empty_json_str(cards, "schema_version", "cards.json")?;
    require_non_empty_json_str(cards, "scope", "cards.json")?;
    require_non_empty_json_str(cards, "mode", "cards.json")?;
    require_non_empty_json_str(cards, "root", "cards.json")?;
    for key in [
        "rust_files",
        "changed_rust_files",
        "unsafe_sites",
        "cards",
        "open_actionable_gaps",
        "contract_missing",
        "guard_missing",
        "guarded_unwitnessed",
        "unsafe_unreached",
        "requires_loom",
        "miri_unsupported",
        "static_unknown",
    ] {
        json_usize_at(cards, &format!("/summary/{key}"), "cards.json")?;
    }
    for card in json_array_at(cards, "/cards", "cards.json")? {
        require_card_json_shape(card)?;
    }
    Ok(())
}

fn require_pr_summary_shape(
    text: &str,
    card_count: usize,
    card_ids: &BTreeSet<String>,
    path: &Path,
) -> Result<(), String> {
    for needle in [
        "# unsafe-review PR summary",
        "- Scope:",
        "- Open actionable gaps:",
        "- Policy mode: `advisory`",
        "## Top card",
        "## Card table",
        "| ID | Class | Location | Operation | Missing evidence | Route | Next action |",
        "## Witness plan",
        "## Trust boundary",
    ] {
        require_text_contains(text, needle, path)?;
    }
    if card_count == 0 {
        require_text_contains(text, "No changed unsafe-review gaps were found.", path)?;
        require_text_contains(text, "No witness route is recommended", path)?;
        return Ok(());
    }
    for card_id in card_ids {
        require_text_contains(text, card_id, path)?;
    }
    Ok(())
}

fn require_card_json_shape(card: &serde_json::Value) -> Result<(), String> {
    require_non_empty_json_str(card, "id", "cards.json card")?;
    require_non_empty_json_str(card, "class", "cards.json card")?;
    require_non_empty_json_str(card, "priority", "cards.json card")?;
    require_non_empty_json_str(card, "confidence", "cards.json card")?;
    require_non_empty_json_str_at(card, "/site/file", "cards.json card")?;
    require_positive_json_u64_at(card, "/site/line", "cards.json card")?;
    require_positive_json_u64_at(card, "/site/column", "cards.json card")?;
    require_non_empty_json_str_at(card, "/site/kind", "cards.json card")?;
    require_non_empty_json_str_at(card, "/site/visibility", "cards.json card")?;
    require_json_bool_at(card, "/site/public_api_surface", "cards.json card")?;
    require_non_empty_json_str_at(card, "/site/snippet", "cards.json card")?;
    require_non_empty_json_str(card, "operation_family", "cards.json card")?;
    require_non_empty_json_array_at(card, "/hazards", "cards.json card")?;
    let obligations = require_non_empty_json_array_at(card, "/obligations", "cards.json card")?;
    let obligation_evidence =
        require_non_empty_json_array_at(card, "/obligation_evidence", "cards.json card")?;
    if obligation_evidence.len() != obligations.len() {
        return Err(format!(
            "cards.json card obligation_evidence has {} item(s), but obligations has {}",
            obligation_evidence.len(),
            obligations.len()
        ));
    }
    for evidence in obligation_evidence {
        require_obligation_evidence_shape(evidence)?;
    }
    require_non_empty_json_str(card, "contract", "cards.json card")?;
    require_non_empty_json_str(card, "discharge", "cards.json card")?;
    require_non_empty_json_str(card, "reach", "cards.json card")?;
    require_non_empty_json_str(card, "witness", "cards.json card")?;
    json_array_at(card, "/missing", "cards.json card")?;
    json_array_at(card, "/verify_commands", "cards.json card")?;
    Ok(())
}

fn require_obligation_evidence_shape(evidence: &serde_json::Value) -> Result<(), String> {
    require_non_empty_json_str(evidence, "key", "cards.json obligation_evidence")?;
    require_non_empty_json_str(evidence, "description", "cards.json obligation_evidence")?;
    for lane in ["contract", "discharge", "reach", "witness"] {
        require_json_bool_at(
            evidence,
            &format!("/{lane}/present"),
            "cards.json obligation_evidence",
        )?;
        require_non_empty_json_str_at(
            evidence,
            &format!("/{lane}/state"),
            "cards.json obligation_evidence",
        )?;
        require_non_empty_json_str_at(
            evidence,
            &format!("/{lane}/summary"),
            "cards.json obligation_evidence",
        )?;
    }
    Ok(())
}

fn json_array_at<'a>(
    value: &'a serde_json::Value,
    pointer: &str,
    path: &str,
) -> Result<&'a Vec<serde_json::Value>, String> {
    value
        .pointer(pointer)
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| format!("{path} is missing array at `{pointer}`"))
}

fn json_string_set_at(
    value: &serde_json::Value,
    pointer: &str,
    path: &str,
) -> Result<BTreeSet<String>, String> {
    let array = json_array_at(value, pointer, path)?;
    let mut strings = BTreeSet::new();
    for (idx, item) in array.iter().enumerate() {
        let Some(item) = item.as_str() else {
            return Err(format!(
                "{path} array at `{pointer}` item {idx} must be a string"
            ));
        };
        if !strings.insert(item.to_string()) {
            return Err(format!(
                "{path} array at `{pointer}` contains duplicate `{item}`"
            ));
        }
    }
    Ok(strings)
}

fn require_json_string_set_at(
    value: &serde_json::Value,
    pointer: &str,
    expected: &BTreeSet<String>,
    path: &str,
) -> Result<(), String> {
    let actual = json_string_set_at(value, pointer, path)?;
    if &actual == expected {
        Ok(())
    } else {
        Err(format!(
            "{path} array at `{pointer}` is {actual:?}, expected {expected:?}"
        ))
    }
}

fn json_usize_at(value: &serde_json::Value, pointer: &str, path: &str) -> Result<usize, String> {
    let number = json_u64_at(value, pointer, path)?;
    usize::try_from(number)
        .map_err(|err| format!("{path} integer at `{pointer}` is too large: {err}"))
}

fn json_u64_at(value: &serde_json::Value, pointer: &str, path: &str) -> Result<u64, String> {
    let Some(number) = value.pointer(pointer).and_then(serde_json::Value::as_u64) else {
        return Err(format!("{path} is missing unsigned integer at `{pointer}`"));
    };
    Ok(number)
}

fn require_json_usize_at(
    value: &serde_json::Value,
    pointer: &str,
    expected: usize,
    path: &str,
) -> Result<(), String> {
    let actual = json_usize_at(value, pointer, path)?;
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "{path} integer at `{pointer}` is {actual}, expected {expected}"
        ))
    }
}

fn require_toml_string(value: &toml::Value, key: &str, path: &str) -> Result<(), String> {
    match value.get(key).and_then(toml::Value::as_str) {
        Some(_) => Ok(()),
        None => Err(format!("{path} is missing string key `{key}`")),
    }
}

fn required_toml_string<'a>(
    value: &'a toml::Value,
    key: &str,
    path: &str,
) -> Result<&'a str, String> {
    let Some(value) = value.get(key).and_then(toml::Value::as_str) else {
        return Err(format!("{path} is missing string key `{key}`"));
    };
    if value.trim().is_empty() {
        Err(format!("{path} string key `{key}` is empty"))
    } else {
        Ok(value)
    }
}

fn required_target_string<'a>(
    target: &'a toml::map::Map<String, toml::Value>,
    key: &str,
    idx: usize,
) -> Result<&'a str, String> {
    let Some(value) = target.get(key).and_then(toml::Value::as_str) else {
        return Err(format!(
            "{DOGFOOD_MANIFEST} targets[{idx}] is missing string `{key}`"
        ));
    };
    if value.trim().is_empty() {
        Err(format!(
            "{DOGFOOD_MANIFEST} targets[{idx}] string `{key}` is empty"
        ))
    } else {
        Ok(value)
    }
}

fn check_dogfood_path(path: &str, idx: usize, key: &str) -> Result<(), String> {
    if path.starts_with('/') || has_windows_path(path) {
        return Err(format!(
            "{DOGFOOD_MANIFEST} targets[{idx}] {key} path must be relative and use forward slashes: {path}"
        ));
    }
    if path.contains("..") {
        return Err(format!(
            "{DOGFOOD_MANIFEST} targets[{idx}] {key} path must not contain `..`: {path}"
        ));
    }
    Ok(())
}

fn required_case_string<'a>(
    case: &'a toml::map::Map<String, toml::Value>,
    key: &str,
    idx: usize,
) -> Result<&'a str, String> {
    let Some(value) = case.get(key).and_then(toml::Value::as_str) else {
        return Err(format!(
            "fixtures/calibration.toml cases[{idx}] is missing string `{key}`"
        ));
    };
    if value.trim().is_empty() {
        Err(format!(
            "fixtures/calibration.toml cases[{idx}] string `{key}` is empty"
        ))
    } else {
        Ok(value)
    }
}

fn optional_case_string<'a>(
    case: &'a toml::map::Map<String, toml::Value>,
    key: &str,
    idx: usize,
) -> Result<Option<&'a str>, String> {
    let Some(value) = case.get(key) else {
        return Ok(None);
    };
    let Some(value) = value.as_str() else {
        return Err(format!(
            "fixtures/calibration.toml cases[{idx}] optional `{key}` must be a string"
        ));
    };
    if value.trim().is_empty() {
        return Err(format!(
            "fixtures/calibration.toml cases[{idx}] optional `{key}` is empty"
        ));
    }
    Ok(Some(value))
}

fn required_case_usize(
    case: &toml::map::Map<String, toml::Value>,
    key: &str,
    idx: usize,
) -> Result<usize, String> {
    let Some(value) = case.get(key).and_then(toml::Value::as_integer) else {
        return Err(format!(
            "fixtures/calibration.toml cases[{idx}] is missing integer `{key}`"
        ));
    };
    usize::try_from(value).map_err(|err| {
        format!("fixtures/calibration.toml cases[{idx}] integer `{key}` is invalid: {err}")
    })
}

fn json_str<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(serde_json::Value::as_str)
}

fn json_array_contains_str(value: &serde_json::Value, key: &str, needle: &str) -> bool {
    value
        .get(key)
        .and_then(serde_json::Value::as_array)
        .is_some_and(|items| {
            items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .any(|item| item == needle)
        })
}

fn require_json_str(
    value: &serde_json::Value,
    key: &str,
    expected: &str,
    path: &str,
) -> Result<(), String> {
    match value.get(key).and_then(serde_json::Value::as_str) {
        Some(actual) if actual == expected => Ok(()),
        Some(actual) => Err(format!(
            "{path} key `{key}` is `{actual}`, expected `{expected}`"
        )),
        None => Err(format!("{path} is missing string key `{key}`")),
    }
}

fn require_json_str_at(
    value: &serde_json::Value,
    pointer: &str,
    expected: &str,
    path: &str,
) -> Result<(), String> {
    match value.pointer(pointer).and_then(serde_json::Value::as_str) {
        Some(actual) if actual == expected => Ok(()),
        Some(actual) => Err(format!(
            "{path} string at `{pointer}` is `{actual}`, expected `{expected}`"
        )),
        None => Err(format!("{path} is missing string at `{pointer}`")),
    }
}

fn require_json_str_key_matches(
    value: &serde_json::Value,
    key: &str,
    expected: &str,
    path: &str,
) -> Result<(), String> {
    match value.get(key).and_then(serde_json::Value::as_str) {
        Some(actual) if actual == expected => Ok(()),
        Some(actual) => Err(format!(
            "{path} key `{key}` is `{actual}`, but cards.json has `{expected}`"
        )),
        None => Err(format!("{path} is missing string key `{key}`")),
    }
}

fn require_json_str_at_matches(
    value: &serde_json::Value,
    pointer: &str,
    expected: &str,
    path: &str,
) -> Result<(), String> {
    match value.pointer(pointer).and_then(serde_json::Value::as_str) {
        Some(actual) if actual == expected => Ok(()),
        Some(actual) => Err(format!(
            "{path} string at `{pointer}` is `{actual}`, but cards.json has `{expected}`"
        )),
        None => Err(format!("{path} is missing string at `{pointer}`")),
    }
}

fn require_json_u64_key_matches(
    value: &serde_json::Value,
    key: &str,
    expected: u64,
    path: &str,
) -> Result<(), String> {
    match value.get(key).and_then(serde_json::Value::as_u64) {
        Some(actual) if actual == expected => Ok(()),
        Some(actual) => Err(format!(
            "{path} key `{key}` is {actual}, but cards.json has {expected}"
        )),
        None => Err(format!("{path} is missing unsigned integer key `{key}`")),
    }
}

fn require_json_u64_at_matches(
    value: &serde_json::Value,
    pointer: &str,
    expected: u64,
    path: &str,
) -> Result<(), String> {
    match value.pointer(pointer).and_then(serde_json::Value::as_u64) {
        Some(actual) if actual == expected => Ok(()),
        Some(actual) => Err(format!(
            "{path} unsigned integer at `{pointer}` is {actual}, but cards.json has {expected}"
        )),
        None => Err(format!("{path} is missing unsigned integer at `{pointer}`")),
    }
}

fn require_json_array(value: &serde_json::Value, key: &str, path: &str) -> Result<(), String> {
    if value.get(key).is_some_and(serde_json::Value::is_array) {
        Ok(())
    } else {
        Err(format!("{path} is missing array key `{key}`"))
    }
}

fn require_non_empty_json_array_at<'a>(
    value: &'a serde_json::Value,
    pointer: &str,
    path: &str,
) -> Result<&'a Vec<serde_json::Value>, String> {
    let array = json_array_at(value, pointer, path)?;
    if array.is_empty() {
        Err(format!("{path} array at `{pointer}` is empty"))
    } else {
        Ok(array)
    }
}

fn require_json_bool_at(
    value: &serde_json::Value,
    pointer: &str,
    path: &str,
) -> Result<(), String> {
    if value
        .pointer(pointer)
        .is_some_and(serde_json::Value::is_boolean)
    {
        Ok(())
    } else {
        Err(format!("{path} is missing boolean at `{pointer}`"))
    }
}

fn require_non_empty_json_str<'a>(
    value: &'a serde_json::Value,
    key: &str,
    path: &str,
) -> Result<&'a str, String> {
    let Some(actual) = value.get(key).and_then(serde_json::Value::as_str) else {
        return Err(format!("{path} is missing string key `{key}`"));
    };
    if actual.trim().is_empty() {
        Err(format!("{path} string key `{key}` is empty"))
    } else {
        Ok(actual)
    }
}

fn require_non_empty_json_str_at<'a>(
    value: &'a serde_json::Value,
    pointer: &str,
    path: &str,
) -> Result<&'a str, String> {
    let Some(actual) = value.pointer(pointer).and_then(serde_json::Value::as_str) else {
        return Err(format!("{path} is missing string at `{pointer}`"));
    };
    if actual.trim().is_empty() {
        Err(format!("{path} string at `{pointer}` is empty"))
    } else {
        Ok(actual)
    }
}

fn require_positive_json_u64_at(
    value: &serde_json::Value,
    pointer: &str,
    path: &str,
) -> Result<(), String> {
    let Some(number) = value.pointer(pointer).and_then(serde_json::Value::as_u64) else {
        return Err(format!("{path} is missing unsigned integer at `{pointer}`"));
    };
    if number == 0 {
        Err(format!(
            "{path} unsigned integer at `{pointer}` must be greater than zero"
        ))
    } else {
        Ok(())
    }
}

fn require_sarif_run_shape(sarif: &serde_json::Value) -> Result<(), String> {
    require_json_str_at(
        sarif,
        "/runs/0/tool/driver/name",
        "unsafe-review",
        "cards.sarif",
    )?;
    require_non_empty_json_str_at(sarif, "/runs/0/tool/driver/semanticVersion", "cards.sarif")?;
    require_non_empty_json_str_at(sarif, "/runs/0/tool/driver/informationUri", "cards.sarif")?;
    json_array_at(sarif, "/runs/0/tool/driver/rules", "cards.sarif")?;
    require_non_empty_json_str_at(sarif, "/runs/0/properties/schemaVersion", "cards.sarif")?;
    require_non_empty_json_str_at(sarif, "/runs/0/properties/scope", "cards.sarif")?;
    require_non_empty_json_str_at(sarif, "/runs/0/properties/mode", "cards.sarif")?;
    require_json_str_at(
        sarif,
        "/runs/0/properties/policy",
        "advisory",
        "cards.sarif",
    )?;
    Ok(())
}

fn require_sarif_result_shape(result: &serde_json::Value) -> Result<(), String> {
    let rule_id = require_non_empty_json_str_at(result, "/ruleId", "cards.sarif result")?;
    let class = require_non_empty_json_str_at(result, "/properties/class", "cards.sarif result")?;
    if rule_id != class {
        return Err(format!(
            "cards.sarif result ruleId `{rule_id}` does not match properties.class `{class}`"
        ));
    }
    let level = require_non_empty_json_str_at(result, "/level", "cards.sarif result")?;
    if !matches!(level, "warning" | "note" | "none") {
        return Err(format!("cards.sarif result has unknown level `{level}`"));
    }
    require_non_empty_json_str_at(result, "/message/text", "cards.sarif result")?;
    require_non_empty_json_str_at(
        result,
        "/locations/0/physicalLocation/artifactLocation/uri",
        "cards.sarif result",
    )?;
    require_positive_json_u64_at(
        result,
        "/locations/0/physicalLocation/region/startLine",
        "cards.sarif result",
    )?;
    require_positive_json_u64_at(
        result,
        "/locations/0/physicalLocation/region/startColumn",
        "cards.sarif result",
    )?;
    require_non_empty_json_str_at(result, "/properties/priority", "cards.sarif result")?;
    require_non_empty_json_str_at(result, "/properties/confidence", "cards.sarif result")?;
    require_non_empty_json_str_at(result, "/properties/operationFamily", "cards.sarif result")?;
    require_non_empty_json_str_at(result, "/properties/operation", "cards.sarif result")?;
    json_array_at(result, "/properties/hazards", "cards.sarif result")?;
    json_array_at(result, "/properties/missingEvidence", "cards.sarif result")?;
    json_array_at(result, "/properties/witnessRoutes", "cards.sarif result")?;
    require_non_empty_json_str_at(result, "/properties/nextAction", "cards.sarif result")?;
    let boundary =
        require_non_empty_json_str_at(result, "/properties/trustBoundary", "cards.sarif result")?;
    require_boundary_text(boundary, "cards.sarif result")?;
    Ok(())
}

fn require_sarif_result_matches_card(
    result: &serde_json::Value,
    facts: &AdvisoryCardFacts,
) -> Result<(), String> {
    require_json_str_at_matches(
        result,
        "/properties/class",
        &facts.class,
        "cards.sarif result",
    )?;
    require_json_str_at_matches(
        result,
        "/properties/priority",
        &facts.priority,
        "cards.sarif result",
    )?;
    require_json_str_at_matches(
        result,
        "/properties/confidence",
        &facts.confidence,
        "cards.sarif result",
    )?;
    require_json_str_at_matches(
        result,
        "/properties/operationFamily",
        &facts.operation_family,
        "cards.sarif result",
    )?;
    require_json_str_at_matches(
        result,
        "/locations/0/physicalLocation/artifactLocation/uri",
        &facts.file,
        "cards.sarif result",
    )?;
    require_json_u64_at_matches(
        result,
        "/locations/0/physicalLocation/region/startLine",
        facts.line,
        "cards.sarif result",
    )?;
    require_json_u64_at_matches(
        result,
        "/locations/0/physicalLocation/region/startColumn",
        facts.column,
        "cards.sarif result",
    )?;
    Ok(())
}

fn require_comment_plan_comment_matches_card(
    comment: &serde_json::Value,
    facts: &AdvisoryCardFacts,
) -> Result<(), String> {
    require_json_str_key_matches(comment, "class", &facts.class, "comment-plan.json comment")?;
    require_json_str_key_matches(
        comment,
        "priority",
        &facts.priority,
        "comment-plan.json comment",
    )?;
    require_json_str_key_matches(
        comment,
        "confidence",
        &facts.confidence,
        "comment-plan.json comment",
    )?;
    require_json_str_key_matches(
        comment,
        "operation_family",
        &facts.operation_family,
        "comment-plan.json comment",
    )?;
    require_json_str_key_matches(comment, "path", &facts.file, "comment-plan.json comment")?;
    require_json_u64_key_matches(comment, "line", facts.line, "comment-plan.json comment")?;
    Ok(())
}

fn require_comment_line(comment: &serde_json::Value) -> Result<(), String> {
    let Some(line) = comment.get("line").and_then(serde_json::Value::as_u64) else {
        return Err("comment-plan.json comment is missing unsigned integer key `line`".to_string());
    };
    if line == 0 {
        Err("comment-plan.json comment key `line` must be greater than zero".to_string())
    } else {
        Ok(())
    }
}

fn require_text_contains(text: &str, needle: &str, path: &Path) -> Result<(), String> {
    if text_contains_ignore_ascii_case(text, needle) {
        Ok(())
    } else {
        Err(format!("{} is missing `{needle}`", path.display()))
    }
}

fn require_boundary_text(text: &str, path: &str) -> Result<(), String> {
    for needle in [
        "static unsafe contract review",
        "not a proof of memory safety",
        "not UB-free status",
        "not a Miri result",
    ] {
        if !text_contains_ignore_ascii_case(text, needle) {
            return Err(format!("{path} trust boundary is missing `{needle}`"));
        }
    }
    Ok(())
}

fn require_comment_body_boundary_text(text: &str, path: &str) -> Result<(), String> {
    for needle in [
        "static unsafe contract review",
        "not memory-safety proof",
        "not UB-free status",
        "not a Miri result",
        "unsafe-review did not post this comment",
    ] {
        if !text_contains_ignore_ascii_case(text, needle) {
            return Err(format!("{path} is missing `{needle}`"));
        }
    }
    Ok(())
}

fn text_contains_ignore_ascii_case(text: &str, needle: &str) -> bool {
    text.to_ascii_lowercase()
        .contains(&needle.to_ascii_lowercase())
}

fn require_file(path: &str) -> Result<(), String> {
    if Path::new(path).is_file() {
        Ok(())
    } else {
        Err(format!("required file missing: {path}"))
    }
}

fn require_repo_file(path: &str) -> Result<(), String> {
    if repo_path(path).is_file() {
        Ok(())
    } else {
        Err(format!("required file missing: {path}"))
    }
}

fn require_fixture_file(dir: &Path, relative: &str) -> Result<(), String> {
    let path = dir.join(relative);
    if path.is_file() {
        Ok(())
    } else {
        Err(format!("fixture {} is missing {relative}", dir.display()))
    }
}

fn read_to_string(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|err| format!("read {} failed: {err}", path.display()))
}

fn workspace_path(relative: &str) -> PathBuf {
    let current_dir_path = PathBuf::from(relative);
    if current_dir_path.exists() {
        current_dir_path
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(relative)
    }
}

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(relative)
}

fn fixture_dirs(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut dirs = Vec::new();
    let entries =
        fs::read_dir(dir).map_err(|err| format!("read {} failed: {err}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|err| format!("read_dir entry failed: {err}"))?;
        let path = entry.path();
        if path.is_dir() {
            dirs.push(path);
        }
    }
    dirs.sort();
    Ok(dirs)
}

fn markdown_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    visit_text(dir, &mut |path| {
        if path.extension().is_some_and(|ext| ext == "md") {
            files.push(path.to_path_buf());
        }
        Ok(())
    })?;
    files.sort();
    Ok(files)
}

fn visit_text(
    dir_or_file: &Path,
    f: &mut impl FnMut(&Path) -> Result<(), String>,
) -> Result<(), String> {
    if dir_or_file.is_file() {
        if is_text_file(dir_or_file) {
            f(dir_or_file)?;
        }
        return Ok(());
    }
    let entries = fs::read_dir(dir_or_file)
        .map_err(|err| format!("read {} failed: {err}", dir_or_file.display()))?;
    for entry in entries {
        let entry = entry.map_err(|err| format!("read_dir entry failed: {err}"))?;
        let path = entry.path();
        if path.is_dir() {
            visit_text(&path, f)?;
        } else if is_text_file(&path) {
            f(&path)?;
        }
    }
    Ok(())
}

fn is_text_file(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| matches!(ext.to_str(), Some("md" | "toml" | "yml" | "yaml" | "txt")))
}

#[cfg(test)]
fn support_tier_from_row(line: &str) -> Option<&str> {
    let Ok(Some(row)) = support_tier_row_from_line(line, "support tier table", 0) else {
        return None;
    };
    Some(row.tier)
}

fn support_capability_from_row(line: &str) -> Option<&str> {
    let Ok(Some(row)) = support_tier_row_from_line(line, "support tier table", 0) else {
        return None;
    };
    Some(row.capability)
}

struct SupportTierRow<'a> {
    capability: &'a str,
    tier: &'a str,
    proof: &'a str,
}

fn support_tier_row_from_line<'a>(
    line: &'a str,
    path: &str,
    line_no: usize,
) -> Result<Option<SupportTierRow<'a>>, String> {
    if !line.starts_with('|') || line.contains("---") || line.contains("Capability") {
        return Ok(None);
    }
    let columns = markdown_table_columns(line);
    if columns.len() != 5 {
        return Err(format!(
            "{path}:{line_no} support-tier rows must have 5 columns, found {}",
            columns.len()
        ));
    }
    for (idx, name) in [
        (0, "Capability"),
        (1, "Tier"),
        (2, "Surface"),
        (3, "Proof"),
        (4, "Known limits"),
    ] {
        reject_placeholder_cell(path, line_no, name, columns[idx])?;
    }
    Ok(Some(SupportTierRow {
        capability: columns[0],
        tier: columns[1],
        proof: columns[3],
    }))
}

fn markdown_table_columns(line: &str) -> Vec<&str> {
    let mut columns = Vec::new();
    let mut start = 0usize;
    let mut in_code = false;
    for (idx, ch) in line.char_indices() {
        if ch == '`' {
            in_code = !in_code;
        } else if ch == '|' && !in_code {
            let column = line[start..idx].trim();
            if !column.is_empty() {
                columns.push(column);
            }
            start = idx + ch.len_utf8();
        }
    }
    let column = line[start..].trim();
    if !column.is_empty() {
        columns.push(column);
    }
    columns
}

fn reject_placeholder_cell(
    path: &str,
    line_no: usize,
    column: &str,
    value: &str,
) -> Result<(), String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty()
        || matches!(
            normalized.as_str(),
            "-" | "n/a" | "na" | "none" | "todo" | "tbd" | "placeholder"
        )
    {
        Err(format!(
            "{path}:{line_no} `{column}` cell must not be empty or placeholder"
        ))
    } else {
        Ok(())
    }
}

fn support_proof_cell_has_evidence_term(proof: &str) -> bool {
    let proof = proof.to_ascii_lowercase();
    SUPPORT_PROOF_TERMS.iter().any(|term| proof.contains(term))
}

fn support_summary_posture_from_row(line: &str) -> Option<&str> {
    if !line.starts_with('|') || line.contains("---") || line.contains("Surface") {
        return None;
    }
    let columns = line
        .split('|')
        .map(str::trim)
        .filter(|column| !column.is_empty())
        .collect::<Vec<_>>();
    (columns.len() == 4).then(|| columns[1])
}

fn support_tier_capabilities() -> Result<BTreeSet<String>, String> {
    let path = workspace_path(SUPPORT_TIERS_DOC);
    let text = read_to_string(&path)?;
    let mut capabilities = BTreeSet::new();
    for line in text.lines() {
        if let Some(capability) = support_capability_from_row(line) {
            capabilities.insert(capability.to_string());
        }
    }
    if capabilities.is_empty() {
        return Err(format!("{} has no support-tier rows", path.display()));
    }
    Ok(capabilities)
}

fn fixture_dir_name(path: &Path) -> Result<&str, String> {
    path.file_name()
        .and_then(std::ffi::OsStr::to_str)
        .ok_or_else(|| format!("{} has a non-UTF-8 fixture directory name", path.display()))
}

fn fixture_package_prefix(name: &str) -> String {
    FIXTURE_PACKAGE_PREFIX_EXCEPTIONS
        .iter()
        .find_map(|(fixture, package_prefix)| (*fixture == name).then_some(*package_prefix))
        .unwrap_or(name)
        .replace('_', "-")
}

fn is_snake_case_name(name: &str) -> bool {
    let mut previous_underscore = false;
    let mut has_segment_char = false;
    for ch in name.chars() {
        match ch {
            'a'..='z' | '0'..='9' => {
                previous_underscore = false;
                has_segment_char = true;
            }
            '_' if has_segment_char && !previous_underscore => {
                previous_underscore = true;
            }
            _ => return false,
        }
    }
    has_segment_char && !previous_underscore
}

fn looks_like_git_diff(text: &str) -> bool {
    text.contains("diff --git") && text.contains("--- a/") && text.contains("+++ b/")
}

fn has_windows_path(line: &str) -> bool {
    line.contains(":\\") || line.contains("\\\\")
}

fn is_forbidden_generated_path(path: &str) -> bool {
    path.starts_with("target/")
        || (path.starts_with("badges/") && !is_public_badge_endpoint(path))
        || path.ends_with(".sarif")
        || path.ends_with(".profraw")
        || path.ends_with(".profdata")
}

fn is_public_badge_endpoint(path: &str) -> bool {
    PUBLIC_BADGE_ENDPOINTS
        .iter()
        .any(|(endpoint, _label)| *endpoint == path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn registry_view<'a>(
        families: &'a BTreeSet<String>,
        obligation_keys: &'a BTreeMap<String, BTreeSet<String>>,
        hazards: &'a BTreeMap<String, BTreeSet<String>>,
        fixture_proofs: &'a BTreeMap<String, BTreeSet<String>>,
        witness_routes: &'a BTreeMap<String, BTreeSet<String>>,
    ) -> OperationFamilyRegistryView<'a> {
        OperationFamilyRegistryView {
            families,
            obligation_keys,
            hazards,
            fixture_proofs,
            witness_routes,
        }
    }

    fn workflow_policy(path: &str, actions: &[&str]) -> WorkflowPolicyEntry {
        WorkflowPolicyEntry {
            path: path.to_string(),
            permissions: "contents: read".to_string(),
            actions: actions.iter().map(|action| (*action).to_string()).collect(),
        }
    }

    #[test]
    fn xtask_rejects_unexpected_trailing_args() -> Result<(), String> {
        let args = vec![
            "xtask".to_string(),
            "check-pr".to_string(),
            "unexpected".to_string(),
        ];
        let Err(err) = require_no_extra_args(&args, "check-pr") else {
            return Err("extra argument should be rejected".to_string());
        };

        assert!(err.contains("check-pr"));
        assert!(err.contains("unexpected"));
        require_no_extra_args(&args[..2], "check-pr")?;
        Ok(())
    }

    #[test]
    fn xtask_single_path_commands_reject_second_path() -> Result<(), String> {
        let args = vec![
            "xtask".to_string(),
            "check-advisory-artifacts".to_string(),
            "target/unsafe-review".to_string(),
            "extra".to_string(),
        ];
        let Err(err) = require_max_args(&args, "check-advisory-artifacts", 3) else {
            return Err("second artifact directory should be rejected".to_string());
        };

        assert!(err.contains("check-advisory-artifacts"));
        assert!(err.contains("extra"));
        require_max_args(&args[..3], "check-advisory-artifacts", 3)?;
        Ok(())
    }

    #[test]
    fn workflow_used_actions_extracts_yaml_uses_lines() {
        let text = r#"
permissions:
  contents: read
jobs:
  test:
    steps:
      - uses: actions/checkout@v6
      - uses: "dtolnay/rust-toolchain@1.95.0"
"#;

        let actions = workflow_used_actions(text);

        assert!(actions.contains("actions/checkout@v6"));
        assert!(actions.contains("dtolnay/rust-toolchain@1.95.0"));
    }

    #[test]
    fn workflow_policy_accepts_listed_actions_and_read_only_permission() -> Result<(), String> {
        let text = r#"
permissions:
  contents: read
jobs:
  test:
    steps:
      - uses: actions/checkout@v6
      - uses: dtolnay/rust-toolchain@1.95.0
"#;

        check_workflow_text_against_policy(
            ".github/workflows/ci.yml",
            text,
            &workflow_policy(
                ".github/workflows/ci.yml",
                &["actions/checkout@v6", "dtolnay/rust-toolchain@1.95.0"],
            ),
        )
    }

    #[test]
    fn workflow_policy_rejects_unlisted_actions() -> Result<(), String> {
        let text = r#"
permissions:
  contents: read
jobs:
  test:
    steps:
      - uses: actions/checkout@v6
      - uses: actions/setup-rust@v1
"#;

        let Err(err) = check_workflow_text_against_policy(
            ".github/workflows/ci.yml",
            text,
            &workflow_policy(".github/workflows/ci.yml", &["actions/checkout@v6"]),
        ) else {
            return Err("unlisted action should fail".to_string());
        };

        assert!(err.contains("actions/setup-rust@v1"));
        assert!(err.contains("not listed"));
        Ok(())
    }

    #[test]
    fn workflow_policy_rejects_stale_listed_actions() -> Result<(), String> {
        let text = r#"
permissions:
  contents: read
jobs:
  test:
    steps:
      - uses: actions/checkout@v6
"#;

        let Err(err) = check_workflow_text_against_policy(
            ".github/workflows/ci.yml",
            text,
            &workflow_policy(
                ".github/workflows/ci.yml",
                &["actions/checkout@v6", "dtolnay/rust-toolchain@1.95.0"],
            ),
        ) else {
            return Err("stale action should fail".to_string());
        };

        assert!(err.contains("dtolnay/rust-toolchain@1.95.0"));
        assert!(err.contains("does not use it"));
        Ok(())
    }

    #[test]
    fn workflow_policy_rejects_missing_read_only_permission() -> Result<(), String> {
        let text = r#"
jobs:
  test:
    steps:
      - uses: actions/checkout@v6
"#;

        let Err(err) = check_workflow_text_against_policy(
            ".github/workflows/ci.yml",
            text,
            &workflow_policy(".github/workflows/ci.yml", &["actions/checkout@v6"]),
        ) else {
            return Err("missing permissions should fail".to_string());
        };

        assert!(err.contains("contents: read"));
        Ok(())
    }

    #[test]
    fn support_tier_parser_reads_tier_column() {
        assert_eq!(
            support_tier_from_row("| Review cards | scaffold | CLI | proof | limit |"),
            Some("scaffold")
        );
        assert_eq!(support_tier_from_row("|---|---|"), None);
    }

    #[test]
    fn support_tier_rows_reject_placeholder_proof_cells() -> Result<(), String> {
        let text = "| Capability | Tier | Surface | Proof | Known limits |\n\
                    |---|---|---|---|---|\n\
                    | ReviewCard schema | experimental | CLI JSON | TBD | source-based only |\n";

        let Err(err) = check_support_tiers_text("support.md", text) else {
            return Err("placeholder proof should fail".to_string());
        };

        assert!(err.contains("Proof"));
        assert!(err.contains("placeholder"));
        Ok(())
    }

    #[test]
    fn support_tier_rows_require_proof_terms_for_usable_tiers() -> Result<(), String> {
        let text = "| Capability | Tier | Surface | Proof | Known limits |\n\
                    |---|---|---|---|---|\n\
                    | ReviewCard schema | experimental | CLI JSON | human-reviewed claim text | source-based only |\n";

        let Err(err) = check_support_tiers_text("support.md", text) else {
            return Err("proof without concrete evidence terms should fail".to_string());
        };

        assert!(err.contains("must name concrete evidence"));
        assert!(err.contains("ReviewCard schema"));
        Ok(())
    }

    #[test]
    fn support_tier_rows_accept_planned_placeholder_proof_source() -> Result<(), String> {
        let text = "| Capability | Tier | Surface | Proof | Known limits |\n\
                    |---|---|---|---|---|\n\
                    | Future adapter | deferred | optional adapter | ADR needed | not default |\n";

        check_support_tiers_text("support.md", text)
    }

    #[test]
    fn support_tier_rows_keep_code_span_pipes_inside_cells() -> Result<(), String> {
        let text = "| Capability | Tier | Surface | Proof | Known limits |\n\
                    |---|---|---|---|---|\n\
                    | Sanitizer receipt adapter | experimental | `--tool asan|msan|tsan|lsan` | parser tests cover saved logs | saved output only |\n";

        check_support_tiers_text("support.md", text)
    }

    #[test]
    fn support_capability_parser_reads_capability_column() {
        assert_eq!(
            support_capability_from_row("| Review cards | scaffold | CLI | proof | limit |"),
            Some("Review cards")
        );
        assert_eq!(support_capability_from_row("|---|---|"), None);
    }

    #[test]
    fn support_summary_parser_reads_current_posture_column() {
        assert_eq!(
            support_summary_posture_from_row(
                "| ReviewCard schema | Experimental | Fixture-backed | Stable schema |"
            ),
            Some("Experimental")
        );
        assert_eq!(
            support_summary_posture_from_row("| Label | Meaning |"),
            None
        );
        assert_eq!(
            support_summary_posture_from_row(
                "| Surface | Current posture | Evidence | Not claimed |"
            ),
            None
        );
    }

    #[test]
    fn support_summary_rejects_unknown_current_posture() -> Result<(), String> {
        let mut text = SUPPORT_SUMMARY_REQUIRED_PHRASES.join("\n");
        text.push_str(
            "\n| Surface | Current posture | Evidence | Not claimed |\n\
             |---|---|---|---|\n\
             | ReviewCard schema | Unsupported | Fixture-backed | Safety |\n",
        );

        let Err(err) = check_support_summary_text(SUPPORT_SUMMARY_DOC, &text) else {
            return Err("unknown support summary posture should fail".to_string());
        };

        assert!(err.contains("unknown support summary posture"));
        assert!(err.contains("Unsupported"));
        Ok(())
    }

    #[test]
    fn zero_card_calibration_cases_reject_card_expectations() -> Result<(), String> {
        let mut case = toml::map::Map::new();
        case.insert(
            "expected_class".to_string(),
            toml::Value::String("guard_missing".to_string()),
        );
        let Err(err) = check_zero_card_expectations(&case, 7) else {
            return Err("zero-card case with expected_class should fail".to_string());
        };
        assert!(err.contains("cases[7]"));
        assert!(err.contains("expected_class"));
        Ok(())
    }

    #[test]
    fn calibration_cases_reject_unknown_fields() -> Result<(), String> {
        let mut case = toml::map::Map::new();
        case.insert(
            "expected_hazards".to_string(),
            toml::Value::String("alignment".to_string()),
        );
        let Err(err) = check_calibration_case_fields(&case, 2) else {
            return Err("calibration case with unknown field should fail".to_string());
        };
        assert!(err.contains("cases[2]"));
        assert!(err.contains("expected_hazards"));
        Ok(())
    }

    #[test]
    fn optional_calibration_strings_reject_wrong_type() -> Result<(), String> {
        let mut case = toml::map::Map::new();
        case.insert("expected_hazard".to_string(), toml::Value::Integer(1));
        let Err(err) = optional_case_string(&case, "expected_hazard", 9) else {
            return Err("wrong-typed optional calibration field should fail".to_string());
        };
        assert!(err.contains("cases[9]"));
        assert!(err.contains("expected_hazard"));
        Ok(())
    }

    #[test]
    fn calibration_kind_card_counts_match_semantics() -> Result<(), String> {
        let Err(err) = check_calibration_kind_card_count("positive", 0, 3) else {
            return Err("positive calibration case with zero cards should fail".to_string());
        };
        assert!(err.contains("cases[3]"));
        assert!(err.contains("positive"));

        let Err(err) = check_calibration_kind_card_count("negative", 1, 4) else {
            return Err("negative calibration case with cards should fail".to_string());
        };
        assert!(err.contains("cases[4]"));
        assert!(err.contains("negative"));

        check_calibration_kind_card_count("positive", 1, 5)?;
        check_calibration_kind_card_count("negative", 0, 6)?;
        check_calibration_kind_card_count("false_positive_control", 0, 7)?;
        check_calibration_kind_card_count("false_positive_control", 1, 8)?;
        Ok(())
    }

    #[test]
    fn fixture_names_must_be_snake_case() {
        assert!(is_snake_case_name("raw_pointer_deref"));
        assert!(is_snake_case_name("ffi_sanitizer_route"));
        assert!(!is_snake_case_name("RawPointerDeref"));
        assert!(!is_snake_case_name("raw-pointer-deref"));
        assert!(!is_snake_case_name("raw_pointer_deref_"));
        assert!(!is_snake_case_name("_raw_pointer_deref"));
    }

    #[test]
    fn fixture_dir_name_reads_last_path_component() -> Result<(), String> {
        assert_eq!(
            fixture_dir_name(Path::new("fixtures/raw_pointer_deref"))?,
            "raw_pointer_deref"
        );
        Ok(())
    }

    #[test]
    fn expected_card_exceptions_are_exact_fixture_names() {
        assert!(FIXTURE_EXPECTED_CARDS_EXCEPTIONS.contains(&"duplicate_raw_pointer_reads"));
        assert!(FIXTURE_EXPECTED_CARDS_EXCEPTIONS.contains(&"raw_pointer_alignment_line_drift"));
        assert!(!FIXTURE_EXPECTED_CARDS_EXCEPTIONS.contains(&"raw_pointer_alignment"));
    }

    #[test]
    fn fixture_exception_ledgers_reference_current_fixtures() -> Result<(), String> {
        let dirs = fixture_dirs(&workspace_path("fixtures"))?;
        check_fixture_exception_ledgers(&dirs)
    }

    #[test]
    fn calibration_manifest_validates_current_fixture_contract() -> Result<(), String> {
        check_calibration()
    }

    #[test]
    fn operation_registry_rejects_missing_fixture_backed_family() -> Result<(), String> {
        let mut families = operation_family_registry_rows()?;
        let fixture_proofs = operation_family_registry_fixture_proofs()?;
        families.insert("new_unregistered_family".to_string());

        let Err(err) = check_operation_family_registry_coverage(&families, &fixture_proofs) else {
            return Err("unregistered calibration family should fail".to_string());
        };

        assert!(err.contains("missing operation_family row"));
        assert!(err.contains("new_unregistered_family"));
        Ok(())
    }

    #[test]
    fn operation_registry_rejects_unbacked_registry_row() -> Result<(), String> {
        let mut families = operation_family_registry_rows()?;
        let fixture_proofs = operation_family_registry_fixture_proofs()?;
        families.remove("unknown");

        let Err(err) = check_operation_family_registry_coverage(&families, &fixture_proofs) else {
            return Err("registry row without calibration family should fail".to_string());
        };

        assert!(err.contains("without fixture-backed calibration"));
        assert!(err.contains("unknown"));
        Ok(())
    }

    #[test]
    fn operation_registry_rejects_wrong_family_fixture_proof() -> Result<(), String> {
        let calibration_families = BTreeSet::from(["raw_pointer_read".to_string()]);
        let calibration_fixtures = BTreeMap::from([(
            "raw_pointer_read".to_string(),
            BTreeSet::from(["raw_pointer_alignment".to_string()]),
        )]);
        let registry_families = BTreeSet::from(["raw_pointer_read".to_string()]);
        let registry_hazards = BTreeMap::from([(
            "raw_pointer_read".to_string(),
            BTreeSet::from(["pointer_validity".to_string()]),
        )]);
        let registry_obligation_keys = BTreeMap::from([(
            "raw_pointer_read".to_string(),
            BTreeSet::from(["pointer-live".to_string()]),
        )]);
        let known_operation_families = BTreeSet::from(["raw_pointer_read".to_string()]);
        let known_obligation_keys = BTreeSet::from(["pointer-live".to_string()]);
        let known_hazards = BTreeSet::from(["pointer_validity".to_string()]);
        let known_witness_routes = BTreeSet::from(["miri".to_string()]);
        let registry_fixtures = BTreeMap::from([(
            "raw_pointer_read".to_string(),
            BTreeSet::from(["raw_pointer_write_assignment".to_string()]),
        )]);
        let registry_routes = BTreeMap::from([(
            "raw_pointer_read".to_string(),
            BTreeSet::from(["miri".to_string()]),
        )]);
        let registry = registry_view(
            &registry_families,
            &registry_obligation_keys,
            &registry_hazards,
            &registry_fixtures,
            &registry_routes,
        );

        let Err(err) = check_operation_family_registry_coverage_with_registry(
            &calibration_families,
            &calibration_fixtures,
            &known_operation_families,
            &known_obligation_keys,
            &known_hazards,
            &known_witness_routes,
            &registry,
        ) else {
            return Err("wrong-family fixture proof should fail".to_string());
        };

        assert!(err.contains("cites fixture proof"));
        assert!(err.contains("raw_pointer_write_assignment"));
        Ok(())
    }

    #[test]
    fn operation_registry_rejects_unknown_operation_family() -> Result<(), String> {
        let calibration_families = BTreeSet::from(["spooky_operation".to_string()]);
        let calibration_fixtures = BTreeMap::from([(
            "spooky_operation".to_string(),
            BTreeSet::from(["raw_pointer_alignment".to_string()]),
        )]);
        let registry_families = BTreeSet::from(["spooky_operation".to_string()]);
        let known_operation_families = BTreeSet::from(["raw_pointer_read".to_string()]);
        let known_obligation_keys = BTreeSet::from(["pointer-live".to_string()]);
        let known_hazards = BTreeSet::from(["pointer_validity".to_string()]);
        let known_witness_routes = BTreeSet::from(["miri".to_string()]);
        let registry_obligation_keys = BTreeMap::from([(
            "spooky_operation".to_string(),
            BTreeSet::from(["pointer-live".to_string()]),
        )]);
        let registry_hazards = BTreeMap::from([(
            "spooky_operation".to_string(),
            BTreeSet::from(["pointer_validity".to_string()]),
        )]);
        let registry_fixtures = BTreeMap::from([(
            "spooky_operation".to_string(),
            BTreeSet::from(["raw_pointer_alignment".to_string()]),
        )]);
        let registry_routes = BTreeMap::from([(
            "spooky_operation".to_string(),
            BTreeSet::from(["miri".to_string()]),
        )]);
        let registry = registry_view(
            &registry_families,
            &registry_obligation_keys,
            &registry_hazards,
            &registry_fixtures,
            &registry_routes,
        );

        let Err(err) = check_operation_family_registry_coverage_with_registry(
            &calibration_families,
            &calibration_fixtures,
            &known_operation_families,
            &known_obligation_keys,
            &known_hazards,
            &known_witness_routes,
            &registry,
        ) else {
            return Err("unknown operation family should fail".to_string());
        };

        assert!(err.contains("unknown operation_family"));
        assert!(err.contains("spooky_operation"));
        Ok(())
    }

    #[test]
    fn operation_registry_rejects_unknown_hazard() -> Result<(), String> {
        let calibration_families = BTreeSet::from(["raw_pointer_read".to_string()]);
        let calibration_fixtures = BTreeMap::from([(
            "raw_pointer_read".to_string(),
            BTreeSet::from(["raw_pointer_alignment".to_string()]),
        )]);
        let registry_families = BTreeSet::from(["raw_pointer_read".to_string()]);
        let registry_hazards = BTreeMap::from([(
            "raw_pointer_read".to_string(),
            BTreeSet::from(["spooky_action".to_string()]),
        )]);
        let registry_obligation_keys = BTreeMap::from([(
            "raw_pointer_read".to_string(),
            BTreeSet::from(["pointer-live".to_string()]),
        )]);
        let known_operation_families = BTreeSet::from(["raw_pointer_read".to_string()]);
        let known_obligation_keys = BTreeSet::from(["pointer-live".to_string()]);
        let known_hazards = BTreeSet::from(["pointer_validity".to_string()]);
        let known_witness_routes = BTreeSet::from(["miri".to_string()]);
        let registry_fixtures = BTreeMap::from([(
            "raw_pointer_read".to_string(),
            BTreeSet::from(["raw_pointer_alignment".to_string()]),
        )]);
        let registry_routes = BTreeMap::from([(
            "raw_pointer_read".to_string(),
            BTreeSet::from(["miri".to_string()]),
        )]);
        let registry = registry_view(
            &registry_families,
            &registry_obligation_keys,
            &registry_hazards,
            &registry_fixtures,
            &registry_routes,
        );

        let Err(err) = check_operation_family_registry_coverage_with_registry(
            &calibration_families,
            &calibration_fixtures,
            &known_operation_families,
            &known_obligation_keys,
            &known_hazards,
            &known_witness_routes,
            &registry,
        ) else {
            return Err("unknown hazard should fail".to_string());
        };

        assert!(err.contains("unknown hazard"));
        assert!(err.contains("spooky_action"));
        Ok(())
    }

    #[test]
    fn operation_registry_rejects_unknown_obligation_key() -> Result<(), String> {
        let calibration_families = BTreeSet::from(["raw_pointer_read".to_string()]);
        let calibration_fixtures = BTreeMap::from([(
            "raw_pointer_read".to_string(),
            BTreeSet::from(["raw_pointer_alignment".to_string()]),
        )]);
        let registry_families = BTreeSet::from(["raw_pointer_read".to_string()]);
        let registry_obligation_keys = BTreeMap::from([(
            "raw_pointer_read".to_string(),
            BTreeSet::from(["shape-proof".to_string()]),
        )]);
        let registry_hazards = BTreeMap::from([(
            "raw_pointer_read".to_string(),
            BTreeSet::from(["pointer_validity".to_string()]),
        )]);
        let known_operation_families = BTreeSet::from(["raw_pointer_read".to_string()]);
        let known_obligation_keys = BTreeSet::from(["pointer-live".to_string()]);
        let known_hazards = BTreeSet::from(["pointer_validity".to_string()]);
        let known_witness_routes = BTreeSet::from(["miri".to_string()]);
        let registry_fixtures = BTreeMap::from([(
            "raw_pointer_read".to_string(),
            BTreeSet::from(["raw_pointer_alignment".to_string()]),
        )]);
        let registry_routes = BTreeMap::from([(
            "raw_pointer_read".to_string(),
            BTreeSet::from(["miri".to_string()]),
        )]);
        let registry = registry_view(
            &registry_families,
            &registry_obligation_keys,
            &registry_hazards,
            &registry_fixtures,
            &registry_routes,
        );

        let Err(err) = check_operation_family_registry_coverage_with_registry(
            &calibration_families,
            &calibration_fixtures,
            &known_operation_families,
            &known_obligation_keys,
            &known_hazards,
            &known_witness_routes,
            &registry,
        ) else {
            return Err("unknown obligation key should fail".to_string());
        };

        assert!(err.contains("unknown obligation/evidence key"));
        assert!(err.contains("shape-proof"));
        Ok(())
    }

    #[test]
    fn operation_registry_rejects_unknown_witness_route() -> Result<(), String> {
        let calibration_families = BTreeSet::from(["raw_pointer_read".to_string()]);
        let calibration_fixtures = BTreeMap::from([(
            "raw_pointer_read".to_string(),
            BTreeSet::from(["raw_pointer_alignment".to_string()]),
        )]);
        let registry_families = BTreeSet::from(["raw_pointer_read".to_string()]);
        let registry_hazards = BTreeMap::from([(
            "raw_pointer_read".to_string(),
            BTreeSet::from(["pointer_validity".to_string()]),
        )]);
        let registry_obligation_keys = BTreeMap::from([(
            "raw_pointer_read".to_string(),
            BTreeSet::from(["pointer-live".to_string()]),
        )]);
        let known_operation_families = BTreeSet::from(["raw_pointer_read".to_string()]);
        let known_obligation_keys = BTreeSet::from(["pointer-live".to_string()]);
        let known_hazards = BTreeSet::from(["pointer_validity".to_string()]);
        let known_witness_routes = BTreeSet::from(["miri".to_string()]);
        let registry_fixtures = BTreeMap::from([(
            "raw_pointer_read".to_string(),
            BTreeSet::from(["raw_pointer_alignment".to_string()]),
        )]);
        let registry_routes = BTreeMap::from([(
            "raw_pointer_read".to_string(),
            BTreeSet::from(["vibes".to_string()]),
        )]);
        let registry = registry_view(
            &registry_families,
            &registry_obligation_keys,
            &registry_hazards,
            &registry_fixtures,
            &registry_routes,
        );

        let Err(err) = check_operation_family_registry_coverage_with_registry(
            &calibration_families,
            &calibration_fixtures,
            &known_operation_families,
            &known_obligation_keys,
            &known_hazards,
            &known_witness_routes,
            &registry,
        ) else {
            return Err("unknown witness route should fail".to_string());
        };

        assert!(err.contains("unknown witness route"));
        assert!(err.contains("vibes"));
        Ok(())
    }

    #[test]
    fn operation_registry_parser_rejects_duplicate_rows() -> Result<(), String> {
        let text = "| `raw_pointer_read` | shape | hazards | not hazards | keys | route | fixtures | controls | limits |\n| `raw_pointer_read` | shape | hazards | not hazards | keys | route | fixtures | controls | limits |\n";

        let Err(err) = operation_family_registry_rows_from_text(text) else {
            return Err("duplicate registry row should fail".to_string());
        };

        assert!(err.contains("duplicate operation_family row"));
        assert!(err.contains("raw_pointer_read"));
        Ok(())
    }

    #[test]
    fn operation_registry_parser_rejects_wrong_column_count() -> Result<(), String> {
        let text = "| `raw_pointer_read` | shape | hazards |\n";

        let Err(err) = operation_family_registry_rows_from_text(text) else {
            return Err("wrong registry row shape should fail".to_string());
        };

        assert!(err.contains("must have 9 columns"));
        assert!(err.contains("raw_pointer_read"));
        Ok(())
    }

    #[test]
    fn operation_registry_parser_rejects_placeholder_required_text() -> Result<(), String> {
        let text = "| `raw_pointer_read` | todo | hazards | none | keys | miri | `raw_pointer_alignment` | controls | limits |\n";

        let Err(err) = operation_family_registry_rows_from_text(text) else {
            return Err("placeholder registry text should fail".to_string());
        };

        assert!(err.contains("detected syntax shapes"));
        assert!(err.contains("raw_pointer_read"));
        assert!(err.contains("todo"));
        Ok(())
    }

    #[test]
    fn operation_registry_parser_rejects_unknown_obligation_keys_for_concrete_family()
    -> Result<(), String> {
        let text = "| `raw_pointer_read` | shape | hazards | none | unknown | miri | `raw_pointer_alignment` | controls | limits |\n";

        let Err(err) = operation_family_registry_rows_from_text(text) else {
            return Err("unknown obligation keys should fail for concrete families".to_string());
        };

        assert!(err.contains("obligation / evidence keys"));
        assert!(err.contains("raw_pointer_read"));
        assert!(err.contains("unknown"));
        Ok(())
    }

    #[test]
    fn operation_registry_parser_allows_unknown_obligation_keys_for_unknown_family()
    -> Result<(), String> {
        let text = "| `unknown` | changed unsafe fallback shapes | unknown | none | unknown | human-deep-review | `split_unsafe_block` | concrete operation cards suppress duplicate wrapper cards | unknown is a review fallback, not proof |\n";

        let rows = operation_family_registry_rows_from_text(text)?;

        assert!(rows.contains("unknown"));
        Ok(())
    }

    #[test]
    fn operation_registry_parser_extracts_obligation_key_names() -> Result<(), String> {
        let text = "| `raw_pointer_read` | shape | hazards | none | pointer-live, bounds, alignment | miri | `raw_pointer_alignment` | controls | limits |\n";

        let keys = operation_family_registry_obligation_keys_from_text(text)?;
        let Some(keys) = keys.get("raw_pointer_read") else {
            return Err("raw_pointer_read obligation key row should be parsed".to_string());
        };

        assert!(keys.contains("pointer-live"));
        assert!(keys.contains("bounds"));
        assert!(keys.contains("alignment"));
        assert_eq!(keys.len(), 3);
        Ok(())
    }

    #[test]
    fn safety_obligation_parser_extracts_new_labels() -> Result<(), String> {
        let text = r#"
OperationFamily::RawPointerRead => vec![
    SafetyObligation::new("pointer-live", "pointer is live"),
    SafetyObligation::new("alignment", "pointer is aligned"),
    SafetyObligation::new(
        "state-transition",
        "state transition is valid",
    ),
],
"#;

        let labels = safety_obligation_labels_from_text(text)?;

        assert!(labels.contains("pointer-live"));
        assert!(labels.contains("alignment"));
        assert!(labels.contains("state-transition"));
        assert_eq!(labels.len(), 3);
        Ok(())
    }

    #[test]
    fn operation_registry_parser_rejects_prose_obligation_keys() -> Result<(), String> {
        let text = "| `raw_pointer_read` | shape | hazards | none | pointer live proof | miri | `raw_pointer_alignment` | controls | limits |\n";

        let Err(err) = operation_family_registry_rows_from_text(text) else {
            return Err("prose obligation keys should fail".to_string());
        };

        assert!(err.contains("invalid key token"));
        assert!(err.contains("pointer live proof"));
        Ok(())
    }

    #[test]
    fn registry_key_tokens_are_machine_readable() {
        assert!(is_registry_key_token("pointer-live"));
        assert!(is_registry_key_token("utf8"));
        assert!(is_registry_key_token("valid-zero"));
        assert!(!is_registry_key_token("pointer live"));
        assert!(!is_registry_key_token("PointerLive"));
        assert!(!is_registry_key_token("pointer_live"));
        assert!(!is_registry_key_token("pointer--live"));
    }

    #[test]
    fn operation_registry_header_accepts_expected_columns() -> Result<(), String> {
        let text = "| operation_family | detected syntax shapes | hazards | not hazards | obligation / evidence keys | witness route | fixture proof | known false-positive controls | known limits |\n";

        check_operation_family_registry_header_from_text(text)
    }

    #[test]
    fn operation_registry_header_rejects_renamed_columns() -> Result<(), String> {
        let text = "| operation_family | syntax | hazards | not hazards | obligation / evidence keys | witness route | fixture proof | known false-positive controls | known limits |\n";

        let Err(err) = check_operation_family_registry_header_from_text(text) else {
            return Err("renamed registry header should fail".to_string());
        };

        assert!(err.contains("registry header must be"));
        assert!(err.contains("detected syntax shapes"));
        Ok(())
    }

    #[test]
    fn handoff_index_validates_current_closeout_docs() -> Result<(), String> {
        check_handoff_index(
            &repo_path("docs/handoffs"),
            &repo_path("docs/handoffs/README.md"),
        )
    }

    #[test]
    fn docs_map_paths_point_at_existing_repository_files() -> Result<(), String> {
        check_docs_map_paths("../docs/README.md")
    }

    #[test]
    fn front_door_markdown_links_point_at_existing_local_targets() -> Result<(), String> {
        for path in FRONT_DOOR_MARKDOWN_DOCS {
            check_markdown_local_links(path)?;
        }
        Ok(())
    }

    #[test]
    fn markdown_link_target_parser_finds_plain_local_links() {
        let targets = markdown_link_targets(
            "[First use](docs/FIRST_USE.md) [external](https://example.com) [anchor](#trust)",
        );

        assert!(targets.contains(&"docs/FIRST_USE.md".to_string()));
        assert!(targets.contains(&"https://example.com".to_string()));
        assert!(targets.contains(&"#trust".to_string()));
        assert_eq!(
            local_markdown_link_target("docs/FIRST_USE.md#install"),
            Some("docs/FIRST_USE.md")
        );
        assert_eq!(local_markdown_link_target("https://example.com"), None);
        assert_eq!(local_markdown_link_target("#trust"), None);
    }

    #[test]
    fn markdown_code_span_parser_extracts_backticked_paths() {
        let spans = markdown_code_spans(
            "| Layer | Path |\n|---|---|\n| Docs | `docs/README.md`, `policy/` |\n",
        );

        assert!(spans.contains(&"docs/README.md".to_string()));
        assert!(spans.contains(&"policy/".to_string()));
    }

    #[test]
    fn operation_family_parser_extracts_as_str_labels() -> Result<(), String> {
        let text = r#"
impl UnsafeSiteKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UnsafeBlock => "unsafe_block",
        }
    }
}

impl OperationFamily {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RawPointerRead => "raw_pointer_read",
            Self::RawPointerWrite => "raw_pointer_write",
        }
    }
}

pub struct UnsafeOperation;
"#;

        let labels = operation_family_labels_from_text(text)?;

        assert!(labels.contains("raw_pointer_read"));
        assert!(labels.contains("raw_pointer_write"));
        assert!(!labels.contains("unsafe_block"));
        assert_eq!(labels.len(), 2);
        Ok(())
    }

    #[test]
    fn hazard_kind_parser_extracts_as_str_labels() -> Result<(), String> {
        let text = r#"
impl HazardKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PointerValidity => "pointer_validity",
            Self::Alignment => "alignment",
        }
    }
}
"#;

        let labels = hazard_kind_labels_from_text(text)?;

        assert!(labels.contains("pointer_validity"));
        assert!(labels.contains("alignment"));
        assert_eq!(labels.len(), 2);
        Ok(())
    }

    #[test]
    fn witness_kind_parser_extracts_as_str_labels() -> Result<(), String> {
        let text = r#"
impl WitnessKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Miri => "miri",
            Self::CargoCareful => "cargo-careful",
            Self::HumanDeepReview => "human-deep-review",
        }
    }
}
"#;

        let labels = witness_kind_labels_from_text(text)?;

        assert!(labels.contains("miri"));
        assert!(labels.contains("cargo-careful"));
        assert!(labels.contains("human-deep-review"));
        assert_eq!(labels.len(), 3);
        Ok(())
    }

    #[test]
    fn operation_registry_parser_extracts_hazard_names() -> Result<(), String> {
        let text = "| `raw_pointer_read` | shape | pointer_validity, alignment, initialized_memory | none | keys | miri | `raw_pointer_alignment` | controls | limits |\n";

        let hazards = operation_family_registry_hazards_from_text(text)?;
        let Some(hazards) = hazards.get("raw_pointer_read") else {
            return Err("raw_pointer_read hazard row should be parsed".to_string());
        };

        assert!(hazards.contains("pointer_validity"));
        assert!(hazards.contains("alignment"));
        assert!(hazards.contains("initialized_memory"));
        assert_eq!(hazards.len(), 3);
        Ok(())
    }

    #[test]
    fn operation_registry_parser_rejects_empty_hazard_column() -> Result<(), String> {
        let text = "| `raw_pointer_read` | shape | ??? | none | keys | miri | `raw_pointer_alignment` | controls | limits |\n";

        let Err(err) = operation_family_registry_hazards_from_text(text) else {
            return Err("empty hazard column should fail".to_string());
        };

        assert!(err.contains("hazard column has no hazard names"));
        assert!(err.contains("raw_pointer_read"));
        Ok(())
    }

    #[test]
    fn operation_registry_parser_extracts_fixture_proof_names() -> Result<(), String> {
        let text = "| `raw_pointer_read` | shape | hazards | none | keys | route | `raw_pointer_alignment`, `split_raw_pointer_read_call` | controls | limits |\n";

        let proofs = operation_family_registry_fixture_proofs_from_text(text)?;
        let Some(fixtures) = proofs.get("raw_pointer_read") else {
            return Err("raw_pointer_read proof row should be parsed".to_string());
        };

        assert!(fixtures.contains("raw_pointer_alignment"));
        assert!(fixtures.contains("split_raw_pointer_read_call"));
        assert_eq!(fixtures.len(), 2);
        Ok(())
    }

    #[test]
    fn operation_registry_parser_extracts_witness_route_names() -> Result<(), String> {
        let text = "| `transmute` | shape | hazards | none | keys | miri -> kani/crux -> human-deep-review | `transmute_invalid_value` | controls | limits |\n";

        let routes = operation_family_registry_witness_routes_from_text(text)?;
        let Some(routes) = routes.get("transmute") else {
            return Err("transmute route row should be parsed".to_string());
        };

        assert!(routes.contains("miri"));
        assert!(routes.contains("kani"));
        assert!(routes.contains("crux"));
        assert!(routes.contains("human-deep-review"));
        assert_eq!(routes.len(), 4);
        Ok(())
    }

    #[test]
    fn operation_registry_parser_rejects_empty_witness_route() -> Result<(), String> {
        let text = "| `raw_pointer_read` | shape | hazards | none | keys | ??? | `raw_pointer_alignment` | controls | limits |\n";

        let Err(err) = operation_family_registry_witness_routes_from_text(text) else {
            return Err("empty witness route should fail".to_string());
        };

        assert!(err.contains("witness route column has no route names"));
        assert!(err.contains("raw_pointer_read"));
        Ok(())
    }

    #[test]
    fn operation_registry_parser_rejects_empty_fixture_proof() -> Result<(), String> {
        let text = "| `raw_pointer_read` | shape | hazards | none | keys | route | none | controls | limits |\n";

        let Err(err) = operation_family_registry_fixture_proofs_from_text(text) else {
            return Err("empty fixture proof should fail".to_string());
        };

        assert!(err.contains("fixture proof column has no fixture names"));
        assert!(err.contains("raw_pointer_read"));
        Ok(())
    }

    #[test]
    fn operation_registry_parser_rejects_empty_registry() -> Result<(), String> {
        let text = "| operation_family | hazards |\n|---|---|\n";

        let Err(err) = operation_family_registry_rows_from_text(text) else {
            return Err("empty registry should fail".to_string());
        };

        assert!(err.contains("contains no operation_family registry rows"));
        Ok(())
    }

    #[test]
    fn dogfood_manifest_validates_current_corpus_contract() -> Result<(), String> {
        check_dogfood()
    }

    #[test]
    fn dogfood_index_target_sets_reject_missing_manifest_target() -> Result<(), String> {
        let row = serde_json::json!({
            "snapshot_targets": ["smallvec-capped"],
        });
        let expected = [
            "smallvec-capped".to_string(),
            "async-task-capped".to_string(),
        ]
        .into_iter()
        .collect();

        let Err(err) =
            require_json_string_set_at(&row, "/snapshot_targets", &expected, "dogfood-index")
        else {
            return Err("missing dogfood target should fail".to_string());
        };

        assert!(err.contains("async-task-capped"));
        assert!(err.contains("expected"));
        Ok(())
    }

    #[test]
    fn dogfood_index_target_sets_reject_duplicate_index_target() -> Result<(), String> {
        let row = serde_json::json!({
            "pr_diff_targets": ["hashbrown-pr692", "hashbrown-pr692"],
        });

        let Err(err) = json_string_set_at(&row, "/pr_diff_targets", "dogfood-index") else {
            return Err("duplicate dogfood target should fail".to_string());
        };

        assert!(err.contains("duplicate"));
        assert!(err.contains("hashbrown-pr692"));
        Ok(())
    }

    #[test]
    fn manual_fuzz_harness_validates_current_shape() -> Result<(), String> {
        check_manual_fuzz_harness()
    }

    #[test]
    fn calibration_manifest_requires_known_case_kinds() {
        assert!(CALIBRATION_REQUIRED_KINDS.contains(&"positive"));
        assert!(CALIBRATION_REQUIRED_KINDS.contains(&"negative"));
        assert!(CALIBRATION_REQUIRED_KINDS.contains(&"false_positive_control"));
        assert!(!CALIBRATION_REQUIRED_KINDS.contains(&"aspirational"));
    }

    #[test]
    fn fixture_package_prefix_can_preserve_identity_fixture_package() {
        assert_eq!(
            fixture_package_prefix("raw_pointer_alignment_line_drift"),
            "raw-pointer-alignment"
        );
        assert_eq!(
            fixture_package_prefix("duplicate_raw_pointer_reads"),
            "duplicate-raw-pointer-reads"
        );
    }

    #[test]
    fn git_diff_shape_requires_file_headers() {
        assert!(looks_like_git_diff(
            "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n"
        ));
        assert!(!looks_like_git_diff(
            "diff --git a/src/lib.rs b/src/lib.rs\n"
        ));
    }

    #[test]
    fn generated_artifact_detector_is_narrow() {
        assert!(is_forbidden_generated_path("target/debug/tool.exe"));
        assert!(is_forbidden_generated_path("badges/scratch.json"));
        assert!(is_forbidden_generated_path("reports/cards.sarif"));
        assert!(!is_forbidden_generated_path("badges/unsafe-review.json"));
        assert!(!is_forbidden_generated_path(
            "badges/unsafe-review-plus.json"
        ));
        assert!(!is_forbidden_generated_path("Cargo.lock"));
        assert!(!is_forbidden_generated_path("docs/status/SUPPORT_TIERS.md"));
    }

    #[test]
    fn public_badge_endpoints_match_readme_and_json() -> Result<(), String> {
        check_public_badge_endpoints()
    }

    #[test]
    fn public_badge_endpoint_url_uses_public_source_repo() {
        assert_eq!(
            public_badge_endpoint_url("badges/unsafe-review.json"),
            "https://img.shields.io/endpoint?url=https%3A%2F%2Fraw.githubusercontent.com%2FEffortlessMetrics%2Funsafe-review%2Fmain%2Fbadges%2Funsafe-review.json"
        );
    }

    #[test]
    fn advisory_artifact_checker_accepts_expected_artifact_set() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-ok")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        result
    }

    #[test]
    fn first_pr_artifact_checker_accepts_expected_bundle() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-ok")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        result
    }

    #[test]
    fn first_pr_artifact_checker_accepts_zero_card_bundle_with_no_card_wording()
    -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-zero-card-ok")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_zero_card_first_pr_artifacts(&dir)?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        result
    }

    #[test]
    fn first_pr_artifact_checker_rejects_missing_witness_plan() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-missing-witness")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains("witness-plan.md"));
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_unknown_card_id() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-bad-lsp")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        fs::write(
            dir.join("lsp.json"),
            r#"{"tool":"unsafe-review","mode":"read_only_projection","policy":"advisory","status":{"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"},"diagnostics":[{"card_id":"missing","witness_routes":[],"verify_commands":[],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}],"hovers":[],"code_actions":[],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
        )
        .map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains("unknown card id"));
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_positive_overclaims() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-overclaim")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        fs::write(
            dir.join("witness-plan.md"),
            "# unsafe-review witness plan\n\n- Review cards: 1\n- Open actionable gaps: 1\n- Policy mode: `advisory`\n\n## Route groups\n\n### Miri / cargo-careful\n\n- Limit: Concrete runtime evidence is path-specific.\n\n#### `card-1`\n\n- Route: `miri`\n  - Reason: route\n  - What it can show: a focused run\n  - What it cannot prove: arbitrary callers\n  - Receipt hint: unsafe-review receipt import-miri card-1\n\nAll clear.\n\n## Trust boundary\n\nThis artifact is static unsafe contract review. It routes reviewers to credible witnesses but does not run Miri, cargo-careful, sanitizers, Loom, Shuttle, Kani, or Crux. It is not a proof of memory safety, not UB-free status, and not a Miri result unless a witness receipt is attached.\n",
        )
        .map_err(|err| format!("write witness plan failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains("all clear"));
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_missing_trust_boundary() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-missing-boundary")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        fs::write(dir.join("pr-summary.md"), "- Review cards: 1\n")
            .map_err(|err| format!("write pr summary failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("static unsafe contract review")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_malformed_pr_summary() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-malformed-pr-summary")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        fs::write(
            dir.join("pr-summary.md"),
            "# unsafe-review PR summary\n\n- Scope: `diff`\n- Review cards: 1\n- Open actionable gaps: 1\n- Policy mode: `advisory`\n\n## Top card\n\n- ID: `card-1`\n\n## Card table\n\n| ID | Class | Location | Operation | Missing evidence | Route | Next action |\n|---|---|---|---|---|---|---|\n| `card-1` | `guard_missing` | src/lib.rs:1 | `ptr.read()` | alignment guard missing | `miri` | Add missing guard evidence |\n\n## Trust boundary\n\nThis artifact is static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result unless a witness receipt is attached.\n",
        )
        .map_err(|err| format!("write pr summary failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains("Witness plan"));
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_cards_json_without_trust_boundary() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-missing-cards-boundary")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        fs::write(
            dir.join("cards.json"),
            r#"{"tool":"unsafe-review","policy":"advisory","summary":{"cards":1},"cards":[{"id":"card-1"}]}"#,
        )
        .map_err(|err| format!("write cards failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("cards.json is missing trust_boundary")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_malformed_cards_json_card() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-malformed-card-json")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        write_cards_artifact(
            &dir,
            r#"{"id":"card-1","class":"guard_missing","priority":"high","confidence":"high","site":{"file":"src/lib.rs","line":1,"column":1,"kind":"unsafe_block","owner":"","visibility":"private","public_api_surface":false,"snippet":"ptr.read()"},"operation_family":"raw_pointer_read","hazards":["alignment"],"obligations":["pointer is aligned"],"obligation_evidence":[],"contract":"contract missing","discharge":"guard missing","reach":"owner reached","witness":"witness missing","missing":["alignment guard missing"],"verify_commands":[]}"#,
        )?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("obligation_evidence")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_malformed_sarif_result() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-malformed-sarif")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        write_sarif_artifact(
            &dir,
            r#"{"ruleId":"guard_missing","level":"warning","message":{"text":"guard_missing: add alignment evidence"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"src/lib.rs"},"region":{"startLine":0,"startColumn":1}}}],"properties":{"cardId":"card-1","class":"guard_missing","priority":"high","confidence":"high","operationFamily":"raw_pointer_read","operation":"ptr.read()","hazards":["alignment"],"missingEvidence":["alignment guard missing"],"witnessRoutes":["miri: pointer validity"],"nextAction":"Add missing guard evidence","trustBoundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}}"#,
        )?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("must be greater than zero")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_sarif_result_without_boundary() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-sarif-result-boundary")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        write_sarif_artifact(
            &dir,
            r#"{"ruleId":"guard_missing","level":"warning","message":{"text":"guard_missing: add alignment evidence"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"src/lib.rs"},"region":{"startLine":1,"startColumn":1}}}],"properties":{"cardId":"card-1","class":"guard_missing","priority":"high","confidence":"high","operationFamily":"raw_pointer_read","operation":"ptr.read()","hazards":["alignment"],"missingEvidence":["alignment guard missing"],"witnessRoutes":["miri: pointer validity"],"nextAction":"Add missing guard evidence"}}"#,
        )?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("/properties/trustBoundary")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_sarif_card_metadata_mismatch() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-sarif-mismatch")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        write_sarif_artifact(
            &dir,
            r#"{"ruleId":"guard_missing","level":"warning","message":{"text":"guard_missing: add alignment evidence"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"src/lib.rs"},"region":{"startLine":2,"startColumn":1}}}],"properties":{"cardId":"card-1","class":"guard_missing","priority":"high","confidence":"high","operationFamily":"raw_pointer_read","operation":"ptr.read()","hazards":["alignment"],"missingEvidence":["alignment guard missing"],"witnessRoutes":["miri: pointer validity"],"nextAction":"Add missing guard evidence","trustBoundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}}"#,
        )?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("cards.json has 1")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_duplicate_sarif_card_ids() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-sarif-duplicate")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_two_card_artifacts(&dir)?;
        write_sarif_artifact(
            &dir,
            &format!("{},{}", valid_sarif_result(), valid_sarif_result()),
        )?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("duplicate result")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_unknown_projection_card_ids() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-unknown-id")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        fs::write(
            dir.join("comment-plan.json"),
            r#"{"mode":"plan_only","policy":"advisory","comments":[{"card_id":"missing"}],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
        )
        .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains("unknown card id"));
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_comment_plan_over_candidate_cap() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-plan-cap")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        fs::write(
            dir.join("comment-plan.json"),
            r#"{"mode":"plan_only","policy":"advisory","comments":[{"card_id":"card-1"},{"card_id":"card-1"},{"card_id":"card-1"},{"card_id":"card-1"}],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
        )
        .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains("allow at most 3"));
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_comment_plan_without_renderable_location()
    -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-location")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        fs::write(
            dir.join("comment-plan.json"),
            r#"{"mode":"plan_only","policy":"advisory","comments":[{"card_id":"card-1","witness_routes":[{"kind":"miri","reason":"route","command":"cargo +nightly miri test card","required":false}],"verify_commands":["cargo +nightly miri test card"],"body":"Plan boundary: artifact-only inline comment candidate; unsafe-review did not post this comment, run witnesses, or make a policy decision."}],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
        )
        .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains("key `path`"));
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_duplicate_comment_plan_card_ids() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-plan-duplicate")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        fs::write(
            dir.join("comment-plan.json"),
            r#"{"mode":"plan_only","policy":"advisory","comments":[{"card_id":"card-1","path":"src/lib.rs","line":1,"class":"guard_missing","priority":"high","confidence":"high","operation":"ptr.read()","operation_family":"raw_pointer_read","witness_routes":[{"kind":"miri","reason":"route","command":"cargo +nightly miri test card","required":false}],"verify_commands":["cargo +nightly miri test card"],"selection_reason":"actionable high-confidence review card","body":"Plan boundary: artifact-only inline comment candidate; unsafe-review did not post this comment, run witnesses, or make a policy decision.\n\nTrust boundary: static unsafe contract review only; not memory-safety proof, not UB-free status, and not a Miri result unless a witness receipt is attached."},{"card_id":"card-1","path":"src/lib.rs","line":1,"class":"guard_missing","priority":"high","confidence":"high","operation":"ptr.read()","operation_family":"raw_pointer_read","witness_routes":[{"kind":"miri","reason":"route","command":"cargo +nightly miri test card","required":false}],"verify_commands":["cargo +nightly miri test card"],"selection_reason":"actionable high-confidence review card","body":"Plan boundary: artifact-only inline comment candidate; unsafe-review did not post this comment, run witnesses, or make a policy decision.\n\nTrust boundary: static unsafe contract review only; not memory-safety proof, not UB-free status, and not a Miri result unless a witness receipt is attached."}],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
        )
        .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("duplicate planned comment")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_malformed_comment_plan_comment() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-plan-shape")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        fs::write(
            dir.join("comment-plan.json"),
            r#"{"mode":"plan_only","policy":"advisory","comments":[{"card_id":"card-1","path":"src/lib.rs","line":0,"class":"guard_missing","priority":"high","confidence":"high","operation_family":"raw_pointer_read","selection_reason":"actionable high-confidence review card","body":"Trust boundary: static unsafe contract review only; not memory-safety proof, not UB-free status, and not a Miri result unless a witness receipt is attached."}],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
        )
        .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("greater than zero")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_comment_plan_card_metadata_mismatch() -> Result<(), String>
    {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-plan-mismatch")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        fs::write(
            dir.join("comment-plan.json"),
            r#"{"mode":"plan_only","policy":"advisory","comments":[{"card_id":"card-1","path":"src/lib.rs","line":1,"class":"contract_missing","priority":"high","confidence":"high","operation":"ptr.read()","operation_family":"raw_pointer_read","witness_routes":[{"kind":"miri","reason":"route","command":"cargo +nightly miri test card","required":false}],"verify_commands":["cargo +nightly miri test card"],"selection_reason":"actionable high-confidence review card","body":"Plan boundary: artifact-only inline comment candidate; unsafe-review did not post this comment, run witnesses, or make a policy decision.\n\nTrust boundary: static unsafe contract review only; not memory-safety proof, not UB-free status, and not a Miri result unless a witness receipt is attached."}],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
        )
        .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("cards.json has `guard_missing`")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_comment_plan_without_posting_boundary()
    -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-boundary")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        fs::write(
            dir.join("comment-plan.json"),
            r#"{"mode":"plan_only","policy":"advisory","comments":[{"card_id":"card-1","path":"src/lib.rs","line":1,"class":"guard_missing","priority":"high","confidence":"high","operation":"ptr.read()","operation_family":"raw_pointer_read","witness_routes":[{"kind":"miri","reason":"route","command":"cargo +nightly miri test card","required":false}],"verify_commands":["cargo +nightly miri test card"],"selection_reason":"actionable high-confidence review card","body":"Trust boundary: static unsafe contract review only; not memory-safety proof, not UB-free status, and not a Miri result unless a witness receipt is attached."}],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
        )
        .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("unsafe-review did not post this comment")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_comment_plan_body_without_boundary() -> Result<(), String>
    {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-plan-body")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        fs::write(
            dir.join("comment-plan.json"),
            r#"{"mode":"plan_only","policy":"advisory","comments":[{"card_id":"card-1","path":"src/lib.rs","line":1,"class":"guard_missing","priority":"high","confidence":"high","operation":"ptr.read()","operation_family":"raw_pointer_read","witness_routes":[{"kind":"miri","reason":"route","command":"cargo +nightly miri test card","required":false}],"verify_commands":["cargo +nightly miri test card"],"selection_reason":"actionable high-confidence review card","body":"Please inspect this unsafe operation."}],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
        )
        .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("static unsafe contract review")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_pr_summary_missing_card_id() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-pr-summary-missing-card")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_two_card_artifacts(&dir)?;
        fs::write(
            dir.join("pr-summary.md"),
            "# unsafe-review PR summary\n\n- Scope: `diff`\n- Review cards: 2\n- Open actionable gaps: 2\n- Policy mode: `advisory`\n\n## Top card\n\n- ID: `card-1`\n\n## Card table\n\n| ID | Class | Location | Operation | Missing evidence | Route | Next action |\n|---|---|---|---|---|---|---|\n| `card-1` | `guard_missing` | src/lib.rs:1 | `ptr.read()` | alignment guard missing | `miri` | Add missing guard evidence |\n\n## Witness plan\n\n- `card-1`: `miri` because pointer validity\n\n## Trust boundary\n\nThis artifact projects existing unsafe-review cards for PR review. It is static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result unless a witness receipt is attached.\n",
        )
        .map_err(|err| format!("write pr summary failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains("card-2"));
        Ok(())
    }

    #[test]
    fn policy_ledger_accepts_empty_status_without_entries() -> Result<(), String> {
        let path = unique_temp_dir("unsafe-review-empty-ledger")?.with_extension("toml");
        fs::write(
            &path,
            r#"schema_version = "0.1"
policy = "unsafe-review-baseline"
status = "empty"
"#,
        )
        .map_err(|err| format!("write ledger failed: {err}"))?;

        let result = check_unsafe_review_ledger(&path, LedgerKind::Baseline);

        fs::remove_file(&path).map_err(|err| format!("remove ledger failed: {err}"))?;
        result
    }

    #[test]
    fn policy_ledger_requires_exact_counted_identity_metadata() -> Result<(), String> {
        let path = unique_temp_dir("unsafe-review-baseline-ledger")?.with_extension("toml");
        fs::write(
            &path,
            r#"schema_version = "0.1"
policy = "unsafe-review-baseline"
status = "active"

[[entries]]
card_id = "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
owner = "core/policy"
reason = "accepted current fixture debt"
evidence = "review-card fixture"
review_after = "2026-08-01"
"#,
        )
        .map_err(|err| format!("write ledger failed: {err}"))?;

        let result = check_unsafe_review_ledger(&path, LedgerKind::Baseline);

        fs::remove_file(&path).map_err(|err| format!("remove ledger failed: {err}"))?;
        result
    }

    #[test]
    fn suppression_ledger_requires_review_or_expiry_date() -> Result<(), String> {
        let path = unique_temp_dir("unsafe-review-suppression-ledger")?.with_extension("toml");
        fs::write(
            &path,
            r#"schema_version = "0.1"
policy = "unsafe-review-suppressions"
status = "active"

[[entries]]
card_id = "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
owner = "core/policy"
reason = "false positive under review"
evidence = "manual review"
"#,
        )
        .map_err(|err| format!("write ledger failed: {err}"))?;

        let result = check_unsafe_review_ledger(&path, LedgerKind::Suppression);

        fs::remove_file(&path).map_err(|err| format!("remove ledger failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("review_after or expires")
        );
        Ok(())
    }

    #[test]
    fn policy_ledger_rejects_uncounted_card_identity() -> Result<(), String> {
        let path = unique_temp_dir("unsafe-review-bad-identity-ledger")?.with_extension("toml");
        fs::write(
            &path,
            r#"schema_version = "0.1"
policy = "unsafe-review-baseline"
status = "active"

[[entries]]
card_id = "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment"
owner = "core/policy"
reason = "accepted current fixture debt"
evidence = "review-card fixture"
review_after = "2026-08-01"
"#,
        )
        .map_err(|err| format!("write ledger failed: {err}"))?;

        let result = check_unsafe_review_ledger(&path, LedgerKind::Baseline);

        fs::remove_file(&path).map_err(|err| format!("remove ledger failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains("exact counted"));
        Ok(())
    }

    fn write_valid_artifacts(dir: &Path) -> Result<(), String> {
        write_cards_artifact(dir, valid_card_json())?;
        fs::write(
            dir.join("pr-summary.md"),
            "# unsafe-review PR summary\n\n- Scope: `diff`\n- Review cards: 1\n- Open actionable gaps: 1\n- Policy mode: `advisory`\n\n## Top card\n\n- ID: `card-1`\n- Class: `guard_missing`\n- Location: src/lib.rs:1\n- Operation: `ptr.read()`\n- Missing evidence: alignment guard missing\n- Primary route: `miri` because pointer validity\n- Next action: Add missing guard evidence\n\n## Card table\n\n| ID | Class | Location | Operation | Missing evidence | Route | Next action |\n|---|---|---|---|---|---|---|\n| `card-1` | `guard_missing` | src/lib.rs:1 | `ptr.read()` | alignment guard missing | `miri` | Add missing guard evidence |\n\n## Witness plan\n\n- `card-1`: `miri` because pointer validity\n\n## Trust boundary\n\nThis artifact projects existing unsafe-review cards for PR review. It is static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result unless a witness receipt is attached.\n",
        )
        .map_err(|err| format!("write pr summary failed: {err}"))?;
        write_sarif_artifact(dir, valid_sarif_result())?;
        fs::write(
            dir.join("comment-plan.json"),
            r#"{"mode":"plan_only","policy":"advisory","comments":[{"card_id":"card-1","path":"src/lib.rs","line":1,"class":"guard_missing","priority":"high","confidence":"high","operation":"ptr.read()","operation_family":"raw_pointer_read","witness_routes":[{"kind":"miri","reason":"route","command":"cargo +nightly miri test card","required":false}],"verify_commands":["cargo +nightly miri test card"],"selection_reason":"actionable high-confidence review card","body":"Plan boundary: artifact-only inline comment candidate; unsafe-review did not post this comment, run witnesses, or make a policy decision.\n\nTrust boundary: static unsafe contract review only; not memory-safety proof, not UB-free status, and not a Miri result unless a witness receipt is attached."}],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
        )
        .map_err(|err| format!("write comment plan failed: {err}"))?;
        Ok(())
    }

    fn write_two_card_artifacts(dir: &Path) -> Result<(), String> {
        write_cards_artifact_with_summary(
            dir,
            &format!("{},{}", valid_card_json(), card_two_json()),
            2,
        )?;
        fs::write(
            dir.join("pr-summary.md"),
            "# unsafe-review PR summary\n\n- Scope: `diff`\n- Review cards: 2\n- Open actionable gaps: 2\n- Policy mode: `advisory`\n\n## Top card\n\n- ID: `card-1`\n- Class: `guard_missing`\n- Location: src/lib.rs:1\n- Operation: `ptr.read()`\n- Missing evidence: alignment guard missing\n- Primary route: `miri` because pointer validity\n- Next action: Add missing guard evidence\n\n## Card table\n\n| ID | Class | Location | Operation | Missing evidence | Route | Next action |\n|---|---|---|---|---|---|---|\n| `card-1` | `guard_missing` | src/lib.rs:1 | `ptr.read()` | alignment guard missing | `miri` | Add missing guard evidence |\n| `card-2` | `guard_missing` | src/lib.rs:2 | `ptr.write(value)` | alignment guard missing | `miri` | Add missing guard evidence |\n\n## Witness plan\n\n- `card-1`: `miri` because pointer validity\n- `card-2`: `miri` because pointer validity\n\n## Trust boundary\n\nThis artifact projects existing unsafe-review cards for PR review. It is static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result unless a witness receipt is attached.\n",
        )
        .map_err(|err| format!("write pr summary failed: {err}"))?;
        write_sarif_artifact(
            dir,
            &format!("{},{}", valid_sarif_result(), sarif_result_two()),
        )?;
        fs::write(
            dir.join("comment-plan.json"),
            r#"{"mode":"plan_only","policy":"advisory","comments":[{"card_id":"card-1","path":"src/lib.rs","line":1,"class":"guard_missing","priority":"high","confidence":"high","operation":"ptr.read()","operation_family":"raw_pointer_read","witness_routes":[{"kind":"miri","reason":"route","command":"cargo +nightly miri test card","required":false}],"verify_commands":["cargo +nightly miri test card"],"selection_reason":"actionable high-confidence review card","body":"Plan boundary: artifact-only inline comment candidate; unsafe-review did not post this comment, run witnesses, or make a policy decision.\n\nTrust boundary: static unsafe contract review only; not memory-safety proof, not UB-free status, and not a Miri result unless a witness receipt is attached."}],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
        )
        .map_err(|err| format!("write comment plan failed: {err}"))?;
        Ok(())
    }

    fn write_cards_artifact(dir: &Path, card: &str) -> Result<(), String> {
        write_cards_artifact_with_summary(dir, card, 1)
    }

    fn write_cards_artifact_with_summary(
        dir: &Path,
        cards: &str,
        card_count: usize,
    ) -> Result<(), String> {
        fs::write(
            dir.join("cards.json"),
            format!(
                r#"{{"schema_version":"0.1","tool":"unsafe-review","scope":"diff","mode":"draft","policy":"advisory","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result","root":".","summary":{{"rust_files":1,"changed_rust_files":1,"unsafe_sites":{card_count},"cards":{card_count},"open_actionable_gaps":{card_count},"contract_missing":0,"guard_missing":{card_count},"guarded_unwitnessed":0,"unsafe_unreached":0,"requires_loom":0,"miri_unsupported":0,"static_unknown":0}},"cards":[{cards}]}}"#
            ),
        )
        .map_err(|err| format!("write cards failed: {err}"))
    }

    fn valid_card_json() -> &'static str {
        r#"{"id":"card-1","class":"guard_missing","priority":"high","confidence":"high","site":{"file":"src/lib.rs","line":1,"column":1,"kind":"unsafe_block","owner":"","visibility":"private","public_api_surface":false,"snippet":"ptr.read()"},"operation_family":"raw_pointer_read","hazards":["alignment"],"obligations":["pointer is aligned"],"obligation_evidence":[{"key":"alignment","description":"pointer is aligned","contract":{"present":false,"state":"missing","summary":"No contract evidence was found"},"discharge":{"present":false,"state":"missing","summary":"No alignment guard was found"},"reach":{"present":true,"state":"owner_reached","summary":"Related owner appears in changed code"},"witness":{"present":false,"state":"missing","summary":"No witness receipt was found"}}],"contract":"contract missing","discharge":"guard missing","reach":"owner reached","witness":"witness missing","missing":["alignment guard missing"],"verify_commands":[]}"#
    }

    fn card_two_json() -> &'static str {
        r#"{"id":"card-2","class":"guard_missing","priority":"high","confidence":"high","site":{"file":"src/lib.rs","line":2,"column":1,"kind":"unsafe_block","owner":"","visibility":"private","public_api_surface":false,"snippet":"ptr.write(value)"},"operation_family":"raw_pointer_write","hazards":["alignment"],"obligations":["pointer is aligned"],"obligation_evidence":[{"key":"alignment","description":"pointer is aligned","contract":{"present":false,"state":"missing","summary":"No contract evidence was found"},"discharge":{"present":false,"state":"missing","summary":"No alignment guard was found"},"reach":{"present":true,"state":"owner_reached","summary":"Related owner appears in changed code"},"witness":{"present":false,"state":"missing","summary":"No witness receipt was found"}}],"contract":"contract missing","discharge":"guard missing","reach":"owner reached","witness":"witness missing","missing":["alignment guard missing"],"verify_commands":[]}"#
    }

    fn write_sarif_artifact(dir: &Path, result: &str) -> Result<(), String> {
        fs::write(
            dir.join("cards.sarif"),
            format!(
                r#"{{"version":"2.1.0","runs":[{{"tool":{{"driver":{{"name":"unsafe-review","semanticVersion":"0.1.0","informationUri":"https://github.com/EffortlessMetrics/unsafe-review","rules":[{{"id":"guard_missing"}}]}}}},"results":[{result}],"properties":{{"schemaVersion":"0.1","scope":"diff","mode":"draft","policy":"advisory","trustBoundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}}}}]}}"#
            ),
        )
        .map_err(|err| format!("write sarif failed: {err}"))
    }

    fn valid_sarif_result() -> &'static str {
        r#"{"ruleId":"guard_missing","level":"warning","message":{"text":"guard_missing: add alignment evidence"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"src/lib.rs"},"region":{"startLine":1,"startColumn":1}}}],"properties":{"cardId":"card-1","class":"guard_missing","priority":"high","confidence":"high","operationFamily":"raw_pointer_read","operation":"ptr.read()","hazards":["alignment"],"missingEvidence":["alignment guard missing"],"witnessRoutes":["miri: pointer validity"],"nextAction":"Add missing guard evidence","trustBoundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}}"#
    }

    fn sarif_result_two() -> &'static str {
        r#"{"ruleId":"guard_missing","level":"warning","message":{"text":"guard_missing: add alignment evidence"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"src/lib.rs"},"region":{"startLine":2,"startColumn":1}}}],"properties":{"cardId":"card-2","class":"guard_missing","priority":"high","confidence":"high","operationFamily":"raw_pointer_write","operation":"ptr.write(value)","hazards":["alignment"],"missingEvidence":["alignment guard missing"],"witnessRoutes":["miri: pointer validity"],"nextAction":"Add missing guard evidence","trustBoundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}}"#
    }

    fn write_valid_first_pr_artifacts(dir: &Path) -> Result<(), String> {
        write_valid_artifacts(dir)?;
        fs::write(
            dir.join("witness-plan.md"),
            "# unsafe-review witness plan\n\n- Review cards: 1\n- Open actionable gaps: 1\n- Policy mode: `advisory`\n\n## Route groups\n\n### Miri / cargo-careful\n\n- Limit: Concrete runtime evidence is path-specific. It can support the exercised route, but it does not prove arbitrary callers, repo safety, UB-free status, or site execution unless a matching receipt records the run.\n\n#### `card-1`\n\n- Route: `miri`\n  - Reason: route\n  - What it can show: a focused run\n  - What it cannot prove: arbitrary callers\n  - Command:\n\n```bash\ncargo +nightly miri test card\n```\n  - Receipt hint: unsafe-review receipt import-miri card-1\n\n## Trust boundary\n\nThis artifact is static unsafe contract review. It routes reviewers to credible witnesses but does not run Miri, cargo-careful, sanitizers, Loom, Shuttle, Kani, or Crux. It is not a proof of memory safety, not UB-free status, and not a Miri result unless a witness receipt is attached.\n",
        )
        .map_err(|err| format!("write witness plan failed: {err}"))?;
        fs::write(
            dir.join("lsp.json"),
            r#"{"tool":"unsafe-review","mode":"read_only_projection","policy":"advisory","status":{"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"},"diagnostics":[{"card_id":"card-1","operation_family":"raw_pointer_read","hazards":["alignment"],"missing_evidence":["alignment guard missing"],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}],"hovers":[{"card_id":"card-1","contents":"Trust boundary: static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}],"code_actions":[{"card_id":"card-1","command":"unsafe-review.collectAgentPacket"}],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
        )
        .map_err(|err| format!("write lsp failed: {err}"))?;
        Ok(())
    }

    fn write_valid_zero_card_first_pr_artifacts(dir: &Path) -> Result<(), String> {
        write_cards_artifact_with_summary(dir, "", 0)?;
        fs::write(
            dir.join("pr-summary.md"),
            "# unsafe-review PR summary\n\n- Scope: `diff`\n- Review cards: 0\n- Open actionable gaps: 0\n- Policy mode: `advisory`\n\n## Top card\n\nNo changed unsafe-review gaps were found.\nThis does not prove the repo safe, UB-free, Miri-clean, or that any unsafe site executed.\n\n## Card table\n\n| ID | Class | Location | Operation | Missing evidence | Route | Next action |\n|---|---|---|---|---|---|---|\n\n## Witness plan\n\nNo witness route is recommended because no review cards were emitted.\n\n## Trust boundary\n\nThis artifact projects existing unsafe-review cards for PR review. It is static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result unless a witness receipt is attached.\n",
        )
        .map_err(|err| format!("write pr summary failed: {err}"))?;
        write_sarif_artifact(dir, "")?;
        fs::write(
            dir.join("comment-plan.json"),
            r#"{"mode":"plan_only","policy":"advisory","comments":[],"no_changed_gaps":{"message":"No changed unsafe-review gaps were found.","limitation":"This does not prove the repo safe, UB-free, Miri-clean, or that any unsafe site executed."},"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
        )
        .map_err(|err| format!("write comment plan failed: {err}"))?;
        fs::write(
            dir.join("witness-plan.md"),
            "# unsafe-review witness plan\n\n- Review cards: 0\n- Open actionable gaps: 0\n- Policy mode: `advisory`\n\nNo changed unsafe-review gaps were found.\nThis does not prove the repo safe, UB-free, Miri-clean, or that any unsafe site executed.\n\nNo witness routes are recommended because no review cards were emitted.\n\n## Trust boundary\n\nThis artifact is static unsafe contract review. It routes reviewers to credible witnesses but does not run Miri, cargo-careful, sanitizers, Loom, Shuttle, Kani, or Crux. It is not a proof of memory safety, not UB-free status, and not a Miri result unless a witness receipt is attached.\n",
        )
        .map_err(|err| format!("write witness plan failed: {err}"))?;
        fs::write(
            dir.join("lsp.json"),
            r#"{"tool":"unsafe-review","mode":"read_only_projection","policy":"advisory","status":{"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"},"diagnostics":[],"hovers":[],"code_actions":[],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
        )
        .map_err(|err| format!("write lsp failed: {err}"))?;
        Ok(())
    }

    fn unique_temp_dir(prefix: &str) -> Result<PathBuf, String> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system clock before UNIX_EPOCH: {err}"))?
            .as_nanos();
        Ok(std::env::temp_dir().join(format!("{prefix}-{nanos}")))
    }
}
