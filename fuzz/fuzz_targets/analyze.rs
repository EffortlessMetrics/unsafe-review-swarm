#![no_main]

use libfuzzer_sys::fuzz_target;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;
use unsafe_review_core::{
    AnalysisMode, AnalyzeInput, DiffSource, PolicyMode, Scope, analyze, render_badge_jsons,
    render_comment_plan, render_human, render_json, render_lsp, render_markdown, render_pr_summary,
    render_repair_queue, render_sarif, render_witness_plan,
};

const MAX_SOURCE_BYTES: usize = 16 * 1024;
const MAX_DIFF_BYTES: usize = 16 * 1024;
const SPLIT_MARKER: &str = "\n---DIFF---\n";
const SPLIT_MARKER_CRLF: &str = "\r\n---DIFF---\r\n";

fuzz_target!(|data: &[u8]| {
    let (config, body) = parse_config(data);
    let input = String::from_utf8_lossy(body);
    let (source, diff_tail) = split_input(&input);
    let source = clamp(source, MAX_SOURCE_BYTES);
    let diff_tail = clamp(diff_tail, MAX_DIFF_BYTES);

    let root = fuzz_root(data);
    let _cleanup = CleanupGuard::new(root.clone());
    if write_fixture(&root, source).is_err() {
        return;
    }

    let diff = changed_lib_diff(source, diff_tail, config.emit_empty_hunk);
    let diff_source = if config.diff_file {
        let diff_path = root.join("change.diff");
        if fs::write(&diff_path, &diff).is_err() {
            return;
        }
        DiffSource::File(diff_path)
    } else {
        DiffSource::Text(diff)
    };

    run_analysis(
        AnalyzeInput {
            root: root.clone(),
            scope: config.scope,
            diff: diff_source,
            mode: config.mode,
            policy: PolicyMode::Advisory,
            include_unchanged_tests: true,
            max_cards: config.max_cards,
        },
        config.max_cards,
    );
    run_analysis(
        AnalyzeInput {
            root: root.clone(),
            scope: Scope::Repo,
            diff: DiffSource::NoneRepoScan,
            mode: AnalysisMode::Repo,
            policy: PolicyMode::Advisory,
            include_unchanged_tests: true,
            max_cards: Some(64),
        },
        Some(64),
    );
});

struct FuzzConfig {
    scope: Scope,
    mode: AnalysisMode,
    max_cards: Option<usize>,
    emit_empty_hunk: bool,
    diff_file: bool,
}

fn assert_json_parses(json: &str, label: &str) {
    let parsed = serde_json::from_str::<serde_json::Value>(json);
    assert!(parsed.is_ok(), "rendered {label} must parse");
}

fn parse_config(data: &[u8]) -> (FuzzConfig, &[u8]) {
    if data.len() < 2 {
        return (
            FuzzConfig {
                scope: Scope::Diff,
                mode: AnalysisMode::Draft,
                max_cards: Some(64),
                emit_empty_hunk: false,
                diff_file: false,
            },
            data,
        );
    }

    let header = data[0];
    let max_cards_seed = data[1];
    let scope = if header & 1 == 0 {
        Scope::Diff
    } else {
        Scope::Repo
    };
    let mode = match (header & 2 != 0, header & 16 != 0) {
        (false, true) => AnalysisMode::Instant,
        (true, true) => AnalysisMode::Repo,
        (false, false) => AnalysisMode::Draft,
        (true, false) => AnalysisMode::Ready,
    };
    let emit_empty_hunk = header & 4 != 0;
    let max_cards = if header & 8 == 0 {
        Some((max_cards_seed as usize % 128).max(1))
    } else {
        None
    };
    let diff_file = header & 32 != 0;

    (
        FuzzConfig {
            scope,
            mode,
            max_cards,
            emit_empty_hunk,
            diff_file,
        },
        &data[2..],
    )
}

fn split_input(input: &str) -> (&str, &str) {
    input
        .split_once(SPLIT_MARKER)
        .or_else(|| input.split_once(SPLIT_MARKER_CRLF))
        .map_or((input, ""), |(source, diff)| (source, diff))
}

struct CleanupGuard {
    root: std::path::PathBuf,
}

impl CleanupGuard {
    fn new(root: std::path::PathBuf) -> Self {
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

fn fuzz_root(data: &[u8]) -> std::path::PathBuf {
    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    std::env::temp_dir().join(format!(
        "unsafe-review-fuzz-{}-{:016x}",
        std::process::id(),
        hasher.finish()
    ))
}

fn write_fixture(root: &Path, source: &str) -> Result<(), std::io::Error> {
    let _ = fs::remove_dir_all(root);
    fs::create_dir_all(root.join("src"))?;
    fs::write(root.join("src/lib.rs"), source)?;
    Ok(())
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

fn run_analysis(input: AnalyzeInput, max_cards: Option<usize>) {
    if let Ok(output) = analyze(input) {
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
        assert_json_parses(&render_json(&output), "analysis JSON");
        assert_json_parses(&render_sarif(&output), "SARIF JSON");
        assert_json_parses(&render_comment_plan(&output), "comment plan JSON");
        assert_json_parses(&render_lsp(&output), "LSP JSON");
        assert_json_parses(&render_repair_queue(&output), "repair queue JSON");

        let (badge, badge_plus) = render_badge_jsons(&output);
        assert_json_parses(&badge, "badge JSON");
        assert_json_parses(&badge_plus, "badge plus JSON");

        let human = render_human(&output);
        assert!(
            !human.trim().is_empty(),
            "rendered human output must not be empty"
        );

        let markdown = render_markdown(&output);
        assert!(
            !markdown.trim().is_empty(),
            "rendered markdown output must not be empty"
        );

        let pr_summary = render_pr_summary(&output);
        assert!(
            !pr_summary.trim().is_empty(),
            "rendered PR summary output must not be empty"
        );

        let witness_plan = render_witness_plan(&output);
        assert!(
            !witness_plan.trim().is_empty(),
            "rendered witness plan output must not be empty"
        );
    }
}
