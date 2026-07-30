use crate::api::DiffSource;
use crate::input::diff;
use std::fs;
use std::path::Path;

pub(super) fn package_name(root: &Path) -> String {
    let Ok(text) = fs::read_to_string(root.join("Cargo.toml")) else {
        return root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("workspace")
            .to_string();
    };
    let mut in_package = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_package = trimmed == "[package]";
            continue;
        }
        if !in_package || !trimmed.starts_with("name") {
            continue;
        }
        let Some((_key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let name = value.trim().trim_matches('"');
        if !name.is_empty() {
            return name.to_string();
        }
    }
    root.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("workspace")
        .to_string()
}

pub(super) fn load_diff_index(source: &DiffSource) -> Result<diff::DiffIndex, String> {
    match source {
        DiffSource::NoneRepoScan => Ok(diff::DiffIndex::default()),
        DiffSource::Text(text) => {
            let index = diff::parse_unified_diff(text);
            reject_unparseable_diff(text, "inline diff", &index)?;
            Ok(index)
        }
        DiffSource::File(path) => {
            let text = fs::read_to_string(path)
                .map_err(|err| format!("read diff {} failed: {err}", path.display()))?;
            let index = diff::parse_unified_diff(&text);
            reject_unparseable_diff(&text, &path.display().to_string(), &index)?;
            Ok(index)
        }
    }
}

/// Return `Err` when the input text is non-empty and contains no recognizable
/// unified-diff structure (no `diff --git`, `--- `, `+++ `, or `@@` line
/// prefix). An empty or whitespace-only input is accepted as a zero-change diff
/// (a `git diff` with no changed files legitimately produces empty output).
/// A structurally diff-like input that yields zero indexed files — for example,
/// a binary-only diff that carries `diff --git` markers but no `+++ b/` lines —
/// is also accepted. We prefer false-accepts over false-rejects for advisory
/// tooling: if any recognized diff marker is present we leave further validation
/// to the caller.
fn reject_unparseable_diff(
    text: &str,
    source_label: &str,
    index: &diff::DiffIndex,
) -> Result<(), String> {
    if !index.is_empty() || text.trim().is_empty() {
        return Ok(());
    }
    let has_diff_marker = text.lines().any(|line| {
        line.starts_with("diff --git ")
            || line.starts_with("--- ")
            || line.starts_with("+++ ")
            || line.starts_with("@@")
    });
    if has_diff_marker {
        return Ok(());
    }
    Err(format!(
        "{source_label} could not be parsed as a unified diff (no diff --git, ---, +++, or @@ \
         markers found); no analysis was run. Supply a valid `git diff` or unified diff output."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::DiffSource;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_path(prefix: &str) -> Result<std::path::PathBuf, String> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system time error: {err}"))?
            .as_nanos();
        Ok(std::env::temp_dir().join(format!("{prefix}-{nanos}.diff")))
    }

    #[test]
    fn garbage_text_via_text_source_is_rejected() -> Result<(), String> {
        let source = DiffSource::Text("this is not a diff at all".to_string());
        let err = match load_diff_index(&source) {
            Err(e) => e,
            Ok(_) => return Err("expected garbage text to be rejected, but it was accepted".into()),
        };
        assert!(
            err.contains("inline diff"),
            "error should name the source: {err}"
        );
        assert!(
            err.contains("could not be parsed as a unified diff"),
            "error should state parse failure: {err}"
        );
        assert!(
            err.contains("no analysis was run"),
            "error should state no analysis ran: {err}"
        );
        Ok(())
    }

    #[test]
    fn garbage_file_via_file_source_is_rejected() -> Result<(), String> {
        let path = unique_temp_path("unsafe-review-garbage-diff-test")?;
        fs::write(&path, "this is not a diff at all")
            .map_err(|err| format!("write temp diff failed: {err}"))?;
        let source = DiffSource::File(path.clone());
        let err = match load_diff_index(&source) {
            Err(e) => e,
            Ok(_) => return Err("expected garbage file to be rejected, but it was accepted".into()),
        };
        let path_str = path.display().to_string();
        assert!(
            err.contains(&path_str),
            "error should include the path: {err}"
        );
        assert!(
            err.contains("could not be parsed as a unified diff"),
            "error should state parse failure: {err}"
        );
        assert!(
            err.contains("no analysis was run"),
            "error should state no analysis ran: {err}"
        );
        let _ = fs::remove_file(&path);
        Ok(())
    }

    #[test]
    fn non_utf8_diff_file_is_rejected_fail_closed() -> Result<(), String> {
        // A diff file that is not valid UTF-8 must fail closed at read time
        // (`fs::read_to_string` rejects it) rather than being silently treated
        // as an empty / zero-change diff. Hostile-input regression coverage for
        // the `File` diff source (issue #1883): the only prior non-UTF-8 test
        // targeted a source file during repo scan, not the diff input itself.
        let path = unique_temp_path("unsafe-review-non-utf8-diff-test")?;
        fs::write(&path, [0xffu8, 0xfe, 0x00, 0xfd])
            .map_err(|err| format!("write temp diff failed: {err}"))?;
        let source = DiffSource::File(path.clone());
        let err = match load_diff_index(&source) {
            Err(e) => e,
            Ok(_) => {
                let _ = fs::remove_file(&path);
                return Err(
                    "expected a non-UTF-8 diff file to be rejected, but it was accepted".into(),
                );
            }
        };
        let path_str = path.display().to_string();
        assert!(
            err.contains(&path_str),
            "error should include the diff path: {err}"
        );
        assert!(
            err.contains("read diff"),
            "error should name the diff read step: {err}"
        );
        assert!(
            err.contains("failed"),
            "error should state the read failed: {err}"
        );
        let _ = fs::remove_file(&path);
        Ok(())
    }

    #[test]
    fn absolute_and_traversal_diff_paths_are_indexed_inertly_not_escaped() -> Result<(), String> {
        // Hostile-input regression coverage for the `+++ b/` path-traversal /
        // absolute-path row of issue #1883. A unified diff whose changed-file
        // header names an absolute path or a `../` traversal path must be
        // accepted and indexed as the *literal* path string; the parser must
        // never normalize, resolve, or open it. Because the diff index is only
        // ever consulted by exact-match lookups keyed by files discovered under
        // the scan root (all relative), a foreign path is inert -- it can never
        // pull an out-of-root file into analysis (the "cannot escape configured
        // roots" contract). This test pins that the path is stored verbatim and
        // is NOT silently normalized to a root-relative form.
        let diff = concat!(
            "diff --git a/../../../../etc/passwd.rs b/../../../../etc/passwd.rs\n",
            "--- a/../../../../etc/passwd.rs\n",
            "+++ b/../../../../etc/passwd.rs\n",
            "@@ -0,0 +1,1 @@\n",
            "+pub unsafe fn escaped() {}\n",
            "diff --git a//etc/shadow.rs b//etc/shadow.rs\n",
            "--- a//etc/shadow.rs\n",
            "+++ b//etc/shadow.rs\n",
            "@@ -0,0 +1,1 @@\n",
            "+pub unsafe fn absolute() {}\n",
        );
        let index = load_diff_index(&DiffSource::Text(diff.to_string()))?;

        // Both foreign paths are accepted as structurally valid diff entries.
        assert_eq!(
            index.changed_file_count(),
            2,
            "both foreign-path files should be indexed"
        );

        // The traversal path is stored as the literal string after `+++ b/`,
        // not resolved or stripped to a root-relative path.
        let traversal = std::path::PathBuf::from("../../../../etc/passwd.rs");
        assert!(
            index.contains_in_range(&traversal, 1, 1),
            "traversal path must be indexed under its literal, unresolved key"
        );
        assert!(
            !index.contains_in_range(&std::path::PathBuf::from("etc/passwd.rs"), 1, 1),
            "traversal must not be normalized to a root-relative key"
        );

        // The absolute path is likewise stored literally (leading slash kept).
        let absolute = std::path::PathBuf::from("/etc/shadow.rs");
        assert!(
            index.contains_in_range(&absolute, 1, 1),
            "absolute path must be indexed under its literal key"
        );
        Ok(())
    }

    #[test]
    fn oversized_hunk_numbers_and_long_lines_are_handled_without_panic() -> Result<(), String> {
        // Hostile-input regression coverage for the oversized-hunk / extreme
        // line-length row of issue #1883. A hunk header whose `+` start line is
        // too large to fit in `usize` must not panic: `parse_new_start` returns
        // `None`, so the coordinate simply stays at its default rather than
        // overflowing, and the added line is still indexed (at the degenerate
        // line 0, which no real 1-based site query can match -- fail-safe). An
        // extremely long added line must likewise be handled without panic. The
        // parser advances the coordinate with `saturating_add`, so large inputs
        // truncate rather than crash.
        let long_line = "a".repeat(200_000);
        let diff = format!(
            concat!(
                "diff --git a/src/huge.rs b/src/huge.rs\n",
                "--- a/src/huge.rs\n",
                "+++ b/src/huge.rs\n",
                // A `+` start line far beyond usize::MAX on any platform.
                "@@ -0,0 +999999999999999999999999999999,1 @@\n",
                "+{}\n",
            ),
            long_line
        );
        let index = load_diff_index(&DiffSource::Text(diff))?;

        // The diff is accepted (structurally valid) and the file is indexed.
        assert_eq!(
            index.changed_file_count(),
            1,
            "an oversized-hunk diff should still index its changed file"
        );
        // The unparseable start line fell back to the degenerate coordinate 0,
        // which cannot collide with a real 1-based unsafe-site query.
        let path = std::path::PathBuf::from("src/huge.rs");
        assert!(
            index.contains_in_range(&path, 0, 0),
            "the added line falls back to the degenerate line-0 coordinate"
        );
        assert!(
            !index.contains_in_range(&path, 1, usize::MAX),
            "no real 1-based line range should match the degenerate fallback"
        );
        Ok(())
    }

    #[test]
    fn empty_string_is_accepted_as_empty_index() -> Result<(), String> {
        let source = DiffSource::Text(String::new());
        let index = load_diff_index(&source)?;
        assert!(index.is_empty(), "empty text should yield an empty index");
        Ok(())
    }

    #[test]
    fn whitespace_only_text_is_accepted_as_empty_index() -> Result<(), String> {
        let source = DiffSource::Text("   \n\t\n  ".to_string());
        let index = load_diff_index(&source)?;
        assert!(
            index.is_empty(),
            "whitespace-only text should yield an empty index"
        );
        Ok(())
    }

    #[test]
    fn valid_diff_is_accepted_with_expected_file_count() -> Result<(), String> {
        let diff_text = concat!(
            "diff --git a/src/lib.rs b/src/lib.rs\n",
            "--- a/src/lib.rs\n",
            "+++ b/src/lib.rs\n",
            "@@ -1,0 +1,1 @@\n",
            "+pub fn added() {}\n",
        );
        let source = DiffSource::Text(diff_text.to_string());
        let index = load_diff_index(&source)?;
        assert_eq!(
            index.changed_file_count(),
            1,
            "valid diff should index exactly one file"
        );
        Ok(())
    }

    #[test]
    fn binary_only_diff_with_diff_git_marker_is_accepted() -> Result<(), String> {
        // A binary-only diff carries `diff --git` but no `+++ b/` lines, so
        // the index has zero files. It must still be accepted because it has
        // recognizable diff structure.
        let diff_text = "diff --git a/assets/logo.png b/assets/logo.png\n\
                         Binary files a/assets/logo.png and b/assets/logo.png differ\n";
        let source = DiffSource::Text(diff_text.to_string());
        let index = load_diff_index(&source)?;
        assert!(
            index.is_empty(),
            "binary-only diff should yield an empty index without error"
        );
        Ok(())
    }
}
