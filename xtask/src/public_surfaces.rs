use std::collections::BTreeSet;
use std::path::Path;

use crate::{
    check_markdown_local_links, public_badges, read_to_string, reject_positive_overclaims,
    require_file, require_known, require_toml_string, required_toml_string,
    text_contains_ignore_ascii_case, toml_str_array, workspace_path,
};

const PUBLIC_SURFACES_LEDGER: &str = "policy/public-surfaces.toml";

const PUBLIC_SURFACE_STATUSES: &[&str] = &["experimental", "accepted", "deferred"];
const PUBLIC_SURFACE_FRONT_DOORS: &[&str] = &[
    "README.md",
    "docs/FIRST_USE.md",
    "docs/CLI.md",
    "crates/unsafe-review/README.md",
    "crates/unsafe-review-cli/README.md",
    "crates/unsafe-review-core/README.md",
];
const FIRST_PR_ARTIFACT_LIST_SURFACES: &[&str] = &[
    ".github/examples/unsafe-review-first-pr.yml",
    ".github/workflows/unsafe-review.yml",
    "docs/CLI.md",
    "docs/FIRST_HOUR.md",
    "docs/FIRST_USE.md",
    "docs/ci/PR_CI.md",
    "docs/editor/saved-lsp-json.md",
    "docs/specs/UNSAFE-REVIEW-SPEC-0011-pr-ci-output.md",
    "docs/specs/UNSAFE-REVIEW-SPEC-0024-ci-design.md",
];
pub(crate) const FIRST_PR_BUNDLE_ARTIFACT_PATHS: &[&str] = &[
    "target/unsafe-review/review-kit.json",
    "target/unsafe-review/cards.json",
    "target/unsafe-review/pr-summary.md",
    "target/unsafe-review/github-summary.md",
    "target/unsafe-review/cards.sarif",
    "target/unsafe-review/comment-plan.json",
    "target/unsafe-review/witness-plan.md",
    "target/unsafe-review/lsp.json",
    "target/unsafe-review/manual-repair-queue.json",
    "target/unsafe-review/repair-queue.json",
];

pub(crate) fn check() -> Result<(), String> {
    let surfaces = check_impl()?;
    println!("check-public-surfaces: ok ({surfaces} surfaces)");
    Ok(())
}

pub(crate) fn check_impl() -> Result<usize, String> {
    let value = crate::parse_toml_file(&workspace_path(PUBLIC_SURFACES_LEDGER))?;
    require_toml_string(&value, "schema_version", PUBLIC_SURFACES_LEDGER)?;
    require_known(
        required_toml_string(&value, "status", PUBLIC_SURFACES_LEDGER)?,
        PUBLIC_SURFACE_STATUSES,
        PUBLIC_SURFACES_LEDGER,
        "status",
    )?;
    check_trust_boundary(required_toml_string(
        &value,
        "trust_boundary",
        PUBLIC_SURFACES_LEDGER,
    )?)?;
    check_forbidden_terms(&value)?;

    public_badges::check_endpoints()?;
    public_badges::check_generated_projection()?;
    for path in PUBLIC_SURFACE_FRONT_DOORS {
        check_front_door(path)?;
    }

    Ok(PUBLIC_SURFACE_FRONT_DOORS.len() + public_badges::endpoint_count())
}

pub(crate) fn check_first_pr_artifact_list_surfaces() -> Result<(), String> {
    for path in FIRST_PR_ARTIFACT_LIST_SURFACES {
        require_file(path)?;
        let source = workspace_path(path);
        let text = read_to_string(&source)?;
        require_first_pr_artifact_paths(path, &text)?;
    }
    Ok(())
}

fn check_trust_boundary(trust_boundary: &str) -> Result<(), String> {
    for required in ["advisory", "memory-safety proof", "UB-free", "Miri-clean"] {
        if !text_contains_ignore_ascii_case(trust_boundary, required) {
            return Err(format!(
                "{PUBLIC_SURFACES_LEDGER} trust_boundary must mention `{required}`"
            ));
        }
    }
    Ok(())
}

fn check_forbidden_terms(value: &toml::Value) -> Result<(), String> {
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
    Ok(())
}

pub(crate) fn require_first_pr_artifact_paths(path: &str, text: &str) -> Result<(), String> {
    for artifact in FIRST_PR_BUNDLE_ARTIFACT_PATHS {
        if !text.contains(artifact) {
            return Err(format!("{path} must list first-pr artifact `{artifact}`"));
        }
    }
    Ok(())
}

fn check_front_door(path: &str) -> Result<(), String> {
    require_file(path)?;
    check_markdown_local_links(path)?;
    let source = workspace_path(path);
    let text = read_to_string(&source)?;
    reject_positive_overclaims(Path::new(path), &text)?;
    if !has_trust_boundary(&text) {
        return Err(format!(
            "{path} must include advisory trust-boundary wording such as not-proof, not-UB-free, no-default-witness, or no-default-blocking language"
        ));
    }
    Ok(())
}

pub(crate) fn has_trust_boundary(text: &str) -> bool {
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
