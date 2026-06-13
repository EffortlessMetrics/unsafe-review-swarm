use crate::domain::{ReachEvidence, RelatedTest};
use std::fs;
use std::path::{Path, PathBuf};

/// Returns true when `owner` appears in `text` as a whole identifier — i.e. every
/// occurrence is bounded on both sides by a non-identifier character (or the
/// start/end of the text).  Used as a cheap prefilter before the per-line check.
fn text_contains_owner_as_ident(text: &str, owner: &str) -> bool {
    let owner_bytes = owner.as_bytes();
    let text_bytes = text.as_bytes();
    let owner_len = owner_bytes.len();
    if owner_len == 0 {
        return false;
    }
    let mut start = 0usize;
    while start + owner_len <= text_bytes.len() {
        if let Some(pos) = text[start..].find(owner) {
            let abs = start + pos;
            if is_ident_boundary(text_bytes, abs, owner_len) {
                return true;
            }
            start = abs + 1;
        } else {
            break;
        }
    }
    false
}

/// Returns true when `owner` appears in `line` as a whole identifier.
fn line_contains_owner_as_ident(line: &str, owner: &str) -> bool {
    text_contains_owner_as_ident(line, owner)
}

/// Returns true when the slice `bytes[pos..pos+len]` is surrounded by
/// non-identifier chars on both sides (start-of-string and end-of-string count
/// as non-identifier boundaries).  The identifier-char predicate mirrors
/// `parse_ident` and `parse_test_name`: `_` or ASCII alphanumeric.
fn is_ident_boundary(bytes: &[u8], pos: usize, len: usize) -> bool {
    let before_ok = pos == 0 || !is_ident_char(bytes[pos - 1]);
    let after_ok = pos + len >= bytes.len() || !is_ident_char(bytes[pos + len]);
    before_ok && after_ok
}

/// The identifier-char predicate shared with `parse_ident` (unsafe_impl.rs) and
/// `parse_test_name`.  A character is part of a Rust identifier when it is `_`
/// or ASCII alphanumeric.
fn is_ident_char(b: u8) -> bool {
    b == b'_' || b.is_ascii_alphanumeric()
}

pub(crate) fn reach_evidence(
    root: &Path,
    owner: Option<&String>,
) -> (ReachEvidence, Vec<RelatedTest>) {
    let Some(owner) = owner else {
        return (
            ReachEvidence {
                state: "unknown".to_string(),
                summary: "No owner function could be inferred".to_string(),
            },
            Vec::new(),
        );
    };
    let mut tests = Vec::new();
    let test_files = collect_test_files(root).unwrap_or_default();
    for rel in test_files {
        let abs = root.join(&rel);
        let Ok(text) = fs::read_to_string(&abs) else {
            continue;
        };
        if !text_contains_owner_as_ident(&text, owner) {
            continue;
        }
        let mut last_test: Option<(String, usize)> = None;
        for (idx, line) in text.lines().enumerate() {
            if line.contains("#[test]") {
                last_test = Some(("test".to_string(), idx + 1));
            }
            if let Some(name) = parse_test_name(line) {
                last_test = Some((name, idx + 1));
            }
            if line_contains_owner_as_ident(line, owner) {
                let (name, line_no) = last_test
                    .clone()
                    .unwrap_or_else(|| (format!("mentions {owner}"), idx + 1));
                tests.push(RelatedTest {
                    name,
                    file: rel.to_string_lossy().replace('\\', "/"),
                    line: line_no,
                });
                break;
            }
        }
    }
    if tests.is_empty() {
        (
            ReachEvidence {
                state: "unreached".to_string(),
                summary: format!("No static test mention of owner `{owner}` was found"),
            },
            tests,
        )
    } else {
        (
            ReachEvidence {
                state: "owner_reached".to_string(),
                summary: format!(
                    "{} related test file(s) mention owner `{owner}`",
                    tests.len()
                ),
            },
            tests,
        )
    }
}

fn parse_test_name(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !(trimmed.starts_with("fn ") || trimmed.starts_with("pub fn ")) {
        return None;
    }
    let pos = trimmed.find("fn ")?;
    let rest = &trimmed[pos + 3..];
    let mut name = String::new();
    for ch in rest.chars() {
        if ch == '_' || ch.is_ascii_alphanumeric() {
            name.push(ch);
        } else {
            break;
        }
    }
    (!name.is_empty()).then_some(name)
}

fn collect_test_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    visit(root, root, &mut out)?;
    out.sort();
    Ok(out)
}

fn visit(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries =
        fs::read_dir(dir).map_err(|err| format!("read {} failed: {err}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|err| format!("read_dir entry failed: {err}"))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if matches!(
                name.as_str(),
                ".git" | "target" | ".unsafe-review" | ".rails" | "node_modules"
            ) {
                continue;
            }
            visit(root, &path, out)?;
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            let rel_text = rel.to_string_lossy();
            if rel_text.contains("tests")
                || rel_text.contains("test")
                || fs::read_to_string(&path).is_ok_and(|text| text.contains("#[test]"))
            {
                out.push(rel);
            }
        }
    }
    Ok(())
}
