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
