use crate::domain::{ReachEvidence, RelatedTest};
use std::fs;
use std::path::{Path, PathBuf};

/// Returns true when `line` contains the owner in a **call or use shape**.
///
/// Accepted shapes (all require the owner to appear as a whole identifier):
/// - `owner(` — free call or tuple-struct constructor
/// - `.owner(` — method call
/// - `::owner(` — qualified call
/// - `owner!` — macro invocation
/// - `owner {` — struct literal / record constructor
///
/// A bare identifier (in a comment, a type position, or an unrelated token) is
/// NOT sufficient.  This is the owner-decided rule: "bare static mention is not
/// reach; a call inside test scope can be reach."
///
/// Self-reach exclusion: a function *definition* (`fn owner(` or `fn owner {`)
/// does NOT count, because the owner appears only as the function's own name,
/// not as a call site.
fn line_has_owner_call_shape(line: &str, owner: &str) -> bool {
    if owner.is_empty() {
        return false;
    }
    let owner_bytes = owner.as_bytes();
    let line_bytes = line.as_bytes();
    let owner_len = owner_bytes.len();
    let mut start = 0usize;
    while start + owner_len <= line_bytes.len() {
        let Some(pos) = line[start..].find(owner) else {
            break;
        };
        let abs = start + pos;
        if is_ident_boundary(line_bytes, abs, owner_len) {
            // Check what immediately follows the owner identifier (skip whitespace).
            let after_pos = abs + owner_len;
            // Find the first non-whitespace byte at or after after_pos.
            let next_non_ws = line_bytes[after_pos..]
                .iter()
                .position(|&b| b != b' ' && b != b'\t')
                .map(|p| after_pos + p);
            let call_suffix = next_non_ws.map(|p| line_bytes[p]);
            // `(` — free/tuple/qualified call, `!` — macro, `{` — struct literal.
            if matches!(call_suffix, Some(b'(' | b'!' | b'{')) {
                // Self-reach exclusion: `fn owner(` / `fn owner {` is a function
                // definition, not a call site.  The keyword `fn` must not appear
                // immediately before the owner (with only whitespace between).
                if is_fn_definition(line_bytes, abs) {
                    start = abs + 1;
                    continue;
                }
                return true;
            }
        }
        start = abs + 1;
    }
    false
}

/// Returns true when `owner` at byte position `abs` in `line_bytes` is
/// preceded only by `fn` (with any amount of whitespace between them).
/// This is the syntactic marker that the owner appears as a function *name*
/// (a definition site), not a call site.
fn is_fn_definition(line_bytes: &[u8], abs: usize) -> bool {
    // Walk backwards past whitespace.
    let mut i = abs.saturating_sub(1);
    while i > 0 && (line_bytes[i] == b' ' || line_bytes[i] == b'\t') {
        i = i.saturating_sub(1);
    }
    // i now points at the last non-whitespace byte before the owner.
    // Check if bytes [i-1..=i] spell "fn" (or the very start of a "fn" keyword).
    if i >= 1 && line_bytes[i] == b'n' && line_bytes[i - 1] == b'f' {
        // Make sure this `fn` is itself whole-identifier-bounded.
        let fn_start = i - 1;
        let before_fn_ok = fn_start == 0 || !is_ident_char(line_bytes[fn_start - 1]);
        // After the `n` must be whitespace (already confirmed: we walked past WS).
        before_fn_ok
    } else {
        false
    }
}

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

/// Returns true when `rel` is a pure test file — i.e. its path has a component
/// that is exactly `tests` (e.g. `tests/integration.rs`).  Pure test files are
/// entirely test code, so any owner mention anywhere in the file counts.
///
/// Files outside a `tests/` directory but containing `#[test]` (the dominant
/// Rust convention of an inline `#[cfg(test)] mod tests { … }` block) are
/// *mixed* files and require scope-aware matching; see `reach_in_mixed_file`.
fn is_pure_test_file(rel: &Path) -> bool {
    rel.components().any(|c| {
        c.as_os_str()
            .to_str()
            .is_some_and(|s| s == "tests" || s == "test")
    })
}

/// Scans `text` for an owner mention that is **inside** a `#[cfg(test)]` or
/// `#[test]`-gated scope.  Returns the `(test_name, line_number)` of the first
/// such mention, or `None` if no mention is found inside a test scope.
///
/// Scope tracking uses syntactic brace counting:
/// - A line containing `#[cfg(test)]` or `#[test]` starts a "pending test
///   attribute" state.
/// - The first `{` found while the attribute is pending opens the test scope
///   (depth = 1).  Subsequent `{` / `}` increment / decrement the depth.
/// - When depth returns to 0 the scope ends.
/// - An owner mention inside an open scope (depth > 0) credits test reach.
///
/// This is a source-text heuristic only.  It does not handle nested attributes,
/// string literals containing braces, or proc-macro-generated code.
fn reach_in_mixed_file(text: &str, owner: &str) -> Option<(String, usize)> {
    let mut last_test: Option<(String, usize)> = None;
    // True once we have seen a `#[cfg(test)]` / `#[test]` line but have not yet
    // entered the opening brace of the corresponding block.
    let mut pending_test_attr = false;
    // Nesting depth inside the current `#[cfg(test)]` / `#[test]` block.
    // 0 = outside any test scope.
    let mut test_depth: u32 = 0;

    for (idx, line) in text.lines().enumerate() {
        let line_no = idx + 1;

        // Detect a test-gating attribute.
        if line.contains("#[cfg(test)]") || line.contains("#[test]") {
            pending_test_attr = true;
        }

        // Track brace depth.
        for ch in line.chars() {
            match ch {
                '{' => {
                    if pending_test_attr {
                        // This brace opens the test scope.
                        test_depth += 1;
                        pending_test_attr = false;
                    } else if test_depth > 0 {
                        test_depth += 1;
                    }
                }
                '}' => {
                    test_depth = test_depth.saturating_sub(1);
                }
                _ => {}
            }
        }

        // Update the "last test seen" tracker (for naming the RelatedTest).
        if line.contains("#[test]") {
            last_test = Some(("test".to_string(), line_no));
        }
        if let Some(name) = parse_test_name(line) {
            last_test = Some((name, line_no));
        }

        // Credit reach only when we are inside a test scope AND the line has a
        // call/use shape (not a bare mention or a comment).
        if test_depth > 0 && line_has_owner_call_shape(line, owner) {
            let (name, ln) = last_test
                .clone()
                .unwrap_or_else(|| (format!("calls {owner}"), line_no));
            return Some((name, ln));
        }
    }
    None
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
        // Cheap whole-file prefilter: skip files that do not mention owner at all.
        if !text_contains_owner_as_ident(&text, owner) {
            continue;
        }

        let found = if is_pure_test_file(&rel) {
            // Pure test file (lives under a `tests/` directory): the entire file
            // is test code.  Any owner mention anywhere counts as test reach.
            // Preserve the existing per-line scan so we can capture a test name.
            let mut last_test: Option<(String, usize)> = None;
            let mut result = None;
            for (idx, line) in text.lines().enumerate() {
                let line_no = idx + 1;
                if line.contains("#[test]") {
                    last_test = Some(("test".to_string(), line_no));
                }
                if let Some(name) = parse_test_name(line) {
                    last_test = Some((name, line_no));
                }
                if line_has_owner_call_shape(line, owner) {
                    let (name, ln) = last_test
                        .clone()
                        .unwrap_or_else(|| (format!("calls {owner}"), line_no));
                    result = Some((name, ln));
                    break;
                }
            }
            result
        } else {
            // Mixed file (src/ file with an inline `#[cfg(test)] mod tests` block):
            // only credit the mention if it is inside a test-gated scope.
            reach_in_mixed_file(&text, owner)
        };

        if let Some((name, line_no)) = found {
            tests.push(RelatedTest {
                name,
                file: rel.to_string_lossy().replace('\\', "/"),
                line: line_no,
            });
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
