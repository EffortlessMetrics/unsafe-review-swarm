use crate::analysis::scanner::text_detection::{LineCommentState, split_code_and_comment};
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
/// Every line is first reduced to its code portion with
/// [`split_code_and_comment`], so call shapes, attributes, and braces inside
/// comments and string/char literals cannot open test scope, close it, or
/// supply evidence.  Scope tracking then uses brace counting on that code:
/// - A line containing `#[cfg(test)]` or `#[test]` starts a "pending test
///   attribute" state.
/// - The first `{` found while the attribute is pending opens the test scope
///   (depth = 1).  Subsequent `{` / `}` increment / decrement the depth.
/// - When depth returns to 0 the scope ends.
/// - A line that both opens a scope and carries a call (a one-line test
///   function) credits reach: entering the scope on the line counts.
/// - An owner mention inside an open scope (depth > 0) credits test reach.
///
/// Test naming only trusts `#[test]`-attributed functions: a nested ordinary
/// helper cannot overwrite the enclosing test's name, and definitions outside
/// test scope never become names.
///
/// This is a source-text heuristic only.  It does not handle proc-macro
/// generated code.
fn reach_in_mixed_file(text: &str, owner: &str) -> Option<(String, usize)> {
    let mut last_test: Option<(String, usize)> = None;
    // True once we have seen a `#[cfg(test)]` / `#[test]` line but have not yet
    // entered the opening brace of the corresponding block.
    let mut pending_test_attr = false;
    // True once we have seen a `#[test]` line whose function name has not been
    // attributed yet.  Only a function parsed while a test scope is open (or
    // being entered) consumes it.
    let mut pending_test_name = false;
    // Nesting depth inside the current `#[cfg(test)]` / `#[test]` block.
    // 0 = outside any test scope.
    let mut test_depth: u32 = 0;
    let mut mask_state = LineCommentState::default();

    for (idx, line) in text.lines().enumerate() {
        let line_no = idx + 1;
        // Code portion only: comments and literal contents are gone, so they
        // can neither match a call shape nor move scope tracking.
        let code = split_code_and_comment(line, &mut mask_state).0;

        // Detect a test-gating attribute.
        if code.contains("#[cfg(test)]") || code.contains("#[test]") {
            pending_test_attr = true;
        }
        if code.contains("#[test]") {
            pending_test_name = true;
        }

        // Track brace depth.  A line that opens the scope below also counts
        // as inside it, so one-line test functions are not missed.
        let mut entered_scope_this_line = false;
        for ch in code.chars() {
            match ch {
                '{' => {
                    if pending_test_attr {
                        // This brace opens the test scope.
                        test_depth += 1;
                        pending_test_attr = false;
                        entered_scope_this_line = true;
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
        // Only `#[test]`-attributed functions establish the name; a nested
        // helper fills it in only when no test name is known, and definitions
        // outside test scope never do.
        if let Some(name) = test_fn_name(&code) {
            if pending_test_name && (test_depth > 0 || entered_scope_this_line) {
                last_test = Some((name, line_no));
                pending_test_name = false;
            } else {
                pending_test_name = false;
                if last_test.is_none() && test_depth > 0 {
                    last_test = Some((name, line_no));
                }
            }
        }

        // Credit reach only when we are inside a test scope AND the line has a
        // call/use shape (not a bare mention or a comment).
        if (test_depth > 0 || entered_scope_this_line) && line_has_owner_call_shape(&code, owner) {
            let (name, ln) = last_test
                .clone()
                .unwrap_or_else(|| (format!("calls {owner}"), line_no));
            return Some((name, ln));
        }
    }
    None
}

/// Parse a test function name from a masked code line, including the
/// `#[test] fn name() { ... }` one-line shape.  Returns `None` for
/// non-function lines.
fn test_fn_name(code: &str) -> Option<String> {
    if let Some(name) = parse_test_name(code) {
        return Some(name);
    }
    // Attribute-prefixed definition on one line: parse past the attribute.
    let after_attr = code.find(']')?;
    parse_test_name(code[after_attr + 1..].trim_start())
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
            // Lines are masked first so comment/string call shapes cannot supply
            // evidence and `#[test]` text in prose cannot fabricate a name.
            let mut mask_state = LineCommentState::default();
            let mut last_test: Option<(String, usize)> = None;
            let mut result = None;
            for (idx, line) in text.lines().enumerate() {
                let line_no = idx + 1;
                let code = split_code_and_comment(line, &mut mask_state).0;
                if let Some(name) = test_fn_name(&code) {
                    last_test = Some((name, line_no));
                }
                if line_has_owner_call_shape(&code, owner) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn mixed_case(body: &str) -> String {
        format!("#[cfg(test)]\nmod tests {{\n{body}\n}}\n")
    }

    #[test]
    fn multiline_and_one_line_test_calls_are_equivalent() {
        let multiline =
            mixed_case("    #[test]\n    fn checks_target() {\n        target();\n    }\n");
        let one_line = mixed_case("    #[test] fn checks_target() { target() }\n");
        let multi = reach_in_mixed_file(&multiline, "target");
        let single = reach_in_mixed_file(&one_line, "target");
        assert_eq!(
            multi.as_ref().map(|(name, _)| name.as_str()),
            Some("checks_target")
        );
        assert_eq!(
            single.as_ref().map(|(name, _)| name.as_str()),
            Some("checks_target"),
            "one-line test must attribute the same test name"
        );
        assert_eq!(
            multi.is_some(),
            single.is_some(),
            "one-line and multiline calls must agree on evidence"
        );
    }

    #[test]
    fn raw_call_shape_matches_comments_so_masking_is_required() {
        // The bare predicate still matches comment/string shapes on raw
        // lines; every caller must feed it masked code. If this ever goes
        // false, the masking call sites deserve a second look, not applause.
        assert!(line_has_owner_call_shape("// target()", "target"));
        assert!(line_has_owner_call_shape("let _ = \"target()\";", "target"));
    }

    #[test]
    fn comment_only_call_shape_supplies_no_evidence() {
        let text = mixed_case(
            "    #[test]\n    fn checks_target() {\n        // target()\n        let _ = 1;\n    }\n",
        );
        assert_eq!(
            reach_in_mixed_file(&text, "target"),
            None,
            "comment-only call shape must not count"
        );
    }

    #[test]
    fn string_only_call_shapes_supply_no_evidence() {
        let text = mixed_case(
            "    #[test]\n    fn checks_target() {\n        let _ = \"target()\";\n        let _ = r#\"target()\"#;\n        let _ = 'x';\n    }\n",
        );
        assert_eq!(
            reach_in_mixed_file(&text, "target"),
            None,
            "string/char call shapes must not count"
        );
    }

    #[test]
    fn real_call_with_string_decoy_still_counts() {
        let text = mixed_case(
            "    #[test]\n    fn checks_target() {\n        let _ = \"target()\"; target(); // trailing\n    }\n",
        );
        let found = reach_in_mixed_file(&text, "target");
        assert_eq!(
            found.as_ref().map(|(name, _)| name.as_str()),
            Some("checks_target")
        );
    }

    #[test]
    fn attribute_text_in_comment_opens_no_scope() {
        let text = "fn production() {\n    // #[test]\n    target();\n}\n";
        assert_eq!(
            reach_in_mixed_file(text, "target"),
            None,
            "commented attribute must not open test scope"
        );
    }

    #[test]
    fn braces_in_strings_do_not_move_scope() {
        // The `}` inside the string must not close the test scope before the
        // real call; the `{` must not open one in production code.
        let text = mixed_case(
            "    #[test]\n    fn checks_target() {\n        let _ = \"}\";\n        target();\n    }\n",
        );
        assert_eq!(
            reach_in_mixed_file(&text, "target")
                .as_ref()
                .map(|(name, _)| name.as_str()),
            Some("checks_target")
        );
        let production = "fn production() {\n    let _ = \"{\";\n    target();\n}\n";
        assert_eq!(
            reach_in_mixed_file(production, "target"),
            None,
            "string brace must not open scope in production code"
        );
    }

    #[test]
    fn production_call_outside_test_scope_is_not_reach() {
        let text = "fn production() {\n    target();\n}\n#[cfg(test)]\nmod tests {\n}\n";
        assert_eq!(
            reach_in_mixed_file(text, "target"),
            None,
            "production call must not count even with a test module present"
        );
    }

    #[test]
    fn definition_and_unrelated_text_are_not_calls() {
        // The `fn target` definition must not count as a call site, and a
        // bare mention without a call shape must not either.
        let text = mixed_case(
            "    fn target() {}\n    #[test]\n    fn mentions() {\n        let _ = target;\n    }\n",
        );
        assert_eq!(
            reach_in_mixed_file(&text, "target"),
            None,
            "definitions and bare mentions are not calls"
        );
    }

    #[test]
    fn nested_helper_cannot_overwrite_enclosing_test_name() {
        let text = mixed_case(
            "    #[test]\n    fn checks_target() {\n        target();\n        fn helper() {}\n    }\n",
        );
        assert_eq!(
            reach_in_mixed_file(&text, "target")
                .as_ref()
                .map(|(name, _)| name.as_str()),
            Some("checks_target"),
            "nested helper must not overwrite the enclosing test name"
        );
    }

    fn write_temp_source(prefix: &str, files: &[(&str, &str)]) -> Result<PathBuf, String> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system clock before UNIX_EPOCH: {err}"))?
            .as_nanos();
        let root = std::env::temp_dir().join(format!("{prefix}-{nanos}"));
        for (rel, contents) in files {
            let path = root.join(rel);
            let parent = path
                .parent()
                .ok_or_else(|| format!("no parent for {}", path.display()))?;
            fs::create_dir_all(parent)
                .map_err(|err| format!("create temp parent failed: {err}"))?;
            fs::write(&path, contents).map_err(|err| format!("write temp source failed: {err}"))?;
        }
        Ok(root)
    }

    #[test]
    fn pipeline_comment_only_fixture_is_unreached() -> Result<(), String> {
        let root = write_temp_source(
            "unsafe-review-reach-comment-only",
            &[(
                "tests/target.rs",
                "#[test]\nfn checks_target() {\n    // target()\n}\n",
            )],
        )?;
        let (evidence, related) = reach_evidence(&root, Some(&"target".to_string()));
        let _ = fs::remove_dir_all(&root);
        assert_eq!(evidence.state, "unreached");
        assert!(related.is_empty());
        Ok(())
    }

    #[test]
    fn pipeline_real_call_names_the_test() -> Result<(), String> {
        let root = write_temp_source(
            "unsafe-review-reach-real-call",
            &[(
                "tests/target.rs",
                "#[test]\nfn checks_target() {\n    target();\n}\n",
            )],
        )?;
        let (evidence, related) = reach_evidence(&root, Some(&"target".to_string()));
        let _ = fs::remove_dir_all(&root);
        assert_eq!(evidence.state, "owner_reached");
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].name, "checks_target");
        Ok(())
    }
}
