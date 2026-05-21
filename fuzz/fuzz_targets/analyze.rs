#![no_main]

use libfuzzer_sys::fuzz_target;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;
use unsafe_review_core::{
    AnalysisMode, AnalyzeInput, DiffSource, PolicyMode, Scope, analyze, render_json,
};

const MAX_SOURCE_BYTES: usize = 16 * 1024;
const MAX_DIFF_BYTES: usize = 16 * 1024;
const SPLIT_MARKER: &str = "\n---DIFF---\n";
const SPLIT_MARKER_CRLF: &str = "\r\n---DIFF---\r\n";

fuzz_target!(|data: &[u8]| {
    let input = String::from_utf8_lossy(data);
    let (source, diff_tail) = split_input(&input);
    let source = clamp(source, MAX_SOURCE_BYTES);
    let diff_tail = clamp(diff_tail, MAX_DIFF_BYTES);

    let root = fuzz_root(data);
    let _cleanup = CleanupGuard::new(root.clone());
    if write_fixture(&root, source).is_err() {
        return;
    }

    let diff = changed_lib_diff(source, diff_tail);
    let result = analyze(AnalyzeInput {
        root: root.clone(),
        scope: Scope::Diff,
        diff: DiffSource::Text(diff),
        mode: AnalysisMode::Draft,
        policy: PolicyMode::Advisory,
        include_unchanged_tests: true,
        max_cards: Some(64),
    });

    if let Ok(output) = result {
        let json = render_json(&output);
        let parsed = serde_json::from_str::<serde_json::Value>(&json);
        assert!(parsed.is_ok(), "rendered analysis JSON must parse");
    }

});

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
