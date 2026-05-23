#![no_main]

use libfuzzer_sys::fuzz_target;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;
use unsafe_review_core::{
    AnalysisMode, AnalyzeInput, DiffSource, PolicyMode, Scope, analyze, render_human,
    render_json, render_markdown,
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
    run_analysis(AnalyzeInput {
        root: root.clone(),
        scope: config.scope,
        diff: DiffSource::Text(diff),
        mode: config.mode,
        policy: PolicyMode::Advisory,
        include_unchanged_tests: true,
        max_cards: config.max_cards,
    });
    run_analysis(AnalyzeInput {
        root: root.clone(),
        scope: Scope::Repo,
        diff: DiffSource::NoneRepoScan,
        mode: AnalysisMode::Repo,
        policy: PolicyMode::Advisory,
        include_unchanged_tests: true,
        max_cards: Some(64),
    });
});

struct FuzzConfig {
    scope: Scope,
    mode: AnalysisMode,
    max_cards: Option<usize>,
    emit_empty_hunk: bool,
}

fn parse_config(data: &[u8]) -> (FuzzConfig, &[u8]) {
    if data.len() < 2 {
        return (
            FuzzConfig {
                scope: Scope::Diff,
                mode: AnalysisMode::Draft,
                max_cards: Some(64),
                emit_empty_hunk: false,
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
    let mode = if header & 2 == 0 {
        AnalysisMode::Draft
    } else {
        AnalysisMode::Ready
    };
    let emit_empty_hunk = header & 4 != 0;
    let max_cards = if header & 8 == 0 {
        Some((max_cards_seed as usize % 128).max(1))
    } else {
        None
    };

    (
        FuzzConfig {
            scope,
            mode,
            max_cards,
            emit_empty_hunk,
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

fn run_analysis(input: AnalyzeInput) {
    if let Ok(output) = analyze(input) {
        let json = render_json(&output);
        let parsed = serde_json::from_str::<serde_json::Value>(&json);
        assert!(parsed.is_ok(), "rendered analysis JSON must parse");

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
    }
}
