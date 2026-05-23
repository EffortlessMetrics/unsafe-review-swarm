#![forbid(unsafe_code)]
use std::collections::{BTreeMap, BTreeSet};
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
mod command_args;
mod commands;
mod docs_automation_paths;
mod markdown;
mod source_sync;
mod workflow_allowlist;

use advisory_artifacts::{check_advisory_artifacts, check_first_pr_artifacts};

#[cfg(test)]
use command_args::{require_max_args, require_no_extra_args};

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
const PUBLIC_SURFACES_LEDGER: &str = "policy/public-surfaces.toml";
const CI_LANE_LEDGER: &str = "policy/ci-lane-whitelist.toml";
const PACKAGE_BOUNDARY_LEDGER: &str = "policy/package-boundary.toml";
const SOURCE_OF_TRUTH_INDEX: &str = ".unsafe-review-spec/index.toml";
const ACTIVE_GOAL_MANIFEST: &str = ".unsafe-review-spec/goals/active.toml";
const DOC_ARTIFACT_KINDS: &[&str] = &["proposal", "spec", "adr", "plan", "goal"];
const DOC_ARTIFACT_STATUSES: &[&str] = &["proposed", "accepted", "active", "done", "deferred"];
const DOCS_AUTOMATION_KINDS: &[&str] = &[
    "spec_status_dashboard",
    "operator_front_door",
    "docs_map",
    "published_surface",
    "handoff_receipt",
];
const DOCS_AUTOMATION_MODES: &[&str] = &["check", "generate"];
const PUBLIC_SURFACE_STATUSES: &[&str] = &["experimental", "accepted", "deferred"];
const PUBLIC_SURFACE_FRONT_DOORS: &[&str] = &[
    "README.md",
    "docs/FIRST_USE.md",
    "docs/CLI.md",
    "crates/unsafe-review/README.md",
    "crates/unsafe-review-cli/README.md",
    "crates/unsafe-review-core/README.md",
];
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

    match commands::XtaskCommand::parse(&args)? {
        commands::XtaskCommand::Help => {
            println!(
                "xtask commands: check-pr, check-docs, check-policy, check-support-tiers, check-fixtures, check-calibration, check-dogfood, check-fuzz, check-doc-artifacts, check-docs-automation, check-public-surfaces, check-goals, check-package-boundary, check-ci-lanes, check-advisory-artifacts <dir>, check-first-pr-artifacts <dir>, source-divergence, check-source-sync"
            );
            Ok(())
        }
        commands::XtaskCommand::CheckPr => {
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
        commands::XtaskCommand::CheckDocs => check_docs(),
        commands::XtaskCommand::CheckPolicy => check_policy(),
        commands::XtaskCommand::CheckDocArtifacts => check_doc_artifacts(),
        commands::XtaskCommand::CheckDocsAutomation => check_docs_automation(),
        commands::XtaskCommand::CheckPublicSurfaces => check_public_surfaces(),
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
    check_public_surfaces()?;
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
        "Rust Small on GitHub Hosted",
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

fn check_doc_artifacts() -> Result<(), String> {
    let ids = check_doc_artifacts_impl()?;
    println!("check-doc-artifacts: ok ({} artifacts)", ids.len());
    Ok(())
}

fn check_doc_artifacts_impl() -> Result<BTreeSet<String>, String> {
    let value = parse_toml_file(Path::new(DOC_ARTIFACT_LEDGER))?;
    require_toml_string(&value, "schema_version", DOC_ARTIFACT_LEDGER)?;
    let artifacts = toml_array(&value, "artifact", DOC_ARTIFACT_LEDGER)?;
    if artifacts.is_empty() {
        return Err(format!(
            "{DOC_ARTIFACT_LEDGER} must list at least one artifact"
        ));
    }

    let mut ids = BTreeSet::new();
    let mut linked_ids = Vec::new();
    for (idx, artifact) in artifacts.iter().enumerate() {
        let table = toml_table(artifact, DOC_ARTIFACT_LEDGER, "artifact", idx)?;
        let id = required_table_string(table, "id", DOC_ARTIFACT_LEDGER, "artifact", idx)?;
        let kind = required_table_string(table, "kind", DOC_ARTIFACT_LEDGER, "artifact", idx)?;
        let path = required_table_string(table, "path", DOC_ARTIFACT_LEDGER, "artifact", idx)?;
        let status = required_table_string(table, "status", DOC_ARTIFACT_LEDGER, "artifact", idx)?;
        required_table_string(table, "owner", DOC_ARTIFACT_LEDGER, "artifact", idx)?;

        require_known(kind, DOC_ARTIFACT_KINDS, DOC_ARTIFACT_LEDGER, "kind")?;
        require_known(status, DOC_ARTIFACT_STATUSES, DOC_ARTIFACT_LEDGER, "status")?;
        if !ids.insert(id.to_string()) {
            return Err(format!(
                "{DOC_ARTIFACT_LEDGER} contains duplicate id `{id}`"
            ));
        }
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

    Ok(ids)
}

fn check_docs_automation() -> Result<(), String> {
    let surfaces = check_docs_automation_impl()?;
    println!("check-docs-automation: ok ({surfaces} surfaces)");
    Ok(())
}

fn check_public_surfaces() -> Result<(), String> {
    let surfaces = check_public_surfaces_impl()?;
    println!("check-public-surfaces: ok ({surfaces} surfaces)");
    Ok(())
}

fn check_public_surfaces_impl() -> Result<usize, String> {
    let value = parse_toml_file(&workspace_path(PUBLIC_SURFACES_LEDGER))?;
    require_toml_string(&value, "schema_version", PUBLIC_SURFACES_LEDGER)?;
    require_known(
        required_toml_string(&value, "status", PUBLIC_SURFACES_LEDGER)?,
        PUBLIC_SURFACE_STATUSES,
        PUBLIC_SURFACES_LEDGER,
        "status",
    )?;
    let trust_boundary = required_toml_string(&value, "trust_boundary", PUBLIC_SURFACES_LEDGER)?;
    for required in ["advisory", "memory-safety proof", "UB-free", "Miri-clean"] {
        if !text_contains_ignore_ascii_case(trust_boundary, required) {
            return Err(format!(
                "{PUBLIC_SURFACES_LEDGER} trust_boundary must mention `{required}`"
            ));
        }
    }

    let forbidden_terms = value
        .get("forbidden_terms")
        .ok_or_else(|| format!("{PUBLIC_SURFACES_LEDGER} is missing `forbidden_terms` array"))?;
    let forbidden_terms =
        toml_str_array(forbidden_terms, PUBLIC_SURFACES_LEDGER, "forbidden_terms")?;
    if forbidden_terms.is_empty() {
        return Err(format!(
            "{PUBLIC_SURFACES_LEDGER} forbidden_terms must not be empty"
        ));
    }
    let mut seen = BTreeSet::new();
    for term in forbidden_terms {
        if term.trim().is_empty() {
            return Err(format!(
                "{PUBLIC_SURFACES_LEDGER} forbidden_terms entries must be non-empty"
            ));
        }
        if !seen.insert(term.to_ascii_lowercase()) {
            return Err(format!(
                "{PUBLIC_SURFACES_LEDGER} contains duplicate forbidden term `{term}`"
            ));
        }
    }

    check_public_badge_endpoints()?;
    for path in PUBLIC_SURFACE_FRONT_DOORS {
        check_public_surface_front_door(path)?;
    }

    Ok(PUBLIC_SURFACE_FRONT_DOORS.len() + PUBLIC_BADGE_ENDPOINTS.len())
}

fn check_public_surface_front_door(path: &str) -> Result<(), String> {
    require_file(path)?;
    check_markdown_local_links(path)?;
    let source = workspace_path(path);
    let text = read_to_string(&source)?;
    reject_positive_overclaims(Path::new(path), &text)?;
    if !public_surface_has_trust_boundary(&text) {
        return Err(format!(
            "{path} must include advisory trust-boundary wording such as not-proof, not-UB-free, no-default-witness, or no-default-blocking language"
        ));
    }
    Ok(())
}

fn public_surface_has_trust_boundary(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let has_negative = lower.contains("not")
        || lower.contains("does not")
        || lower.contains("no ")
        || lower.contains("without");
    let has_boundary = lower.contains("proof")
        || lower.contains("ub-free")
        || lower.contains("miri")
        || lower.contains("witness")
        || lower.contains("blocking");
    has_negative && has_boundary
}

fn check_docs_automation_impl() -> Result<usize, String> {
    let value = parse_toml_file(Path::new(DOCS_AUTOMATION_LEDGER))?;
    require_toml_string(&value, "schema_version", DOCS_AUTOMATION_LEDGER)?;

    let scope = value
        .get("scope")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{DOCS_AUTOMATION_LEDGER} is missing table `scope`"))?;
    require_scope_paths(scope, "owned_roots", true)?;
    require_scope_paths(scope, "external_awareness_only", false)?;

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
            }
        }

        let paths = docs_automation_paths(table, idx)?;
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
) -> Result<(), String> {
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
        for value in values {
            require_existing_repo_path(value, DOCS_AUTOMATION_LEDGER, key)?;
        }
    }
    Ok(())
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

fn source_truth_index_ids(value: &toml::Value, kind: &str) -> Result<BTreeSet<String>, String> {
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
    let mut fixture_cases = BTreeMap::new();
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
        let expected_cards = required_case_usize(case, "expected_cards", idx)?;
        let expected_class = optional_case_string(case, "expected_class", idx)?.map(str::to_string);
        let expected_operation_family =
            optional_case_string(case, "expected_operation_family", idx)?.map(str::to_string);
        let expected_hazard =
            optional_case_string(case, "expected_hazard", idx)?.map(str::to_string);
        check_calibration_case(case, fixture, kind, idx)?;
        fixture_cases.insert(
            fixture.to_string(),
            accuracy_labels::CalibrationFixtureCase {
                kind: kind.to_string(),
                expected_cards,
                expected_class,
                expected_operation_family,
                expected_hazard,
            },
        );
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

    let accuracy_policy = parse_toml_file(&workspace_path("policy/accuracy-calibration.toml"))?;
    let label_count =
        accuracy_labels::check_accuracy_label_ledgers(&accuracy_policy, &fixture_cases)?;

    println!(
        "check-calibration: ok ({} cases, {label_count} labels)",
        cases.len()
    );
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
    let mut repo_snapshots = 0usize;
    let mut pr_diffs = 0usize;
    for (idx, target) in targets.iter().enumerate() {
        let stats = dogfood_checks::validate_target(target, idx, &mut ids)?;
        repositories.insert(stats.repository);
        *artifact_status_counts
            .entry(stats.artifact_status)
            .or_insert(0usize) += 1;
        repo_snapshots += stats.repo_snapshots;
        pr_diffs += stats.pr_diffs;
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
        &repositories,
        &artifact_status_counts,
    )?;

    println!(
        "check-dogfood: ok ({} targets, {} repositories)",
        targets.len(),
        repositories.len()
    );
    Ok(())
}

mod dogfood_checks {
    use super::*;

    pub(super) struct TargetStats {
        pub(super) repository: String,
        pub(super) artifact_status: String,
        pub(super) repo_snapshots: usize,
        pub(super) pr_diffs: usize,
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
        let repository = required_target_string(target, "repository", idx)?;
        if !repository.contains('/') {
            return Err(format!(
                "{DOGFOOD_MANIFEST} targets[{idx}] repository `{repository}` must be owner/repo"
            ));
        }
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
        validate_artifacts(target, idx, artifact_status)?;
        let (repo_snapshots, pr_diffs) = validate_kind_fields(target, idx, kind)?;
        Ok(TargetStats {
            repository: repository.to_string(),
            artifact_status: artifact_status.to_string(),
            repo_snapshots,
            pr_diffs,
        })
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
    ) -> Result<(usize, usize), String> {
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
                Ok((1, 0))
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
                Ok((0, 1))
            }
            _ => Err(format!(
                "{DOGFOOD_MANIFEST} targets[{idx}] uses unsupported kind `{kind}`"
            )),
        }
    }
}

fn check_dogfood_index(
    target_count: usize,
    repository_count: usize,
    repo_snapshots: usize,
    pr_diffs: usize,
    repositories: &BTreeSet<String>,
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
        let schema = json_usize_at(&value, "/schemaVersion", path)?;
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
    parse_text_file(path, "TOML", |text| {
        text.parse::<toml::Table>().map(toml::Value::Table)
    })
}

fn parse_json_file(path: &Path) -> Result<serde_json::Value, String> {
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

fn json_usize_at(value: &serde_json::Value, pointer: &str, path: &str) -> Result<usize, String> {
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

fn require_known(value: &str, known: &[&str], path: &str, field: &str) -> Result<(), String> {
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

fn require_non_empty_json_str<'a>(
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

fn text_contains_ignore_ascii_case(text: &str, needle: &str) -> bool {
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

fn require_file(path: &str) -> Result<(), String> {
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

    fn err_text<T>(result: Result<T, String>) -> Result<String, String> {
        match result {
            Ok(_) => Err("expected error".to_string()),
            Err(err) => Ok(err),
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
    fn public_surface_checker_validates_current_contract() -> Result<(), String> {
        check_public_surfaces_impl().map(|_| ())
    }

    #[test]
    fn public_surface_boundary_requires_negative_claim_limit() {
        assert!(public_surface_has_trust_boundary(
            "This is advisory review evidence, not memory-safety proof."
        ));
        assert!(public_surface_has_trust_boundary(
            "The command does not run Miri or enable blocking policy by default."
        ));
        assert!(!public_surface_has_trust_boundary(
            "This command proves the reviewed code is safe."
        ));
        assert!(!public_surface_has_trust_boundary(
            "Install this command to review pull requests."
        ));
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
    fn first_pr_artifact_checker_rejects_lsp_without_obligation_evidence() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-obligation-evidence")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        fs::write(
            dir.join("lsp.json"),
            r#"{"tool":"unsafe-review","mode":"read_only_projection","policy":"advisory","status":{"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"},"diagnostics":[{"card_id":"card-1","witness_routes":[{"kind":"miri","reason":"route","command":"cargo +nightly miri test card","required":false}],"verify_commands":["cargo +nightly miri test card"],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}],"hovers":[{"card_id":"card-1","contents":"Trust boundary: static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}],"code_actions":[{"card_id":"card-1","command":"unsafe-review.copyAgentPacket","payload":{"kind":"unsafe-review.agent_packet","card_id":"card-1","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"},"arguments":["card-1"]}],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
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
    fn first_pr_artifact_checker_rejects_lsp_code_action_without_payload() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-first-pr-lsp-action-payload")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_first_pr_artifacts(&dir)?;
        fs::write(
            dir.join("lsp.json"),
            valid_lsp_json(
                r#"[{"card_id":"card-1","command":"unsafe-review.copyAgentPacket","arguments":["card-1"]}]"#,
            ),
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
    fn advisory_artifact_checker_rejects_unknown_projection_card_ids() -> Result<(), String> {
        let dir = unique_temp_dir("unsafe-review-artifacts-unknown-id")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        fs::write(
            dir.join("comment-plan.json"),
            r#"{"mode":"plan_only","policy":"advisory","comments":[{"card_id":"missing","body":"Plan boundary: artifact-only inline comment candidate; unsafe-review did not post this comment, run witnesses, or make a policy decision."}],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
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
            r#"{"mode":"plan_only","policy":"advisory","comments":[{"card_id":"card-1","witness_routes":[{"kind":"miri","reason":"route","command":"cargo +nightly miri test card","required":false}],"verify_commands":["cargo +nightly miri test card"],"body":"Plan boundary: artifact-only inline comment candidate; unsafe-review did not post this comment, run witnesses, or make a policy decision."}],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
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
            r#"{"version":"2.1.0","runs":[{"results":[{"properties":{"cardId":"card-1","verifyCommands":["cargo test"]}}],"properties":{"trustBoundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}}]}"#,
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
    fn advisory_artifact_checker_rejects_comment_plan_without_route_details() -> Result<(), String>
    {
        let dir = unique_temp_dir("unsafe-review-artifacts-comment-routes")?;
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        write_valid_artifacts(&dir)?;
        fs::write(
            dir.join("comment-plan.json"),
            r#"{"mode":"plan_only","policy":"advisory","comments":[{"card_id":"card-1","path":"src/lib.rs","line":7,"body":"Plan boundary: artifact-only inline comment candidate; unsafe-review did not post this comment, run witnesses, or make a policy decision."}],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
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
            r#"{"mode":"plan_only","policy":"advisory","comments":[{"card_id":"card-1","path":"src/lib.rs","line":7,"witness_routes":[{"kind":"miri","reason":"route","command":"cargo +nightly miri test card","required":false}],"verify_commands":["cargo +nightly miri test card"],"body":"Missing evidence only."}],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
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
        fs::write(
            dir.join("cards.json"),
            r#"{"tool":"unsafe-review","policy":"advisory","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result","summary":{"cards":1},"cards":[{"id":"card-1"}]}"#,
        )
        .map_err(|err| format!("write cards failed: {err}"))?;
        fs::write(
            dir.join("pr-summary.md"),
            "- Review cards: 1\n\nThis artifact is static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result unless a witness receipt is attached.\n",
        )
        .map_err(|err| format!("write pr summary failed: {err}"))?;
        fs::write(
            dir.join("cards.sarif"),
            r#"{"version":"2.1.0","runs":[{"results":[{"properties":{"cardId":"card-1","witnessRouteDetails":[{"kind":"miri","reason":"route","command":"cargo +nightly miri test card","required":false}],"verifyCommands":["cargo +nightly miri test card"]}}],"properties":{"trustBoundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}}]}"#,
        )
        .map_err(|err| format!("write sarif failed: {err}"))?;
        fs::write(
            dir.join("comment-plan.json"),
            r#"{"mode":"plan_only","policy":"advisory","comments":[{"card_id":"card-1","path":"src/lib.rs","line":7,"witness_routes":[{"kind":"miri","reason":"route","command":"cargo +nightly miri test card","required":false}],"verify_commands":["cargo +nightly miri test card"],"body":"Plan boundary: artifact-only inline comment candidate; unsafe-review did not post this comment, run witnesses, or make a policy decision."}],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}"#,
        )
        .map_err(|err| format!("write comment plan failed: {err}"))?;
        Ok(())
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
            valid_lsp_json(
                r#"[{"card_id":"card-1","command":"unsafe-review.copyAgentPacket","payload":{"kind":"unsafe-review.agent_packet","card_id":"card-1","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"},"arguments":["card-1"]}]"#,
            ),
        )
        .map_err(|err| format!("write lsp failed: {err}"))?;
        Ok(())
    }

    fn valid_lsp_json(code_actions: &str) -> String {
        format!(
            r#"{{"tool":"unsafe-review","mode":"read_only_projection","policy":"advisory","status":{{"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}},"diagnostics":[{{"card_id":"card-1","required_safety_conditions":[{{"key":"alignment","description":"pointer aligned"}}],"evidence_summary":{{"contract":{{"present":true,"state":"present","summary":"safety contract"}},"discharge":{{"present":false,"state":"missing","summary":"No visible local guard"}},"reach":{{"state":"owner_reached","summary":"related test mention"}},"witness":{{"present":false,"state":"missing","summary":"No imported witness receipt"}},"reach_limitation":"static reach evidence is not proof that the unsafe site executed"}},"obligation_evidence":[{{"key":"alignment","description":"pointer aligned","contract":{{"present":true,"state":"present","summary":"safety contract"}},"discharge":{{"present":false,"state":"missing","summary":"No visible local guard"}},"reach":{{"present":true,"state":"present","summary":"related test mention"}},"witness":{{"present":false,"state":"missing","summary":"No imported witness receipt"}}}}],"witness_routes":[{{"kind":"miri","reason":"route","command":"cargo +nightly miri test card","required":false}}],"verify_commands":["cargo +nightly miri test card"],"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}}],"hovers":[{{"card_id":"card-1","contents":"Trust boundary: static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}}],"code_actions":{code_actions},"trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}}"#
        )
    }

    fn write_valid_zero_card_first_pr_artifacts(dir: &Path) -> Result<(), String> {
        fs::write(
            dir.join("cards.json"),
            r#"{"tool":"unsafe-review","policy":"advisory","trust_boundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result","summary":{"cards":0},"cards":[]}"#,
        )
        .map_err(|err| format!("write cards failed: {err}"))?;
        fs::write(
            dir.join("pr-summary.md"),
            "- Review cards: 0\n\nNo changed unsafe-review gaps were found.\nThis does not prove the repo safe, UB-free, Miri-clean, or that any unsafe site executed.\n\nThis artifact is static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result unless a witness receipt is attached.\n",
        )
        .map_err(|err| format!("write pr summary failed: {err}"))?;
        fs::write(
            dir.join("cards.sarif"),
            r#"{"version":"2.1.0","runs":[{"results":[],"properties":{"trustBoundary":"static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result"}}]}"#,
        )
        .map_err(|err| format!("write sarif failed: {err}"))?;
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
