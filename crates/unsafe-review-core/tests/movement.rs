//! #2271 PR1: base/head movement acceptance matrix.
//!
//! A small owned two-file crate is analyzed as base and as head through the
//! real analyzer; [`evaluate_base_head_movement`] classifies each canonical
//! subject. No saved baseline ledger is involved: an untouched card must
//! report inherited context, not a PR introduction.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use unsafe_review_core::{
    AnalysisMode, AnalyzeInput, AnalyzeOutput, DiffSource, PolicyMode, Scope, SubjectMovement,
    analyze, evaluate_base_head_movement,
};

fn unique_root(prefix: &str) -> Result<PathBuf, String> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("clock before UNIX epoch: {error}"))?
        .as_nanos();
    let root = std::env::temp_dir().join(format!("unsafe-review-movement-{prefix}-{nanos}"));
    fs::create_dir_all(root.join("src"))
        .map_err(|error| format!("create fixture dirs failed: {error}"))?;
    Ok(root)
}

fn write_crate(root: &Path, lib_rs: &str) -> Result<(), String> {
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"movement-fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    )
    .map_err(|error| format!("write manifest failed: {error}"))?;
    fs::write(root.join("src/lib.rs"), lib_rs)
        .map_err(|error| format!("write source failed: {error}"))?;
    Ok(())
}

fn analyze_repo(root: &Path, max_cards: Option<usize>) -> Result<AnalyzeOutput, String> {
    analyze(AnalyzeInput {
        root: root.to_path_buf(),
        scope: Scope::Repo,
        diff: DiffSource::NoneRepoScan,
        mode: AnalysisMode::Repo,
        policy: PolicyMode::Advisory,
        include_unchanged_tests: true,
        max_cards,
    })
}

fn analyze_diff(root: &Path, diff_name: &str) -> Result<AnalyzeOutput, String> {
    analyze(AnalyzeInput {
        root: root.to_path_buf(),
        scope: Scope::Diff,
        diff: DiffSource::File(root.join(diff_name)),
        mode: AnalysisMode::Draft,
        policy: PolicyMode::Advisory,
        include_unchanged_tests: true,
        max_cards: None,
    })
}

fn movement_for(
    output: &AnalyzeOutput,
    base: &AnalyzeOutput,
    owner: &str,
) -> Result<SubjectMovement, String> {
    let report = evaluate_base_head_movement(base, output);
    report
        .subjects
        .iter()
        .find(|entry| {
            entry
                .head_card_id
                .as_deref()
                .or(entry.base_card_id.as_deref())
                .is_some_and(|id| id.contains(owner))
        })
        .map(|entry| entry.movement)
        .ok_or_else(|| format!("no subject for owner `{owner}`"))
}

fn assert_reconciled(report: &unsafe_review_core::MovementReport) {
    let summary = &report.summary;
    assert_eq!(
        summary.introduced
            + summary.worsened
            + summary.improved
            + summary.resolved
            + summary.inherited
            + summary.unknown,
        summary.subjects,
        "movement buckets must reconcile exactly with the subject count"
    );
    assert_eq!(
        summary.subjects,
        report.subjects.len(),
        "summary subject count must match the per-card entries"
    );
    for entry in &report.subjects {
        assert!(
            !entry.reason.trim().is_empty(),
            "every verdict needs its reason"
        );
        assert!(
            !entry.ledger_baseline_state.trim().is_empty()
                && !entry.ledger_outcome_movement.trim().is_empty(),
            "ledger posture must be recorded side by side"
        );
    }
}

const BASE_LIB: &str = r#"pub struct Header(u32);

pub fn read_header(ptr: *const u8) -> Header {
    unsafe { ptr.cast::<Header>().read() }
}

pub fn peek_word(ptr: *const u32) -> u32 {
    unsafe { *ptr }
}
"#;

const GUARDED_LIB: &str = r#"pub struct Header(u32);

pub fn read_header(ptr: *const u8) -> Header {
    unsafe { ptr.cast::<Header>().read() }
}

pub fn peek_word(ptr: *const u32) -> u32 {
    assert!((ptr as usize) % core::mem::align_of::<u32>() == 0);
    unsafe { *ptr }
}
"#;

fn fixture_pair(head_lib: &str) -> Result<(PathBuf, PathBuf), String> {
    let base = unique_root("base")?;
    let head = unique_root("head")?;
    write_crate(&base, BASE_LIB)?;
    write_crate(&head, head_lib)?;
    Ok((base, head))
}

fn cleanup(root: &Path) -> Result<(), String> {
    fs::remove_dir_all(root).map_err(|error| format!("remove temp root failed: {error}"))
}

#[test]
fn unchanged_cards_are_inherited_context_without_any_baseline() -> Result<(), String> {
    let (base_root, head_root) = fixture_pair(BASE_LIB)?;
    let base = analyze_repo(&base_root, None)?;
    let head = analyze_repo(&head_root, None)?;
    assert!(!base.cards.is_empty(), "fixture must produce cards");
    let report = evaluate_base_head_movement(&base, &head);
    for entry in &report.subjects {
        assert_eq!(
            entry.movement,
            SubjectMovement::Inherited,
            "unchanged subject must be inherited context, not introduced: {}",
            entry.subject
        );
    }
    assert_eq!(report.summary.inherited, report.summary.subjects);
    assert_reconciled(&report);
    cleanup(&base_root)?;
    cleanup(&head_root)?;
    Ok(())
}

#[test]
fn line_shift_does_not_break_subject_matching() -> Result<(), String> {
    // Prepend comment lines: every line number moves, but subjects stay.
    let shifted = format!("// movement probe header\n// second line\n{BASE_LIB}");
    let (base_root, head_root) = fixture_pair(&shifted)?;
    let base = analyze_repo(&base_root, None)?;
    let head = analyze_repo(&head_root, None)?;
    let report = evaluate_base_head_movement(&base, &head);
    assert!(!report.subjects.is_empty());
    for entry in &report.subjects {
        assert_eq!(
            entry.movement,
            SubjectMovement::Inherited,
            "line-shifted subject must still match: {}",
            entry.subject
        );
    }
    assert_reconciled(&report);
    cleanup(&base_root)?;
    cleanup(&head_root)?;
    Ok(())
}

#[test]
fn new_operation_is_introduced_while_context_stays_inherited() -> Result<(), String> {
    let head_lib = format!(
        "{BASE_LIB}\n/// # Safety\n///\n/// Caller guarantees `ptr` is valid.\npub fn peek_byte(ptr: *const u8) -> u8 {{\n    unsafe {{ *ptr }}\n}}\n"
    );
    let (base_root, head_root) = fixture_pair(&head_lib)?;
    let base = analyze_repo(&base_root, None)?;
    let head = analyze_repo(&head_root, None)?;
    let report = evaluate_base_head_movement(&base, &head);
    assert_eq!(
        movement_for(&head, &base, "peek-byte")?,
        SubjectMovement::Introduced
    );
    assert_eq!(
        movement_for(&head, &base, "read-header")?,
        SubjectMovement::Inherited
    );
    assert_eq!(
        movement_for(&head, &base, "peek-word")?,
        SubjectMovement::Inherited
    );
    assert_eq!(report.summary.introduced, 1);
    assert_reconciled(&report);
    cleanup(&base_root)?;
    cleanup(&head_root)?;
    Ok(())
}

#[test]
fn added_guard_is_improved_without_resolving() -> Result<(), String> {
    // peek-word gains an alignment guard; the unsafe operation stays.
    let base_root = unique_root("guard-base")?;
    let head_root = unique_root("guard-head")?;
    write_crate(&base_root, BASE_LIB)?;
    write_crate(&head_root, GUARDED_LIB)?;
    let base = analyze_repo(&base_root, None)?;
    let head = analyze_repo(&head_root, None)?;
    let report = evaluate_base_head_movement(&base, &head);
    let entry = report
        .subjects
        .iter()
        .find(|entry| {
            entry
                .head_card_id
                .as_deref()
                .is_some_and(|id| id.contains("peek-word"))
        })
        .ok_or_else(|| "peek-word subject missing".to_string())?;
    assert_eq!(entry.movement, SubjectMovement::Improved);
    let card = head
        .cards
        .iter()
        .find(|card| card.id.0.contains("peek-word"))
        .ok_or_else(|| "peek-word card missing".to_string())?;
    assert!(
        card.class.is_actionable(),
        "improved evidence must keep the gap open"
    );
    assert_eq!(
        movement_for(&head, &base, "read-header")?,
        SubjectMovement::Inherited
    );
    assert_reconciled(&report);
    cleanup(&base_root)?;
    cleanup(&head_root)?;
    Ok(())
}

#[test]
fn removed_guard_is_worsened() -> Result<(), String> {
    // read-header keeps its shape; peek-word loses its alignment guard.
    let base_root = unique_root("unguard-base")?;
    let head_root = unique_root("unguard-head")?;
    write_crate(&base_root, GUARDED_LIB)?;
    write_crate(&head_root, BASE_LIB)?;
    let base = analyze_repo(&base_root, None)?;
    let head = analyze_repo(&head_root, None)?;
    let report = evaluate_base_head_movement(&base, &head);
    assert_eq!(
        movement_for(&head, &base, "peek-word")?,
        SubjectMovement::Worsened
    );
    assert_eq!(
        movement_for(&head, &base, "read-header")?,
        SubjectMovement::Inherited
    );
    assert_reconciled(&report);
    cleanup(&base_root)?;
    cleanup(&head_root)?;
    Ok(())
}

#[test]
fn removed_operation_is_resolved_only_when_head_is_complete() -> Result<(), String> {
    let trimmed = BASE_LIB.replace(
        "\npub fn peek_word(ptr: *const u32) -> u32 {\n    unsafe { *ptr }\n}\n",
        "\n",
    );
    assert!(
        !trimmed.contains("peek_word"),
        "head fixture must drop the operation"
    );
    let (base_root, head_root) = fixture_pair(&trimmed)?;
    let base = analyze_repo(&base_root, None)?;
    let head = analyze_repo(&head_root, None)?;
    let report = evaluate_base_head_movement(&base, &head);
    assert_eq!(
        movement_for(&head, &base, "peek-word")?,
        SubjectMovement::Resolved
    );
    assert_eq!(
        movement_for(&head, &base, "read-header")?,
        SubjectMovement::Inherited
    );
    assert_eq!(report.summary.resolved, 1);
    assert_reconciled(&report);
    cleanup(&base_root)?;
    cleanup(&head_root)?;
    Ok(())
}

#[test]
fn removed_operation_is_resolved_when_diff_head_covers_its_file() -> Result<(), String> {
    // A diff-scoped head that scans the subject's file and finds the
    // operation gone reports resolved: coverage, not mere absence.
    let trimmed = BASE_LIB.replace(
        "\npub fn peek_word(ptr: *const u32) -> u32 {\n    unsafe { *ptr }\n}\n",
        "\n",
    );
    let (base_root, head_root) = fixture_pair(&trimmed)?;
    fs::write(
        head_root.join("change.diff"),
        "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,0 +1,1 @@\n+// context edit\n",
    )
    .map_err(|error| format!("write change.diff failed: {error}"))?;
    let base = analyze_repo(&base_root, None)?;
    let head = analyze_diff(&head_root, "change.diff")?;
    assert!(
        head.diff_scoped_files
            .iter()
            .any(|file| file.ends_with("src/lib.rs") || file.ends_with("src\\lib.rs")),
        "diff-scoped head must cover the subject file: {:?}",
        head.diff_scoped_files
    );
    let report = evaluate_base_head_movement(&base, &head);
    assert_eq!(
        movement_for(&head, &base, "peek-word")?,
        SubjectMovement::Resolved
    );
    assert_eq!(
        movement_for(&head, &base, "read-header")?,
        SubjectMovement::Inherited
    );
    assert_reconciled(&report);
    cleanup(&base_root)?;
    cleanup(&head_root)?;
    Ok(())
}

#[test]
fn removed_operation_is_unknown_when_head_is_capped() -> Result<(), String> {
    let trimmed = BASE_LIB.replace(
        "\npub fn peek_word(ptr: *const u32) -> u32 {\n    unsafe { *ptr }\n}\n",
        "\n",
    );
    let (base_root, head_root) = fixture_pair(&trimmed)?;
    let base = analyze_repo(&base_root, None)?;
    let head = analyze_repo(&head_root, Some(0))?;
    assert!(
        head.summary.scan_capped,
        "capped head must report scan_capped"
    );
    let report = evaluate_base_head_movement(&base, &head);
    for entry in &report.subjects {
        assert_eq!(
            entry.movement,
            SubjectMovement::Unknown,
            "a capped head cannot emit a clean verdict: {}",
            entry.subject
        );
    }
    assert_reconciled(&report);
    cleanup(&base_root)?;
    cleanup(&head_root)?;
    Ok(())
}

#[test]
fn narrow_head_scan_cannot_resolve_uncovered_cards() -> Result<(), String> {
    // A diff-scoped head that never scans the subject's file must not
    // report its base card as resolved.
    let trimmed = BASE_LIB.replace(
        "\npub fn peek_word(ptr: *const u32) -> u32 {\n    unsafe { *ptr }\n}\n",
        "\n",
    );
    let (base_root, head_root) = fixture_pair(&trimmed)?;
    fs::write(
        head_root.join("change.diff"),
        "diff --git a/src/other.rs b/src/other.rs\n--- /dev/null\n+++ b/src/other.rs\n@@ -0,0 +1 @@\n+// unrelated\n",
    )
    .map_err(|error| format!("write change.diff failed: {error}"))?;
    let base = analyze_repo(&base_root, None)?;
    let head = analyze_diff(&head_root, "change.diff")?;
    let report = evaluate_base_head_movement(&base, &head);
    for entry in &report.subjects {
        assert_eq!(
            entry.movement,
            SubjectMovement::Unknown,
            "an uncovered base card must not resolve: {}",
            entry.subject
        );
    }
    assert_reconciled(&report);
    cleanup(&base_root)?;
    cleanup(&head_root)?;
    Ok(())
}

#[test]
fn unresolved_diff_entries_force_unknown() -> Result<(), String> {
    let (base_root, head_root) = fixture_pair(BASE_LIB)?;
    fs::write(
        head_root.join("change.diff"),
        "diff --git a/src/ghost.rs b/src/ghost.rs\n--- /dev/null\n+++ b/src/ghost.rs\n@@ -0,0 +1 @@\n+// added\n",
    )
    .map_err(|error| format!("write change.diff failed: {error}"))?;
    let base = analyze_repo(&base_root, None)?;
    let head = analyze_diff(&head_root, "change.diff")?;
    assert!(
        !head.unresolved_diff_files.is_empty(),
        "ghost file must stay unresolved"
    );
    let report = evaluate_base_head_movement(&base, &head);
    for entry in &report.subjects {
        assert_eq!(
            entry.movement,
            SubjectMovement::Unknown,
            "unresolved diff input must not emit a clean verdict: {}",
            entry.subject
        );
    }
    assert_reconciled(&report);
    cleanup(&base_root)?;
    cleanup(&head_root)?;
    Ok(())
}

#[test]
fn narrow_base_scan_cannot_introduce_uncovered_subjects() -> Result<(), String> {
    // A diff-scoped base that never scanned lib.rs must not report its
    // head-only cards as introduced, even though the base side is otherwise
    // complete. This is the introduced-direction mirror of the uncovered
    // head rule.
    let (base_root, head_root) = fixture_pair(BASE_LIB)?;
    for root in [&base_root, &head_root] {
        fs::write(
            root.join("src/other.rs"),
            "pub fn helper() -> u32 {\n    41\n}\n",
        )
        .map_err(|error| format!("write other.rs failed: {error}"))?;
        fs::write(
            root.join("change.diff"),
            "diff --git a/src/other.rs b/src/other.rs\n--- /dev/null\n+++ b/src/other.rs\n@@ -0,0 +1,3 @@\n+pub fn helper() -> u32 {\n+    41\n+}\n",
        )
        .map_err(|error| format!("write change.diff failed: {error}"))?;
    }
    let base = analyze_diff(&base_root, "change.diff")?;
    let head = analyze_repo(&head_root, None)?;
    assert!(!head.summary.scan_capped, "test setup must stay uncapped");
    let report = evaluate_base_head_movement(&base, &head);
    for entry in &report.subjects {
        assert_eq!(
            entry.movement,
            SubjectMovement::Unknown,
            "a base that never scanned the file must not report introduced: {}",
            entry.subject
        );
        assert!(
            entry.reason.contains("did not cover"),
            "unexpected reason: {}",
            entry.reason
        );
    }
    assert_reconciled(&report);
    cleanup(&base_root)?;
    cleanup(&head_root)?;
    Ok(())
}

#[test]
fn diff_scoped_base_covering_the_file_can_introduce() -> Result<(), String> {
    // A diff-scoped base that DID scan lib.rs and found no peek_word card
    // licenses a genuine introduction at head.
    let trimmed = BASE_LIB.replace(
        "\npub fn peek_word(ptr: *const u32) -> u32 {\n    unsafe { *ptr }\n}\n",
        "\n",
    );
    let (base_root, head_root) = fixture_pair(&trimmed)?;
    // Head regains the operation: copying the full fixture over the head
    // root keeps both checkouts otherwise identical.
    write_crate(&head_root, BASE_LIB)?;
    for root in [&base_root, &head_root] {
        fs::write(
            root.join("change.diff"),
            "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -0,0 +1 @@\n+// touch\n",
        )
        .map_err(|error| format!("write change.diff failed: {error}"))?;
    }
    let base = analyze_diff(&base_root, "change.diff")?;
    assert!(
        base.diff_scoped_files
            .iter()
            .any(|file| file.ends_with("src/lib.rs") || file.ends_with("src\\lib.rs")),
        "base must cover the subject file: {:?}",
        base.diff_scoped_files
    );
    let head = analyze_repo(&head_root, None)?;
    let report = evaluate_base_head_movement(&base, &head);
    // peek-word is absent from the covering base scan and present at head.
    let introduced: Vec<&str> = report
        .subjects
        .iter()
        .filter(|entry| entry.movement == SubjectMovement::Introduced)
        .map(|entry| entry.subject.as_str())
        .collect();
    assert!(
        introduced
            .iter()
            .any(|subject| subject.contains("peek-word")),
        "covering base scan must license the introduction: {introduced:?}"
    );
    assert_reconciled(&report);
    cleanup(&base_root)?;
    cleanup(&head_root)?;
    Ok(())
}

#[test]
fn renamed_owner_splits_subjects_without_collapse() -> Result<(), String> {
    // Same operation text under a different owner is a different subject:
    // resolved plus introduced, never a silent match.
    let renamed = BASE_LIB.replace("pub fn peek_word", "pub fn peek_dword");
    let (base_root, head_root) = fixture_pair(&renamed)?;
    let base = analyze_repo(&base_root, None)?;
    let head = analyze_repo(&head_root, None)?;
    let report = evaluate_base_head_movement(&base, &head);
    assert_eq!(
        movement_for(&head, &base, "peek-word")?,
        SubjectMovement::Resolved
    );
    assert_eq!(
        movement_for(&head, &base, "peek-dword")?,
        SubjectMovement::Introduced
    );
    assert_eq!(
        movement_for(&head, &base, "read-header")?,
        SubjectMovement::Inherited
    );
    assert_reconciled(&report);
    cleanup(&base_root)?;
    cleanup(&head_root)?;
    Ok(())
}

#[test]
fn unknown_verdicts_name_their_exact_cause() -> Result<(), String> {
    // Capped head: the base-only subject reports head incompleteness.
    let trimmed = BASE_LIB.replace(
        "\npub fn peek_word(ptr: *const u32) -> u32 {\n    unsafe { *ptr }\n}\n",
        "\n",
    );
    let (base_root, head_root) = fixture_pair(&trimmed)?;
    let base = analyze_repo(&base_root, None)?;
    let capped = analyze_repo(&head_root, Some(0))?;
    let capped_report = evaluate_base_head_movement(&base, &capped);
    let capped_entry = capped_report
        .subjects
        .iter()
        .find(|entry| entry.movement == SubjectMovement::Unknown)
        .ok_or_else(|| "capped head must yield unknown".to_string())?;
    assert!(
        capped_entry.reason.contains("head analysis is incomplete"),
        "unexpected reason: {}",
        capped_entry.reason
    );
    cleanup(&base_root)?;
    cleanup(&head_root)?;

    // Uncovered file: the diff-scoped head resolves its named file but
    // never scans the subject's file.
    let (base_root, head_root) = fixture_pair(&trimmed)?;
    fs::write(
        head_root.join("src/other.rs"),
        "pub fn helper() -> u32 {\n    41\n}\n",
    )
    .map_err(|error| format!("write other.rs failed: {error}"))?;
    fs::write(
        head_root.join("change.diff"),
        "diff --git a/src/other.rs b/src/other.rs\n--- /dev/null\n+++ b/src/other.rs\n@@ -0,0 +1,3 @@\n+pub fn helper() -> u32 {\n+    41\n+}\n",
    )
    .map_err(|error| format!("write change.diff failed: {error}"))?;
    let narrowed = analyze_diff(&head_root, "change.diff")?;
    let narrowed_report = evaluate_base_head_movement(&base, &narrowed);
    let narrowed_entry = narrowed_report
        .subjects
        .iter()
        .find(|entry| entry.movement == SubjectMovement::Unknown)
        .ok_or_else(|| "uncovered head must yield unknown".to_string())?;
    assert!(
        narrowed_entry.reason.contains("did not cover"),
        "unexpected reason: {}",
        narrowed_entry.reason
    );
    cleanup(&base_root)?;
    cleanup(&head_root)?;

    // Duplicates: identical subjects on one side name the ambiguity.
    let duplicated = format!(
        "{BASE_LIB}\npub fn peek_word(ptr: *const u32) -> u32 {{\n    unsafe {{ *ptr }}\n}}\n"
    );
    let (base_root, head_root) = fixture_pair(&duplicated)?;
    let dup_base = analyze_repo(&base_root, None)?;
    let dup_head = analyze_repo(&head_root, None)?;
    let dup_report = evaluate_base_head_movement(&dup_base, &dup_head);
    let dup_entry = dup_report
        .subjects
        .iter()
        .find(|entry| entry.movement == SubjectMovement::Unknown)
        .ok_or_else(|| "duplicates must yield unknown".to_string())?;
    assert!(
        dup_entry.reason.contains("duplicate subject identity"),
        "unexpected reason: {}",
        dup_entry.reason
    );
    assert_reconciled(&dup_report);
    cleanup(&base_root)?;
    cleanup(&head_root)?;
    Ok(())
}

#[test]
fn duplicate_subjects_are_unknown_never_forced() -> Result<(), String> {
    let duplicated = format!(
        "{BASE_LIB}\n/// # Safety\n///\n/// Caller guarantees `ptr` is valid.\npub fn peek_word(ptr: *const u32) -> u32 {{\n    unsafe {{ *ptr }}\n}}\n"
    );
    let (base_root, head_root) = fixture_pair(&duplicated)?;
    let base = analyze_repo(&base_root, None)?;
    let head = analyze_repo(&head_root, None)?;
    let report = evaluate_base_head_movement(&base, &head);
    let peek: Vec<_> = report
        .subjects
        .iter()
        .filter(|entry| entry.subject.contains("peek-word"))
        .collect();
    assert_eq!(peek.len(), 1, "same base identity must collapse once");
    assert_eq!(peek[0].movement, SubjectMovement::Unknown);
    assert_reconciled(&report);
    cleanup(&base_root)?;
    cleanup(&head_root)?;
    Ok(())
}

#[test]
fn ledger_posture_rides_alongside_without_changing() -> Result<(), String> {
    let (base_root, head_root) = fixture_pair(BASE_LIB)?;
    let base = analyze_repo(&base_root, None)?;
    let head = analyze_repo(&head_root, None)?;
    let report = evaluate_base_head_movement(&base, &head);
    for entry in &report.subjects {
        assert!(
            !entry.ledger_baseline_state.trim().is_empty()
                && !entry.ledger_outcome_movement.trim().is_empty(),
            "every entry records the ledger-derived posture side by side"
        );
    }
    // No ledger is configured here, so every card keeps its non-baseline
    // posture while the new verdict reports inherited context.
    for card in &head.cards {
        let block = card.coverage_block();
        assert_eq!(
            block.baseline_state.as_str(),
            "new",
            "untouched cards stay non-baseline without a ledger"
        );
    }
    assert_reconciled(&report);
    cleanup(&base_root)?;
    cleanup(&head_root)?;
    Ok(())
}
