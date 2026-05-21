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

fuzz_target!(|data: &[u8]| {
    let input = String::from_utf8_lossy(data);
    let (source, diff_tail) = split_input(&input);
    let source = clamp(source, MAX_SOURCE_BYTES);
    let diff_tail = clamp(diff_tail, MAX_DIFF_BYTES);

    let root = fuzz_root(data);
    if write_fixture(&root, source).is_err() {
        return;
    }

    let diff = changed_lib_diff(source, diff_tail);
    run_analysis(AnalyzeInput {
        root: root.clone(),
        scope: Scope::Diff,
        diff: DiffSource::Text(diff),
        mode: AnalysisMode::Draft,
        policy: PolicyMode::Advisory,
        include_unchanged_tests: true,
        max_cards: Some(64),
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

    let _ = fs::remove_dir_all(root);
});

fn split_input(input: &str) -> (&str, &str) {
    input
        .split_once(SPLIT_MARKER)
        .map_or((input, ""), |(source, diff)| (source, diff))
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

fn changed_lib_diff(source: &str, diff_tail: &str) -> String {
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
