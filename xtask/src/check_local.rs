#![forbid(unsafe_code)]

//! `check-local`: an honest, fast *local* proof tier.
//!
//! `check-pr` is the comprehensive gate and remains the only merge-readiness
//! proof. This command exists so iterative local work has a fast, deterministic
//! subset that is *structurally unable to masquerade as the full gate*:
//!
//! - it inspects the current diff and maps every changed path to a category;
//! - it selects the deterministic `check-pr` components relevant to those
//!   categories (unknown or code/xtask paths conservatively force the full set);
//! - it runs the selected checks and records each executed and skipped check
//!   with the reason it was selected or omitted;
//! - it emits a deterministic machine-readable receipt; and
//! - it always states that the full `check-pr` gate is still required.
//!
//! Trust boundary: the receipt reports a *bounded subset* of repository proof.
//! It does not establish full merge readiness, analyzer accuracy, memory
//! safety, UB-free status, Miri-clean status, or site execution. A skipped
//! check is never represented as passed.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::json;

pub(crate) const SCHEMA_VERSION: &str = "unsafe-review/check-local/v1";
pub(crate) const NEXT_COMMAND: &str = "cargo run --locked -p xtask -- check-pr";
const DEFAULT_OUT: &str = "target/check-local/receipt-v1.json";

/// The category a single changed path maps to.
///
/// Categories are ordered from most to least specific in [`categorize_path`].
/// Categories that cannot be safely sub-selected (product Rust source, xtask
/// routing, or anything unrecognized) return `true` from [`Self::forces_full`],
/// which promotes the whole run to the conservative full set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum PathCategory {
    Docs,
    Fixtures,
    Calibration,
    Policy,
    Workflow,
    Corpus,
    Fuzz,
    ProductRust,
    Xtask,
    Unknown,
}

impl PathCategory {
    pub(crate) fn label(self) -> &'static str {
        match self {
            PathCategory::Docs => "docs",
            PathCategory::Fixtures => "fixtures",
            PathCategory::Calibration => "calibration",
            PathCategory::Policy => "policy",
            PathCategory::Workflow => "workflow",
            PathCategory::Corpus => "corpus",
            PathCategory::Fuzz => "fuzz",
            PathCategory::ProductRust => "product-rust",
            PathCategory::Xtask => "xtask",
            PathCategory::Unknown => "unknown",
        }
    }

    /// Categories whose changes cannot be safely reduced to a check subset.
    ///
    /// Product Rust source and xtask routing can affect any downstream
    /// deterministic surface, and an unrecognized path has no mapping we can
    /// trust — all three force the conservative full set rather than an empty
    /// or partial selection.
    pub(crate) fn forces_full(self) -> bool {
        matches!(
            self,
            PathCategory::ProductRust | PathCategory::Xtask | PathCategory::Unknown
        )
    }
}

/// Map a repository-relative path to its [`PathCategory`].
///
/// Ordering is deliberate: more specific prefixes (`docs/dogfood/`,
/// `policy/calibration.toml`) are matched before their generic parents so a
/// corpus or calibration change is not misfiled as generic docs/policy.
/// Anything that is not explicitly recognized returns [`PathCategory::Unknown`]
/// so it forces the full set rather than silently selecting nothing.
pub(crate) fn categorize_path(path: &str) -> PathCategory {
    let path = path.trim();
    if path.starts_with("docs/dogfood/") {
        return PathCategory::Corpus;
    }
    if path == "policy/calibration.toml" {
        return PathCategory::Calibration;
    }
    if path.starts_with("fixtures/") {
        return PathCategory::Fixtures;
    }
    if path.starts_with("policy/") {
        return PathCategory::Policy;
    }
    if path.starts_with(".github/") {
        return PathCategory::Workflow;
    }
    if path.starts_with("fuzz/") {
        return PathCategory::Fuzz;
    }
    if path.starts_with("xtask/") {
        return PathCategory::Xtask;
    }
    if path.starts_with("crates/") && path.ends_with(".rs") {
        return PathCategory::ProductRust;
    }
    if path.starts_with("docs/")
        || matches!(
            path,
            "README.md" | "CHANGELOG.md" | "AGENTS.md" | "CLAUDE.md"
        )
    {
        return PathCategory::Docs;
    }
    PathCategory::Unknown
}

/// A single `check-pr` component and the rule that selects it locally.
#[derive(Clone, Copy)]
pub(crate) struct CheckSpec {
    /// Stable machine identifier, used as the receipt `name` and dispatch key.
    pub(crate) id: &'static str,
    /// Human-facing description of what the check validates.
    pub(crate) name: &'static str,
    /// Standalone xtask command that runs this check on its own, if one exists.
    /// `None` means the check only runs inside `check-pr` (and `check-local`).
    pub(crate) command: Option<&'static str>,
    /// Always run regardless of changed paths because a path-based skip would
    /// be unsafe (front-door wording, policy ledgers, self-`unsafe` guard).
    pub(crate) always: bool,
    /// Categories whose presence selects this check (in addition to the
    /// conservative full set triggered by [`PathCategory::forces_full`]).
    pub(crate) triggers: &'static [PathCategory],
}

/// The proof-map: every component of `check-pr` (in `check-pr` order) mapped to
/// a selection rule. Keep this in lock-step with the `CheckPr` arm in `main.rs`.
///
/// Rationale for the always-required entries (path-based skip would be unsafe):
/// `docs` guards front-door / claim-boundary wording across every surface,
/// `policy` guards ledgers and allowlists, and `self-unsafe` guards the
/// repository-wide `unsafe` forbiddance — all three are cheap and any change
/// could regress them.
pub(crate) const CATALOG: &[CheckSpec] = &[
    CheckSpec {
        id: "docs",
        name: "documentation and claim-boundary gates",
        command: Some("check-docs"),
        always: true,
        triggers: &[],
    },
    CheckSpec {
        id: "generated-projection",
        name: "generated badge/endpoint projection",
        command: None,
        always: false,
        // Badge/endpoint projections derive from docs, fixtures, calibration,
        // and corpus-derived counts; trigger conservatively on all four rather
        // than risk skipping a projection whose inputs a corpus diff touched.
        triggers: &[
            PathCategory::Docs,
            PathCategory::Fixtures,
            PathCategory::Calibration,
            PathCategory::Corpus,
        ],
    },
    CheckSpec {
        id: "policy",
        name: "policy ledgers and allowlists",
        command: Some("check-policy"),
        always: true,
        triggers: &[],
    },
    CheckSpec {
        id: "support-tiers",
        name: "support-tier claim-to-proof ledger",
        command: Some("check-support-tiers"),
        always: false,
        triggers: &[PathCategory::Docs],
    },
    CheckSpec {
        id: "fixtures",
        name: "fixture surfaces",
        command: Some("check-fixtures"),
        always: false,
        triggers: &[PathCategory::Fixtures, PathCategory::Calibration],
    },
    CheckSpec {
        id: "calibration",
        name: "calibration manifest",
        command: Some("check-calibration"),
        always: false,
        triggers: &[PathCategory::Fixtures, PathCategory::Calibration],
    },
    CheckSpec {
        id: "fixture-surface-parity",
        name: "fixture surface parity",
        command: Some("check-fixture-surface-parity"),
        always: false,
        triggers: &[PathCategory::Fixtures, PathCategory::Calibration],
    },
    CheckSpec {
        id: "surface-determinism",
        name: "surface determinism",
        command: Some("check-surface-determinism"),
        always: false,
        triggers: &[PathCategory::Fixtures, PathCategory::Calibration],
    },
    CheckSpec {
        id: "real-pr-corpus",
        name: "real PR corpus",
        command: Some("check-real-pr-corpus"),
        always: false,
        triggers: &[PathCategory::Corpus],
    },
    CheckSpec {
        id: "corpus-partitions",
        name: "corpus partitions",
        command: Some("check-corpus-partitions"),
        always: false,
        triggers: &[PathCategory::Corpus],
    },
    CheckSpec {
        id: "evidence-loss-challenges",
        name: "evidence-loss challenges",
        command: Some("check-evidence-loss-challenges"),
        always: false,
        triggers: &[PathCategory::Corpus],
    },
    CheckSpec {
        id: "external-pilots",
        name: "external pilots",
        command: Some("check-external-pilots"),
        always: false,
        triggers: &[PathCategory::Corpus],
    },
    CheckSpec {
        id: "dogfood",
        name: "dogfood evidence",
        command: Some("check-dogfood"),
        always: false,
        triggers: &[PathCategory::Corpus],
    },
    CheckSpec {
        id: "fuzz-manual-harness",
        name: "manual fuzz harness",
        command: Some("check-fuzz"),
        always: false,
        triggers: &[PathCategory::Fuzz],
    },
    CheckSpec {
        id: "fuzz-tracked-artifacts",
        name: "tracked generated fuzz artifacts",
        command: None,
        always: false,
        triggers: &[PathCategory::Fuzz],
    },
    CheckSpec {
        id: "self-unsafe",
        name: "self-`unsafe` forbiddance",
        command: Some("check-self-unsafe"),
        always: true,
        triggers: &[],
    },
];

/// A catalog check plus the decision `check-local` made about it.
pub(crate) struct PlannedCheck {
    pub(crate) spec: &'static CheckSpec,
    pub(crate) selected: bool,
    pub(crate) reason: String,
}

/// The full selection decision for one diff.
pub(crate) struct Plan {
    /// Distinct categories present in the diff, sorted for determinism.
    pub(crate) categories: Vec<PathCategory>,
    /// True when the diff forced the conservative full set (unknown/empty/code).
    pub(crate) conservative_full: bool,
    /// Every catalog check with its selected/skipped decision, in catalog order.
    pub(crate) checks: Vec<PlannedCheck>,
}

/// Decide which `check-pr` components to run for a set of changed paths.
///
/// Conservative by construction: a degraded diff (`diff_degraded`), an empty
/// diff, an unrecognized path, or a product-Rust/xtask change forces the full
/// set. Unknown paths therefore never select an empty proof set — they select
/// everything.
///
/// `diff_degraded` is set by [`resolve_diff`] when the diff could not be fully
/// determined (a git command failed, or no base ref resolved). A degraded diff
/// may under-report changed paths, so it forces the conservative full set even
/// when the partial `changed_paths` list looks innocuous — otherwise a stray
/// untracked file could produce a targeted selection that silently omits an
/// unseen product-Rust change.
pub(crate) fn plan_inner(changed_paths: &[String], diff_degraded: bool) -> Plan {
    let mut categories: Vec<PathCategory> = Vec::new();
    for path in changed_paths {
        let category = categorize_path(path);
        if !categories.contains(&category) {
            categories.push(category);
        }
    }
    categories.sort();

    let empty_diff = changed_paths.is_empty();
    let force_categories: Vec<PathCategory> = categories
        .iter()
        .copied()
        .filter(|category| category.forces_full())
        .collect();
    let conservative_full = diff_degraded || empty_diff || !force_categories.is_empty();

    let full_reason = if diff_degraded {
        "conservative full set: the diff could not be fully determined (a git command failed or no base ref resolved), so nothing can be safely skipped"
            .to_string()
    } else if empty_diff {
        "conservative full set: empty or unavailable diff, so nothing can be safely skipped"
            .to_string()
    } else {
        format!(
            "conservative full set: changed paths include {} which cannot be safely sub-selected",
            force_categories
                .iter()
                .map(|category| format!("`{}`", category.label()))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };

    let category_note = if categories.is_empty() {
        "none".to_string()
    } else {
        categories
            .iter()
            .map(|category| category.label())
            .collect::<Vec<_>>()
            .join(", ")
    };

    let checks = CATALOG
        .iter()
        .map(|spec| {
            if spec.always {
                PlannedCheck {
                    spec,
                    selected: true,
                    reason: "always required (path-based skip would be unsafe)".to_string(),
                }
            } else if conservative_full {
                PlannedCheck {
                    spec,
                    selected: true,
                    reason: full_reason.clone(),
                }
            } else if let Some(trigger) = spec
                .triggers
                .iter()
                .find(|trigger| categories.contains(trigger))
            {
                PlannedCheck {
                    spec,
                    selected: true,
                    reason: format!("selected for changed category `{}`", trigger.label()),
                }
            } else {
                PlannedCheck {
                    spec,
                    selected: false,
                    reason: format!(
                        "not selected for changed paths (categories present: {category_note})"
                    ),
                }
            }
        })
        .collect();

    Plan {
        categories,
        conservative_full,
        checks,
    }
}

/// The resolved diff `check-local` is reasoning about.
pub(crate) struct Diff {
    pub(crate) base: String,
    pub(crate) head: String,
    pub(crate) changed_paths: Vec<String>,
}

/// Parsed `check-local` arguments.
pub(crate) struct Args {
    base: Option<String>,
    format: Format,
    out: Option<PathBuf>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Format {
    Human,
    Json,
}

impl Args {
    /// Parse `check-local [--base <ref>] [--format human|json] [--out <path>]`.
    ///
    /// `raw` is the full xtask argv (`raw[0]` = program, `raw[1]` = command),
    /// mirroring how `dogfood-exec` forwards its arguments.
    pub(crate) fn parse(raw: &[String]) -> Result<Self, String> {
        let mut base = None;
        let mut format = Format::Human;
        let mut out = None;
        let mut index = 2;
        while index < raw.len() {
            match raw[index].as_str() {
                "--base" => {
                    base = Some(require_value(raw, index, "--base")?);
                    index += 2;
                }
                "--format" => {
                    let value = require_value(raw, index, "--format")?;
                    format = match value.as_str() {
                        "human" => Format::Human,
                        "json" => Format::Json,
                        other => {
                            return Err(format!(
                                "check-local: --format expects `human` or `json`, got `{other}`"
                            ));
                        }
                    };
                    index += 2;
                }
                "--out" => {
                    out = Some(PathBuf::from(require_value(raw, index, "--out")?));
                    index += 2;
                }
                other => {
                    return Err(format!(
                        "check-local: unexpected argument `{other}`; usage: check-local [--base <ref>] [--format human|json] [--out <path>]"
                    ));
                }
            }
        }
        Ok(Self { base, format, out })
    }
}

fn require_value(raw: &[String], index: usize, flag: &str) -> Result<String, String> {
    raw.get(index + 1)
        .cloned()
        .ok_or_else(|| format!("check-local: {flag} requires a value"))
}

/// Run `check-local`: resolve the diff, plan, execute the selected checks via
/// `dispatch`, emit the receipt, and require the full gate.
///
/// `dispatch` maps a [`CheckSpec::id`] to the underlying `check-pr` component so
/// the selection logic here stays pure and unit-testable without executing the
/// heavyweight checks.
pub(crate) fn run(
    raw: &[String],
    dispatch: &dyn Fn(&str) -> Result<(), String>,
) -> Result<(), String> {
    let args = Args::parse(raw)?;
    let (diff, degraded) = resolve_diff(args.base.as_deref());
    let plan = plan_inner(&diff.changed_paths, degraded);

    let mut outcomes: Vec<(&'static str, Result<(), String>)> = Vec::new();
    for check in plan.checks.iter().filter(|check| check.selected) {
        let result = dispatch(check.spec.id);
        outcomes.push((check.spec.id, result));
    }

    let receipt = build_receipt(&diff, &plan, &outcomes);
    let mut receipt_text = serde_json::to_string_pretty(&receipt)
        .map_err(|err| format!("check-local: failed to serialize receipt: {err}"))?;
    receipt_text.push('\n');

    let out_path = args
        .out
        .clone()
        .unwrap_or_else(|| PathBuf::from(DEFAULT_OUT));
    write_receipt(&out_path, &receipt_text)?;

    match args.format {
        Format::Human => print!("{}", render_human(&diff, &plan, &outcomes, &out_path)),
        Format::Json => print!("{receipt_text}"),
    }

    let failed: Vec<&str> = outcomes
        .iter()
        .filter(|(_, result)| result.is_err())
        .map(|(id, _)| *id)
        .collect();
    if failed.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "check-local: {} selected check(s) failed: {}. This is a partial local proof; the full gate is still required before merge: {NEXT_COMMAND}",
            failed.len(),
            failed.join(", ")
        ))
    }
}

/// Resolve the base/head refs and the union of changed paths from git.
///
/// Returns the [`Diff`] plus a `degraded` flag that is `true` when the diff may
/// be incomplete: no base ref resolved, or a git command we rely on for the
/// changed-path set failed. A degraded diff must force the conservative full
/// set (see [`plan_inner`]) — otherwise a partially-observed path list (e.g. one
/// stray untracked file while the base diff errored) could yield a targeted
/// selection that silently omits an unseen product-Rust change.
fn resolve_diff(base_arg: Option<&str>) -> (Diff, bool) {
    let head = git(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|_| "unknown".to_string());
    let base_ref = base_arg
        .map(str::to_string)
        .or_else(|| git(&["merge-base", "origin/main", "HEAD"]).ok())
        .or_else(|| git(&["merge-base", "main", "HEAD"]).ok())
        .or_else(|| git(&["rev-parse", "HEAD~1"]).ok());

    let mut paths: BTreeSet<String> = BTreeSet::new();
    let mut degraded = false;

    // Untracked files are always part of "the current diff".
    match git(&["ls-files", "--others", "--exclude-standard"]) {
        Ok(out) => collect_lines(&out, &mut paths),
        Err(_) => degraded = true,
    }

    let base_display = match &base_ref {
        Some(base) => {
            // `git diff --name-only <base>` already reports committed-since-base,
            // staged, and unstaged tracked changes vs the base, so no separate
            // `--cached` pass is needed. If it fails we may be under-reporting.
            match git(&["diff", "--name-only", base]) {
                Ok(out) => collect_lines(&out, &mut paths),
                Err(_) => degraded = true,
            }
            git(&["rev-parse", "--short", base]).unwrap_or_else(|_| base.clone())
        }
        None => {
            // No base ref resolved: we cannot compute a reliable diff, so mark
            // the result degraded (forces the full set) while still surfacing any
            // working-tree changes vs HEAD for the receipt.
            degraded = true;
            if let Ok(out) = git(&["diff", "--name-only", "HEAD"]) {
                collect_lines(&out, &mut paths);
            }
            "unknown".to_string()
        }
    };

    let diff = Diff {
        base: base_display,
        head,
        changed_paths: paths.into_iter().collect(),
    };
    (diff, degraded)
}

fn collect_lines(output: &str, into: &mut BTreeSet<String>) {
    for line in output.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            into.insert(trimmed.to_string());
        }
    }
}

fn git(args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .output()
        .map_err(|err| format!("failed to run git {args:?}: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn write_receipt(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|err| {
            format!(
                "check-local: failed to create receipt directory {}: {err}",
                parent.display()
            )
        })?;
    }
    std::fs::write(path, text).map_err(|err| {
        format!(
            "check-local: failed to write receipt {}: {err}",
            path.display()
        )
    })
}

/// Build the deterministic `unsafe-review/check-local/v1` receipt.
///
/// Arrays are emitted in catalog order (executed, skipped) and sorted order
/// (changed_paths, categories), so the same diff and the same check results
/// always produce byte-identical output. `full_gate_required` is always `true`;
/// a skipped check is only ever listed under `skipped`, never `executed`.
pub(crate) fn build_receipt(
    diff: &Diff,
    plan: &Plan,
    outcomes: &[(&str, Result<(), String>)],
) -> serde_json::Value {
    let executed: Vec<serde_json::Value> = plan
        .checks
        .iter()
        .filter(|check| check.selected)
        .map(|check| {
            let (result, detail) = match outcomes.iter().find(|(id, _)| *id == check.spec.id) {
                Some((_, Ok(()))) => ("pass", serde_json::Value::Null),
                Some((_, Err(message))) => ("fail", json!(first_line(message))),
                None => ("not_executed", json!("selected but not executed")),
            };
            json!({
                "name": check.spec.id,
                "check": check.spec.name,
                "command": check.spec.command,
                "selection_reason": check.reason,
                "result": result,
                "detail": detail,
            })
        })
        .collect();

    let skipped: Vec<serde_json::Value> = plan
        .checks
        .iter()
        .filter(|check| !check.selected)
        .map(|check| {
            json!({
                "name": check.spec.id,
                "check": check.spec.name,
                "command": check.spec.command,
                "reason": check.reason,
            })
        })
        .collect();

    let all_passed = executed
        .iter()
        .all(|entry| entry.get("result") == Some(&json!("pass")));

    json!({
        "schema_version": SCHEMA_VERSION,
        "status": "partial_proof",
        "result": if all_passed { "pass" } else { "fail" },
        "selection_mode": if plan.conservative_full { "conservative_full" } else { "targeted" },
        "base": diff.base,
        "head": diff.head,
        "changed_paths": diff.changed_paths,
        "categories": plan
            .categories
            .iter()
            .map(|category| category.label())
            .collect::<Vec<_>>(),
        "executed": executed,
        "skipped": skipped,
        "full_gate_required": true,
        "next_command": NEXT_COMMAND,
    })
}

fn first_line(message: &str) -> String {
    message.lines().next().unwrap_or("").trim().to_string()
}

/// Render the human summary. Carries the same information as the receipt and
/// ends with the full-gate requirement.
fn render_human(
    diff: &Diff,
    plan: &Plan,
    outcomes: &[(&str, Result<(), String>)],
    out_path: &Path,
) -> String {
    let mut lines = String::new();
    lines.push_str(
        "unsafe-review check-local — partial local proof (NOT a substitute for check-pr)\n",
    );
    let mode = if plan.conservative_full {
        "conservative_full"
    } else {
        "targeted"
    };
    lines.push_str(&format!(
        "base {}  head {}  mode {mode}\n",
        diff.base, diff.head
    ));

    if diff.changed_paths.is_empty() {
        lines.push_str("changed paths: none detected (running the conservative full set)\n");
    } else {
        lines.push_str(&format!("changed paths ({}):\n", diff.changed_paths.len()));
        for path in &diff.changed_paths {
            lines.push_str(&format!("  - {path}\n"));
        }
    }
    let categories = if plan.categories.is_empty() {
        "none".to_string()
    } else {
        plan.categories
            .iter()
            .map(|category| category.label())
            .collect::<Vec<_>>()
            .join(", ")
    };
    lines.push_str(&format!("categories: {categories}\n\n"));

    let executed: Vec<&PlannedCheck> = plan.checks.iter().filter(|check| check.selected).collect();
    lines.push_str(&format!("Executed ({}):\n", executed.len()));
    for check in &executed {
        let (marker, detail) = match outcomes.iter().find(|(id, _)| *id == check.spec.id) {
            Some((_, Ok(()))) => ("pass", String::new()),
            Some((_, Err(message))) => ("FAIL", format!("  — {}", first_line(message))),
            None => ("????", "  — selected but not executed".to_string()),
        };
        lines.push_str(&format!(
            "  [{marker}] {} — {} ({}){detail}\n",
            check.spec.id, check.spec.name, check.reason
        ));
    }

    let skipped: Vec<&PlannedCheck> = plan.checks.iter().filter(|check| !check.selected).collect();
    lines.push_str(&format!("\nSkipped ({}):\n", skipped.len()));
    if skipped.is_empty() {
        lines.push_str("  (none — every check-pr component was selected)\n");
    } else {
        for check in &skipped {
            lines.push_str(&format!(
                "  - {} — {}: {}\n",
                check.spec.id, check.spec.name, check.reason
            ));
        }
    }

    lines.push_str(&format!("\nReceipt: {}\n", out_path.display()));
    lines.push_str(
        "This is a partial local proof and a skipped check is NOT a passed check. The full gate is still required before merge:\n",
    );
    lines.push_str(&format!("  {NEXT_COMMAND}\n"));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(list: &[&str]) -> Vec<String> {
        list.iter().map(|path| (*path).to_string()).collect()
    }

    /// Test shorthand for a non-degraded plan.
    fn plan(changed_paths: &[String]) -> Plan {
        plan_inner(changed_paths, false)
    }

    fn selected_ids(plan: &Plan) -> Vec<&'static str> {
        plan.checks
            .iter()
            .filter(|check| check.selected)
            .map(|check| check.spec.id)
            .collect()
    }

    fn skipped_ids(plan: &Plan) -> Vec<&'static str> {
        plan.checks
            .iter()
            .filter(|check| !check.selected)
            .map(|check| check.spec.id)
            .collect()
    }

    #[test]
    fn catalog_matches_check_pr_component_count() {
        // check-pr runs exactly 16 deterministic components; the proof-map must
        // enumerate all of them so none is silently unmapped.
        assert_eq!(CATALOG.len(), 16);
    }

    #[test]
    fn catalog_ids_are_pinned() {
        // Pin the exact ids (and order) so a rename here forces a matching
        // update to `run_named_check` in main.rs and to the docs proof-map.
        let ids: Vec<&str> = CATALOG.iter().map(|spec| spec.id).collect();
        assert_eq!(
            ids,
            vec![
                "docs",
                "generated-projection",
                "policy",
                "support-tiers",
                "fixtures",
                "calibration",
                "fixture-surface-parity",
                "surface-determinism",
                "real-pr-corpus",
                "corpus-partitions",
                "evidence-loss-challenges",
                "external-pilots",
                "dogfood",
                "fuzz-manual-harness",
                "fuzz-tracked-artifacts",
                "self-unsafe",
            ]
        );
    }

    #[test]
    fn catalog_ids_are_unique() {
        let mut ids: Vec<&str> = CATALOG.iter().map(|spec| spec.id).collect();
        ids.sort_unstable();
        let mut deduped = ids.clone();
        deduped.dedup();
        assert_eq!(ids, deduped, "catalog ids must be unique");
    }

    #[test]
    fn categorize_path_orders_specific_before_generic() {
        assert_eq!(
            categorize_path("docs/dogfood/corpus.toml"),
            PathCategory::Corpus
        );
        assert_eq!(categorize_path("docs/specs/FOO.md"), PathCategory::Docs);
        assert_eq!(
            categorize_path("policy/calibration.toml"),
            PathCategory::Calibration
        );
        assert_eq!(
            categorize_path("policy/no-panic.toml"),
            PathCategory::Policy
        );
        assert_eq!(
            categorize_path("fixtures/foo/src/lib.rs"),
            PathCategory::Fixtures
        );
        assert_eq!(
            categorize_path(".github/workflows/ci.yml"),
            PathCategory::Workflow
        );
        assert_eq!(
            categorize_path("fuzz/fuzz_targets/analyze.rs"),
            PathCategory::Fuzz
        );
        assert_eq!(categorize_path("xtask/src/main.rs"), PathCategory::Xtask);
        assert_eq!(
            categorize_path("crates/unsafe-review-core/src/lib.rs"),
            PathCategory::ProductRust
        );
        assert_eq!(categorize_path("README.md"), PathCategory::Docs);
        assert_eq!(categorize_path("Cargo.toml"), PathCategory::Unknown);
    }

    #[test]
    fn docs_only_diff_runs_always_plus_docs_and_skips_the_rest() {
        let plan = plan(&paths(&["docs/specs/FOO.md", "README.md"]));
        assert!(!plan.conservative_full);
        let selected = selected_ids(&plan);
        // always-required + docs-triggered.
        assert!(selected.contains(&"docs"));
        assert!(selected.contains(&"policy"));
        assert!(selected.contains(&"self-unsafe"));
        assert!(selected.contains(&"support-tiers"));
        assert!(selected.contains(&"generated-projection"));
        // heavy corpus/fixture/fuzz checks are skipped.
        let skipped = skipped_ids(&plan);
        assert!(skipped.contains(&"dogfood"));
        assert!(skipped.contains(&"fixtures"));
        assert!(skipped.contains(&"fuzz-manual-harness"));
        assert!(skipped.contains(&"external-pilots"));
    }

    #[test]
    fn analyzer_source_diff_forces_conservative_full() {
        let plan = plan(&paths(&[
            "crates/unsafe-review-core/src/analysis/scanner.rs",
        ]));
        assert!(plan.conservative_full);
        assert_eq!(selected_ids(&plan).len(), CATALOG.len());
        assert!(skipped_ids(&plan).is_empty());
    }

    #[test]
    fn fixture_diff_selects_fixture_family_only() {
        let plan = plan(&paths(&["fixtures/raw_pointer_alignment/change.diff"]));
        assert!(!plan.conservative_full);
        let selected = selected_ids(&plan);
        assert!(selected.contains(&"fixtures"));
        assert!(selected.contains(&"calibration"));
        assert!(selected.contains(&"fixture-surface-parity"));
        assert!(selected.contains(&"surface-determinism"));
        assert!(selected.contains(&"generated-projection"));
        let skipped = skipped_ids(&plan);
        assert!(skipped.contains(&"dogfood"));
        assert!(skipped.contains(&"support-tiers"));
        assert!(skipped.contains(&"fuzz-manual-harness"));
    }

    #[test]
    fn projection_source_diff_forces_full() {
        // Output/projection logic lives in product Rust, which cannot be safely
        // sub-selected.
        let plan = plan(&paths(&[
            "crates/unsafe-review-core/src/output/comment_plan.rs",
        ]));
        assert!(plan.conservative_full);
        assert_eq!(selected_ids(&plan).len(), CATALOG.len());
    }

    #[test]
    fn workflow_and_policy_diff_runs_always_set_only() {
        let plan = plan(&paths(&[
            ".github/workflows/ci.yml",
            "policy/workflow-allowlist.toml",
        ]));
        assert!(!plan.conservative_full);
        let selected = selected_ids(&plan);
        assert!(selected.contains(&"policy"));
        assert!(selected.contains(&"docs"));
        assert!(selected.contains(&"self-unsafe"));
        // Nothing corpus/fixture/fuzz-specific.
        let skipped = skipped_ids(&plan);
        assert!(skipped.contains(&"fixtures"));
        assert!(skipped.contains(&"dogfood"));
        assert!(skipped.contains(&"fuzz-manual-harness"));
    }

    #[test]
    fn corpus_diff_selects_corpus_family() {
        let plan = plan(&paths(&["docs/dogfood/corpus.toml"]));
        assert!(!plan.conservative_full);
        let selected = selected_ids(&plan);
        for id in [
            "real-pr-corpus",
            "corpus-partitions",
            "evidence-loss-challenges",
            "external-pilots",
            "dogfood",
        ] {
            assert!(selected.contains(&id), "corpus diff must select {id}");
        }
        assert!(skipped_ids(&plan).contains(&"fixtures"));
    }

    #[test]
    fn unknown_path_forces_full_never_empty() {
        let plan = plan(&paths(&["some/unmapped/thing.txt"]));
        assert!(plan.conservative_full);
        assert_eq!(selected_ids(&plan).len(), CATALOG.len());
        assert!(skipped_ids(&plan).is_empty());
    }

    #[test]
    fn empty_diff_forces_full_never_empty() {
        let plan = plan(&[]);
        assert!(plan.conservative_full);
        assert_eq!(selected_ids(&plan).len(), CATALOG.len());
    }

    #[test]
    fn degraded_diff_forces_full_even_with_innocuous_paths() {
        // A stray untracked doc while the base diff failed must NOT yield a
        // targeted docs-only selection: the degraded flag forces the full set so
        // an unseen product-Rust change can never be silently skipped.
        let targeted = plan_inner(&paths(&["docs/x.md"]), false);
        assert!(!targeted.conservative_full);
        let degraded = plan_inner(&paths(&["docs/x.md"]), true);
        assert!(degraded.conservative_full);
        assert_eq!(selected_ids(&degraded).len(), CATALOG.len());
        assert!(skipped_ids(&degraded).is_empty());
    }

    #[test]
    fn always_required_checks_are_never_skipped() {
        for changed in [
            paths(&["docs/x.md"]),
            paths(&["fixtures/x/change.diff"]),
            paths(&["docs/dogfood/corpus.toml"]),
            paths(&[".github/workflows/ci.yml"]),
            paths(&["fuzz/fuzz_targets/analyze.rs"]),
        ] {
            let plan = plan(&changed);
            let skipped = skipped_ids(&plan);
            for always in ["docs", "policy", "self-unsafe"] {
                assert!(
                    !skipped.contains(&always),
                    "{always} must never be skipped (diff: {changed:?})"
                );
            }
        }
    }

    #[test]
    fn receipt_always_requires_full_gate_and_is_partial() {
        let diff = Diff {
            base: "aaaa".to_string(),
            head: "bbbb".to_string(),
            changed_paths: paths(&["docs/x.md"]),
        };
        let plan = plan(&diff.changed_paths);
        let outcomes: Vec<(&str, Result<(), String>)> = plan
            .checks
            .iter()
            .filter(|check| check.selected)
            .map(|check| (check.spec.id, Ok(())))
            .collect();
        let receipt = build_receipt(&diff, &plan, &outcomes);
        assert_eq!(receipt["schema_version"], json!(SCHEMA_VERSION));
        assert_eq!(receipt["status"], json!("partial_proof"));
        assert_eq!(receipt["full_gate_required"], json!(true));
        assert_eq!(receipt["result"], json!("pass"));
        assert_eq!(receipt["next_command"], json!(NEXT_COMMAND));
    }

    #[test]
    fn skipped_check_is_never_reported_as_passed() {
        let diff = Diff {
            base: "aaaa".to_string(),
            head: "bbbb".to_string(),
            changed_paths: paths(&["docs/x.md"]),
        };
        let plan = plan(&diff.changed_paths);
        let outcomes: Vec<(&str, Result<(), String>)> = plan
            .checks
            .iter()
            .filter(|check| check.selected)
            .map(|check| (check.spec.id, Ok(())))
            .collect();
        let receipt = build_receipt(&diff, &plan, &outcomes);
        let executed_names: Vec<String> = receipt["executed"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|entry| entry["name"].as_str().map(str::to_string))
            .collect();
        let skipped_names: Vec<String> = receipt["skipped"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|entry| entry["name"].as_str().map(str::to_string))
            .collect();
        // A skipped check appears only under skipped, never executed.
        for skipped in &skipped_names {
            assert!(!executed_names.contains(skipped));
        }
        assert!(skipped_names.contains(&"dogfood".to_string()));
    }

    #[test]
    fn receipt_records_failed_executed_check() {
        let diff = Diff {
            base: "aaaa".to_string(),
            head: "bbbb".to_string(),
            changed_paths: paths(&["docs/x.md"]),
        };
        let plan = plan(&diff.changed_paths);
        let outcomes: Vec<(&str, Result<(), String>)> = plan
            .checks
            .iter()
            .filter(|check| check.selected)
            .map(|check| {
                if check.spec.id == "docs" {
                    (check.spec.id, Err("docs boom\nsecond line".to_string()))
                } else {
                    (check.spec.id, Ok(()))
                }
            })
            .collect();
        let receipt = build_receipt(&diff, &plan, &outcomes);
        assert_eq!(receipt["result"], json!("fail"));
        let docs = receipt["executed"]
            .as_array()
            .and_then(|entries| {
                entries
                    .iter()
                    .find(|entry| entry["name"] == json!("docs"))
                    .cloned()
            })
            .unwrap_or(serde_json::Value::Null);
        assert_eq!(docs["result"], json!("fail"));
        assert_eq!(docs["detail"], json!("docs boom"));
    }

    #[test]
    fn receipt_is_deterministic_for_the_same_diff_and_results() {
        let diff = Diff {
            base: "aaaa".to_string(),
            head: "bbbb".to_string(),
            changed_paths: paths(&["fixtures/x/change.diff", "docs/dogfood/corpus.toml"]),
        };
        let build = || {
            let plan = plan(&diff.changed_paths);
            let outcomes: Vec<(&str, Result<(), String>)> = plan
                .checks
                .iter()
                .filter(|check| check.selected)
                .map(|check| (check.spec.id, Ok(())))
                .collect();
            serde_json::to_string_pretty(&build_receipt(&diff, &plan, &outcomes))
                .unwrap_or_default()
        };
        assert_eq!(build(), build());
    }

    #[test]
    fn args_parse_flags() -> Result<(), String> {
        let raw = paths(&[
            "xtask",
            "check-local",
            "--base",
            "origin/main",
            "--format",
            "json",
            "--out",
            "target/x.json",
        ]);
        let args = Args::parse(&raw)?;
        assert_eq!(args.base.as_deref(), Some("origin/main"));
        assert!(args.format == Format::Json);
        assert_eq!(args.out, Some(PathBuf::from("target/x.json")));
        Ok(())
    }

    #[test]
    fn args_reject_unknown_flag_and_bad_format() {
        assert!(Args::parse(&paths(&["xtask", "check-local", "--nope"])).is_err());
        assert!(Args::parse(&paths(&["xtask", "check-local", "--format", "yaml"])).is_err());
        assert!(Args::parse(&paths(&["xtask", "check-local", "--base"])).is_err());
    }
}
