#![no_main]

use libfuzzer_sys::fuzz_target;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use unsafe_review_core::{
    AnalysisMode, AnalyzeInput, DiffSource, PolicyMode, Scope, analyze, audit_witness_receipts,
    evaluate_policy_report, render_badge_jsons, render_comment_plan, render_github_summary,
    render_human, render_json, render_lsp, render_markdown, render_policy_report_json,
    render_policy_report_markdown, render_pr_summary, render_receipt_audit_json,
    render_receipt_audit_markdown, render_repair_queue, render_sarif, render_witness_plan,
};

const MAX_SOURCE_BYTES: usize = 16 * 1024;
const MAX_DIFF_BYTES: usize = 16 * 1024;
const MAX_TEST_BYTES: usize = 8 * 1024;
const SPLIT_MARKER: &str = "\n---DIFF---\n";
const SPLIT_MARKER_CRLF: &str = "\r\n---DIFF---\r\n";
const TESTS_MARKER: &str = "\n---TESTS---\n";
const TESTS_MARKER_CRLF: &str = "\r\n---TESTS---\r\n";

fuzz_target!(|data: &[u8]| {
    let (config, body) = parse_config(data);
    let input = String::from_utf8_lossy(body);
    let (source, diff_tail, tests) = split_input(&input);
    let source = clamp(source, MAX_SOURCE_BYTES);
    let diff_tail = clamp(diff_tail, MAX_DIFF_BYTES);
    let tests = clamp(tests, MAX_TEST_BYTES);

    let root = fuzz_root(data);
    let _cleanup = CleanupGuard::new(root.clone());
    if write_fixture(&root, source, tests).is_err() {
        return;
    }

    let diff = changed_lib_diff(source, diff_tail, config.emit_empty_hunk);
    let diff_source = if config.use_diff_file {
        match write_diff_file(&root, &diff) {
            Ok(path) => DiffSource::File(path),
            Err(_) => return,
        }
    } else {
        DiffSource::Text(diff)
    };

    run_analysis(AnalyzeInput {
        root: root.clone(),
        scope: config.scope,
        diff: diff_source,
        mode: config.mode,
        policy: config.policy.clone(),
        include_unchanged_tests: config.include_unchanged_tests,
        max_cards: config.max_cards,
    });
    run_analysis(AnalyzeInput {
        root: root.clone(),
        scope: Scope::Repo,
        diff: DiffSource::NoneRepoScan,
        mode: AnalysisMode::Repo,
        policy: config.policy,
        include_unchanged_tests: config.include_unchanged_tests,
        max_cards: Some(64),
    });

    if config.run_receipt_audit {
        run_receipt_audit(AnalyzeInput {
            root,
            scope: Scope::Repo,
            diff: DiffSource::NoneRepoScan,
            mode: AnalysisMode::Repo,
            policy: PolicyMode::Advisory,
            include_unchanged_tests: config.include_unchanged_tests,
            max_cards: Some(64),
        });
    }
});

struct FuzzConfig {
    scope: Scope,
    mode: AnalysisMode,
    policy: PolicyMode,
    include_unchanged_tests: bool,
    max_cards: Option<usize>,
    emit_empty_hunk: bool,
    use_diff_file: bool,
    run_receipt_audit: bool,
}

fn parse_config(data: &[u8]) -> (FuzzConfig, &[u8]) {
    if data.len() < 2 {
        return (default_config(), data);
    }

    let header = data[0];
    let max_cards_seed = data[1];
    let mode_seed = data.get(2).copied().unwrap_or(0);
    let policy_seed = data.get(3).copied().unwrap_or(0);
    let body_start = data.len().min(4);
    let scope = if header & 1 == 0 {
        Scope::Diff
    } else {
        Scope::Repo
    };
    let mode = match mode_seed % 4 {
        0 => AnalysisMode::Instant,
        1 => AnalysisMode::Draft,
        2 => AnalysisMode::Ready,
        _ => AnalysisMode::Repo,
    };
    let emit_empty_hunk = header & 4 != 0;
    let max_cards = if header & 8 == 0 {
        Some((max_cards_seed as usize % 128).max(1))
    } else {
        None
    };
    let include_unchanged_tests = header & 16 == 0;
    let use_diff_file = header & 32 != 0;
    let run_receipt_audit = header & 64 != 0;
    let policy = match policy_seed % 3 {
        0 => PolicyMode::Advisory,
        1 => PolicyMode::NoNewDebt,
        _ => PolicyMode::Blocking,
    };

    (
        FuzzConfig {
            scope,
            mode,
            policy,
            include_unchanged_tests,
            max_cards,
            emit_empty_hunk,
            use_diff_file,
            run_receipt_audit,
        },
        &data[body_start..],
    )
}

fn default_config() -> FuzzConfig {
    FuzzConfig {
        scope: Scope::Diff,
        mode: AnalysisMode::Draft,
        policy: PolicyMode::Advisory,
        include_unchanged_tests: true,
        max_cards: Some(64),
        emit_empty_hunk: false,
        use_diff_file: false,
        run_receipt_audit: false,
    }
}

fn split_input(input: &str) -> (&str, &str, &str) {
    let (source_and_diff, tests) = split_once_marker(input, TESTS_MARKER, TESTS_MARKER_CRLF);
    let (source, diff) = split_once_marker(source_and_diff, SPLIT_MARKER, SPLIT_MARKER_CRLF);
    (source, diff, tests)
}

fn split_once_marker<'a>(input: &'a str, lf: &str, crlf: &str) -> (&'a str, &'a str) {
    input
        .split_once(lf)
        .or_else(|| input.split_once(crlf))
        .map_or((input, ""), |(before, after)| (before, after))
}

struct CleanupGuard {
    root: PathBuf,
}

impl CleanupGuard {
    fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

impl Drop for CleanupGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn clamp(input: &str, max_bytes: usize) -> &str {
    if input.len() <= max_bytes {
        return input;
    }

    let mut end = max_bytes;
    while !input.is_char_boundary(end) {
        end -= 1;
    }
    &input[..end]
}

fn fuzz_root(data: &[u8]) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    std::env::temp_dir().join(format!(
        "unsafe-review-fuzz-{}-{:016x}",
        std::process::id(),
        hasher.finish()
    ))
}

fn write_fixture(root: &Path, source: &str, tests: &str) -> Result<(), std::io::Error> {
    let _ = fs::remove_dir_all(root);
    fs::create_dir_all(root.join("src"))?;
    fs::write(root.join("src/lib.rs"), source)?;
    if !tests.is_empty() {
        fs::create_dir_all(root.join("tests"))?;
        fs::write(root.join("tests/fuzz.rs"), tests)?;
    }
    Ok(())
}

fn write_diff_file(root: &Path, diff: &str) -> Result<PathBuf, std::io::Error> {
    let path = root.join("fuzz.diff");
    fs::write(&path, diff)?;
    Ok(path)
}

fn changed_lib_diff(source: &str, diff_tail: &str, emit_empty_hunk: bool) -> String {
    if emit_empty_hunk {
        return format!(
            "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n{diff_tail}"
        );
    }

    let added_lines = source.lines().count().max(1);
    let mut diff = format!(
        "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -0,0 +1,{added_lines} @@\n"
    );

    if source.is_empty() {
        diff.push_str("+\n");
    } else {
        for line in source.lines() {
            diff.push('+');
            diff.push_str(line);
            diff.push('\n');
        }
    }

    diff.push_str(diff_tail);
    diff
}

fn run_analysis(input: AnalyzeInput) {
    let max_cards = input.max_cards;
    if let Ok(output) = analyze(input.clone()) {
        assert_json("analysis", &render_json(&output));
        assert_json("sarif", &render_sarif(&output));
        assert_json("comment_plan", &render_comment_plan(&output));
        assert_json("lsp", &render_lsp(&output));
        assert_json("repair_queue", &render_repair_queue(&output));

        assert_eq!(
            output.summary.cards,
            output.cards.len(),
            "summary card count must match ReviewCard list length"
        );
        if let Some(max_cards) = max_cards {
            assert!(
                output.cards.len() <= max_cards,
                "analysis must honor configured max_cards"
            );
        }

        let (badge, badge_plus) = render_badge_jsons(&output);
        assert_json("badge", &badge);
        assert_json("badge_plus", &badge_plus);

        for (name, rendered) in [
            ("human", render_human(&output)),
            ("markdown", render_markdown(&output)),
            ("pr_summary", render_pr_summary(&output)),
            ("github_summary", render_github_summary(&output)),
            ("witness_plan", render_witness_plan(&output)),
        ] {
            assert_not_empty(name, &rendered);
        }
    }

    if let Ok(report) = evaluate_policy_report(input) {
        assert_json("policy_report", &render_policy_report_json(&report));
        assert_not_empty(
            "policy_report_markdown",
            &render_policy_report_markdown(&report),
        );
    }
}

fn run_receipt_audit(input: AnalyzeInput) {
    if let Ok(report) = audit_witness_receipts(input) {
        assert_json("receipt_audit", &render_receipt_audit_json(&report));
        assert_not_empty(
            "receipt_audit_markdown",
            &render_receipt_audit_markdown(&report),
        );
    }
}

fn assert_json(name: &str, rendered: &str) {
    let parsed = serde_json::from_str::<serde_json::Value>(rendered);
    assert!(parsed.is_ok(), "rendered {name} JSON must parse");
}

fn assert_not_empty(name: &str, rendered: &str) {
    assert!(
        !rendered.trim().is_empty(),
        "rendered {name} output must not be empty"
    );
}
