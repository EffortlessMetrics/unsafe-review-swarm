//! Issue #2248: a recorded adverse/uncertain observation must not retire review work.
//! These tests scan an owned copy of an existing fixture and import synthetic
//! receipts. They do not run the fixture, Miri, or any other witness tool.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use unsafe_review_core::{
    AnalysisMode, AnalyzeInput, AnalyzeOutput, DiffSource, PolicyMode, ReviewCard, Scope,
    analyze, compare_outcome_json, render_json,
};

static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);

struct FixtureRoot(PathBuf);

impl FixtureRoot {
    fn new() -> Result<Self, String> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("clock before epoch: {error}"))?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "unsafe-review-2248-{}-{nonce}-{}",
            std::process::id(),
            NEXT_ROOT.fetch_add(1, Ordering::Relaxed)
        ));
        // Atomic creation: never reuse or delete a directory owned by another test.
        fs::create_dir(&root).map_err(|error| format!("create fixture root: {error}"))?;
        let owned = Self(root);
        fs::create_dir(owned.0.join("src"))
            .map_err(|error| format!("create fixture source directory: {error}"))?;
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/box_from_raw_box_origin");
        for relative in ["Cargo.toml", "change.diff", "src/lib.rs"] {
            fs::copy(source.join(relative), owned.0.join(relative))
                .map_err(|error| format!("copy fixture {relative}: {error}"))?;
        }
        Ok(owned)
    }
}

impl Drop for FixtureRoot {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!(
                "cannot remove owned #2248 fixture {}: {error}",
                self.0.display()
            );
        }
    }
}

struct Observation {
    // Keep the isolated input alive while renderers inspect its receipt directory.
    _root: FixtureRoot,
    id: String,
    before: AnalyzeOutput,
    after: AnalyzeOutput,
    before_json: String,
    after_json: String,
}

fn analyze_root(root: &Path) -> Result<AnalyzeOutput, String> {
    analyze(AnalyzeInput {
        root: root.to_path_buf(),
        scope: Scope::Diff,
        diff: DiffSource::File(root.join("change.diff")),
        mode: AnalysisMode::Draft,
        policy: PolicyMode::Advisory,
        include_unchanged_tests: true,
        max_cards: None,
    })
}

fn selected_card<'a>(output: &'a AnalyzeOutput, id: &str) -> Result<&'a ReviewCard, String> {
    output
        .cards
        .iter()
        .find(|card| card.id.0 == id)
        .ok_or_else(|| format!("selected card {id} is absent"))
}

fn observe(tool: &str, verdict: Option<&str>) -> Result<Observation, String> {
    let root = FixtureRoot::new()?;
    let before = analyze_root(&root.0)?;
    let mut candidates = before.cards.iter().filter(|card| {
        card.operation.family.as_str() == "box_from_raw"
            && card.site.owner.as_deref() == Some("round_trip_box")
    });
    let original = candidates
        .next()
        .ok_or_else(|| "expected the Box-origin operation card".to_string())?;
    if candidates.next().is_some() {
        return Err("fixture has multiple matching Box-origin operation cards".to_string());
    }
    if original.class.as_str() != "guarded_unwitnessed" || original.witness.present {
        return Err(format!(
            "unexpected fixture baseline: {} / witness={}",
            original.class.as_str(),
            original.witness.present
        ));
    }
    let id = original.id.0.clone();
    // Serialize before adding the receipt; do not let a later directory read
    // contaminate the pre-import snapshot.
    let before_json = render_json(&before);
    let directory = root.0.join(".unsafe-review/receipts");
    fs::create_dir_all(&directory)
        .map_err(|error| format!("create receipt directory: {error}"))?;
    let mut receipt = serde_json::json!({
        "schema_version": "0.1",
        "card_id": id,
        "tool": tool,
        "strength": "ran",
        "author": "controlled-regression/2248",
        "recorded_at": "2025-12-18T00:00:00Z",
        "expires_at": "2099-12-31",
        "summary": "Synthetic saved observation for a classification regression",
        "limitations": ["Synthetic fixture receipt; no witness tool was executed by this test"]
    });
    if let Some(value) = verdict {
        receipt["verdict"] = serde_json::Value::String(value.to_string());
    }
    let bytes = serde_json::to_vec_pretty(&receipt)
        .map_err(|error| format!("serialize receipt: {error}"))?;
    fs::write(directory.join("observation.json"), bytes)
        .map_err(|error| format!("write receipt: {error}"))?;
    let after = analyze_root(&root.0)?;
    selected_card(&after, &id)?;
    let after_json = render_json(&after);
    Ok(Observation {
        _root: root,
        id,
        before,
        after,
        before_json,
        after_json,
    })
}

fn require_outstanding_work(verdict: &str) -> Result<(), String> {
    let observation = observe("miri", Some(verdict))?;
    let before = selected_card(&observation.before, &observation.id)?;
    let after = selected_card(&observation.after, &observation.id)?;
    let mut failures = Vec::new();
    if !after.witness.present || after.witness.verdict.as_deref() != Some(verdict) {
        failures.push("the imported observation was discarded or changed".to_string());
    }
    if !after.class.is_actionable() {
        failures.push(format!("review work retired into {}", after.class.as_str()));
    }
    if after.class.sarif_level() == "none" || after.class.lsp_severity() == 4 {
        failures.push("canonical severity treats remaining work as closed".to_string());
    }
    if matches!(
        (before.priority.as_str(), after.priority.as_str()),
        ("high", "medium" | "low") | ("medium", "low")
    ) {
        failures.push("adverse/uncertain receipt lowered the review priority".to_string());
    }
    if observation.after.summary.open_actionable_gaps
        < observation.before.summary.open_actionable_gaps
    {
        failures.push("the open-action count fell without repairing the source".to_string());
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!("{verdict}: {}", failures.join("; ")))
    }
}

#[test]
fn confirmed_observation_keeps_review_work_open() -> Result<(), String> {
    require_outstanding_work("confirmed")
}

#[test]
fn inconclusive_observation_keeps_review_work_open() -> Result<(), String> {
    require_outstanding_work("inconclusive")
}

#[test]
fn adverse_observations_remain_in_outcome_work() -> Result<(), String> {
    let mut failures = Vec::new();
    for verdict in ["confirmed", "inconclusive"] {
        let observation = observe("miri", Some(verdict))?;
        let report = compare_outcome_json(&observation.before_json, &observation.after_json)?;
        if report.after.open_actionable_gaps < report.before.open_actionable_gaps {
            failures.push(format!("{verdict}: outcome retires the outstanding action"));
        }
        if !report
            .reviewer_delta
            .top_remaining_gaps
            .iter()
            .any(|gap| gap.card_id == observation.id)
        {
            failures.push(format!("{verdict}: selected action absent from remaining work"));
        }
        if report
            .cards
            .resolved
            .iter()
            .any(|card| card.card_id == observation.id)
        {
            failures.push(format!("{verdict}: unchanged operation reported resolved"));
        }
        // Stronger evidence may improve knowledge while a defect remains open.
        // Do not prohibit every `improved` label or force a particular new class.
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

#[test]
fn non_adverse_receipt_controls_preserve_their_observations() -> Result<(), String> {
    for verdict in [None, Some("not_reproduced")] {
        let observation = observe("miri", verdict)?;
        let after = selected_card(&observation.after, &observation.id)?;
        if !after.witness.present || after.witness.verdict.as_deref() != verdict {
            return Err(format!("supported receipt control changed: {verdict:?}"));
        }
    }
    Ok(())
}

#[test]
fn wrong_tool_receipt_does_not_clear_work() -> Result<(), String> {
    let observation = observe("loom", Some("inconclusive"))?;
    let after = selected_card(&observation.after, &observation.id)?;
    if after.witness.present || !after.class.is_actionable() {
        return Err("an unrouted tool receipt cleared the witness-review action".to_string());
    }
    Ok(())
}
