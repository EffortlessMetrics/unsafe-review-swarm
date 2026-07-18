//! `workflow-pin-sync`: repair immutable-SHA pin drift between workflow YAML
//! `uses:` lines and `policy/workflow-allowlist.toml`.
//!
//! Scope is deliberately narrow: this command only repairs the mechanical case
//! where a workflow's `uses:` ref has moved to a new 40-hex-char immutable SHA
//! while the allowlist still lists an older 40-hex-char SHA for the same
//! action name (`owner/repo`). Anything else -- mutable tag bumps, added or
//! removed actions, an action name that resolves to two different refs on
//! either side -- is reported as drift that needs a human-reviewed PR and is
//! never auto-written. `workflow_allowlist::check_workflow_allowlist` (driven
//! by `check-policy`) remains the actual parity gate; this command is a
//! convenience repair tool for the one case that is safe to automate.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::json;

use crate::{WORKFLOW_ALLOWLIST, WORKFLOW_DIR, read_to_string, workflow_allowlist, workspace_path};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Check,
    Write,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputFormat {
    Human,
    Json,
}

/// Parsed `workflow-pin-sync [--check] [--write] [--format human|json]` arguments.
struct Args {
    mode: Mode,
    format: OutputFormat,
}

impl Args {
    /// `raw[0]` is the xtask binary name and `raw[1]` is "workflow-pin-sync",
    /// mirroring how `dogfood-exec` and `check-local` forward their argv.
    fn parse(raw: &[String]) -> Result<Self, String> {
        let mut mode: Option<Mode> = None;
        let mut format = OutputFormat::Human;
        let mut index = 2;
        while index < raw.len() {
            match raw[index].as_str() {
                "--check" => set_mode(&mut mode, Mode::Check)?,
                "--write" => set_mode(&mut mode, Mode::Write)?,
                "--format" => {
                    let value = raw.get(index + 1).ok_or_else(|| {
                        "workflow-pin-sync: --format requires a value".to_string()
                    })?;
                    format = match value.as_str() {
                        "human" => OutputFormat::Human,
                        "json" => OutputFormat::Json,
                        other => {
                            return Err(format!(
                                "workflow-pin-sync: --format expects `human` or `json`, got `{other}`"
                            ));
                        }
                    };
                    index += 1;
                }
                other => {
                    return Err(format!(
                        "workflow-pin-sync: unexpected argument `{other}`; usage: workflow-pin-sync [--check] [--write] [--format human|json]"
                    ));
                }
            }
            index += 1;
        }
        Ok(Self {
            mode: mode.unwrap_or(Mode::Check),
            format,
        })
    }
}

fn set_mode(mode: &mut Option<Mode>, value: Mode) -> Result<(), String> {
    match mode {
        Some(existing) if *existing != value => {
            Err("workflow-pin-sync: --check and --write are mutually exclusive".to_string())
        }
        Some(_) => Ok(()),
        None => {
            *mode = Some(value);
            Ok(())
        }
    }
}

/// The two action sets (`owner/repo@ref`) compared for one workflow file.
struct WorkflowActionSets {
    allowlist: BTreeSet<String>,
    used: BTreeSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum DriftStatus {
    Repairable,
    NeedsReview,
    Ambiguous,
}

impl DriftStatus {
    fn as_str(self) -> &'static str {
        match self {
            DriftStatus::Repairable => "repairable",
            DriftStatus::NeedsReview => "needs_review",
            DriftStatus::Ambiguous => "ambiguous",
        }
    }
}

/// One drift record: an action pin that differs between the allowlist and the
/// workflow file it governs (or is present on only one side).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Drift {
    workflow: String,
    action: String,
    old_pin: String,
    new_pin: String,
    status: DriftStatus,
    human_review_required: bool,
}

/// A proposed, already-classified SHA-mirror repair to apply to the allowlist.
#[derive(Debug, Clone)]
struct Repair {
    workflow: String,
    old_pin: String,
    new_pin: String,
}

/// Classify pin drift across a set of workflows. Pure: no filesystem access.
/// Workflows that are fully in sync produce no records. The result is sorted
/// by workflow path, then action name (the derived `Ord` field order).
fn classify_drift(workflows: &BTreeMap<String, WorkflowActionSets>) -> Vec<Drift> {
    let mut drifts: Vec<Drift> = workflows
        .iter()
        .flat_map(|(workflow, sets)| classify_workflow(workflow, sets))
        .collect();
    drifts.sort();
    drifts
}

fn classify_workflow(workflow: &str, sets: &WorkflowActionSets) -> Vec<Drift> {
    let allow_by_name = group_by_action_name(&sets.allowlist);
    let used_by_name = group_by_action_name(&sets.used);

    let mut names: BTreeSet<&String> = BTreeSet::new();
    names.extend(allow_by_name.keys());
    names.extend(used_by_name.keys());

    let mut out = Vec::new();
    for name in names {
        let allow_refs = allow_by_name.get(name);
        let used_refs = used_by_name.get(name);

        let ambiguous = allow_refs.is_some_and(|refs| refs.len() > 1)
            || used_refs.is_some_and(|refs| refs.len() > 1);
        if ambiguous {
            out.push(Drift {
                workflow: workflow.to_string(),
                action: name.clone(),
                old_pin: format_ref_set(name, allow_refs),
                new_pin: format_ref_set(name, used_refs),
                status: DriftStatus::Ambiguous,
                human_review_required: true,
            });
            continue;
        }

        let allow_ref = allow_refs
            .and_then(|refs| refs.iter().next())
            .map(String::as_str);
        let used_ref = used_refs
            .and_then(|refs| refs.iter().next())
            .map(String::as_str);

        if let Some(drift) = classify_action(workflow, name, allow_ref, used_ref) {
            out.push(drift);
        }
    }
    out
}

fn classify_action(
    workflow: &str,
    name: &str,
    allow_ref: Option<&str>,
    used_ref: Option<&str>,
) -> Option<Drift> {
    let (Some(allow_ref), Some(used_ref)) = (allow_ref, used_ref) else {
        // Present on only one side: an added or removed action for this workflow.
        return Some(Drift {
            workflow: workflow.to_string(),
            action: name.to_string(),
            old_pin: allow_ref.map(|r| format!("{name}@{r}")).unwrap_or_default(),
            new_pin: used_ref.map(|r| format!("{name}@{r}")).unwrap_or_default(),
            status: DriftStatus::NeedsReview,
            human_review_required: true,
        });
    };

    if allow_ref == used_ref {
        return None;
    }

    let status = if is_immutable_sha(allow_ref) && is_immutable_sha(used_ref) {
        DriftStatus::Repairable
    } else {
        DriftStatus::NeedsReview
    };
    Some(Drift {
        workflow: workflow.to_string(),
        action: name.to_string(),
        old_pin: format!("{name}@{allow_ref}"),
        new_pin: format!("{name}@{used_ref}"),
        human_review_required: status != DriftStatus::Repairable,
        status,
    })
}

/// Group `owner/repo@ref` strings by the action name (everything before the
/// last `@`), keeping every distinct ref seen for that name so ambiguous
/// duplicates (same name, different refs) are visible to the caller.
fn group_by_action_name(actions: &BTreeSet<String>) -> BTreeMap<String, BTreeSet<String>> {
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for full in actions {
        if let Some((name, reference)) = full.rsplit_once('@') {
            out.entry(name.to_string())
                .or_default()
                .insert(reference.to_string());
        }
    }
    out
}

fn format_ref_set(name: &str, refs: Option<&BTreeSet<String>>) -> String {
    match refs {
        None => String::new(),
        Some(set) => set
            .iter()
            .map(|reference| format!("{name}@{reference}"))
            .collect::<Vec<_>>()
            .join(","),
    }
}

/// Exactly 40 lowercase hex characters, the shape of a Git commit SHA.
fn is_immutable_sha(reference: &str) -> bool {
    reference.len() == 40
        && reference
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Apply only the given repairs to `toml_text`, editing exclusively inside the
/// `[[workflow]]` block that matches each repair's `workflow` path, and
/// replacing only the exact quoted `old_pin` string with `new_pin`. All other
/// bytes (formatting, key order, comments, unrelated blocks) are preserved
/// verbatim. Idempotent: a repair whose `new_pin` is already present in the
/// matching block is a no-op.
fn apply_repairs(toml_text: &str, repairs: &[Repair]) -> Result<String, String> {
    let blocks = split_workflow_blocks(toml_text);
    let mut updated: Vec<String> = blocks.iter().map(|block| block.text.clone()).collect();

    for repair in repairs {
        let old_needle = format!("\"{}\"", repair.old_pin);
        let new_needle = format!("\"{}\"", repair.new_pin);

        let block_idx = blocks
            .iter()
            .position(|block| block.path.as_deref() == Some(repair.workflow.as_str()))
            .ok_or_else(|| {
                format!(
                    "workflow-pin-sync: no [[workflow]] block for `{}` found while applying repair",
                    repair.workflow
                )
            })?;

        let block_text = &mut updated[block_idx];
        if block_text.contains(&new_needle) {
            // Already applied (e.g. a repeat run): nothing to do for this repair.
            continue;
        }
        if !block_text.contains(&old_needle) {
            return Err(format!(
                "workflow-pin-sync: expected pin `{}` not found in the `{}` block; refusing to write (drift may have changed since classification)",
                repair.old_pin, repair.workflow
            ));
        }
        *block_text = block_text.replacen(&old_needle, &new_needle, 1);
    }

    Ok(updated.concat())
}

struct TomlBlock {
    text: String,
    path: Option<String>,
}

/// Split TOML text into `[[workflow]]`-delimited blocks, byte-exact (every
/// original character, including line endings, lands in exactly one block so
/// re-joining unmodified blocks reproduces the input verbatim).
fn split_workflow_blocks(toml_text: &str) -> Vec<TomlBlock> {
    let mut blocks = Vec::new();
    let mut current = String::new();

    for line in split_keep_newline(toml_text) {
        if line.trim() == "[[workflow]]" && !current.is_empty() {
            blocks.push(finish_block(current));
            current = String::new();
        }
        current.push_str(line);
    }
    if !current.is_empty() {
        blocks.push(finish_block(current));
    }
    blocks
}

fn finish_block(text: String) -> TomlBlock {
    let path = text.lines().find_map(|line| {
        let rest = line.trim().strip_prefix("path")?;
        let rest = rest.trim_start().strip_prefix('=')?;
        extract_quoted(rest.trim())
    });
    TomlBlock { text, path }
}

fn extract_quoted(value: &str) -> Option<String> {
    let inner = value.trim().strip_prefix('"')?;
    let end = inner.find('"')?;
    Some(inner[..end].to_string())
}

/// Split `text` into lines that retain their trailing `\n` (and any preceding
/// `\r`), so concatenating every returned slice reproduces `text` exactly.
fn split_keep_newline(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    for (idx, byte) in text.bytes().enumerate() {
        if byte == b'\n' {
            out.push(&text[start..=idx]);
            start = idx + 1;
        }
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}

fn workflow_yaml_files(workflow_dir: &Path) -> Result<BTreeSet<String>, String> {
    let dir = workspace_path(&workflow_dir.display().to_string());
    let read_dir = std::fs::read_dir(&dir)
        .map_err(|err| format!("workflow-pin-sync: read {} failed: {err}", dir.display()))?;
    let mut files = BTreeSet::new();
    for entry in read_dir {
        let entry =
            entry.map_err(|err| format!("workflow-pin-sync: read_dir entry failed: {err}"))?;
        let path = entry.path();
        let extension = path.extension().and_then(std::ffi::OsStr::to_str);
        if !matches!(extension, Some("yml" | "yaml")) {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(std::ffi::OsStr::to_str) else {
            return Err(format!(
                "workflow-pin-sync: non-UTF-8 workflow file name: {}",
                path.display()
            ));
        };
        files.insert(format!("{WORKFLOW_DIR}/{file_name}"));
    }
    Ok(files)
}

/// Entry point for the `workflow-pin-sync` xtask command.
pub(crate) fn run(raw: &[String]) -> Result<(), String> {
    let args = Args::parse(raw)?;

    let allowlist_path = workspace_path(WORKFLOW_ALLOWLIST);
    let entries = workflow_allowlist::workflow_policy_entries(&allowlist_path)?;

    let mut workflows: BTreeMap<String, WorkflowActionSets> = BTreeMap::new();
    for entry in &entries {
        let workflow_path = workspace_path(&entry.path);
        if !workflow_path.is_file() {
            return Err(format!(
                "workflow-pin-sync: {WORKFLOW_ALLOWLIST} lists missing workflow `{}`",
                entry.path
            ));
        }
        let text = read_to_string(&workflow_path)?;
        let used = workflow_allowlist::workflow_used_actions(&text);
        if workflows
            .insert(
                entry.path.clone(),
                WorkflowActionSets {
                    allowlist: entry.actions.clone(),
                    used,
                },
            )
            .is_some()
        {
            return Err(format!(
                "workflow-pin-sync: {WORKFLOW_ALLOWLIST} contains duplicate workflow entry `{}`",
                entry.path
            ));
        }
    }

    for file in workflow_yaml_files(Path::new(WORKFLOW_DIR))? {
        if !workflows.contains_key(&file) {
            return Err(format!(
                "workflow-pin-sync: {WORKFLOW_ALLOWLIST} is missing a workflow allowlist entry for `{file}`; run `cargo run --locked -p xtask -- check-policy` for full parity details"
            ));
        }
    }

    let drifts = classify_drift(&workflows);

    match args.mode {
        Mode::Check => run_check(&drifts, args.format),
        Mode::Write => run_write(&allowlist_path, &drifts, args.format),
    }
}

fn run_check(drifts: &[Drift], format: OutputFormat) -> Result<(), String> {
    match format {
        OutputFormat::Json => print_json(drifts)?,
        OutputFormat::Human => print_human_drifts(drifts),
    }

    if drifts.is_empty() {
        if matches!(format, OutputFormat::Human) {
            println!("workflow-pin-sync: ok, allowlist pins match workflow files");
        }
        Ok(())
    } else {
        Err(drift_found_error(drifts))
    }
}

fn run_write(allowlist_path: &Path, drifts: &[Drift], format: OutputFormat) -> Result<(), String> {
    match format {
        OutputFormat::Json => print_json(drifts)?,
        OutputFormat::Human => print_human_drifts(drifts),
    }

    let repairs: Vec<Repair> = drifts
        .iter()
        .filter(|drift| drift.status == DriftStatus::Repairable)
        .map(|drift| Repair {
            workflow: drift.workflow.clone(),
            old_pin: drift.old_pin.clone(),
            new_pin: drift.new_pin.clone(),
        })
        .collect();
    let remaining: Vec<&Drift> = drifts
        .iter()
        .filter(|drift| drift.status != DriftStatus::Repairable)
        .collect();

    if !repairs.is_empty() {
        let original = read_to_string(allowlist_path)?;
        let updated = apply_repairs(&original, &repairs)?;
        if updated != original {
            std::fs::write(allowlist_path, &updated).map_err(|err| {
                format!(
                    "workflow-pin-sync: failed to write {}: {err}",
                    allowlist_path.display()
                )
            })?;
            if matches!(format, OutputFormat::Human) {
                println!(
                    "workflow-pin-sync: wrote {} repaired pin(s) to {}",
                    repairs.len(),
                    allowlist_path.display()
                );
                for repair in &repairs {
                    println!(
                        "  {}: {} -> {}",
                        repair.workflow, repair.old_pin, repair.new_pin
                    );
                }
            }
        } else if matches!(format, OutputFormat::Human) {
            println!(
                "workflow-pin-sync: {} pin(s) already applied (no-op)",
                repairs.len()
            );
        }
    } else if remaining.is_empty() && matches!(format, OutputFormat::Human) {
        println!("workflow-pin-sync: ok, allowlist pins match workflow files; nothing to write");
    }

    if remaining.is_empty() {
        Ok(())
    } else {
        Err(non_repairable_error(&remaining))
    }
}

fn drift_found_error(drifts: &[Drift]) -> String {
    format!(
        "workflow-pin-sync: {} pin drift record(s) found; run `cargo run --locked -p xtask -- workflow-pin-sync --write` to repair the SHA-only bumps, then send the rest through a normal reviewed PR:\n{}",
        drifts.len(),
        render_drift_lines(drifts),
    )
}

fn non_repairable_error(remaining: &[&Drift]) -> String {
    format!(
        "workflow-pin-sync: {} drift record(s) require a normal reviewed PR (not auto-written):\n{}",
        remaining.len(),
        remaining
            .iter()
            .map(|drift| drift_line(drift))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

fn render_drift_lines(drifts: &[Drift]) -> String {
    drifts.iter().map(drift_line).collect::<Vec<_>>().join("\n")
}

fn drift_line(drift: &Drift) -> String {
    format!(
        "  {}: {}: {} -> {} [{}]",
        drift.workflow,
        drift.action,
        pin_display(&drift.old_pin),
        pin_display(&drift.new_pin),
        drift.status.as_str(),
    )
}

fn pin_display(pin: &str) -> &str {
    if pin.is_empty() { "<absent>" } else { pin }
}

fn print_human_drifts(drifts: &[Drift]) {
    for drift in drifts {
        println!("{}", drift_line(drift));
    }
}

fn print_json(drifts: &[Drift]) -> Result<(), String> {
    let value: Vec<serde_json::Value> = drifts
        .iter()
        .map(|drift| {
            json!({
                "workflow": drift.workflow,
                "action": drift.action,
                "old_pin": drift.old_pin,
                "new_pin": drift.new_pin,
                "status": drift.status.as_str(),
                "human_review_required": drift.human_review_required,
            })
        })
        .collect();
    let text = serde_json::to_string_pretty(&value)
        .map_err(|err| format!("workflow-pin-sync: failed to serialize drift report: {err}"))?;
    println!("{text}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sha(fill: char) -> String {
        fill.to_string().repeat(40)
    }

    fn sha_pin(name: &str, fill: char) -> String {
        format!("{name}@{}", sha(fill))
    }

    fn one_action_sets(name: &str, allow_pin: &str, used_pin: &str) -> WorkflowActionSets {
        WorkflowActionSets {
            allowlist: BTreeSet::from([format!("{name}@{allow_pin}")]),
            used: BTreeSet::from([format!("{name}@{used_pin}")]),
        }
    }

    fn sample_allowlist_toml(entries: &[(&str, &[&str])]) -> String {
        let mut out = String::from("schema_version = \"1\"\n\n");
        for (path, actions) in entries {
            out.push_str("[[workflow]]\n");
            out.push_str(&format!("path = \"{path}\"\n"));
            out.push_str("permissions = \"contents: read\"\n");
            out.push_str("actions = [\n");
            for action in *actions {
                out.push_str(&format!("  \"{action}\",\n"));
            }
            out.push_str("]\n");
            out.push_str("reason = \"pinned action for supply-chain integrity\"\n");
            out.push_str("created = \"2026-01-01\"\n");
            out.push_str("review_after = \"2026-12-31\"\n\n");
        }
        out
    }

    // (a) single SHA-only bump -> one repairable one-line repair.
    #[test]
    fn classify_drift_detects_single_sha_bump_as_repairable() {
        let mut workflows = BTreeMap::new();
        workflows.insert(
            ".github/workflows/ci.yml".to_string(),
            one_action_sets("owner/repo", &sha('a'), &sha('b')),
        );

        let drifts = classify_drift(&workflows);

        assert_eq!(drifts.len(), 1);
        assert_eq!(drifts[0].workflow, ".github/workflows/ci.yml");
        assert_eq!(drifts[0].action, "owner/repo");
        assert_eq!(drifts[0].old_pin, sha_pin("owner/repo", 'a'));
        assert_eq!(drifts[0].new_pin, sha_pin("owner/repo", 'b'));
        assert_eq!(drifts[0].status, DriftStatus::Repairable);
        assert!(!drifts[0].human_review_required);
    }

    // (b) multiple independent SHA bumps -> deterministic ordering by workflow
    // path, then action name.
    #[test]
    fn classify_drift_orders_multiple_bumps_deterministically() {
        let mut workflows = BTreeMap::new();
        workflows.insert(
            "z.yml".to_string(),
            one_action_sets("zzz/action", &sha('1'), &sha('2')),
        );
        workflows.insert(
            "a.yml".to_string(),
            one_action_sets("aaa/action", &sha('1'), &sha('2')),
        );

        let drifts = classify_drift(&workflows);

        assert_eq!(drifts.len(), 2);
        assert_eq!(drifts[0].workflow, "a.yml");
        assert_eq!(drifts[1].workflow, "z.yml");

        // Re-running classification must reproduce the exact same order.
        let drifts_again = classify_drift(&workflows);
        assert_eq!(drifts, drifts_again);
    }

    // (c) permission/action-set/version-tag change -> needs_review, never
    // auto-written (status != Repairable).
    #[test]
    fn classify_drift_flags_action_set_and_tag_changes_as_needs_review() {
        let mut workflows = BTreeMap::new();
        // Mutable-tag bump: v6 -> v7, not a SHA on either side.
        workflows.insert(
            "tag.yml".to_string(),
            one_action_sets("actions/checkout", "v6", "v7"),
        );
        // Action-set change: allowlist lists an action the workflow no longer uses.
        workflows.insert(
            "set.yml".to_string(),
            WorkflowActionSets {
                allowlist: BTreeSet::from([
                    sha_pin("actions/checkout", 'a'),
                    sha_pin("actions/setup-go", 'a'),
                ]),
                used: BTreeSet::from([sha_pin("actions/checkout", 'a')]),
            },
        );

        let drifts = classify_drift(&workflows);

        assert_eq!(drifts.len(), 2);
        for drift in &drifts {
            assert_eq!(drift.status, DriftStatus::NeedsReview);
            assert!(drift.human_review_required);
        }
    }

    // (d) ambiguous duplicate action name -> fail closed, never repairable.
    #[test]
    fn classify_drift_flags_ambiguous_duplicate_names() {
        let mut workflows = BTreeMap::new();
        workflows.insert(
            "dup.yml".to_string(),
            WorkflowActionSets {
                allowlist: BTreeSet::from([sha_pin("owner/repo", 'a'), sha_pin("owner/repo", 'b')]),
                used: BTreeSet::from([sha_pin("owner/repo", 'a')]),
            },
        );

        let drifts = classify_drift(&workflows);

        assert_eq!(drifts.len(), 1);
        assert_eq!(drifts[0].status, DriftStatus::Ambiguous);
        assert!(drifts[0].human_review_required);
        assert_ne!(drifts[0].status, DriftStatus::Repairable);
    }

    // (e) mutable-tag/non-SHA new ref -> needs_review, even when the old ref
    // was a SHA.
    #[test]
    fn classify_drift_flags_non_sha_new_ref_as_needs_review() {
        let mut workflows = BTreeMap::new();
        workflows.insert(
            "branch.yml".to_string(),
            one_action_sets("owner/repo", &sha('a'), "main"),
        );

        let drifts = classify_drift(&workflows);

        assert_eq!(drifts.len(), 1);
        assert_eq!(drifts[0].status, DriftStatus::NeedsReview);
        assert!(drifts[0].human_review_required);
    }

    // In-sync actions never produce a drift record.
    #[test]
    fn classify_drift_is_silent_when_pins_match() {
        let mut workflows = BTreeMap::new();
        workflows.insert(
            "ok.yml".to_string(),
            one_action_sets("owner/repo", &sha('a'), &sha('a')),
        );

        assert!(classify_drift(&workflows).is_empty());
    }

    // (f) apply_repairs is idempotent: applying the same repair twice only
    // changes the text once.
    #[test]
    fn apply_repairs_is_idempotent() -> Result<(), String> {
        let old_pin = sha_pin("owner/repo", 'a');
        let new_pin = sha_pin("owner/repo", 'b');
        let toml_text = sample_allowlist_toml(&[(".github/workflows/ci.yml", &[old_pin.as_str()])]);
        let repair = Repair {
            workflow: ".github/workflows/ci.yml".to_string(),
            old_pin: old_pin.clone(),
            new_pin: new_pin.clone(),
        };

        let once = apply_repairs(&toml_text, std::slice::from_ref(&repair))?;
        assert!(once.contains(&new_pin));
        assert!(!once.contains(&old_pin));

        let twice = apply_repairs(&once, std::slice::from_ref(&repair))?;
        assert_eq!(twice, once, "second application must be a no-op");
        Ok(())
    }

    // apply_repairs with no repairs reproduces the input verbatim.
    #[test]
    fn apply_repairs_with_no_repairs_is_a_no_op() -> Result<(), String> {
        let toml_text = sample_allowlist_toml(&[(
            ".github/workflows/ci.yml",
            &[sha_pin("owner/repo", 'a').as_str()],
        )]);

        let unchanged = apply_repairs(&toml_text, &[])?;
        assert_eq!(unchanged, toml_text);
        Ok(())
    }

    // (g) a targeted write only touches the block for the named workflow, even
    // when another block contains a pin for the same action name.
    #[test]
    fn apply_repairs_only_touches_the_matching_workflow_block() -> Result<(), String> {
        let old_pin_ci = sha_pin("actions/checkout", 'a');
        let new_pin_ci = sha_pin("actions/checkout", 'b');
        let pin_release = sha_pin("actions/checkout", 'c');

        let toml_text = sample_allowlist_toml(&[
            (".github/workflows/ci.yml", &[old_pin_ci.as_str()]),
            (".github/workflows/release.yml", &[pin_release.as_str()]),
        ]);

        let repair = Repair {
            workflow: ".github/workflows/ci.yml".to_string(),
            old_pin: old_pin_ci.clone(),
            new_pin: new_pin_ci.clone(),
        };

        let updated = apply_repairs(&toml_text, &[repair])?;

        assert!(updated.contains(&new_pin_ci));
        assert!(!updated.contains(&old_pin_ci));
        assert!(
            updated.contains(&pin_release),
            "unrelated block's pin must be untouched"
        );
        Ok(())
    }

    #[test]
    fn apply_repairs_fails_closed_when_expected_pin_is_absent() {
        let toml_text = sample_allowlist_toml(&[(
            ".github/workflows/ci.yml",
            &[sha_pin("owner/repo", 'z').as_str()],
        )]);
        let repair = Repair {
            workflow: ".github/workflows/ci.yml".to_string(),
            old_pin: sha_pin("owner/repo", 'a'),
            new_pin: sha_pin("owner/repo", 'b'),
        };

        let err = apply_repairs(&toml_text, &[repair])
            .err()
            .unwrap_or_default();
        assert!(err.contains("not found"), "unexpected error: {err}");
    }

    #[test]
    fn apply_repairs_fails_closed_when_workflow_block_is_missing() {
        let toml_text = sample_allowlist_toml(&[(
            ".github/workflows/ci.yml",
            &[sha_pin("owner/repo", 'a').as_str()],
        )]);
        let repair = Repair {
            workflow: ".github/workflows/missing.yml".to_string(),
            old_pin: sha_pin("owner/repo", 'a'),
            new_pin: sha_pin("owner/repo", 'b'),
        };

        let err = apply_repairs(&toml_text, &[repair])
            .err()
            .unwrap_or_default();
        assert!(
            err.contains("no [[workflow]] block"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn args_parse_defaults_to_check_mode_and_human_format() -> Result<(), String> {
        let raw = vec!["xtask".to_string(), "workflow-pin-sync".to_string()];
        let args = Args::parse(&raw)?;
        assert_eq!(args.mode, Mode::Check);
        assert_eq!(args.format, OutputFormat::Human);
        Ok(())
    }

    #[test]
    fn args_parse_accepts_write_and_json_format() -> Result<(), String> {
        let raw = vec![
            "xtask".to_string(),
            "workflow-pin-sync".to_string(),
            "--write".to_string(),
            "--format".to_string(),
            "json".to_string(),
        ];
        let args = Args::parse(&raw)?;
        assert_eq!(args.mode, Mode::Write);
        assert_eq!(args.format, OutputFormat::Json);
        Ok(())
    }

    #[test]
    fn args_parse_rejects_check_and_write_together() {
        let raw = vec![
            "xtask".to_string(),
            "workflow-pin-sync".to_string(),
            "--check".to_string(),
            "--write".to_string(),
        ];
        let err = Args::parse(&raw).err().unwrap_or_default();
        assert!(
            err.contains("mutually exclusive"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn args_parse_rejects_unknown_flag() {
        let raw = vec![
            "xtask".to_string(),
            "workflow-pin-sync".to_string(),
            "--bogus".to_string(),
        ];
        let err = Args::parse(&raw).err().unwrap_or_default();
        assert!(err.contains("--bogus"), "unexpected error: {err}");
    }

    #[test]
    fn args_parse_rejects_unknown_format_value() {
        let raw = vec![
            "xtask".to_string(),
            "workflow-pin-sync".to_string(),
            "--format".to_string(),
            "yaml".to_string(),
        ];
        let err = Args::parse(&raw).err().unwrap_or_default();
        assert!(err.contains("--format"), "unexpected error: {err}");
    }

    #[test]
    fn is_immutable_sha_requires_exactly_40_lowercase_hex_chars() {
        assert!(is_immutable_sha(&sha('a')));
        assert!(!is_immutable_sha("v7"));
        assert!(!is_immutable_sha("main"));
        assert!(!is_immutable_sha(&"A".repeat(40)));
        assert!(!is_immutable_sha(&"a".repeat(39)));
    }
}
