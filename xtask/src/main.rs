#![forbid(unsafe_code)]
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use calibration_constants::{
    CALIBRATION_CASE_FIELDS, CALIBRATION_REQUIRED_KINDS, HAZARD_KIND_SOURCE,
    OPERATION_FAMILY_REGISTRY, OPERATION_FAMILY_REGISTRY_COLUMNS, OPERATION_FAMILY_REGISTRY_HEADER,
    OPERATION_FAMILY_REGISTRY_OBLIGATION_KEYS_COLUMN,
    OPERATION_FAMILY_REGISTRY_REQUIRED_TEXT_COLUMNS, OPERATION_FAMILY_SOURCE,
    SAFETY_OBLIGATION_SOURCE, WITNESS_KIND_SOURCE, ZERO_CARD_EXPECTATION_FIELDS,
};

mod accuracy_labels;
mod advisory_artifacts;
mod calibration_constants;
mod calibration_manifest;
mod command_args;
mod commands;
mod docs_automation_paths;
mod first_hour;
mod markdown;
mod public_badges;
mod public_surfaces;
mod source_sync;
mod spec_status;
mod support_tiers;
mod workflow_allowlist;

use advisory_artifacts::{check_advisory_artifacts, check_first_pr_artifacts};
use first_hour::check_first_hour;
use support_tiers::{SUPPORT_TIERS_DOC, check_support_tiers, support_tier_capabilities};

#[cfg(test)]
use command_args::{require_max_args, require_no_extra_args};
#[cfg(test)]
use support_tiers::{
    SUPPORT_SUMMARY_DOC, SUPPORT_SUMMARY_REQUIRED_PHRASES, check_support_summary_text,
    check_support_tiers_text, support_capability_from_row, support_summary_posture_from_row,
    support_tier_from_row,
};

#[cfg(test)]
use workflow_allowlist::{
    WorkflowPolicyEntry, check_workflow_text_against_policy, workflow_used_actions,
};

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
    "policy/doc-artifacts.toml",
    "policy/ci-lane-whitelist.toml",
    "policy/package-boundary.toml",
    "policy/source-sync.toml",
    "policy/docs-automation.toml",
    "policy/accuracy-calibration.toml",
    "policy/public-surfaces.toml",
];
const WORKFLOW_ALLOWLIST: &str = "policy/workflow-allowlist.toml";
const WORKFLOW_DIR: &str = ".github/workflows";
const DOC_ARTIFACT_LEDGER: &str = "policy/doc-artifacts.toml";
const DOCS_AUTOMATION_LEDGER: &str = "policy/docs-automation.toml";
const CI_LANE_LEDGER: &str = "policy/ci-lane-whitelist.toml";
const PACKAGE_BOUNDARY_LEDGER: &str = "policy/package-boundary.toml";
const SOURCE_OF_TRUTH_INDEX: &str = ".unsafe-review-spec/index.toml";
const ACTIVE_GOAL_MANIFEST: &str = ".unsafe-review-spec/goals/active.toml";
const DOC_ARTIFACT_KINDS: &[&str] = &["proposal", "spec", "adr", "plan", "goal"];
const DOC_ARTIFACT_STATUSES: &[&str] = &["proposed", "accepted", "active", "done", "deferred"];
const DOCS_AUTOMATION_KINDS: &[&str] = &[
    "spec_status_dashboard",
    "operator_front_door",
    "agent_operating_contract",
    "lane_plan",
    "docs_map",
    "published_surface",
    "handoff_receipt",
];
const DOCS_AUTOMATION_MODES: &[&str] = &["check", "generate"];
const GOAL_WORK_ITEM_STATUSES: &[&str] = &["ready", "active", "blocked", "done", "superseded"];
const PACKAGE_CLASSIFICATIONS: &[&str] = &["published", "private", "internal", "deferred"];
const CI_LANE_STATUSES: &[&str] = &["advisory", "required", "deferred", "retired"];

const FIXTURE_REQUIRED_FILES: &[&str] = &["Cargo.toml", "change.diff", "src/lib.rs"];

const FIXTURE_EXPECTED_CARDS_EXCEPTIONS: &[&str] = &[
    "duplicate_raw_pointer_reads",
    "raw_pointer_alignment_line_drift",
];

const FIXTURE_PACKAGE_PREFIX_EXCEPTIONS: &[(&str, &str)] =
    &[("raw_pointer_alignment_line_drift", "raw-pointer-alignment")];
const MANUAL_CANDIDATE_EXAMPLE_DIR: &str = "docs/examples/manual-candidates";
const MANUAL_CANDIDATE_SMOKE_FIXTURE_DIR: &str =
    "target/unsafe-review-manual-candidate-smoke-fixture";
const MANUAL_CANDIDATE_SMOKE_OUT_DIR: &str = "target/unsafe-review-manual-candidate-smoke";

const DOGFOOD_MANIFEST: &str = "docs/dogfood/corpus.toml";
const DOGFOOD_INDEX: &str = "docs/dogfood/index.json";
const DOGFOOD_README: &str = "docs/dogfood/README.md";
const DOGFOOD_FOLLOW_UP_SEEDS: &str = "docs/dogfood/follow-up-seeds.md";
const DOGFOOD_JUDGMENT_DIR: &str = "docs/dogfood/judgments";
const DOGFOOD_JUDGMENTS_README: &str = "docs/dogfood/judgments/README.md";
const DOGFOOD_REPORT_DIR: &str = "docs/dogfood/reports";
const ACCURACY_CALIBRATION_POLICY: &str = "policy/accuracy-calibration.toml";
const ACCURACY_CALIBRATION_REPORT: &str = "docs/accuracy/CALIBRATION_REPORT.md";
const OBJECTIVE_AUDIT: &str = "docs/status/OBJECTIVE_AUDIT.md";
const ACCURACY_CLAIM_STATUSES: &[&str] = &[
    "fixture_pinned",
    "dogfood_measured",
    "labeled_calibrated",
    "policy_eligible",
];
const ACCURACY_CLAIM_KINDS: &[&str] = &[
    "inventory",
    "operation_family",
    "hazard",
    "obligation",
    "evidence_precision",
    "false_positive_control",
    "false_negative_probe",
    "route_quality",
    "identity_stability",
    "artifact_honesty",
];
const ACCURACY_PROMOTION_FORBIDDEN_TERMS: &[&str] = &[
    "global precision",
    "global recall",
    "policy ready",
    "policy-ready",
    "policy readiness",
    "memory-safety proof",
    "ub-free",
    "miri-clean",
    "witness execution proof",
];
const ACCURACY_REQUIRED_FORBIDDEN_CLAIMS: &[&str] =
    &["global precision", "global recall", "memory-safety proof"];
const DOGFOOD_TARGET_KINDS: &[&str] = &["repo-snapshot", "pr-diff", "fixture-control"];
const DOGFOOD_TARGET_STATUSES: &[&str] = &["active", "parked", "retired"];
const DOGFOOD_ARTIFACT_STATUSES: &[&str] = &["checked_in", "local_untracked", "remote_manual"];
const DOGFOOD_TRIAGE_LABELS: &[&str] = &[
    "actionable",
    "noise",
    "missed",
    "needs-fixture",
    "needs-doc",
    "needs-route",
    "needs-analyzer",
    "needs-verifier",
];
const DOGFOOD_TRIAGE_HEADER: &[&str] = &[
    "Target",
    "Card or family",
    "Primary label",
    "Evidence",
    "Follow-up",
];
const DOGFOOD_FOLLOW_UP_STATUSES: &[&str] = &["open", "done", "parked", "superseded"];
const DOGFOOD_FOLLOW_UP_SURFACES: &[&str] = &[
    "comment_plan",
    "first_pr_projection",
    "manual_candidate_projection",
    "repo_posture",
];
const DOGFOOD_JUDGMENT_SURFACES: &[&str] = &[
    "comment_plan",
    "context_packet",
    "first_pr_projection",
    "github_summary",
    "manual_candidate_projection",
    "pr_summary",
    "receipt_audit",
    "repair_queue",
    "repo_posture",
    "witness_plan",
];
const DOGFOOD_JUDGMENT_LABELS: &[&str] = &[
    "actionable",
    "noise",
    "missed",
    "uncertain",
    "human-only",
    "good-agent-task",
    "bad-agent-task",
];
const DOGFOOD_MISSED_JUDGMENT_STATUSES: &[&str] = &["open", "converted", "deferred", "superseded"];
const DOGFOOD_FOLLOW_UP_HEADER: &[&str] = &[
    "Seed ID",
    "Status",
    "Target",
    "Family/surface",
    "Primary label",
    "Source report",
    "Next PR slice",
    "Notes",
];
const FUZZ_REQUIRED_FILES: &[&str] = &[
    "docs/FUZZING.md",
    "fuzz/.gitignore",
    "fuzz/Cargo.lock",
    "fuzz/Cargo.toml",
    "fuzz/corpus/analyze/basic",
    "fuzz/fuzz_targets/analyze.rs",
];
#[derive(Clone, Debug, PartialEq, Eq)]
struct DocArtifactEntry {
    kind: String,
    path: String,
    status: String,
    owner: String,
}

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

    match commands::XtaskCommand::parse(&args)? {
        commands::XtaskCommand::Help => {
            println!(
                "xtask commands: check-pr, check-docs, check-policy, check-support-tiers, check-fixtures, check-calibration, check-dogfood, check-fuzz, check-doc-artifacts, check-docs-automation, check-spec-status, check-public-surfaces, check-goals, check-package-boundary, check-ci-lanes, check-advisory-artifacts <dir>, check-first-pr-artifacts <dir>, check-manual-candidate-examples, check-first-hour, source-divergence, check-source-sync"
            );
            Ok(())
        }
        commands::XtaskCommand::CheckPr => {
            check_docs()?;
            public_badges::check_generated_projection()?;
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
        commands::XtaskCommand::CheckDocs => check_docs(),
        commands::XtaskCommand::CheckPolicy => check_policy(),
        commands::XtaskCommand::CheckDocArtifacts => check_doc_artifacts(),
        commands::XtaskCommand::CheckDocsAutomation => check_docs_automation(),
        commands::XtaskCommand::CheckSpecStatus => spec_status::check(),
        commands::XtaskCommand::CheckPublicSurfaces => public_surfaces::check(),
        commands::XtaskCommand::CheckGoals => check_goals(),
        commands::XtaskCommand::CheckPackageBoundary => check_package_boundary(),
        commands::XtaskCommand::CheckCiLanes => check_ci_lanes(),
        commands::XtaskCommand::CheckSupportTiers => check_support_tiers(),
        commands::XtaskCommand::CheckFixtures => check_fixtures(),
        commands::XtaskCommand::CheckCalibration => check_calibration(),
        commands::XtaskCommand::CheckDogfood => check_dogfood(),
        commands::XtaskCommand::CheckFuzz => check_manual_fuzz_harness(),
        commands::XtaskCommand::CheckAdvisoryArtifacts(dir) => check_advisory_artifacts(&dir),
        commands::XtaskCommand::CheckFirstPrArtifacts(dir) => check_first_pr_artifacts(&dir),
        commands::XtaskCommand::CheckManualCandidateExamples => check_manual_candidate_examples(),
        commands::XtaskCommand::CheckFirstHour => check_first_hour(),
        commands::XtaskCommand::SourceDivergence => source_sync::report_source_divergence(),
    }
}

fn workspace_root() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "failed to resolve workspace root from xtask manifest path".to_string())
}

fn check_docs() -> Result<(), String> {
    for path in REQUIRED_DOCS {
        require_file(path)?;
    }
    for path in FRONT_DOOR_MARKDOWN_DOCS {
        check_markdown_local_links(path)?;
    }
    public_badges::check_endpoints()?;
    spec_status::check_dashboard_impl()?;
    check_docs_map_paths("docs/README.md")?;
    public_surfaces::check_first_pr_artifact_list_surfaces()?;
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
        Path::new("docs"),
        Path::new("plans"),
        Path::new(".unsafe-review-spec"),
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
    workflow_allowlist::check_workflow_allowlist(
        Path::new(WORKFLOW_ALLOWLIST),
        Path::new(WORKFLOW_DIR),
    )?;
    check_unsafe_review_ledger(
        Path::new("policy/unsafe-review-baseline.toml"),
        LedgerKind::Baseline,
    )?;
    check_unsafe_review_ledger(
        Path::new("policy/unsafe-review-suppressions.toml"),
        LedgerKind::Suppression,
    )?;
    check_doc_artifacts()?;
    check_docs_automation()?;
    public_surfaces::check()?;
    check_goals()?;
    check_package_boundary()?;
    check_ci_lanes()?;
    check_ci_routing_contract()?;
    println!("check-policy: ok");
    Ok(())
}

fn check_ci_routing_contract() -> Result<(), String> {
    let path = ".github/workflows/ci.yml";
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("failed to read {path}: {err}"))?;
    if text.contains("repos/${") && text.contains("/actions/runners") {
        return Err(format!(
            "{path} must use organization runner discovery, not repository runner discovery"
        ));
    }
    for needle in [
        "gh api \"orgs/${ORG}/actions/runners?per_page=100\"",
        "EM_RUNNER_READ_TOKEN",
        "router_target=",
        "router_reason=",
        "Rust Small on CPX42",
        "labels: [self-hosted, linux, x64, em-ci, cpx42, rust-16gb, rust-medium, trusted-pr]",
        "Prepare CPX42 scratch",
        "dtolnay/rust-toolchain@v1",
        "toolchain: 1.95.0",
        "Rust Small on CX43",
        "Rust Small on CX53",
        "dtolnay/rust-toolchain@1.95.0",
        "Rust Tiny Fallback on GitHub Hosted",
        "Rust Small Blocked (capacity/config)",
        "fallback_allowed=",
        "fallback_mode=",
        "allow-github-hosted",
        "ci-budget-ack",
        "full-ci",
        "Unsafe Review Rust Small Result",
    ] {
        if !text.contains(needle) {
            return Err(format!(
                "{path} missing required routed CI contract marker: {needle}"
            ));
        }
    }
    for forbidden in ["em-ci-rust:1.95", "docker run --rm"] {
        if text.contains(forbidden) {
            return Err(format!(
                "{path} must not depend on broken Docker Rust Small marker: {forbidden}"
            ));
        }
    }
    Ok(())
}

struct ManualCandidateExample {
    path: PathBuf,
    id: String,
    expected: serde_json::Value,
}

fn check_manual_candidate_examples() -> Result<(), String> {
    let examples = manual_candidate_examples()?;
    let fixture_dir = Path::new(MANUAL_CANDIDATE_SMOKE_FIXTURE_DIR);
    let out_dir = Path::new(MANUAL_CANDIDATE_SMOKE_OUT_DIR);

    reset_target_dir(fixture_dir)?;
    reset_target_dir(out_dir)?;
    copy_dir_all(Path::new("fixtures/raw_pointer_alignment"), fixture_dir)?;

    let candidate_dir = fixture_dir.join(".unsafe-review").join("candidates");
    fs::create_dir_all(&candidate_dir)
        .map_err(|err| format!("create {} failed: {err}", candidate_dir.display()))?;
    for example in &examples {
        let out = candidate_dir.join(format!("{}.json", example.id));
        run_unsafe_review([
            os("candidate"),
            os("import"),
            example.path.as_os_str().to_os_string(),
            os("--out"),
            out.as_os_str().to_os_string(),
        ])?;
    }

    run_unsafe_review([
        os("first-pr"),
        os("--root"),
        fixture_dir.as_os_str().to_os_string(),
        os("--diff"),
        fixture_dir.join("change.diff").as_os_str().to_os_string(),
        os("--out-dir"),
        out_dir.as_os_str().to_os_string(),
    ])?;

    check_first_pr_artifacts(out_dir)?;
    check_manual_candidate_smoke_matches_examples(out_dir, &examples)?;
    println!(
        "check-manual-candidate-examples: ok ({} candidates -> {})",
        examples.len(),
        out_dir.display()
    );
    Ok(())
}

fn manual_candidate_examples() -> Result<Vec<ManualCandidateExample>, String> {
    let dir = Path::new(MANUAL_CANDIDATE_EXAMPLE_DIR);
    let mut examples = Vec::new();
    let entries =
        fs::read_dir(dir).map_err(|err| format!("read {} failed: {err}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|err| format!("read_dir entry failed: {err}"))?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let path_display = path.display().to_string();
        let value = parse_json_file(&path)?;
        require_json_str(
            &value,
            "schema_version",
            "manual-candidate/v1",
            &path_display,
        )?;
        require_json_str(&value, "source", "manual", &path_display)?;
        if value.get("manual_candidate") != Some(&serde_json::Value::Bool(true)) {
            return Err(format!("{path_display} manual_candidate must be true"));
        }
        if value.get("analyzer_discovered") != Some(&serde_json::Value::Bool(false)) {
            return Err(format!("{path_display} analyzer_discovered must be false"));
        }
        let id = require_non_empty_json_str(&value, "id", &path_display)?.to_string();
        if !is_path_safe_manual_candidate_id(&id) {
            return Err(format!(
                "{path_display} id `{id}` is not safe for a candidate artifact filename"
            ));
        }
        examples.push(ManualCandidateExample {
            path,
            id,
            expected: value,
        });
    }
    if examples.is_empty() {
        return Err(format!(
            "{MANUAL_CANDIDATE_EXAMPLE_DIR} has no JSON examples"
        ));
    }
    examples.sort_by(|left, right| left.id.cmp(&right.id).then(left.path.cmp(&right.path)));
    let mut ids = BTreeSet::new();
    for example in &examples {
        if !ids.insert(example.id.clone()) {
            return Err(format!(
                "{MANUAL_CANDIDATE_EXAMPLE_DIR} contains duplicate manual candidate id `{}`",
                example.id
            ));
        }
    }
    Ok(examples)
}

fn check_manual_candidate_smoke_matches_examples(
    out_dir: &Path,
    examples: &[ManualCandidateExample],
) -> Result<(), String> {
    let path = out_dir.join("manual-candidates.json");
    let value = parse_json_file(&path)?;
    let path_display = path.display().to_string();
    let actual_count = json_usize_at(&value, "/summary/manual_candidates", &path_display)?;
    if actual_count != examples.len() {
        return Err(format!(
            "{} summary.manual_candidates is {actual_count}, expected {} committed examples",
            path.display(),
            examples.len()
        ));
    }
    let candidates = json_array_at(&value, "/candidates", &path_display)?;
    let mut actual_ids = Vec::new();
    let mut actual_by_id = BTreeMap::new();
    for candidate in candidates {
        let id = require_non_empty_json_str(candidate, "id", &path_display)?.to_string();
        if actual_by_id.insert(id.clone(), candidate).is_some() {
            return Err(format!("{} repeats candidate ID `{id}`", path.display()));
        }
        actual_ids.push(id);
    }
    let expected_ids = examples
        .iter()
        .map(|example| example.id.clone())
        .collect::<Vec<_>>();
    if actual_ids != expected_ids {
        return Err(format!(
            "{} candidate IDs {:?} do not match sorted committed example IDs {:?}",
            path.display(),
            actual_ids,
            expected_ids
        ));
    }
    for example in examples {
        let actual = actual_by_id.get(&example.id).ok_or_else(|| {
            format!(
                "{} is missing generated candidate ID `{}`",
                path.display(),
                example.id
            )
        })?;
        check_manual_candidate_smoke_entry_matches_example(actual, example)?;
    }
    Ok(())
}

fn check_manual_candidate_smoke_entry_matches_example(
    actual: &serde_json::Value,
    example: &ManualCandidateExample,
) -> Result<(), String> {
    let example_path = example.path.display().to_string();
    let context = format!("manual-candidates.json candidate `{}`", example.id);
    for field in [
        "schema_version",
        "id",
        "source",
        "manual_candidate",
        "analyzer_discovered",
        "title",
        "location",
        "operation_family",
        "unsafe_operation",
        "invariant",
        "safe_caller",
        "evidence",
        "trust_boundary",
    ] {
        require_generated_example_field_match(
            actual,
            &example.expected,
            field,
            &context,
            &example_path,
        )?;
    }
    for field in ["fix_options", "test_targets", "do_not_touch"] {
        require_generated_example_optional_array_match(
            actual,
            &example.expected,
            field,
            &context,
            &example_path,
        )?;
    }
    Ok(())
}

fn require_generated_example_field_match(
    actual: &serde_json::Value,
    expected: &serde_json::Value,
    field: &str,
    context: &str,
    example_path: &str,
) -> Result<(), String> {
    if actual.get(field) == expected.get(field) {
        return Ok(());
    }
    Err(format!(
        "{context} field `{field}` must match committed example {example_path}; expected {}, got {}",
        json_field_display(expected.get(field)),
        json_field_display(actual.get(field))
    ))
}

fn require_generated_example_optional_array_match(
    actual: &serde_json::Value,
    expected: &serde_json::Value,
    field: &str,
    context: &str,
    example_path: &str,
) -> Result<(), String> {
    let actual = optional_json_array(actual, field);
    let expected = optional_json_array(expected, field);
    if actual == expected {
        return Ok(());
    }
    Err(format!(
        "{context} field `{field}` must match committed example {example_path}; expected {}, got {}",
        json_field_display(expected),
        json_field_display(actual)
    ))
}

fn optional_json_array<'a>(
    value: &'a serde_json::Value,
    field: &str,
) -> Option<&'a serde_json::Value> {
    match value.get(field) {
        Some(serde_json::Value::Array(items)) if items.is_empty() => None,
        other => other,
    }
}

fn json_field_display(value: Option<&serde_json::Value>) -> String {
    value
        .map(serde_json::Value::to_string)
        .unwrap_or_else(|| "<missing>".to_string())
}

fn reset_target_dir(path: &Path) -> Result<(), String> {
    require_target_subpath(path)?;
    if path.exists() {
        fs::remove_dir_all(path)
            .map_err(|err| format!("remove {} failed: {err}", path.display()))?;
    }
    Ok(())
}

fn require_target_subpath(path: &Path) -> Result<(), String> {
    if path.is_absolute()
        || !matches!(
            path.components().next(),
            Some(std::path::Component::Normal(component)) if component == "target"
        )
        || path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(format!(
            "{} must be a relative generated path under target/",
            path.display()
        ));
    }
    Ok(())
}

fn copy_dir_all(source: &Path, target: &Path) -> Result<(), String> {
    fs::create_dir_all(target)
        .map_err(|err| format!("create {} failed: {err}", target.display()))?;
    for entry in
        fs::read_dir(source).map_err(|err| format!("read {} failed: {err}", source.display()))?
    {
        let entry = entry.map_err(|err| format!("read_dir entry failed: {err}"))?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir_all(&source_path, &target_path)?;
        } else {
            fs::copy(&source_path, &target_path).map_err(|err| {
                format!(
                    "copy {} to {} failed: {err}",
                    source_path.display(),
                    target_path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn run_unsafe_review(args: impl IntoIterator<Item = OsString>) -> Result<(), String> {
    let args = args.into_iter().collect::<Vec<_>>();
    let display_args = args
        .iter()
        .map(|arg| arg.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");
    let output = Command::new("cargo")
        .args(["run", "--locked", "-p", "unsafe-review", "--"])
        .args(&args)
        .output()
        .map_err(|err| format!("failed to run unsafe-review {display_args}: {err}"))?;
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "unsafe-review {display_args} failed with status {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout).trim(),
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

fn os(value: &str) -> OsString {
    OsString::from(value)
}

fn is_path_safe_manual_candidate_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.')
        && !id.contains("..")
}

fn check_doc_artifacts() -> Result<(), String> {
    let ids = check_doc_artifacts_impl()?;
    println!("check-doc-artifacts: ok ({} artifacts)", ids.len());
    Ok(())
}

fn check_doc_artifacts_impl() -> Result<BTreeSet<String>, String> {
    let value = parse_toml_file(Path::new(DOC_ARTIFACT_LEDGER))?;
    let source_index = parse_toml_file(Path::new(SOURCE_OF_TRUTH_INDEX))?;
    let source_artifacts = source_truth_index_artifacts(&source_index)?;
    require_toml_string(&value, "schema_version", DOC_ARTIFACT_LEDGER)?;
    let artifacts = toml_array(&value, "artifact", DOC_ARTIFACT_LEDGER)?;
    if artifacts.is_empty() {
        return Err(format!(
            "{DOC_ARTIFACT_LEDGER} must list at least one artifact"
        ));
    }

    let mut ids = BTreeSet::new();
    let mut ledger_artifacts = BTreeMap::new();
    let mut linked_ids = Vec::new();
    for (idx, artifact) in artifacts.iter().enumerate() {
        let table = toml_table(artifact, DOC_ARTIFACT_LEDGER, "artifact", idx)?;
        let id = required_table_string(table, "id", DOC_ARTIFACT_LEDGER, "artifact", idx)?;
        let kind = required_table_string(table, "kind", DOC_ARTIFACT_LEDGER, "artifact", idx)?;
        let path = required_table_string(table, "path", DOC_ARTIFACT_LEDGER, "artifact", idx)?;
        let status = required_table_string(table, "status", DOC_ARTIFACT_LEDGER, "artifact", idx)?;
        let owner = required_table_string(table, "owner", DOC_ARTIFACT_LEDGER, "artifact", idx)?;

        require_known(kind, DOC_ARTIFACT_KINDS, DOC_ARTIFACT_LEDGER, "kind")?;
        require_known(status, DOC_ARTIFACT_STATUSES, DOC_ARTIFACT_LEDGER, "status")?;
        if !ids.insert(id.to_string()) {
            return Err(format!(
                "{DOC_ARTIFACT_LEDGER} contains duplicate id `{id}`"
            ));
        }
        ledger_artifacts.insert(
            id.to_string(),
            DocArtifactEntry {
                kind: kind.to_string(),
                path: path.to_string(),
                status: status.to_string(),
                owner: owner.to_string(),
            },
        );
        require_file(path)?;
        if let Some(linked_proposal) = table.get("linked_proposal").and_then(toml::Value::as_str) {
            linked_ids.push((
                id.to_string(),
                "linked_proposal",
                linked_proposal.to_string(),
            ));
        }
        if let Some(linked_spec) = table.get("linked_spec").and_then(toml::Value::as_str) {
            linked_ids.push((id.to_string(), "linked_spec", linked_spec.to_string()));
        }
        if let Some(policy_impact) = table.get("policy_impact") {
            for path in toml_str_array(policy_impact, DOC_ARTIFACT_LEDGER, "policy_impact")? {
                require_file(path)?;
            }
        }
    }

    for (id, field, linked_id) in linked_ids {
        if !ids.contains(&linked_id) {
            return Err(format!(
                "{DOC_ARTIFACT_LEDGER} artifact `{id}` has {field} `{linked_id}` not listed as an artifact"
            ));
        }
    }

    check_doc_artifacts_source_index_consistency(&ledger_artifacts, &source_artifacts)?;

    Ok(ids)
}

fn check_doc_artifacts_source_index_consistency(
    ledger_artifacts: &BTreeMap<String, DocArtifactEntry>,
    source_artifacts: &BTreeMap<String, DocArtifactEntry>,
) -> Result<(), String> {
    for (id, ledger) in ledger_artifacts {
        let Some(indexed) = source_artifacts.get(id) else {
            continue;
        };
        for (field, ledger_value, index_value) in [
            ("kind", &ledger.kind, &indexed.kind),
            ("path", &ledger.path, &indexed.path),
            ("status", &ledger.status, &indexed.status),
            ("owner", &ledger.owner, &indexed.owner),
        ] {
            if ledger_value != index_value {
                return Err(format!(
                    "{SOURCE_OF_TRUTH_INDEX} artifact `{id}` {field} `{index_value}` must match {DOC_ARTIFACT_LEDGER} `{ledger_value}`"
                ));
            }
        }
    }
    Ok(())
}

fn check_docs_automation() -> Result<(), String> {
    let surfaces = check_docs_automation_impl()?;
    println!("check-docs-automation: ok ({surfaces} surfaces)");
    Ok(())
}

fn check_docs_automation_impl() -> Result<usize, String> {
    let value = parse_toml_file(Path::new(DOCS_AUTOMATION_LEDGER))?;
    require_toml_string(&value, "schema_version", DOCS_AUTOMATION_LEDGER)?;

    let scope = value
        .get("scope")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{DOCS_AUTOMATION_LEDGER} is missing table `scope`"))?;
    let owned_roots = require_scope_paths(scope, "owned_roots", true)?;
    let external_awareness_roots = require_scope_paths(scope, "external_awareness_only", false)?;
    check_docs_automation_scope_boundaries(&owned_roots, &external_awareness_roots)?;

    let surfaces = toml_array(&value, "generated_or_checked", DOCS_AUTOMATION_LEDGER)?;
    if surfaces.is_empty() {
        return Err(format!(
            "{DOCS_AUTOMATION_LEDGER} must list at least one generated_or_checked entry"
        ));
    }

    let mut ids = BTreeSet::new();
    for (idx, surface) in surfaces.iter().enumerate() {
        let table = toml_table(surface, DOCS_AUTOMATION_LEDGER, "generated_or_checked", idx)?;
        let id = required_table_string(
            table,
            "id",
            DOCS_AUTOMATION_LEDGER,
            "generated_or_checked",
            idx,
        )?;
        if !ids.insert(id.to_string()) {
            return Err(format!(
                "{DOCS_AUTOMATION_LEDGER} contains duplicate generated_or_checked id `{id}`"
            ));
        }

        let kind = required_table_string(
            table,
            "kind",
            DOCS_AUTOMATION_LEDGER,
            "generated_or_checked",
            idx,
        )?;
        let mode = required_table_string(
            table,
            "mode",
            DOCS_AUTOMATION_LEDGER,
            "generated_or_checked",
            idx,
        )?;
        require_known(
            kind,
            DOCS_AUTOMATION_KINDS,
            DOCS_AUTOMATION_LEDGER,
            "generated_or_checked.kind",
        )?;
        require_known(
            mode,
            DOCS_AUTOMATION_MODES,
            DOCS_AUTOMATION_LEDGER,
            "generated_or_checked.mode",
        )?;

        if let Some(sources) = table.get("sources") {
            for source in toml_str_array(sources, DOCS_AUTOMATION_LEDGER, "sources")? {
                require_existing_repo_path(source, DOCS_AUTOMATION_LEDGER, "sources")?;
                reject_docs_automation_external_path(
                    id,
                    "sources",
                    source,
                    &external_awareness_roots,
                )?;
            }
        }

        if let Some(path) = table.get("path").and_then(toml::Value::as_str) {
            reject_docs_automation_external_path(id, "path", path, &external_awareness_roots)?;
        }
        if let Some(path_glob) = table.get("path_glob").and_then(toml::Value::as_str) {
            reject_docs_automation_external_path(
                id,
                "path_glob",
                path_glob,
                &external_awareness_roots,
            )?;
        }
        let paths = docs_automation_paths(table, idx)?;
        for path in &paths {
            let path = path.display().to_string();
            reject_docs_automation_external_path(id, "path", &path, &external_awareness_roots)?;
        }
        if kind == "spec_status_dashboard" {
            if !paths
                .iter()
                .any(|path| path == Path::new(spec_status::DASHBOARD))
            {
                return Err(format!(
                    "{DOCS_AUTOMATION_LEDGER} generated_or_checked `{id}` must point at {}",
                    spec_status::DASHBOARD
                ));
            }
            spec_status::check_dashboard_impl()?;
        }
        if let Some(required_text) = table.get("must_include") {
            let required_text =
                toml_str_array(required_text, DOCS_AUTOMATION_LEDGER, "must_include")?;
            require_docs_automation_text(id, &paths, &required_text)?;
        }
    }

    Ok(ids.len())
}

fn require_scope_paths(
    scope: &toml::map::Map<String, toml::Value>,
    key: &str,
    must_exist: bool,
) -> Result<Vec<String>, String> {
    let Some(values) = scope.get(key) else {
        return Err(format!(
            "{DOCS_AUTOMATION_LEDGER} scope is missing array `{key}`"
        ));
    };
    let values = toml_str_array(values, DOCS_AUTOMATION_LEDGER, key)?;
    if values.is_empty() {
        return Err(format!(
            "{DOCS_AUTOMATION_LEDGER} scope `{key}` must not be empty"
        ));
    }
    if must_exist {
        for value in &values {
            require_existing_repo_path(value, DOCS_AUTOMATION_LEDGER, key)?;
        }
    }
    Ok(values.into_iter().map(str::to_string).collect())
}

fn check_docs_automation_scope_boundaries(
    owned_roots: &[String],
    external_awareness_roots: &[String],
) -> Result<(), String> {
    for owned_root in owned_roots {
        if let Some(external_root) = external_awareness_roots
            .iter()
            .find(|root| repo_path_is_under_scope_root(owned_root, root))
        {
            return Err(format!(
                "{DOCS_AUTOMATION_LEDGER} scope owned_roots entry `{owned_root}` must not be under external_awareness_only root `{external_root}`"
            ));
        }
    }
    Ok(())
}

fn reject_docs_automation_external_path(
    id: &str,
    field: &str,
    path: &str,
    external_awareness_roots: &[String],
) -> Result<(), String> {
    if let Some(external_root) = external_awareness_roots
        .iter()
        .find(|root| repo_path_is_under_scope_root(path, root))
    {
        return Err(format!(
            "{DOCS_AUTOMATION_LEDGER} generated_or_checked `{id}` {field} `{path}` must not be under external_awareness_only root `{external_root}`"
        ));
    }
    Ok(())
}

fn repo_path_is_under_scope_root(path: &str, root: &str) -> bool {
    let path = normalize_repo_scope_path(path);
    let root = normalize_repo_scope_path(root);
    path == root || path.starts_with(&format!("{root}/"))
}

fn normalize_repo_scope_path(value: &str) -> String {
    value
        .trim()
        .trim_start_matches("./")
        .trim_end_matches('/')
        .replace('\\', "/")
        .to_ascii_lowercase()
}

fn docs_automation_paths(
    table: &toml::map::Map<String, toml::Value>,
    idx: usize,
) -> Result<Vec<PathBuf>, String> {
    let path = table.get("path").and_then(toml::Value::as_str);
    let path_glob = table.get("path_glob").and_then(toml::Value::as_str);
    match (path, path_glob) {
        (Some(path), None) => {
            require_file(path)?;
            Ok(vec![PathBuf::from(path)])
        }
        (None, Some(path_glob)) => docs_automation_glob_paths(path_glob),
        (Some(_), Some(_)) => Err(format!(
            "{DOCS_AUTOMATION_LEDGER} generated_or_checked[{idx}] must not set both path and path_glob"
        )),
        (None, None) => Err(format!(
            "{DOCS_AUTOMATION_LEDGER} generated_or_checked[{idx}] must set path or path_glob"
        )),
    }
}

fn docs_automation_glob_paths(path_glob: &str) -> Result<Vec<PathBuf>, String> {
    let pattern_path = Path::new(path_glob);
    let file_pattern = pattern_path.file_name().and_then(|value| value.to_str());
    if file_pattern.is_some_and(|pattern| !pattern.contains('*')) {
        require_file(path_glob)?;
        return Ok(vec![PathBuf::from(path_glob)]);
    }

    let paths = docs_automation_paths::collect_paths(path_glob, DOCS_AUTOMATION_LEDGER)?;
    if paths.is_empty() {
        Err(format!(
            "{DOCS_AUTOMATION_LEDGER} path_glob `{path_glob}` did not match any files"
        ))
    } else {
        Ok(paths)
    }
}

fn require_docs_automation_text(
    id: &str,
    paths: &[PathBuf],
    required_text: &[&str],
) -> Result<(), String> {
    let mut documents = Vec::new();
    for path in paths {
        documents.push((path, read_to_string(path)?));
    }
    for needle in required_text {
        if !documents.iter().any(|(_, text)| text.contains(needle)) {
            let paths = paths
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!(
                "{DOCS_AUTOMATION_LEDGER} generated_or_checked `{id}` requires text `{needle}` in one of: {paths}"
            ));
        }
    }
    Ok(())
}

fn require_existing_repo_path(path: &str, ledger: &str, field: &str) -> Result<(), String> {
    if Path::new(path).exists() {
        Ok(())
    } else {
        Err(format!("{ledger} {field} path does not exist: {path}"))
    }
}

fn check_goals() -> Result<(), String> {
    let artifact_ids = check_doc_artifacts_impl()?;
    let source_index = parse_toml_file(Path::new(SOURCE_OF_TRUTH_INDEX))?;
    let indexed_artifact_ids = source_truth_index_ids(&source_index, "artifact")?;
    let indexed_lane_ids = source_truth_index_ids(&source_index, "lane")?;
    let value = parse_toml_file(Path::new(ACTIVE_GOAL_MANIFEST))?;
    require_toml_string(&value, "schema_version", ACTIVE_GOAL_MANIFEST)?;
    for key in ["id", "title", "status", "owner", "created", "objective"] {
        required_toml_string(&value, key, ACTIVE_GOAL_MANIFEST)?;
    }
    require_known(
        required_toml_string(&value, "status", ACTIVE_GOAL_MANIFEST)?,
        GOAL_WORK_ITEM_STATUSES,
        ACTIVE_GOAL_MANIFEST,
        "status",
    )?;
    let end_state = toml_array(&value, "end_state", ACTIVE_GOAL_MANIFEST)?;
    if end_state.is_empty() {
        return Err(format!(
            "{ACTIVE_GOAL_MANIFEST} end_state must not be empty"
        ));
    }
    for item in end_state {
        if item.as_str().is_none_or(|value| value.trim().is_empty()) {
            return Err(format!(
                "{ACTIVE_GOAL_MANIFEST} end_state entries must be non-empty strings"
            ));
        }
    }

    let work_items = toml_array(&value, "work_item", ACTIVE_GOAL_MANIFEST)?;
    if work_items.is_empty() {
        return Err(format!(
            "{ACTIVE_GOAL_MANIFEST} must list at least one work_item"
        ));
    }
    let mut ids = BTreeSet::new();
    for (idx, item) in work_items.iter().enumerate() {
        let table = toml_table(item, ACTIVE_GOAL_MANIFEST, "work_item", idx)?;
        let id = required_table_string(table, "id", ACTIVE_GOAL_MANIFEST, "work_item", idx)?;
        if !ids.insert(id.to_string()) {
            return Err(format!(
                "{ACTIVE_GOAL_MANIFEST} contains duplicate work_item `{id}`"
            ));
        }
        let status =
            required_table_string(table, "status", ACTIVE_GOAL_MANIFEST, "work_item", idx)?;
        require_known(
            status,
            GOAL_WORK_ITEM_STATUSES,
            ACTIVE_GOAL_MANIFEST,
            "work_item.status",
        )?;
        for key in ["proposal", "spec"] {
            if let Some(linked_id) = table.get(key).and_then(toml::Value::as_str)
                && !artifact_ids.contains(linked_id)
            {
                return Err(format!(
                    "{ACTIVE_GOAL_MANIFEST} work_item `{id}` references {key} `{linked_id}` not listed in {DOC_ARTIFACT_LEDGER}"
                ));
            }
            if let Some(linked_id) = table.get(key).and_then(toml::Value::as_str)
                && !indexed_artifact_ids.contains(linked_id)
            {
                return Err(format!(
                    "{ACTIVE_GOAL_MANIFEST} work_item `{id}` references {key} `{linked_id}` not listed in {SOURCE_OF_TRUTH_INDEX}"
                ));
            }
        }
        if !indexed_lane_ids.contains(id) {
            return Err(format!(
                "{ACTIVE_GOAL_MANIFEST} work_item `{id}` is not listed as a lane in {SOURCE_OF_TRUTH_INDEX}"
            ));
        }
        let plan = required_table_string(table, "plan", ACTIVE_GOAL_MANIFEST, "work_item", idx)?;
        require_file(plan)?;
        let commands = table.get("commands").ok_or_else(|| {
            format!("{ACTIVE_GOAL_MANIFEST} work_item `{id}` is missing commands")
        })?;
        let commands = toml_str_array(commands, ACTIVE_GOAL_MANIFEST, "commands")?;
        if commands.is_empty() {
            return Err(format!(
                "{ACTIVE_GOAL_MANIFEST} work_item `{id}` commands must not be empty"
            ));
        }
    }
    println!("check-goals: ok ({} work items)", ids.len());
    Ok(())
}

pub(crate) fn source_truth_index_ids(
    value: &toml::Value,
    kind: &str,
) -> Result<BTreeSet<String>, String> {
    let entries = toml_array(value, kind, SOURCE_OF_TRUTH_INDEX)?;
    let mut ids = BTreeSet::new();
    for (idx, entry) in entries.iter().enumerate() {
        let table = toml_table(entry, SOURCE_OF_TRUTH_INDEX, kind, idx)?;
        let id = required_table_string(table, "id", SOURCE_OF_TRUTH_INDEX, kind, idx)?;
        if !ids.insert(id.to_string()) {
            return Err(format!(
                "{SOURCE_OF_TRUTH_INDEX} contains duplicate {kind} id `{id}`"
            ));
        }
        let path = required_table_string(table, "path", SOURCE_OF_TRUTH_INDEX, kind, idx)?;
        require_file(path)?;
        required_table_string(table, "status", SOURCE_OF_TRUTH_INDEX, kind, idx)?;
        required_table_string(table, "owner", SOURCE_OF_TRUTH_INDEX, kind, idx)?;
    }
    Ok(ids)
}

fn source_truth_index_artifacts(
    value: &toml::Value,
) -> Result<BTreeMap<String, DocArtifactEntry>, String> {
    let entries = toml_array(value, "artifact", SOURCE_OF_TRUTH_INDEX)?;
    let mut artifacts = BTreeMap::new();
    for (idx, entry) in entries.iter().enumerate() {
        let table = toml_table(entry, SOURCE_OF_TRUTH_INDEX, "artifact", idx)?;
        let id = required_table_string(table, "id", SOURCE_OF_TRUTH_INDEX, "artifact", idx)?;
        if artifacts.contains_key(id) {
            return Err(format!(
                "{SOURCE_OF_TRUTH_INDEX} contains duplicate artifact id `{id}`"
            ));
        }
        let kind = required_table_string(table, "kind", SOURCE_OF_TRUTH_INDEX, "artifact", idx)?;
        let path = required_table_string(table, "path", SOURCE_OF_TRUTH_INDEX, "artifact", idx)?;
        let status =
            required_table_string(table, "status", SOURCE_OF_TRUTH_INDEX, "artifact", idx)?;
        let owner = required_table_string(table, "owner", SOURCE_OF_TRUTH_INDEX, "artifact", idx)?;
        require_file(path)?;
        artifacts.insert(
            id.to_string(),
            DocArtifactEntry {
                kind: kind.to_string(),
                path: path.to_string(),
                status: status.to_string(),
                owner: owner.to_string(),
            },
        );
    }
    Ok(artifacts)
}

fn check_package_boundary() -> Result<(), String> {
    let value = parse_toml_file(Path::new(PACKAGE_BOUNDARY_LEDGER))?;
    require_toml_string(&value, "schema_version", PACKAGE_BOUNDARY_LEDGER)?;
    let packages = toml_array(&value, "package", PACKAGE_BOUNDARY_LEDGER)?;
    if packages.is_empty() {
        return Err(format!(
            "{PACKAGE_BOUNDARY_LEDGER} must list at least one package"
        ));
    }
    let mut names = BTreeSet::new();
    for (idx, package) in packages.iter().enumerate() {
        let table = toml_table(package, PACKAGE_BOUNDARY_LEDGER, "package", idx)?;
        let name = required_table_string(table, "name", PACKAGE_BOUNDARY_LEDGER, "package", idx)?;
        if !names.insert(name.to_string()) {
            return Err(format!(
                "{PACKAGE_BOUNDARY_LEDGER} contains duplicate package `{name}`"
            ));
        }
        let path = required_table_string(table, "path", PACKAGE_BOUNDARY_LEDGER, "package", idx)?;
        let classification = required_table_string(
            table,
            "classification",
            PACKAGE_BOUNDARY_LEDGER,
            "package",
            idx,
        )?;
        require_known(
            classification,
            PACKAGE_CLASSIFICATIONS,
            PACKAGE_BOUNDARY_LEDGER,
            "classification",
        )?;
        required_table_string(table, "owner", PACKAGE_BOUNDARY_LEDGER, "package", idx)?;
        required_table_string(table, "reason", PACKAGE_BOUNDARY_LEDGER, "package", idx)?;
        require_file(&format!("{path}/Cargo.toml"))?;
    }
    println!("check-package-boundary: ok ({} packages)", names.len());
    Ok(())
}

fn check_ci_lanes() -> Result<(), String> {
    ci_lanes::check()
}

mod ci_lanes {
    use super::*;

    const REQUIRED_LANE_KEYS: &[&str] = &[
        "owner",
        "intent",
        "proof_obligation",
        "cost_estimate",
        "trigger_policy",
        "review_after",
    ];

    pub(super) fn check() -> Result<(), String> {
        let lanes = parse_lanes()?;
        let lane_ids = collect_lane_ids(lanes)?;
        println!("check-ci-lanes: ok ({} lanes)", lane_ids.len());
        Ok(())
    }

    fn parse_lanes() -> Result<Vec<toml::Value>, String> {
        let value = parse_toml_file(Path::new(CI_LANE_LEDGER))?;
        require_toml_string(&value, "schema_version", CI_LANE_LEDGER)?;
        let lanes = toml_array(&value, "lane", CI_LANE_LEDGER)?;
        if lanes.is_empty() {
            return Err(format!("{CI_LANE_LEDGER} must list at least one lane"));
        }
        Ok(lanes.to_vec())
    }

    fn collect_lane_ids(lanes: Vec<toml::Value>) -> Result<BTreeSet<String>, String> {
        let mut ids = BTreeSet::new();
        for (idx, lane) in lanes.iter().enumerate() {
            let lane_id = validate_lane(lane, idx)?;
            if !ids.insert(lane_id.to_string()) {
                return Err(format!(
                    "{CI_LANE_LEDGER} contains duplicate lane `{lane_id}`"
                ));
            }
        }
        Ok(ids)
    }

    fn validate_lane(lane: &toml::Value, idx: usize) -> Result<&str, String> {
        let table = toml_table(lane, CI_LANE_LEDGER, "lane", idx)?;
        let lane_id = required_table_string(table, "id", CI_LANE_LEDGER, "lane", idx)?;
        for key in REQUIRED_LANE_KEYS {
            required_table_string(table, key, CI_LANE_LEDGER, "lane", idx)?;
        }
        let status = required_table_string(table, "status", CI_LANE_LEDGER, "lane", idx)?;
        require_known(status, CI_LANE_STATUSES, CI_LANE_LEDGER, "status")?;
        if lane_id == "policy-contracts" {
            require_file(".github/workflows/policy-contracts.yml")?;
        }
        Ok(lane_id)
    }
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
    let manifest = calibration_manifest::validate()?;

    let accuracy_policy = parse_toml_file(&workspace_path("policy/accuracy-calibration.toml"))?;
    let label_count =
        accuracy_labels::check_accuracy_label_ledgers(&accuracy_policy, &manifest.fixture_cases)?;
    let report_stats =
        accuracy_calibration_report_stats(&accuracy_policy, manifest.case_count, label_count)?;
    check_accuracy_calibration_report(&report_stats)?;
    check_objective_audit_calibration_snapshot(&report_stats)?;

    println!(
        "check-calibration: ok ({} cases, {label_count} labels)",
        manifest.case_count
    );
    Ok(())
}

#[derive(Debug)]
struct AccuracyCalibrationReportStats {
    claim_count: usize,
    calibration_case_count: usize,
    label_ledger_count: usize,
    label_sample_count: usize,
    labeled_report_count: usize,
    fixture_pinned_claims: usize,
    dogfood_measured_claims: usize,
    labeled_calibrated_claims: usize,
    policy_eligible_claims: usize,
}

struct AccuracyPolicyClaim<'a> {
    id: &'a str,
    status: &'a str,
    label_ledgers: Vec<&'a str>,
    labeled_reports: Vec<&'a str>,
}

fn accuracy_calibration_report_stats(
    policy: &toml::Value,
    calibration_case_count: usize,
    label_sample_count: usize,
) -> Result<AccuracyCalibrationReportStats, String> {
    let claims = toml_array(policy, "claim", ACCURACY_CALIBRATION_POLICY)?;
    let dogfood_target_ids = dogfood_target_ids()?;
    let support_capabilities = support_tier_capabilities()?;
    let mut claim_ids = BTreeSet::new();
    let mut status_counts = BTreeMap::new();
    let mut label_ledgers = BTreeSet::new();
    let mut labeled_report_count = 0usize;
    for (idx, claim) in claims.iter().enumerate() {
        let table = toml_table(claim, ACCURACY_CALIBRATION_POLICY, "claim", idx)?;
        let claim =
            validate_accuracy_policy_claim(table, idx, &dogfood_target_ids, &support_capabilities)?;
        if !claim_ids.insert(claim.id.to_string()) {
            return Err(format!(
                "{ACCURACY_CALIBRATION_POLICY} claim[{idx}] duplicates id `{}`",
                claim.id
            ));
        }
        *status_counts
            .entry(claim.status.to_string())
            .or_insert(0usize) += 1;

        for ledger in claim.label_ledgers {
            label_ledgers.insert(ledger.to_string());
        }
        for report in claim.labeled_reports {
            require_file(report)?;
            labeled_report_count += 1;
        }
    }

    Ok(AccuracyCalibrationReportStats {
        claim_count: claims.len(),
        calibration_case_count,
        label_ledger_count: label_ledgers.len(),
        label_sample_count,
        labeled_report_count,
        fixture_pinned_claims: *status_counts.get("fixture_pinned").unwrap_or(&0),
        dogfood_measured_claims: *status_counts.get("dogfood_measured").unwrap_or(&0),
        labeled_calibrated_claims: *status_counts.get("labeled_calibrated").unwrap_or(&0),
        policy_eligible_claims: *status_counts.get("policy_eligible").unwrap_or(&0),
    })
}

fn validate_accuracy_policy_claim<'a>(
    table: &'a toml::map::Map<String, toml::Value>,
    idx: usize,
    known_dogfood_targets: &BTreeSet<String>,
    support_capabilities: &BTreeSet<String>,
) -> Result<AccuracyPolicyClaim<'a>, String> {
    let id = required_table_string(table, "id", ACCURACY_CALIBRATION_POLICY, "claim", idx)?;
    let status = required_table_string(table, "status", ACCURACY_CALIBRATION_POLICY, "claim", idx)?;
    require_known(
        status,
        ACCURACY_CLAIM_STATUSES,
        ACCURACY_CALIBRATION_POLICY,
        "claim.status",
    )?;
    let kind = required_table_string(table, "kind", ACCURACY_CALIBRATION_POLICY, "claim", idx)?;
    require_known(
        kind,
        ACCURACY_CLAIM_KINDS,
        ACCURACY_CALIBRATION_POLICY,
        "claim.kind",
    )?;
    required_table_string(table, "owner", ACCURACY_CALIBRATION_POLICY, "claim", idx)?;
    let support_tier = required_table_string(
        table,
        "support_tier",
        ACCURACY_CALIBRATION_POLICY,
        "claim",
        idx,
    )?;
    if !support_capabilities.contains(support_tier) {
        return Err(format!(
            "{ACCURACY_CALIBRATION_POLICY} claim[{idx}] support_tier `{support_tier}` is not a capability in {SUPPORT_TIERS_DOC}"
        ));
    }

    let fixtures =
        required_table_str_array(table, "fixtures", ACCURACY_CALIBRATION_POLICY, "claim", idx)?;
    let dogfood_targets = required_table_str_array(
        table,
        "dogfood_targets",
        ACCURACY_CALIBRATION_POLICY,
        "claim",
        idx,
    )?;
    let label_ledgers = required_table_str_array(
        table,
        "label_ledgers",
        ACCURACY_CALIBRATION_POLICY,
        "claim",
        idx,
    )?;
    let labeled_reports = required_table_str_array(
        table,
        "labeled_reports",
        ACCURACY_CALIBRATION_POLICY,
        "claim",
        idx,
    )?;
    let forbidden_claims = required_table_str_array(
        table,
        "forbidden_claims",
        ACCURACY_CALIBRATION_POLICY,
        "claim",
        idx,
    )?;
    let allowed_public_claim = required_table_string(
        table,
        "allowed_public_claim",
        ACCURACY_CALIBRATION_POLICY,
        "claim",
        idx,
    )?;

    if allowed_public_claim.trim().len() < 32 {
        return Err(format!(
            "{ACCURACY_CALIBRATION_POLICY} claim[{idx}] allowed_public_claim is too short to bound the claim"
        ));
    }
    for term in ACCURACY_PROMOTION_FORBIDDEN_TERMS {
        if text_contains_ignore_ascii_case(allowed_public_claim, term) {
            return Err(format!(
                "{ACCURACY_CALIBRATION_POLICY} claim[{idx}] allowed_public_claim contains forbidden promotion term `{term}`"
            ));
        }
    }
    let required_claim_level = accuracy_claim_status_public_label(status);
    if !text_contains_ignore_ascii_case(allowed_public_claim, required_claim_level) {
        return Err(format!(
            "{ACCURACY_CALIBRATION_POLICY} claim[{idx}] allowed_public_claim must include claim level `{required_claim_level}`"
        ));
    }
    if forbidden_claims.is_empty() {
        return Err(format!(
            "{ACCURACY_CALIBRATION_POLICY} claim[{idx}] forbidden_claims must list overclaims this entry does not support"
        ));
    }
    for required in ACCURACY_REQUIRED_FORBIDDEN_CLAIMS {
        if !forbidden_claims
            .iter()
            .any(|claim| claim.eq_ignore_ascii_case(required))
        {
            return Err(format!(
                "{ACCURACY_CALIBRATION_POLICY} claim[{idx}] forbidden_claims must include `{required}`"
            ));
        }
    }

    for ledger in &label_ledgers {
        require_file(ledger)?;
    }
    for report in &labeled_reports {
        require_file(report)?;
    }
    for target in &dogfood_targets {
        if !known_dogfood_targets.contains(*target) {
            return Err(format!(
                "{ACCURACY_CALIBRATION_POLICY} claim[{idx}] references unknown dogfood target `{target}`"
            ));
        }
    }

    match status {
        "fixture_pinned" => {
            if fixtures.is_empty() {
                return Err(format!(
                    "{ACCURACY_CALIBRATION_POLICY} claim[{idx}] fixture_pinned claims require fixtures"
                ));
            }
            if label_ledgers.is_empty() {
                return Err(format!(
                    "{ACCURACY_CALIBRATION_POLICY} claim[{idx}] fixture_pinned claims require label_ledgers"
                ));
            }
            if !dogfood_targets.is_empty() {
                return Err(format!(
                    "{ACCURACY_CALIBRATION_POLICY} claim[{idx}] fixture_pinned claims must not carry dogfood_targets"
                ));
            }
            if !labeled_reports.is_empty() {
                return Err(format!(
                    "{ACCURACY_CALIBRATION_POLICY} claim[{idx}] fixture_pinned claims must not carry labeled_reports"
                ));
            }
        }
        "dogfood_measured" if dogfood_targets.is_empty() => {
            return Err(format!(
                "{ACCURACY_CALIBRATION_POLICY} claim[{idx}] dogfood_measured claims require dogfood_targets"
            ));
        }
        "labeled_calibrated" if labeled_reports.is_empty() => {
            return Err(format!(
                "{ACCURACY_CALIBRATION_POLICY} claim[{idx}] labeled_calibrated claims require labeled_reports"
            ));
        }
        "policy_eligible" if labeled_reports.is_empty() => {
            return Err(format!(
                "{ACCURACY_CALIBRATION_POLICY} claim[{idx}] policy_eligible claims require labeled_reports"
            ));
        }
        _ => {}
    }

    Ok(AccuracyPolicyClaim {
        id,
        status,
        label_ledgers,
        labeled_reports,
    })
}

fn accuracy_claim_status_public_label(status: &str) -> &str {
    match status {
        "fixture_pinned" => "Fixture-pinned",
        "dogfood_measured" => "Dogfood measured",
        "labeled_calibrated" => "Labeled calibrated",
        "policy_eligible" => "Policy-eligible",
        _ => status,
    }
}

fn required_table_str_array<'a>(
    table: &'a toml::map::Map<String, toml::Value>,
    key: &str,
    path: &str,
    table_name: &str,
    idx: usize,
) -> Result<Vec<&'a str>, String> {
    let Some(value) = table.get(key) else {
        return Err(format!(
            "{path} {table_name}[{idx}] is missing array `{key}`"
        ));
    };
    toml_str_array(value, path, &format!("{table_name}.{key}"))
}

fn dogfood_target_ids() -> Result<BTreeSet<String>, String> {
    let value = parse_toml_file(&workspace_path(DOGFOOD_MANIFEST))?;
    let targets = toml_array(&value, "targets", DOGFOOD_MANIFEST)?;
    let mut ids = BTreeSet::new();
    for (idx, target) in targets.iter().enumerate() {
        let table = toml_table(target, DOGFOOD_MANIFEST, "targets", idx)?;
        let id = required_target_string(table, "id", idx)?;
        if !ids.insert(id.to_string()) {
            return Err(format!(
                "{DOGFOOD_MANIFEST} contains duplicate target id `{id}`"
            ));
        }
    }
    Ok(ids)
}

fn check_accuracy_calibration_report(stats: &AccuracyCalibrationReportStats) -> Result<(), String> {
    require_file(ACCURACY_CALIBRATION_REPORT)?;
    let text = read_to_string(&workspace_path(ACCURACY_CALIBRATION_REPORT))?;
    check_accuracy_calibration_report_text(ACCURACY_CALIBRATION_REPORT, &text, stats)
}

fn check_objective_audit_calibration_snapshot(
    stats: &AccuracyCalibrationReportStats,
) -> Result<(), String> {
    require_file(OBJECTIVE_AUDIT)?;
    let text = read_to_string(&workspace_path(OBJECTIVE_AUDIT))?;
    check_objective_audit_calibration_snapshot_text(OBJECTIVE_AUDIT, &text, stats)
}

fn check_objective_audit_calibration_snapshot_text(
    path: &str,
    text: &str,
    stats: &AccuracyCalibrationReportStats,
) -> Result<(), String> {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    for expected in [
        format!("{} fixture-pinned claims", stats.fixture_pinned_claims),
        format!("{} calibration cases", stats.calibration_case_count),
        format!("{} label ledgers", stats.label_ledger_count),
        format!("{} label samples", stats.label_sample_count),
    ] {
        if !normalized.contains(&expected) {
            return Err(format!(
                "{path} is missing current calibration snapshot `{expected}`"
            ));
        }
    }
    Ok(())
}

fn check_accuracy_calibration_report_text(
    path: &str,
    text: &str,
    stats: &AccuracyCalibrationReportStats,
) -> Result<(), String> {
    require_boundary_text(text, path)?;
    for needle in [
        "No global precision/recall claim",
        "No policy readiness claim",
        "No support-tier promotion",
        "No witness execution claim",
        "No memory-safety proof",
        "No UB-free status",
        "No Miri-clean status",
    ] {
        if !text_contains_ignore_ascii_case(text, needle) {
            return Err(format!(
                "{path} is missing required boundary text `{needle}`"
            ));
        }
    }

    for expected in [
        format!("- Claims: {}", stats.claim_count),
        format!("- Fixture-pinned claims: {}", stats.fixture_pinned_claims),
        format!(
            "- Dogfood-measured claims: {}",
            stats.dogfood_measured_claims
        ),
        format!(
            "- Labeled-calibrated claims: {}",
            stats.labeled_calibrated_claims
        ),
        format!("- Policy-eligible claims: {}", stats.policy_eligible_claims),
        format!("- Calibration cases: {}", stats.calibration_case_count),
        format!("- Label ledgers: {}", stats.label_ledger_count),
        format!("- Label samples: {}", stats.label_sample_count),
        format!("- Labeled reports: {}", stats.labeled_report_count),
    ] {
        if !text.contains(&expected) {
            return Err(format!(
                "{path} is missing expected report line `{expected}`"
            ));
        }
    }

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
    let mut artifact_status_counts = BTreeMap::new();
    let mut fixture_control_ids = BTreeSet::new();
    let mut repo_snapshots = 0usize;
    let mut pr_diffs = 0usize;
    let mut fixture_controls = 0usize;
    for (idx, target) in targets.iter().enumerate() {
        let stats = dogfood_checks::validate_target(target, idx, &mut ids)?;
        if let Some(repository) = stats.repository {
            repositories.insert(repository);
        }
        *artifact_status_counts
            .entry(stats.artifact_status)
            .or_insert(0usize) += 1;
        repo_snapshots += stats.repo_snapshots;
        pr_diffs += stats.pr_diffs;
        fixture_controls += stats.fixture_controls;
        if let Some(id) = stats.fixture_control_id {
            fixture_control_ids.insert(id);
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
    check_dogfood_index(
        targets.len(),
        repositories.len(),
        repo_snapshots,
        pr_diffs,
        fixture_controls,
        &repositories,
        &fixture_control_ids,
        &artifact_status_counts,
    )?;
    check_dogfood_report_triage_labels()?;
    check_dogfood_reports_indexed()?;
    check_dogfood_report_trust_boundaries()?;
    check_dogfood_report_overclaims()?;
    check_dogfood_follow_up_seeds(&ids)?;
    check_dogfood_judgment_schema_docs()?;
    check_dogfood_judgments(&ids)?;

    println!(
        "check-dogfood: ok ({} targets, {} repositories)",
        targets.len(),
        repositories.len()
    );
    Ok(())
}

fn check_dogfood_judgments(known_targets: &BTreeSet<String>) -> Result<(), String> {
    let judgment_dir = workspace_path(DOGFOOD_JUDGMENT_DIR);
    if !judgment_dir.is_dir() {
        return Err(format!("{DOGFOOD_JUDGMENT_DIR} is missing"));
    }

    let reports = dogfood_report_names()?;
    let known_families_or_surfaces = dogfood_known_families_or_surfaces()?;
    let mut judgment_files = Vec::new();
    for entry in fs::read_dir(&judgment_dir)
        .map_err(|err| format!("read {DOGFOOD_JUDGMENT_DIR} failed: {err}"))?
    {
        let entry =
            entry.map_err(|err| format!("read {DOGFOOD_JUDGMENT_DIR} entry failed: {err}"))?;
        let path = entry.path();
        if path.file_name().and_then(|name| name.to_str()) == Some("README.md") {
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
            return Err(format!(
                "{} may contain only README.md and *.toml judgment files; found {}",
                DOGFOOD_JUDGMENT_DIR,
                path.display()
            ));
        }
        judgment_files.push(path);
    }
    judgment_files.sort();

    for path in judgment_files {
        let judgment_path = path.to_string_lossy().replace('\\', "/");
        let text = read_to_string(&path)?;
        check_dogfood_judgment_text(
            &judgment_path,
            &text,
            known_targets,
            &known_families_or_surfaces,
            &reports,
        )?;
    }

    Ok(())
}

fn dogfood_known_families_or_surfaces() -> Result<BTreeSet<String>, String> {
    let mut known = operation_family_registry_rows()?;
    known.extend(
        DOGFOOD_FOLLOW_UP_SURFACES
            .iter()
            .map(|surface| (*surface).to_string()),
    );
    known.extend(
        DOGFOOD_JUDGMENT_SURFACES
            .iter()
            .map(|surface| (*surface).to_string()),
    );
    Ok(known)
}

fn check_dogfood_judgment_text(
    path: &str,
    text: &str,
    known_targets: &BTreeSet<String>,
    known_families_or_surfaces: &BTreeSet<String>,
    reports: &[String],
) -> Result<usize, String> {
    reject_positive_overclaims(Path::new(path), text)?;
    let value = text
        .parse::<toml::Table>()
        .map(toml::Value::Table)
        .map_err(|err| format!("parse {path} failed: {err}"))?;
    check_dogfood_judgment_value(
        path,
        &value,
        known_targets,
        known_families_or_surfaces,
        reports,
    )
}

fn check_dogfood_judgment_value(
    path: &str,
    value: &toml::Value,
    known_targets: &BTreeSet<String>,
    known_families_or_surfaces: &BTreeSet<String>,
    reports: &[String],
) -> Result<usize, String> {
    let schema_version = required_toml_string(value, "schema_version", path)?;
    if schema_version != "1.0" {
        return Err(format!(
            "{path} unsupported dogfood judgment schema_version `{schema_version}`"
        ));
    }

    let target = required_toml_string(value, "target", path)?;
    if !known_targets.contains(target) {
        return Err(format!(
            "{path} references unknown dogfood target `{target}`"
        ));
    }

    let report = required_toml_string(value, "report", path)?;
    check_dogfood_judgment_report(path, report, reports)?;
    required_toml_string(value, "reviewer", path)?;
    let date = required_toml_string(value, "date", path)?;
    check_dogfood_judgment_date(path, date)?;
    required_toml_string(value, "scope", path)?;

    let trust_boundary = required_toml_string(value, "trust_boundary", path)?;
    check_dogfood_judgment_trust_boundary(path, trust_boundary)?;

    let card_ids = if let Some(cards_artifact) =
        optional_nonempty_toml_string(value, "cards_artifact", path)?
    {
        check_dogfood_judgment_relative_path(path, "cards_artifact", cards_artifact)?;
        Some(dogfood_card_ids_from_artifact(path, cards_artifact)?)
    } else {
        None
    };

    let mut rows = 0usize;
    if let Some(cards) = optional_toml_array(value, "cards", path)? {
        for (idx, card) in cards.iter().enumerate() {
            let table = toml_table(card, path, "cards", idx)?;
            check_dogfood_judgment_card(
                path,
                table,
                idx,
                known_families_or_surfaces,
                card_ids.as_ref(),
            )?;
            rows += 1;
        }
    }
    if let Some(missed) = optional_toml_array(value, "missed", path)? {
        for (idx, missed) in missed.iter().enumerate() {
            let table = toml_table(missed, path, "missed", idx)?;
            check_dogfood_missed_judgment(path, table, idx, known_families_or_surfaces)?;
            rows += 1;
        }
    }

    if rows == 0 {
        return Err(format!(
            "{path} must include at least one `[[cards]]` or `[[missed]]` judgment"
        ));
    }
    Ok(rows)
}

fn check_dogfood_judgment_report(
    path: &str,
    report: &str,
    reports: &[String],
) -> Result<(), String> {
    check_dogfood_judgment_relative_path(path, "report", report)?;
    let Some(report_name) = report.strip_prefix("reports/") else {
        return Err(format!("{path} report must link under reports/"));
    };
    let report_set = reports.iter().map(String::as_str).collect::<BTreeSet<_>>();
    if !report_set.contains(report_name) {
        return Err(format!("{path} links missing dogfood report `{report}`"));
    }
    Ok(())
}

fn check_dogfood_judgment_date(path: &str, date: &str) -> Result<(), String> {
    let bytes = date.as_bytes();
    let valid = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(idx, byte)| idx == 4 || idx == 7 || byte.is_ascii_digit());
    if valid {
        Ok(())
    } else {
        Err(format!("{path} date `{date}` must use YYYY-MM-DD"))
    }
}

fn check_dogfood_judgment_trust_boundary(path: &str, text: &str) -> Result<(), String> {
    require_boundary_text(text, path)?;
    let lower = text.to_ascii_lowercase();
    for needle in [
        "not calibrated",
        "precision",
        "recall",
        "site execution",
        "witness adequacy",
        "policy readiness",
    ] {
        if !lower.contains(needle) {
            return Err(format!("{path} trust_boundary must document `{needle}`"));
        }
    }
    Ok(())
}

fn check_dogfood_judgment_card(
    path: &str,
    table: &toml::map::Map<String, toml::Value>,
    idx: usize,
    known_families_or_surfaces: &BTreeSet<String>,
    card_ids: Option<&BTreeSet<String>>,
) -> Result<(), String> {
    let family = required_table_string(table, "family", path, "cards", idx)?;
    if !known_families_or_surfaces.contains(family) {
        return Err(format!(
            "{path} cards[{idx}] references unknown family/surface `{family}`"
        ));
    }
    let judgment = required_table_string(table, "judgment", path, "cards", idx)?;
    if !DOGFOOD_JUDGMENT_LABELS.contains(&judgment) {
        return Err(format!(
            "{path} cards[{idx}] uses unknown judgment `{judgment}`"
        ));
    }
    required_table_string(table, "reason", path, "cards", idx)?;
    required_table_string(table, "next_step", path, "cards", idx)?;

    if let Some(card_ids) = card_ids {
        let card_id = required_table_string(table, "card_id", path, "cards", idx)?;
        if !card_ids.contains(card_id) {
            return Err(format!(
                "{path} cards[{idx}] card_id `{card_id}` is not present in cards_artifact"
            ));
        }
    } else if let Some(card_id) = table.get("card_id") {
        let Some(card_id) = card_id.as_str() else {
            return Err(format!("{path} cards[{idx}] `card_id` must be a string"));
        };
        if card_id.trim().is_empty() {
            return Err(format!("{path} cards[{idx}] string `card_id` is empty"));
        }
    }

    Ok(())
}

fn check_dogfood_missed_judgment(
    path: &str,
    table: &toml::map::Map<String, toml::Value>,
    idx: usize,
    known_families_or_surfaces: &BTreeSet<String>,
) -> Result<(), String> {
    let file = required_table_string(table, "file", path, "missed", idx)?;
    check_dogfood_judgment_relative_path(path, "missed.file", file)?;
    let Some(line) = table.get("line").and_then(toml::Value::as_integer) else {
        return Err(format!("{path} missed[{idx}] is missing integer `line`"));
    };
    if line <= 0 {
        return Err(format!("{path} missed[{idx}] line must be positive"));
    }
    let expected_family = required_table_string(table, "expected_family", path, "missed", idx)?;
    if !known_families_or_surfaces.contains(expected_family) {
        return Err(format!(
            "{path} missed[{idx}] references unknown expected_family `{expected_family}`"
        ));
    }
    let status = required_table_string(table, "status", path, "missed", idx)?;
    if !DOGFOOD_MISSED_JUDGMENT_STATUSES.contains(&status) {
        return Err(format!(
            "{path} missed[{idx}] uses unknown status `{status}`"
        ));
    }
    required_table_string(table, "reason", path, "missed", idx)?;
    required_table_string(table, "next_step", path, "missed", idx)?;
    Ok(())
}

fn optional_toml_array<'a>(
    value: &'a toml::Value,
    key: &str,
    path: &str,
) -> Result<Option<&'a Vec<toml::Value>>, String> {
    let Some(value) = value.get(key) else {
        return Ok(None);
    };
    value
        .as_array()
        .ok_or_else(|| format!("{path} `{key}` must be an array"))
        .map(Some)
}

fn optional_nonempty_toml_string<'a>(
    value: &'a toml::Value,
    key: &str,
    path: &str,
) -> Result<Option<&'a str>, String> {
    let Some(value) = value.get(key) else {
        return Ok(None);
    };
    let Some(value) = value.as_str() else {
        return Err(format!("{path} `{key}` must be a string"));
    };
    if value.trim().is_empty() {
        return Err(format!("{path} string key `{key}` is empty"));
    }
    Ok(Some(value))
}

fn check_dogfood_judgment_relative_path(
    path: &str,
    field: &str,
    value: &str,
) -> Result<(), String> {
    if value.starts_with('/') || has_windows_path(value) || value.contains("..") {
        return Err(format!(
            "{path} `{field}` path must be relative, forward-slash only, and stay inside the workspace: {value}"
        ));
    }
    Ok(())
}

fn dogfood_card_ids_from_artifact(
    path: &str,
    cards_artifact: &str,
) -> Result<BTreeSet<String>, String> {
    let artifact_path = workspace_path(cards_artifact);
    let text = read_to_string(&artifact_path)?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|err| format!("parse {cards_artifact} for {path} failed: {err}"))?;
    dogfood_card_ids_from_json_value(path, cards_artifact, &value)
}

fn dogfood_card_ids_from_json_value(
    path: &str,
    cards_artifact: &str,
    value: &serde_json::Value,
) -> Result<BTreeSet<String>, String> {
    let cards = if let Some(cards) = value.as_array() {
        cards
    } else {
        value
            .get("cards")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                format!("{path} cards_artifact `{cards_artifact}` must contain a cards array")
            })?
    };
    if cards.is_empty() {
        return Err(format!(
            "{path} cards_artifact `{cards_artifact}` has no cards"
        ));
    }
    let mut ids = BTreeSet::new();
    for (idx, card) in cards.iter().enumerate() {
        let Some(id) = card.get("id").and_then(serde_json::Value::as_str) else {
            return Err(format!(
                "{path} cards_artifact `{cards_artifact}` cards[{idx}] is missing string `id`"
            ));
        };
        if id.trim().is_empty() {
            return Err(format!(
                "{path} cards_artifact `{cards_artifact}` cards[{idx}] id is empty"
            ));
        }
        ids.insert(id.to_string());
    }
    Ok(ids)
}

fn check_dogfood_judgment_schema_docs() -> Result<(), String> {
    let readme = read_to_string(&workspace_path(DOGFOOD_README))?;
    if !readme.contains("judgments/README.md") {
        return Err(format!(
            "{DOGFOOD_README} must link `{DOGFOOD_JUDGMENTS_README}`"
        ));
    }

    let path = workspace_path(DOGFOOD_JUDGMENTS_README);
    let text = read_to_string(&path)?;
    require_boundary_text(&text, DOGFOOD_JUDGMENTS_README)?;
    reject_positive_overclaims(&path, &text)?;

    let lower = text.to_ascii_lowercase();
    for needle in [
        "docs/dogfood/judgments/<target>.toml",
        "not calibrated",
        "precision",
        "recall",
        "site execution",
        "witness adequacy",
        "policy readiness",
    ] {
        if !lower.contains(needle) {
            return Err(format!(
                "{DOGFOOD_JUDGMENTS_README} must document `{needle}`"
            ));
        }
    }

    for label in DOGFOOD_JUDGMENT_LABELS {
        let needle = format!("`{label}`");
        if !text.contains(&needle) {
            return Err(format!(
                "{DOGFOOD_JUDGMENTS_README} must document reviewer judgment label `{label}`"
            ));
        }
    }

    for field in [
        "schema_version",
        "target",
        "report",
        "reviewer",
        "date",
        "scope",
        "trust_boundary",
        "cards_artifact",
        "[[cards]]",
        "card_id",
        "family",
        "judgment",
        "reason",
        "next_step",
        "[[missed]]",
        "expected_family",
        "status",
    ] {
        if !text.contains(field) {
            return Err(format!(
                "{DOGFOOD_JUDGMENTS_README} must document reviewer judgment field `{field}`"
            ));
        }
    }

    Ok(())
}

fn check_dogfood_report_trust_boundaries() -> Result<(), String> {
    let report_dir = workspace_path(DOGFOOD_REPORT_DIR);
    if !report_dir.is_dir() {
        return Err(format!("{DOGFOOD_REPORT_DIR} is missing"));
    }
    for entry in fs::read_dir(&report_dir)
        .map_err(|err| format!("read {DOGFOOD_REPORT_DIR} failed: {err}"))?
    {
        let entry =
            entry.map_err(|err| format!("read {DOGFOOD_REPORT_DIR} entry failed: {err}"))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        let report_path = path.to_string_lossy().replace('\\', "/");
        let text = read_to_string(&path)?;
        check_dogfood_report_trust_boundary_text(&report_path, &text)?;
    }
    Ok(())
}

fn check_dogfood_report_trust_boundary_text(path: &str, text: &str) -> Result<(), String> {
    let lower = text.to_ascii_lowercase();
    if !lower.contains("## trust boundary") {
        return Err(format!("{path} must include a `## Trust boundary` section"));
    }
    if !lower.contains("witness") {
        return Err(format!("{path} trust boundary must mention witness limits"));
    }
    for (label, required) in [
        ("safety", &["memory-safety", " safe", "safe,"][..]),
        ("ub-free", &["ub-free"][..]),
        ("miri-clean", &["miri-clean"][..]),
        ("site-execution", &["site-execut"][..]),
        ("calibration", &["calibrated", "precision", "recall"][..]),
        ("policy", &["policy"][..]),
    ] {
        if !required.iter().any(|needle| lower.contains(needle)) {
            return Err(format!(
                "{path} trust boundary must mention `{label}` limits"
            ));
        }
    }
    Ok(())
}

fn check_dogfood_report_overclaims() -> Result<(), String> {
    let report_dir = workspace_path(DOGFOOD_REPORT_DIR);
    if !report_dir.is_dir() {
        return Err(format!("{DOGFOOD_REPORT_DIR} is missing"));
    }
    for entry in fs::read_dir(&report_dir)
        .map_err(|err| format!("read {DOGFOOD_REPORT_DIR} failed: {err}"))?
    {
        let entry =
            entry.map_err(|err| format!("read {DOGFOOD_REPORT_DIR} entry failed: {err}"))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        let text = read_to_string(&path)?;
        reject_positive_overclaims(&path, &text)?;
    }
    Ok(())
}

fn check_dogfood_reports_indexed() -> Result<(), String> {
    let readme = read_to_string(&workspace_path(DOGFOOD_README))?;
    let reports = dogfood_report_names()?;
    check_dogfood_report_index_text(DOGFOOD_README, &readme, &reports)
}

fn check_dogfood_report_index_text(
    path: &str,
    text: &str,
    reports: &[String],
) -> Result<(), String> {
    if reports.is_empty() {
        return Err(format!("{DOGFOOD_REPORT_DIR} has no Markdown reports"));
    }
    for report in reports {
        let link = format!("reports/{report}");
        if !text.contains(&link) {
            return Err(format!("{path} must link dogfood report `{link}`"));
        }
    }
    let report_set = reports.iter().map(String::as_str).collect::<BTreeSet<_>>();
    for linked_report in dogfood_report_links_from_text(text) {
        if !report_set.contains(linked_report.as_str()) {
            return Err(format!(
                "{path} links missing dogfood report `reports/{linked_report}`"
            ));
        }
    }
    Ok(())
}

fn dogfood_report_links_from_text(text: &str) -> BTreeSet<String> {
    let mut links = BTreeSet::new();
    for line in text.lines() {
        let mut rest = line;
        while let Some(pos) = rest.find("reports/") {
            let after_prefix = &rest[pos + "reports/".len()..];
            let end = after_prefix
                .find(|ch: char| ch == ')' || ch == '"' || ch == '\'' || ch.is_whitespace())
                .unwrap_or(after_prefix.len());
            let report = after_prefix[..end].trim_end_matches(['.', ',', ';']);
            if report.ends_with(".md") {
                links.insert(report.to_string());
            }
            rest = &after_prefix[end.min(after_prefix.len())..];
        }
    }
    links
}

fn dogfood_report_names() -> Result<Vec<String>, String> {
    let report_dir = workspace_path(DOGFOOD_REPORT_DIR);
    let mut reports = Vec::new();
    for entry in fs::read_dir(&report_dir)
        .map_err(|err| format!("read {DOGFOOD_REPORT_DIR} failed: {err}"))?
    {
        let entry =
            entry.map_err(|err| format!("read {DOGFOOD_REPORT_DIR} entry failed: {err}"))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            return Err(format!(
                "{} report has non-UTF-8 file name: {}",
                DOGFOOD_REPORT_DIR,
                path.display()
            ));
        };
        reports.push(file_name.to_string());
    }
    reports.sort();
    Ok(reports)
}

fn check_dogfood_follow_up_seeds(known_targets: &BTreeSet<String>) -> Result<(), String> {
    let readme = read_to_string(&workspace_path(DOGFOOD_README))?;
    if !readme.contains("follow-up-seeds.md") {
        return Err(format!(
            "{DOGFOOD_README} must link `{DOGFOOD_FOLLOW_UP_SEEDS}`"
        ));
    }
    let text = read_to_string(&workspace_path(DOGFOOD_FOLLOW_UP_SEEDS))?;
    check_dogfood_follow_up_status_glossary(DOGFOOD_FOLLOW_UP_SEEDS, &text)?;
    let reports = dogfood_report_names()?;
    let report_triage_keys = dogfood_report_triage_keys_by_report(&reports)?;
    let mut known_families_or_surfaces = operation_family_registry_rows()?;
    known_families_or_surfaces.extend(
        DOGFOOD_FOLLOW_UP_SURFACES
            .iter()
            .map(|surface| (*surface).to_string()),
    );
    check_dogfood_follow_up_seeds_text(
        DOGFOOD_FOLLOW_UP_SEEDS,
        &text,
        known_targets,
        &known_families_or_surfaces,
        &reports,
        &report_triage_keys,
    )?;
    Ok(())
}

fn check_dogfood_follow_up_seeds_text(
    path: &str,
    text: &str,
    known_targets: &BTreeSet<String>,
    known_families_or_surfaces: &BTreeSet<String>,
    reports: &[String],
    report_triage_keys: &BTreeMap<String, BTreeSet<(String, String)>>,
) -> Result<usize, String> {
    let lower = text.to_ascii_lowercase();
    if !lower.contains("## trust boundary") {
        return Err(format!("{path} must include a `## Trust boundary` section"));
    }
    if !lower.contains("not a proof")
        || !lower.contains("ub-free")
        || !lower.contains("miri-clean")
        || !lower.contains("site execution")
        || !lower.contains("calibrated")
        || !lower.contains("witness")
        || !lower.contains("policy")
    {
        return Err(format!(
            "{path} trust boundary must preserve dogfood advisory limits"
        ));
    }

    let report_set = reports.iter().map(String::as_str).collect::<BTreeSet<_>>();
    let mut in_table = false;
    let mut rows = 0usize;
    let mut seed_ids = BTreeSet::new();
    for (line_idx, line) in text.lines().enumerate() {
        if !in_table {
            if line.contains("| Seed ID |") {
                let columns = markdown_table_columns(line);
                if columns != DOGFOOD_FOLLOW_UP_HEADER {
                    return Err(format!(
                        "{path}:{} dogfood follow-up header must be `{}`",
                        line_idx + 1,
                        DOGFOOD_FOLLOW_UP_HEADER.join(" | ")
                    ));
                }
                in_table = true;
            }
            continue;
        }
        if !line.trim_start().starts_with('|') {
            break;
        }
        if line.contains("|---") {
            continue;
        }
        let columns = markdown_table_columns(line);
        if columns.len() != DOGFOOD_FOLLOW_UP_HEADER.len() {
            return Err(format!(
                "{path}:{} dogfood follow-up row must include Seed ID, Status, Target, Family/surface, Primary label, Source report, Next PR slice, and Notes columns",
                line_idx + 1
            ));
        }
        for (column_idx, column_name) in DOGFOOD_FOLLOW_UP_HEADER.iter().enumerate() {
            if markdown_code_cell_value(columns[column_idx]).is_empty() {
                return Err(format!(
                    "{path}:{} dogfood follow-up row must include a non-empty {column_name} column",
                    line_idx + 1
                ));
            }
        }

        let seed_id = markdown_code_cell_value(columns[0]);
        if !seed_ids.insert(seed_id.clone()) {
            return Err(format!(
                "{path}:{} duplicate dogfood follow-up seed id `{seed_id}`",
                line_idx + 1
            ));
        }

        let status = markdown_code_cell_value(columns[1]);
        if !DOGFOOD_FOLLOW_UP_STATUSES.contains(&status.as_str()) {
            return Err(format!(
                "{path}:{} unknown dogfood follow-up status `{status}`",
                line_idx + 1
            ));
        }

        let next_pr_slice = markdown_code_cell_value(columns[6]);
        check_dogfood_follow_up_next_pr_slice(path, line_idx + 1, &seed_id, &next_pr_slice)?;

        let target = markdown_code_cell_value(columns[2]);
        if !known_targets.contains(&target) {
            return Err(format!(
                "{path}:{} dogfood follow-up seed `{seed_id}` references unknown target `{target}`",
                line_idx + 1
            ));
        }

        let family_or_surface = markdown_code_cell_value(columns[3]);
        if !known_families_or_surfaces.contains(&family_or_surface) {
            return Err(format!(
                "{path}:{} dogfood follow-up seed `{seed_id}` references unknown family/surface `{family_or_surface}`",
                line_idx + 1
            ));
        }

        let label = markdown_code_cell_value(columns[4]);
        if !DOGFOOD_TRIAGE_LABELS.contains(&label.as_str()) {
            return Err(format!(
                "{path}:{} unknown dogfood follow-up label `{label}`",
                line_idx + 1
            ));
        }

        let report = markdown_report_link(columns[5]).ok_or_else(|| {
            format!(
                "{path}:{} dogfood follow-up seed `{seed_id}` source report must link a report",
                line_idx + 1
            )
        })?;
        let Some(report_name) = report.strip_prefix("reports/") else {
            return Err(format!(
                "{path}:{} dogfood follow-up seed `{seed_id}` source report must link under reports/",
                line_idx + 1
            ));
        };
        if !report_set.contains(report_name) {
            return Err(format!(
                "{path}:{} dogfood follow-up seed `{seed_id}` links missing report `{report}`",
                line_idx + 1
            ));
        }
        let triage_keys = report_triage_keys.get(report_name).ok_or_else(|| {
            format!(
                "{path}:{} dogfood follow-up seed `{seed_id}` links report `{report}` without parsed triage keys",
                line_idx + 1
            )
        })?;
        if !triage_keys.contains(&(target.clone(), label.clone())) {
            return Err(format!(
                "{path}:{} dogfood follow-up seed `{seed_id}` source report `{report}` must include a triage row for target `{target}` with primary label `{label}`",
                line_idx + 1
            ));
        }

        let notes = markdown_code_cell_value(columns[7]);
        if status == "open" {
            check_open_dogfood_follow_up_notes(path, line_idx + 1, &seed_id, &notes)?;
        } else if status == "parked" {
            check_parked_dogfood_follow_up_notes(path, line_idx + 1, &seed_id, &notes)?;
        } else if status == "done" {
            check_done_dogfood_follow_up_notes(path, line_idx + 1, &seed_id, &notes)?;
        } else if status == "superseded" {
            check_superseded_dogfood_follow_up_notes(path, line_idx + 1, &seed_id, &notes)?;
        }

        rows += 1;
    }
    if !in_table {
        return Err(format!(
            "{path} must include a dogfood follow-up seed table"
        ));
    }
    if rows == 0 {
        return Err(format!("{path} has a dogfood follow-up table with no rows"));
    }
    Ok(rows)
}

fn check_dogfood_follow_up_next_pr_slice(
    path: &str,
    line: usize,
    seed_id: &str,
    next_pr_slice: &str,
) -> Result<(), String> {
    let lower = next_pr_slice.to_ascii_lowercase();
    for forbidden in [
        "omnibus",
        "broad recognizer",
        "broad analyzer",
        "all families",
        "multiple families",
        "catch-all",
        "blanket",
    ] {
        if lower.contains(forbidden) {
            return Err(format!(
                "{path}:{line} dogfood follow-up seed `{seed_id}` next PR slice must stay narrow; found `{forbidden}`"
            ));
        }
    }
    Ok(())
}

fn check_open_dogfood_follow_up_notes(
    path: &str,
    line: usize,
    seed_id: &str,
    notes: &str,
) -> Result<(), String> {
    let lower = notes.to_ascii_lowercase();
    let explains_ready_slice = [
        "ready",
        "narrow",
        "fixture",
        "control",
        "verifier",
        "concrete",
        "actionable",
        "report-backed",
    ]
    .iter()
    .any(|needle| lower.contains(needle));
    if !explains_ready_slice {
        return Err(format!(
            "{path}:{line} open dogfood follow-up seed `{seed_id}` notes must explain why it is ready for a narrow fixture, verifier, or report-backed PR"
        ));
    }
    Ok(())
}

fn check_parked_dogfood_follow_up_notes(
    path: &str,
    line: usize,
    seed_id: &str,
    notes: &str,
) -> Result<(), String> {
    let lower = notes.to_ascii_lowercase();
    let explains_future_pressure = [
        "future",
        "not enough evidence",
        "without a matching real card",
        "until real dogfood",
        "separate evidence",
        "does not exercise",
        "only when dogfood exposes",
    ]
    .iter()
    .any(|needle| lower.contains(needle));
    if !explains_future_pressure {
        return Err(format!(
            "{path}:{line} parked dogfood follow-up seed `{seed_id}` notes must explain why it is future pressure rather than ready implementation work"
        ));
    }
    Ok(())
}

fn check_done_dogfood_follow_up_notes(
    path: &str,
    line: usize,
    seed_id: &str,
    notes: &str,
) -> Result<(), String> {
    let lower = notes.to_ascii_lowercase();
    let explains_done_coverage = [
        "covered",
        "landed",
        "fixture",
        "verifier",
        "report",
        "excluded",
        "preserve",
        "regression pressure",
    ]
    .iter()
    .any(|needle| lower.contains(needle));
    if !explains_done_coverage {
        return Err(format!(
            "{path}:{line} done dogfood follow-up seed `{seed_id}` notes must explain what landed, covers, or preserves the follow-up"
        ));
    }
    Ok(())
}

fn check_superseded_dogfood_follow_up_notes(
    path: &str,
    line: usize,
    seed_id: &str,
    notes: &str,
) -> Result<(), String> {
    let lower = notes.to_ascii_lowercase();
    let explains_replacement = [
        "superseded by",
        "replaced by",
        "newer seed",
        "newer report",
        "merged into",
    ]
    .iter()
    .any(|needle| lower.contains(needle));
    if !explains_replacement {
        return Err(format!(
            "{path}:{line} superseded dogfood follow-up seed `{seed_id}` notes must name the newer seed or report that replaces it"
        ));
    }
    Ok(())
}

fn check_dogfood_follow_up_status_glossary(path: &str, text: &str) -> Result<(), String> {
    let mut in_statuses = false;
    let mut documented = BTreeSet::new();
    for (line_idx, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed == "Statuses:" {
            in_statuses = true;
            continue;
        }
        if in_statuses && trimmed.starts_with("## ") {
            break;
        }
        if !in_statuses || !trimmed.starts_with("- `") {
            continue;
        }
        let status = first_markdown_code_span(trimmed).ok_or_else(|| {
            format!(
                "{path}:{} dogfood follow-up status bullet must start with a code-spanned status",
                line_idx + 1
            )
        })?;
        if !DOGFOOD_FOLLOW_UP_STATUSES.contains(&status.as_str()) {
            return Err(format!(
                "{path}:{} documents unknown dogfood follow-up status `{status}`",
                line_idx + 1
            ));
        }
        if !documented.insert(status.clone()) {
            return Err(format!(
                "{path}:{} documents duplicate dogfood follow-up status `{status}`",
                line_idx + 1
            ));
        }
    }
    if !in_statuses {
        return Err(format!(
            "{path} must document dogfood follow-up statuses before the seed table"
        ));
    }
    for status in DOGFOOD_FOLLOW_UP_STATUSES {
        if !documented.contains(*status) {
            return Err(format!(
                "{path} must document dogfood follow-up status `{status}`"
            ));
        }
    }
    Ok(())
}

fn first_markdown_code_span(text: &str) -> Option<String> {
    let start = text.find('`')?;
    let rest = &text[start + 1..];
    let end = rest.find('`')?;
    Some(rest[..end].to_string())
}

fn markdown_report_link(cell: &str) -> Option<String> {
    if let Some(start) = cell.find("](") {
        let after_open = &cell[start + 2..];
        let end = after_open.find(')')?;
        return Some(after_open[..end].to_string());
    }
    let value = markdown_code_cell_value(cell);
    (!value.is_empty()).then_some(value)
}

fn dogfood_report_triage_keys_by_report(
    reports: &[String],
) -> Result<BTreeMap<String, BTreeSet<(String, String)>>, String> {
    let mut keys_by_report = BTreeMap::new();
    for report in reports {
        let path = workspace_path(DOGFOOD_REPORT_DIR).join(report);
        let report_path = path.to_string_lossy().replace('\\', "/");
        let text = read_to_string(&path)?;
        keys_by_report.insert(
            report.clone(),
            dogfood_report_triage_keys_text(&report_path, &text)?
                .into_iter()
                .collect(),
        );
    }
    Ok(keys_by_report)
}

fn check_dogfood_report_triage_labels() -> Result<(), String> {
    let report_dir = workspace_path(DOGFOOD_REPORT_DIR);
    if !report_dir.is_dir() {
        return Err(format!("{DOGFOOD_REPORT_DIR} is missing"));
    }
    for entry in fs::read_dir(&report_dir)
        .map_err(|err| format!("read {DOGFOOD_REPORT_DIR} failed: {err}"))?
    {
        let entry =
            entry.map_err(|err| format!("read {DOGFOOD_REPORT_DIR} entry failed: {err}"))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        let report_path = path.to_string_lossy().replace('\\', "/");
        let text = read_to_string(&path)?;
        check_dogfood_report_triage_labels_text(&report_path, &text)?;
    }
    Ok(())
}

fn check_dogfood_report_triage_labels_text(path: &str, text: &str) -> Result<usize, String> {
    Ok(dogfood_report_triage_keys_text(path, text)?.len())
}

fn dogfood_report_triage_keys_text(
    path: &str,
    text: &str,
) -> Result<Vec<(String, String)>, String> {
    let mut in_triage_table = false;
    let mut rows = Vec::new();
    for (line_idx, line) in text.lines().enumerate() {
        if !in_triage_table {
            if line.contains("| Primary label |") {
                let columns = line
                    .trim()
                    .trim_matches('|')
                    .split('|')
                    .map(str::trim)
                    .collect::<Vec<_>>();
                if columns != DOGFOOD_TRIAGE_HEADER {
                    return Err(format!(
                        "{path}:{} dogfood triage header must be `{}`",
                        line_idx + 1,
                        DOGFOOD_TRIAGE_HEADER.join(" | ")
                    ));
                }
                in_triage_table = true;
            }
            continue;
        }
        if !line.trim_start().starts_with('|') {
            break;
        }
        if line.contains("|---") {
            continue;
        }
        let columns = line
            .trim()
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect::<Vec<_>>();
        if columns.len() != 5 {
            return Err(format!(
                "{path}:{} dogfood triage row must include Target, Card or family, Primary label, Evidence, and Follow-up columns",
                line_idx + 1
            ));
        }
        for (column_idx, column_name) in DOGFOOD_TRIAGE_HEADER.iter().enumerate() {
            if markdown_code_cell_value(columns[column_idx]).is_empty() {
                return Err(format!(
                    "{path}:{} dogfood triage row must include a non-empty {column_name} column",
                    line_idx + 1
                ));
            }
        }
        let label = markdown_code_cell_value(columns[2]);
        if !DOGFOOD_TRIAGE_LABELS.contains(&label.as_str()) {
            return Err(format!(
                "{path}:{} unknown dogfood triage label `{label}`",
                line_idx + 1
            ));
        }
        let target = markdown_code_cell_value(columns[0]);
        rows.push((target, label));
    }
    if in_triage_table && rows.is_empty() {
        return Err(format!("{path} has a dogfood triage table with no rows"));
    }
    Ok(rows)
}

fn markdown_code_cell_value(cell: &str) -> String {
    cell.trim().trim_matches('`').trim().to_string()
}

mod dogfood_checks {
    use super::*;

    pub(super) struct TargetStats {
        pub(super) repository: Option<String>,
        pub(super) artifact_status: String,
        pub(super) repo_snapshots: usize,
        pub(super) pr_diffs: usize,
        pub(super) fixture_controls: usize,
        pub(super) fixture_control_id: Option<String>,
    }

    pub(super) fn validate_target(
        target: &toml::Value,
        idx: usize,
        ids: &mut BTreeSet<String>,
    ) -> Result<TargetStats, String> {
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
        let kind = required_target_string(target, "kind", idx)?;
        if !DOGFOOD_TARGET_KINDS.contains(&kind) {
            return Err(format!(
                "{DOGFOOD_MANIFEST} targets[{idx}] uses unknown kind `{kind}`"
            ));
        }
        let repository = if kind == "fixture-control" {
            None
        } else {
            let repository = required_target_string(target, "repository", idx)?;
            if !repository.contains('/') {
                return Err(format!(
                    "{DOGFOOD_MANIFEST} targets[{idx}] repository `{repository}` must be owner/repo"
                ));
            }
            Some(repository.to_string())
        };
        required_target_string(target, "crate", idx)?;
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
        if !command_matches_dogfood_target_kind(command, kind) {
            return Err(format!(
                "{DOGFOOD_MANIFEST} targets[{idx}] command must run unsafe-review JSON output or the manual-candidate example smoke"
            ));
        }
        let artifact_status = required_target_string(target, "artifact_status", idx)?;
        if !DOGFOOD_ARTIFACT_STATUSES.contains(&artifact_status) {
            return Err(format!(
                "{DOGFOOD_MANIFEST} targets[{idx}] uses unknown artifact_status `{artifact_status}`"
            ));
        }
        validate_artifacts(target, idx, artifact_status)?;
        let (repo_snapshots, pr_diffs, fixture_controls) = validate_kind_fields(target, idx, kind)?;
        let fixture_control_id = (fixture_controls > 0).then(|| id.to_string());
        Ok(TargetStats {
            repository,
            artifact_status: artifact_status.to_string(),
            repo_snapshots,
            pr_diffs,
            fixture_controls,
            fixture_control_id,
        })
    }

    fn command_matches_dogfood_target_kind(command: &str, kind: &str) -> bool {
        let unsafe_review_json =
            command.contains("unsafe-review") && command.contains("--format json");
        if unsafe_review_json {
            return true;
        }
        kind == "fixture-control"
            && command.contains("xtask")
            && command.contains("check-manual-candidate-examples")
    }

    fn validate_artifacts(
        target: &toml::Table,
        idx: usize,
        artifact_status: &str,
    ) -> Result<(), String> {
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
        Ok(())
    }

    fn validate_kind_fields(
        target: &toml::Table,
        idx: usize,
        kind: &str,
    ) -> Result<(usize, usize, usize), String> {
        match kind {
            "repo-snapshot" => {
                let commit = required_target_string(target, "commit", idx)?;
                if commit.len() != 40 || !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                    return Err(format!(
                        "{DOGFOOD_MANIFEST} targets[{idx}] commit must be a full 40-character hex SHA"
                    ));
                }
                let root = required_target_string(target, "root", idx)?;
                check_dogfood_path(root, idx, "root")?;
                Ok((1, 0, 0))
            }
            "pr-diff" => {
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
                Ok((0, 1, 0))
            }
            "fixture-control" => {
                let fixture = required_target_string(target, "fixture", idx)?;
                check_dogfood_path(fixture, idx, "fixture")?;
                if !fixture.starts_with("fixtures/") {
                    return Err(format!(
                        "{DOGFOOD_MANIFEST} targets[{idx}] fixture-control fixture must live under fixtures/"
                    ));
                }
                let root = required_target_string(target, "root", idx)?;
                check_dogfood_path(root, idx, "root")?;
                let diff = required_target_string(target, "diff", idx)?;
                check_dogfood_path(diff, idx, "diff")?;
                Ok((0, 0, 1))
            }
            _ => Err(format!(
                "{DOGFOOD_MANIFEST} targets[{idx}] uses unsupported kind `{kind}`"
            )),
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "the dogfood manifest/index cross-check uses independent counts from separate sources"
)]
fn check_dogfood_index(
    target_count: usize,
    repository_count: usize,
    repo_snapshots: usize,
    pr_diffs: usize,
    fixture_controls: usize,
    repositories: &BTreeSet<String>,
    fixture_control_ids: &BTreeSet<String>,
    artifact_status_counts: &BTreeMap<String, usize>,
) -> Result<(), String> {
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
        repository_count,
        DOGFOOD_INDEX,
    )?;
    require_json_usize_at(&index, "/summary/targets", target_count, DOGFOOD_INDEX)?;
    require_json_usize_at(
        &index,
        "/summary/repo_snapshots",
        repo_snapshots,
        DOGFOOD_INDEX,
    )?;
    require_json_usize_at(&index, "/summary/pr_diffs", pr_diffs, DOGFOOD_INDEX)?;
    require_json_usize_at(
        &index,
        "/summary/fixture_controls",
        fixture_controls,
        DOGFOOD_INDEX,
    )?;

    for (status, count) in artifact_status_counts {
        require_json_usize_at(
            &index,
            &format!("/summary/artifact_statuses/{status}"),
            *count,
            DOGFOOD_INDEX,
        )?;
    }

    let repository_rows = json_array_at(&index, "/repositories", DOGFOOD_INDEX)?;
    if repository_rows.len() != repository_count {
        return Err(format!(
            "{DOGFOOD_INDEX} repositories has {}, expected {repository_count}",
            repository_rows.len()
        ));
    }
    let mut seen = BTreeSet::new();
    for (idx, row) in repository_rows.iter().enumerate() {
        let Some(repository) = row.get("repository").and_then(serde_json::Value::as_str) else {
            return Err(format!(
                "{DOGFOOD_INDEX} repositories[{idx}] is missing repository"
            ));
        };
        if !repositories.contains(repository) {
            return Err(format!(
                "{DOGFOOD_INDEX} repositories[{idx}] references unknown repository `{repository}`"
            ));
        }
        if !seen.insert(repository.to_string()) {
            return Err(format!(
                "{DOGFOOD_INDEX} repositories contains duplicate `{repository}`"
            ));
        }
        json_array_at(row, "/snapshot_targets", DOGFOOD_INDEX)?;
        json_array_at(row, "/pr_diff_targets", DOGFOOD_INDEX)?;
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

    let control_rows = json_array_at(&index, "/control_targets", DOGFOOD_INDEX)?;
    if control_rows.len() != fixture_controls {
        return Err(format!(
            "{DOGFOOD_INDEX} control_targets has {}, expected {fixture_controls}",
            control_rows.len()
        ));
    }
    let mut seen_controls = BTreeSet::new();
    for (idx, row) in control_rows.iter().enumerate() {
        let Some(id) = row.get("id").and_then(serde_json::Value::as_str) else {
            return Err(format!(
                "{DOGFOOD_INDEX} control_targets[{idx}] is missing id"
            ));
        };
        if !fixture_control_ids.contains(id) {
            return Err(format!(
                "{DOGFOOD_INDEX} control_targets[{idx}] references unknown fixture-control target `{id}`"
            ));
        }
        if !seen_controls.insert(id.to_string()) {
            return Err(format!(
                "{DOGFOOD_INDEX} control_targets contains duplicate `{id}`"
            ));
        }
        let Some(fixture) = row.get("fixture").and_then(serde_json::Value::as_str) else {
            return Err(format!(
                "{DOGFOOD_INDEX} control_targets[{idx}] is missing fixture"
            ));
        };
        if !fixture.starts_with("fixtures/") {
            return Err(format!(
                "{DOGFOOD_INDEX} control_targets[{idx}] fixture `{fixture}` must live under fixtures/"
            ));
        }
        let Some(summary) = row
            .get("primary_exercise")
            .and_then(serde_json::Value::as_str)
        else {
            return Err(format!(
                "{DOGFOOD_INDEX} control_targets[{idx}] is missing primary_exercise"
            ));
        };
        if summary.len() < 24 {
            return Err(format!(
                "{DOGFOOD_INDEX} control_targets[{idx}] primary_exercise is too terse"
            ));
        }
        let Some(report) = row.get("report").and_then(serde_json::Value::as_str) else {
            return Err(format!(
                "{DOGFOOD_INDEX} control_targets[{idx}] is missing report"
            ));
        };
        require_file(report)?;
        let report_text = read_to_string(&workspace_path(report))?;
        let report_keys = dogfood_report_triage_keys_text(report, &report_text)?;
        if !report_keys.iter().any(|(target, _label)| target == id) {
            return Err(format!(
                "{DOGFOOD_INDEX} control_targets[{idx}] report `{report}` must include a triage row for target `{id}`"
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
    let corpus_dir = repo_path("fuzz/corpus/analyze");
    let corpus_entries = fs::read_dir(&corpus_dir)
        .map_err(|err| format!("failed to read {}: {err}", corpus_dir.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("failed to enumerate {}: {err}", corpus_dir.display()))?;
    if corpus_entries.is_empty() {
        return Err("fuzz/corpus/analyze must include at least one corpus seed".to_string());
    }
    let mut has_diff_marker_seed = false;
    for entry in corpus_entries {
        let seed_path = entry.path();
        if !seed_path.is_file() {
            continue;
        }
        let seed = fs::read_to_string(&seed_path)
            .map_err(|err| format!("failed to read {}: {err}", seed_path.display()))?;
        if seed.contains("---DIFF---") {
            has_diff_marker_seed = true;
            break;
        }
    }
    if !has_diff_marker_seed {
        return Err(
            "fuzz/corpus/analyze must include at least one seed containing `---DIFF---`"
                .to_string(),
        );
    }

    println!("check-fuzz: ok");
    Ok(())
}

fn reject_positive_overclaims(path: &Path, text: &str) -> Result<(), String> {
    let mut previous = String::new();
    for (line_no, line) in text.lines().enumerate() {
        let lower = normalize_claim_line(line);
        let context = format!("{previous} {lower}");
        for forbidden in ["all clear", "safe to merge"] {
            if lower.contains(forbidden) {
                return Err(format!(
                    "{}:{} must not imply `{forbidden}`",
                    path.display(),
                    line_no + 1
                ));
            }
        }
        for forbidden in [
            "proved safe",
            "proven safe",
            "verified safe",
            "verified sound",
            "proved sound",
            "proven sound",
            "proved memory safety",
            "proven memory safety",
            "proof of safety",
            "safety verified",
            "soundness verified",
            "blocking-ready",
            "calibrated precision",
            "calibrated recall",
        ] {
            if lower.contains(forbidden) && !has_negative_claim_context(&context) {
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
            && !has_negative_claim_context(&context)
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
            && !has_negative_claim_context(&context)
        {
            return Err(format!(
                "{}:{} must not imply UB-free status",
                path.display(),
                line_no + 1
            ));
        }
        if lower.contains("site reached") && !has_negative_claim_context(&context) {
            return Err(format!(
                "{}:{} must not imply site execution",
                path.display(),
                line_no + 1
            ));
        }
        previous = lower;
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
        let Some(cards) = expected_cards.as_array() else {
            return Err(format!(
                "{}/expected.cards.json must contain a JSON array of cards",
                dir.display()
            ));
        };
        check_fixture_card_identities(dir, name, cards)?;
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

fn check_fixture_card_identities(
    dir: &Path,
    fixture: &str,
    cards: &[serde_json::Value],
) -> Result<(), String> {
    let path = format!("{}/expected.cards.json", dir.display());
    for (idx, card) in cards.iter().enumerate() {
        check_fixture_card_identity(&path, idx, fixture, card)?;
    }
    Ok(())
}

fn check_fixture_card_identity(
    path: &str,
    idx: usize,
    fixture: &str,
    card: &serde_json::Value,
) -> Result<(), String> {
    let id = require_non_empty_json_str(card, "id", &format!("{path} card[{idx}]"))?;
    if !id.starts_with("UR-") {
        return Err(format!(
            "{path} card[{idx}] id `{id}` must start with `UR-`"
        ));
    }
    if !has_reviewcard_count_suffix(id) {
        return Err(format!(
            "{path} card[{idx}] id `{id}` must end with a counted identity suffix like `-c1`"
        ));
    }
    if !contains_hex_run(id, 12) {
        return Err(format!(
            "{path} card[{idx}] id `{id}` must include a normalized snippet hash"
        ));
    }

    let site = card
        .get("site")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| format!("{path} card[{idx}] is missing object key `site`"))?;
    check_fixture_site_metadata(path, idx, card, site)?;
    let operation_family =
        require_non_empty_json_str(card, "operation_family", &format!("{path} card[{idx}]"))?;
    check_fixture_operation_family(path, idx, operation_family)?;
    let operation_path = fixture_card_operation_path(card, site, operation_family, path, idx)?;
    for (field, token) in [
        ("fixture", fixture),
        (
            "site.file",
            required_json_object_str(site, "file", path, idx)?,
        ),
        (
            "site.owner",
            required_json_object_str(site, "owner", path, idx)?,
        ),
        (
            "site.kind",
            required_json_object_str(site, "kind", path, idx)?,
        ),
        ("operation_family", operation_family),
        ("operation_path", operation_path.as_str()),
    ] {
        require_identity_token(id, token, path, idx, field)?;
    }

    check_fixture_hazards(path, idx, card, id, operation_family)?;

    check_fixture_card_classification(path, idx, card)?;
    let has_missing_evidence =
        check_fixture_obligation_evidence(path, idx, card, operation_family)?;
    check_fixture_top_level_evidence_summaries(path, idx, card)?;
    check_fixture_reach_evidence(path, idx, card, site)?;
    check_fixture_missing_summary(path, idx, card, has_missing_evidence)?;
    check_fixture_next_action(path, idx, card, operation_family)?;
    check_fixture_witness_routes(path, idx, card, operation_family)?;

    Ok(())
}

fn check_fixture_operation_family(
    path: &str,
    idx: usize,
    operation_family: &str,
) -> Result<(), String> {
    if !fixture_known_operation_family(operation_family) {
        return Err(format!(
            "{path} card[{idx}] operation_family `{operation_family}` must be a known OperationFamily string"
        ));
    }
    Ok(())
}

fn fixture_known_operation_family(operation_family: &str) -> bool {
    matches!(
        operation_family,
        "raw_pointer_deref"
            | "raw_pointer_read"
            | "raw_pointer_read_unaligned"
            | "raw_pointer_write"
            | "raw_pointer_write_unaligned"
            | "pointer_arithmetic"
            | "ptr_copy"
            | "ptr_replace"
            | "copy_nonoverlapping"
            | "slice_from_raw_parts"
            | "vec_from_raw_parts"
            | "str_from_utf8_unchecked"
            | "maybe_uninit_assume_init"
            | "vec_set_len"
            | "transmute"
            | "zeroed"
            | "drop_in_place"
            | "atomic_pointer_state"
            | "unwrap_unchecked"
            | "unreachable_unchecked"
            | "unsafe_fn_call"
            | "box_from_raw"
            | "nonnull_unchecked"
            | "pin_unchecked"
            | "get_unchecked"
            | "unsafe_impl_send_sync"
            | "ffi"
            | "static_mut"
            | "inline_asm"
            | "target_feature"
            | "unknown"
    )
}

fn check_fixture_hazards(
    path: &str,
    idx: usize,
    card: &serde_json::Value,
    id: &str,
    operation_family: &str,
) -> Result<(), String> {
    let hazards = json_array_at(card, "/hazards", &format!("{path} card[{idx}]"))?;
    if hazards.is_empty() {
        return Err(format!("{path} card[{idx}] hazards must not be empty"));
    }
    let registry_hazards = operation_family_registry_hazards()?;
    let allowed_hazards = registry_hazards.get(operation_family).ok_or_else(|| {
        format!(
            "{path} card[{idx}] operation_family `{operation_family}` must have a hazard row in {OPERATION_FAMILY_REGISTRY}"
        )
    })?;
    let mut seen = BTreeSet::new();
    let mut has_hazard_token = false;
    for (hazard_idx, hazard) in hazards.iter().enumerate() {
        let Some(hazard) = hazard.as_str() else {
            return Err(format!(
                "{path} card[{idx}] hazards[{hazard_idx}] must be a string"
            ));
        };
        if hazard.trim().is_empty() {
            return Err(format!(
                "{path} card[{idx}] hazards[{hazard_idx}] must not be empty"
            ));
        }
        if !fixture_known_hazard(hazard) {
            return Err(format!(
                "{path} card[{idx}] hazards[{hazard_idx}] `{hazard}` must be a known HazardKind string"
            ));
        }
        if !allowed_hazards.contains(hazard) {
            return Err(format!(
                "{path} card[{idx}] hazards[{hazard_idx}] `{hazard}` is not listed for operation_family `{operation_family}` in {OPERATION_FAMILY_REGISTRY}"
            ));
        }
        if !seen.insert(hazard) {
            return Err(format!(
                "{path} card[{idx}] hazards must not duplicate `{hazard}`"
            ));
        }
        if identity_contains_token(id, hazard) {
            has_hazard_token = true;
        }
    }
    if !has_hazard_token {
        return Err(format!(
            "{path} card[{idx}] id `{id}` must include at least one hazard token"
        ));
    }

    Ok(())
}

fn fixture_known_hazard(hazard: &str) -> bool {
    matches!(
        hazard,
        "pointer_validity"
            | "alignment"
            | "same_allocation"
            | "bounds"
            | "initialized_memory"
            | "invalid_value"
            | "aliasing_or_provenance"
            | "panic_safety"
            | "drop_or_deallocation"
            | "ffi_abi"
            | "ffi_ownership"
            | "send_sync_invariant"
            | "pin_invariant"
            | "atomic_ordering"
            | "layout_or_repr"
            | "static_mut_global_state"
            | "target_feature"
            | "inline_asm"
            | "leak_or_ownership_transfer"
            | "unknown"
    )
}

fn check_fixture_site_metadata(
    path: &str,
    idx: usize,
    card: &serde_json::Value,
    site: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    let card_context = format!("{path} card[{idx}]");
    let file = required_json_object_str(site, "file", path, idx)?;
    if !file.ends_with(".rs")
        || file.starts_with('/')
        || file.contains('\\')
        || file.split('/').any(|part| part == ".." || part.is_empty())
    {
        return Err(format!(
            "{card_context} site.file `{file}` must be a relative Rust source path"
        ));
    }

    let line = required_json_object_u64(site, "line", path, idx)?;
    if line == 0 {
        return Err(format!("{card_context} site.line must be positive"));
    }
    let column = required_json_object_u64(site, "column", path, idx)?;
    if column == 0 {
        return Err(format!("{card_context} site.column must be positive"));
    }

    let kind = required_json_object_str(site, "kind", path, idx)?;
    if !fixture_known_site_kind(kind) {
        return Err(format!(
            "{card_context} site.kind `{kind}` must be a known UnsafeSiteKind string"
        ));
    }

    let visibility = required_json_object_str(site, "visibility", path, idx)?;
    if !matches!(visibility, "public" | "private") {
        return Err(format!(
            "{card_context} site.visibility `{visibility}` must be `public` or `private`"
        ));
    }

    let Some(public_api_surface) = site
        .get("public_api_surface")
        .and_then(serde_json::Value::as_bool)
    else {
        return Err(format!(
            "{card_context} site is missing boolean key `public_api_surface`"
        ));
    };
    if public_api_surface && visibility != "public" {
        return Err(format!(
            "{card_context} public_api_surface requires site.visibility `public`"
        ));
    }

    let snippet = required_json_object_str(site, "snippet", path, idx)?;
    if snippet.contains('\n') || snippet.contains('\r') {
        return Err(format!(
            "{card_context} site.snippet must be a single-line review snippet"
        ));
    }

    let operation = require_non_empty_json_str(card, "operation", &card_context)?;
    if operation != snippet && !is_fixture_operation_snippet_exception(path, operation) {
        return Err(format!(
            "{card_context} operation must match site.snippet so card projections share one operation expression"
        ));
    }

    Ok(())
}

fn is_fixture_operation_snippet_exception(path: &str, operation: &str) -> bool {
    path.replace('\\', "/")
        .contains("fixtures/js_buffer_reentry_")
        && operation.starts_with("JS-backed buffer descriptor captured before possible JS reentry")
}

fn fixture_known_site_kind(kind: &str) -> bool {
    matches!(
        kind,
        "unsafe_block"
            | "unsafe_fn"
            | "unsafe_trait"
            | "unsafe_impl"
            | "unsafe_impl_send"
            | "unsafe_impl_sync"
            | "extern_block"
            | "ffi_call"
            | "static_mut"
            | "operation"
    )
}

fn check_fixture_card_classification(
    path: &str,
    idx: usize,
    card: &serde_json::Value,
) -> Result<(), String> {
    let card_context = format!("{path} card[{idx}]");
    let class_name = require_non_empty_json_str(card, "class", &card_context)?;
    let priority = require_non_empty_json_str(card, "priority", &card_context)?;
    let confidence = require_non_empty_json_str(card, "confidence", &card_context)?;

    if !fixture_known_review_class(class_name) {
        return Err(format!(
            "{card_context} class `{class_name}` must be a known ReviewClass string"
        ));
    }
    if !matches!(priority, "high" | "medium" | "low") {
        return Err(format!(
            "{card_context} priority `{priority}` must be `high`, `medium`, or `low`"
        ));
    }
    if !matches!(confidence, "high" | "medium" | "low" | "unknown") {
        return Err(format!(
            "{card_context} confidence `{confidence}` must be `high`, `medium`, `low`, or `unknown`"
        ));
    }

    let Some((expected_priority, expected_confidence)) =
        fixture_expected_classification_signal(class_name)
    else {
        return Ok(());
    };
    if priority != expected_priority || confidence != expected_confidence {
        return Err(format!(
            "{card_context} class `{class_name}` must use priority `{expected_priority}` and confidence `{expected_confidence}`, got priority `{priority}` and confidence `{confidence}`"
        ));
    }

    Ok(())
}

fn fixture_known_review_class(class_name: &str) -> bool {
    matches!(
        class_name,
        "guarded_and_witnessed"
            | "guarded_unwitnessed"
            | "contract_missing"
            | "guard_missing"
            | "reachable_unwitnessed"
            | "unsafe_unreached"
            | "witness_mismatch"
            | "requires_loom"
            | "requires_sanitizer"
            | "requires_kani_or_crux"
            | "miri_unsupported"
            | "static_unknown"
            | "baseline_known"
            | "suppressed"
    )
}

fn fixture_expected_classification_signal(
    class_name: &str,
) -> Option<(&'static str, &'static str)> {
    match class_name {
        "contract_missing" => Some(("high", "high")),
        "guard_missing" | "requires_loom" => Some(("high", "medium")),
        "guarded_unwitnessed" | "miri_unsupported" | "unsafe_unreached" => {
            Some(("medium", "medium"))
        }
        _ => None,
    }
}

fn check_fixture_next_action(
    path: &str,
    idx: usize,
    card: &serde_json::Value,
    operation_family: &str,
) -> Result<(), String> {
    let card_context = format!("{path} card[{idx}]");
    let action = require_non_empty_json_str(card, "next_action", &card_context)?;
    let normalized = normalize_claim_line(action);
    let starts_with_action = [
        "add ",
        "attach ",
        "use ",
        "run ",
        "review ",
        "document ",
        "mark ",
    ]
    .iter()
    .any(|prefix| normalized.starts_with(prefix));
    if !starts_with_action {
        return Err(format!(
            "{card_context} next_action must start with a concrete reviewer action verb"
        ));
    }

    for forbidden in [
        "all clear",
        "safe to merge",
        "proved safe",
        "proven safe",
        "verified safe",
        "proof of safety",
        "no action needed",
        "nothing to do",
    ] {
        if normalized.contains(forbidden) {
            return Err(format!(
                "{card_context} next_action must not imply `{forbidden}`"
            ));
        }
    }

    if (normalized.contains("miri-clean") || normalized.contains("miri clean"))
        && !has_negative_claim_context(&normalized)
    {
        return Err(format!(
            "{card_context} next_action must not imply Miri-clean status"
        ));
    }
    if normalized.contains("ub-free") && !has_negative_claim_context(&normalized) {
        return Err(format!(
            "{card_context} next_action must not imply UB-free status"
        ));
    }
    if (normalized.contains("site reached") || normalized.contains("site executed"))
        && !has_negative_claim_context(&normalized)
    {
        return Err(format!(
            "{card_context} next_action must not imply site execution"
        ));
    }

    if normalized.contains("safety obligation") {
        let normalized_family = operation_family.replace('_', "").to_ascii_lowercase();
        if !normalized.contains(&normalized_family) {
            return Err(format!(
                "{card_context} next_action safety-obligation wording must name operation_family `{operation_family}`"
            ));
        }
    }
    if normalized.contains("unknown safety obligation")
        || normalized.contains("unknown obligation")
        || normalized.contains("unknown safety contract")
    {
        return Err(format!(
            "{card_context} next_action must not route reviewers to an unknown obligation; ask for manual unsafe-site review and obligation-specific guard evidence"
        ));
    }
    let has_public_safety_missing = json_array_at(card, "/missing", &card_context)?
        .iter()
        .filter_map(serde_json::Value::as_str)
        .any(|missing| missing.contains("Missing public `# Safety` documentation"));
    if has_public_safety_missing {
        if !normalized.contains("public `# safety`") && !normalized.contains("public # safety") {
            return Err(format!(
                "{card_context} next_action for public unsafe API contract evidence must ask for public `# Safety` documentation"
            ));
        }
        if normalized.contains("`safety:`") || normalized.contains(" safety:") {
            return Err(format!(
                "{card_context} next_action for public unsafe API contract evidence must not suggest a `SAFETY:` comment as a substitute for public `# Safety` documentation"
            ));
        }
    }
    let class_name = require_non_empty_json_str(card, "class", &card_context)?;
    let witness_routes = json_array_at(card, "/witness_routes", &card_context)?;
    let has_human_deep_review_route = witness_routes.iter().any(|route| {
        route
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|kind| kind == "human-deep-review")
    });
    let has_miri_route = witness_routes.iter().any(|route| {
        route
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|kind| kind == "miri")
    });
    let has_cargo_careful_route = witness_routes.iter().any(|route| {
        route
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|kind| kind == "cargo-careful")
    });
    if class_name == "guard_missing" {
        if !normalized.contains("guard") {
            return Err(format!(
                "{card_context} guard_missing next_action must ask for concrete guard evidence"
            ));
        }
        if normalized.contains("# safety")
            || normalized.contains("`safety:`")
            || normalized.contains(" safety:")
            || normalized.contains("comment")
        {
            return Err(format!(
                "{card_context} guard_missing next_action must not suggest documentation or comments as a substitute for guard evidence"
            ));
        }
        if has_human_deep_review_route
            && requires_manual_review_guard_missing_wording(operation_family)
            && (normalized.contains("local guard that discharges")
                || !normalized.contains("manual")
                || !normalized.contains("guard evidence"))
        {
            return Err(format!(
                "{card_context} human-deep-review guard_missing next_action for operation_family `{operation_family}` must name manual review and guard evidence instead of generic local-guard discharge"
            ));
        }
    }
    match class_name {
        "requires_loom" if !normalized.contains("loom") || !normalized.contains("shuttle") => {
            return Err(format!(
                "{card_context} requires_loom next_action must route reviewers to Loom/Shuttle model evidence"
            ));
        }
        "requires_sanitizer"
            if !normalized.contains("sanitizer") || !normalized.contains("receipt") =>
        {
            return Err(format!(
                "{card_context} requires_sanitizer next_action must route reviewers to sanitizer receipt evidence"
            ));
        }
        "requires_kani_or_crux"
            if !normalized.contains("kani")
                || !normalized.contains("crux")
                || !normalized.contains("receipt") =>
        {
            return Err(format!(
                "{card_context} requires_kani_or_crux next_action must route reviewers to Kani/Crux proof receipt evidence"
            ));
        }
        "miri_unsupported"
            if !normalized.contains("sanitizer") || !normalized.contains("cargo-careful") =>
        {
            return Err(format!(
                "{card_context} miri_unsupported next_action must route reviewers to sanitizer/cargo-careful evidence"
            ));
        }
        "miri_unsupported"
            if !normalized.contains("ffi boundary contract")
                || !normalized.contains("miri may not exercise") =>
        {
            return Err(format!(
                "{card_context} miri_unsupported next_action must name the FFI boundary contract and Miri non-execution limitation"
            ));
        }
        "reachable_unwitnessed"
            if !normalized.contains("witness receipt")
                || !normalized.contains("static limitation") =>
        {
            return Err(format!(
                "{card_context} reachable_unwitnessed next_action must ask for a witness receipt or explicit static limitation"
            ));
        }
        "unsafe_unreached"
            if !normalized.contains("test path") || !normalized.contains("safe wrapper") =>
        {
            return Err(format!(
                "{card_context} unsafe_unreached next_action must ask for a focused test path to the safe wrapper"
            ));
        }
        "guarded_unwitnessed"
            if !normalized.contains("witness receipt")
                || !normalized.contains("static limitation") =>
        {
            return Err(format!(
                "{card_context} guarded_unwitnessed next_action must ask for a witness receipt or explicit static limitation"
            ));
        }
        "guarded_unwitnessed"
            if has_human_deep_review_route
                && !normalized.contains("human")
                && !normalized.contains("manual") =>
        {
            return Err(format!(
                "{card_context} guarded_unwitnessed next_action for human-deep-review routes must name human or manual review evidence"
            ));
        }
        "guarded_unwitnessed" if has_miri_route && !normalized.contains("miri") => {
            return Err(format!(
                "{card_context} guarded_unwitnessed next_action for Miri routes must name Miri receipt evidence"
            ));
        }
        "guarded_unwitnessed"
            if has_cargo_careful_route && !normalized.contains("cargo-careful") =>
        {
            return Err(format!(
                "{card_context} guarded_unwitnessed next_action for cargo-careful routes must name cargo-careful receipt evidence"
            ));
        }
        "witness_mismatch"
            if !normalized.contains("witness")
                || !normalized.contains("mismatch")
                || !normalized.contains("matching receipt") =>
        {
            return Err(format!(
                "{card_context} witness_mismatch next_action must ask for a matching receipt after reviewing the mismatch"
            ));
        }
        "static_unknown"
            if !normalized.contains("manual")
                || !normalized.contains("contract")
                || !normalized.contains("witness route") =>
        {
            return Err(format!(
                "{card_context} static_unknown next_action must route reviewers to manual contract and witness-route identification"
            ));
        }
        "baseline_known" if !normalized.contains("baseline") || !normalized.contains("ledger") => {
            return Err(format!(
                "{card_context} baseline_known next_action must keep baseline ledger review evidence current"
            ));
        }
        "suppressed"
            if !normalized.contains("suppressed")
                || !normalized.contains("owner")
                || !normalized.contains("reason") =>
        {
            return Err(format!(
                "{card_context} suppressed next_action must keep suppression owner and reason evidence current"
            ));
        }
        _ => {}
    }

    Ok(())
}

fn requires_manual_review_guard_missing_wording(operation_family: &str) -> bool {
    matches!(
        operation_family,
        "inline_asm" | "pin_unchecked" | "unsafe_fn_call"
    )
}

fn check_fixture_witness_routes(
    path: &str,
    idx: usize,
    card: &serde_json::Value,
    operation_family: &str,
) -> Result<(), String> {
    let card_context = format!("{path} card[{idx}]");
    let routes = json_array_at(card, "/witness_routes", &card_context)?;
    if routes.is_empty() {
        return Err(format!(
            "{card_context} witness_routes must include at least one route"
        ));
    }

    let registry_routes = operation_family_registry_witness_routes()?;
    let allowed_routes = registry_routes.get(operation_family).ok_or_else(|| {
        format!(
            "{card_context} operation_family `{operation_family}` must have a witness route row in {OPERATION_FAMILY_REGISTRY}"
        )
    })?;
    let mut route_keys = BTreeSet::new();
    let mut route_commands = BTreeSet::new();
    for (route_idx, route) in routes.iter().enumerate() {
        let Some(route) = route.as_object() else {
            return Err(format!(
                "{card_context} witness_routes[{route_idx}] must be an object"
            ));
        };
        let kind = required_fixture_route_str(route, "kind", &card_context, route_idx)?;
        if !allowed_routes.contains(kind) {
            return Err(format!(
                "{card_context} witness_routes[{route_idx}] kind `{kind}` is not listed for operation_family `{operation_family}` in {OPERATION_FAMILY_REGISTRY}"
            ));
        }
        let _reason = required_fixture_route_str(route, "reason", &card_context, route_idx)?;
        let Some(required) = route.get("required").and_then(serde_json::Value::as_bool) else {
            return Err(format!(
                "{card_context} witness_routes[{route_idx}] required must be a boolean"
            ));
        };
        if required {
            return Err(format!(
                "{card_context} witness_routes[{route_idx}] required must remain false; unsafe-review routes witnesses but does not require execution by default"
            ));
        }
        let command = optional_fixture_route_command(route, &card_context, route_idx)?;
        check_fixture_route_command_matches_kind(&card_context, route_idx, kind, command)?;
        let route_key = format!("{kind}\0{}", command.unwrap_or(""));
        if !route_keys.insert(route_key) {
            return Err(format!(
                "{card_context} witness_routes must not duplicate kind `{kind}` with the same command"
            ));
        }
        if let Some(command) = command {
            route_commands.insert(command);
        }
    }

    let verify_commands = json_array_at(card, "/verify_commands", &card_context)?;
    let mut verify_command_set = BTreeSet::new();
    for (cmd_idx, command) in verify_commands.iter().enumerate() {
        let Some(command) = command.as_str() else {
            return Err(format!(
                "{card_context} verify_commands[{cmd_idx}] must be a string"
            ));
        };
        if command.trim().is_empty() {
            return Err(format!(
                "{card_context} verify_commands[{cmd_idx}] must not be empty"
            ));
        }
        if !verify_command_set.insert(command) {
            return Err(format!(
                "{card_context} verify_commands must not duplicate `{command}`"
            ));
        }
        if !route_commands.contains(command) {
            return Err(format!(
                "{card_context} verify_commands[{cmd_idx}] `{command}` must be backed by a witness route command"
            ));
        }
    }
    for command in route_commands {
        if !verify_command_set.contains(command) {
            return Err(format!(
                "{card_context} witness route command `{command}` must appear in verify_commands"
            ));
        }
    }

    Ok(())
}

fn required_fixture_route_str<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
    card_context: &str,
    route_idx: usize,
) -> Result<&'a str, String> {
    let Some(value) = object.get(key).and_then(serde_json::Value::as_str) else {
        return Err(format!(
            "{card_context} witness_routes[{route_idx}] is missing string key `{key}`"
        ));
    };
    if value.trim().is_empty() {
        Err(format!(
            "{card_context} witness_routes[{route_idx}] string key `{key}` is empty"
        ))
    } else {
        Ok(value)
    }
}

fn check_fixture_route_command_matches_kind(
    card_context: &str,
    route_idx: usize,
    kind: &str,
    command: Option<&str>,
) -> Result<(), String> {
    let Some(command) = command else {
        if fixture_route_kind_requires_command(kind) {
            return Err(format!(
                "{card_context} witness_routes[{route_idx}] kind `{kind}` must include a matching command"
            ));
        }
        return Ok(());
    };

    let command_matches_kind = match kind {
        "miri" => command_has_token(command, "miri"),
        "cargo-careful" => command_has_token(command, "careful"),
        "asan" => {
            command_contains_ascii(command, "sanitizer=address")
                || command_has_token(command, "asan")
        }
        "msan" => {
            command_contains_ascii(command, "sanitizer=memory")
                || command_has_token(command, "msan")
        }
        "tsan" => {
            command_contains_ascii(command, "sanitizer=thread")
                || command_has_token(command, "tsan")
        }
        "lsan" => {
            command_contains_ascii(command, "sanitizer=leak") || command_has_token(command, "lsan")
        }
        "loom" => command_has_token(command, "loom"),
        "shuttle" => command_has_token(command, "shuttle"),
        "kani" => command_has_token(command, "kani"),
        "crux" => command_has_token(command, "crux"),
        "human-deep-review" | "unsupported" => {
            return Err(format!(
                "{card_context} witness_routes[{route_idx}] kind `{kind}` must not include a command by default"
            ));
        }
        _ => false,
    };

    if !command_matches_kind {
        return Err(format!(
            "{card_context} witness_routes[{route_idx}] kind `{kind}` command `{command}` must name the matching witness tool"
        ));
    }

    Ok(())
}

fn fixture_route_kind_requires_command(kind: &str) -> bool {
    matches!(
        kind,
        "miri" | "cargo-careful" | "asan" | "msan" | "tsan" | "lsan" | "kani" | "crux"
    )
}

fn command_contains_ascii(command: &str, needle: &str) -> bool {
    command.to_ascii_lowercase().contains(needle)
}

fn command_has_token(command: &str, needle: &str) -> bool {
    command
        .to_ascii_lowercase()
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '-')
        .any(|token| token == needle)
}

fn optional_fixture_route_command<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    card_context: &str,
    route_idx: usize,
) -> Result<Option<&'a str>, String> {
    let Some(value) = object.get("command") else {
        return Err(format!(
            "{card_context} witness_routes[{route_idx}] is missing key `command`"
        ));
    };
    if value.is_null() {
        return Ok(None);
    }
    let Some(command) = value.as_str() else {
        return Err(format!(
            "{card_context} witness_routes[{route_idx}] command must be null or a string"
        ));
    };
    if command.trim().is_empty() {
        Err(format!(
            "{card_context} witness_routes[{route_idx}] command must not be empty"
        ))
    } else {
        Ok(Some(command))
    }
}

fn check_fixture_obligation_evidence(
    path: &str,
    idx: usize,
    card: &serde_json::Value,
    operation_family: &str,
) -> Result<bool, String> {
    let card_context = format!("{path} card[{idx}]");
    let obligations = json_array_at(card, "/obligations", &card_context)?;
    if obligations.is_empty() {
        return Err(format!("{card_context} obligations must not be empty"));
    }

    let mut obligation_descriptions = BTreeSet::new();
    for (obligation_idx, obligation) in obligations.iter().enumerate() {
        let Some(description) = obligation.as_str() else {
            return Err(format!(
                "{card_context} obligations[{obligation_idx}] must be a string"
            ));
        };
        if description.trim().is_empty() {
            return Err(format!(
                "{card_context} obligations[{obligation_idx}] must not be empty"
            ));
        }
        if !obligation_descriptions.insert(description) {
            return Err(format!(
                "{card_context} obligations must not duplicate `{description}`"
            ));
        }
    }

    let evidence = json_array_at(card, "/obligation_evidence", &card_context)?;
    if evidence.len() != obligations.len() {
        return Err(format!(
            "{card_context} obligation_evidence count {} must match obligations count {}",
            evidence.len(),
            obligations.len()
        ));
    }

    let registry_obligation_keys = operation_family_registry_obligation_keys()?;
    let allowed_keys = registry_obligation_keys
        .get(operation_family)
        .ok_or_else(|| {
            format!(
                "{card_context} operation_family `{operation_family}` must have an obligation/evidence key row in {OPERATION_FAMILY_REGISTRY}"
            )
        })?;
    let mut evidence_keys = BTreeSet::new();
    let mut evidence_descriptions = BTreeSet::new();
    let mut has_missing_evidence = false;
    for (evidence_idx, entry) in evidence.iter().enumerate() {
        let Some(entry) = entry.as_object() else {
            return Err(format!(
                "{card_context} obligation_evidence[{evidence_idx}] must be an object"
            ));
        };
        let key = required_fixture_evidence_str(entry, "key", &card_context, evidence_idx)?;
        if !allowed_keys.contains(key) {
            return Err(format!(
                "{card_context} obligation_evidence[{evidence_idx}] key `{key}` is not listed for operation_family `{operation_family}` in {OPERATION_FAMILY_REGISTRY}"
            ));
        }
        if !evidence_keys.insert(key) {
            return Err(format!(
                "{card_context} obligation_evidence must not duplicate key `{key}`"
            ));
        }
        let description =
            required_fixture_evidence_str(entry, "description", &card_context, evidence_idx)?;
        if !obligation_descriptions.contains(description) {
            return Err(format!(
                "{card_context} obligation_evidence[{evidence_idx}] description `{description}` must match an obligation"
            ));
        }
        if !evidence_descriptions.insert(description) {
            return Err(format!(
                "{card_context} obligation_evidence must not duplicate description `{description}`"
            ));
        }

        for axis in ["contract", "discharge", "reach", "witness"] {
            if check_fixture_evidence_state(
                &card_context,
                evidence_idx,
                key,
                axis,
                entry.get(axis),
            )? {
                has_missing_evidence = true;
            }
        }
    }

    Ok(has_missing_evidence)
}

fn check_fixture_top_level_evidence_summaries(
    path: &str,
    idx: usize,
    card: &serde_json::Value,
) -> Result<(), String> {
    for axis in ["contract", "witness"] {
        check_fixture_direct_axis_summary(path, idx, card, axis)?;
    }
    check_fixture_discharge_summary(path, idx, card)
}

fn check_fixture_direct_axis_summary(
    path: &str,
    idx: usize,
    card: &serde_json::Value,
    axis: &str,
) -> Result<(), String> {
    let card_context = format!("{path} card[{idx}]");
    let top_level = require_non_empty_json_str(card, axis, &card_context)?;
    let summaries = fixture_obligation_axis_summaries(&card_context, card, axis)?;
    if summaries.len() != 1 {
        return Err(format!(
            "{card_context} top-level {axis} summary must have one matching obligation-level {axis}.summary, found {}",
            summaries.len()
        ));
    }
    let Some(summary) = summaries.iter().next() else {
        return Err(format!(
            "{card_context} top-level {axis} summary must have a matching obligation-level {axis}.summary"
        ));
    };
    if top_level != *summary {
        return Err(format!(
            "{card_context} top-level {axis} `{top_level}` must match obligation-level {axis}.summary `{summary}`"
        ));
    }
    Ok(())
}

fn check_fixture_discharge_summary(
    path: &str,
    idx: usize,
    card: &serde_json::Value,
) -> Result<(), String> {
    let card_context = format!("{path} card[{idx}]");
    let top_level = require_non_empty_json_str(card, "discharge", &card_context)?;
    let states = fixture_obligation_axis_states(&card_context, card, "discharge")?;
    let summaries = fixture_obligation_axis_summaries(&card_context, card, "discharge")?;

    let expected = if states.len() == 1 && states.contains("missing") {
        "No visible local guard detected"
    } else if states.contains("missing") && states.contains("present") {
        "Some inferred safety obligations are missing local guard evidence"
    } else if states.len() == 1 && states.contains("present") {
        if summaries.len() == 1
            && summaries
                .iter()
                .next()
                .is_some_and(|summary| top_level == *summary)
        {
            return Ok(());
        }
        "All inferred safety obligations have visible local discharge evidence"
    } else {
        return Err(format!(
            "{card_context} discharge obligation states must contain `present`, `missing`, or both"
        ));
    };

    if top_level != expected {
        return Err(format!(
            "{card_context} top-level discharge `{top_level}` must summarize obligation-level discharge states as `{expected}`"
        ));
    }
    Ok(())
}

fn fixture_obligation_axis_states<'a>(
    card_context: &str,
    card: &'a serde_json::Value,
    axis: &str,
) -> Result<BTreeSet<&'a str>, String> {
    fixture_obligation_axis_values(card_context, card, axis, "state")
}

fn fixture_obligation_axis_summaries<'a>(
    card_context: &str,
    card: &'a serde_json::Value,
    axis: &str,
) -> Result<BTreeSet<&'a str>, String> {
    fixture_obligation_axis_values(card_context, card, axis, "summary")
}

fn fixture_obligation_axis_values<'a>(
    card_context: &str,
    card: &'a serde_json::Value,
    axis: &str,
    key: &str,
) -> Result<BTreeSet<&'a str>, String> {
    let evidence = json_array_at(card, "/obligation_evidence", card_context)?;
    let mut values = BTreeSet::new();
    for (evidence_idx, entry) in evidence.iter().enumerate() {
        let Some(entry) = entry.as_object() else {
            return Err(format!(
                "{card_context} obligation_evidence[{evidence_idx}] must be an object"
            ));
        };
        let evidence_key = required_fixture_evidence_str(entry, "key", card_context, evidence_idx)?;
        let axis_value = entry
            .get(axis)
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| {
                format!(
                    "{card_context} obligation_evidence[{evidence_idx}] `{evidence_key}` is missing object key `{axis}`"
                )
            })?;
        values.insert(required_fixture_state_str(
            axis_value,
            key,
            card_context,
            evidence_idx,
            evidence_key,
            axis,
        )?);
    }
    Ok(values)
}

fn check_fixture_reach_evidence(
    path: &str,
    idx: usize,
    card: &serde_json::Value,
    site: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    let card_context = format!("{path} card[{idx}]");
    let owner = required_json_object_str(site, "owner", path, idx)?;
    let top_level_reach = require_non_empty_json_str(card, "reach", &card_context)?;
    reject_fixture_reach_overclaim(&card_context, "reach", top_level_reach)?;
    let reach_claim = parse_fixture_reach_claim(top_level_reach).ok_or_else(|| {
        format!(
            "{card_context} reach `{top_level_reach}` must describe static test mentions for the site owner"
        )
    })?;
    if reach_claim.owner != owner {
        return Err(format!(
            "{card_context} reach owner `{}` must match site.owner `{owner}`",
            reach_claim.owner
        ));
    }

    let evidence = json_array_at(card, "/obligation_evidence", &card_context)?;
    for (evidence_idx, entry) in evidence.iter().enumerate() {
        let Some(entry) = entry.as_object() else {
            return Err(format!(
                "{card_context} obligation_evidence[{evidence_idx}] must be an object"
            ));
        };
        let evidence_key =
            required_fixture_evidence_str(entry, "key", &card_context, evidence_idx)?;
        let reach = entry
            .get("reach")
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| {
                format!(
                    "{card_context} obligation_evidence[{evidence_idx}] `{evidence_key}` is missing object key `reach`"
                )
            })?;
        let Some(present) = reach.get("present").and_then(serde_json::Value::as_bool) else {
            return Err(format!(
                "{card_context} obligation_evidence[{evidence_idx}] `{evidence_key}` reach.present must be a boolean"
            ));
        };
        let state_name = required_fixture_state_str(
            reach,
            "state",
            &card_context,
            evidence_idx,
            evidence_key,
            "reach",
        )?;
        let summary = required_fixture_state_str(
            reach,
            "summary",
            &card_context,
            evidence_idx,
            evidence_key,
            "reach",
        )?;
        reject_fixture_reach_overclaim(&card_context, "obligation reach.summary", summary)?;
        if summary != top_level_reach {
            return Err(format!(
                "{card_context} obligation_evidence[{evidence_idx}] `{evidence_key}` reach.summary `{summary}` must match top-level reach `{top_level_reach}`"
            ));
        }

        let (expected_present, expected_state) = match reach_claim.kind {
            FixtureReachClaimKind::RelatedTestMention => (true, "present"),
            FixtureReachClaimKind::NoStaticTestMention => (false, "missing"),
            FixtureReachClaimKind::NoOwnerInferred => (false, "missing"),
        };
        if present != expected_present || state_name != expected_state {
            return Err(format!(
                "{card_context} obligation_evidence[{evidence_idx}] `{evidence_key}` reach state must match top-level reach posture `{top_level_reach}`"
            ));
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct FixtureReachClaim<'a> {
    kind: FixtureReachClaimKind,
    owner: &'a str,
}

#[derive(Debug, Clone, Copy)]
enum FixtureReachClaimKind {
    RelatedTestMention,
    NoStaticTestMention,
    NoOwnerInferred,
}

fn parse_fixture_reach_claim(summary: &str) -> Option<FixtureReachClaim<'_>> {
    if summary == "No owner function could be inferred" {
        return Some(FixtureReachClaim {
            kind: FixtureReachClaimKind::NoOwnerInferred,
            owner: "unknown",
        });
    }

    if let Some((count, owner)) = parse_related_test_reach(summary) {
        if count > 0 {
            return Some(FixtureReachClaim {
                kind: FixtureReachClaimKind::RelatedTestMention,
                owner,
            });
        }
        return None;
    }

    let owner = summary
        .strip_prefix("No static test mention of owner `")?
        .strip_suffix("` was found")?;
    if owner.trim().is_empty() {
        None
    } else {
        Some(FixtureReachClaim {
            kind: FixtureReachClaimKind::NoStaticTestMention,
            owner,
        })
    }
}

fn parse_related_test_reach(summary: &str) -> Option<(usize, &str)> {
    let (count, rest) = summary.split_once(" related test file(s) mention owner `")?;
    let owner = rest.strip_suffix('`')?;
    if owner.trim().is_empty() {
        return None;
    }
    Some((count.parse().ok()?, owner))
}

fn reject_fixture_reach_overclaim(
    card_context: &str,
    field: &str,
    summary: &str,
) -> Result<(), String> {
    let lower = summary.to_ascii_lowercase();
    for term in [
        "site reached",
        "site executed",
        "test covered",
        "execution proof",
        "proves execution",
    ] {
        if lower.contains(term) {
            return Err(format!(
                "{card_context} {field} must describe static test mentions, not `{term}`"
            ));
        }
    }
    Ok(())
}

fn check_fixture_missing_summary(
    path: &str,
    idx: usize,
    card: &serde_json::Value,
    has_missing_evidence: bool,
) -> Result<(), String> {
    let card_context = format!("{path} card[{idx}]");
    let missing = json_array_at(card, "/missing", &card_context)?;
    let mut summaries = BTreeSet::new();
    for (missing_idx, summary) in missing.iter().enumerate() {
        let Some(summary) = summary.as_str() else {
            return Err(format!(
                "{card_context} missing[{missing_idx}] must be a string"
            ));
        };
        if summary.trim().is_empty() {
            return Err(format!(
                "{card_context} missing[{missing_idx}] must not be empty"
            ));
        }
        if !summaries.insert(summary) {
            return Err(format!(
                "{card_context} missing must not duplicate `{summary}`"
            ));
        }
    }

    if has_missing_evidence && missing.is_empty() {
        return Err(format!(
            "{card_context} missing must summarize at least one missing evidence item"
        ));
    }
    if !has_missing_evidence && !missing.is_empty() {
        return Err(format!(
            "{card_context} missing must be empty when all obligation evidence is present"
        ));
    }

    check_fixture_missing_summary_axes(&card_context, card, &summaries)?;

    Ok(())
}

fn check_fixture_missing_summary_axes(
    card_context: &str,
    card: &serde_json::Value,
    summaries: &BTreeSet<&str>,
) -> Result<(), String> {
    check_fixture_missing_summary_axis(
        card_context,
        card,
        summaries,
        "contract",
        summary_is_contract_missing,
        "contract evidence is missing",
        "contract missing summary",
    )?;
    check_fixture_missing_summary_axis(
        card_context,
        card,
        summaries,
        "discharge",
        summary_is_discharge_missing,
        "discharge evidence is missing",
        "guard missing summary",
    )?;
    check_fixture_missing_summary_axis(
        card_context,
        card,
        summaries,
        "witness",
        summary_is_witness_missing,
        "witness evidence is missing",
        "witness missing summary",
    )?;

    if !fixture_obligation_axis_has_state(card_context, card, "reach", "missing")?
        && summaries
            .iter()
            .any(|summary| summary_is_reach_missing(summary))
    {
        return Err(format!(
            "{card_context} missing must not include a reach missing summary when reach evidence is present"
        ));
    }

    Ok(())
}

fn check_fixture_missing_summary_axis(
    card_context: &str,
    card: &serde_json::Value,
    summaries: &BTreeSet<&str>,
    axis: &str,
    matches_summary: fn(&str) -> bool,
    missing_reason: &str,
    summary_name: &str,
) -> Result<(), String> {
    let axis_missing = fixture_obligation_axis_has_state(card_context, card, axis, "missing")?;
    let summary_present = summaries.iter().any(|summary| matches_summary(summary));
    match (axis_missing, summary_present) {
        (true, false) => Err(format!(
            "{card_context} missing must include {summary_name} because {missing_reason}"
        )),
        (false, true) => Err(format!(
            "{card_context} missing must not include {summary_name} when {axis} evidence is present"
        )),
        _ => Ok(()),
    }
}

fn fixture_obligation_axis_has_state(
    card_context: &str,
    card: &serde_json::Value,
    axis: &str,
    state: &str,
) -> Result<bool, String> {
    Ok(fixture_obligation_axis_states(card_context, card, axis)?.contains(state))
}

fn summary_is_contract_missing(summary: &str) -> bool {
    summary.contains("Missing `# Safety` documentation")
        || summary.contains("Missing public `# Safety` documentation")
}

fn summary_is_discharge_missing(summary: &str) -> bool {
    summary == "Missing visible local guard for inferred safety obligations"
}

fn summary_is_witness_missing(summary: &str) -> bool {
    summary == "No witness receipt imported for this card"
}

fn summary_is_reach_missing(summary: &str) -> bool {
    summary == "No related test path was found by static search"
}

fn required_fixture_evidence_str<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
    card_context: &str,
    evidence_idx: usize,
) -> Result<&'a str, String> {
    let Some(value) = object.get(key).and_then(serde_json::Value::as_str) else {
        return Err(format!(
            "{card_context} obligation_evidence[{evidence_idx}] is missing string key `{key}`"
        ));
    };
    if value.trim().is_empty() {
        Err(format!(
            "{card_context} obligation_evidence[{evidence_idx}] string key `{key}` is empty"
        ))
    } else {
        Ok(value)
    }
}

fn check_fixture_evidence_state(
    card_context: &str,
    evidence_idx: usize,
    evidence_key: &str,
    axis: &str,
    value: Option<&serde_json::Value>,
) -> Result<bool, String> {
    let Some(state) = value.and_then(serde_json::Value::as_object) else {
        return Err(format!(
            "{card_context} obligation_evidence[{evidence_idx}] `{evidence_key}` is missing object key `{axis}`"
        ));
    };
    let Some(present) = state.get("present").and_then(serde_json::Value::as_bool) else {
        return Err(format!(
            "{card_context} obligation_evidence[{evidence_idx}] `{evidence_key}` {axis}.present must be a boolean"
        ));
    };
    let state_name = required_fixture_state_str(
        state,
        "state",
        card_context,
        evidence_idx,
        evidence_key,
        axis,
    )?;
    let _summary = required_fixture_state_str(
        state,
        "summary",
        card_context,
        evidence_idx,
        evidence_key,
        axis,
    )?;
    if !matches!(state_name, "present" | "missing") {
        return Err(format!(
            "{card_context} obligation_evidence[{evidence_idx}] `{evidence_key}` {axis}.state `{state_name}` must be `present` or `missing`"
        ));
    }
    if present != (state_name == "present") {
        return Err(format!(
            "{card_context} obligation_evidence[{evidence_idx}] `{evidence_key}` {axis}.present={present} must agree with state `{state_name}`"
        ));
    }
    Ok(state_name == "missing")
}

fn required_fixture_state_str<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
    card_context: &str,
    evidence_idx: usize,
    evidence_key: &str,
    axis: &str,
) -> Result<&'a str, String> {
    let Some(value) = object.get(key).and_then(serde_json::Value::as_str) else {
        return Err(format!(
            "{card_context} obligation_evidence[{evidence_idx}] `{evidence_key}` {axis}.{key} must be a string"
        ));
    };
    if value.trim().is_empty() {
        Err(format!(
            "{card_context} obligation_evidence[{evidence_idx}] `{evidence_key}` {axis}.{key} must not be empty"
        ))
    } else {
        Ok(value)
    }
}

fn required_json_object_str<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
    path: &str,
    idx: usize,
) -> Result<&'a str, String> {
    let Some(value) = object.get(key).and_then(serde_json::Value::as_str) else {
        return Err(format!(
            "{path} card[{idx}] site is missing string key `{key}`"
        ));
    };
    if value.trim().is_empty() {
        Err(format!(
            "{path} card[{idx}] site string key `{key}` is empty"
        ))
    } else {
        Ok(value)
    }
}

fn required_json_object_u64(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    path: &str,
    idx: usize,
) -> Result<u64, String> {
    object
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| format!("{path} card[{idx}] site is missing unsigned integer key `{key}`"))
}

fn fixture_card_operation_path(
    card: &serde_json::Value,
    site: &serde_json::Map<String, serde_json::Value>,
    operation_family: &str,
    path: &str,
    idx: usize,
) -> Result<String, String> {
    if operation_family == "raw_pointer_deref" {
        return Ok("deref".to_string());
    }
    if operation_family == "unreachable_unchecked" {
        return Ok("unreachable_unchecked".to_string());
    }
    if operation_family == "unsafe_fn_call" {
        let operation =
            require_non_empty_json_str(card, "operation", &format!("{path} card[{idx}]"))?;
        return Ok(unsafe_call_identity_path(operation));
    }
    if operation_family == "unknown" {
        return Ok(site
            .get("owner")
            .and_then(serde_json::Value::as_str)
            .filter(|owner| !owner.trim().is_empty())
            .unwrap_or_else(|| {
                site.get("kind")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or(operation_family)
            })
            .to_string());
    }

    let operation = require_non_empty_json_str(card, "operation", &format!("{path} card[{idx}]"))?;
    let normalized = normalize_operation_snippet(operation);
    let target = normalized
        .split('(')
        .next()
        .unwrap_or(normalized.as_str())
        .trim();
    if let Some((_prefix, method)) = target.rsplit_once('.') {
        return Ok(method.trim_matches(':').to_string());
    }
    if let Some((_prefix, function)) = target.rsplit_once("::") {
        return Ok(function.trim_matches(':').to_string());
    }
    Ok(operation_family.to_string())
}

fn unsafe_call_identity_path(expression: &str) -> String {
    let normalized = normalize_operation_snippet(expression);
    if operation_contains_call_name(&normalized, "new_unchecked") {
        return "new_unchecked".to_string();
    }
    let call = normalized
        .split_once("unsafe")
        .and_then(|(_prefix, after_unsafe)| {
            after_unsafe.split_once('{').map(|(_open, after)| after)
        })
        .unwrap_or(normalized.as_str())
        .split('(')
        .next()
        .unwrap_or("unsafe_fn_call")
        .trim()
        .trim_start_matches("match")
        .trim();
    let call = strip_trailing_turbofish(call);
    if call.is_empty() {
        "unsafe_fn_call".to_string()
    } else if let Some((_prefix, method)) = call.rsplit_once('.') {
        method.trim_matches(':').to_string()
    } else if let Some((_prefix, function)) = call.rsplit_once("::") {
        function.trim_matches(':').to_string()
    } else {
        call.trim_matches(':').to_string()
    }
}

fn normalize_operation_snippet(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn strip_trailing_turbofish(call: &str) -> &str {
    let call = call.trim();
    if !call.ends_with('>') {
        return call;
    }

    let mut depth = 0usize;
    for (idx, ch) in call.char_indices().rev() {
        match ch {
            '>' => depth += 1,
            '<' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let prefix = &call[..idx];
                    return prefix.trim_end_matches(':').trim();
                }
            }
            _ => {}
        }
    }
    call
}

fn operation_contains_call_name(line: &str, name: &str) -> bool {
    for pattern in [
        format!("{name}("),
        format!("{name}::"),
        format!("{name}<"),
        format!("{name}::<"),
    ] {
        if line.contains(&pattern) {
            return true;
        }
    }
    false
}

fn require_identity_token(
    id: &str,
    token: &str,
    path: &str,
    idx: usize,
    field: &str,
) -> Result<(), String> {
    if identity_contains_token(id, token) {
        Ok(())
    } else {
        Err(format!(
            "{path} card[{idx}] id `{id}` must include {field} token `{}`",
            identity_slug(token)
        ))
    }
}

fn identity_contains_token(id: &str, token: &str) -> bool {
    id.contains(token) || id.contains(&identity_slug(token))
}

fn identity_slug(text: &str) -> String {
    let mut slug = String::new();
    let mut previous_separator = false;
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            previous_separator = false;
        } else if !slug.is_empty() && !previous_separator {
            slug.push('-');
            previous_separator = true;
        }
    }
    if previous_separator {
        slug.pop();
    }
    slug
}

fn has_reviewcard_count_suffix(id: &str) -> bool {
    let Some((_, count)) = id.rsplit_once("-c") else {
        return false;
    };
    !count.is_empty() && count.chars().all(|ch| ch.is_ascii_digit())
}

fn contains_hex_run(text: &str, min_len: usize) -> bool {
    let mut run_len = 0usize;
    for ch in text.chars() {
        if ch.is_ascii_hexdigit() {
            run_len += 1;
            if run_len >= min_len {
                return true;
            }
        } else {
            run_len = 0;
        }
    }
    false
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
    token_set_by_delimiter(text, |ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn witness_route_tokens(text: &str) -> BTreeSet<String> {
    token_set_by_delimiter(text, |ch| ch.is_ascii_alphanumeric() || ch == '-')
}

fn token_set_by_delimiter(text: &str, allow: impl Fn(char) -> bool) -> BTreeSet<String> {
    text.split(|ch: char| !allow(ch))
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
    for span in markdown::code_spans(&text) {
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
    for target in markdown::link_targets(&text) {
        let Some(local) = markdown::local_link_target(&target) else {
            continue;
        };
        let resolved = markdown::link_path(&source, local);
        if !resolved.exists() {
            return Err(format!("{path} references missing local link `{target}`"));
        }
    }
    Ok(())
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

pub(crate) fn parse_toml_file(path: &Path) -> Result<toml::Value, String> {
    parse_text_file(path, "TOML", |text| {
        text.parse::<toml::Table>().map(toml::Value::Table)
    })
}

pub(crate) fn parse_json_file(path: &Path) -> Result<serde_json::Value, String> {
    parse_text_file(path, "JSON", |text| serde_json::from_str(text))
}

fn parse_text_file<T, E>(
    path: &Path,
    format_name: &str,
    parser: impl FnOnce(&str) -> Result<T, E>,
) -> Result<T, String>
where
    E: std::fmt::Display,
{
    let text = read_to_string(path)?;
    parser(&text).map_err(|err| format!("{} is not valid {format_name}: {err}", path.display()))
}

fn advisory_card_ids(cards: &serde_json::Value) -> Result<BTreeSet<String>, String> {
    let mut ids = BTreeSet::new();
    for card in json_array_at(cards, "/cards", "cards.json")? {
        let Some(id) = card.get("id").and_then(serde_json::Value::as_str) else {
            return Err("cards.json card is missing id".to_string());
        };
        if !ids.insert(id.to_string()) {
            return Err(format!("cards.json contains duplicate card id `{id}`"));
        }
    }
    Ok(ids)
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

pub(crate) fn json_usize_at(
    value: &serde_json::Value,
    pointer: &str,
    path: &str,
) -> Result<usize, String> {
    let Some(number) = value.pointer(pointer).and_then(serde_json::Value::as_u64) else {
        return Err(format!("{path} is missing unsigned integer at `{pointer}`"));
    };
    usize::try_from(number)
        .map_err(|err| format!("{path} integer at `{pointer}` is too large: {err}"))
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

fn toml_array<'a>(
    value: &'a toml::Value,
    key: &str,
    path: &str,
) -> Result<&'a Vec<toml::Value>, String> {
    value
        .get(key)
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{path} is missing array key `{key}`"))
}

fn toml_table<'a>(
    value: &'a toml::Value,
    path: &str,
    key: &str,
    idx: usize,
) -> Result<&'a toml::map::Map<String, toml::Value>, String> {
    value
        .as_table()
        .ok_or_else(|| format!("{path} {key}[{idx}] must be a table"))
}

fn toml_str_array<'a>(
    value: &'a toml::Value,
    path: &str,
    key: &str,
) -> Result<Vec<&'a str>, String> {
    let values = value
        .as_array()
        .ok_or_else(|| format!("{path} `{key}` must be an array"))?;
    let mut result = Vec::new();
    for (idx, value) in values.iter().enumerate() {
        let Some(text) = value.as_str() else {
            return Err(format!("{path} `{key}`[{idx}] must be a string"));
        };
        if text.trim().is_empty() {
            return Err(format!("{path} `{key}`[{idx}] must not be empty"));
        }
        result.push(text);
    }
    Ok(result)
}

fn required_table_string<'a>(
    table: &'a toml::map::Map<String, toml::Value>,
    key: &str,
    path: &str,
    table_name: &str,
    idx: usize,
) -> Result<&'a str, String> {
    let Some(value) = table.get(key).and_then(toml::Value::as_str) else {
        return Err(format!(
            "{path} {table_name}[{idx}] is missing string `{key}`"
        ));
    };
    if value.trim().is_empty() {
        Err(format!(
            "{path} {table_name}[{idx}] string `{key}` is empty"
        ))
    } else {
        Ok(value)
    }
}

pub(crate) fn require_known(
    value: &str,
    known: &[&str],
    path: &str,
    field: &str,
) -> Result<(), String> {
    if known.contains(&value) {
        Ok(())
    } else {
        Err(format!(
            "{path} has unsupported {field} `{value}`; expected one of {}",
            known.join(", ")
        ))
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

pub(crate) fn require_non_empty_json_str<'a>(
    value: &'a serde_json::Value,
    key: &str,
    path: &str,
) -> Result<&'a str, String> {
    let Some(actual) = json_str(value, key) else {
        return Err(format!("{path} is missing string key `{key}`"));
    };
    if actual.trim().is_empty() {
        Err(format!("{path} string key `{key}` is empty"))
    } else {
        Ok(actual)
    }
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

pub(crate) fn require_json_str(
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

fn require_json_array(value: &serde_json::Value, key: &str, path: &str) -> Result<(), String> {
    if value.get(key).is_some_and(serde_json::Value::is_array) {
        Ok(())
    } else {
        Err(format!("{path} is missing array key `{key}`"))
    }
}

fn require_text_contains(text: &str, needle: &str, path: &Path) -> Result<(), String> {
    if text_contains_ignore_ascii_case(text, needle) {
        Ok(())
    } else {
        Err(format!("{} is missing `{needle}`", path.display()))
    }
}

fn require_text_contains_all(text: &str, path: &Path, needles: &[&str]) -> Result<(), String> {
    for needle in needles {
        require_text_contains(text, needle, path)?;
    }
    Ok(())
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

pub(crate) fn text_contains_ignore_ascii_case(text: &str, needle: &str) -> bool {
    text.to_ascii_lowercase()
        .contains(&needle.to_ascii_lowercase())
}

fn normalize_claim_line(line: &str) -> String {
    line.chars()
        .filter(|character| !matches!(character, '*' | '`' | '_'))
        .collect::<String>()
        .to_ascii_lowercase()
}

fn has_negative_claim_context(text: &str) -> bool {
    text.contains("not")
        || text.contains("does not")
        || text.contains("cannot prove")
        || text.contains("no ")
        || text.contains("without")
}

pub(crate) fn require_file(path: &str) -> Result<(), String> {
    if workspace_path(path).is_file() {
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

pub(crate) fn read_to_string(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|err| format!("read {} failed: {err}", path.display()))
}

pub(crate) fn workspace_path(relative: &str) -> PathBuf {
    let current_dir_path = PathBuf::from(relative);
    if current_dir_path.exists() {
        current_dir_path
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(relative)
    }
}

pub(crate) fn repo_path(relative: &str) -> PathBuf {
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

pub(crate) fn markdown_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
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

pub(crate) fn markdown_table_columns(line: &str) -> Vec<&str> {
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
        || (path.starts_with("badges/") && !public_badges::is_public_endpoint(path))
        || path.ends_with(".sarif")
        || path.ends_with(".profraw")
        || path.ends_with(".profdata")
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

    fn err_text<T>(result: Result<T, String>) -> Result<String, String> {
        match result {
            Ok(_) => Err("expected error".to_string()),
            Err(err) => Ok(err),
        }
    }

    fn doc_artifact_entry(status: &str) -> DocArtifactEntry {
        DocArtifactEntry {
            kind: "spec".to_string(),
            path: "docs/specs/UNSAFE-REVIEW-SPEC-0026-accuracy-validation-and-calibration.md"
                .to_string(),
            status: status.to_string(),
            owner: "calibration".to_string(),
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
    fn source_divergence_counts_parse_git_output() -> Result<(), String> {
        assert_eq!(
            source_sync::parse_rev_list_counts("424\t113\n")?,
            (424, 113)
        );
        assert_eq!(source_sync::parse_rev_list_counts("0 7")?, (0, 7));
        Ok(())
    }

    #[test]
    fn source_divergence_counts_reject_malformed_output() -> Result<(), String> {
        assert!(err_text(source_sync::parse_rev_list_counts(""))?.contains("empty"));
        assert!(err_text(source_sync::parse_rev_list_counts("12"))?.contains("two counts"));
        assert!(
            err_text(source_sync::parse_rev_list_counts("12 3 extra"))?.contains("only two counts")
        );
        assert!(
            err_text(source_sync::parse_rev_list_counts("source 3"))?
                .contains("invalid source-only count")
        );
        assert!(
            err_text(source_sync::parse_rev_list_counts("12 swarm"))?
                .contains("invalid swarm-only count")
        );
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
    fn docs_automation_glob_matches_publication_receipts() {
        assert!(docs_automation_paths::wildcard_match(
            "*publication*.md",
            "2026-05-21-release-0.2.0-publication.md",
        ));
        assert!(!docs_automation_paths::wildcard_match(
            "*publication*.md",
            "2026-05-21-source-promotion-0.2-sync.md",
        ));
    }

    #[test]
    fn first_pr_artifact_list_surfaces_include_full_bundle() -> Result<(), String> {
        public_surfaces::check_first_pr_artifact_list_surfaces()
    }

    #[test]
    fn first_pr_artifact_list_rejects_missing_review_kit() -> Result<(), String> {
        let text = public_surfaces::FIRST_PR_BUNDLE_ARTIFACT_PATHS
            .iter()
            .copied()
            .filter(|artifact| *artifact != "target/unsafe-review/review-kit.json")
            .collect::<Vec<_>>()
            .join("\n");

        let Err(err) = public_surfaces::require_first_pr_artifact_paths("docs/example.md", &text)
        else {
            return Err("missing review-kit artifact should fail".to_string());
        };

        assert!(err.contains("docs/example.md"));
        assert!(err.contains("target/unsafe-review/review-kit.json"));
        Ok(())
    }

    #[test]
    fn docs_automation_scope_detects_external_agent_state_roots() {
        assert!(repo_path_is_under_scope_root(
            ".codex/agent-state.md",
            ".codex"
        ));
        assert!(repo_path_is_under_scope_root(
            ".jules\\goals\\README.md",
            ".jules"
        ));
        assert!(!repo_path_is_under_scope_root(
            "docs/contributing/spec-rails.md",
            ".codex"
        ));
    }

    #[test]
    fn docs_automation_rejects_owned_external_state_root() -> Result<(), String> {
        let owned_roots = vec!["docs".to_string(), ".codex".to_string()];
        let external_roots = vec![".codex".to_string()];

        let Err(err) = check_docs_automation_scope_boundaries(&owned_roots, &external_roots) else {
            return Err("external state root in owned_roots should fail".to_string());
        };

        assert!(err.contains("owned_roots"));
        assert!(err.contains("external_awareness_only"));
        assert!(err.contains(".codex"));
        Ok(())
    }

    #[test]
    fn docs_automation_rejects_checked_external_state_path() -> Result<(), String> {
        let external_roots = vec![".codex".to_string()];

        let Err(err) = reject_docs_automation_external_path(
            "agent-operating-contract",
            "path",
            ".codex/AGENTS.md",
            &external_roots,
        ) else {
            return Err("external state path should fail".to_string());
        };

        assert!(err.contains("agent-operating-contract"));
        assert!(err.contains("external_awareness_only"));
        assert!(err.contains(".codex/AGENTS.md"));
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
    fn fixture_card_identity_accepts_stable_tokens() -> Result<(), String> {
        let card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;

        check_fixture_card_identity(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            "raw_pointer_alignment",
            &card,
        )
    }

    #[test]
    fn fixture_card_identity_rejects_missing_owner_token() -> Result<(), String> {
        let card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;

        let Err(err) = check_fixture_card_identity(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            "raw_pointer_alignment",
            &card,
        ) else {
            return Err("card id without owner token should fail".to_string());
        };

        assert!(err.contains("site.owner"));
        assert!(err.contains("read-header"));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_missing_operation_path_token() -> Result<(), String> {
        let card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-8a1362456e39-pointer_validity-c1",
        )?;

        let Err(err) = check_fixture_card_identity(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            "raw_pointer_alignment",
            &card,
        ) else {
            return Err("card id without operation-path token should fail".to_string());
        };

        assert!(err.contains("operation_path"));
        assert!(err.contains("cast-header"));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_unknown_operation_family() -> Result<(), String> {
        let mut card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-surprise_operation-cast-header-8a1362456e39-pointer_validity-c1",
        )?;
        card["operation_family"] = serde_json::Value::String("surprise_operation".to_string());

        let Err(err) = check_fixture_card_identity(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            "raw_pointer_alignment",
            &card,
        ) else {
            return Err("unknown operation family should fail".to_string());
        };

        assert!(err.contains("operation_family `surprise_operation`"));
        assert!(err.contains("known OperationFamily"));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_unknown_hazard() -> Result<(), String> {
        let mut card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;
        card["hazards"] = serde_json::json!(["pointer_validity", "mystery_hazard"]);

        let Err(err) = check_fixture_card_identity(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            "raw_pointer_alignment",
            &card,
        ) else {
            return Err("unknown hazard should fail".to_string());
        };

        assert!(err.contains("hazards[1] `mystery_hazard`"));
        assert!(err.contains("known HazardKind"));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_duplicate_hazard() -> Result<(), String> {
        let mut card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;
        card["hazards"] = serde_json::json!(["pointer_validity", "pointer_validity"]);

        let Err(err) = check_fixture_card_identity(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            "raw_pointer_alignment",
            &card,
        ) else {
            return Err("duplicate hazard should fail".to_string());
        };

        assert!(err.contains("hazards must not duplicate `pointer_validity`"));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_hazard_outside_operation_registry() -> Result<(), String> {
        let mut card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;
        card["hazards"] = serde_json::json!(["pointer_validity", "ffi_ownership"]);

        let Err(err) = check_fixture_card_identity(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            "raw_pointer_alignment",
            &card,
        ) else {
            return Err("operation-family/hazard mismatch should fail".to_string());
        };

        assert!(err.contains("hazards[1] `ffi_ownership`"));
        assert!(err.contains("operation_family `raw_pointer_read`"));
        assert!(err.contains(OPERATION_FAMILY_REGISTRY));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_unknown_site_kind() -> Result<(), String> {
        let mut card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;
        card["site"]["kind"] = serde_json::Value::String("unsafeish".to_string());

        let Err(err) = check_fixture_card_identity(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            "raw_pointer_alignment",
            &card,
        ) else {
            return Err("unknown site kind should fail".to_string());
        };

        assert!(err.contains("site.kind `unsafeish`"));
        assert!(err.contains("known UnsafeSiteKind"));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_private_public_api_surface() -> Result<(), String> {
        let mut card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;
        card["site"]["public_api_surface"] = serde_json::Value::Bool(true);

        let Err(err) = check_fixture_card_identity(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            "raw_pointer_alignment",
            &card,
        ) else {
            return Err("private public-api surface should fail".to_string());
        };

        assert!(err.contains("public_api_surface"));
        assert!(err.contains("visibility `public`"));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_operation_snippet_mismatch() -> Result<(), String> {
        let mut card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;
        card["operation"] = serde_json::Value::String("ptr.read()".to_string());

        let Err(err) = check_fixture_card_identity(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            "raw_pointer_alignment",
            &card,
        ) else {
            return Err("operation/snippet mismatch should fail".to_string());
        };

        assert!(err.contains("operation must match site.snippet"));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_allows_js_buffer_reentry_operation_context() {
        assert!(is_fixture_operation_snippet_exception(
            "fixtures/js_buffer_reentry_sync_compression/expected.cards.json",
            "JS-backed buffer descriptor captured before possible JS reentry and materialized afterward; capture: let input = StringOrBuffer::from_js(global, arg0)?;; reentry: let level = options.get(global, \"\")?;; materialize: native_compress(&input, level)",
        ));
        assert!(is_fixture_operation_snippet_exception(
            "fixtures\\js_buffer_reentry_sync_compression\\expected.cards.json",
            "JS-backed buffer descriptor captured before possible JS reentry and materialized afterward; capture: let input = StringOrBuffer::from_js(global, arg0)?;; reentry: let level = options.get(global, \"\")?;; materialize: native_compress(&input, level)",
        ));
        assert!(!is_fixture_operation_snippet_exception(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            "JS-backed buffer descriptor captured before possible JS reentry and materialized afterward",
        ));
        assert!(!is_fixture_operation_snippet_exception(
            "fixtures/js_buffer_reentry_sync_compression/expected.cards.json",
            "ptr.read()",
        ));
    }

    #[test]
    fn fixture_card_identity_rejects_unknown_class() -> Result<(), String> {
        let mut card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;
        card["class"] = serde_json::Value::String("maybe_safe".to_string());

        let Err(err) = check_fixture_card_identity(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            "raw_pointer_alignment",
            &card,
        ) else {
            return Err("unknown class should fail".to_string());
        };

        assert!(err.contains("class `maybe_safe`"));
        assert!(err.contains("known ReviewClass"));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_unknown_priority() -> Result<(), String> {
        let mut card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;
        card["priority"] = serde_json::Value::String("urgent".to_string());

        let Err(err) = check_fixture_card_identity(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            "raw_pointer_alignment",
            &card,
        ) else {
            return Err("unknown priority should fail".to_string());
        };

        assert!(err.contains("priority `urgent`"));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_stale_classification_signal() -> Result<(), String> {
        let mut card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;
        card["class"] = serde_json::Value::String("contract_missing".to_string());
        card["confidence"] = serde_json::Value::String("medium".to_string());

        let Err(err) = check_fixture_card_identity(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            "raw_pointer_alignment",
            &card,
        ) else {
            return Err("stale classification signal should fail".to_string());
        };

        assert!(err.contains("contract_missing"));
        assert!(err.contains("confidence `high`"));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_orphan_obligation_evidence() -> Result<(), String> {
        let mut card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;
        card["obligation_evidence"][0]["description"] =
            serde_json::Value::String("unlisted obligation".to_string());

        let Err(err) = check_fixture_card_identity(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            "raw_pointer_alignment",
            &card,
        ) else {
            return Err("orphan obligation evidence should fail".to_string());
        };

        assert!(err.contains("obligation_evidence[0]"));
        assert!(err.contains("must match an obligation"));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_obligation_key_outside_operation_registry()
    -> Result<(), String> {
        let mut card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;
        card["obligation_evidence"][0]["key"] = serde_json::Value::String("utf8".to_string());

        let Err(err) = check_fixture_card_identity(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            "raw_pointer_alignment",
            &card,
        ) else {
            return Err("operation-family/obligation-key mismatch should fail".to_string());
        };

        assert!(err.contains("obligation_evidence[0] key `utf8`"));
        assert!(err.contains("operation_family `raw_pointer_read`"));
        assert!(err.contains(OPERATION_FAMILY_REGISTRY));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_evidence_state_mismatch() -> Result<(), String> {
        let mut card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;
        card["obligation_evidence"][0]["discharge"]["present"] = serde_json::Value::Bool(true);

        let Err(err) = check_fixture_card_identity(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            "raw_pointer_alignment",
            &card,
        ) else {
            return Err("evidence present/state mismatch should fail".to_string());
        };

        assert!(err.contains("discharge.present=true"));
        assert!(err.contains("state `missing`"));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_top_level_contract_drift() -> Result<(), String> {
        let mut card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;
        card["contract"] = serde_json::Value::String("Unrelated contract summary".to_string());

        let Err(err) = check_fixture_card_identity(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            "raw_pointer_alignment",
            &card,
        ) else {
            return Err("top-level contract drift should fail".to_string());
        };

        assert!(err.contains("top-level contract"));
        assert!(err.contains("obligation-level contract.summary"));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_top_level_discharge_drift() -> Result<(), String> {
        let mut card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;
        card["discharge"] = serde_json::Value::String(
            "All inferred safety obligations have visible local discharge evidence".to_string(),
        );

        let Err(err) = check_fixture_card_identity(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            "raw_pointer_alignment",
            &card,
        ) else {
            return Err("top-level discharge drift should fail".to_string());
        };

        assert!(err.contains("top-level discharge"));
        assert!(err.contains("No visible local guard detected"));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_top_level_witness_drift() -> Result<(), String> {
        let mut card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;
        card["witness"] = serde_json::Value::String(
            "Imported miri receipt with `ran` strength: focused fixture witness passed".to_string(),
        );

        let Err(err) = check_fixture_card_identity(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            "raw_pointer_alignment",
            &card,
        ) else {
            return Err("top-level witness drift should fail".to_string());
        };

        assert!(err.contains("top-level witness"));
        assert!(err.contains("obligation-level witness.summary"));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_reach_owner_mismatch() -> Result<(), String> {
        let mut card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;
        card["reach"] =
            serde_json::Value::String("1 related test file(s) mention owner `other_owner`".into());

        let Err(err) = check_fixture_card_identity(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            "raw_pointer_alignment",
            &card,
        ) else {
            return Err("top-level reach owner mismatch should fail".to_string());
        };

        assert!(err.contains("reach owner `other_owner`"));
        assert!(err.contains("site.owner `read_header`"));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_obligation_reach_summary_drift() -> Result<(), String> {
        let mut card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;
        card["obligation_evidence"][0]["reach"]["summary"] = serde_json::Value::String(
            "No static test mention of owner `read_header` was found".into(),
        );

        let Err(err) = check_fixture_card_identity(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            "raw_pointer_alignment",
            &card,
        ) else {
            return Err("obligation reach summary drift should fail".to_string());
        };

        assert!(err.contains("obligation_evidence[0]"));
        assert!(err.contains("reach.summary"));
        assert!(err.contains("top-level reach"));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_reach_execution_overclaim() -> Result<(), String> {
        let mut card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;
        card["reach"] = serde_json::Value::String("site executed in read_header test".into());

        let Err(err) = check_fixture_card_identity(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            "raw_pointer_alignment",
            &card,
        ) else {
            return Err("reach execution overclaim should fail".to_string());
        };

        assert!(err.contains("reach"));
        assert!(err.contains("site executed"));
        assert!(err.contains("static test mentions"));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_empty_missing_summary_for_missing_evidence()
    -> Result<(), String> {
        let mut card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;
        card["missing"] = serde_json::Value::Array(Vec::new());

        let Err(err) = check_fixture_card_identity(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            "raw_pointer_alignment",
            &card,
        ) else {
            return Err(
                "card with missing evidence but empty missing summary should fail".to_string(),
            );
        };

        assert!(err.contains("missing must summarize"));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_missing_summary_without_witness_gap() -> Result<(), String> {
        let mut card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;
        card["obligation_evidence"][0]["witness"]["present"] = serde_json::Value::Bool(true);
        card["obligation_evidence"][0]["witness"]["state"] =
            serde_json::Value::String("present".to_string());
        card["obligation_evidence"][0]["witness"]["summary"] =
            serde_json::Value::String("Imported miri receipt with `ran` strength".to_string());
        card["witness"] =
            serde_json::Value::String("Imported miri receipt with `ran` strength".to_string());

        let Err(err) = check_fixture_card_identity(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            "raw_pointer_alignment",
            &card,
        ) else {
            return Err("stale witness missing summary should fail".to_string());
        };

        assert!(err.contains("witness missing summary"));
        assert!(err.contains("witness evidence is present"));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_missing_contract_summary_omission() -> Result<(), String> {
        let mut card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;
        let missing_contract = "Missing `# Safety` documentation or `SAFETY:` / `Safety:` comment";
        card["obligation_evidence"][0]["contract"]["present"] = serde_json::Value::Bool(false);
        card["obligation_evidence"][0]["contract"]["state"] =
            serde_json::Value::String("missing".to_string());
        card["obligation_evidence"][0]["contract"]["summary"] =
            serde_json::Value::String(missing_contract.to_string());
        card["contract"] = serde_json::Value::String(missing_contract.to_string());

        let Err(err) = check_fixture_card_identity(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            "raw_pointer_alignment",
            &card,
        ) else {
            return Err("omitted contract missing summary should fail".to_string());
        };

        assert!(err.contains("contract missing summary"));
        assert!(err.contains("contract evidence is missing"));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_stale_reach_missing_summary() -> Result<(), String> {
        let mut card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;
        card["missing"] = serde_json::json!([
            "Missing visible local guard for inferred safety obligations",
            "No related test path was found by static search",
            "No witness receipt imported for this card"
        ]);

        let Err(err) = check_fixture_card_identity(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            "raw_pointer_alignment",
            &card,
        ) else {
            return Err("stale reach missing summary should fail".to_string());
        };

        assert!(err.contains("reach missing summary"));
        assert!(err.contains("reach evidence is present"));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_stale_missing_summary_for_present_evidence()
    -> Result<(), String> {
        let mut card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;
        card["obligation_evidence"][0]["discharge"]["present"] = serde_json::Value::Bool(true);
        card["obligation_evidence"][0]["discharge"]["state"] =
            serde_json::Value::String("present".to_string());
        card["obligation_evidence"][0]["discharge"]["summary"] =
            serde_json::Value::String("Alignment guard code was detected".to_string());
        card["discharge"] = serde_json::Value::String(
            "All inferred safety obligations have visible local discharge evidence".to_string(),
        );
        card["obligation_evidence"][0]["witness"]["present"] = serde_json::Value::Bool(true);
        card["obligation_evidence"][0]["witness"]["state"] =
            serde_json::Value::String("present".to_string());
        card["obligation_evidence"][0]["witness"]["summary"] =
            serde_json::Value::String("Imported miri receipt with `ran` strength".to_string());
        card["witness"] =
            serde_json::Value::String("Imported miri receipt with `ran` strength".to_string());

        let Err(err) = check_fixture_card_identity(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            "raw_pointer_alignment",
            &card,
        ) else {
            return Err(
                "card with present evidence but stale missing summary should fail".to_string(),
            );
        };

        assert!(err.contains("missing must be empty"));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_missing_next_action() -> Result<(), String> {
        let mut card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;
        card.as_object_mut()
            .ok_or_else(|| "test card must be an object".to_string())?
            .remove("next_action");

        let Err(err) = check_fixture_card_identity(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            "raw_pointer_alignment",
            &card,
        ) else {
            return Err("missing next_action should fail".to_string());
        };

        assert!(err.contains("next_action"));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_next_action_overclaim() -> Result<(), String> {
        let mut card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;
        card["next_action"] =
            serde_json::Value::String("Add proof that this is all clear.".to_string());

        let Err(err) = check_fixture_card_identity(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            "raw_pointer_alignment",
            &card,
        ) else {
            return Err("overclaiming next_action should fail".to_string());
        };

        assert!(err.contains("next_action"));
        assert!(err.contains("all clear"));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_next_action_wrong_operation_family() -> Result<(), String> {
        let mut card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;
        card["next_action"] = serde_json::Value::String(
            "Add or expose the local guard that discharges the `vec_set_len` safety obligation."
                .to_string(),
        );

        let Err(err) = check_fixture_card_identity(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            "raw_pointer_alignment",
            &card,
        ) else {
            return Err("wrong-operation next_action should fail".to_string());
        };

        assert!(err.contains("next_action"));
        assert!(err.contains("operation_family"));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_guard_missing_comment_next_action() -> Result<(), String> {
        let mut card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;
        card["next_action"] = serde_json::Value::String(
            "Add a `SAFETY:` comment that explains why the guard is unnecessary.".to_string(),
        );

        let Err(err) = check_fixture_next_action(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            &card,
            "raw_pointer_read",
        ) else {
            return Err("guard-missing comment next_action should fail".to_string());
        };

        assert!(err.contains("guard_missing"));
        assert!(err.contains("guard evidence"));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_generic_manual_route_guard_next_action() -> Result<(), String>
    {
        let mut card = test_fixture_card(
            "UR-inline-asm-human-review-src-lib-rs-pause-once-operation-inline_asm-asm-a6c6a5d4bbb1-inline_asm-c1",
        )?;
        card["operation_family"] = serde_json::Value::String("inline_asm".to_string());
        card["witness_routes"][0]["kind"] =
            serde_json::Value::String("human-deep-review".to_string());
        card["witness_routes"][0]["command"] = serde_json::Value::Null;
        card["next_action"] = serde_json::Value::String(
            "Add or expose the local guard that discharges the `inline_asm` safety obligation."
                .to_string(),
        );

        let Err(err) = check_fixture_next_action(
            "fixtures/inline_asm_human_review/expected.cards.json",
            0,
            &card,
            "inline_asm",
        ) else {
            return Err("generic manual-route guard next_action should fail".to_string());
        };

        assert!(err.contains("human-deep-review guard_missing"));
        assert!(err.contains("manual review"));
        assert!(err.contains("guard evidence"));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_stale_class_next_actions() -> Result<(), String> {
        for (class_name, stale_action, expected) in [
            (
                "requires_loom",
                "Add or expose the local guard for this concurrency invariant.",
                "Loom/Shuttle",
            ),
            (
                "requires_sanitizer",
                "Run Miri for this sanitizer-routed card.",
                "sanitizer receipt",
            ),
            (
                "requires_kani_or_crux",
                "Attach a Miri receipt for this proof-routed card.",
                "Kani/Crux",
            ),
            (
                "miri_unsupported",
                "Run Miri for this FFI seam.",
                "sanitizer/cargo-careful",
            ),
            (
                "miri_unsupported",
                "Use sanitizer/cargo-careful evidence for this FFI seam.",
                "FFI boundary contract",
            ),
            (
                "reachable_unwitnessed",
                "Add or expose the local guard for this card.",
                "witness receipt",
            ),
            (
                "unsafe_unreached",
                "Attach a witness receipt for this card.",
                "safe wrapper",
            ),
            (
                "guarded_unwitnessed",
                "Add or expose the local guard for this card.",
                "witness receipt",
            ),
            (
                "witness_mismatch",
                "Mark this card as reviewed.",
                "matching receipt",
            ),
            (
                "static_unknown",
                "Attach a focused witness receipt for this card.",
                "manual contract",
            ),
            (
                "baseline_known",
                "Mark this card as reviewed.",
                "baseline ledger",
            ),
            (
                "suppressed",
                "Mark this card as reviewed.",
                "suppression owner",
            ),
        ] {
            let mut card = test_fixture_card(
                "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
            )?;
            card["class"] = serde_json::Value::String(class_name.to_string());
            card["next_action"] = serde_json::Value::String(stale_action.to_string());

            let Err(err) = check_fixture_next_action(
                "fixtures/raw_pointer_alignment/expected.cards.json",
                0,
                &card,
                "raw_pointer_read",
            ) else {
                return Err(format!("{class_name} stale next_action should fail"));
            };

            assert!(err.contains(class_name));
            assert!(
                err.contains(expected),
                "expected `{err}` to mention `{expected}`"
            );
        }
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_generic_human_route_witness_next_action() -> Result<(), String>
    {
        let mut card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;
        card["class"] = serde_json::Value::String("guarded_unwitnessed".to_string());
        card["witness_routes"][0]["kind"] =
            serde_json::Value::String("human-deep-review".to_string());
        card["witness_routes"][0]["command"] = serde_json::Value::Null;
        card["next_action"] = serde_json::Value::String(
            "Attach a focused witness receipt or mark the static limitation explicitly."
                .to_string(),
        );

        let Err(err) = check_fixture_next_action(
            "fixtures/documented_private_unsafe_fn/expected.cards.json",
            0,
            &card,
            "unknown",
        ) else {
            return Err("generic human-route witness next_action should fail".to_string());
        };

        assert!(err.contains("human-deep-review"));
        assert!(err.contains("human or manual review"));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_generic_executable_route_witness_next_action()
    -> Result<(), String> {
        let mut card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;
        card["class"] = serde_json::Value::String("guarded_unwitnessed".to_string());
        card["next_action"] = serde_json::Value::String(
            "Attach a focused witness receipt or mark the static limitation explicitly."
                .to_string(),
        );

        let Err(err) = check_fixture_next_action(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            &card,
            "raw_pointer_read",
        ) else {
            return Err("generic Miri-route witness next_action should fail".to_string());
        };

        assert!(err.contains("Miri"));

        card["witness_routes"][0]["kind"] = serde_json::Value::String("cargo-careful".to_string());
        card["witness_routes"][0]["command"] =
            serde_json::Value::String("cargo +nightly careful test read_header".to_string());

        let Err(err) = check_fixture_next_action(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            &card,
            "raw_pointer_read",
        ) else {
            return Err("generic cargo-careful-route witness next_action should fail".to_string());
        };

        assert!(err.contains("cargo-careful"));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_unknown_obligation_next_action() -> Result<(), String> {
        let mut card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;
        card["next_action"] = serde_json::Value::String(
            "Add or expose the local guard that discharges the `unknown` safety obligation."
                .to_string(),
        );

        let Err(err) = check_fixture_next_action(
            "fixtures/private_unsafe_helper_safety_comment/expected.cards.json",
            0,
            &card,
            "unknown",
        ) else {
            return Err("unknown obligation next_action should fail".to_string());
        };

        assert!(err.contains("unknown obligation"));
        assert!(err.contains("manual unsafe-site review"));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_public_safety_comment_next_action() -> Result<(), String> {
        let mut card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;
        card["missing"] = serde_json::json!([
            "Missing public `# Safety` documentation for unsafe API",
            "No witness receipt imported for this card"
        ]);
        card["next_action"] = serde_json::Value::String(
            "Add a precise `# Safety` section or `SAFETY:` / `Safety:` comment that names the required conditions."
                .to_string(),
        );

        let Err(err) = check_fixture_next_action(
            "fixtures/public_unsafe_fn_missing_safety/expected.cards.json",
            0,
            &card,
            "unknown",
        ) else {
            return Err("public-safety comment next_action should fail".to_string());
        };

        assert!(err.contains("public unsafe API"));
        assert!(err.contains("public `# Safety`"));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_route_command_missing_from_verify_commands()
    -> Result<(), String> {
        let mut card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;
        card["verify_commands"] = serde_json::Value::Array(Vec::new());

        let Err(err) = check_fixture_card_identity(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            "raw_pointer_alignment",
            &card,
        ) else {
            return Err("route command missing from verify_commands should fail".to_string());
        };

        assert!(err.contains("witness route command"));
        assert!(err.contains("verify_commands"));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_unbacked_verify_command() -> Result<(), String> {
        let mut card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;
        card["verify_commands"] = serde_json::json!(["cargo test unrelated"]);

        let Err(err) = check_fixture_card_identity(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            "raw_pointer_alignment",
            &card,
        ) else {
            return Err("unbacked verify command should fail".to_string());
        };

        assert!(err.contains("verify_commands[0]"));
        assert!(err.contains("must be backed by a witness route command"));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_route_command_wrong_tool() -> Result<(), String> {
        let mut card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;
        card["witness_routes"][0]["command"] =
            serde_json::Value::String("cargo +nightly careful test read_header".to_string());
        card["verify_commands"] = serde_json::json!(["cargo +nightly careful test read_header"]);

        let Err(err) = check_fixture_card_identity(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            "raw_pointer_alignment",
            &card,
        ) else {
            return Err("wrong witness tool command should fail".to_string());
        };

        assert!(err.contains("witness_routes[0] kind `miri`"));
        assert!(err.contains("matching witness tool"));
        Ok(())
    }

    #[test]
    fn fixture_route_command_rejects_commandless_manual_route_kind() -> Result<(), String> {
        let Err(err) = check_fixture_route_command_matches_kind(
            "fixtures/manual/expected.cards.json card[0]",
            0,
            "human-deep-review",
            Some("cargo test manual_review"),
        ) else {
            return Err("manual route command should fail".to_string());
        };

        assert!(err.contains("witness_routes[0] kind `human-deep-review`"));
        assert!(err.contains("must not include a command by default"));
        Ok(())
    }

    #[test]
    fn fixture_route_command_allows_manual_route_without_command() -> Result<(), String> {
        check_fixture_route_command_matches_kind(
            "fixtures/manual/expected.cards.json card[0]",
            0,
            "human-deep-review",
            None,
        )
    }

    #[test]
    fn fixture_card_identity_rejects_witness_route_outside_operation_registry() -> Result<(), String>
    {
        let mut card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;
        card["witness_routes"][0]["kind"] = serde_json::Value::String("loom".to_string());

        let Err(err) = check_fixture_card_identity(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            "raw_pointer_alignment",
            &card,
        ) else {
            return Err("operation-family/witness-route mismatch should fail".to_string());
        };

        assert!(err.contains("witness_routes[0] kind `loom`"));
        assert!(err.contains("operation_family `raw_pointer_read`"));
        assert!(err.contains(OPERATION_FAMILY_REGISTRY));
        Ok(())
    }

    #[test]
    fn fixture_card_identity_rejects_required_witness_route() -> Result<(), String> {
        let mut card = test_fixture_card(
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1",
        )?;
        card["witness_routes"][0]["required"] = serde_json::Value::Bool(true);

        let Err(err) = check_fixture_card_identity(
            "fixtures/raw_pointer_alignment/expected.cards.json",
            0,
            "raw_pointer_alignment",
            &card,
        ) else {
            return Err("required witness route should fail".to_string());
        };

        assert!(err.contains("required must remain false"));
        assert!(err.contains("does not require execution by default"));
        Ok(())
    }

    #[test]
    fn calibration_manifest_validates_current_fixture_contract() -> Result<(), String> {
        check_calibration()
    }

    #[test]
    fn calibration_report_rejects_stale_counts() -> Result<(), String> {
        let stats = test_accuracy_report_stats();
        let text = test_accuracy_report_text().replace("- Label samples: 2", "- Label samples: 1");

        let Err(err) = check_accuracy_calibration_report_text("test-report.md", &text, &stats)
        else {
            return Err("stale label sample counts should fail".to_string());
        };

        assert!(err.contains("- Label samples: 2"));
        Ok(())
    }

    #[test]
    fn objective_audit_rejects_stale_calibration_counts() -> Result<(), String> {
        let stats = test_accuracy_report_stats();
        let text = "The checked report currently records 1 fixture-pinned claims, 2 calibration cases, 1 label ledgers, and 1 label samples.";

        let Err(err) = check_objective_audit_calibration_snapshot_text(
            "docs/status/OBJECTIVE_AUDIT.md",
            text,
            &stats,
        ) else {
            return Err("stale objective audit label sample counts should fail".to_string());
        };

        assert!(err.contains("2 label samples"));
        Ok(())
    }

    #[test]
    fn calibration_report_requires_boundary_text() -> Result<(), String> {
        let stats = test_accuracy_report_stats();
        let text =
            test_accuracy_report_text().replace("No policy readiness claim.", "Policy ready.");

        let Err(err) = check_accuracy_calibration_report_text("test-report.md", &text, &stats)
        else {
            return Err("missing policy readiness boundary should fail".to_string());
        };

        assert!(err.contains("No policy readiness claim"));
        Ok(())
    }

    #[test]
    fn accuracy_claim_promotion_rejects_fixture_claim_with_labeled_report() -> Result<(), String> {
        let policy = test_accuracy_policy_claim(
            "fixture_pinned",
            r#"["raw_pointer_alignment"]"#,
            "[]",
            r#"["docs/accuracy/labels/raw-pointer-read-alignment.toml"]"#,
            &format!(r#"["{ACCURACY_CALIBRATION_REPORT}"]"#),
            "Fixture-pinned test claim remains limited to fixture and label-ledger evidence.",
        )?;

        let Err(err) = accuracy_calibration_report_stats(&policy, 1, 1) else {
            return Err("fixture-pinned claim with labeled report should fail".to_string());
        };

        assert!(err.contains("fixture_pinned"));
        assert!(err.contains("labeled_reports"));
        Ok(())
    }

    #[test]
    fn accuracy_claim_promotion_requires_dogfood_target_for_dogfood_status() -> Result<(), String> {
        let policy = test_accuracy_policy_claim(
            "dogfood_measured",
            "[]",
            "[]",
            "[]",
            "[]",
            "Dogfood measured test claim remains limited to named corpus targets.",
        )?;

        let Err(err) = accuracy_calibration_report_stats(&policy, 1, 1) else {
            return Err("dogfood-measured claim without dogfood target should fail".to_string());
        };

        assert!(err.contains("dogfood_measured"));
        assert!(err.contains("dogfood_targets"));
        Ok(())
    }

    #[test]
    fn accuracy_claim_promotion_rejects_unknown_dogfood_target() -> Result<(), String> {
        let policy = test_accuracy_policy_claim(
            "dogfood_measured",
            "[]",
            r#"["missing-target"]"#,
            "[]",
            "[]",
            "Dogfood measured test claim remains limited to named corpus targets.",
        )?;

        let Err(err) = accuracy_calibration_report_stats(&policy, 1, 1) else {
            return Err(
                "dogfood-measured claim with unknown dogfood target should fail".to_string(),
            );
        };

        assert!(err.contains("unknown dogfood target"));
        assert!(err.contains("missing-target"));
        Ok(())
    }

    #[test]
    fn accuracy_claim_promotion_requires_labeled_report_for_calibrated_status() -> Result<(), String>
    {
        let policy = test_accuracy_policy_claim(
            "labeled_calibrated",
            "[]",
            "[]",
            "[]",
            "[]",
            "Labeled calibrated test claim remains limited to checked labeled report evidence.",
        )?;

        let Err(err) = accuracy_calibration_report_stats(&policy, 1, 1) else {
            return Err("labeled-calibrated claim without labeled report should fail".to_string());
        };

        assert!(err.contains("labeled_calibrated"));
        assert!(err.contains("labeled_reports"));
        Ok(())
    }

    #[test]
    fn accuracy_claim_promotion_rejects_allowed_public_overclaim() -> Result<(), String> {
        let policy = test_accuracy_policy_claim(
            "fixture_pinned",
            r#"["raw_pointer_alignment"]"#,
            "[]",
            r#"["docs/accuracy/labels/raw-pointer-read-alignment.toml"]"#,
            "[]",
            "Fixture-pinned test claim reports global precision for the analyzer.",
        )?;

        let Err(err) = accuracy_calibration_report_stats(&policy, 1, 1) else {
            return Err("allowed public claim with overclaim wording should fail".to_string());
        };

        assert!(err.contains("allowed_public_claim"));
        assert!(err.contains("global precision"));
        Ok(())
    }

    #[test]
    fn accuracy_claim_promotion_requires_public_claim_level() -> Result<(), String> {
        let policy = test_accuracy_policy_claim(
            "fixture_pinned",
            r#"["raw_pointer_alignment"]"#,
            "[]",
            r#"["docs/accuracy/labels/raw-pointer-read-alignment.toml"]"#,
            "[]",
            "This test claim remains limited to fixture and label-ledger evidence.",
        )?;

        let Err(err) = accuracy_calibration_report_stats(&policy, 1, 1) else {
            return Err("allowed public claim without claim level should fail".to_string());
        };

        assert!(err.contains("allowed_public_claim"));
        assert!(err.contains("Fixture-pinned"));
        Ok(())
    }

    #[test]
    fn accuracy_claim_promotion_requires_common_forbidden_claims() -> Result<(), String> {
        let policy = test_accuracy_policy_claim_from(TestAccuracyPolicyClaim {
            status: "fixture_pinned",
            support_tier: "Core operation smoke slice",
            fixtures: r#"["raw_pointer_alignment"]"#,
            dogfood_targets: "[]",
            label_ledgers: r#"["docs/accuracy/labels/raw-pointer-read-alignment.toml"]"#,
            labeled_reports: "[]",
            allowed_public_claim: "Fixture-pinned test claim remains limited to fixture and label-ledger evidence.",
            forbidden_claims: r#"["memory-safety proof"]"#,
        })?;

        let Err(err) = accuracy_calibration_report_stats(&policy, 1, 1) else {
            return Err("claim without common forbidden claims should fail".to_string());
        };

        assert!(err.contains("forbidden_claims"));
        assert!(err.contains("global precision"));
        Ok(())
    }

    #[test]
    fn accuracy_claim_promotion_rejects_unknown_support_tier() -> Result<(), String> {
        let policy = test_accuracy_policy_claim_with_support_tier(
            "fixture_pinned",
            "Missing support tier",
            r#"["raw_pointer_alignment"]"#,
            "[]",
            r#"["docs/accuracy/labels/raw-pointer-read-alignment.toml"]"#,
            "[]",
            "Fixture-pinned test claim remains limited to known support tier evidence.",
        )?;

        let Err(err) = accuracy_calibration_report_stats(&policy, 1, 1) else {
            return Err("accuracy claim with unknown support tier should fail".to_string());
        };

        assert!(err.contains("support_tier"));
        assert!(err.contains("Missing support tier"));
        assert!(err.contains(SUPPORT_TIERS_DOC));
        Ok(())
    }

    fn test_fixture_card(id: &str) -> Result<serde_json::Value, String> {
        format!(
            r#"{{
  "id": "{id}",
  "class": "guard_missing",
  "priority": "high",
  "confidence": "medium",
  "site": {{
    "file": "src/lib.rs",
    "line": 8,
    "column": 5,
    "owner": "read_header",
    "kind": "operation",
    "visibility": "private",
    "public_api_surface": false,
    "snippet": "unsafe {{ ptr.cast::<Header>().read() }}"
  }},
  "operation": "unsafe {{ ptr.cast::<Header>().read() }}",
  "operation_family": "raw_pointer_read",
  "hazards": ["pointer_validity", "alignment"],
  "obligations": ["pointer is aligned for the accessed type"],
  "obligation_evidence": [
    {{
      "key": "alignment",
      "description": "pointer is aligned for the accessed type",
      "contract": {{
        "present": true,
        "state": "present",
        "summary": "Nearby `SAFETY:` comment was detected"
      }},
      "discharge": {{
        "present": false,
        "state": "missing",
        "summary": "No alignment guard code was detected"
      }},
      "reach": {{
        "present": true,
        "state": "present",
        "summary": "1 related test file(s) mention owner `read_header`"
      }},
      "witness": {{
        "present": false,
        "state": "missing",
        "summary": "No imported witness receipt was found"
      }}
    }}
  ],
  "missing": [
    "Missing visible local guard for inferred safety obligations",
    "No witness receipt imported for this card"
  ],
  "contract": "Nearby `SAFETY:` comment was detected",
  "discharge": "No visible local guard detected",
  "reach": "1 related test file(s) mention owner `read_header`",
  "witness": "No imported witness receipt was found",
  "next_action": "Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.",
  "witness_routes": [
    {{
      "kind": "miri",
      "reason": "Pure-Rust UB-adjacent hazard; Miri is the strongest concrete-execution witness when the path is supported",
      "command": "cargo +nightly miri test read_header",
      "required": false
    }}
  ],
  "verify_commands": [
    "cargo +nightly miri test read_header"
  ]
}}"#
        )
        .parse::<serde_json::Value>()
        .map_err(|err| format!("parse test card failed: {err}"))
    }

    fn test_accuracy_report_stats() -> AccuracyCalibrationReportStats {
        AccuracyCalibrationReportStats {
            claim_count: 1,
            calibration_case_count: 2,
            label_ledger_count: 1,
            label_sample_count: 2,
            labeled_report_count: 0,
            fixture_pinned_claims: 1,
            dogfood_measured_claims: 0,
            labeled_calibrated_claims: 0,
            policy_eligible_claims: 0,
        }
    }

    fn test_accuracy_report_text() -> String {
        r#"
Static unsafe contract review only. This is not a proof of memory safety, not UB-free status, and not a Miri result.

- Claims: 1
- Fixture-pinned claims: 1
- Dogfood-measured claims: 0
- Labeled-calibrated claims: 0
- Policy-eligible claims: 0
- Calibration cases: 2
- Label ledgers: 1
- Label samples: 2
- Labeled reports: 0

- No global precision/recall claim.
- No policy readiness claim.
- No support-tier promotion.
- No witness execution claim.
- No memory-safety proof.
- No UB-free status.
- No Miri-clean status.
"#
        .to_string()
    }

    fn test_accuracy_policy_claim(
        status: &str,
        fixtures: &str,
        dogfood_targets: &str,
        label_ledgers: &str,
        labeled_reports: &str,
        allowed_public_claim: &str,
    ) -> Result<toml::Value, String> {
        test_accuracy_policy_claim_with_support_tier(
            status,
            "Core operation smoke slice",
            fixtures,
            dogfood_targets,
            label_ledgers,
            labeled_reports,
            allowed_public_claim,
        )
    }

    fn test_accuracy_policy_claim_with_support_tier(
        status: &str,
        support_tier: &str,
        fixtures: &str,
        dogfood_targets: &str,
        label_ledgers: &str,
        labeled_reports: &str,
        allowed_public_claim: &str,
    ) -> Result<toml::Value, String> {
        test_accuracy_policy_claim_from(TestAccuracyPolicyClaim {
            status,
            support_tier,
            fixtures,
            dogfood_targets,
            label_ledgers,
            labeled_reports,
            allowed_public_claim,
            forbidden_claims: r#"["global precision", "global recall", "memory-safety proof"]"#,
        })
    }

    struct TestAccuracyPolicyClaim<'a> {
        status: &'a str,
        support_tier: &'a str,
        fixtures: &'a str,
        dogfood_targets: &'a str,
        label_ledgers: &'a str,
        labeled_reports: &'a str,
        allowed_public_claim: &'a str,
        forbidden_claims: &'a str,
    }

    fn test_accuracy_policy_claim_from(
        claim: TestAccuracyPolicyClaim<'_>,
    ) -> Result<toml::Value, String> {
        let TestAccuracyPolicyClaim {
            status,
            support_tier,
            fixtures,
            dogfood_targets,
            label_ledgers,
            labeled_reports,
            allowed_public_claim,
            forbidden_claims,
        } = claim;

        format!(
            r#"
[[claim]]
id = "test-claim"
status = "{status}"
kind = "evidence_precision"
owner = "calibration"
support_tier = "{support_tier}"
fixtures = {fixtures}
dogfood_targets = {dogfood_targets}
label_ledgers = {label_ledgers}
labeled_reports = {labeled_reports}
allowed_public_claim = """
{allowed_public_claim}
"""
forbidden_claims = {forbidden_claims}
"#
        )
        .parse::<toml::Table>()
        .map(toml::Value::Table)
        .map_err(|err| format!("test policy TOML parse failed: {err}"))
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
    fn spec_status_dashboard_validates_current_table() -> Result<(), String> {
        spec_status::check_dashboard_impl().map(|_| ())
    }

    #[test]
    fn spec_status_table_parser_extracts_rows() -> Result<(), String> {
        let text = r#"
| Spec | Status | Implementation state | Proof commands | Last touched | Notes |
|---|---|---|---|---|---|
| `UNSAFE-REVIEW-SPEC-0024` CI design | draft | CI lane taxonomy documented | `cargo run --locked -p xtask -- check-pr` | 2026-05-23 | Advisory findings stay non-blocking |
"#;

        let rows = spec_status::rows_from_text(text)?;

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].spec_id, "UNSAFE-REVIEW-SPEC-0024");
        assert_eq!(rows[0].status, "draft");
        assert_eq!(rows[0].last_touched, "2026-05-23");
        Ok(())
    }

    #[test]
    fn spec_status_lifecycle_header_accepts_plain_and_bulleted_status() -> Result<(), String> {
        assert_eq!(
            spec_status::lifecycle_status_from_text(
                "Status: accepted, partial-runtime",
                "plain.md"
            )?,
            "accepted"
        );
        assert_eq!(
            spec_status::lifecycle_status_from_text("- Status: Accepted", "bulleted.md")?,
            "accepted"
        );
        Ok(())
    }

    #[test]
    fn spec_status_lifecycle_match_rejects_dashboard_drift() -> Result<(), String> {
        let err = err_text(spec_status::check_lifecycle_match(
            "UNSAFE-REVIEW-SPEC-0026",
            "accepted",
            "proposed",
            "docs/specs/UNSAFE-REVIEW-SPEC-0026-accuracy-validation-and-calibration.md",
        ))?;

        assert!(err.contains("UNSAFE-REVIEW-SPEC-0026"));
        assert!(err.contains("status `accepted` must match"));
        assert!(err.contains("Status lifecycle `proposed`"));
        Ok(())
    }

    #[test]
    fn doc_artifact_index_status_matches_policy_ledger() -> Result<(), String> {
        let mut ledger = BTreeMap::new();
        ledger.insert(
            "UNSAFE-REVIEW-SPEC-0026".to_string(),
            doc_artifact_entry("proposed"),
        );
        let mut index = BTreeMap::new();
        index.insert(
            "UNSAFE-REVIEW-SPEC-0026".to_string(),
            doc_artifact_entry("proposed"),
        );

        check_doc_artifacts_source_index_consistency(&ledger, &index)
    }

    #[test]
    fn doc_artifact_index_status_rejects_policy_ledger_drift() -> Result<(), String> {
        let mut ledger = BTreeMap::new();
        ledger.insert(
            "UNSAFE-REVIEW-SPEC-0026".to_string(),
            doc_artifact_entry("proposed"),
        );
        let mut index = BTreeMap::new();
        index.insert(
            "UNSAFE-REVIEW-SPEC-0026".to_string(),
            doc_artifact_entry("draft"),
        );

        let err = err_text(check_doc_artifacts_source_index_consistency(
            &ledger, &index,
        ))?;

        assert!(err.contains(".unsafe-review-spec/index.toml"));
        assert!(err.contains("UNSAFE-REVIEW-SPEC-0026"));
        assert!(err.contains("status `draft` must match"));
        Ok(())
    }

    #[test]
    fn spec_status_proof_commands_reject_unknown_xtask_commands() -> Result<(), String> {
        let Err(err) = spec_status::check_proof_commands(
            "UNSAFE-REVIEW-SPEC-0024",
            "`cargo run --locked -p xtask -- check-fake-thing`",
        ) else {
            return Err("unknown xtask commands should fail".to_string());
        };

        assert!(err.contains("check-fake-thing"));
        Ok(())
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
        let targets = markdown::link_targets(
            "[First use](docs/FIRST_USE.md) [external](https://example.com) [anchor](#trust)",
        );

        assert!(targets.contains(&"docs/FIRST_USE.md".to_string()));
        assert!(targets.contains(&"https://example.com".to_string()));
        assert!(targets.contains(&"#trust".to_string()));
        assert_eq!(
            markdown::local_link_target("docs/FIRST_USE.md#install"),
            Some("docs/FIRST_USE.md")
        );
        assert_eq!(markdown::local_link_target("https://example.com"), None);
        assert_eq!(markdown::local_link_target("#trust"), None);
    }

    #[test]
    fn markdown_code_span_parser_extracts_backticked_paths() {
        let spans = markdown::code_spans(
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
    fn manual_candidate_smoke_rejects_example_projection_drift() -> Result<(), String> {
        let mut actual = manual_candidate_fixture();
        let example = ManualCandidateExample {
            path: PathBuf::from("docs/examples/manual-candidates/textdecoder-sab.json"),
            id: "R4R2-S001".to_string(),
            expected: actual.clone(),
        };
        actual["safe_caller"] = serde_json::json!("unrelated JS route");

        let err = err_text(check_manual_candidate_smoke_entry_matches_example(
            &actual, &example,
        ))?;

        assert!(err.contains("safe_caller"), "{err}");
        assert!(err.contains("must match committed example"), "{err}");
        assert!(err.contains("unrelated JS route"), "{err}");
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
    fn dogfood_triage_report_accepts_known_labels() -> Result<(), String> {
        let text = r#"
## Triage observations

| Target | Card or family | Primary label | Evidence | Follow-up |
|---|---|---|---|---|
| `target` | `family` | `needs-fixture` | grounded observation | add a fixture |
| `target` | `family` | `noise` | broad card cluster | add ranking pressure |
| `target` | `family` | `needs-verifier` | projection rail can drift | add a checker |
"#;

        let rows = check_dogfood_report_triage_labels_text("docs/dogfood/reports/test.md", text)?;

        assert_eq!(rows, 3);
        Ok(())
    }

    #[test]
    fn dogfood_triage_report_rejects_unknown_labels() -> Result<(), String> {
        let text = r#"
## Triage observations

| Target | Card or family | Primary label | Evidence | Follow-up |
|---|---|---|---|---|
| `target` | `family` | `probably-actionable` | grounded observation | none |
"#;

        let err = err_text(check_dogfood_report_triage_labels_text(
            "docs/dogfood/reports/test.md",
            text,
        ))?;

        assert!(err.contains("unknown dogfood triage label"));
        assert!(err.contains("probably-actionable"));
        Ok(())
    }

    #[test]
    fn dogfood_triage_report_rejects_missing_required_fields() -> Result<(), String> {
        let text = r#"
## Triage observations

| Target | Card or family | Primary label | Evidence | Follow-up |
|---|---|---|---|---|
| `target` |  | `needs-fixture` | grounded observation | add a fixture |
"#;

        let err = err_text(check_dogfood_report_triage_labels_text(
            "docs/dogfood/reports/test.md",
            text,
        ))?;

        assert!(err.contains("non-empty Card or family column"));
        Ok(())
    }

    #[test]
    fn dogfood_triage_report_rejects_wrong_column_count() -> Result<(), String> {
        let text = r#"
## Triage observations

| Target | Card or family | Primary label | Evidence | Follow-up |
|---|---|---|---|---|
| `target` | `family` | `needs-fixture` | grounded observation |
"#;

        let err = err_text(check_dogfood_report_triage_labels_text(
            "docs/dogfood/reports/test.md",
            text,
        ))?;

        assert!(err.contains("Target, Card or family, Primary label, Evidence, and Follow-up"));
        Ok(())
    }

    #[test]
    fn dogfood_triage_report_rejects_unexpected_header() -> Result<(), String> {
        let text = r#"
## Triage observations

| Target | Family | Primary label | Evidence | Follow-up |
|---|---|---|---|---|
| `target` | `family` | `needs-fixture` | grounded observation | add a fixture |
"#;

        let err = err_text(check_dogfood_report_triage_labels_text(
            "docs/dogfood/reports/test.md",
            text,
        ))?;

        assert!(err.contains("dogfood triage header must be"));
        assert!(err.contains("Card or family"));
        Ok(())
    }

    fn dogfood_report_triage_keys_for_tests(
        rows: &[(&str, &str, &str)],
    ) -> BTreeMap<String, BTreeSet<(String, String)>> {
        let mut reports = BTreeMap::new();
        for (report, target, label) in rows {
            reports
                .entry((*report).to_string())
                .or_insert_with(BTreeSet::new)
                .insert(((*target).to_string(), (*label).to_string()));
        }
        reports
    }

    fn dogfood_follow_up_family_surface_set_for_tests() -> BTreeSet<String> {
        BTreeSet::from([
            "comment_plan".to_string(),
            "ffi".to_string(),
            "first_pr_projection".to_string(),
            "vec_set_len".to_string(),
        ])
    }

    fn dogfood_judgment_targets_for_tests() -> BTreeSet<String> {
        BTreeSet::from(["arrayvec-pr288".to_string(), "mio-pr1388".to_string()])
    }

    fn dogfood_judgment_families_for_tests() -> BTreeSet<String> {
        BTreeSet::from([
            "ffi".to_string(),
            "repair_queue".to_string(),
            "vec_set_len".to_string(),
        ])
    }

    fn dogfood_judgment_reports_for_tests() -> Vec<String> {
        vec![
            "2026-05-28-arrayvec-first-pr-projection-smoke.md".to_string(),
            "2026-05-26-mio-ffi-route-wording.md".to_string(),
        ]
    }

    fn dogfood_judgment_boundary_for_tests() -> &'static str {
        "Static unsafe contract review measurement input; not calibrated precision or recall, not a proof of memory safety, not UB-free status, not a Miri result, not site execution evidence, not witness adequacy, and not policy readiness."
    }

    #[test]
    fn dogfood_judgment_accepts_known_target_report_and_card_label() -> Result<(), String> {
        let text = format!(
            r#"
schema_version = "1.0"
target = "arrayvec-pr288"
report = "reports/2026-05-28-arrayvec-first-pr-projection-smoke.md"
reviewer = "manual"
date = "2026-05-31"
scope = "first-pr review packet"
trust_boundary = "{}"

[[cards]]
card_id = "UR-arrayvec-src-array-string-rs-from-byte-string-operation-vec_set_len-set-len-073a0fa631f6-initialized_memory-c1"
family = "vec_set_len"
judgment = "actionable"
reason = "The card names the initialized-memory obligation and a concrete next action."
next_step = "Record this as a usefulness sample without promoting calibration."
"#,
            dogfood_judgment_boundary_for_tests()
        );

        let rows = check_dogfood_judgment_text(
            "docs/dogfood/judgments/arrayvec-pr288.toml",
            &text,
            &dogfood_judgment_targets_for_tests(),
            &dogfood_judgment_families_for_tests(),
            &dogfood_judgment_reports_for_tests(),
        )?;

        assert_eq!(rows, 1);
        Ok(())
    }

    #[test]
    fn dogfood_judgment_accepts_missed_obligation_sample() -> Result<(), String> {
        let text = format!(
            r#"
schema_version = "1.0"
target = "mio-pr1388"
report = "reports/2026-05-26-mio-ffi-route-wording.md"
reviewer = "manual"
date = "2026-05-31"
scope = "manual missed-card review"
trust_boundary = "{}"

[[missed]]
file = "src/sys/unix/sockaddr.rs"
line = 42
expected_family = "ffi"
status = "open"
reason = "Manual review found a changed FFI boundary that needs a follow-up seed."
next_step = "Open one report-backed seed before changing analyzer behavior."
"#,
            dogfood_judgment_boundary_for_tests()
        );

        let rows = check_dogfood_judgment_text(
            "docs/dogfood/judgments/mio-pr1388.toml",
            &text,
            &dogfood_judgment_targets_for_tests(),
            &dogfood_judgment_families_for_tests(),
            &dogfood_judgment_reports_for_tests(),
        )?;

        assert_eq!(rows, 1);
        Ok(())
    }

    #[test]
    fn dogfood_judgment_accepts_card_ids_from_checked_artifact() -> Result<(), String> {
        let text = format!(
            r#"
schema_version = "1.0"
target = "arrayvec-pr288"
report = "reports/2026-05-28-arrayvec-first-pr-projection-smoke.md"
reviewer = "manual"
date = "2026-05-31"
scope = "first-pr review packet"
cards_artifact = "fixtures/vec_set_len_self_new_const_cap_not_guard/expected.cards.json"
trust_boundary = "{}"

[[cards]]
card_id = "UR-vec-set-len-self-new-const-cap-not-guard-src-lib-rs-from-array-operation-vec_set_len-set-len-3ba2e696cbd8-initialized_memory-c1"
family = "vec_set_len"
judgment = "actionable"
reason = "The artifact-backed card identifies the initialized-memory obligation."
next_step = "Use as a checked usefulness sample only."
"#,
            dogfood_judgment_boundary_for_tests()
        );

        let rows = check_dogfood_judgment_text(
            "docs/dogfood/judgments/arrayvec-pr288.toml",
            &text,
            &dogfood_judgment_targets_for_tests(),
            &dogfood_judgment_families_for_tests(),
            &dogfood_judgment_reports_for_tests(),
        )?;

        assert_eq!(rows, 1);
        Ok(())
    }

    #[test]
    fn dogfood_judgment_rejects_unknown_target() -> Result<(), String> {
        let text = format!(
            r#"
schema_version = "1.0"
target = "unknown-target"
report = "reports/2026-05-28-arrayvec-first-pr-projection-smoke.md"
reviewer = "manual"
date = "2026-05-31"
scope = "first-pr review packet"
trust_boundary = "{}"

[[cards]]
family = "vec_set_len"
judgment = "actionable"
reason = "The card has a concrete next action."
next_step = "Record a usefulness sample."
"#,
            dogfood_judgment_boundary_for_tests()
        );

        let err = err_text(check_dogfood_judgment_text(
            "docs/dogfood/judgments/unknown-target.toml",
            &text,
            &dogfood_judgment_targets_for_tests(),
            &dogfood_judgment_families_for_tests(),
            &dogfood_judgment_reports_for_tests(),
        ))?;

        assert!(err.contains("unknown dogfood target"));
        assert!(err.contains("unknown-target"));
        Ok(())
    }

    #[test]
    fn dogfood_judgment_rejects_unknown_judgment_label() -> Result<(), String> {
        let text = format!(
            r#"
schema_version = "1.0"
target = "arrayvec-pr288"
report = "reports/2026-05-28-arrayvec-first-pr-projection-smoke.md"
reviewer = "manual"
date = "2026-05-31"
scope = "first-pr review packet"
trust_boundary = "{}"

[[cards]]
family = "vec_set_len"
judgment = "maybe-useful"
reason = "The card has a concrete next action."
next_step = "Record a usefulness sample."
"#,
            dogfood_judgment_boundary_for_tests()
        );

        let err = err_text(check_dogfood_judgment_text(
            "docs/dogfood/judgments/arrayvec-pr288.toml",
            &text,
            &dogfood_judgment_targets_for_tests(),
            &dogfood_judgment_families_for_tests(),
            &dogfood_judgment_reports_for_tests(),
        ))?;

        assert!(err.contains("unknown judgment"));
        assert!(err.contains("maybe-useful"));
        Ok(())
    }

    #[test]
    fn dogfood_judgment_rejects_missing_report() -> Result<(), String> {
        let text = format!(
            r#"
schema_version = "1.0"
target = "arrayvec-pr288"
report = "reports/missing.md"
reviewer = "manual"
date = "2026-05-31"
scope = "first-pr review packet"
trust_boundary = "{}"

[[cards]]
family = "vec_set_len"
judgment = "actionable"
reason = "The card has a concrete next action."
next_step = "Record a usefulness sample."
"#,
            dogfood_judgment_boundary_for_tests()
        );

        let err = err_text(check_dogfood_judgment_text(
            "docs/dogfood/judgments/arrayvec-pr288.toml",
            &text,
            &dogfood_judgment_targets_for_tests(),
            &dogfood_judgment_families_for_tests(),
            &dogfood_judgment_reports_for_tests(),
        ))?;

        assert!(err.contains("links missing dogfood report"));
        assert!(err.contains("reports/missing.md"));
        Ok(())
    }

    #[test]
    fn dogfood_judgment_rejects_overclaim_wording() -> Result<(), String> {
        let text = format!(
            r#"
schema_version = "1.0"
target = "arrayvec-pr288"
report = "reports/2026-05-28-arrayvec-first-pr-projection-smoke.md"
reviewer = "manual"
date = "2026-05-31"
scope = "first-pr review packet"
trust_boundary = "{}"

[[cards]]
family = "vec_set_len"
judgment = "actionable"
reason = "The card makes this safe to merge."
next_step = "Record a usefulness sample."
"#,
            dogfood_judgment_boundary_for_tests()
        );

        let err = err_text(check_dogfood_judgment_text(
            "docs/dogfood/judgments/arrayvec-pr288.toml",
            &text,
            &dogfood_judgment_targets_for_tests(),
            &dogfood_judgment_families_for_tests(),
            &dogfood_judgment_reports_for_tests(),
        ))?;

        assert!(err.contains("safe to merge"));
        Ok(())
    }

    #[test]
    fn dogfood_judgment_rejects_card_id_missing_from_checked_artifact() -> Result<(), String> {
        let text = format!(
            r#"
schema_version = "1.0"
target = "arrayvec-pr288"
report = "reports/2026-05-28-arrayvec-first-pr-projection-smoke.md"
reviewer = "manual"
date = "2026-05-31"
scope = "first-pr review packet"
cards_artifact = "fixtures/vec_set_len_self_new_const_cap_not_guard/expected.cards.json"
trust_boundary = "{}"

[[cards]]
card_id = "UR-missing-card"
family = "vec_set_len"
judgment = "actionable"
reason = "The card has a concrete next action."
next_step = "Record a usefulness sample."
"#,
            dogfood_judgment_boundary_for_tests()
        );

        let err = err_text(check_dogfood_judgment_text(
            "docs/dogfood/judgments/arrayvec-pr288.toml",
            &text,
            &dogfood_judgment_targets_for_tests(),
            &dogfood_judgment_families_for_tests(),
            &dogfood_judgment_reports_for_tests(),
        ))?;

        assert!(err.contains("not present in cards_artifact"));
        assert!(err.contains("UR-missing-card"));
        Ok(())
    }

    #[test]
    fn dogfood_follow_up_seed_index_accepts_known_targets_labels_and_reports() -> Result<(), String>
    {
        let text = r#"
# Dogfood follow-up seed index

## Seeds

| Seed ID | Status | Target | Family/surface | Primary label | Source report | Next PR slice | Notes |
|---|---|---|---|---|---|---|---|
| `dogfood-arrayvec-set-len` | `done` | `arrayvec-pr288` | `vec_set_len` | `actionable` | [arrayvec rerun](reports/2026-05-26-arrayvec-vec-set-len-rerun.md) | `analysis: keep vec_set_len regression pressure` | Current follow-up is covered by fixture regression pressure. |
| `dogfood-mio-ffi-route` | `open` | `mio-pr1388` | `ffi` | `needs-route` | [mio route](reports/2026-05-26-mio-ffi-route-wording.md) | `analysis: split ffi route wording` | Ready for a narrow verifier PR; route stays advisory. |

## Trust boundary

Dogfood follow-up seeds are static advisory review notes. They are not a proof
of memory safety, not UB-free status, not Miri-clean status, not site execution
evidence, not calibrated precision or recall, not witness adequacy, and not
policy readiness.
"#;
        let targets = BTreeSet::from(["arrayvec-pr288".to_string(), "mio-pr1388".to_string()]);
        let reports = vec![
            "2026-05-26-arrayvec-vec-set-len-rerun.md".to_string(),
            "2026-05-26-mio-ffi-route-wording.md".to_string(),
        ];
        let report_triage_keys = dogfood_report_triage_keys_for_tests(&[
            (
                "2026-05-26-arrayvec-vec-set-len-rerun.md",
                "arrayvec-pr288",
                "actionable",
            ),
            (
                "2026-05-26-mio-ffi-route-wording.md",
                "mio-pr1388",
                "needs-route",
            ),
        ]);

        let rows = check_dogfood_follow_up_seeds_text(
            "docs/dogfood/follow-up-seeds.md",
            text,
            &targets,
            &dogfood_follow_up_family_surface_set_for_tests(),
            &reports,
            &report_triage_keys,
        )?;

        assert_eq!(rows, 2);
        Ok(())
    }

    #[test]
    fn dogfood_follow_up_seed_index_accepts_open_readiness_notes() -> Result<(), String> {
        let text = r#"
# Dogfood follow-up seed index

## Seeds

| Seed ID | Status | Target | Family/surface | Primary label | Source report | Next PR slice | Notes |
|---|---|---|---|---|---|---|---|
| `dogfood-open-ready` | `open` | `arrayvec-pr288` | `vec_set_len` | `actionable` | [arrayvec rerun](reports/2026-05-26-arrayvec-vec-set-len-rerun.md) | `analysis: keep vec_set_len regression pressure` | Ready for a narrow fixture control from the linked report. |

## Trust boundary

Dogfood follow-up seeds are static advisory review notes. They are not a proof
of memory safety, not UB-free status, not Miri-clean status, not site execution
evidence, not calibrated precision or recall, not witness adequacy, and not
policy readiness.
"#;
        let targets = BTreeSet::from(["arrayvec-pr288".to_string()]);
        let reports = vec!["2026-05-26-arrayvec-vec-set-len-rerun.md".to_string()];
        let report_triage_keys = dogfood_report_triage_keys_for_tests(&[(
            "2026-05-26-arrayvec-vec-set-len-rerun.md",
            "arrayvec-pr288",
            "actionable",
        )]);

        let rows = check_dogfood_follow_up_seeds_text(
            "docs/dogfood/follow-up-seeds.md",
            text,
            &targets,
            &dogfood_follow_up_family_surface_set_for_tests(),
            &reports,
            &report_triage_keys,
        )?;

        assert_eq!(rows, 1);
        Ok(())
    }

    #[test]
    fn dogfood_follow_up_seed_index_accepts_parked_future_pressure_notes() -> Result<(), String> {
        let text = r#"
# Dogfood follow-up seed index

## Seeds

| Seed ID | Status | Target | Family/surface | Primary label | Source report | Next PR slice | Notes |
|---|---|---|---|---|---|---|---|
| `dogfood-parked-future` | `parked` | `arrayvec-pr288` | `vec_set_len` | `actionable` | [arrayvec rerun](reports/2026-05-26-arrayvec-vec-set-len-rerun.md) | `analysis: wait for future dogfood pressure` | Parked for future pressure; add a fixture only when dogfood exposes a concrete stale or wrong-target shape. |

## Trust boundary

Dogfood follow-up seeds are static advisory review notes. They are not a proof
of memory safety, not UB-free status, not Miri-clean status, not site execution
evidence, not calibrated precision or recall, not witness adequacy, and not
policy readiness.
"#;
        let targets = BTreeSet::from(["arrayvec-pr288".to_string()]);
        let reports = vec!["2026-05-26-arrayvec-vec-set-len-rerun.md".to_string()];
        let report_triage_keys = dogfood_report_triage_keys_for_tests(&[(
            "2026-05-26-arrayvec-vec-set-len-rerun.md",
            "arrayvec-pr288",
            "actionable",
        )]);

        let rows = check_dogfood_follow_up_seeds_text(
            "docs/dogfood/follow-up-seeds.md",
            text,
            &targets,
            &dogfood_follow_up_family_surface_set_for_tests(),
            &reports,
            &report_triage_keys,
        )?;

        assert_eq!(rows, 1);
        Ok(())
    }

    #[test]
    fn dogfood_follow_up_seed_index_accepts_superseded_replacement_notes() -> Result<(), String> {
        let text = r#"
# Dogfood follow-up seed index

## Seeds

| Seed ID | Status | Target | Family/surface | Primary label | Source report | Next PR slice | Notes |
|---|---|---|---|---|---|---|---|
| `dogfood-superseded-seed` | `superseded` | `arrayvec-pr288` | `vec_set_len` | `actionable` | [arrayvec rerun](reports/2026-05-26-arrayvec-vec-set-len-rerun.md) | `analysis: keep vec_set_len regression pressure` | Superseded by `dogfood-arrayvec-set-len` after the newer report narrowed the fixture pressure. |

## Trust boundary

Dogfood follow-up seeds are static advisory review notes. They are not a proof
of memory safety, not UB-free status, not Miri-clean status, not site execution
evidence, not calibrated precision or recall, not witness adequacy, and not
policy readiness.
"#;
        let targets = BTreeSet::from(["arrayvec-pr288".to_string()]);
        let reports = vec!["2026-05-26-arrayvec-vec-set-len-rerun.md".to_string()];
        let report_triage_keys = dogfood_report_triage_keys_for_tests(&[(
            "2026-05-26-arrayvec-vec-set-len-rerun.md",
            "arrayvec-pr288",
            "actionable",
        )]);

        let rows = check_dogfood_follow_up_seeds_text(
            "docs/dogfood/follow-up-seeds.md",
            text,
            &targets,
            &dogfood_follow_up_family_surface_set_for_tests(),
            &reports,
            &report_triage_keys,
        )?;

        assert_eq!(rows, 1);
        Ok(())
    }

    #[test]
    fn dogfood_follow_up_status_glossary_accepts_closed_vocabulary() -> Result<(), String> {
        let text = r#"
# Dogfood follow-up seed index

Statuses:

- `open`: ready for a narrow swarm PR.
- `done`: the linked report's follow-up has landed.
- `parked`: recorded for future pressure.
- `superseded`: replaced by a newer seed or report.

## Seeds
"#;

        check_dogfood_follow_up_status_glossary("docs/dogfood/follow-up-seeds.md", text)
    }

    #[test]
    fn dogfood_follow_up_status_glossary_rejects_missing_status() -> Result<(), String> {
        let text = r#"
# Dogfood follow-up seed index

Statuses:

- `open`: ready for a narrow swarm PR.
- `done`: the linked report's follow-up has landed.
- `parked`: recorded for future pressure.

## Seeds
"#;

        let err = err_text(check_dogfood_follow_up_status_glossary(
            "docs/dogfood/follow-up-seeds.md",
            text,
        ))?;

        assert!(err.contains("must document dogfood follow-up status `superseded`"));
        Ok(())
    }

    #[test]
    fn dogfood_follow_up_status_glossary_rejects_unknown_status() -> Result<(), String> {
        let text = r#"
# Dogfood follow-up seed index

Statuses:

- `open`: ready for a narrow swarm PR.
- `done`: the linked report's follow-up has landed.
- `parked`: recorded for future pressure.
- `superseded`: replaced by a newer seed or report.
- `reviewing`: waiting on a reviewer.

## Seeds
"#;

        let err = err_text(check_dogfood_follow_up_status_glossary(
            "docs/dogfood/follow-up-seeds.md",
            text,
        ))?;

        assert!(err.contains("documents unknown dogfood follow-up status `reviewing`"));
        Ok(())
    }

    #[test]
    fn dogfood_follow_up_seed_index_rejects_open_without_readiness_note() -> Result<(), String> {
        let text = r#"
# Dogfood follow-up seed index

## Seeds

| Seed ID | Status | Target | Family/surface | Primary label | Source report | Next PR slice | Notes |
|---|---|---|---|---|---|---|---|
| `dogfood-open-without-readiness` | `open` | `arrayvec-pr288` | `vec_set_len` | `actionable` | [arrayvec rerun](reports/2026-05-26-arrayvec-vec-set-len-rerun.md) | `analysis: keep vec_set_len regression pressure` | no overclaim |

## Trust boundary

Dogfood follow-up seeds are static advisory review notes. They are not a proof
of memory safety, not UB-free status, not Miri-clean status, not site execution
evidence, not calibrated precision or recall, not witness adequacy, and not
policy readiness.
"#;
        let targets = BTreeSet::from(["arrayvec-pr288".to_string()]);
        let reports = vec!["2026-05-26-arrayvec-vec-set-len-rerun.md".to_string()];
        let report_triage_keys = dogfood_report_triage_keys_for_tests(&[(
            "2026-05-26-arrayvec-vec-set-len-rerun.md",
            "arrayvec-pr288",
            "actionable",
        )]);

        let err = err_text(check_dogfood_follow_up_seeds_text(
            "docs/dogfood/follow-up-seeds.md",
            text,
            &targets,
            &dogfood_follow_up_family_surface_set_for_tests(),
            &reports,
            &report_triage_keys,
        ))?;

        assert!(err.contains("open dogfood follow-up seed"));
        assert!(err.contains("ready for a narrow"));
        assert!(err.contains("dogfood-open-without-readiness"));
        Ok(())
    }

    #[test]
    fn dogfood_follow_up_seed_index_rejects_superseded_without_replacement_note()
    -> Result<(), String> {
        let text = r#"
# Dogfood follow-up seed index

## Seeds

| Seed ID | Status | Target | Family/surface | Primary label | Source report | Next PR slice | Notes |
|---|---|---|---|---|---|---|---|
| `dogfood-superseded-without-replacement` | `superseded` | `arrayvec-pr288` | `vec_set_len` | `actionable` | [arrayvec rerun](reports/2026-05-26-arrayvec-vec-set-len-rerun.md) | `analysis: keep vec_set_len regression pressure` | no longer current |

## Trust boundary

Dogfood follow-up seeds are static advisory review notes. They are not a proof
of memory safety, not UB-free status, not Miri-clean status, not site execution
evidence, not calibrated precision or recall, not witness adequacy, and not
policy readiness.
"#;
        let targets = BTreeSet::from(["arrayvec-pr288".to_string()]);
        let reports = vec!["2026-05-26-arrayvec-vec-set-len-rerun.md".to_string()];
        let report_triage_keys = dogfood_report_triage_keys_for_tests(&[(
            "2026-05-26-arrayvec-vec-set-len-rerun.md",
            "arrayvec-pr288",
            "actionable",
        )]);

        let err = err_text(check_dogfood_follow_up_seeds_text(
            "docs/dogfood/follow-up-seeds.md",
            text,
            &targets,
            &dogfood_follow_up_family_surface_set_for_tests(),
            &reports,
            &report_triage_keys,
        ))?;

        assert!(err.contains("superseded dogfood follow-up seed"));
        assert!(err.contains("newer seed or report"));
        assert!(err.contains("dogfood-superseded-without-replacement"));
        Ok(())
    }

    #[test]
    fn dogfood_follow_up_seed_index_rejects_done_without_coverage_note() -> Result<(), String> {
        let text = r#"
# Dogfood follow-up seed index

## Seeds

| Seed ID | Status | Target | Family/surface | Primary label | Source report | Next PR slice | Notes |
|---|---|---|---|---|---|---|---|
| `dogfood-done-without-coverage` | `done` | `arrayvec-pr288` | `vec_set_len` | `actionable` | [arrayvec rerun](reports/2026-05-26-arrayvec-vec-set-len-rerun.md) | `analysis: keep vec_set_len regression pressure` | no new analyzer breadth |

## Trust boundary

Dogfood follow-up seeds are static advisory review notes. They are not a proof
of memory safety, not UB-free status, not Miri-clean status, not site execution
evidence, not calibrated precision or recall, not witness adequacy, and not
policy readiness.
"#;
        let targets = BTreeSet::from(["arrayvec-pr288".to_string()]);
        let reports = vec!["2026-05-26-arrayvec-vec-set-len-rerun.md".to_string()];
        let report_triage_keys = dogfood_report_triage_keys_for_tests(&[(
            "2026-05-26-arrayvec-vec-set-len-rerun.md",
            "arrayvec-pr288",
            "actionable",
        )]);

        let err = err_text(check_dogfood_follow_up_seeds_text(
            "docs/dogfood/follow-up-seeds.md",
            text,
            &targets,
            &dogfood_follow_up_family_surface_set_for_tests(),
            &reports,
            &report_triage_keys,
        ))?;

        assert!(err.contains("done dogfood follow-up seed"));
        assert!(err.contains("landed, covers, or preserves"));
        assert!(err.contains("dogfood-done-without-coverage"));
        Ok(())
    }

    #[test]
    fn dogfood_follow_up_seed_index_rejects_unknown_status() -> Result<(), String> {
        let text = r#"
# Dogfood follow-up seed index

## Seeds

| Seed ID | Status | Target | Family/surface | Primary label | Source report | Next PR slice | Notes |
|---|---|---|---|---|---|---|---|
| `dogfood-arrayvec-set-len` | `reviewing` | `arrayvec-pr288` | `vec_set_len` | `actionable` | [arrayvec rerun](reports/2026-05-26-arrayvec-vec-set-len-rerun.md) | `analysis: keep vec_set_len regression pressure` | no new analyzer breadth |

## Trust boundary

Dogfood follow-up seeds are static advisory review notes. They are not a proof
of memory safety, not UB-free status, not Miri-clean status, not site execution
evidence, not calibrated precision or recall, not witness adequacy, and not
policy readiness.
"#;
        let targets = BTreeSet::from(["arrayvec-pr288".to_string()]);
        let reports = vec!["2026-05-26-arrayvec-vec-set-len-rerun.md".to_string()];
        let report_triage_keys = dogfood_report_triage_keys_for_tests(&[(
            "2026-05-26-arrayvec-vec-set-len-rerun.md",
            "arrayvec-pr288",
            "actionable",
        )]);

        let err = err_text(check_dogfood_follow_up_seeds_text(
            "docs/dogfood/follow-up-seeds.md",
            text,
            &targets,
            &dogfood_follow_up_family_surface_set_for_tests(),
            &reports,
            &report_triage_keys,
        ))?;

        assert!(err.contains("unknown dogfood follow-up status"));
        assert!(err.contains("reviewing"));
        Ok(())
    }

    #[test]
    fn dogfood_follow_up_seed_index_rejects_parked_without_scope_boundary() -> Result<(), String> {
        let text = r#"
# Dogfood follow-up seed index

## Seeds

| Seed ID | Status | Target | Family/surface | Primary label | Source report | Next PR slice | Notes |
|---|---|---|---|---|---|---|---|
| `dogfood-parked-ready-work` | `parked` | `arrayvec-pr288` | `vec_set_len` | `actionable` | [arrayvec rerun](reports/2026-05-26-arrayvec-vec-set-len-rerun.md) | `analysis: wait for fixture pressure` | ready to implement this recognizer now |

## Trust boundary

Dogfood follow-up seeds are static advisory review notes. They are not a proof
of memory safety, not UB-free status, not Miri-clean status, not site execution
evidence, not calibrated precision or recall, not witness adequacy, and not
policy readiness.
"#;
        let targets = BTreeSet::from(["arrayvec-pr288".to_string()]);
        let reports = vec!["2026-05-26-arrayvec-vec-set-len-rerun.md".to_string()];
        let report_triage_keys = dogfood_report_triage_keys_for_tests(&[(
            "2026-05-26-arrayvec-vec-set-len-rerun.md",
            "arrayvec-pr288",
            "actionable",
        )]);

        let err = err_text(check_dogfood_follow_up_seeds_text(
            "docs/dogfood/follow-up-seeds.md",
            text,
            &targets,
            &dogfood_follow_up_family_surface_set_for_tests(),
            &reports,
            &report_triage_keys,
        ))?;

        assert!(err.contains("parked dogfood follow-up seed"));
        assert!(err.contains("future pressure"));
        assert!(err.contains("dogfood-parked-ready-work"));
        Ok(())
    }

    #[test]
    fn dogfood_follow_up_seed_index_rejects_broad_next_pr_slice() -> Result<(), String> {
        let text = r#"
# Dogfood follow-up seed index

## Seeds

| Seed ID | Status | Target | Family/surface | Primary label | Source report | Next PR slice | Notes |
|---|---|---|---|---|---|---|---|
| `dogfood-broad-slice` | `open` | `arrayvec-pr288` | `vec_set_len` | `actionable` | [arrayvec rerun](reports/2026-05-26-arrayvec-vec-set-len-rerun.md) | `analysis: broad analyzer expansion across all families` | keep review boundary |

## Trust boundary

Dogfood follow-up seeds are static advisory review notes. They are not a proof
of memory safety, not UB-free status, not Miri-clean status, not site execution
evidence, not calibrated precision or recall, not witness adequacy, and not
policy readiness.
"#;
        let targets = BTreeSet::from(["arrayvec-pr288".to_string()]);
        let reports = vec!["2026-05-26-arrayvec-vec-set-len-rerun.md".to_string()];
        let report_triage_keys = dogfood_report_triage_keys_for_tests(&[(
            "2026-05-26-arrayvec-vec-set-len-rerun.md",
            "arrayvec-pr288",
            "actionable",
        )]);

        let err = err_text(check_dogfood_follow_up_seeds_text(
            "docs/dogfood/follow-up-seeds.md",
            text,
            &targets,
            &dogfood_follow_up_family_surface_set_for_tests(),
            &reports,
            &report_triage_keys,
        ))?;

        assert!(err.contains("next PR slice must stay narrow"));
        assert!(err.contains("broad analyzer"));
        assert!(err.contains("dogfood-broad-slice"));
        Ok(())
    }

    #[test]
    fn dogfood_follow_up_seed_index_rejects_unknown_triage_label() -> Result<(), String> {
        let text = r#"
# Dogfood follow-up seed index

## Seeds

| Seed ID | Status | Target | Family/surface | Primary label | Source report | Next PR slice | Notes |
|---|---|---|---|---|---|---|---|
| `dogfood-arrayvec-set-len` | `open` | `arrayvec-pr288` | `vec_set_len` | `maybe-useful` | [arrayvec rerun](reports/2026-05-26-arrayvec-vec-set-len-rerun.md) | `analysis: keep vec_set_len regression pressure` | no new analyzer breadth |

## Trust boundary

Dogfood follow-up seeds are static advisory review notes. They are not a proof
of memory safety, not UB-free status, not Miri-clean status, not site execution
evidence, not calibrated precision or recall, not witness adequacy, and not
policy readiness.
"#;
        let targets = BTreeSet::from(["arrayvec-pr288".to_string()]);
        let reports = vec!["2026-05-26-arrayvec-vec-set-len-rerun.md".to_string()];
        let report_triage_keys = dogfood_report_triage_keys_for_tests(&[(
            "2026-05-26-arrayvec-vec-set-len-rerun.md",
            "arrayvec-pr288",
            "actionable",
        )]);

        let err = err_text(check_dogfood_follow_up_seeds_text(
            "docs/dogfood/follow-up-seeds.md",
            text,
            &targets,
            &dogfood_follow_up_family_surface_set_for_tests(),
            &reports,
            &report_triage_keys,
        ))?;

        assert!(err.contains("unknown dogfood follow-up label"));
        assert!(err.contains("maybe-useful"));
        Ok(())
    }

    #[test]
    fn dogfood_follow_up_seed_index_rejects_duplicate_seed_ids() -> Result<(), String> {
        let text = r#"
# Dogfood follow-up seed index

## Seeds

| Seed ID | Status | Target | Family/surface | Primary label | Source report | Next PR slice | Notes |
|---|---|---|---|---|---|---|---|
| `dogfood-arrayvec-set-len` | `open` | `arrayvec-pr288` | `vec_set_len` | `actionable` | [arrayvec rerun](reports/2026-05-26-arrayvec-vec-set-len-rerun.md) | `analysis: keep vec_set_len regression pressure` | Ready for a narrow fixture control. |
| `dogfood-arrayvec-set-len` | `done` | `mio-pr1388` | `ffi` | `needs-route` | [mio route](reports/2026-05-26-mio-ffi-route-wording.md) | `analysis: keep ffi route wording` | route stays advisory |

## Trust boundary

Dogfood follow-up seeds are static advisory review notes. They are not a proof
of memory safety, not UB-free status, not Miri-clean status, not site execution
evidence, not calibrated precision or recall, not witness adequacy, and not
policy readiness.
"#;
        let targets = BTreeSet::from(["arrayvec-pr288".to_string(), "mio-pr1388".to_string()]);
        let reports = vec![
            "2026-05-26-arrayvec-vec-set-len-rerun.md".to_string(),
            "2026-05-26-mio-ffi-route-wording.md".to_string(),
        ];
        let report_triage_keys = dogfood_report_triage_keys_for_tests(&[
            (
                "2026-05-26-arrayvec-vec-set-len-rerun.md",
                "arrayvec-pr288",
                "actionable",
            ),
            (
                "2026-05-26-mio-ffi-route-wording.md",
                "mio-pr1388",
                "needs-route",
            ),
        ]);

        let err = err_text(check_dogfood_follow_up_seeds_text(
            "docs/dogfood/follow-up-seeds.md",
            text,
            &targets,
            &dogfood_follow_up_family_surface_set_for_tests(),
            &reports,
            &report_triage_keys,
        ))?;

        assert!(err.contains("duplicate dogfood follow-up seed id"));
        assert!(err.contains("dogfood-arrayvec-set-len"));
        Ok(())
    }

    #[test]
    fn dogfood_follow_up_seed_index_rejects_unknown_targets() -> Result<(), String> {
        let text = r#"
# Dogfood follow-up seed index

## Seeds

| Seed ID | Status | Target | Family/surface | Primary label | Source report | Next PR slice | Notes |
|---|---|---|---|---|---|---|---|
| `dogfood-missing` | `open` | `missing-target` | `vec_set_len` | `needs-fixture` | [report](reports/2026-05-26-post-burst.md) | `analysis: add fixture` | no overclaim |

## Trust boundary

Dogfood follow-up seeds are static advisory review notes. They are not a proof
of memory safety, not UB-free status, not Miri-clean status, not site execution
evidence, not calibrated precision or recall, not witness adequacy, and not
policy readiness.
"#;
        let targets = BTreeSet::from(["arrayvec-pr288".to_string()]);
        let reports = vec!["2026-05-26-post-burst.md".to_string()];
        let report_triage_keys = dogfood_report_triage_keys_for_tests(&[(
            "2026-05-26-post-burst.md",
            "arrayvec-pr288",
            "actionable",
        )]);

        let err = err_text(check_dogfood_follow_up_seeds_text(
            "docs/dogfood/follow-up-seeds.md",
            text,
            &targets,
            &dogfood_follow_up_family_surface_set_for_tests(),
            &reports,
            &report_triage_keys,
        ))?;

        assert!(err.contains("unknown target"));
        assert!(err.contains("missing-target"));
        Ok(())
    }

    #[test]
    fn dogfood_follow_up_seed_index_rejects_unknown_family_or_surface() -> Result<(), String> {
        let text = r#"
# Dogfood follow-up seed index

## Seeds

| Seed ID | Status | Target | Family/surface | Primary label | Source report | Next PR slice | Notes |
|---|---|---|---|---|---|---|---|
| `dogfood-unknown-family` | `open` | `arrayvec-pr288` | `unknown_surface` | `needs-fixture` | [report](reports/2026-05-26-post-burst.md) | `analysis: add fixture` | no overclaim |

## Trust boundary

Dogfood follow-up seeds are static advisory review notes. They are not a proof
of memory safety, not UB-free status, not Miri-clean status, not site execution
evidence, not calibrated precision or recall, not witness adequacy, and not
policy readiness.
"#;
        let targets = BTreeSet::from(["arrayvec-pr288".to_string()]);
        let reports = vec!["2026-05-26-post-burst.md".to_string()];
        let report_triage_keys = dogfood_report_triage_keys_for_tests(&[(
            "2026-05-26-post-burst.md",
            "arrayvec-pr288",
            "needs-fixture",
        )]);

        let err = err_text(check_dogfood_follow_up_seeds_text(
            "docs/dogfood/follow-up-seeds.md",
            text,
            &targets,
            &dogfood_follow_up_family_surface_set_for_tests(),
            &reports,
            &report_triage_keys,
        ))?;

        assert!(err.contains("unknown family/surface"));
        assert!(err.contains("unknown_surface"));
        Ok(())
    }

    #[test]
    fn dogfood_follow_up_seed_index_rejects_missing_report_links() -> Result<(), String> {
        let text = r#"
# Dogfood follow-up seed index

## Seeds

| Seed ID | Status | Target | Family/surface | Primary label | Source report | Next PR slice | Notes |
|---|---|---|---|---|---|---|---|
| `dogfood-missing-report` | `open` | `arrayvec-pr288` | `vec_set_len` | `needs-fixture` | [report](reports/missing.md) | `analysis: add fixture` | no overclaim |

## Trust boundary

Dogfood follow-up seeds are static advisory review notes. They are not a proof
of memory safety, not UB-free status, not Miri-clean status, not site execution
evidence, not calibrated precision or recall, not witness adequacy, and not
policy readiness.
"#;
        let targets = BTreeSet::from(["arrayvec-pr288".to_string()]);
        let reports = vec!["2026-05-26-post-burst.md".to_string()];
        let report_triage_keys = dogfood_report_triage_keys_for_tests(&[(
            "2026-05-26-post-burst.md",
            "arrayvec-pr288",
            "actionable",
        )]);

        let err = err_text(check_dogfood_follow_up_seeds_text(
            "docs/dogfood/follow-up-seeds.md",
            text,
            &targets,
            &dogfood_follow_up_family_surface_set_for_tests(),
            &reports,
            &report_triage_keys,
        ))?;

        assert!(err.contains("links missing report"));
        assert!(err.contains("reports/missing.md"));
        Ok(())
    }

    #[test]
    fn dogfood_follow_up_seed_index_rejects_report_links_outside_reports_dir() -> Result<(), String>
    {
        let text = r#"
# Dogfood follow-up seed index

## Seeds

| Seed ID | Status | Target | Family/surface | Primary label | Source report | Next PR slice | Notes |
|---|---|---|---|---|---|---|---|
| `dogfood-outside-report-dir` | `open` | `arrayvec-pr288` | `vec_set_len` | `needs-fixture` | [report](../handoffs/2026-05-26-post-burst-analyzer-audit.md) | `analysis: add fixture` | no overclaim |

## Trust boundary

Dogfood follow-up seeds are static advisory review notes. They are not a proof
of memory safety, not UB-free status, not Miri-clean status, not site execution
evidence, not calibrated precision or recall, not witness adequacy, and not
policy readiness.
"#;
        let targets = BTreeSet::from(["arrayvec-pr288".to_string()]);
        let reports = vec!["2026-05-26-post-burst.md".to_string()];
        let report_triage_keys = dogfood_report_triage_keys_for_tests(&[(
            "2026-05-26-post-burst.md",
            "arrayvec-pr288",
            "needs-fixture",
        )]);

        let err = err_text(check_dogfood_follow_up_seeds_text(
            "docs/dogfood/follow-up-seeds.md",
            text,
            &targets,
            &dogfood_follow_up_family_surface_set_for_tests(),
            &reports,
            &report_triage_keys,
        ))?;

        assert!(err.contains("source report must link under reports/"));
        assert!(err.contains("dogfood-outside-report-dir"));
        Ok(())
    }

    #[test]
    fn dogfood_follow_up_seed_index_rejects_report_without_parsed_triage_keys() -> Result<(), String>
    {
        let text = r#"
# Dogfood follow-up seed index

## Seeds

| Seed ID | Status | Target | Family/surface | Primary label | Source report | Next PR slice | Notes |
|---|---|---|---|---|---|---|---|
| `dogfood-report-without-triage` | `open` | `arrayvec-pr288` | `vec_set_len` | `needs-fixture` | [report](reports/2026-05-26-post-burst.md) | `analysis: add fixture` | no overclaim |

## Trust boundary

Dogfood follow-up seeds are static advisory review notes. They are not a proof
of memory safety, not UB-free status, not Miri-clean status, not site execution
evidence, not calibrated precision or recall, not witness adequacy, and not
policy readiness.
"#;
        let targets = BTreeSet::from(["arrayvec-pr288".to_string()]);
        let reports = vec!["2026-05-26-post-burst.md".to_string()];
        let report_triage_keys = BTreeMap::new();

        let err = err_text(check_dogfood_follow_up_seeds_text(
            "docs/dogfood/follow-up-seeds.md",
            text,
            &targets,
            &dogfood_follow_up_family_surface_set_for_tests(),
            &reports,
            &report_triage_keys,
        ))?;

        assert!(err.contains("without parsed triage keys"));
        assert!(err.contains("dogfood-report-without-triage"));
        assert!(err.contains("2026-05-26-post-burst.md"));
        Ok(())
    }

    #[test]
    fn dogfood_follow_up_seed_index_rejects_report_without_matching_triage_row()
    -> Result<(), String> {
        let text = r#"
# Dogfood follow-up seed index

## Seeds

| Seed ID | Status | Target | Family/surface | Primary label | Source report | Next PR slice | Notes |
|---|---|---|---|---|---|---|---|
| `dogfood-wrong-report` | `open` | `arrayvec-pr288` | `vec_set_len` | `needs-fixture` | [report](reports/2026-05-26-post-burst.md) | `analysis: add fixture` | no overclaim |

## Trust boundary

Dogfood follow-up seeds are static advisory review notes. They are not a proof
of memory safety, not UB-free status, not Miri-clean status, not site execution
evidence, not calibrated precision or recall, not witness adequacy, and not
policy readiness.
"#;
        let targets = BTreeSet::from(["arrayvec-pr288".to_string()]);
        let reports = vec!["2026-05-26-post-burst.md".to_string()];
        let report_triage_keys = dogfood_report_triage_keys_for_tests(&[(
            "2026-05-26-post-burst.md",
            "arrayvec-pr288",
            "actionable",
        )]);

        let err = err_text(check_dogfood_follow_up_seeds_text(
            "docs/dogfood/follow-up-seeds.md",
            text,
            &targets,
            &dogfood_follow_up_family_surface_set_for_tests(),
            &reports,
            &report_triage_keys,
        ))?;

        assert!(err.contains("must include a triage row"));
        assert!(err.contains("dogfood-wrong-report"));
        assert!(err.contains("arrayvec-pr288"));
        assert!(err.contains("needs-fixture"));
        Ok(())
    }

    #[test]
    fn dogfood_report_trust_boundary_accepts_advisory_limits() -> Result<(), String> {
        let text = r#"
## Trust boundary

This report records static advisory review evidence. It is not memory-safety
proof, UB-free status, Miri-clean status, site-execution proof, calibrated
precision or recall, witness adequacy, or policy readiness.
"#;

        check_dogfood_report_trust_boundary_text("docs/dogfood/reports/test.md", text)
    }

    #[test]
    fn dogfood_report_trust_boundary_rejects_missing_witness_limits() -> Result<(), String> {
        let text = r#"
## Trust boundary

This report records static advisory review evidence. It is not memory-safety
proof, UB-free status, Miri-clean status, site-execution proof, calibrated
precision or recall, or policy readiness.
"#;

        let err = err_text(check_dogfood_report_trust_boundary_text(
            "docs/dogfood/reports/test.md",
            text,
        ))?;

        assert!(err.contains("witness limits"));
        Ok(())
    }

    #[test]
    fn dogfood_report_trust_boundary_rejects_missing_policy_limits() -> Result<(), String> {
        let text = r#"
## Trust boundary

This report records static advisory review evidence. It is not memory-safety
proof, UB-free status, Miri-clean status, site-execution proof, calibrated
precision or recall, or witness adequacy.
"#;

        let err = err_text(check_dogfood_report_trust_boundary_text(
            "docs/dogfood/reports/test.md",
            text,
        ))?;

        assert!(err.contains("`policy` limits"));
        Ok(())
    }

    #[test]
    fn dogfood_report_overclaim_rejects_all_clear() -> Result<(), String> {
        let text = r#"
# Dogfood report

All clear.

## Trust boundary

This report records static advisory review evidence. It is not memory-safety
proof, UB-free status, Miri-clean status, site-execution proof, calibrated
precision or recall, witness adequacy, or policy readiness.
"#;

        let err = err_text(reject_positive_overclaims(
            Path::new("docs/dogfood/reports/test.md"),
            text,
        ))?;

        assert!(err.contains("all clear"));
        Ok(())
    }

    #[test]
    fn dogfood_report_index_requires_every_report_link() -> Result<(), String> {
        let reports = vec![
            "2026-05-26-post-burst.md".to_string(),
            "2026-05-26-no-card-control.md".to_string(),
        ];
        let text = r#"
Snapshot reports:

- [post burst](reports/2026-05-26-post-burst.md)
- [no-card control](reports/2026-05-26-no-card-control.md)
"#;

        check_dogfood_report_index_text("docs/dogfood/README.md", text, &reports)
    }

    #[test]
    fn dogfood_report_index_rejects_missing_report_link() -> Result<(), String> {
        let reports = vec![
            "2026-05-26-post-burst.md".to_string(),
            "2026-05-26-no-card-control.md".to_string(),
        ];
        let text = r#"
Snapshot reports:

- [post burst](reports/2026-05-26-post-burst.md)
"#;

        let err = err_text(check_dogfood_report_index_text(
            "docs/dogfood/README.md",
            text,
            &reports,
        ))?;

        assert!(err.contains("must link dogfood report"));
        assert!(err.contains("reports/2026-05-26-no-card-control.md"));
        Ok(())
    }

    #[test]
    fn dogfood_report_index_rejects_stale_report_link() -> Result<(), String> {
        let reports = vec![
            "2026-05-26-post-burst.md".to_string(),
            "2026-05-26-no-card-control.md".to_string(),
        ];
        let text = r#"
Snapshot reports:

- [post burst](reports/2026-05-26-post-burst.md)
- [no-card control](reports/2026-05-26-no-card-control.md)
- [stale report](reports/2026-05-26-missing-report.md)
"#;

        let err = err_text(check_dogfood_report_index_text(
            "docs/dogfood/README.md",
            text,
            &reports,
        ))?;

        assert!(err.contains("links missing dogfood report"));
        assert!(err.contains("reports/2026-05-26-missing-report.md"));
        Ok(())
    }

    #[test]
    fn public_badge_endpoints_match_readme_and_json() -> Result<(), String> {
        public_badges::check_endpoints()
    }

    #[test]
    fn public_badge_endpoints_match_generated_repo_projection() -> Result<(), String> {
        public_badges::check_generated_projection()
    }

    #[test]
    fn public_surface_checker_validates_current_contract() -> Result<(), String> {
        public_surfaces::check_impl().map(|_| ())
    }

    #[test]
    fn public_surface_boundary_requires_negative_claim_limit() {
        assert!(public_surfaces::has_trust_boundary(
            "This is advisory review evidence, not memory-safety proof."
        ));
        assert!(public_surfaces::has_trust_boundary(
            "The command does not run Miri or enable blocking policy by default."
        ));
        assert!(!public_surfaces::has_trust_boundary(
            "This command proves the reviewed code is safe."
        ));
        assert!(!public_surfaces::has_trust_boundary(
            "Install this command to review pull requests."
        ));
    }

    #[test]
    fn positive_overclaim_rejects_policy_and_calibration_claims() {
        for forbidden in [
            "verified safe",
            "proved sound",
            "proof of safety",
            "blocking-ready",
            "calibrated precision",
            "calibrated recall",
        ] {
            let text = format!("This artifact is {forbidden}.");
            let err = reject_positive_overclaims(Path::new("artifact.md"), &text)
                .err()
                .unwrap_or_default();
            assert!(
                err.contains(forbidden),
                "expected `{forbidden}` rejection, got `{err}`"
            );
        }
    }

    #[test]
    fn positive_overclaim_allows_explicit_negative_calibration_context() -> Result<(), String> {
        reject_positive_overclaims(
            Path::new("artifact.md"),
            "This is not a calibrated precision claim.\nThis is not blocking-ready.",
        )
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
    fn advisory_artifact_checker_rejects_cards_json_positive_overclaim() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-cards-json-overclaim")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        let path = dir.join("cards.json");
        let mut cards = parse_json_file(&path)?;
        cards["note"] = serde_json::json!("safe to merge");
        fs::write(
            &path,
            serde_json::to_string(&cards)
                .map_err(|err| format!("serialize cards json failed: {err}"))?,
        )
        .map_err(|err| format!("write cards failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains("safe to merge"));
        Ok(())
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
    fn first_pr_artifact_checker_rejects_lsp_status_count_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-status-count-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let lsp_path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&lsp_path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        lsp["status"]["cards"] = serde_json::json!(2);
        fs::write(&lsp_path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("lsp.json status cards")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_status_message_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-status-message-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let lsp_path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&lsp_path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        lsp["status"]["message"] =
            serde_json::json!("1 unsafe-review card(s), 0 open actionable gap(s)");
        fs::write(&lsp_path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("lsp.json status message")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_review_kit_unknown_top_card() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-review-kit-unknown-top-card")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("review-kit.json");
        let mut review_kit: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read review kit failed: {err}"))?,
        )
        .map_err(|err| format!("parse review kit failed: {err}"))?;
        review_kit["top_card_id"] = serde_json::json!("missing-card");
        fs::write(&path, review_kit.to_string())
            .map_err(|err| format!("write review kit failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let err = match result {
            Ok(()) => {
                return Err("unknown top card should fail review-kit verification".to_string());
            }
            Err(err) => err,
        };
        assert!(
            err.contains("review-kit.json top_card_id `missing-card` is not present"),
            "{err}"
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_review_kit_handoff_context_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-review-kit-handoff-context-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("review-kit.json");
        let mut review_kit: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read review kit failed: {err}"))?,
        )
        .map_err(|err| format!("parse review kit failed: {err}"))?;
        review_kit["handoff"]["top_card"]["context_json"] =
            serde_json::json!("unsafe-review context missing-card --json");
        fs::write(&path, review_kit.to_string())
            .map_err(|err| format!("write review kit failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let err = match result {
            Ok(()) => {
                return Err("top-card handoff drift should fail verification".to_string());
            }
            Err(err) => err,
        };
        assert!(
            err.contains("review-kit.json handoff top_card context_json"),
            "{err}"
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_review_kit_review_card_queue_id_drift()
    -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-review-kit-review-card-queue-id-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("review-kit.json");
        let mut review_kit = parse_json_file(&path)?;
        let entry = &mut review_kit["handoff"]["review_cards"]["card_queue"][0];
        entry["card_id"] = serde_json::json!("missing-card");
        entry["explain"] = serde_json::json!("unsafe-review explain missing-card");
        entry["context_json"] = serde_json::json!("unsafe-review context missing-card --json");
        fs::write(&path, review_kit.to_string())
            .map_err(|err| format!("write review kit failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let err = match result {
            Ok(()) => {
                return Err("ReviewCard queue id drift should fail verification".to_string());
            }
            Err(err) => err,
        };
        assert!(
            err.contains(
                "review-kit.json handoff review_cards card_queue[0] card_id `missing-card` must match cards.json card `card-1`"
            ),
            "{err}"
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_review_kit_review_card_queue_projection_drift()
    -> Result<(), String> {
        let dir = unique_temp_dir(
            "unsafe-review-first-pr-review-kit-review-card-queue-projection-drift",
        )?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("review-kit.json");
        let mut review_kit = parse_json_file(&path)?;
        review_kit["handoff"]["review_cards"]["card_queue"][0]["operation_family"] =
            serde_json::json!("slice_from_raw_parts");
        fs::write(&path, review_kit.to_string())
            .map_err(|err| format!("write review kit failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let err = match result {
            Ok(()) => {
                return Err(
                    "ReviewCard queue cards.json projection drift should fail verification"
                        .to_string(),
                );
            }
            Err(err) => err,
        };
        assert!(
            err.contains("review-kit.json handoff review_cards card_queue[0] operation_family"),
            "{err}"
        );
        assert!(err.contains("raw_pointer_read"), "{err}");
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_review_kit_review_card_queue_verify_command_drift()
    -> Result<(), String> {
        let dir = unique_temp_dir(
            "unsafe-review-first-pr-review-kit-review-card-queue-verify-command-drift",
        )?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("review-kit.json");
        let mut review_kit = parse_json_file(&path)?;
        review_kit["handoff"]["review_cards"]["card_queue"][0]["verify_commands"][0] =
            serde_json::json!("cargo test unrelated");
        fs::write(&path, review_kit.to_string())
            .map_err(|err| format!("write review kit failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let err = match result {
            Ok(()) => {
                return Err(
                    "ReviewCard queue verify command drift should fail verification".to_string(),
                );
            }
            Err(err) => err,
        };
        assert!(
            err.contains("review-kit.json handoff review_cards card_queue[0] verify_commands"),
            "{err}"
        );
        assert!(err.contains("cargo +nightly miri test card"), "{err}");
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_review_kit_review_card_queue_witness_route_drift()
    -> Result<(), String> {
        let dir = unique_temp_dir(
            "unsafe-review-first-pr-review-kit-review-card-queue-witness-route-drift",
        )?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("review-kit.json");
        let mut review_kit = parse_json_file(&path)?;
        review_kit["handoff"]["review_cards"]["card_queue"][0]["witness_routes"][0]["kind"] =
            serde_json::json!("asan");
        fs::write(&path, review_kit.to_string())
            .map_err(|err| format!("write review kit failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let err = match result {
            Ok(()) => {
                return Err(
                    "ReviewCard queue witness route drift should fail verification".to_string(),
                );
            }
            Err(err) => err,
        };
        assert!(
            err.contains(
                "review-kit.json handoff review_cards card_queue[0] witness_routes[0] kind"
            ),
            "{err}"
        );
        assert!(err.contains("miri"), "{err}");
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_review_kit_review_card_queue_repair_drift()
    -> Result<(), String> {
        let dir =
            unique_temp_dir("unsafe-review-first-pr-review-kit-review-card-queue-repair-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("review-kit.json");
        let mut review_kit = parse_json_file(&path)?;
        review_kit["handoff"]["review_cards"]["card_queue"][0]["repair_queue_buckets"] =
            serde_json::json!(["repairable_by_safety_docs"]);
        fs::write(&path, review_kit.to_string())
            .map_err(|err| format!("write review kit failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let err = match result {
            Ok(()) => {
                return Err(
                    "ReviewCard queue repair-queue projection drift should fail verification"
                        .to_string(),
                );
            }
            Err(err) => err,
        };
        assert!(
            err.contains("review-kit.json handoff review_cards card_queue[0] repair_queue_buckets"),
            "{err}"
        );
        assert!(err.contains("requires_witness_receipt"), "{err}");
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_review_kit_receipt_handoff_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-review-kit-receipt-handoff-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("review-kit.json");
        let mut review_kit: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read review kit failed: {err}"))?,
        )
        .map_err(|err| format!("parse review kit failed: {err}"))?;
        review_kit["handoff"]["receipt_audit_markdown"] =
            serde_json::json!("cargo +nightly miri test");
        fs::write(&path, review_kit.to_string())
            .map_err(|err| format!("write review kit failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let err = match result {
            Ok(()) => {
                return Err("receipt-audit handoff drift should fail verification".to_string());
            }
            Err(err) => err,
        };
        assert!(
            err.contains("review-kit.json handoff receipt_audit_markdown"),
            "{err}"
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_review_kit_changed_file_count_drift() -> Result<(), String>
    {
        let dir = unique_temp_dir("unsafe-review-first-pr-review-kit-changed-count-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("review-kit.json");
        let mut review_kit = parse_json_file(&path)?;
        review_kit["summary"]["changed_non_rust_files"] = serde_json::json!(7);
        fs::write(&path, review_kit.to_string())
            .map_err(|err| format!("write review kit failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let err = match result {
            Ok(()) => {
                return Err(
                    "changed-file count drift should fail review-kit verification".to_string(),
                );
            }
            Err(err) => err,
        };
        assert!(
            err.contains(
                "review-kit.json summary.changed_non_rust_files is 7, but cards.json summary.changed_non_rust_files is 0"
            ),
            "{err}"
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_repair_queue_changed_file_count_drift()
    -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-repair-queue-changed-count-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("repair-queue.json");
        let mut repair_queue = parse_json_file(&path)?;
        repair_queue["summary"]["changed_non_rust_files"] = serde_json::json!(7);
        fs::write(&path, repair_queue.to_string())
            .map_err(|err| format!("write repair queue failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let err = match result {
            Ok(()) => {
                return Err(
                    "changed-file count drift should fail repair-queue verification".to_string(),
                );
            }
            Err(err) => err,
        };
        assert!(
            err.contains(
                "repair-queue.json summary.changed_non_rust_files is 7, but cards.json summary.changed_non_rust_files is 0"
            ),
            "{err}"
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_review_kit_manual_candidate_count_drift()
    -> Result<(), String> {
        let dir =
            unique_temp_dir("unsafe-review-first-pr-review-kit-manual-candidate-count-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_one_manual_candidate_first_pr_artifacts(&dir)?;
        let path = dir.join("review-kit.json");
        let mut review_kit = parse_json_file(&path)?;
        review_kit["handoff"]["manual_candidates"]["manual_candidates"] = serde_json::json!(2);
        fs::write(&path, review_kit.to_string())
            .map_err(|err| format!("write review kit failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let err = match result {
            Ok(()) => {
                return Err(
                    "manual candidate count drift should fail review-kit verification".to_string(),
                );
            }
            Err(err) => err,
        };
        assert!(
            err.contains("review-kit.json handoff manual_candidates.manual_candidates is 2, but manual-candidates.json has 1"),
            "{err}"
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_manual_candidate_operation_family_summary_drift()
    -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-manual-candidate-family-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_one_manual_candidate_first_pr_artifacts(&dir)?;
        let path = dir.join("manual-candidates.json");
        let mut manual_candidates = parse_json_file(&path)?;
        manual_candidates["summary"]["operation_families"]["raw_pointer_read"] =
            serde_json::json!(2);
        fs::write(&path, manual_candidates.to_string())
            .map_err(|err| format!("write manual candidates failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let err = match result {
            Ok(()) => {
                return Err(
                    "manual candidate operation-family summary drift should fail verification"
                        .to_string(),
                );
            }
            Err(err) => err,
        };
        assert!(
            err.contains("manual-candidates.json summary.operation_families"),
            "{err}"
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_review_kit_manual_candidate_evidence_kind_summary_drift()
    -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-review-kit-manual-evidence-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_one_manual_candidate_first_pr_artifacts(&dir)?;
        let path = dir.join("review-kit.json");
        let mut review_kit = parse_json_file(&path)?;
        review_kit["handoff"]["manual_candidates"]["evidence_kinds"]["runtime_witness"] =
            serde_json::json!(2);
        fs::write(&path, review_kit.to_string())
            .map_err(|err| format!("write review kit failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let err = match result {
            Ok(()) => {
                return Err(
                    "review-kit manual candidate evidence-kind summary drift should fail verification"
                        .to_string(),
                );
            }
            Err(err) => err,
        };
        assert!(
            err.contains("review-kit.json handoff manual_candidates.evidence_kinds"),
            "{err}"
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_review_kit_manual_candidate_id_drift() -> Result<(), String>
    {
        let dir = unique_temp_dir("unsafe-review-first-pr-review-kit-manual-candidate-id-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_one_manual_candidate_first_pr_artifacts(&dir)?;
        let path = dir.join("review-kit.json");
        let mut review_kit = parse_json_file(&path)?;
        let first_candidate = &mut review_kit["handoff"]["manual_candidates"]["first_candidate"];
        first_candidate["id"] = serde_json::json!("R4R2-S999");
        first_candidate["explain"] = serde_json::json!("unsafe-review explain R4R2-S999");
        first_candidate["context_json"] =
            serde_json::json!("unsafe-review context R4R2-S999 --json");
        first_candidate["witness_plan"] =
            serde_json::json!("unsafe-review candidate witness-plan R4R2-S999");
        fs::write(&path, review_kit.to_string())
            .map_err(|err| format!("write review kit failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let err = match result {
            Ok(()) => {
                return Err(
                    "manual candidate id drift should fail review-kit verification".to_string(),
                );
            }
            Err(err) => err,
        };
        assert!(
            err.contains("review-kit.json handoff manual_candidates first_candidate id `R4R2-S999` is not present in manual-candidates.json"),
            "{err}"
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_review_kit_manual_candidate_queue_drift()
    -> Result<(), String> {
        let dir =
            unique_temp_dir("unsafe-review-first-pr-review-kit-manual-candidate-queue-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_one_manual_candidate_first_pr_artifacts(&dir)?;
        let path = dir.join("review-kit.json");
        let mut review_kit = parse_json_file(&path)?;
        let queue_entry = &mut review_kit["handoff"]["manual_candidates"]["candidate_queue"][0];
        queue_entry["id"] = serde_json::json!("R4R2-S999");
        queue_entry["explain"] = serde_json::json!("unsafe-review explain R4R2-S999");
        queue_entry["context_json"] = serde_json::json!("unsafe-review context R4R2-S999 --json");
        queue_entry["witness_plan"] =
            serde_json::json!("unsafe-review candidate witness-plan R4R2-S999");
        fs::write(&path, review_kit.to_string())
            .map_err(|err| format!("write review kit failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let err = match result {
            Ok(()) => {
                return Err(
                    "manual candidate queue drift should fail review-kit verification".to_string(),
                );
            }
            Err(err) => err,
        };
        assert!(
            err.contains("review-kit.json handoff manual_candidates candidate_queue[0] id `R4R2-S999` must match manual-candidates.json candidate `R4R2-S001`"),
            "{err}"
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_manual_candidate_handoff_route_drift() -> Result<(), String>
    {
        let dir = unique_temp_dir("unsafe-review-first-pr-manual-candidate-handoff-route-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_one_manual_candidate_first_pr_artifacts(&dir)?;
        let path = dir.join("manual-candidates.json");
        let mut manual_candidates = parse_json_file(&path)?;
        manual_candidates["candidates"][0]["implementer_handoff"]["route"]["safe_caller"] =
            serde_json::json!("unrelated JS route");
        fs::write(&path, manual_candidates.to_string())
            .map_err(|err| format!("write manual candidates failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let err = match result {
            Ok(()) => {
                return Err(
                    "manual candidate handoff route drift should fail verification".to_string(),
                );
            }
            Err(err) => err,
        };
        assert!(
            err.contains("manual-candidates.json candidate implementer_handoff safe_caller"),
            "{err}"
        );
        assert!(
            err.contains("TextDecoder.decode SharedArrayBuffer route"),
            "{err}"
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_manual_candidate_guidance_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-manual-candidate-guidance-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_one_manual_candidate_first_pr_artifacts(&dir)?;
        let path = dir.join("manual-candidates.json");
        let mut manual_candidates = parse_json_file(&path)?;
        manual_candidates["candidates"][0]["implementer_handoff"]["fix_options"][0] =
            serde_json::json!("unrelated repair");
        fs::write(&path, manual_candidates.to_string())
            .map_err(|err| format!("write manual candidates failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let err = match result {
            Ok(()) => {
                return Err("manual candidate guidance drift should fail verification".to_string());
            }
            Err(err) => err,
        };
        assert!(
            err.contains(
                "manual-candidates.json candidate implementer_handoff fix_options must match manual-candidates.json candidate fix_options"
            ),
            "{err}"
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_review_kit_manual_candidate_handoff_drift()
    -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-review-kit-manual-handoff-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_one_manual_candidate_first_pr_artifacts(&dir)?;
        let path = dir.join("review-kit.json");
        let mut review_kit = parse_json_file(&path)?;
        review_kit["handoff"]["manual_candidates"]["candidate_queue"][0]["implementer_handoff"]["route"]
            ["safe_caller"] = serde_json::json!("unrelated JS route");
        fs::write(&path, review_kit.to_string())
            .map_err(|err| format!("write review kit failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let err = match result {
            Ok(()) => {
                return Err(
                    "review-kit manual candidate handoff drift should fail verification"
                        .to_string(),
                );
            }
            Err(err) => err,
        };
        assert!(
            err.contains(
                "review-kit.json handoff manual_candidates candidate_queue[0] implementer_handoff must match manual-candidates.json candidate `R4R2-S001` implementer_handoff"
            ),
            "{err}"
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_manual_candidate_markers_in_reviewcard_only_artifacts()
    -> Result<(), String> {
        for (artifact, object_pointer) in [
            ("cards.json", "/cards/0"),
            ("cards.sarif", "/runs/0/results/0/properties"),
            ("comment-plan.json", "/comments/0"),
            ("lsp.json", "/diagnostics/0"),
            ("repair-queue.json", "/buckets/repairable_by_guard/0"),
        ] {
            let dir = unique_temp_dir(&format!(
                "unsafe-review-first-pr-reviewcard-only-manual-marker-{artifact}"
            ))?;
            fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
            write_one_manual_candidate_first_pr_artifacts(&dir)?;
            let path = dir.join(artifact);
            let mut value = parse_json_file(&path)?;
            value
                .pointer_mut(object_pointer)
                .and_then(serde_json::Value::as_object_mut)
                .ok_or_else(|| format!("{artifact} fixture is missing object `{object_pointer}`"))?
                .insert("manual_candidate".to_string(), serde_json::json!(true));
            fs::write(&path, value.to_string())
                .map_err(|err| format!("write {artifact} failed: {err}"))?;

            let result = check_first_pr_artifacts(&dir);

            fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
            let err = match result {
                Ok(()) => {
                    return Err(format!(
                        "{artifact} manual candidate marker should fail verification"
                    ));
                }
                Err(err) => err,
            };
            assert!(err.contains(artifact), "{artifact}: {err}");
            assert!(err.contains("manual_candidate"), "{artifact}: {err}");
            assert!(err.contains("manual-candidates.json"), "{artifact}: {err}");
        }
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_manual_candidate_front_door_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-manual-front-door-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_one_manual_candidate_first_pr_artifacts(&dir)?;
        let path = dir.join("pr-summary.md");
        let text =
            fs::read_to_string(&path).map_err(|err| format!("read pr summary failed: {err}"))?;
        let text = text.replace(manual_candidate_front_panel_fixture(), "");
        fs::write(&path, text).map_err(|err| format!("write pr summary failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let err = match result {
            Ok(()) => return Err("missing manual candidate summary cue should fail".to_string()),
            Err(err) => err,
        };
        assert!(
            err.contains("pr-summary.md"),
            "expected pr-summary front-door drift, got {err}"
        );
        assert!(err.contains("## Manual candidates"), "{err}");
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_review_kit_missing_artifact() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-review-kit-missing-artifact")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("review-kit.json");
        let mut review_kit: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read review kit failed: {err}"))?,
        )
        .map_err(|err| format!("parse review kit failed: {err}"))?;
        review_kit["artifacts"]
            .as_array_mut()
            .ok_or_else(|| "review kit artifacts should be an array".to_string())?
            .push(serde_json::json!({
                "path": "sidecar.json",
                "kind": "unknown",
                "format": "json",
                "schema_version": "0.1"
            }));
        fs::write(&path, review_kit.to_string())
            .map_err(|err| format!("write review kit failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let err = match result {
            Ok(()) => {
                return Err("missing review-kit artifact should fail verification".to_string());
            }
            Err(err) => err,
        };
        assert!(
            err.contains("review-kit.json lists missing artifact `sidecar.json`"),
            "{err}"
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_zero_card_review_kit_handoff_top_card()
    -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-zero-card-review-kit-top-card")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_zero_card_first_pr_artifacts(&dir)?;
        let path = dir.join("review-kit.json");
        let mut review_kit: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read review kit failed: {err}"))?,
        )
        .map_err(|err| format!("parse review kit failed: {err}"))?;
        review_kit["handoff"]["top_card"] = serde_json::json!({
            "card_id": "card-1",
            "explain": "unsafe-review explain card-1",
            "context_json": "unsafe-review context card-1 --json"
        });
        fs::write(&path, review_kit.to_string())
            .map_err(|err| format!("write review kit failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let err = match result {
            Ok(()) => {
                return Err("zero-card top-card handoff should fail verification".to_string());
            }
            Err(err) => err,
        };
        assert!(
            err.contains("review-kit.json handoff top_card must be null"),
            "{err}"
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_zero_card_lsp_status_state_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-zero-card-lsp-status-state-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_zero_card_first_pr_artifacts(&dir)?;
        let lsp_path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&lsp_path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        lsp["status"]["state"] = serde_json::json!("actionable");
        fs::write(&lsp_path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("lsp.json status state")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_pr_summary_open_actionable_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-summary-open-actionable-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("pr-summary.md");
        let summary =
            fs::read_to_string(&path).map_err(|err| format!("read pr summary failed: {err}"))?;
        fs::write(
            &path,
            summary.replace("- Open actionable gaps: 1", "- Open actionable gaps: 0"),
        )
        .map_err(|err| format!("write pr summary failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("- Open actionable gaps: 1")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_github_summary_open_actionable_drift() -> Result<(), String>
    {
        let dir = unique_temp_dir("unsafe-review-first-pr-github-open-actionable-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("github-summary.md");
        let summary = fs::read_to_string(&path)
            .map_err(|err| format!("read github summary failed: {err}"))?;
        fs::write(
            &path,
            summary.replace("- Open actionable gaps: 1", "- Open actionable gaps: 0"),
        )
        .map_err(|err| format!("write github summary failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("- Open actionable gaps: 1")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_witness_plan_open_actionable_drift() -> Result<(), String>
    {
        let dir = unique_temp_dir("unsafe-review-first-pr-witness-open-actionable-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("witness-plan.md");
        let witness_plan =
            fs::read_to_string(&path).map_err(|err| format!("read witness plan failed: {err}"))?;
        fs::write(
            &path,
            witness_plan.replace("- Open actionable gaps: 1", "- Open actionable gaps: 0"),
        )
        .map_err(|err| format!("write witness plan failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("- Open actionable gaps: 1")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_pr_summary_policy_mode_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-summary-policy-mode-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("pr-summary.md");
        let summary =
            fs::read_to_string(&path).map_err(|err| format!("read pr summary failed: {err}"))?;
        fs::write(
            &path,
            summary.replace("- Policy mode: `advisory`", "- Policy mode: `blocking`"),
        )
        .map_err(|err| format!("write pr summary failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("- Policy mode: `advisory`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_github_summary_policy_mode_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-github-policy-mode-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("github-summary.md");
        let summary = fs::read_to_string(&path)
            .map_err(|err| format!("read github summary failed: {err}"))?;
        fs::write(
            &path,
            summary.replace("- Policy mode: `advisory`", "- Policy mode: `blocking`"),
        )
        .map_err(|err| format!("write github summary failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("- Policy mode: `advisory`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_cards_json_repo_scope() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-cards-scope-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("cards.json");
        let mut cards: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read cards failed: {err}"))?,
        )
        .map_err(|err| format!("parse cards failed: {err}"))?;
        cards["scope"] = serde_json::json!("repo");
        fs::write(&path, cards.to_string()).map_err(|err| format!("write cards failed: {err}"))?;
        for file_name in ["pr-summary.md", "github-summary.md"] {
            let path = dir.join(file_name);
            let text = fs::read_to_string(&path)
                .map_err(|err| format!("read {file_name} failed: {err}"))?;
            fs::write(&path, text.replace("- Scope: `diff`", "- Scope: `repo`"))
                .map_err(|err| format!("write {file_name} failed: {err}"))?;
        }
        let path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        lsp["scope"] = serde_json::json!("repo");
        fs::write(&path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;
        let path = dir.join("cards.sarif");
        let mut sarif: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read sarif failed: {err}"))?,
        )
        .map_err(|err| format!("parse sarif failed: {err}"))?;
        sarif["runs"][0]["properties"]["scope"] = serde_json::json!("repo");
        fs::write(&path, sarif.to_string()).map_err(|err| format!("write sarif failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let err = result.err().unwrap_or_default();
        assert!(
            err.contains("cards.json scope for first-pr artifacts must be `diff`"),
            "unexpected error: {err}"
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_pr_summary_scope_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-summary-scope-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("pr-summary.md");
        let summary =
            fs::read_to_string(&path).map_err(|err| format!("read pr summary failed: {err}"))?;
        fs::write(&path, summary.replace("- Scope: `diff`", "- Scope: `repo`"))
            .map_err(|err| format!("write pr summary failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains("- Scope: `diff`"));
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_github_summary_scope_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-github-scope-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("github-summary.md");
        let summary = fs::read_to_string(&path)
            .map_err(|err| format!("read github summary failed: {err}"))?;
        fs::write(&path, summary.replace("- Scope: `diff`", "- Scope: `repo`"))
            .map_err(|err| format!("write github summary failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains("- Scope: `diff`"));
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_scope_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-scope-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        lsp["scope"] = serde_json::json!("repo");
        fs::write(&path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("lsp.json key `scope` is `repo`, expected `diff`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_witness_plan_policy_mode_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-witness-policy-mode-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("witness-plan.md");
        let witness_plan =
            fs::read_to_string(&path).map_err(|err| format!("read witness plan failed: {err}"))?;
        fs::write(
            &path,
            witness_plan.replace("- Policy mode: `advisory`", "- Policy mode: `blocking`"),
        )
        .map_err(|err| format!("write witness plan failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("- Policy mode: `advisory`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_github_summary_no_card_overclaim() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-zero-card-github-overclaim")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_zero_card_first_pr_artifacts(&dir)?;
        let path = dir.join("github-summary.md");
        let summary = fs::read_to_string(&path)
            .map_err(|err| format!("read github summary failed: {err}"))?;
        fs::write(
            &path,
            summary.replace(
                "This does not prove the repo safe, UB-free, Miri-clean, or that any unsafe site executed.\n\n",
                "",
            ),
        )
        .map_err(|err| format!("write github summary failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("This does not prove the repo safe")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_missing_witness_plan() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-missing-witness")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        fs::remove_file(dir.join("witness-plan.md"))
            .map_err(|err| format!("remove witness plan failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let err = result.err().unwrap_or_default();
        assert!(err.contains("witness-plan.md"), "{err}");
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_witness_plan_verify_command_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-witness-command-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("witness-plan.md");
        let witness_plan =
            fs::read_to_string(&path).map_err(|err| format!("read witness plan failed: {err}"))?;
        fs::write(
            &path,
            witness_plan.replace(
                "cargo +nightly miri test card",
                "cargo +nightly miri test unrelated_card",
            ),
        )
        .map_err(|err| format!("write witness plan failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains(
            "must include verify command `cargo +nightly miri test card` for ReviewCard `card-1`"
        ));
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_witness_plan_unknown_card_heading() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-witness-unknown-heading")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("witness-plan.md");
        let witness_plan =
            fs::read_to_string(&path).map_err(|err| format!("read witness plan failed: {err}"))?;
        fs::write(
            &path,
            witness_plan.replace(
                "## Trust boundary",
                "#### `missing`\n\n- Route: `human-deep-review`\n  - Reason: route\n  - What it can show: focused reviewer attention\n  - What it cannot prove: arbitrary callers\n  - Receipt hint: unsafe-review receipt import-manual missing\n\n## Trust boundary",
            ),
        )
        .map_err(|err| format!("write witness plan failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("witness-plan route heading references unknown card id `missing`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_witness_plan_class_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-witness-class-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("witness-plan.md");
        let witness_plan =
            fs::read_to_string(&path).map_err(|err| format!("read witness plan failed: {err}"))?;
        fs::write(
            &path,
            witness_plan.replace("- Class: `guard_missing`", "- Class: `contract_missing`"),
        )
        .map_err(|err| format!("write witness plan failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains(
            "witness-plan ReviewCard `card-1` class must include `- Class: `guard_missing``"
        ));
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_witness_plan_next_action_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-witness-next-action-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("witness-plan.md");
        let witness_plan =
            fs::read_to_string(&path).map_err(|err| format!("read witness plan failed: {err}"))?;
        fs::write(
            &path,
            witness_plan.replace(
                "- Next action: Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.",
                "- Next action: Run broad tests.",
            ),
        )
        .map_err(|err| format!("write witness plan failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains(
            "witness-plan ReviewCard `card-1` next action must include `- Next action: Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.`"
        ));
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_witness_plan_route_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-witness-route-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("witness-plan.md");
        let witness_plan =
            fs::read_to_string(&path).map_err(|err| format!("read witness plan failed: {err}"))?;
        fs::write(
            &path,
            witness_plan.replace("- Route: `miri`", "- Route: `loom`"),
        )
        .map_err(|err| format!("write witness plan failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains(
            "witness-plan ReviewCard `card-1` witness route must include `- Route: `miri``"
        ));
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_witness_plan_route_reason_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-witness-route-reason-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("witness-plan.md");
        let witness_plan =
            fs::read_to_string(&path).map_err(|err| format!("read witness plan failed: {err}"))?;
        fs::write(
            &path,
            witness_plan.replace("  - Reason: route", "  - Reason: unrelated route"),
        )
        .map_err(|err| format!("write witness plan failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains(
            "witness-plan ReviewCard `card-1` witness route reason must include `  - Reason: route`"
        ));
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_witness_plan_route_command_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-witness-route-command-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("witness-plan.md");
        let witness_plan =
            fs::read_to_string(&path).map_err(|err| format!("read witness plan failed: {err}"))?;
        let witness_plan = witness_plan
            .replace(
                "```bash\ncargo +nightly miri test card\n```",
                "```bash\ncargo +nightly miri test unrelated_card\n```",
            )
            .replace(
                "## Trust boundary",
                "## Trust boundary\n\nOriginal verify command mention: cargo +nightly miri test card",
            );
        fs::write(&path, witness_plan)
            .map_err(|err| format!("write witness plan failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains(
            "witness-plan ReviewCard `card-1` witness route command must include fenced command `cargo +nightly miri test card`"
        ));
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_unknown_card_id() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-bad-lsp")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        fs::write(
            dir.join("lsp.json"),
            r#"{"schema_version":"0.1","tool":"unsafe-review","mode":"read_only_projection","policy":"advisory","scope":"diff","status":{"state":"actionable","cards":1,"open_actionable_gaps":1,"high_priority_cards":1,"message":"1 unsafe-review card(s), 1 open actionable gap(s)","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"},"diagnostics":[{"card_id":"missing","witness_routes":[],"verify_commands":[],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}],"hovers":[],"code_actions":[],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
        )
        .map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains("unknown card id"));
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_sarif_unknown_card_id() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-bad-sarif-card-id")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("cards.sarif");
        let mut sarif: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read sarif failed: {err}"))?,
        )
        .map_err(|err| format!("parse sarif failed: {err}"))?;
        sarif["runs"][0]["results"][0]["properties"]["cardId"] = serde_json::json!("missing");
        fs::write(&path, sarif.to_string()).map_err(|err| format!("write sarif failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("cards.sarif result references unknown card id `missing`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_sarif_missing_card_id() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-missing-sarif-card-id")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("cards.sarif");
        let mut sarif: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read sarif failed: {err}"))?,
        )
        .map_err(|err| format!("parse sarif failed: {err}"))?;
        sarif["runs"][0]["results"][0]["properties"]
            .as_object_mut()
            .ok_or_else(|| "sarif result properties fixture must be an object".to_string())?
            .remove("cardId");
        fs::write(&path, sarif.to_string()).map_err(|err| format!("write sarif failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("cards.sarif result is missing properties.cardId")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_missing_card_diagnostic() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-missing-diagnostic")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let mut lsp: serde_json::Value = serde_json::from_str(&valid_lsp_json(
            r#"[{"card_id":"card-1","command":"unsafe-review.copyAgentPacket","payload":{"kind":"unsafe-review.agent_packet","card_id":"card-1","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"},"arguments":["card-1"]}]"#,
        )?)
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        *lsp.get_mut("diagnostics")
            .ok_or_else(|| "test lsp missing diagnostics".to_string())? = serde_json::json!([]);
        fs::write(dir.join("lsp.json"), lsp.to_string())
            .map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("diagnostics missing card id `card-1`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_duplicate_card_diagnostic() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-duplicate-diagnostic")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let mut lsp: serde_json::Value = serde_json::from_str(&valid_lsp_json(
            r#"[{"card_id":"card-1","command":"unsafe-review.copyAgentPacket","payload":{"kind":"unsafe-review.agent_packet","card_id":"card-1","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"},"arguments":["card-1"]}]"#,
        )?)
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        let diagnostics = lsp
            .get_mut("diagnostics")
            .and_then(serde_json::Value::as_array_mut)
            .ok_or_else(|| "test lsp missing diagnostics".to_string())?;
        let duplicate = diagnostics
            .first()
            .cloned()
            .ok_or_else(|| "test lsp diagnostics empty".to_string())?;
        diagnostics.push(duplicate);
        fs::write(dir.join("lsp.json"), lsp.to_string())
            .map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("diagnostics repeat card id `card-1`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_diagnostic_missing_path() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-diagnostic-missing-path")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let lsp_path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&lsp_path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        let first_diagnostic = lsp
            .get_mut("diagnostics")
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|diagnostics| diagnostics.first_mut())
            .and_then(serde_json::Value::as_object_mut)
            .ok_or_else(|| "test lsp missing first diagnostic".to_string())?;
        first_diagnostic.remove("path");
        fs::write(&lsp_path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("missing string key `path`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_diagnostic_location_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-diagnostic-location-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let lsp_path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&lsp_path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        let first_diagnostic = lsp
            .get_mut("diagnostics")
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|diagnostics| diagnostics.first_mut())
            .ok_or_else(|| "test lsp missing first diagnostic".to_string())?;
        first_diagnostic["path"] = serde_json::json!("src/other.rs");
        fs::write(&lsp_path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("lsp.json diagnostic path must be `src/lib.rs`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_diagnostic_class_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-diagnostic-class-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let lsp_path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&lsp_path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        let first_diagnostic = lsp
            .get_mut("diagnostics")
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|diagnostics| diagnostics.first_mut())
            .ok_or_else(|| "test lsp missing first diagnostic".to_string())?;
        first_diagnostic["code"] = serde_json::json!("contract_missing");
        fs::write(&lsp_path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("lsp.json diagnostic code must be `guard_missing`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_diagnostic_operation_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-diagnostic-operation-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let lsp_path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&lsp_path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        let first_diagnostic = lsp
            .get_mut("diagnostics")
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|diagnostics| diagnostics.first_mut())
            .ok_or_else(|| "test lsp missing first diagnostic".to_string())?;
        first_diagnostic["operation"] = serde_json::json!("unsafe { other.read() }");
        fs::write(&lsp_path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains(
            "lsp.json diagnostic operation must be `unsafe { ptr.cast::<Header>().read() }`"
        ));
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_diagnostic_operation_family_drift()
    -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-diagnostic-family-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let lsp_path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&lsp_path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        let first_diagnostic = lsp
            .get_mut("diagnostics")
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|diagnostics| diagnostics.first_mut())
            .ok_or_else(|| "test lsp missing first diagnostic".to_string())?;
        first_diagnostic["operation_family"] = serde_json::json!("unknown");
        fs::write(&lsp_path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("lsp.json diagnostic operation_family must be `raw_pointer_read`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_diagnostic_next_action_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-diagnostic-next-action-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let lsp_path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&lsp_path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        let first_diagnostic = lsp
            .get_mut("diagnostics")
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|diagnostics| diagnostics.first_mut())
            .ok_or_else(|| "test lsp missing first diagnostic".to_string())?;
        first_diagnostic["next_action"] = serde_json::json!("Run an unrelated broad audit.");
        fs::write(&lsp_path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains(
            "lsp.json diagnostic next_action must be `Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.`"
        ));
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_diagnostic_hazard_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-diagnostic-hazard-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let lsp_path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&lsp_path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        let first_diagnostic = lsp
            .get_mut("diagnostics")
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|diagnostics| diagnostics.first_mut())
            .ok_or_else(|| "test lsp missing first diagnostic".to_string())?;
        first_diagnostic["hazards"] = serde_json::json!(["bounds"]);
        fs::write(&lsp_path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("lsp.json diagnostic hazards must project cards.json value")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_diagnostic_verify_command_drift() -> Result<(), String>
    {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-diagnostic-verify-command-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let lsp_path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&lsp_path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        let first_diagnostic = lsp
            .get_mut("diagnostics")
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|diagnostics| diagnostics.first_mut())
            .ok_or_else(|| "test lsp missing first diagnostic".to_string())?;
        first_diagnostic["verify_commands"] = serde_json::json!(["cargo test unrelated"]);
        first_diagnostic["witness_routes"][0]["command"] =
            serde_json::json!("cargo test unrelated");
        fs::write(&lsp_path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("lsp.json diagnostic verify_commands must project cards.json value")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_diagnostic_missing_evidence_drift()
    -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-diagnostic-missing-evidence-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let lsp_path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&lsp_path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        let first_diagnostic = lsp
            .get_mut("diagnostics")
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|diagnostics| diagnostics.first_mut())
            .ok_or_else(|| "test lsp missing first diagnostic".to_string())?;
        first_diagnostic["missing_evidence"] = serde_json::json!(["unrelated missing evidence"]);
        fs::write(&lsp_path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let err = result.err().unwrap_or_default();
        assert!(
            err.contains("lsp.json diagnostic missing_evidence must project cards.json value"),
            "{err}"
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_required_condition_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-required-condition-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let lsp_path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&lsp_path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        lsp["diagnostics"][0]["required_safety_conditions"][0]["description"] =
            serde_json::json!("unrelated condition");
        fs::write(&lsp_path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let err = result.err().unwrap_or_default();
        assert!(
            err.contains(
                "lsp.json diagnostic required_safety_conditions[0] must project cards.json value"
            ),
            "{err}"
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_obligation_evidence_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-obligation-evidence-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let lsp_path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&lsp_path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        lsp["diagnostics"][0]["obligation_evidence"][0]["discharge"]["summary"] =
            serde_json::json!("unrelated discharge evidence");
        fs::write(&lsp_path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let err = result.err().unwrap_or_default();
        assert!(
            err.contains(
                "lsp.json diagnostic obligation_evidence[0] must project cards.json value"
            ),
            "{err}"
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_evidence_summary_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-evidence-summary-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let lsp_path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&lsp_path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        lsp["diagnostics"][0]["evidence_summary"]["discharge"]["summary"] =
            serde_json::json!("unrelated discharge evidence");
        fs::write(&lsp_path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let err = result.err().unwrap_or_default();
        assert!(
            err.contains(
                "lsp.json diagnostic evidence_summary.discharge.summary must project cards.json value `No visible local guard`"
            ),
            "{err}"
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_diagnostic_reversed_range() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-diagnostic-reversed-range")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let lsp_path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&lsp_path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        let first_diagnostic = lsp
            .get_mut("diagnostics")
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|diagnostics| diagnostics.first_mut())
            .ok_or_else(|| "test lsp missing first diagnostic".to_string())?;
        first_diagnostic["range"]["end"]["line"] = serde_json::json!(5);
        first_diagnostic["range"]["end"]["character"] = serde_json::json!(0);
        fs::write(&lsp_path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("range end must not precede start")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_missing_card_hover() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-missing-hover")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let mut lsp: serde_json::Value = serde_json::from_str(&valid_lsp_json(
            r#"[{"card_id":"card-1","command":"unsafe-review.copyAgentPacket","payload":{"kind":"unsafe-review.agent_packet","card_id":"card-1","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"},"arguments":["card-1"]}]"#,
        )?)
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        *lsp.get_mut("hovers")
            .ok_or_else(|| "test lsp missing hovers".to_string())? = serde_json::json!([]);
        fs::write(dir.join("lsp.json"), lsp.to_string())
            .map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("hovers missing card id `card-1`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_duplicate_card_hover() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-duplicate-hover")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let mut lsp: serde_json::Value = serde_json::from_str(&valid_lsp_json(
            r#"[{"card_id":"card-1","command":"unsafe-review.copyAgentPacket","payload":{"kind":"unsafe-review.agent_packet","card_id":"card-1","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"},"arguments":["card-1"]}]"#,
        )?)
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        let hovers = lsp
            .get_mut("hovers")
            .and_then(serde_json::Value::as_array_mut)
            .ok_or_else(|| "test lsp missing hovers".to_string())?;
        let duplicate = hovers
            .first()
            .cloned()
            .ok_or_else(|| "test lsp hovers empty".to_string())?;
        hovers.push(duplicate);
        fs::write(dir.join("lsp.json"), lsp.to_string())
            .map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("hovers repeat card id `card-1`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_hover_missing_path() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-hover-missing-path")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let lsp_path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&lsp_path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        let first_hover = lsp
            .get_mut("hovers")
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|hovers| hovers.first_mut())
            .and_then(serde_json::Value::as_object_mut)
            .ok_or_else(|| "test lsp missing first hover".to_string())?;
        first_hover.remove("path");
        fs::write(&lsp_path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("missing string key `path`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_hover_missing_position() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-hover-missing-position")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let lsp_path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&lsp_path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        let first_hover = lsp
            .get_mut("hovers")
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|hovers| hovers.first_mut())
            .and_then(serde_json::Value::as_object_mut)
            .ok_or_else(|| "test lsp missing first hover".to_string())?;
        first_hover.remove("position");
        fs::write(&lsp_path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("missing unsigned integer at `/position/line`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_hover_card_identity_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-hover-card-id")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let lsp_path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&lsp_path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        let first_hover = lsp
            .get_mut("hovers")
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|hovers| hovers.first_mut())
            .ok_or_else(|| "test lsp missing first hover".to_string())?;
        first_hover["contents"] = serde_json::json!(
            "Card: `missing`\n\nTrust boundary: static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"
        );
        fs::write(&lsp_path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("hover contents must mention card id `card-1`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_hover_unknown_card_mentions() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-hover-unknown-card")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let lsp_path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&lsp_path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        let first_hover = lsp
            .get_mut("hovers")
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|hovers| hovers.first_mut())
            .ok_or_else(|| "test lsp missing first hover".to_string())?;
        let contents = first_hover
            .get("contents")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "test lsp hover missing contents".to_string())?;
        first_hover["contents"] =
            serde_json::json!(format!("{contents}\n\nRelated card: `card-2`\n"));
        fs::write(&lsp_path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("mentions unknown ReviewCard id `card-2`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_hover_missing_hazard_projection() -> Result<(), String>
    {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-hover-hazard")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let lsp_path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&lsp_path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        let first_hover = lsp
            .get_mut("hovers")
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|hovers| hovers.first_mut())
            .ok_or_else(|| "test lsp missing first hover".to_string())?;
        first_hover["contents"] = serde_json::json!(
            "Card: `card-1`; priority `high`; confidence `medium`\n\nLocation: src/lib.rs:7\n\nWhy this card exists:\n- The changed code contains a `raw_pointer_read` unsafe operation that unsafe-review classifies as `guard_missing`.\n- Operation: `unsafe { ptr.cast::<Header>().read() }`\n\nRequired safety conditions:\n- pointer aligned\n\nEvidence found:\n- Contract [present]: safety contract\n- Guard/discharge [missing]: No visible local guard\n- Reach [owner_reached]: related test mention\n- Witness [missing]: No imported witness receipt\n\nEvidence missing:\n- none recorded\n\nWhat would resolve this:\n- Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.\n\nVerify commands:\n- `cargo +nightly miri test card`\n\nWhat would not resolve this:\n- A `SAFETY:` comment alone does not discharge missing guard evidence.\n- A related test mention is not proof that this unsafe site executed.\n- Do not claim witness proof unless a matching receipt exists.\n- Do not widen unsafe scope, suppress the card, or change unrelated unsafe code to silence this review item.\n\nWitness route: `miri` because route.\n\nHandoff commands:\n- Explain: `unsafe-review explain card-1`\n- Agent context: `unsafe-review context card-1 --json`\n\nTrust boundary: static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"
        );
        fs::write(&lsp_path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("hover contents must include ReviewCard hazard families")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_hover_missing_next_action_projection()
    -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-hover-next-action")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let lsp_path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&lsp_path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        let first_hover = lsp
            .get_mut("hovers")
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|hovers| hovers.first_mut())
            .ok_or_else(|| "test lsp missing first hover".to_string())?;
        let contents = first_hover
            .get("contents")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "test lsp hover missing contents".to_string())?
            .replace(
                "- Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.",
                "- Run broad tests.",
            );
        first_hover["contents"] = serde_json::json!(contents);
        fs::write(&lsp_path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("contents must project ReviewCard next action")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_hover_missing_repair_boundary() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-hover-repair-boundary")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let lsp_path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&lsp_path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        let first_hover = lsp
            .get_mut("hovers")
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|hovers| hovers.first_mut())
            .ok_or_else(|| "test lsp missing first hover".to_string())?;
        let contents = first_hover
            .get("contents")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "test lsp hover missing contents".to_string())?
            .replace(
                "- Do not widen unsafe scope, suppress the card, or change unrelated unsafe code to silence this review item.\n",
                "",
            );
        first_hover["contents"] = serde_json::json!(contents);
        fs::write(&lsp_path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains(
            "contents must include `Do not widen unsafe scope, suppress the card, or change unrelated unsafe code`"
        ));
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_hover_missing_handoff_commands() -> Result<(), String>
    {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-hover-handoff-commands")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let lsp_path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&lsp_path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        let first_hover = lsp
            .get_mut("hovers")
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|hovers| hovers.first_mut())
            .ok_or_else(|| "test lsp missing first hover".to_string())?;
        let contents = first_hover
            .get("contents")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "test lsp hover missing contents".to_string())?
            .replace(
                "- Agent context: `unsafe-review context card-1 --json`\n",
                "",
            );
        first_hover["contents"] = serde_json::json!(contents);
        fs::write(&lsp_path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains(
            "contents must project ReviewCard agent context command `- Agent context: `unsafe-review context card-1 --json``"
        ));
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_hover_missing_location() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-hover-location")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let lsp_path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&lsp_path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        let first_hover = lsp
            .get_mut("hovers")
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|hovers| hovers.first_mut())
            .ok_or_else(|| "test lsp missing first hover".to_string())?;
        let contents = first_hover
            .get("contents")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "test lsp hover missing contents".to_string())?
            .replace("Location: src/lib.rs:7\n\n", "");
        first_hover["contents"] = serde_json::json!(contents);
        fs::write(&lsp_path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("contents must project ReviewCard location `Location: src/lib.rs:7`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_hover_required_condition_drift() -> Result<(), String>
    {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-hover-required-condition")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let lsp_path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&lsp_path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        let first_hover = lsp
            .get_mut("hovers")
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|hovers| hovers.first_mut())
            .ok_or_else(|| "test lsp missing first hover".to_string())?;
        let contents = first_hover
            .get("contents")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "test lsp hover missing contents".to_string())?
            .replace("- pointer aligned", "- unrelated condition");
        first_hover["contents"] = serde_json::json!(contents);
        fs::write(&lsp_path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains(
            "contents must project ReviewCard required safety condition `pointer aligned`"
        ));
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_hover_evidence_summary_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-hover-evidence-summary")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let lsp_path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&lsp_path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        let first_hover = lsp
            .get_mut("hovers")
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|hovers| hovers.first_mut())
            .ok_or_else(|| "test lsp missing first hover".to_string())?;
        let contents = first_hover
            .get("contents")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "test lsp hover missing contents".to_string())?
            .replace("No visible local guard", "unrelated discharge evidence");
        first_hover["contents"] = serde_json::json!(contents);
        fs::write(&lsp_path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains(
            "contents must project ReviewCard discharge evidence summary `No visible local guard`"
        ));
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_hover_location_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-hover-location-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let lsp_path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&lsp_path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        let first_hover = lsp
            .get_mut("hovers")
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|hovers| hovers.first_mut())
            .ok_or_else(|| "test lsp missing first hover".to_string())?;
        first_hover["position"]["line"] = serde_json::json!(7);
        fs::write(&lsp_path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("lsp.json hover line must point at ReviewCard site line 7")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_missing_required_code_action() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-missing-action")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        fs::write(
            dir.join("lsp.json"),
            valid_lsp_json(
                r#"[{"card_id":"card-1","path":"src/lib.rs","range":{"start":{"line":6,"character":0},"end":{"line":6,"character":1}},"title":"Copy unsafe-review packet for card-1","kind":"quickfix","command":"unsafe-review.copyAgentPacket","payload":{"kind":"unsafe-review.agent_packet","card_id":"card-1","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"},"arguments":["card-1"]}]"#,
            )?,
        )
        .map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("code_actions missing command `unsafe-review.explainWitnessRoute`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_duplicate_code_action_command() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-duplicate-action")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        fs::write(
            dir.join("lsp.json"),
            valid_lsp_json(
                r#"[{"card_id":"card-1","path":"src/lib.rs","range":{"start":{"line":6,"character":0},"end":{"line":6,"character":1}},"title":"Copy unsafe-review packet for card-1","kind":"quickfix","command":"unsafe-review.copyAgentPacket","payload":{"kind":"unsafe-review.agent_packet","card_id":"card-1","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"},"arguments":["card-1"]},{"card_id":"card-1","path":"src/lib.rs","range":{"start":{"line":6,"character":0},"end":{"line":6,"character":1}},"title":"Copy unsafe-review packet for card-1","kind":"quickfix","command":"unsafe-review.copyAgentPacket","payload":{"kind":"unsafe-review.agent_packet","card_id":"card-1","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"},"arguments":["card-1"]},{"card_id":"card-1","path":"src/lib.rs","range":{"start":{"line":6,"character":0},"end":{"line":6,"character":1}},"title":"Explain unsafe-review witness route","kind":"quickfix","command":"unsafe-review.explainWitnessRoute","payload":{"kind":"unsafe-review.witness_route","card_id":"card-1","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"},"arguments":["card-1"]}]"#,
            )?,
        )
        .map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("code_actions repeat command `unsafe-review.copyAgentPacket`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_code_action_argument_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-action-argument-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        fs::write(
            dir.join("lsp.json"),
            valid_lsp_json(
                r#"[{"card_id":"card-1","path":"src/lib.rs","range":{"start":{"line":6,"character":0},"end":{"line":6,"character":1}},"title":"Copy unsafe-review packet for card-1","kind":"quickfix","command":"unsafe-review.copyAgentPacket","payload":{"kind":"unsafe-review.agent_packet","card_id":"card-1","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"},"arguments":["other-card"]},{"card_id":"card-1","path":"src/lib.rs","range":{"start":{"line":6,"character":0},"end":{"line":6,"character":1}},"title":"Explain unsafe-review witness route","kind":"quickfix","command":"unsafe-review.explainWitnessRoute","payload":{"kind":"unsafe-review.witness_route","card_id":"card-1","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"},"arguments":["card-1"]}]"#,
            )?,
        )
        .map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("arguments[0] must be `card-1`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_code_action_title_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-action-title-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let lsp_path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&lsp_path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        let first_action = lsp
            .get_mut("code_actions")
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|actions| actions.first_mut())
            .ok_or_else(|| "test lsp missing first code action".to_string())?;
        first_action["title"] = serde_json::json!("Apply unsafe-review fix");
        fs::write(&lsp_path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("code_action `unsafe-review.copyAgentPacket` title must be")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_witness_command_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-witness-command-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        fs::write(
            dir.join("lsp.json"),
            valid_lsp_json(
                r#"[{"card_id":"card-1","path":"src/lib.rs","range":{"start":{"line":6,"character":0},"end":{"line":6,"character":1}},"title":"Copy unsafe-review packet for card-1","kind":"quickfix","command":"unsafe-review.copyAgentPacket","payload":{"kind":"unsafe-review.agent_packet","card_id":"card-1","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"},"arguments":["card-1"]},{"card_id":"card-1","path":"src/lib.rs","range":{"start":{"line":6,"character":0},"end":{"line":6,"character":1}},"title":"Explain unsafe-review witness route","kind":"quickfix","command":"unsafe-review.explainWitnessRoute","payload":{"kind":"unsafe-review.witness_route","card_id":"card-1","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"},"arguments":["card-1"]},{"card_id":"card-1","path":"src/lib.rs","range":{"start":{"line":6,"character":0},"end":{"line":6,"character":1}},"title":"Copy witness command (does not run)","kind":"quickfix","command":"unsafe-review.copyWitnessCommand","payload":{"kind":"unsafe-review.witness_command","card_id":"card-1","command":"cargo test unrelated","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"},"arguments":["cargo test unrelated"]}]"#,
            )?,
        )
        .map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains(
                    "copyWitnessCommand payload command `cargo test unrelated` must match a ReviewCard verify command"
                )
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_code_action_payload_edit() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-action-payload-edit")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let lsp_path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&lsp_path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        let first_action = lsp
            .get_mut("code_actions")
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|actions| actions.first_mut())
            .ok_or_else(|| "test lsp missing first code action".to_string())?;
        first_action["payload"]["edit"] =
            serde_json::json!({"changes":{"src/lib.rs":[{"newText":"// edit"}]}});
        fs::write(&lsp_path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("lsp.json code_action/payload must not contain source edit field `edit`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_code_action_missing_path() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-action-missing-path")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let lsp_path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&lsp_path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        let first_action = lsp
            .get_mut("code_actions")
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|actions| actions.first_mut())
            .and_then(serde_json::Value::as_object_mut)
            .ok_or_else(|| "test lsp missing first code action".to_string())?;
        first_action.remove("path");
        fs::write(&lsp_path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("missing string key `path`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_code_action_location_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-action-location-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let lsp_path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&lsp_path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        let first_action = lsp
            .get_mut("code_actions")
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|actions| actions.first_mut())
            .ok_or_else(|| "test lsp missing first code action".to_string())?;
        first_action["path"] = serde_json::json!("src/other.rs");
        fs::write(&lsp_path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("lsp.json code_action path must be `src/lib.rs`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_accepts_lsp_related_test_action_location() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-related-action-location")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        fs::write(
            dir.join("lsp.json"),
            valid_lsp_json(
                r#"[{"card_id":"card-1","path":"src/lib.rs","range":{"start":{"line":6,"character":0},"end":{"line":6,"character":1}},"title":"Copy unsafe-review packet for card-1","kind":"quickfix","command":"unsafe-review.copyAgentPacket","payload":{"kind":"unsafe-review.agent_packet","card_id":"card-1","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"},"arguments":["card-1"]},{"card_id":"card-1","path":"src/lib.rs","range":{"start":{"line":6,"character":0},"end":{"line":6,"character":1}},"title":"Explain unsafe-review witness route","kind":"quickfix","command":"unsafe-review.explainWitnessRoute","payload":{"kind":"unsafe-review.witness_route","card_id":"card-1","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"},"arguments":["card-1"]},{"card_id":"card-1","path":"tests/read_header.rs","range":{"start":{"line":2,"character":0},"end":{"line":2,"character":1}},"title":"Open related test read_header","kind":"quickfix","command":"unsafe-review.openRelatedTest","payload":{"kind":"unsafe-review.related_test","card_id":"card-1","file":"tests/read_header.rs","line":3,"name":"read_header","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"},"arguments":["card-1","tests/read_header.rs","3","read_header"]}]"#,
            )?,
        )
        .map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        result
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_related_test_action_location_drift()
    -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-related-action-location-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        fs::write(
            dir.join("lsp.json"),
            valid_lsp_json(
                r#"[{"card_id":"card-1","path":"src/lib.rs","range":{"start":{"line":6,"character":0},"end":{"line":6,"character":1}},"title":"Copy unsafe-review packet for card-1","kind":"quickfix","command":"unsafe-review.copyAgentPacket","payload":{"kind":"unsafe-review.agent_packet","card_id":"card-1","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"},"arguments":["card-1"]},{"card_id":"card-1","path":"src/lib.rs","range":{"start":{"line":6,"character":0},"end":{"line":6,"character":1}},"title":"Explain unsafe-review witness route","kind":"quickfix","command":"unsafe-review.explainWitnessRoute","payload":{"kind":"unsafe-review.witness_route","card_id":"card-1","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"},"arguments":["card-1"]},{"card_id":"card-1","path":"src/lib.rs","range":{"start":{"line":2,"character":0},"end":{"line":2,"character":1}},"title":"Open related test read_header","kind":"quickfix","command":"unsafe-review.openRelatedTest","payload":{"kind":"unsafe-review.related_test","card_id":"card-1","file":"tests/read_header.rs","line":3,"name":"read_header","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"},"arguments":["card-1","tests/read_header.rs","3","read_header"]}]"#,
            )?,
        )
        .map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("related_test path must be `tests/read_header.rs`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_related_test_action_title_drift() -> Result<(), String>
    {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-related-action-title-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        fs::write(
            dir.join("lsp.json"),
            valid_lsp_json(
                r#"[{"card_id":"card-1","path":"src/lib.rs","range":{"start":{"line":6,"character":0},"end":{"line":6,"character":1}},"title":"Copy unsafe-review packet for card-1","kind":"quickfix","command":"unsafe-review.copyAgentPacket","payload":{"kind":"unsafe-review.agent_packet","card_id":"card-1","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"},"arguments":["card-1"]},{"card_id":"card-1","path":"src/lib.rs","range":{"start":{"line":6,"character":0},"end":{"line":6,"character":1}},"title":"Explain unsafe-review witness route","kind":"quickfix","command":"unsafe-review.explainWitnessRoute","payload":{"kind":"unsafe-review.witness_route","card_id":"card-1","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"},"arguments":["card-1"]},{"card_id":"card-1","path":"tests/read_header.rs","range":{"start":{"line":2,"character":0},"end":{"line":2,"character":1}},"title":"Open unrelated test","kind":"quickfix","command":"unsafe-review.openRelatedTest","payload":{"kind":"unsafe-review.related_test","card_id":"card-1","file":"tests/read_header.rs","line":3,"name":"read_header","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"},"arguments":["card-1","tests/read_header.rs","3","read_header"]}]"#,
            )?,
        )
        .map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("code_action `unsafe-review.openRelatedTest` title must be")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_code_action_reversed_range() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-action-reversed-range")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let lsp_path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&lsp_path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        let first_action = lsp
            .get_mut("code_actions")
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|actions| actions.first_mut())
            .ok_or_else(|| "test lsp missing first code action".to_string())?;
        first_action["range"]["end"]["line"] = serde_json::json!(5);
        first_action["range"]["end"]["character"] = serde_json::json!(0);
        fs::write(&lsp_path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("range end must not precede start")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_markdown_missing_card_identity() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-markdown-card-id")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        fs::write(
            dir.join("witness-plan.md"),
            "# unsafe-review witness plan\n\n- Review cards: 1\n- Open actionable gaps: 1\n- Policy mode: `advisory`\n\n## Route groups\n\n### Miri / cargo-careful\n\n- Limit: Concrete runtime evidence is path-specific. It can support the exercised route, but it does not prove arbitrary callers, repo safety, UB-free status, or site execution unless a matching receipt records the run.\n\n- Route: `miri`\n  - Reason: route\n  - What it can show: a focused run\n  - What it cannot prove: arbitrary callers\n  - Command:\n\n```bash\ncargo +nightly miri test card\n```\n  - Receipt hint: unsafe-review receipt import-miri missing\n\n## Trust boundary\n\nThis artifact is static unsafe contract review. It routes reviewers to credible witnesses but does not run Miri, cargo-careful, sanitizers, Loom, Shuttle, Kani, or Crux. It is not a proof of memory safety, not UB-free status, and not a Miri result unless a witness receipt is attached.\n",
        )
        .map_err(|err| format!("write witness plan failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("witness-plan must include a section for ReviewCard `card-1`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_stale_markdown_card_mentions() -> Result<(), String> {
        for (label, file_name) in [
            ("pr-summary", "pr-summary.md"),
            ("github-summary", "github-summary.md"),
            ("witness-plan", "witness-plan.md"),
        ] {
            let dir = unique_temp_dir(&format!("unsafe-review-first-pr-stale-{label}-card-id"))?;
            fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
            write_valid_first_pr_artifacts(&dir)?;

            let path = dir.join(file_name);
            let text = fs::read_to_string(&path)
                .map_err(|err| format!("read {file_name} failed: {err}"))?;
            fs::write(&path, format!("{text}\nStale ReviewCard: `card-2`\n"))
                .map_err(|err| format!("write {file_name} failed: {err}"))?;

            let result = check_first_pr_artifacts(&dir);

            fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
            let err = result.err().unwrap_or_default();
            assert!(
                err.contains(&format!(
                    "{file_name} mentions unknown ReviewCard id `card-2`"
                )),
                "{label} stale card id should be rejected, got: {err}"
            );
        }
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_pr_summary_unknown_top_card_identity() -> Result<(), String>
    {
        let dir = unique_temp_dir("unsafe-review-first-pr-summary-card-id")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        fs::write(
            dir.join("pr-summary.md"),
            "- Scope: `diff`\n- Review cards: 1\n- Open actionable gaps: 1\n- Policy mode: `advisory`\n\n## Top card\n\n- ID: `missing`\n- Class: `guard_missing`\n\nKnown ReviewCard: `card-1`\n\n- Receipt audit: `receipt-audit.md` checks saved receipt metadata only; no witness was run.\n\nThis artifact is static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result unless a witness receipt is attached.\n",
        )
        .map_err(|err| format!("write pr summary failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("top card id `missing` is not present in cards.json")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_pr_summary_top_card_class_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-summary-card-class")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        fs::write(
            dir.join("pr-summary.md"),
            "- Scope: `diff`\n- Review cards: 1\n- Open actionable gaps: 1\n- Policy mode: `advisory`\n\n## Top card\n\n- ID: `card-1`\n- Class: `contract_missing`\n\n- Receipt audit: `receipt-audit.md` checks saved receipt metadata only; no witness was run.\n\nThis artifact is static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result unless a witness receipt is attached.\n",
        )
        .map_err(|err| format!("write pr summary failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("top card `card-1` class must be `guard_missing`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_pr_summary_top_card_location_drift() -> Result<(), String>
    {
        let dir = unique_temp_dir("unsafe-review-first-pr-summary-card-location")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("pr-summary.md");
        let summary =
            fs::read_to_string(&path).map_err(|err| format!("read pr summary failed: {err}"))?;
        fs::write(
            &path,
            summary.replace("- Location: src/lib.rs:7", "- Location: src/other.rs:99"),
        )
        .map_err(|err| format!("write pr summary failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("top card `card-1` location must be `src/lib.rs:7`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_pr_summary_top_card_operation_drift() -> Result<(), String>
    {
        let dir = unique_temp_dir("unsafe-review-first-pr-summary-card-operation")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("pr-summary.md");
        let summary =
            fs::read_to_string(&path).map_err(|err| format!("read pr summary failed: {err}"))?;
        fs::write(
            &path,
            summary.replace(
                "- Operation: `unsafe { ptr.cast::<Header>().read() }`",
                "- Operation: `unsafe { other.cast::<Header>().read() }`",
            ),
        )
        .map_err(|err| format!("write pr summary failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains(
            "top card `card-1` operation must be `unsafe { ptr.cast::<Header>().read() }`"
        ));
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_pr_summary_operation_family_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-summary-operation-family")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("pr-summary.md");
        let summary =
            fs::read_to_string(&path).map_err(|err| format!("read pr summary failed: {err}"))?;
        fs::write(
            &path,
            summary.replace(
                "- Operation family: `raw_pointer_read`",
                "- Operation family: `nonnull`",
            ),
        )
        .map_err(|err| format!("write pr summary failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("top card `card-1` operation family must be `raw_pointer_read`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_pr_summary_next_action_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-summary-next-action")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("pr-summary.md");
        let summary =
            fs::read_to_string(&path).map_err(|err| format!("read pr summary failed: {err}"))?;
        fs::write(
            &path,
            summary.replace(
                "- Next action: Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.",
                "- Next action: Ask the author for unrelated evidence.",
            ),
        )
        .map_err(|err| format!("write pr summary failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("top card `card-1` next action must be `Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_pr_summary_handoff_command_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-summary-handoff")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("pr-summary.md");
        let summary =
            fs::read_to_string(&path).map_err(|err| format!("read pr summary failed: {err}"))?;
        fs::write(
            &path,
            summary.replace(
                "- Explain: `unsafe-review explain card-1`",
                "- Explain: `unsafe-review explain card-2`",
            ),
        )
        .map_err(|err| format!("write pr summary failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result.err().unwrap_or_default().contains(
                "top card `card-1` explain command must be `unsafe-review explain card-1`"
            )
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_pr_summary_agent_handoff_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-summary-agent-handoff")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("pr-summary.md");
        let summary =
            fs::read_to_string(&path).map_err(|err| format!("read pr summary failed: {err}"))?;
        fs::write(
            &path,
            summary.replace(
                "- Agent handoff: `ready_for_agent`; buckets: `repairable_by_guard`, `requires_witness_receipt`; reasons: specific operation family",
                "- Agent handoff: `requires_human_review`; buckets: `requires_human_review`; reasons: unrelated",
            ),
        )
        .map_err(|err| format!("write pr summary failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result.err().unwrap_or_default().contains(
                "top card `card-1` agent handoff must include `- Agent handoff: `ready_for_agent`; buckets: `repairable_by_guard`, `requires_witness_receipt`; reasons: specific operation family`"
            )
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_pr_summary_missing_evidence_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-summary-missing-evidence")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("pr-summary.md");
        let summary =
            fs::read_to_string(&path).map_err(|err| format!("read pr summary failed: {err}"))?;
        fs::write(
            &path,
            summary.replace(
                "- Missing evidence: No missing evidence recorded",
                "- Missing evidence: unrelated missing evidence",
            ),
        )
        .map_err(|err| format!("write pr summary failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result.err().unwrap_or_default().contains(
                "top card `card-1` missing evidence must be `No missing evidence recorded`"
            )
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_pr_summary_primary_route_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-summary-primary-route")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("pr-summary.md");
        let summary =
            fs::read_to_string(&path).map_err(|err| format!("read pr summary failed: {err}"))?;
        fs::write(
            &path,
            summary.replace(
                "- Primary route: `miri` because route",
                "- Primary route: `miri` because unrelated route",
            ),
        )
        .map_err(|err| format!("write pr summary failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("top card `card-1` primary route reason must be `route`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_pr_summary_primary_route_command_drift()
    -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-summary-primary-route-command")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("pr-summary.md");
        let summary =
            fs::read_to_string(&path).map_err(|err| format!("read pr summary failed: {err}"))?;
        fs::write(
            &path,
            summary.replacen(
                "cargo +nightly miri test card",
                "cargo +nightly miri test unrelated_card",
                1,
            ),
        )
        .map_err(|err| format!("write pr summary failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains(
            "top card `card-1` primary route command must include fenced command `cargo +nightly miri test card`"
        ));
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_github_summary_unknown_top_card_identity()
    -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-github-card-id")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("github-summary.md");
        let summary = fs::read_to_string(&path)
            .map_err(|err| format!("read github summary failed: {err}"))?;
        fs::write(&path, summary.replace("- ID: `card-1`", "- ID: `missing`"))
            .map_err(|err| format!("write github summary failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("top card id `missing` is not present in cards.json")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_github_summary_top_card_class_drift() -> Result<(), String>
    {
        let dir = unique_temp_dir("unsafe-review-first-pr-github-card-class")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("github-summary.md");
        let summary = fs::read_to_string(&path)
            .map_err(|err| format!("read github summary failed: {err}"))?;
        fs::write(
            &path,
            summary.replace("- Class: `guard_missing`", "- Class: `contract_missing`"),
        )
        .map_err(|err| format!("write github summary failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("top card `card-1` class must be `guard_missing`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_github_summary_top_card_location_drift()
    -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-github-card-location")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("github-summary.md");
        let summary = fs::read_to_string(&path)
            .map_err(|err| format!("read github summary failed: {err}"))?;
        fs::write(
            &path,
            summary.replace("- Location: src/lib.rs:7", "- Location: src/other.rs:99"),
        )
        .map_err(|err| format!("write github summary failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("top card `card-1` location must be `src/lib.rs:7`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_github_summary_top_card_operation_drift()
    -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-github-card-operation")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("github-summary.md");
        let summary = fs::read_to_string(&path)
            .map_err(|err| format!("read github summary failed: {err}"))?;
        fs::write(
            &path,
            summary.replace(
                "- Operation: `unsafe { ptr.cast::<Header>().read() }`",
                "- Operation: `unsafe { other.cast::<Header>().read() }`",
            ),
        )
        .map_err(|err| format!("write github summary failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains(
            "top card `card-1` operation must be `unsafe { ptr.cast::<Header>().read() }`"
        ));
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_github_summary_operation_family_drift()
    -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-github-operation-family")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("github-summary.md");
        let summary = fs::read_to_string(&path)
            .map_err(|err| format!("read github summary failed: {err}"))?;
        fs::write(
            &path,
            summary.replace(
                "- Operation family: `raw_pointer_read`",
                "- Operation family: `nonnull`",
            ),
        )
        .map_err(|err| format!("write github summary failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("top card `card-1` operation family must be `raw_pointer_read`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_github_summary_next_action_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-github-next-action")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("github-summary.md");
        let summary = fs::read_to_string(&path)
            .map_err(|err| format!("read github summary failed: {err}"))?;
        fs::write(
            &path,
            summary.replace(
                "- Next action: Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.",
                "- Next action: Ask the author for unrelated evidence.",
            ),
        )
        .map_err(|err| format!("write github summary failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("top card `card-1` next action must be `Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_github_summary_handoff_command_drift() -> Result<(), String>
    {
        let dir = unique_temp_dir("unsafe-review-first-pr-github-handoff")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("github-summary.md");
        let summary = fs::read_to_string(&path)
            .map_err(|err| format!("read github summary failed: {err}"))?;
        fs::write(
            &path,
            summary.replace(
                "- Agent context: `unsafe-review context card-1 --json`",
                "- Agent context: `unsafe-review context card-2 --json`",
            ),
        )
        .map_err(|err| format!("write github summary failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains(
            "top card `card-1` agent context command must be `unsafe-review context card-1 --json`"
        ));
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_github_summary_missing_open_next() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-github-open-next")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("github-summary.md");
        let summary = fs::read_to_string(&path)
            .map_err(|err| format!("read github summary failed: {err}"))?;
        fs::write(
            &path,
            summary.replace(
                "## Open next\n\n- Review kit manifest: `review-kit.json`\n- Full reviewer cockpit: `pr-summary.md`\n- Machine-readable ReviewCards: `cards.json`\n- Witness routes: `witness-plan.md`\n- Receipt audit: `receipt-audit.md` checks saved receipt metadata only; no witness was run.\n- Policy report: `policy-report.md`; ReviewCard-only; manual candidates are not policy inputs.\n- Manual candidate index: `manual-candidates.json` lists imported advisory candidates separately from ReviewCards.\n- Agent repair queue: `repair-queue.json` is copy-only; no agent was run.\n- Comment budget: `comment-plan.json` is plan-only; no comments were posted.\n\n",
                "",
            ),
        )
        .map_err(|err| format!("write github summary failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("github-summary.md is missing `## Open next`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_github_summary_missing_evidence_drift()
    -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-github-missing-evidence")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("github-summary.md");
        let summary = fs::read_to_string(&path)
            .map_err(|err| format!("read github summary failed: {err}"))?;
        fs::write(
            &path,
            summary.replace(
                "- Missing evidence: No missing evidence recorded",
                "- Missing evidence: unrelated missing evidence",
            ),
        )
        .map_err(|err| format!("write github summary failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result.err().unwrap_or_default().contains(
                "top card `card-1` missing evidence must be `No missing evidence recorded`"
            )
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_github_summary_primary_route_drift() -> Result<(), String>
    {
        let dir = unique_temp_dir("unsafe-review-first-pr-github-primary-route")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("github-summary.md");
        let summary = fs::read_to_string(&path)
            .map_err(|err| format!("read github summary failed: {err}"))?;
        fs::write(
            &path,
            summary.replace(
                "- Primary route: `miri` because route",
                "- Primary route: `miri` because unrelated route",
            ),
        )
        .map_err(|err| format!("write github summary failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("top card `card-1` primary route reason must be `route`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_github_summary_primary_route_command_drift()
    -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-github-primary-route-command")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("github-summary.md");
        let summary = fs::read_to_string(&path)
            .map_err(|err| format!("read github summary failed: {err}"))?;
        fs::write(
            &path,
            summary.replace(
                "cargo +nightly miri test card",
                "cargo +nightly miri test unrelated_card",
            ),
        )
        .map_err(|err| format!("write github summary failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains(
            "top card `card-1` primary route command must include fenced command `cargo +nightly miri test card`"
        ));
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_pr_summary_card_table_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-card-table-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("pr-summary.md");
        let summary =
            fs::read_to_string(&path).map_err(|err| format!("read pr summary failed: {err}"))?;
        fs::write(
            &path,
            summary.replace(
                "| `card-1` | `guard_missing` | src/lib.rs:7 | `raw_pointer_read` |",
                "| `card-1` | `guard_missing` | src/lib.rs:7 | `nonnull` |",
            ),
        )
        .map_err(|err| format!("write pr summary failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("card table row for `card-1` must include")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_pr_summary_witness_plan_command_drift()
    -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-summary-witness-command-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("pr-summary.md");
        let summary =
            fs::read_to_string(&path).map_err(|err| format!("read pr summary failed: {err}"))?;
        fs::write(
            &path,
            summary.replace(
                "## Witness plan\n\n- `card-1`: `miri` because route\n\n```bash\ncargo +nightly miri test card\n```",
                "## Witness plan\n\n- `card-1`: `miri` because route\n\n```bash\ncargo +nightly miri test unrelated_card\n```",
            ),
        )
        .map_err(|err| format!("write pr summary failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result.err().unwrap_or_default().contains(
                "pr-summary witness plan for `card-1` primary route command must include"
            )
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_without_obligation_evidence() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-obligation-evidence")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        fs::write(
            dir.join("lsp.json"),
            r#"{"schema_version":"0.1","tool":"unsafe-review","mode":"read_only_projection","policy":"advisory","scope":"diff","status":{"state":"actionable","cards":1,"open_actionable_gaps":1,"high_priority_cards":1,"message":"1 unsafe-review card(s), 1 open actionable gap(s)","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"},"diagnostics":[{"card_id":"card-1","path":"src/lib.rs","range":{"start":{"line":6,"character":0},"end":{"line":6,"character":1}},"code":"guard_missing","operation":"unsafe { ptr.cast::<Header>().read() }","operation_family":"raw_pointer_read","next_action":"Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.","hazards":["alignment"],"witness_routes":[{"kind":"miri","reason":"route","command":"cargo +nightly miri test card","required":false}],"verify_commands":["cargo +nightly miri test card"],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}],"hovers":[{"card_id":"card-1","path":"src/lib.rs","position":{"line":6,"character":0},"contents":"Card: `card-1`\n\nRelevant hazard families:\n- `alignment`\n\nTrust boundary: static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}],"code_actions":[{"card_id":"card-1","path":"src/lib.rs","range":{"start":{"line":6,"character":0},"end":{"line":6,"character":1}},"title":"Copy unsafe-review packet for card-1","kind":"quickfix","command":"unsafe-review.copyAgentPacket","payload":{"kind":"unsafe-review.agent_packet","card_id":"card-1","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"},"arguments":["card-1"]},{"card_id":"card-1","path":"src/lib.rs","range":{"start":{"line":6,"character":0},"end":{"line":6,"character":1}},"title":"Explain unsafe-review witness route","kind":"quickfix","command":"unsafe-review.explainWitnessRoute","payload":{"kind":"unsafe-review.witness_route","card_id":"card-1","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"},"arguments":["card-1"]}],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
        )
        .map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("required_safety_conditions")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_required_witness_route() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-required-route")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let lsp_path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&lsp_path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        let first_route = lsp
            .pointer_mut("/diagnostics/0/witness_routes/0")
            .ok_or_else(|| "test lsp missing first witness route".to_string())?;
        first_route["required"] = serde_json::json!(true);
        fs::write(&lsp_path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("required must remain false")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_unbacked_verify_command() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-unbacked-verify-command")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let lsp_path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&lsp_path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        lsp["diagnostics"][0]["verify_commands"] =
            serde_json::json!(["cargo +nightly miri test other"]);
        fs::write(&lsp_path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("must be backed by a witness route command")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_missing_verify_command() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-missing-verify-command")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let lsp_path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&lsp_path).map_err(|err| format!("read lsp failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp failed: {err}"))?;
        lsp["diagnostics"][0]["verify_commands"] = serde_json::json!([]);
        fs::write(&lsp_path, lsp.to_string()).map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains(
            "witness route command `cargo +nightly miri test card` must appear in verify_commands"
        ));
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_code_action_without_payload() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-action-payload")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        fs::write(
            dir.join("lsp.json"),
            valid_lsp_json(
                r#"[{"card_id":"card-1","path":"src/lib.rs","range":{"start":{"line":6,"character":0},"end":{"line":6,"character":1}},"title":"Copy unsafe-review packet for card-1","kind":"quickfix","command":"unsafe-review.copyAgentPacket","arguments":["card-1"]}]"#,
            )?,
        )
        .map_err(|err| format!("write lsp failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains("missing payload"));
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_positive_overclaims() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-overclaim")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        fs::write(
            dir.join("witness-plan.md"),
            "# unsafe-review witness plan\n\n- Review cards: 1\n- Open actionable gaps: 1\n- Policy mode: `advisory`\n\n## Route groups\n\n### Miri / cargo-careful\n\n- Limit: Concrete runtime evidence is path-specific.\n\n#### `card-1`\n\n- Class: `guard_missing`\n- Location: src/lib.rs:7\n- Operation: `unsafe { ptr.cast::<Header>().read() }`\n- Next action: Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.\n- Route: `miri`\n  - Reason: route\n  - What it can show: a focused run\n  - What it cannot prove: arbitrary callers\n  - Command:\n\n```bash\ncargo +nightly miri test card\n```\n  - Receipt hint: unsafe-review receipt import-miri card-1\n\nAll clear.\n\n## Trust boundary\n\nThis artifact is static unsafe contract review. It routes reviewers to credible witnesses but does not run Miri, cargo-careful, sanitizers, Loom, Shuttle, Kani, or Crux. It is not a proof of memory safety, not UB-free status, and not a Miri result unless a witness receipt is attached.\n",
        )
        .map_err(|err| format!("write witness plan failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains("all clear"));
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_github_summary_positive_overclaim() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-github-summary-overclaim")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("github-summary.md");
        let mut summary = fs::read_to_string(&path)
            .map_err(|err| format!("read github summary failed: {err}"))?;
        summary.push_str("\nAll clear.\n");
        fs::write(&path, summary).map_err(|err| format!("write github summary failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains("all clear"));
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_cards_json_positive_overclaim() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-cards-json-overclaim")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("cards.json");
        let mut cards = parse_json_file(&path)?;
        cards["note"] = serde_json::json!("safe to merge");
        fs::write(
            &path,
            serde_json::to_string(&cards)
                .map_err(|err| format!("serialize cards json failed: {err}"))?,
        )
        .map_err(|err| format!("write cards failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains("safe to merge"));
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_sarif_positive_overclaim() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-sarif-overclaim")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        let path = dir.join("cards.sarif");
        let mut sarif = parse_json_file(&path)?;
        sarif["note"] = serde_json::json!("proved safe");
        fs::write(
            &path,
            serde_json::to_string(&sarif)
                .map_err(|err| format!("serialize sarif failed: {err}"))?,
        )
        .map_err(|err| format!("write sarif failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains("proved safe"));
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_missing_trust_boundary() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-missing-boundary")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        fs::write(
            dir.join("pr-summary.md"),
            "- Scope: `diff`\n- Review cards: 1\n- Open actionable gaps: 1\n- Policy mode: `advisory`\n",
        )
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
    fn advisory_artifact_checker_rejects_cards_json_without_trust_boundary() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-missing-cards-boundary")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        fs::write(
            dir.join("cards.json"),
            r#"{"schema_version":"0.1","tool":"unsafe-review","policy":"advisory","summary":{"cards":1},"cards":[{"id":"card-1"}]}"#,
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
    fn advisory_artifact_checker_rejects_unknown_projection_card_ids() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-unknown-id")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        fs::write(
            dir.join("comment-plan.json"),
            r#"{"schema_version":"0.1","mode":"plan_only","policy":"advisory","comments":[{"card_id":"missing","body":"Plan boundary: artifact-only inline comment candidate; unsafe-review did not post this comment, run witnesses, or make a policy decision."}],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
        )
        .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains("unknown card id"));
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
            r#"{"schema_version":"0.1","mode":"plan_only","policy":"advisory","comments":[{"card_id":"card-1","witness_routes":[{"kind":"miri","reason":"route","command":"cargo +nightly miri test card","required":false}],"verify_commands":["cargo +nightly miri test card"],"body":"Plan boundary: artifact-only inline comment candidate; unsafe-review did not post this comment, run witnesses, or make a policy decision."}],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
        )
        .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains("missing path"));
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_sarif_without_route_details() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-sarif-routes")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        fs::write(
            dir.join("cards.sarif"),
            r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"rules":[{"id":"guard_missing"}]}},"results":[{"ruleId":"guard_missing","locations":[{"physicalLocation":{"artifactLocation":{"uri":"src/lib.rs"},"region":{"startLine":7,"startColumn":5}}}],"properties":{"cardId":"card-1","class":"guard_missing","priority":"high","confidence":"medium","operationFamily":"raw_pointer_read","operation":"unsafe { ptr.cast::<Header>().read() }","nextAction":"Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.","verifyCommands":["cargo +nightly miri test card"],"trustBoundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}}],"properties":{"trustBoundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}}]}"#,
        )
        .map_err(|err| format!("write sarif failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("witnessRouteDetails")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_sarif_result_without_trust_boundary() -> Result<(), String>
    {
        let dir = unique_temp_dir("unsafe-review-artifacts-sarif-result-boundary")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        let path = dir.join("cards.sarif");
        let mut sarif: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read sarif failed: {err}"))?,
        )
        .map_err(|err| format!("parse sarif failed: {err}"))?;
        sarif["runs"][0]["results"][0]["properties"]
            .as_object_mut()
            .ok_or_else(|| "sarif result properties fixture must be an object".to_string())?
            .remove("trustBoundary");
        fs::write(&path, sarif.to_string()).map_err(|err| format!("write sarif failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("cards.sarif result properties is missing trustBoundary")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_sarif_scope_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-sarif-scope-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        let path = dir.join("cards.sarif");
        let mut sarif: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read sarif failed: {err}"))?,
        )
        .map_err(|err| format!("parse sarif failed: {err}"))?;
        sarif["runs"][0]["properties"]["scope"] = serde_json::json!("repo");
        fs::write(&path, sarif.to_string()).map_err(|err| format!("write sarif failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("cards.sarif /runs/0/properties/scope must be `diff`; got `repo`")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_sarif_witness_route_projection_drift() -> Result<(), String>
    {
        let dir = unique_temp_dir("unsafe-review-artifacts-sarif-route-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        let path = dir.join("cards.sarif");
        let mut sarif: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read sarif failed: {err}"))?,
        )
        .map_err(|err| format!("parse sarif failed: {err}"))?;
        sarif["runs"][0]["results"][0]["properties"]["witnessRouteDetails"][0]["command"] =
            serde_json::json!("cargo +nightly miri test unrelated_card");
        fs::write(&path, sarif.to_string()).map_err(|err| format!("write sarif failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains(
            "cards.sarif result properties witnessRouteDetails[0] command must project cards.json value"
        ));
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_sarif_witness_route_summary_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-sarif-route-summary-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        let path = dir.join("cards.sarif");
        let mut sarif: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read sarif failed: {err}"))?,
        )
        .map_err(|err| format!("parse sarif failed: {err}"))?;
        sarif["runs"][0]["results"][0]["properties"]["witnessRoutes"] =
            serde_json::json!(["miri: unrelated route"]);
        fs::write(&path, sarif.to_string()).map_err(|err| format!("write sarif failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result.err().unwrap_or_default().contains(
                "cards.sarif result properties witnessRoutes must project cards.json value"
            )
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_sarif_hazard_projection_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-sarif-hazard-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        let path = dir.join("cards.sarif");
        let mut sarif: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read sarif failed: {err}"))?,
        )
        .map_err(|err| format!("parse sarif failed: {err}"))?;
        sarif["runs"][0]["results"][0]["properties"]["hazards"] =
            serde_json::json!(["pointer_validity"]);
        fs::write(&path, sarif.to_string()).map_err(|err| format!("write sarif failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("cards.sarif result properties hazards must project cards.json value")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_sarif_missing_evidence_projection_drift()
    -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-sarif-missing-evidence-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        let path = dir.join("cards.sarif");
        let mut sarif: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read sarif failed: {err}"))?,
        )
        .map_err(|err| format!("parse sarif failed: {err}"))?;
        sarif["runs"][0]["results"][0]["properties"]["missingEvidence"] =
            serde_json::json!(["unrelated evidence"]);
        fs::write(&path, sarif.to_string()).map_err(|err| format!("write sarif failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains(
            "cards.sarif result properties missingEvidence must project cards.json value"
        ));
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_sarif_rule_id_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-sarif-rule-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        fs::write(
            dir.join("cards.sarif"),
            r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"rules":[{"id":"guard_missing"}]}},"results":[{"ruleId":"contract_missing","properties":{"cardId":"card-1","class":"guard_missing","witnessRoutes":["miri: route"],"witnessRouteDetails":[{"kind":"miri","reason":"route","command":"cargo +nightly miri test card","required":false}],"verifyCommands":["cargo +nightly miri test card"]}}],"properties":{"trustBoundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}}]}"#,
        )
        .map_err(|err| format!("write sarif failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains("ruleId"));
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_sarif_unused_rule_id() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-sarif-unused-rule")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        let path = dir.join("cards.sarif");
        let mut sarif: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read sarif failed: {err}"))?,
        )
        .map_err(|err| format!("parse sarif failed: {err}"))?;
        sarif["runs"][0]["tool"]["driver"]["rules"]
            .as_array_mut()
            .ok_or_else(|| "sarif rules fixture must be an array".to_string())?
            .push(serde_json::json!({"id":"contract_missing"}));
        fs::write(&path, sarif.to_string()).map_err(|err| format!("write sarif failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("cards.sarif declares unused rule id `contract_missing`")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_sarif_duplicate_card_id() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-sarif-duplicate-card")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_two_card_artifacts(&dir)?;
        fs::write(
            dir.join("cards.sarif"),
            r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"rules":[{"id":"guard_missing"},{"id":"contract_missing"}]}},"results":[{"ruleId":"guard_missing","locations":[{"physicalLocation":{"artifactLocation":{"uri":"src/lib.rs"},"region":{"startLine":7,"startColumn":5}}}],"properties":{"cardId":"card-1","class":"guard_missing","priority":"high","confidence":"medium","operationFamily":"raw_pointer_read","operation":"unsafe { ptr.cast::<Header>().read() }","hazards":["alignment"],"missingEvidence":[],"nextAction":"Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.","witnessRoutes":["miri: route"],"witnessRouteDetails":[{"kind":"miri","reason":"route","command":"cargo +nightly miri test card","required":false}],"verifyCommands":["cargo +nightly miri test card"],"trustBoundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}},{"ruleId":"guard_missing","locations":[{"physicalLocation":{"artifactLocation":{"uri":"src/lib.rs"},"region":{"startLine":7,"startColumn":5}}}],"properties":{"cardId":"card-1","class":"guard_missing","priority":"high","confidence":"medium","operationFamily":"raw_pointer_read","operation":"unsafe { ptr.cast::<Header>().read() }","hazards":["alignment"],"missingEvidence":[],"nextAction":"Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.","witnessRoutes":["miri: route"],"witnessRouteDetails":[{"kind":"miri","reason":"route","command":"cargo +nightly miri test card","required":false}],"verifyCommands":["cargo +nightly miri test card"],"trustBoundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}}],"properties":{"trustBoundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}}]}"#,
        )
        .map_err(|err| format!("write sarif failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("results repeat card id `card-1`")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_sarif_location_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-sarif-location-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        let sarif_path = dir.join("cards.sarif");
        let mut sarif: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&sarif_path).map_err(|err| format!("read sarif failed: {err}"))?,
        )
        .map_err(|err| format!("parse sarif failed: {err}"))?;
        sarif["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["region"]["startLine"] =
            serde_json::json!(8);
        fs::write(&sarif_path, sarif.to_string())
            .map_err(|err| format!("write sarif failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("location startLine must project cards.json value")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_sarif_operation_projection_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-sarif-operation-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        let sarif_path = dir.join("cards.sarif");
        let mut sarif: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&sarif_path).map_err(|err| format!("read sarif failed: {err}"))?,
        )
        .map_err(|err| format!("parse sarif failed: {err}"))?;
        sarif["runs"][0]["results"][0]["properties"]["operationFamily"] = serde_json::json!("ffi");
        fs::write(&sarif_path, sarif.to_string())
            .map_err(|err| format!("write sarif failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("operationFamily must be `raw_pointer_read`")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_comment_plan_without_route_details() -> Result<(), String>
    {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-routes")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        fs::write(
            dir.join("comment-plan.json"),
            r#"{"schema_version":"0.1","mode":"plan_only","policy":"advisory","comments":[{"card_id":"card-1","path":"src/lib.rs","line":7,"changed_line":true,"class":"guard_missing","priority":"high","confidence":"medium","operation":"unsafe { ptr.cast::<Header>().read() }","next_action":"Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.","verify_commands":["cargo +nightly miri test card"],"operation_family":"raw_pointer_read","body":"Plan boundary: artifact-only inline comment candidate; unsafe-review did not post this comment, run witnesses, or make a policy decision."}],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
        )
        .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains("witness_routes"));
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
            r#"{"schema_version":"0.1","mode":"plan_only","policy":"advisory","comments":[{"card_id":"card-1","path":"src/lib.rs","line":7,"changed_line":true,"class":"guard_missing","priority":"high","confidence":"medium","operation":"unsafe { ptr.cast::<Header>().read() }","next_action":"Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.","operation_family":"raw_pointer_read","witness_routes":[{"kind":"miri","reason":"route","command":"cargo +nightly miri test card","required":false}],"verify_commands":["cargo +nightly miri test card"],"body":"Plan boundary: artifact-only inline comment candidate; unsafe-review did not run witnesses or make a policy decision."}],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
        )
        .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("did not post this comment")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_comment_plan_without_witness_boundary()
    -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-witness-boundary")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        let path = dir.join("comment-plan.json");
        let comment_plan =
            fs::read_to_string(&path).map_err(|err| format!("read comment plan failed: {err}"))?;
        fs::write(
            &path,
            comment_plan.replace(
                "unsafe-review did not post this comment, run witnesses, or make a policy decision",
                "unsafe-review did not post this comment or make a policy decision",
            ),
        )
        .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains("run witnesses"));
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_comment_plan_without_next_action() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-next-action")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        fs::write(
            dir.join("comment-plan.json"),
            r#"{"schema_version":"0.1","mode":"plan_only","policy":"advisory","comments":[{"card_id":"card-1","path":"src/lib.rs","line":7,"changed_line":true,"class":"guard_missing","priority":"high","confidence":"medium","operation":"unsafe { ptr.cast::<Header>().read() }","operation_family":"raw_pointer_read","witness_routes":[{"kind":"miri","reason":"route","command":"cargo +nightly miri test card","required":false}],"verify_commands":["cargo +nightly miri test card"],"selection_reason":"actionable high-priority review card","selection_reason_code":"top_actionable_card","actionability":"specific_guard_missing","relevance":"medium","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result","body":"Next action: Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.\n\nPlan boundary: artifact-only inline comment candidate; unsafe-review did not post this comment, run witnesses, or make a policy decision."}],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
        )
        .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains("next_action"));
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_comment_plan_next_action_body_drift() -> Result<(), String>
    {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-next-action-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        fs::write(
            dir.join("comment-plan.json"),
            r#"{"schema_version":"0.1","mode":"plan_only","policy":"advisory","comments":[{"card_id":"card-1","path":"src/lib.rs","line":7,"changed_line":true,"class":"guard_missing","priority":"high","confidence":"medium","operation":"unsafe { ptr.cast::<Header>().read() }","operation_family":"raw_pointer_read","witness_routes":[{"kind":"miri","reason":"route","command":"cargo +nightly miri test card","required":false}],"next_action":"Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.","verify_commands":["cargo +nightly miri test card"],"selection_reason":"actionable high-priority review card","selection_reason_code":"top_actionable_card","actionability":"specific_guard_missing","relevance":"medium","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result","body":"Next action: Run broad tests.\n\nPlan boundary: artifact-only inline comment candidate; unsafe-review did not post this comment, run witnesses, or make a policy decision."}],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
        )
        .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("structured next_action")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_comment_plan_next_action_projection_drift()
    -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-next-action-projection-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        let path = dir.join("comment-plan.json");
        let mut comment_plan: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read comment plan failed: {err}"))?,
        )
        .map_err(|err| format!("parse comment plan failed: {err}"))?;
        comment_plan["comments"][0]["next_action"] = serde_json::json!("Run broad tests.");
        comment_plan["comments"][0]["body"] = serde_json::json!(
            "Next action: Run broad tests.\n\nPlan boundary: artifact-only inline comment candidate; unsafe-review did not post this comment, run witnesses, or make a policy decision."
        );
        fs::write(&path, comment_plan.to_string())
            .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains(
            "comment-plan.json comment next_action must be `Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.`"
        ));
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_comment_plan_verify_command_projection_drift()
    -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-verify-command-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        let path = dir.join("comment-plan.json");
        let mut comment_plan: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read comment plan failed: {err}"))?,
        )
        .map_err(|err| format!("parse comment plan failed: {err}"))?;
        comment_plan["comments"][0]["verify_commands"] =
            serde_json::json!(["cargo +nightly miri test unrelated_card"]);
        fs::write(&path, comment_plan.to_string())
            .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result.err().unwrap_or_default().contains(
                "comment-plan.json comment verify_commands must project cards.json value"
            )
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_comment_plan_witness_route_projection_drift()
    -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-witness-route-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        let path = dir.join("comment-plan.json");
        let mut comment_plan: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read comment plan failed: {err}"))?,
        )
        .map_err(|err| format!("parse comment plan failed: {err}"))?;
        comment_plan["comments"][0]["witness_routes"][0]["reason"] =
            serde_json::json!("unrelated route");
        fs::write(&path, comment_plan.to_string())
            .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains(
            "comment-plan.json comment witness_routes[0] reason must be `route`; got `unrelated route`"
        ));
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_required_witness_routes() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-required-witness-route")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        let path = dir.join("cards.json");
        let mut cards: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read cards failed: {err}"))?,
        )
        .map_err(|err| format!("parse cards failed: {err}"))?;
        cards["cards"][0]["witness_routes"][0]["required"] = serde_json::json!(true);
        fs::write(&path, cards.to_string()).map_err(|err| format!("write cards failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("witness_routes[] required must remain false")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_comment_plan_witness_required_drift() -> Result<(), String>
    {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-witness-required-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        let path = dir.join("comment-plan.json");
        let mut comment_plan: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read comment plan failed: {err}"))?,
        )
        .map_err(|err| format!("parse comment plan failed: {err}"))?;
        comment_plan["comments"][0]["witness_routes"][0]["required"] = serde_json::json!(true);
        fs::write(&path, comment_plan.to_string())
            .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("comment-plan.json comment witness_routes[0] required must project cards.json value `false`; got `true`")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_comment_plan_forbidden_class() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-forbidden-class")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        fs::write(
            dir.join("comment-plan.json"),
            r#"{"schema_version":"0.1","mode":"plan_only","policy":"advisory","comments":[{"card_id":"card-1","path":"src/lib.rs","line":7,"changed_line":true,"class":"static_unknown","priority":"high","confidence":"medium","operation":"unsafe { ptr.cast::<Header>().read() }","operation_family":"raw_pointer_read","witness_routes":[{"kind":"miri","reason":"route","command":"cargo +nightly miri test card","required":false}],"next_action":"Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.","verify_commands":["cargo +nightly miri test card"],"selection_reason":"actionable high-priority review card","selection_reason_code":"top_actionable_card","actionability":"human_review_only","relevance":"medium","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result","body":"Next action: Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.\n\nPlan boundary: artifact-only inline comment candidate; unsafe-review did not post this comment, run witnesses, or make a policy decision."}],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
        )
        .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains("static_unknown"));
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_comment_plan_without_relevance() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-missing-relevance")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        fs::write(
            dir.join("comment-plan.json"),
            r#"{"schema_version":"0.1","mode":"plan_only","policy":"advisory","comments":[{"card_id":"card-1","path":"src/lib.rs","line":7,"changed_line":true,"class":"guard_missing","priority":"high","confidence":"medium","operation":"unsafe { ptr.cast::<Header>().read() }","operation_family":"raw_pointer_read","witness_routes":[{"kind":"miri","reason":"route","command":"cargo +nightly miri test card","required":false}],"next_action":"Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.","verify_commands":["cargo +nightly miri test card"],"selection_reason":"actionable high-priority review card","selection_reason_code":"top_actionable_card","actionability":"specific_guard_missing","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result","body":"Next action: Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.\n\nPlan boundary: artifact-only inline comment candidate; unsafe-review did not post this comment, run witnesses, or make a policy decision."}],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
        )
        .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains("relevance"));
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_comment_plan_projection_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-projection-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        fs::write(
            dir.join("comment-plan.json"),
            r#"{"schema_version":"0.1","mode":"plan_only","policy":"advisory","comments":[{"card_id":"card-1","path":"src/lib.rs","line":7,"changed_line":true,"class":"guard_missing","priority":"high","confidence":"medium","operation":"unsafe { ptr.cast::<Header>().read() }","operation_family":"ffi","witness_routes":[{"kind":"miri","reason":"route","command":"cargo +nightly miri test card","required":false}],"next_action":"Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.","verify_commands":["cargo +nightly miri test card"],"selection_reason":"actionable high-priority review card","selection_reason_code":"top_actionable_card","actionability":"specific_guard_missing","relevance":"medium","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result","body":"Next action: Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.\n\nPlan boundary: artifact-only inline comment candidate; unsafe-review did not post this comment, run witnesses, or make a policy decision."}],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
        )
        .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("operation_family")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_comment_body_projection_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-body-projection-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        let path = dir.join("comment-plan.json");
        let mut comment_plan: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read comment plan failed: {err}"))?,
        )
        .map_err(|err| format!("parse comment plan failed: {err}"))?;
        comment_plan["comments"][0]["body"] = serde_json::json!(
            "Next action: Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.\n\nPlan boundary: artifact-only inline comment candidate; unsafe-review did not post this comment, run witnesses, or make a policy decision.\n\nTrust boundary: static unsafe contract review only; not memory-safety proof, not UB-free status, and not a Miri result unless a witness receipt is attached."
        );
        fs::write(&path, comment_plan.to_string())
            .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("body must project ReviewCard class")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_comment_body_unknown_card_mentions() -> Result<(), String>
    {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-body-unknown-card")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        let path = dir.join("comment-plan.json");
        let mut comment_plan: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read comment plan failed: {err}"))?,
        )
        .map_err(|err| format!("parse comment plan failed: {err}"))?;
        let body = comment_plan["comments"][0]["body"]
            .as_str()
            .ok_or_else(|| "test comment body missing".to_string())?;
        comment_plan["comments"][0]["body"] =
            serde_json::json!(format!("{body}\n\nRelated card: `card-2`\n"));
        fs::write(&path, comment_plan.to_string())
            .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("mentions unknown ReviewCard id `card-2`")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_comment_plan_missing_card_coverage() -> Result<(), String>
    {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-missing-card-coverage")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_two_card_artifacts(&dir)?;
        let path = dir.join("comment-plan.json");
        let mut comment_plan: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read comment plan failed: {err}"))?,
        )
        .map_err(|err| format!("parse comment plan failed: {err}"))?;
        comment_plan
            .as_object_mut()
            .ok_or_else(|| "comment plan fixture must be an object".to_string())?
            .remove("not_selected");
        fs::write(&path, comment_plan.to_string())
            .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains(
            "comment-plan.json must account for ReviewCard id `card-2` in comments[] or not_selected[]"
        ));
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_comment_plan_missing_summary() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-missing-summary")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        let path = dir.join("comment-plan.json");
        let mut comment_plan: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read comment plan failed: {err}"))?,
        )
        .map_err(|err| format!("parse comment plan failed: {err}"))?;
        comment_plan
            .as_object_mut()
            .ok_or_else(|| "comment plan fixture must be an object".to_string())?
            .remove("summary");
        fs::write(&path, comment_plan.to_string())
            .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("comment-plan.json is missing summary")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_comment_plan_summary_count_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-summary-count-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        let path = dir.join("comment-plan.json");
        let mut comment_plan: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read comment plan failed: {err}"))?,
        )
        .map_err(|err| format!("parse comment plan failed: {err}"))?;
        comment_plan["summary"]["selected_count"] = serde_json::json!(0);
        fs::write(&path, comment_plan.to_string())
            .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("summary.selected_count")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_comment_plan_summary_reason_code_drift()
    -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-summary-reason-code")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        let path = dir.join("comment-plan.json");
        let mut comment_plan: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read comment plan failed: {err}"))?,
        )
        .map_err(|err| format!("parse comment plan failed: {err}"))?;
        comment_plan["summary"]["reason_code"] = serde_json::json!("magic_code");
        fs::write(&path, comment_plan.to_string())
            .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("comment-plan.json summary reason_code")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_planned_comment_outside_changed_line() -> Result<(), String>
    {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-unchanged-selected")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        let path = dir.join("comment-plan.json");
        let mut comment_plan: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read comment plan failed: {err}"))?,
        )
        .map_err(|err| format!("parse comment plan failed: {err}"))?;
        comment_plan["comments"][0]["changed_line"] = serde_json::json!(false);
        fs::write(&path, comment_plan.to_string())
            .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("planned comments must have changed_line=true")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_not_selected_changed_line_reason_drift()
    -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-unchanged-reason")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_two_card_artifacts(&dir)?;
        let path = dir.join("comment-plan.json");
        let mut comment_plan: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read comment plan failed: {err}"))?,
        )
        .map_err(|err| format!("parse comment plan failed: {err}"))?;
        comment_plan["not_selected"][0]["changed_line"] = serde_json::json!(false);
        fs::write(&path, comment_plan.to_string())
            .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("not_selected reason")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_not_selected_reason_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-reason-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        fs::write(
            dir.join("comment-plan.json"),
            r#"{"schema_version":"0.1","mode":"plan_only","policy":"advisory","comments":[],"not_selected":[{"card_id":"card-1","path":"src/lib.rs","line":7,"changed_line":true,"class":"guard_missing","priority":"high","confidence":"medium","operation":"unsafe { ptr.cast::<Header>().read() }","operation_family":"raw_pointer_read","next_action":"Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.","actionability":"specific_guard_missing","relevance":"medium","reason":"priority/confidence below inline comment threshold","reason_code":"lower_relevance"}],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
        )
        .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("not_selected reason")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_unknown_comment_reason_vocabulary() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-reason-vocabulary")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        let path = dir.join("comment-plan.json");
        let mut comment_plan: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read comment plan failed: {err}"))?,
        )
        .map_err(|err| format!("parse comment plan failed: {err}"))?;
        comment_plan["comments"][0]["selection_reason"] = serde_json::json!("magic reason");
        fs::write(&path, comment_plan.to_string())
            .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("known review-budget reason")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_unknown_comment_reason_code() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-reason-code")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        let path = dir.join("comment-plan.json");
        let mut comment_plan: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read comment plan failed: {err}"))?,
        )
        .map_err(|err| format!("parse comment plan failed: {err}"))?;
        comment_plan["comments"][0]["selection_reason_code"] = serde_json::json!("magic_code");
        fs::write(&path, comment_plan.to_string())
            .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("comment-plan.json comment selection_reason_code")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_unknown_not_selected_reason_vocabulary()
    -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-not-selected-reason-vocabulary")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_two_card_artifacts(&dir)?;
        let path = dir.join("comment-plan.json");
        let mut comment_plan: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read comment plan failed: {err}"))?,
        )
        .map_err(|err| format!("parse comment plan failed: {err}"))?;
        comment_plan["not_selected"][0]["reason"] = serde_json::json!("magic reason");
        fs::write(&path, comment_plan.to_string())
            .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("known review-budget reason")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_unknown_not_selected_reason_code() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-not-selected-reason-code")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_two_card_artifacts(&dir)?;
        let path = dir.join("comment-plan.json");
        let mut comment_plan: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read comment plan failed: {err}"))?,
        )
        .map_err(|err| format!("parse comment plan failed: {err}"))?;
        comment_plan["not_selected"][0]["reason_code"] = serde_json::json!("magic_code");
        fs::write(&path, comment_plan.to_string())
            .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("comment-plan.json not_selected reason_code")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_not_selected_operation_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-not-selected-operation-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        fs::write(
            dir.join("comment-plan.json"),
            r#"{"schema_version":"0.1","mode":"plan_only","policy":"advisory","comments":[],"not_selected":[{"card_id":"card-1","path":"src/lib.rs","line":7,"changed_line":true,"class":"guard_missing","priority":"high","confidence":"medium","operation":"unsafe { unrelated.read() }","operation_family":"raw_pointer_read","next_action":"Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.","actionability":"specific_guard_missing","relevance":"medium","reason":"not selected by current inline comment policy","reason_code":"not_selected_by_policy"}],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
        )
        .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("not_selected operation")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_not_selected_next_action_drift() -> Result<(), String> {
        let dir =
            unique_temp_dir("unsafe-review-artifacts-comment-not-selected-next-action-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        fs::write(
            dir.join("comment-plan.json"),
            r#"{"schema_version":"0.1","mode":"plan_only","policy":"advisory","comments":[],"not_selected":[{"card_id":"card-1","path":"src/lib.rs","line":7,"changed_line":true,"class":"guard_missing","priority":"high","confidence":"medium","operation":"unsafe { ptr.cast::<Header>().read() }","operation_family":"raw_pointer_read","next_action":"Run broad tests.","actionability":"specific_guard_missing","relevance":"medium","reason":"not selected by current inline comment policy","reason_code":"not_selected_by_policy"}],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
        )
        .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("not_selected next_action")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_not_selected_unknown_relevance() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-invalid-relevance")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        fs::write(
            dir.join("comment-plan.json"),
            r#"{"schema_version":"0.1","mode":"plan_only","policy":"advisory","comments":[],"not_selected":[{"card_id":"card-1","path":"src/lib.rs","line":7,"changed_line":true,"class":"guard_missing","priority":"high","confidence":"medium","operation":"unsafe { ptr.cast::<Header>().read() }","operation_family":"raw_pointer_read","next_action":"Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.","actionability":"specific_guard_missing","relevance":"urgent","reason":"not selected by current inline comment policy","reason_code":"not_selected_by_policy"}],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
        )
        .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains("high/medium/low"));
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_unknown_not_selected_card_id() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-not-selected-unknown")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        fs::write(
            dir.join("comment-plan.json"),
            r#"{"schema_version":"0.1","mode":"plan_only","policy":"advisory","comments":[],"not_selected":[{"card_id":"missing","path":"src/lib.rs","line":7,"changed_line":true,"class":"miri_unsupported","priority":"medium","confidence":"medium","operation_family":"ffi","actionability":"specific_witness_missing","relevance":"low","reason":"priority/confidence below inline comment threshold","reason_code":"lower_relevance"}],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
        )
        .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("not_selected references unknown card id")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_planned_card_repeated_as_not_selected()
    -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-not-selected-repeat")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        fs::write(
            dir.join("comment-plan.json"),
            r#"{"schema_version":"0.1","mode":"plan_only","policy":"advisory","comments":[{"card_id":"card-1","path":"src/lib.rs","line":7,"changed_line":true,"class":"guard_missing","priority":"high","confidence":"medium","operation":"unsafe { ptr.cast::<Header>().read() }","operation_family":"raw_pointer_read","witness_routes":[{"kind":"miri","reason":"route","command":"cargo +nightly miri test card","required":false}],"next_action":"Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.","verify_commands":["cargo +nightly miri test card"],"selection_reason":"actionable high-priority review card","selection_reason_code":"top_actionable_card","actionability":"specific_guard_missing","relevance":"medium","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result","body":"Next action: Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.\n\nPlan boundary: artifact-only inline comment candidate; unsafe-review did not post this comment, run witnesses, or make a policy decision."}],"not_selected":[{"card_id":"card-1","path":"src/lib.rs","line":7,"changed_line":true,"class":"guard_missing","priority":"high","confidence":"medium","operation":"unsafe { ptr.cast::<Header>().read() }","operation_family":"raw_pointer_read","next_action":"Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.","actionability":"specific_guard_missing","relevance":"medium","reason":"comment-plan max of three candidates reached","reason_code":"budget_exhausted"}],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
        )
        .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("not_selected repeats planned comment card id")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_duplicate_comment_card_ids() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-duplicate-card")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        fs::write(
            dir.join("comment-plan.json"),
            r#"{"schema_version":"0.1","mode":"plan_only","policy":"advisory","comments":[{"card_id":"card-1","path":"src/lib.rs","line":7,"changed_line":true,"class":"guard_missing","priority":"high","confidence":"medium","operation":"unsafe { ptr.cast::<Header>().read() }","operation_family":"raw_pointer_read","witness_routes":[{"kind":"miri","reason":"route","command":"cargo +nightly miri test card","required":false}],"next_action":"Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.","verify_commands":["cargo +nightly miri test card"],"selection_reason":"actionable high-priority review card","selection_reason_code":"top_actionable_card","actionability":"specific_guard_missing","relevance":"medium","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result","body":"Next action: Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.\n\nPlan boundary: artifact-only inline comment candidate; unsafe-review did not post this comment, run witnesses, or make a policy decision."},{"card_id":"card-1","path":"src/lib.rs","line":8,"changed_line":true,"class":"guard_missing","priority":"high","confidence":"medium","operation":"unsafe { ptr.cast::<Header>().read() }","operation_family":"raw_pointer_read","witness_routes":[{"kind":"miri","reason":"route","command":"cargo +nightly miri test card","required":false}],"next_action":"Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.","verify_commands":["cargo +nightly miri test card"],"selection_reason":"actionable high-priority review card","selection_reason_code":"top_actionable_card","actionability":"specific_guard_missing","relevance":"medium","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result","body":"Next action: Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.\n\nPlan boundary: artifact-only inline comment candidate; unsafe-review did not post this comment, run witnesses, or make a policy decision."}],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
        )
        .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains("repeats card id"));
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_duplicate_comment_budget_key() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-duplicate-family")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;

        let cards_path = dir.join("cards.json");
        let mut cards: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&cards_path).map_err(|err| format!("read cards failed: {err}"))?,
        )
        .map_err(|err| format!("parse cards failed: {err}"))?;
        let mut second_card = cards["cards"][0].clone();
        second_card["id"] = serde_json::json!("card-2");
        second_card["site"]["line"] = serde_json::json!(8);
        cards["summary"]["cards"] = serde_json::json!(2);
        cards["summary"]["open_actionable_gaps"] = serde_json::json!(2);
        cards["cards"]
            .as_array_mut()
            .ok_or_else(|| "cards fixture must have cards array".to_string())?
            .push(second_card);
        fs::write(&cards_path, cards.to_string())
            .map_err(|err| format!("write cards failed: {err}"))?;

        let pr_summary_path = dir.join("pr-summary.md");
        let pr_summary = fs::read_to_string(&pr_summary_path)
            .map_err(|err| format!("read pr summary failed: {err}"))?
            .replace("- Review cards: 1", "- Review cards: 2")
            .replace("- Open actionable gaps: 1", "- Open actionable gaps: 2");
        fs::write(&pr_summary_path, pr_summary)
            .map_err(|err| format!("write pr summary failed: {err}"))?;

        let sarif_path = dir.join("cards.sarif");
        let mut sarif: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&sarif_path).map_err(|err| format!("read sarif failed: {err}"))?,
        )
        .map_err(|err| format!("parse sarif failed: {err}"))?;
        let mut second_result = sarif["runs"][0]["results"][0].clone();
        second_result["properties"]["cardId"] = serde_json::json!("card-2");
        second_result["locations"][0]["physicalLocation"]["region"]["startLine"] =
            serde_json::json!(8);
        sarif["runs"][0]["results"]
            .as_array_mut()
            .ok_or_else(|| "sarif fixture must have results array".to_string())?
            .push(second_result);
        fs::write(&sarif_path, sarif.to_string())
            .map_err(|err| format!("write sarif failed: {err}"))?;

        let comment_path = dir.join("comment-plan.json");
        let mut comment_plan: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&comment_path)
                .map_err(|err| format!("read comment plan failed: {err}"))?,
        )
        .map_err(|err| format!("parse comment plan failed: {err}"))?;
        let mut second_comment = comment_plan["comments"][0].clone();
        second_comment["card_id"] = serde_json::json!("card-2");
        second_comment["line"] = serde_json::json!(8);
        comment_plan["comments"]
            .as_array_mut()
            .ok_or_else(|| "comment plan fixture must have comments array".to_string())?
            .push(second_comment);
        fs::write(&comment_path, comment_plan.to_string())
            .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("repeats operation family and obligation budget key")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_duplicate_comment_locations() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-duplicate-location")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_two_card_artifacts(&dir)?;
        fs::write(
            dir.join("comment-plan.json"),
            r#"{"schema_version":"0.1","mode":"plan_only","policy":"advisory","comments":[{"card_id":"card-1","path":"src/lib.rs","line":7,"changed_line":true,"class":"guard_missing","priority":"high","confidence":"medium","operation":"unsafe { ptr.cast::<Header>().read() }","operation_family":"raw_pointer_read","witness_routes":[{"kind":"miri","reason":"route","command":"cargo +nightly miri test card","required":false}],"next_action":"Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.","verify_commands":["cargo +nightly miri test card"],"selection_reason":"actionable high-priority review card","selection_reason_code":"top_actionable_card","actionability":"specific_guard_missing","relevance":"medium","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result","body":"Next action: Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.\n\nPlan boundary: artifact-only inline comment candidate; unsafe-review did not post this comment, run witnesses, or make a policy decision."},{"card_id":"card-2","path":"src/lib.rs","line":7,"changed_line":true,"class":"contract_missing","priority":"high","confidence":"high","operation":"unsafe fn read_header(ptr: *const u8)","operation_family":"unknown","witness_routes":[{"kind":"human-deep-review","reason":"route","command":null,"required":false}],"next_action":"Add a precise public `# Safety` section that names the required caller obligations.","verify_commands":[],"selection_reason":"actionable high-confidence review card","selection_reason_code":"top_actionable_card","actionability":"specific_contract_missing","relevance":"high","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result","body":"Next action: Add a precise public `# Safety` section that names the required caller obligations.\n\nPlan boundary: artifact-only inline comment candidate; unsafe-review did not post this comment, run witnesses, or make a policy decision."}],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
        )
        .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("repeats inline location")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_overlong_comment_body() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-overlong-body")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        let next_action = "Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.";
        let filler = std::iter::repeat_n("word", 230)
            .collect::<Vec<_>>()
            .join(" ");
        let body = format!(
            "Next action: {next_action}\n\nPlan boundary: artifact-only inline comment candidate; unsafe-review did not post this comment, run witnesses, or make a policy decision.\n\n{filler}"
        );
        let comment_plan = serde_json::json!({
            "schema_version": "0.1",
            "mode": "plan_only",
            "policy": "advisory",
            "summary": {
                "selected_count": 1,
                "not_selected_count": 0,
                "budget": 3,
                "reason": "bounded reviewer noise"
            },
            "comments": [{
                "card_id": "card-1",
                "path": "src/lib.rs",
                "line": 7,
                "changed_line": true,
                "class": "guard_missing",
                "priority": "high",
                "confidence": "medium",
                "operation": "unsafe { ptr.cast::<Header>().read() }",
                "operation_family": "raw_pointer_read",
                "witness_routes": [{
                    "kind": "miri",
                    "reason": "route",
                    "command": "cargo +nightly miri test card",
                    "required": false
                }],
                "next_action": next_action,
                "verify_commands": ["cargo +nightly miri test card"],
                "selection_reason": "actionable high-priority review card",
                "selection_reason_code": "top_actionable_card",
                "actionability": "specific_guard_missing",
                "relevance": "medium",
                "trust_boundary": "static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result",
                "body": body
            }],
            "trust_boundary": "static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"
        });
        fs::write(dir.join("comment-plan.json"), comment_plan.to_string())
            .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains("at most 220"));
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_duplicate_repair_queue_bucket_card_ids()
    -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-repair-queue-duplicate-bucket-card")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;

        let path = dir.join("repair-queue.json");
        let mut repair_queue: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read repair queue failed: {err}"))?,
        )
        .map_err(|err| format!("parse repair queue failed: {err}"))?;
        let duplicate = repair_queue["buckets"]["repairable_by_guard"][0].clone();
        repair_queue["buckets"]["repairable_by_guard"]
            .as_array_mut()
            .ok_or_else(|| "repair queue fixture must have repairable_by_guard".to_string())?
            .push(duplicate);
        repair_queue["summary"]["repairable_by_guard"] = serde_json::json!(2);
        fs::write(&path, repair_queue.to_string())
            .map_err(|err| format!("write repair queue failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result.err().unwrap_or_default().contains(
                "repair-queue.json bucket `repairable_by_guard` repeats card id `card-1`"
            )
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_unknown_repair_queue_bucket() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-repair-queue-unknown-bucket")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;

        let path = dir.join("repair-queue.json");
        let mut repair_queue: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read repair queue failed: {err}"))?,
        )
        .map_err(|err| format!("parse repair queue failed: {err}"))?;
        repair_queue["buckets"]["repairable_by_magic"] = serde_json::json!([]);
        fs::write(&path, repair_queue.to_string())
            .map_err(|err| format!("write repair queue failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("repair-queue.json buckets contain unknown bucket `repairable_by_magic`")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_repair_queue_schema_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-repair-queue-schema-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;

        let path = dir.join("repair-queue.json");
        let mut repair_queue: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read repair queue failed: {err}"))?,
        )
        .map_err(|err| format!("parse repair queue failed: {err}"))?;
        repair_queue["schema_version"] = serde_json::json!("2.0");
        fs::write(&path, repair_queue.to_string())
            .map_err(|err| format!("write repair queue failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("repair-queue.json key `schema_version` is `2.0`, expected `0.1`")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_repair_queue_bucket_reason_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-repair-queue-bucket-reason-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;

        let path = dir.join("repair-queue.json");
        let mut repair_queue: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read repair queue failed: {err}"))?,
        )
        .map_err(|err| format!("parse repair queue failed: {err}"))?;
        repair_queue["buckets"]["requires_witness_receipt"][0]["bucket_reason"] =
            serde_json::json!("guard_evidence_missing");
        fs::write(&path, repair_queue.to_string())
            .map_err(|err| format!("write repair queue failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let err = result.err().unwrap_or_default();
        assert!(
            err.contains(
                "repair-queue.json bucket_reason must be `witness_receipt_missing`; got `guard_evidence_missing`"
            ),
            "{err}"
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_agent_ready_human_review_queue_entries()
    -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-repair-queue-human-ready")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_two_card_artifacts(&dir)?;

        let path = dir.join("repair-queue.json");
        let mut repair_queue: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read repair queue failed: {err}"))?,
        )
        .map_err(|err| format!("parse repair queue failed: {err}"))?;
        repair_queue["buckets"]["requires_human_review"][0]["agent_readiness"]["ready"] =
            serde_json::json!(true);
        repair_queue["buckets"]["requires_human_review"][0]["agent_readiness"]["state"] =
            serde_json::json!("ready_for_agent");
        fs::write(&path, repair_queue.to_string())
            .map_err(|err| format!("write repair queue failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let err = result.err().unwrap_or_default();
        assert!(
            err.contains("repair-queue.json requires_human_review entries must not be agent-ready"),
            "{err}"
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_empty_repair_queue_readiness_reasons() -> Result<(), String>
    {
        let dir = unique_temp_dir("unsafe-review-artifacts-repair-queue-empty-readiness-reasons")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;

        let path = dir.join("repair-queue.json");
        let mut repair_queue: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read repair queue failed: {err}"))?,
        )
        .map_err(|err| format!("parse repair queue failed: {err}"))?;
        repair_queue["buckets"]["repairable_by_guard"][0]["agent_readiness"]["reasons"] =
            serde_json::json!([]);
        fs::write(&path, repair_queue.to_string())
            .map_err(|err| format!("write repair queue failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("repair-queue.json agent_readiness.reasons must not be empty")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_unknown_repair_queue_readiness_state() -> Result<(), String>
    {
        let dir = unique_temp_dir("unsafe-review-artifacts-repair-queue-unknown-readiness-state")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;

        let path = dir.join("repair-queue.json");
        let mut repair_queue: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read repair queue failed: {err}"))?,
        )
        .map_err(|err| format!("parse repair queue failed: {err}"))?;
        repair_queue["buckets"]["repairable_by_guard"][0]["agent_readiness"]["state"] =
            serde_json::json!("maybe_ready");
        fs::write(&path, repair_queue.to_string())
            .map_err(|err| format!("write repair queue failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("repair-queue.json agent_readiness.state must be `ready_for_agent`, `requires_human_review`, `requires_witness_receipt`, or `unsupported`; got `maybe_ready`")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_repair_queue_readiness_state_mismatch()
    -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-repair-queue-readiness-state-mismatch")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;

        let path = dir.join("repair-queue.json");
        let mut repair_queue: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read repair queue failed: {err}"))?,
        )
        .map_err(|err| format!("parse repair queue failed: {err}"))?;
        repair_queue["buckets"]["repairable_by_guard"][0]["agent_readiness"]["state"] =
            serde_json::json!("requires_human_review");
        fs::write(&path, repair_queue.to_string())
            .map_err(|err| format!("write repair queue failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains(
            "repair-queue.json agent_readiness.state must be `ready_for_agent` when ready is true"
        ));
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_false_ready_for_agent_state() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-repair-queue-false-ready-for-agent")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;

        let path = dir.join("repair-queue.json");
        let mut repair_queue: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read repair queue failed: {err}"))?,
        )
        .map_err(|err| format!("parse repair queue failed: {err}"))?;
        repair_queue["buckets"]["repairable_by_guard"][0]["agent_readiness"]["ready"] =
            serde_json::json!(false);
        fs::write(&path, repair_queue.to_string())
            .map_err(|err| format!("write repair queue failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(result.err().unwrap_or_default().contains(
            "repair-queue.json agent_readiness.state `ready_for_agent` requires ready = true"
        ));
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_repair_queue_cross_bucket_readiness_state_drift()
    -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-repair-queue-readiness-state-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;

        let path = dir.join("repair-queue.json");
        let mut repair_queue: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read repair queue failed: {err}"))?,
        )
        .map_err(|err| format!("parse repair queue failed: {err}"))?;
        let readiness =
            &mut repair_queue["buckets"]["requires_witness_receipt"][0]["agent_readiness"];
        readiness["ready"] = serde_json::json!(false);
        readiness["state"] = serde_json::json!("requires_human_review");
        fs::write(&path, repair_queue.to_string())
            .map_err(|err| format!("write repair queue failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let err = result.err().unwrap_or_default();
        assert!(
            err.contains(
                "repair-queue.json card `card-1` has inconsistent agent_readiness.state across buckets"
            ),
            "{err}"
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_repair_queue_cross_bucket_readiness_reason_drift()
    -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-repair-queue-readiness-reason-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;

        let path = dir.join("repair-queue.json");
        let mut repair_queue: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read repair queue failed: {err}"))?,
        )
        .map_err(|err| format!("parse repair queue failed: {err}"))?;
        repair_queue["buckets"]["requires_witness_receipt"][0]["agent_readiness"]["reasons"] =
            serde_json::json!(["different readiness reason"]);
        fs::write(&path, repair_queue.to_string())
            .map_err(|err| format!("write repair queue failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let err = result.err().unwrap_or_default();
        assert!(
            err.contains(
                "repair-queue.json card `card-1` has inconsistent agent_readiness.reasons across buckets"
            ),
            "{err}"
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_repair_queue_missing_evidence_drift() -> Result<(), String>
    {
        let dir = unique_temp_dir("unsafe-review-artifacts-repair-queue-missing-evidence-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;

        let path = dir.join("repair-queue.json");
        let mut repair_queue: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read repair queue failed: {err}"))?,
        )
        .map_err(|err| format!("parse repair queue failed: {err}"))?;
        repair_queue["buckets"]["repairable_by_guard"][0]["missing_evidence"] =
            serde_json::json!(["unrelated missing evidence"]);
        fs::write(&path, repair_queue.to_string())
            .map_err(|err| format!("write repair queue failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let err = result.err().unwrap_or_default();
        assert!(
            err.contains("repair-queue.json entry missing_evidence must project cards.json value"),
            "{err}"
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_repair_queue_context_command_drift() -> Result<(), String>
    {
        let dir = unique_temp_dir("unsafe-review-artifacts-repair-queue-context-command-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;

        let path = dir.join("repair-queue.json");
        let mut repair_queue: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read repair queue failed: {err}"))?,
        )
        .map_err(|err| format!("parse repair queue failed: {err}"))?;
        repair_queue["buckets"]["repairable_by_guard"][0]["context_command"] =
            serde_json::json!("unsafe-review explain card-1");
        fs::write(&path, repair_queue.to_string())
            .map_err(|err| format!("write repair queue failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let err = result.err().unwrap_or_default();
        assert!(
            err.contains(
                "repair-queue.json context_command must be `unsafe-review context card-1 --json`"
            ),
            "{err}"
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_missing_repair_queue_do_not_do_boundary()
    -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-repair-queue-missing-boundary")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;

        let path = dir.join("repair-queue.json");
        let mut repair_queue: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read repair queue failed: {err}"))?,
        )
        .map_err(|err| format!("parse repair queue failed: {err}"))?;
        let rules = repair_queue["buckets"]["repairable_by_guard"][0]["do_not_do"]
            .as_array_mut()
            .ok_or_else(|| "repair queue fixture must include do_not_do".to_string())?;
        rules.retain(|rule| {
            !rule
                .as_str()
                .is_some_and(|text| text.contains("automatic safety repair"))
        });
        fs::write(&path, repair_queue.to_string())
            .map_err(|err| format!("write repair queue failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let err = result.err().unwrap_or_default();
        assert!(
            err.contains(
                "repair-queue.json entry do_not_do must include boundary `automatic safety repair`"
            ),
            "{err}"
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_repair_queue_top_level_boundary_drift()
    -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-repair-queue-top-boundary-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;

        let path = dir.join("repair-queue.json");
        let mut repair_queue: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read repair queue failed: {err}"))?,
        )
        .map_err(|err| format!("parse repair queue failed: {err}"))?;
        repair_queue["trust_boundary"] = serde_json::json!(
            "static unsafe contract review, not a proof of memory safety, not UB-free status, not a Miri result, and not an automatic repair queue"
        );
        fs::write(&path, repair_queue.to_string())
            .map_err(|err| format!("write repair queue failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let err = result.err().unwrap_or_default();
        assert!(
            err.contains("repair-queue.json trust_boundary must include `does not run agents`"),
            "{err}"
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_repair_queue_entry_boundary_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-repair-queue-entry-boundary-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;

        let path = dir.join("repair-queue.json");
        let mut repair_queue: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read repair queue failed: {err}"))?,
        )
        .map_err(|err| format!("parse repair queue failed: {err}"))?;
        repair_queue["buckets"]["repairable_by_guard"][0]["trust_boundary"] = serde_json::json!(
            "static unsafe contract review, not a proof of memory safety, not UB-free status, not a Miri result, and not an automatic repair queue"
        );
        fs::write(&path, repair_queue.to_string())
            .map_err(|err| format!("write repair queue failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let err = result.err().unwrap_or_default();
        assert!(
            err.contains(
                "repair-queue.json entry trust_boundary must include `does not run agents`"
            ),
            "{err}"
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_cards_schema_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-cards-schema-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;

        let path = dir.join("cards.json");
        let mut cards: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read cards.json failed: {err}"))?,
        )
        .map_err(|err| format!("parse cards.json failed: {err}"))?;
        cards["schema_version"] = serde_json::json!("2.0");
        fs::write(&path, cards.to_string())
            .map_err(|err| format!("write cards.json failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("cards.json key `schema_version` is `2.0`, expected `0.1`")
        );
        Ok(())
    }

    #[test]
    fn advisory_artifact_checker_rejects_comment_plan_schema_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-plan-schema-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;

        let path = dir.join("comment-plan.json");
        let mut comment_plan: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read comment plan failed: {err}"))?,
        )
        .map_err(|err| format!("parse comment plan failed: {err}"))?;
        comment_plan["schema_version"] = serde_json::json!("2.0");
        fs::write(&path, comment_plan.to_string())
            .map_err(|err| format!("write comment plan failed: {err}"))?;

        let result = check_advisory_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("comment-plan.json key `schema_version` is `2.0`, expected `0.1`")
        );
        Ok(())
    }

    #[test]
    fn first_pr_artifact_checker_rejects_lsp_schema_drift() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-lsp-schema-drift")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;

        let path = dir.join("lsp.json");
        let mut lsp: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|err| format!("read lsp.json failed: {err}"))?,
        )
        .map_err(|err| format!("parse lsp.json failed: {err}"))?;
        lsp["schema_version"] = serde_json::json!("2.0");
        fs::write(&path, lsp.to_string()).map_err(|err| format!("write lsp.json failed: {err}"))?;

        let result = check_first_pr_artifacts(&dir);

        fs::remove_dir_all(&dir).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("lsp.json key `schema_version` is `2.0`, expected `0.1`")
        );
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

    fn repair_queue_do_not_do_fragment() -> &'static str {
        r#""do_not_do":["do not widen unsafe code without reducing the missing evidence","do not suppress this card instead of adding, exposing, or explicitly waiving evidence","do not add a broad suppression","do not replace executable guard or discharge evidence with comments or docs","do not claim Miri proof unless the witness command is run and attached","do not claim automatic safety repair from this packet","do not claim unsafe-review ran an agent, ran witnesses, applied source edits, or posted comments","do not change unrelated unsafe code or public API behavior","do not treat a test mention as proof that the unsafe site executed"],"#
    }

    fn repair_queue_trust_boundary() -> &'static str {
        "static unsafe contract review, not a proof of memory safety, not UB-free status, not a Miri result, and not an automatic repair queue. It does not run agents, does not run witnesses, does not edit source, does not post comments, does not suppress cards, and does not resolve cards"
    }

    fn write_review_kit_artifact(
        dir: &Path,
        card_count: usize,
        open_actionable_gaps: usize,
        top_card_id: Option<&str>,
    ) -> Result<(), String> {
        let top_card_handoff = top_card_id
            .map(|card_id| {
                serde_json::json!({
                    "card_id": card_id,
                    "explain": format!("unsafe-review explain {card_id}"),
                    "context_json": format!("unsafe-review context {card_id} --json"),
                })
            })
            .unwrap_or(serde_json::Value::Null);
        let review_card_queue = top_card_id
            .map(|card_id| {
                serde_json::json!({
                    "card_id": card_id,
                    "source": "review_card",
                    "class": "guard_missing",
                    "priority": "high",
                    "confidence": "medium",
                    "path": "src/lib.rs",
                    "line": 7,
                    "location_text": "src/lib.rs:7",
                    "operation_family": "raw_pointer_read",
                    "operation": "unsafe { ptr.cast::<Header>().read() }",
                    "missing_evidence": [],
                    "next_action": "Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.",
                    "verify_commands": ["cargo +nightly miri test card"],
                    "witness_routes": [{
                        "kind": "miri",
                        "reason": "route",
                        "command": "cargo +nightly miri test card",
                        "required": false
                    }],
                    "repair_queue_buckets": ["repairable_by_guard", "requires_witness_receipt"],
                    "repair_queue_bucket_reasons": ["guard_evidence_missing", "witness_receipt_missing"],
                    "agent_readiness": {
                        "ready": true,
                        "state": "ready_for_agent",
                        "reasons": ["specific operation family"]
                    },
                    "explain": format!("unsafe-review explain {card_id}"),
                    "context_json": format!("unsafe-review context {card_id} --json"),
                    "trust_boundary": "Static unsafe contract review only; copy-only ReviewCard queue entry projected from cards.json and repair-queue.json; it is not a proof of memory safety, not UB-free status, not a Miri result, and not site-execution proof. unsafe-review did not run agents, run witnesses, edit source, post comments, suppress cards, resolve cards, or enforce blocking policy."
                })
            })
            .into_iter()
            .collect::<Vec<_>>();
        let changed_files = usize::from(card_count > 0);
        let omitted_cards = card_count.saturating_sub(review_card_queue.len());
        let value = serde_json::json!({
            "schema_version": "0.1",
            "tool": "unsafe-review",
            "tool_version": "0.2.1-test",
            "mode": "review_kit_manifest",
            "source": "first_pr",
            "policy": "advisory",
            "scope": "diff",
            "base_ref": "origin/main",
            "head_commit": serde_json::Value::Null,
            "summary": {
                "changed_files": changed_files,
                "changed_rust_files": changed_files,
                "changed_non_rust_files": 0,
                "cards": card_count,
                "open_actionable_gaps": open_actionable_gaps,
            },
            "top_card_id": top_card_id,
            "handoff": {
                "reviewer_summary": "pr-summary.md",
                "receipt_audit_markdown": "unsafe-review receipt audit --root fixtures/raw_pointer_alignment --base origin/main --format markdown",
                "review_cards": {
                    "artifact": "cards.json",
                    "repair_queue_artifact": "repair-queue.json",
                    "review_cards": card_count,
                    "card_queue_limit": 5,
                    "card_queue": review_card_queue,
                    "omitted_cards": omitted_cards,
                    "trust_boundary": "Static unsafe contract review only; copy-only ReviewCard queue preview projected from cards.json and repair-queue.json. It does not run agents, run witnesses, edit source, post comments, suppress cards, resolve cards, or enforce blocking policy. It is not a proof of memory safety, not UB-free status, not a Miri result, not Miri-clean status, not site-execution proof, not repair success, and not policy readiness."
                },
                "manual_candidates": {
                    "artifact": "manual-candidates.json",
                    "manual_repair_queue_artifact": "manual-repair-queue.json",
                    "manual_candidates": 0,
                    "analyzer_discovered": 0,
                    "operation_families": {},
                    "evidence_kinds": {},
                    "reviewcard_artifact_applicability": manual_candidate_reviewcard_applicability_fixture(),
                    "first_candidate": serde_json::Value::Null,
                    "candidate_queue_limit": 5,
                    "candidate_queue": [],
                    "omitted_candidates": 0,
                    "trust_boundary": "manual/advisory candidates are not analyzer-discovered ReviewCards, not policy inputs, and not witness execution; receipts against manual candidates do not import ReviewCard witness evidence."
                },
                "top_card": top_card_handoff,
                "trust_boundary": "Copy-only review-kit handoff commands; unsafe-review did not run witnesses, run agents, post comments, edit source, or enforce blocking policy."
            },
            "artifacts": [
                {"path":"review-kit.json","kind":"review_kit_manifest","format":"json","schema_version":"0.1"},
                {"path":"cards.json","kind":"review_cards","format":"json","schema_version":"0.1"},
                {"path":"pr-summary.md","kind":"reviewer_summary","format":"markdown","schema_version":serde_json::Value::Null},
                {"path":"github-summary.md","kind":"github_summary","format":"markdown","schema_version":serde_json::Value::Null},
                {"path":"cards.sarif","kind":"sarif","format":"sarif","schema_version":"2.1.0"},
                {"path":"comment-plan.json","kind":"comment_plan","format":"json","schema_version":"0.1"},
                {"path":"witness-plan.md","kind":"witness_plan","format":"markdown","schema_version":serde_json::Value::Null},
                {"path":"receipt-audit.md","kind":"receipt_audit","format":"markdown","schema_version":serde_json::Value::Null},
                {"path":"policy-report.json","kind":"policy_report_json","format":"json","schema_version":"0.1"},
                {"path":"policy-report.md","kind":"policy_report_markdown","format":"markdown","schema_version":serde_json::Value::Null},
                {"path":"manual-candidates.json","kind":"manual_candidates","format":"json","schema_version":"manual-candidates/v1"},
                {"path":"manual-repair-queue.json","kind":"manual_repair_queue","format":"json","schema_version":"manual-repair-queue/v1"},
                {"path":"lsp.json","kind":"saved_lsp","format":"json","schema_version":"0.1"},
                {"path":"repair-queue.json","kind":"repair_queue","format":"json","schema_version":"0.1"}
            ],
            "trust_boundary": "Static unsafe contract review kit manifest only; this indexes first-pr artifacts and does not reclassify ReviewCards. It is not a proof of memory safety, not UB-free status, not a Miri result, not Miri-clean status, and not site-execution proof. unsafe-review did not run witnesses, post comments, edit source, run an agent, or enforce blocking policy.",
        });
        fs::write(dir.join("review-kit.json"), value.to_string())
            .map_err(|err| format!("write review kit failed: {err}"))
    }

    fn write_empty_manual_candidates_artifact(dir: &Path) -> Result<(), String> {
        let value = serde_json::json!({
            "schema_version": "manual-candidates/v1",
            "tool": "unsafe-review",
            "tool_version": "0.2.1-test",
            "mode": "manual_candidate_index",
            "source": "first_pr",
            "summary": {
                "manual_candidates": 0,
                "external_evidence_refs": 0,
                "operation_families": {},
                "evidence_kinds": {},
                "analyzer_discovered": 0
            },
            "candidates": [],
            "reviewcard_artifact_relationship": manual_candidate_reviewcard_relationship_fixture(),
            "reviewcard_artifact_applicability": manual_candidate_reviewcard_applicability_fixture(),
            "trust_boundary": "Manual/advisory static unsafe contract review candidate index only; candidates are not analyzer-discovered ReviewCards, not a proof of UB, not a proof of memory safety, not UB-free status, not a Miri result, not Miri-clean status, not site-execution proof, not repository safety, and not policy gating. unsafe-review did not run witnesses, post comments, edit source, run an agent, or enforce blocking policy."
        });
        fs::write(dir.join("manual-candidates.json"), value.to_string())
            .map_err(|err| format!("write manual candidates failed: {err}"))
    }

    fn write_empty_manual_repair_queue_artifact(dir: &Path) -> Result<(), String> {
        let value = serde_json::json!({
            "schema_version": "manual-repair-queue/v1",
            "tool": "unsafe-review",
            "tool_version": "0.2.1-test",
            "mode": "manual_candidate_repair_queue",
            "source": "manual_candidate",
            "policy": "advisory",
            "summary": {
                "manual_candidates": 0,
                "queued_candidates": 0,
                "analyzer_discovered": 0,
                "external_evidence_refs": 0,
                "operation_families": {},
                "evidence_kinds": {},
                "with_fix_options": 0,
                "with_test_targets": 0,
                "with_do_not_touch": 0
            },
            "queue": [],
            "trust_boundary": "Copy-only manual candidate repair queue; entries come from imported manual candidates, not analyzer-discovered ReviewCards. This is not an automatic repair queue, not proof of memory safety, not UB-free status, not a Miri result, not Miri-clean status, not site-execution proof, not policy gating, and not repair success. unsafe-review did not run agents, did not run witnesses, did not edit source, did not post comments, and did not enforce blocking policy."
        });
        fs::write(dir.join("manual-repair-queue.json"), value.to_string())
            .map_err(|err| format!("write manual repair queue failed: {err}"))
    }

    fn manual_candidate_reviewcard_relationship_fixture() -> serde_json::Value {
        serde_json::json!({
            "cards.json": "ReviewCard-only analyzer output; manual candidates are listed only in manual-candidates.json.",
            "cards.sarif": "ReviewCard-only analyzer output; manual candidates are not emitted as SARIF analyzer results.",
            "comment-plan.json": "ReviewCard-only comment planning; manual candidates are not selected for automatic comment plans.",
            "lsp.json": "ReviewCard-only saved editor projection; manual candidates are not emitted as analyzer diagnostics.",
            "repair-queue.json": "ReviewCard-only repair queue; manual candidates are not automatic repair tasks.",
            "receipt-audit.md": "Receipts may match manual candidate IDs as manual/advisory targets without importing them as ReviewCard witness evidence.",
            "policy-report.json": "ReviewCard-only policy simulation; manual candidates are not policy gating inputs.",
            "policy-report.md": "ReviewCard-only policy simulation; manual candidates are not policy gating inputs."
        })
    }

    fn manual_candidate_reviewcard_applicability_fixture() -> serde_json::Value {
        serde_json::json!({
            "cards.json": manual_candidate_reviewcard_applicability_entry_fixture(
                "reviewcard_only",
                "Manual candidates stay in manual-candidate ledger surfaces and are not emitted as analyzer ReviewCards."
            ),
            "cards.sarif": manual_candidate_reviewcard_applicability_entry_fixture(
                "reviewcard_only",
                "Manual candidates are not emitted as SARIF analyzer results."
            ),
            "comment-plan.json": manual_candidate_reviewcard_applicability_entry_fixture(
                "reviewcard_only",
                "Manual candidates are not selected for automatic comment plans."
            ),
            "lsp.json": manual_candidate_reviewcard_applicability_entry_fixture(
                "reviewcard_only",
                "Manual candidates are not emitted as saved editor diagnostics."
            ),
            "repair-queue.json": manual_candidate_reviewcard_applicability_entry_fixture(
                "reviewcard_only",
                "Manual candidates are not automatic repair tasks."
            ),
            "policy-report.json": manual_candidate_reviewcard_applicability_entry_fixture(
                "reviewcard_only",
                "Manual candidates are not policy gating inputs for the JSON policy report."
            ),
            "policy-report.md": manual_candidate_reviewcard_applicability_entry_fixture(
                "reviewcard_only",
                "Manual candidates are not policy gating inputs for the Markdown policy report."
            )
        })
    }

    fn manual_candidate_reviewcard_applicability_entry_fixture(
        decision: &str,
        reason: &str,
    ) -> serde_json::Value {
        serde_json::json!({
            "decision": decision,
            "applies_to_manual_candidates": false,
            "manual_candidate_markers_allowed": false,
            "reason": reason,
        })
    }

    fn manual_candidate_handoff_fixture() -> serde_json::Value {
        serde_json::json!({
            "target": {
                "file": "src/runtime/webcore/TextDecoder.rs",
                "line": 237,
                "location_text": "src/runtime/webcore/TextDecoder.rs:237"
            },
            "route": {
                "safe_caller": "TextDecoder.decode SharedArrayBuffer route",
                "unsafe_operation": "core::slice::from_raw_parts",
                "operation_family": "raw_pointer_read"
            },
            "invariant_at_risk": "&[u8] memory must not be concurrently mutated",
            "external_evidence": [{
                "kind": "runtime_witness",
                "path": "target/unsafe-scout/textdecoder-shared-race-route.out",
                "summary": "Bun TextDecoder route reaches shared backing bytes through safe JS",
                "command": "bun test test/js/webcore/textdecoder-sharedarraybuffer.test.ts",
                "limitation": "runtime route evidence only; not memory-safety proof and not analyzer-discovered"
            }],
            "fix_options": [
                "copy SharedArrayBuffer-backed bytes before constructing the slice"
            ],
            "test_targets": [
                "test/js/webcore/textdecoder-sharedarraybuffer.test.ts"
            ],
            "do_not_touch": [
                "Do not rewrite TextDecoder unrelated encodings"
            ],
            "suggested_next_steps": [
                "confirm the file:line and safe caller route before editing",
                "preserve or add concrete contract, guard, test, or witness evidence for the invariant",
                "attach receipts only when the external run targets this manual candidate ID",
                "evaluate the candidate-specific fix options before editing",
                "run or preserve the candidate-specific test targets listed in this handoff",
                "respect the candidate-specific do-not-touch notes before editing"
            ],
            "non_goals": [
                "do not treat this as analyzer-discovered",
                "do not claim proof, UB-free status, Miri-clean status, or site execution",
                "do not broaden the task to unrelated unsafe sites",
                "Do not rewrite TextDecoder unrelated encodings"
            ],
            "stop_condition": "stop before source edits if the route no longer matches this manual candidate, or if the repair would broaden into unrelated unsafe sites"
        })
    }

    fn manual_candidate_fixture() -> serde_json::Value {
        serde_json::json!({
            "schema_version": "manual-candidate/v1",
            "id": "R4R2-S001",
            "source": "manual",
            "manual_candidate": true,
            "analyzer_discovered": false,
            "title": "TextDecoder SharedArrayBuffer decode creates &[u8] over shared bytes",
            "location": {
                "file": "src/runtime/webcore/TextDecoder.rs",
                "line": 237
            },
            "operation_family": "raw_pointer_read",
            "unsafe_operation": "core::slice::from_raw_parts",
            "invariant": "&[u8] memory must not be concurrently mutated",
            "safe_caller": "TextDecoder.decode SharedArrayBuffer route",
            "fix_options": [
                "copy SharedArrayBuffer-backed bytes before constructing the slice"
            ],
            "test_targets": [
                "test/js/webcore/textdecoder-sharedarraybuffer.test.ts"
            ],
            "do_not_touch": [
                "Do not rewrite TextDecoder unrelated encodings"
            ],
            "evidence": [{
                "kind": "runtime_witness",
                "path": "target/unsafe-scout/textdecoder-shared-race-route.out",
                "summary": "Bun TextDecoder route reaches shared backing bytes through safe JS",
                "command": "bun test test/js/webcore/textdecoder-sharedarraybuffer.test.ts",
                "limitation": "runtime route evidence only; not memory-safety proof and not analyzer-discovered"
            }],
            "location_text": "src/runtime/webcore/TextDecoder.rs:237",
            "explain_command": "unsafe-review explain R4R2-S001",
            "context_command": "unsafe-review context R4R2-S001 --json",
            "witness_plan_command": "unsafe-review candidate witness-plan R4R2-S001",
            "implementer_handoff": manual_candidate_handoff_fixture(),
            "trust_boundary": "Manual/advisory candidate only; not analyzer-discovered ReviewCard, not site-execution proof, and not policy gating."
        })
    }

    fn write_one_manual_candidates_artifact(dir: &Path) -> Result<(), String> {
        let value = serde_json::json!({
            "schema_version": "manual-candidates/v1",
            "tool": "unsafe-review",
            "tool_version": "0.2.1-test",
            "mode": "manual_candidate_index",
            "source": "first_pr",
            "summary": {
                "manual_candidates": 1,
                "external_evidence_refs": 1,
                "operation_families": {
                    "raw_pointer_read": 1
                },
                "evidence_kinds": {
                    "runtime_witness": 1
                },
                "analyzer_discovered": 0
            },
            "candidates": [manual_candidate_fixture()],
            "reviewcard_artifact_relationship": manual_candidate_reviewcard_relationship_fixture(),
            "reviewcard_artifact_applicability": manual_candidate_reviewcard_applicability_fixture(),
            "trust_boundary": "Manual/advisory static unsafe contract review candidate index only; candidates are not analyzer-discovered ReviewCards, not a proof of UB, not a proof of memory safety, not UB-free status, not a Miri result, not Miri-clean status, not site-execution proof, not repository safety, and not policy gating. unsafe-review did not run witnesses, post comments, edit source, run an agent, or enforce blocking policy."
        });
        fs::write(dir.join("manual-candidates.json"), value.to_string())
            .map_err(|err| format!("write manual candidates failed: {err}"))
    }

    fn write_one_manual_repair_queue_artifact(dir: &Path) -> Result<(), String> {
        let handoff = manual_candidate_handoff_fixture();
        let value = serde_json::json!({
            "schema_version": "manual-repair-queue/v1",
            "tool": "unsafe-review",
            "tool_version": "0.2.1-test",
            "mode": "manual_candidate_repair_queue",
            "source": "manual_candidate",
            "policy": "advisory",
            "summary": {
                "manual_candidates": 1,
                "queued_candidates": 1,
                "analyzer_discovered": 0,
                "external_evidence_refs": 1,
                "operation_families": {
                    "raw_pointer_read": 1
                },
                "evidence_kinds": {
                    "runtime_witness": 1
                },
                "with_fix_options": 1,
                "with_test_targets": 1,
                "with_do_not_touch": 1
            },
            "queue": [{
                "id": "R4R2-S001",
                "source": "manual",
                "manual_candidate": true,
                "analyzer_discovered": false,
                "title": "TextDecoder SharedArrayBuffer decode creates &[u8] over shared bytes",
                "location_text": "src/runtime/webcore/TextDecoder.rs:237",
                "operation_family": "raw_pointer_read",
                "unsafe_operation": "core::slice::from_raw_parts",
                "safe_caller": "TextDecoder.decode SharedArrayBuffer route",
                "invariant_at_risk": "&[u8] memory must not be concurrently mutated",
                "external_evidence_refs": 1,
                "fix_options": [
                    "copy SharedArrayBuffer-backed bytes before constructing the slice"
                ],
                "test_targets": [
                    "test/js/webcore/textdecoder-sharedarraybuffer.test.ts"
                ],
                "do_not_touch": [
                    "Do not rewrite TextDecoder unrelated encodings"
                ],
                "implementer_handoff": handoff,
                "explain": "unsafe-review explain R4R2-S001",
                "context_json": "unsafe-review context R4R2-S001 --json",
                "witness_plan": "unsafe-review candidate witness-plan R4R2-S001",
                "bucket": "manual_candidate_handoff",
                "bucket_reason": "manual_candidate_copy_only",
                "agent_handoff": {
                    "state": "copy_ready",
                    "automatic": false,
                    "reasons": [
                        "manual candidate includes file:line, safe caller route, invariant, evidence, fix/test/non-goal guidance, and stop condition",
                        "candidate must stay manual/advisory and separate from ReviewCard repair-queue.json"
                    ]
                },
                "trust_boundary": "Copy-only manual candidate repair queue entry; not analyzer-discovered, not automatic repair, not witness execution, not source editing, not proof, and not policy gating."
            }],
            "trust_boundary": "Copy-only manual candidate repair queue; entries come from imported manual candidates, not analyzer-discovered ReviewCards. This is not an automatic repair queue, not proof of memory safety, not UB-free status, not a Miri result, not Miri-clean status, not site-execution proof, not policy gating, and not repair success. unsafe-review did not run agents, did not run witnesses, did not edit source, did not post comments, and did not enforce blocking policy."
        });
        fs::write(dir.join("manual-repair-queue.json"), value.to_string())
            .map_err(|err| format!("write manual repair queue failed: {err}"))
    }

    fn write_one_manual_candidate_first_pr_artifacts(dir: &Path) -> Result<(), String> {
        write_valid_first_pr_artifacts(dir)?;
        write_one_manual_candidates_artifact(dir)?;
        write_one_manual_repair_queue_artifact(dir)?;
        insert_manual_candidate_front_panel_fixture(dir)?;
        insert_manual_candidate_witness_follow_up_fixture(dir)?;
        let path = dir.join("review-kit.json");
        let mut review_kit = parse_json_file(&path)?;
        let handoff = manual_candidate_handoff_fixture();
        review_kit["handoff"]["manual_candidates"] = serde_json::json!({
            "artifact": "manual-candidates.json",
            "manual_candidates": 1,
            "analyzer_discovered": 0,
            "operation_families": {
                "raw_pointer_read": 1
            },
            "evidence_kinds": {
                "runtime_witness": 1
            },
            "reviewcard_artifact_applicability": manual_candidate_reviewcard_applicability_fixture(),
            "first_candidate": {
                "id": "R4R2-S001",
                "source": "manual",
                "manual_candidate": true,
                "analyzer_discovered": false,
                "title": "TextDecoder SharedArrayBuffer decode creates &[u8] over shared bytes",
                "location_text": "src/runtime/webcore/TextDecoder.rs:237",
                "operation_family": "raw_pointer_read",
                "evidence_refs": 1,
                "implementer_handoff": handoff.clone(),
                "explain": "unsafe-review explain R4R2-S001",
                "context_json": "unsafe-review context R4R2-S001 --json",
                "witness_plan": "unsafe-review candidate witness-plan R4R2-S001"
            },
            "candidate_queue_limit": 5,
            "candidate_queue": [{
                "id": "R4R2-S001",
                "source": "manual",
                "manual_candidate": true,
                "analyzer_discovered": false,
                "title": "TextDecoder SharedArrayBuffer decode creates &[u8] over shared bytes",
                "location_text": "src/runtime/webcore/TextDecoder.rs:237",
                "operation_family": "raw_pointer_read",
                "evidence_refs": 1,
                "implementer_handoff": handoff,
                "explain": "unsafe-review explain R4R2-S001",
                "context_json": "unsafe-review context R4R2-S001 --json",
                "witness_plan": "unsafe-review candidate witness-plan R4R2-S001"
            }],
            "omitted_candidates": 0,
            "trust_boundary": "manual/advisory candidates are not analyzer-discovered ReviewCards, not policy inputs, and not witness execution; receipts against manual candidates do not import ReviewCard witness evidence."
        });
        fs::write(&path, review_kit.to_string())
            .map_err(|err| format!("write review kit failed: {err}"))
    }

    fn insert_manual_candidate_front_panel_fixture(dir: &Path) -> Result<(), String> {
        for (artifact, marker) in [
            ("pr-summary.md", "## Card table"),
            ("github-summary.md", "## Open next"),
        ] {
            let path = dir.join(artifact);
            let text = fs::read_to_string(&path)
                .map_err(|err| format!("read {artifact} failed: {err}"))?;
            if !text.contains(marker) {
                return Err(format!("{artifact} fixture is missing `{marker}`"));
            }
            fs::write(
                &path,
                text.replace(
                    marker,
                    &format!("{}{}", manual_candidate_front_panel_fixture(), marker),
                ),
            )
            .map_err(|err| format!("write {artifact} failed: {err}"))?;
        }
        Ok(())
    }

    fn insert_manual_candidate_witness_follow_up_fixture(dir: &Path) -> Result<(), String> {
        let artifact = "witness-plan.md";
        let marker = "## Trust boundary";
        let path = dir.join(artifact);
        let text =
            fs::read_to_string(&path).map_err(|err| format!("read {artifact} failed: {err}"))?;
        if !text.contains(marker) {
            return Err(format!("{artifact} fixture is missing `{marker}`"));
        }
        fs::write(
            &path,
            text.replace(
                marker,
                &format!("{}{}", manual_candidate_witness_follow_up_fixture(), marker),
            ),
        )
        .map_err(|err| format!("write {artifact} failed: {err}"))
    }

    fn manual_candidate_front_panel_fixture() -> &'static str {
        "## Manual candidates\n\n- Imported manual candidates: 1 (manual/advisory; not analyzer-discovered ReviewCards)\n- Operation families: `raw_pointer_read: 1`\n- Evidence kinds: `runtime_witness: 1`\n- First manual candidate: `R4R2-S001` at `src/runtime/webcore/TextDecoder.rs:237` (`raw_pointer_read`)\n- Safe caller route: TextDecoder.decode SharedArrayBuffer route\n- Invariant at risk: &[u8] memory must not be concurrently mutated\n- External evidence refs: 1\n- Guidance: 1 fix option(s), 1 test target(s), 1 do-not-touch note(s)\n- First fix option: copy SharedArrayBuffer-backed bytes before constructing the slice\n- First test target: `test/js/webcore/textdecoder-sharedarraybuffer.test.ts`\n- First do-not-touch note: Do not rewrite TextDecoder unrelated encodings\n- Explain: `unsafe-review explain R4R2-S001`\n- Agent context: `unsafe-review context R4R2-S001 --json`\n- Witness plan: `unsafe-review candidate witness-plan R4R2-S001`\n- Manual candidate queue preview: first 1 of 1 manual candidate(s)\n  - `R4R2-S001` at `src/runtime/webcore/TextDecoder.rs:237` (`raw_pointer_read`); evidence refs: 1; first test target: `test/js/webcore/textdecoder-sharedarraybuffer.test.ts`\n    - Agent context: `unsafe-review context R4R2-S001 --json`\n    - Witness plan: `unsafe-review candidate witness-plan R4R2-S001`\n- Manual candidate index: `manual-candidates.json`; candidates stay out of ReviewCard-only outputs.\n- Boundary: copy-only manual handoff; unsafe-review did not discover these candidates, did not run witnesses, did not edit source, or make them policy inputs.\n\n"
    }

    fn manual_candidate_witness_follow_up_fixture() -> &'static str {
        "## Manual candidate witness follow-up\n\n- Imported manual candidates: 1 (manual/advisory; not analyzer-discovered ReviewCards)\n- Operation families: `raw_pointer_read: 1`\n- Evidence kinds: `runtime_witness: 1`\n- First manual candidate: `R4R2-S001` at `src/runtime/webcore/TextDecoder.rs:237` (`raw_pointer_read`)\n- Safe caller route: TextDecoder.decode SharedArrayBuffer route\n- Invariant at risk: &[u8] memory must not be concurrently mutated\n- External evidence refs: 1\n- Guidance: 1 fix option(s), 1 test target(s), 1 do-not-touch note(s)\n- First fix option: copy SharedArrayBuffer-backed bytes before constructing the slice\n- First test target: `test/js/webcore/textdecoder-sharedarraybuffer.test.ts`\n- First do-not-touch note: Do not rewrite TextDecoder unrelated encodings\n- Full manual witness plan: `unsafe-review candidate witness-plan R4R2-S001`\n- Agent context: `unsafe-review context R4R2-S001 --json`\n- Manual candidate queue preview: first 1 of 1 manual candidate(s)\n  - `R4R2-S001` at `src/runtime/webcore/TextDecoder.rs:237` (`raw_pointer_read`); evidence refs: 1; first test target: `test/js/webcore/textdecoder-sharedarraybuffer.test.ts`\n    - Agent context: `unsafe-review context R4R2-S001 --json`\n    - Witness plan: `unsafe-review candidate witness-plan R4R2-S001`\n- Manual candidate index: `manual-candidates.json`; candidates stay out of ReviewCard-only witness route groups.\n- Receipt boundary: manual candidate receipts attach external evidence to the manual candidate ID only; they do not import ReviewCard witness evidence.\n- Boundary: copy-only manual follow-up; unsafe-review did not discover these candidates, did not run witnesses, did not edit source, or make them policy inputs.\n\n"
    }

    fn add_repair_queue_boundaries(text: &str) -> String {
        text.replace(
            "\"context_command\":\"unsafe-review context card-1 --json\",\"trust_boundary\"",
            &format!(
                "\"context_command\":\"unsafe-review context card-1 --json\",{}\"trust_boundary\"",
                repair_queue_do_not_do_fragment()
            ),
        )
        .replace(
            "\"context_command\":\"unsafe-review context card-2 --json\",\"trust_boundary\"",
            &format!(
                "\"context_command\":\"unsafe-review context card-2 --json\",{}\"trust_boundary\"",
                repair_queue_do_not_do_fragment()
            ),
        )
        .replace(
            "static unsafe contract review, not a proof of memory safety, not UB-free status, not a Miri result, and not an automatic repair queue",
            repair_queue_trust_boundary(),
        )
    }

    fn write_valid_artifacts(dir: &Path) -> Result<(), String> {
        fs::write(
            dir.join("cards.json"),
            r#"{"schema_version":"0.1","tool":"unsafe-review","policy":"advisory","scope":"diff","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result","summary":{"changed_files":1,"changed_rust_files":1,"changed_non_rust_files":0,"cards":1,"open_actionable_gaps":1},"cards":[{"id":"card-1","class":"guard_missing","priority":"high","confidence":"medium","hazards":["alignment"],"site":{"file":"src/lib.rs","line":7,"column":5},"operation":"unsafe { ptr.cast::<Header>().read() }","operation_family":"raw_pointer_read","next_action":"Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.","obligation_evidence":[{"key":"alignment","description":"pointer aligned","contract":{"present":true,"state":"present","summary":"safety contract"},"discharge":{"present":false,"state":"missing","summary":"No visible local guard"},"reach":{"present":true,"state":"present","summary":"related test mention"},"witness":{"present":false,"state":"missing","summary":"No imported witness receipt"}}],"contract":"safety contract","discharge":"No visible local guard","reach":"related test mention","witness":"No imported witness receipt","verify_commands":["cargo +nightly miri test card"],"witness_routes":[{"kind":"miri","reason":"route","command":"cargo +nightly miri test card","required":false}]}]}"#,
        )
        .map_err(|err| format!("write cards failed: {err}"))?;
        fs::write(
            dir.join("pr-summary.md"),
            "- Scope: `diff`\n- Review cards: 1\n- Open actionable gaps: 1\n- Policy mode: `advisory`\n\n## Top card\n\n- ID: `card-1`\n- Class: `guard_missing`\n- Location: src/lib.rs:7\n- Operation: `unsafe { ptr.cast::<Header>().read() }`\n- Operation family: `raw_pointer_read`\n- Missing evidence: No missing evidence recorded\n- Primary route: `miri` because route\n\n```bash\ncargo +nightly miri test card\n```\n- Next action: Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.\n- Receipt audit: `receipt-audit.md` checks saved receipt metadata only; no witness was run.\n- Explain: `unsafe-review explain card-1`\n- Agent context: `unsafe-review context card-1 --json`\n- Agent handoff: `ready_for_agent`; buckets: `repairable_by_guard`, `requires_witness_receipt`; reasons: specific operation family\n\n## Card table\n\n| ID | Class | Location | Operation family | Operation | Missing evidence | Route | Next action |\n|---|---|---|---|---|---|---|---|\n| `card-1` | `guard_missing` | src/lib.rs:7 | `raw_pointer_read` | `unsafe { ptr.cast::<Header>().read() }` | No missing evidence recorded | `miri` | Add or expose the local guard that discharges the `raw_pointer_read` safety obligation. |\n\n## Witness plan\n\n- `card-1`: `miri` because route\n\n```bash\ncargo +nightly miri test card\n```\n\n## Trust boundary\n\nThis artifact is static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result unless a witness receipt is attached.\n",
        )
        .map_err(|err| format!("write pr summary failed: {err}"))?;
        fs::write(
            dir.join("cards.sarif"),
            r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"rules":[{"id":"guard_missing"}]}},"results":[{"ruleId":"guard_missing","locations":[{"physicalLocation":{"artifactLocation":{"uri":"src/lib.rs"},"region":{"startLine":7,"startColumn":5}}}],"properties":{"cardId":"card-1","class":"guard_missing","priority":"high","confidence":"medium","operationFamily":"raw_pointer_read","operation":"unsafe { ptr.cast::<Header>().read() }","hazards":["alignment"],"missingEvidence":[],"nextAction":"Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.","witnessRoutes":["miri: route"],"witnessRouteDetails":[{"kind":"miri","reason":"route","command":"cargo +nightly miri test card","required":false}],"verifyCommands":["cargo +nightly miri test card"],"trustBoundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}}],"properties":{"scope":"diff","trustBoundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}}]}"#,
        )
        .map_err(|err| format!("write sarif failed: {err}"))?;
        fs::write(
            dir.join("comment-plan.json"),
            r#"{"schema_version":"0.1","mode":"plan_only","policy":"advisory","summary":{"selected_count":1,"not_selected_count":0,"budget":3,"reason":"bounded reviewer noise","reason_code":"bounded_reviewer_noise"},"comments":[{"card_id":"card-1","path":"src/lib.rs","line":7,"changed_line":true,"class":"guard_missing","priority":"high","confidence":"medium","operation":"unsafe { ptr.cast::<Header>().read() }","operation_family":"raw_pointer_read","witness_routes":[{"kind":"miri","reason":"route","command":"cargo +nightly miri test card","required":false}],"next_action":"Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.","verify_commands":["cargo +nightly miri test card"],"selection_reason":"actionable high-priority review card","selection_reason_code":"top_actionable_card","actionability":"specific_guard_missing","relevance":"medium","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result","body":"`unsafe-review` found `guard_missing` for `unsafe { ptr.cast::<Header>().read() }` (`raw_pointer_read`).\n\nMissing evidence: No missing evidence recorded\n\nNext action: Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.\n\nWitness route: `miri` because route.\n\nVerify command: `cargo +nightly miri test card`\n\nPlan boundary: artifact-only inline comment candidate; unsafe-review did not post this comment, run witnesses, or make a policy decision.\n\nTrust boundary: static unsafe contract review only; not memory-safety proof, not UB-free status, and not a Miri result unless a witness receipt is attached."}],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
        )
        .map_err(|err| format!("write comment plan failed: {err}"))?;
        fs::write(
            dir.join("repair-queue.json"),
            add_repair_queue_boundaries(r#"{"schema_version":"0.1","tool":"unsafe-review","mode":"aggregate_repair_queue","source":"review_card","policy":"advisory","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, not a Miri result, and not an automatic repair queue","summary":{"changed_files":1,"changed_rust_files":1,"changed_non_rust_files":0,"cards":1,"repairable_by_guard":1,"repairable_by_safety_docs":0,"repairable_by_test":0,"requires_witness_receipt":1,"requires_human_review":0,"do_not_auto_repair":0},"buckets":{"repairable_by_guard":[{"card_id":"card-1","class":"guard_missing","priority":"high","confidence":"medium","operation_family":"raw_pointer_read","operation":"unsafe { ptr.cast::<Header>().read() }","path":"src/lib.rs","line":7,"missing_evidence":[],"agent_readiness":{"ready":true,"state":"ready_for_agent","reasons":["specific operation family"]},"bucket_reason":"guard_evidence_missing","context_command":"unsafe-review context card-1 --json","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, not a Miri result, and not an automatic repair queue"}],"repairable_by_safety_docs":[],"repairable_by_test":[],"requires_witness_receipt":[{"card_id":"card-1","class":"guard_missing","priority":"high","confidence":"medium","operation_family":"raw_pointer_read","operation":"unsafe { ptr.cast::<Header>().read() }","path":"src/lib.rs","line":7,"missing_evidence":[],"agent_readiness":{"ready":true,"state":"ready_for_agent","reasons":["specific operation family"]},"bucket_reason":"witness_receipt_missing","context_command":"unsafe-review context card-1 --json","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, not a Miri result, and not an automatic repair queue"}],"requires_human_review":[],"do_not_auto_repair":[]}}"#),
        )
        .map_err(|err| format!("write repair queue failed: {err}"))?;
        fs::write(dir.join("receipt-audit.md"), receipt_audit_markdown())
            .map_err(|err| format!("write receipt audit failed: {err}"))?;
        write_policy_report_artifacts(
            dir,
            vec![policy_report_card_fixture(
                "card-1",
                "guard_missing",
                "raw_pointer_read",
                "unsafe { ptr.cast::<Header>().read() }",
                0,
                "Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.",
            )],
            1,
        )?;
        Ok(())
    }

    fn policy_report_card_fixture(
        card_id: &str,
        class_name: &str,
        operation_family: &str,
        operation: &str,
        missing_count: usize,
        next_action: &str,
    ) -> serde_json::Value {
        serde_json::json!({
            "card_id": card_id,
            "class": class_name,
            "operation_family": operation_family,
            "operation": operation,
            "policy_status": "new_gap",
            "policy_reason": "Exact ReviewCard identity was not found in the baseline ledger or active suppression ledger.",
            "missing_count": missing_count,
            "next_action": next_action
        })
    }

    fn write_policy_report_artifacts(
        dir: &Path,
        cards: Vec<serde_json::Value>,
        new_gaps: usize,
    ) -> Result<(), String> {
        let card_count = cards.len();
        let report = serde_json::json!({
            "schema_version": "0.1",
            "tool": "unsafe-review",
            "mode": "policy-report",
            "policy": "advisory",
            "generated_at": "2026-05-18",
            "trust_boundary": "Advisory no-new-debt policy report only; this is static unsafe contract review over existing ReviewCards and policy ledgers. It does not execute witnesses, is not a proof of memory safety, not UB-free status, not a Miri result, not Miri-clean status, not site-execution proof, and does not enforce blocking policy.",
            "summary": {
                "cards": card_count,
                "new_gaps": new_gaps,
                "baseline_known": 0,
                "suppressed": 0,
                "expired_suppressions": 0,
                "unmatched_baseline": 0,
                "invalid_ledger_entries": 0
            },
            "cards": cards,
            "unmatched_baseline": [],
            "invalid_ledger_entries": [],
            "limitations": [
                "Advisory report only; review ledgers and source context before making policy decisions.",
                "Manual candidates are not policy-report inputs and remain separate advisory artifacts.",
                "The report does not execute witnesses, post comments, edit source, or prove memory safety."
            ]
        });
        fs::write(dir.join("policy-report.json"), report.to_string())
            .map_err(|err| format!("write policy report json failed: {err}"))?;
        fs::write(
            dir.join("policy-report.md"),
            "# unsafe-review policy report\n\n## Reviewer front panel\n\n- Policy mode: `advisory`\n\n## Current cards\n\nReviewCard-only advisory policy simulation.\n\n## Limitations\n\n- Manual candidates are not policy-report inputs and remain separate advisory artifacts.\n\n## Trust boundary\n\nAdvisory no-new-debt policy report only; this is static unsafe contract review over existing ReviewCards and policy ledgers. It does not execute witnesses, is not a proof of memory safety, not UB-free status, not a Miri result, not Miri-clean status, not site-execution proof, and does not enforce blocking policy.\n",
        )
        .map_err(|err| format!("write policy report markdown failed: {err}"))?;
        Ok(())
    }

    fn github_summary_fixture(
        review_cards: usize,
        open_actionable_gaps: usize,
        top_card: &str,
    ) -> String {
        format!(
            "## unsafe-review advisory summary\n\n- Scope: `diff`\n- Review cards: {review_cards}\n- Open actionable gaps: {open_actionable_gaps}\n- Policy mode: `advisory`\n\n## Top card\n\n{top_card}\n\n## Open next\n\n- Review kit manifest: `review-kit.json`\n- Full reviewer cockpit: `pr-summary.md`\n- Machine-readable ReviewCards: `cards.json`\n- Witness routes: `witness-plan.md`\n- Receipt audit: `receipt-audit.md` checks saved receipt metadata only; no witness was run.\n- Policy report: `policy-report.md`; ReviewCard-only; manual candidates are not policy inputs.\n- Manual candidate index: `manual-candidates.json` lists imported advisory candidates separately from ReviewCards.\n- Agent repair queue: `repair-queue.json` is copy-only; no agent was run.\n- Comment budget: `comment-plan.json` is plan-only; no comments were posted.\n\n---\n\nFull advisory bundle (review-kit.json, cards.json, pr-summary.md, github-summary.md, cards.sarif, comment-plan.json, witness-plan.md, receipt-audit.md, policy-report.json, policy-report.md, manual-candidates.json, manual-repair-queue.json, lsp.json, repair-queue.json) is attached as the workflow artifact.\n\n> Trust boundary: static unsafe contract review only; not memory-safety proof, not UB-free status, not Miri-clean status, and not site-execution proof.\n"
        )
    }

    fn write_two_card_artifacts(dir: &Path) -> Result<(), String> {
        write_valid_artifacts(dir)?;
        fs::write(
            dir.join("cards.json"),
            r#"{"schema_version":"0.1","tool":"unsafe-review","policy":"advisory","scope":"diff","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result","summary":{"changed_files":1,"changed_rust_files":1,"changed_non_rust_files":0,"cards":2,"open_actionable_gaps":2},"cards":[{"id":"card-1","class":"guard_missing","priority":"high","confidence":"medium","hazards":["alignment"],"site":{"file":"src/lib.rs","line":7,"column":5},"operation":"unsafe { ptr.cast::<Header>().read() }","operation_family":"raw_pointer_read","next_action":"Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.","verify_commands":["cargo +nightly miri test card"],"witness_routes":[{"kind":"miri","reason":"route","command":"cargo +nightly miri test card","required":false}]},{"id":"card-2","class":"contract_missing","priority":"high","confidence":"high","hazards":["unknown"],"site":{"file":"src/lib.rs","line":7,"column":1},"operation":"unsafe fn read_header(ptr: *const u8)","operation_family":"unknown","next_action":"Add a precise public `# Safety` section that names the required caller obligations.","verify_commands":[],"witness_routes":[{"kind":"human-deep-review","reason":"route","command":null,"required":false}]}]}"#,
        )
        .map_err(|err| format!("write cards failed: {err}"))?;
        fs::write(
            dir.join("pr-summary.md"),
            "- Scope: `diff`\n- Review cards: 2\n- Open actionable gaps: 2\n- Policy mode: `advisory`\n\n- Receipt audit: `receipt-audit.md` checks saved receipt metadata only; no witness was run.\n\nThis artifact is static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result unless a witness receipt is attached.\n",
        )
        .map_err(|err| format!("write pr summary failed: {err}"))?;
        fs::write(
            dir.join("cards.sarif"),
            r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"rules":[{"id":"guard_missing"},{"id":"contract_missing"}]}},"results":[{"ruleId":"guard_missing","locations":[{"physicalLocation":{"artifactLocation":{"uri":"src/lib.rs"},"region":{"startLine":7,"startColumn":5}}}],"properties":{"cardId":"card-1","class":"guard_missing","priority":"high","confidence":"medium","operationFamily":"raw_pointer_read","operation":"unsafe { ptr.cast::<Header>().read() }","hazards":["alignment"],"missingEvidence":[],"nextAction":"Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.","witnessRoutes":["miri: route"],"witnessRouteDetails":[{"kind":"miri","reason":"route","command":"cargo +nightly miri test card","required":false}],"verifyCommands":["cargo +nightly miri test card"],"trustBoundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}},{"ruleId":"contract_missing","locations":[{"physicalLocation":{"artifactLocation":{"uri":"src/lib.rs"},"region":{"startLine":7,"startColumn":1}}}],"properties":{"cardId":"card-2","class":"contract_missing","priority":"high","confidence":"high","operationFamily":"unknown","operation":"unsafe fn read_header(ptr: *const u8)","hazards":["unknown"],"missingEvidence":[],"nextAction":"Add a precise public `# Safety` section that names the required caller obligations.","witnessRoutes":["human-deep-review: route"],"witnessRouteDetails":[{"kind":"human-deep-review","reason":"route","command":null,"required":false}],"verifyCommands":[],"trustBoundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}}],"properties":{"scope":"diff","trustBoundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}}]}"#,
        )
        .map_err(|err| format!("write sarif failed: {err}"))?;
        fs::write(
            dir.join("comment-plan.json"),
            r##"{"schema_version":"0.1","mode":"plan_only","policy":"advisory","summary":{"selected_count":1,"not_selected_count":1,"budget":3,"reason":"bounded reviewer noise","reason_code":"bounded_reviewer_noise"},"comments":[{"card_id":"card-1","path":"src/lib.rs","line":7,"changed_line":true,"class":"guard_missing","priority":"high","confidence":"medium","operation":"unsafe { ptr.cast::<Header>().read() }","operation_family":"raw_pointer_read","witness_routes":[{"kind":"miri","reason":"route","command":"cargo +nightly miri test card","required":false}],"next_action":"Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.","verify_commands":["cargo +nightly miri test card"],"selection_reason":"actionable high-priority review card","selection_reason_code":"top_actionable_card","actionability":"specific_guard_missing","relevance":"medium","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result","body":"`unsafe-review` found `guard_missing` for `unsafe { ptr.cast::<Header>().read() }` (`raw_pointer_read`).\n\nMissing evidence: No missing evidence recorded\n\nNext action: Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.\n\nWitness route: `miri` because route.\n\nVerify command: `cargo +nightly miri test card`\n\nPlan boundary: artifact-only inline comment candidate; unsafe-review did not post this comment, run witnesses, or make a policy decision.\n\nTrust boundary: static unsafe contract review only; not memory-safety proof, not UB-free status, and not a Miri result unless a witness receipt is attached."}],"not_selected":[{"card_id":"card-2","path":"src/lib.rs","line":7,"changed_line":true,"class":"contract_missing","priority":"high","confidence":"high","operation":"unsafe fn read_header(ptr: *const u8)","operation_family":"unknown","next_action":"Add a precise public `# Safety` section that names the required caller obligations.","actionability":"specific_contract_missing","relevance":"high","reason":"operation family unknown","reason_code":"human_deep_review_only"}],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"##,
        )
        .map_err(|err| format!("write comment plan failed: {err}"))?;
        fs::write(
            dir.join("repair-queue.json"),
            add_repair_queue_boundaries(r#"{"schema_version":"0.1","tool":"unsafe-review","mode":"aggregate_repair_queue","source":"review_card","policy":"advisory","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, not a Miri result, and not an automatic repair queue","summary":{"changed_files":1,"changed_rust_files":1,"changed_non_rust_files":0,"cards":2,"repairable_by_guard":1,"repairable_by_safety_docs":1,"repairable_by_test":0,"requires_witness_receipt":1,"requires_human_review":1,"do_not_auto_repair":1},"buckets":{"repairable_by_guard":[{"card_id":"card-1","class":"guard_missing","priority":"high","confidence":"medium","operation_family":"raw_pointer_read","operation":"unsafe { ptr.cast::<Header>().read() }","path":"src/lib.rs","line":7,"missing_evidence":[],"agent_readiness":{"ready":true,"state":"ready_for_agent","reasons":["specific operation family"]},"bucket_reason":"guard_evidence_missing","context_command":"unsafe-review context card-1 --json","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, not a Miri result, and not an automatic repair queue"}],"repairable_by_safety_docs":[{"card_id":"card-2","class":"contract_missing","priority":"high","confidence":"high","operation_family":"unknown","operation":"unsafe fn read_header(ptr: *const u8)","path":"src/lib.rs","line":7,"missing_evidence":[],"agent_readiness":{"ready":false,"state":"requires_human_review","reasons":["operation family `unknown` is not safe for automatic repair delegation"]},"bucket_reason":"safety_docs_evidence_missing","context_command":"unsafe-review context card-2 --json","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, not a Miri result, and not an automatic repair queue"}],"repairable_by_test":[],"requires_witness_receipt":[{"card_id":"card-1","class":"guard_missing","priority":"high","confidence":"medium","operation_family":"raw_pointer_read","operation":"unsafe { ptr.cast::<Header>().read() }","path":"src/lib.rs","line":7,"missing_evidence":[],"agent_readiness":{"ready":true,"state":"ready_for_agent","reasons":["specific operation family"]},"bucket_reason":"witness_receipt_missing","context_command":"unsafe-review context card-1 --json","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, not a Miri result, and not an automatic repair queue"}],"requires_human_review":[{"card_id":"card-2","class":"contract_missing","priority":"high","confidence":"high","operation_family":"unknown","operation":"unsafe fn read_header(ptr: *const u8)","path":"src/lib.rs","line":7,"missing_evidence":[],"agent_readiness":{"ready":false,"state":"requires_human_review","reasons":["operation family `unknown` is not safe for automatic repair delegation"]},"bucket_reason":"human_review_required","context_command":"unsafe-review context card-2 --json","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, not a Miri result, and not an automatic repair queue"}],"do_not_auto_repair":[{"card_id":"card-2","class":"contract_missing","priority":"high","confidence":"high","operation_family":"unknown","operation":"unsafe fn read_header(ptr: *const u8)","path":"src/lib.rs","line":7,"missing_evidence":[],"agent_readiness":{"ready":false,"state":"requires_human_review","reasons":["operation family `unknown` is not safe for automatic repair delegation"]},"bucket_reason":"not_ready_for_automatic_repair","context_command":"unsafe-review context card-2 --json","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, not a Miri result, and not an automatic repair queue"}]}}"#),
        )
        .map_err(|err| format!("write repair queue failed: {err}"))?;
        fs::write(dir.join("receipt-audit.md"), receipt_audit_markdown())
            .map_err(|err| format!("write receipt audit failed: {err}"))?;
        write_policy_report_artifacts(
            dir,
            vec![
                policy_report_card_fixture(
                    "card-1",
                    "guard_missing",
                    "raw_pointer_read",
                    "unsafe { ptr.cast::<Header>().read() }",
                    0,
                    "Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.",
                ),
                policy_report_card_fixture(
                    "card-2",
                    "contract_missing",
                    "unknown",
                    "unsafe fn read_header(ptr: *const u8)",
                    0,
                    "Add a precise public `# Safety` section that names the required caller obligations.",
                ),
            ],
            2,
        )?;
        Ok(())
    }

    fn write_valid_first_pr_artifacts(dir: &Path) -> Result<(), String> {
        write_valid_artifacts(dir)?;
        fs::write(
            dir.join("witness-plan.md"),
            "# unsafe-review witness plan\n\n- Review cards: 1\n- Open actionable gaps: 1\n- Policy mode: `advisory`\n\n## Route groups\n\n### Miri / cargo-careful\n\n- Limit: Concrete runtime evidence is path-specific. It can support the exercised route, but it does not prove arbitrary callers, repo safety, UB-free status, or site execution unless a matching receipt records the run.\n\n#### `card-1`\n\n- Class: `guard_missing`\n- Location: src/lib.rs:7\n- Operation: `unsafe { ptr.cast::<Header>().read() }`\n- Next action: Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.\n- Route: `miri`\n  - Reason: route\n  - What it can show: a focused run\n  - What it cannot prove: arbitrary callers\n  - Command:\n\n```bash\ncargo +nightly miri test card\n```\n  - Receipt hint: unsafe-review receipt import-miri card-1\n\n## Trust boundary\n\nThis artifact is static unsafe contract review. It routes reviewers to credible witnesses but does not run Miri, cargo-careful, sanitizers, Loom, Shuttle, Kani, or Crux. It is not a proof of memory safety, not UB-free status, and not a Miri result unless a witness receipt is attached.\n",
        )
        .map_err(|err| format!("write witness plan failed: {err}"))?;
        fs::write(
            dir.join("lsp.json"),
            valid_lsp_json(
                r#"[{"card_id":"card-1","path":"src/lib.rs","range":{"start":{"line":6,"character":0},"end":{"line":6,"character":1}},"title":"Copy unsafe-review packet for card-1","kind":"quickfix","command":"unsafe-review.copyAgentPacket","payload":{"kind":"unsafe-review.agent_packet","card_id":"card-1","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"},"arguments":["card-1"]},{"card_id":"card-1","path":"src/lib.rs","range":{"start":{"line":6,"character":0},"end":{"line":6,"character":1}},"title":"Explain unsafe-review witness route","kind":"quickfix","command":"unsafe-review.explainWitnessRoute","payload":{"kind":"unsafe-review.witness_route","card_id":"card-1","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"},"arguments":["card-1"]}]"#,
            )?,
        )
        .map_err(|err| format!("write lsp failed: {err}"))?;
        fs::write(
            dir.join("github-summary.md"),
            github_summary_fixture(
                1,
                1,
                "- ID: `card-1`\n- Class: `guard_missing`\n- Location: src/lib.rs:7\n- Operation: `unsafe { ptr.cast::<Header>().read() }`\n- Operation family: `raw_pointer_read`\n- Missing evidence: No missing evidence recorded\n- Primary route: `miri` because route\n\n```bash\ncargo +nightly miri test card\n```\n- Next action: Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.\n- Explain: `unsafe-review explain card-1`\n- Agent context: `unsafe-review context card-1 --json`",
            ),
        )
        .map_err(|err| format!("write github summary failed: {err}"))?;
        write_empty_manual_candidates_artifact(dir)?;
        write_empty_manual_repair_queue_artifact(dir)?;
        write_review_kit_artifact(dir, 1, 1, Some("card-1"))?;
        Ok(())
    }

    fn valid_lsp_json(code_actions: &str) -> Result<String, String> {
        let mut value: serde_json::Value = serde_json::from_str(&format!(
            r#"{{"schema_version":"0.1","tool":"unsafe-review","mode":"read_only_projection","policy":"advisory","scope":"diff","status":{{"state":"actionable","cards":1,"open_actionable_gaps":1,"high_priority_cards":1,"message":"1 unsafe-review card(s), 1 open actionable gap(s)","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}},"diagnostics":[{{"card_id":"card-1","path":"src/lib.rs","range":{{"start":{{"line":6,"character":0}},"end":{{"line":6,"character":1}}}},"code":"guard_missing","operation":"unsafe {{ ptr.cast::<Header>().read() }}","operation_family":"raw_pointer_read","next_action":"Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.","hazards":["alignment"],"required_safety_conditions":[{{"key":"alignment","description":"pointer aligned"}}],"evidence_summary":{{"contract":{{"present":true,"state":"present","summary":"safety contract"}},"discharge":{{"present":false,"state":"missing","summary":"No visible local guard"}},"reach":{{"state":"owner_reached","summary":"related test mention"}},"witness":{{"present":false,"state":"missing","summary":"No imported witness receipt"}},"reach_limitation":"static reach evidence is not proof that the unsafe site executed"}},"obligation_evidence":[{{"key":"alignment","description":"pointer aligned","contract":{{"present":true,"state":"present","summary":"safety contract"}},"discharge":{{"present":false,"state":"missing","summary":"No visible local guard"}},"reach":{{"present":true,"state":"present","summary":"related test mention"}},"witness":{{"present":false,"state":"missing","summary":"No imported witness receipt"}}}}],"witness_routes":[{{"kind":"miri","reason":"route","command":"cargo +nightly miri test card","required":false}}],"verify_commands":["cargo +nightly miri test card"],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}}],"hovers":[{{"card_id":"card-1","path":"src/lib.rs","position":{{"line":6,"character":0}},"contents":"Card: `card-1`; priority `high`; confidence `medium`\n\nWhy this card exists:\n- The changed code contains a `raw_pointer_read` unsafe operation that unsafe-review classifies as `guard_missing`.\n- Operation: `unsafe {{ ptr.cast::<Header>().read() }}`\n\nRelevant hazard families:\n- `alignment`\n\nRequired safety conditions:\n- pointer aligned\n\nEvidence found:\n- Contract [present]: safety contract\n- Guard/discharge [missing]: No visible local guard\n- Reach [owner_reached]: related test mention\n- Witness [missing]: No imported witness receipt\n\nEvidence missing:\n- none recorded\n\nWhat would resolve this:\n- Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.\n\nVerify commands:\n- `cargo +nightly miri test card`\n\nWhat would not resolve this:\n- A `SAFETY:` comment alone does not discharge missing guard evidence.\n- A related test mention is not proof that this unsafe site executed.\n- Do not claim witness proof unless a matching receipt exists.\n- Do not widen unsafe scope, suppress the card, or change unrelated unsafe code to silence this review item.\n\nWitness route: `miri` because route.\n\nTrust boundary: static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}}],"code_actions":{code_actions},"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}}"#
        ))
        .map_err(|err| format!("valid lsp json fixture failed to parse: {err}"))?;
        let hover_contents = value["hovers"][0]["contents"]
            .as_str()
            .ok_or_else(|| "valid lsp hover contents must be a string".to_string())?
            .replace(
                "\n\nWhy this card exists:",
                "\n\nLocation: src/lib.rs:7\n\nWhy this card exists:",
            )
            .replace(
                "\n\nTrust boundary:",
                "\n\nHandoff commands:\n- Explain: `unsafe-review explain card-1`\n- Agent context: `unsafe-review context card-1 --json`\n\nTrust boundary:",
            );
        value["hovers"][0]["contents"] = serde_json::json!(hover_contents);
        value["diagnostics"][0]["missing_evidence"] = serde_json::json!([]);
        Ok(value.to_string())
    }

    fn write_valid_zero_card_first_pr_artifacts(dir: &Path) -> Result<(), String> {
        fs::write(
            dir.join("cards.json"),
            r#"{"schema_version":"0.1","tool":"unsafe-review","policy":"advisory","scope":"diff","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result","summary":{"changed_files":0,"changed_rust_files":0,"changed_non_rust_files":0,"cards":0,"open_actionable_gaps":0},"cards":[]}"#,
        )
        .map_err(|err| format!("write cards failed: {err}"))?;
        fs::write(
            dir.join("pr-summary.md"),
            "- Scope: `diff`\n- Review cards: 0\n- Open actionable gaps: 0\n- Policy mode: `advisory`\n\nNo changed unsafe-review gaps were found.\nThis does not prove the repo safe, UB-free, Miri-clean, or that any unsafe site executed.\n- Receipt audit: `receipt-audit.md` checks saved receipt metadata only; no witness was run.\n\nThis artifact is static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result unless a witness receipt is attached.\n",
        )
        .map_err(|err| format!("write pr summary failed: {err}"))?;
        fs::write(
            dir.join("cards.sarif"),
            r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"rules":[]}},"results":[],"properties":{"scope":"diff","trustBoundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}}]}"#,
        )
        .map_err(|err| format!("write sarif failed: {err}"))?;
        fs::write(
            dir.join("comment-plan.json"),
            r#"{"schema_version":"0.1","mode":"plan_only","policy":"advisory","summary":{"selected_count":0,"not_selected_count":0,"budget":3,"reason":"bounded reviewer noise","reason_code":"bounded_reviewer_noise"},"comments":[],"no_changed_gaps":{"message":"No changed unsafe-review gaps were found.","limitation":"This does not prove the repo safe, UB-free, Miri-clean, or that any unsafe site executed."},"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
        )
        .map_err(|err| format!("write comment plan failed: {err}"))?;
        fs::write(
            dir.join("witness-plan.md"),
            "# unsafe-review witness plan\n\n- Review cards: 0\n- Open actionable gaps: 0\n- Policy mode: `advisory`\n\nNo changed unsafe-review gaps were found.\nThis does not prove the repo safe, UB-free, Miri-clean, or that any unsafe site executed.\n\nNo witness routes are recommended because no review cards were emitted.\n\n## Trust boundary\n\nThis artifact is static unsafe contract review. It routes reviewers to credible witnesses but does not run Miri, cargo-careful, sanitizers, Loom, Shuttle, Kani, or Crux. It is not a proof of memory safety, not UB-free status, and not a Miri result unless a witness receipt is attached.\n",
        )
        .map_err(|err| format!("write witness plan failed: {err}"))?;
        fs::write(
            dir.join("lsp.json"),
            r#"{"schema_version":"0.1","tool":"unsafe-review","mode":"read_only_projection","policy":"advisory","scope":"diff","status":{"state":"quiet","cards":0,"open_actionable_gaps":0,"high_priority_cards":0,"message":"No unsafe-review cards for this scope","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"},"diagnostics":[],"hovers":[],"code_actions":[],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
        )
        .map_err(|err| format!("write lsp failed: {err}"))?;
        fs::write(
            dir.join("github-summary.md"),
            github_summary_fixture(
                0,
                0,
                "No changed unsafe-review gaps were found.\nThis does not prove the repo safe, UB-free, Miri-clean, or that any unsafe site executed.",
            ),
        )
        .map_err(|err| format!("write github summary failed: {err}"))?;
        fs::write(
            dir.join("repair-queue.json"),
            add_repair_queue_boundaries(r#"{"schema_version":"0.1","tool":"unsafe-review","mode":"aggregate_repair_queue","source":"review_card","policy":"advisory","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, not a Miri result, and not an automatic repair queue","summary":{"changed_files":0,"changed_rust_files":0,"changed_non_rust_files":0,"cards":0,"repairable_by_guard":0,"repairable_by_safety_docs":0,"repairable_by_test":0,"requires_witness_receipt":0,"requires_human_review":0,"do_not_auto_repair":0},"buckets":{"repairable_by_guard":[],"repairable_by_safety_docs":[],"repairable_by_test":[],"requires_witness_receipt":[],"requires_human_review":[],"do_not_auto_repair":[]}}"#),
        )
        .map_err(|err| format!("write repair queue failed: {err}"))?;
        fs::write(dir.join("receipt-audit.md"), receipt_audit_markdown())
            .map_err(|err| format!("write receipt audit failed: {err}"))?;
        write_policy_report_artifacts(dir, Vec::new(), 0)?;
        write_empty_manual_candidates_artifact(dir)?;
        write_empty_manual_repair_queue_artifact(dir)?;
        write_review_kit_artifact(dir, 0, 0, None)?;
        Ok(())
    }

    fn receipt_audit_markdown() -> &'static str {
        "# unsafe-review receipt audit\n\nStatic audit of saved receipt metadata against current ReviewCards.\n\n## Summary\n\n| Receipts | Matched | Unmatched | Expired | Stale | Wrong identity | Wrong tool | Weaker than route | Command hash mismatch | Duplicate | Invalid |\n|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n| 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |\n\n## Reviewer front panel\n\n- Matched receipt metadata: 0\n- Receipts imported as current witness evidence: 0\n- Receipts without a current card match: 0 unmatched, 0 stale\n- Problem flags: none\n- Next action: keep matching receipt metadata attached to the review record.\n- Boundary: matched witness receipts improve witness evidence only; they do not erase missing contracts, guards, or reach evidence.\n- Manual boundary: manual candidate receipts attach external evidence to that manual candidate only and do not make it analyzer-discovered.\n\n## Trust boundary\n\nStatic witness receipt audit only; does not execute witnesses, does not prove site reach, and does not independently prove site reach.\n"
    }

    fn unique_temp_dir(prefix: &str) -> Result<PathBuf, String> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system clock before UNIX_EPOCH: {err}"))?
            .as_nanos();
        Ok(std::env::temp_dir().join(format!("{prefix}-{nanos}")))
    }
}
