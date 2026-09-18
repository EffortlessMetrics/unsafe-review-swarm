use super::site_context::code_context_before;
use super::{compact_code, strip_block_comments_and_literals};
use crate::analysis::scanner::ScannedSite;

/// Text before the site's own operation occurrence: the first occurrence at
/// or after the end of the site's context-before prefix. Falls back to the
/// first occurrence overall when the site's own occurrence cannot be located
/// (e.g. a caller-supplied expression that normalizes differently), which
/// preserves the legacy behavior instead of failing closed on new ground.
pub(super) fn code_before_site_operation(
    site: &ScannedSite,
    lower: &str,
    expression: &str,
) -> Option<String> {
    code_before_operation_at(lower, expression, site_anchor(site, lower))
}

/// Source-text counterpart of [`code_before_site_operation`]: same anchoring,
/// original whitespace preserved for binding analysis.
pub(super) fn source_before_site_operation(
    site: &ScannedSite,
    lower: &str,
    expression: &str,
) -> Option<String> {
    source_before_operation_at(lower, expression, site_anchor(site, lower))
}

fn code_before_operation_at(lower: &str, expression: &str, anchor: usize) -> Option<String> {
    let compact = compact_code(&strip_block_comments_and_literals(lower));
    let expression = compact_code(&expression.to_ascii_lowercase());
    if expression.is_empty() {
        return None;
    }
    operation_pos_at(&compact, &expression, anchor)
        .map(|operation_pos| compact[..operation_pos].to_string())
}

fn source_before_operation_at(lower: &str, expression: &str, anchor: usize) -> Option<String> {
    let cleaned = strip_block_comments_and_literals(lower);
    let expression = compact_code(&expression.to_ascii_lowercase());
    if expression.is_empty() {
        return None;
    }

    let mut compact = String::with_capacity(cleaned.len());
    let mut source_offsets = Vec::new();
    for (idx, ch) in cleaned.char_indices() {
        if !ch.is_ascii_whitespace() {
            compact.push(ch);
            source_offsets.push(idx);
        }
    }

    operation_pos_at(&compact, &expression, anchor)
        .map(|operation_pos| cleaned[..source_offsets[operation_pos]].to_string())
}

/// First occurrence of `expression` at or after `anchor`, falling back to the
/// first occurrence overall. The fallback keeps callers working when the
/// site's own occurrence is unreachable (unfamiliar lower assembly); the
/// anchor fixes the common case where identical operations share a window and
/// evidence would otherwise be donated across sites.
fn operation_pos_at(compact: &str, expression: &str, anchor: usize) -> Option<usize> {
    let mut first = None;
    for (pos, _) in compact.match_indices(expression) {
        if first.is_none() {
            first = Some(pos);
        }
        if pos >= anchor {
            return Some(pos);
        }
    }
    first
}

/// Compact offset where the site's own snippet starts within `lower`, or 0
/// when `lower` was not assembled from this site's context (the prefix check
/// fails and callers transparently keep legacy first-match behavior).
fn site_anchor(site: &ScannedSite, lower: &str) -> usize {
    let before = compact_code(&strip_block_comments_and_literals(
        &code_context_before(site).to_ascii_lowercase(),
    ));
    let compact = compact_code(&strip_block_comments_and_literals(lower));
    if !before.is_empty() && compact.starts_with(&before) {
        before.len()
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::{code_before_site_operation, source_before_site_operation};
    use crate::analysis::scanner::ScannedSite;
    use crate::domain::{
        OperationFamily, SourceLocation, SourceRole, UnsafeOperation, UnsafeSite, UnsafeSiteKind,
    };

    fn site_with_context(context_before: Vec<&str>, snippet: &str) -> ScannedSite {
        ScannedSite {
            site: UnsafeSite {
                location: SourceLocation::new(std::path::PathBuf::from("src/lib.rs"), 1, 1),
                kind: UnsafeSiteKind::Operation,
                owner: Some("probe_fn".to_string()),
                visibility: "private".to_string(),
                public_api_surface: false,
                changed: true,
                snippet: snippet.to_string(),
                role: SourceRole::Unknown,
            },
            operation: UnsafeOperation {
                family: OperationFamily::MaybeUninitAssumeInit,
                expression: snippet.to_string(),
                bound_name: None,
            },
            context_before: context_before.into_iter().map(str::to_string).collect(),
            context_after: Vec::new(),
        }
    }

    #[test]
    fn ignores_comments_and_literals_when_locating_operation() -> Result<(), String> {
        let site = site_with_context(
            vec![
                "// unsafe { ptr.read() }",
                "let _note = \"unsafe { ptr.read() }\";",
                "assert!(idx < len);",
            ],
            "unsafe { ptr.read() }",
        );
        let lower = "// unsafe { ptr.read() }\nlet _note = \"unsafe { ptr.read() }\";\nassert!(idx < len);\nunsafe { ptr.read() }";
        let before = code_before_site_operation(&site, lower, "unsafe { ptr.read() }")
            .ok_or_else(|| "operation should be found".to_string())?;

        assert!(before.contains("assert!(idx<len);"));
        Ok(())
    }

    #[test]
    fn source_before_operation_preserves_binding_whitespace() -> Result<(), String> {
        let site = site_with_context(
            vec!["let mut slot: MaybeUninit<u32> = MaybeUninit::<u32>::new(7);"],
            "unsafe { slot.assume_init_read() }",
        );
        let lower = "let mut slot: MaybeUninit<u32> = MaybeUninit::<u32>::new(7);\nunsafe { slot.assume_init_read() }";
        let before =
            source_before_site_operation(&site, lower, "unsafe { slot.assume_init_read() }")
                .ok_or_else(|| "operation should be found".to_string())?;

        assert!(before.contains("let mut slot: MaybeUninit<u32>"));
        assert!(!before.contains("letmutslot"));
        Ok(())
    }

    #[test]
    fn site_operation_anchors_on_own_occurrence_not_first_duplicate() -> Result<(), String> {
        // Two textually identical operations share one context window (same
        // slot name reused in a later scope). Legacy first-match anchoring
        // stops at the earlier occurrence and cuts off the second site's own
        // binding; the site anchor must reach it.
        let lower = [
            "let mut slot = maybeuninit::<u32>::uninit();",
            "slot.write(1);",
            "unsafe { slot.assume_init() }",
            "let mut slot = maybeuninit::<u32>::new(7);",
            "unsafe { slot.assume_init() }",
        ]
        .join("\n");
        let second = site_with_context(
            vec![
                "let mut slot = maybeuninit::<u32>::uninit();",
                "slot.write(1);",
                "unsafe { slot.assume_init() }",
                "let mut slot = maybeuninit::<u32>::new(7);",
            ],
            "unsafe { slot.assume_init() }",
        );

        let before = code_before_site_operation(&second, &lower, "unsafe { slot.assume_init() }")
            .ok_or_else(|| "operation should be found".to_string())?;
        assert!(before.contains("new(7)"), "own binding cut off: {before}");

        // An empty context carries no anchor: first-match behavior is
        // preserved exactly for callers that cannot name the site.
        let unanchored = site_with_context(vec![], "unsafe { slot.assume_init() }");
        let legacy =
            code_before_site_operation(&unanchored, &lower, "unsafe { slot.assume_init() }")
                .ok_or_else(|| "operation should be found".to_string())?;
        assert!(
            !legacy.contains("new(7)"),
            "legacy anchor changed: {legacy}"
        );
        Ok(())
    }

    #[test]
    fn site_operation_falls_back_to_first_match_when_own_is_absent() -> Result<(), String> {
        // A caller-supplied expression matching only an earlier site has no
        // occurrence at the anchor: keep legacy first-match behavior instead
        // of newly returning None.
        let lower = [
            "unsafe { first.assume_init() }",
            "let y = 2;",
            "unsafe { second.assume_init() }",
        ]
        .join("\n");
        let second = site_with_context(
            vec!["unsafe { first.assume_init() }", "let y = 2;"],
            "unsafe { second.assume_init() }",
        );

        let before = code_before_site_operation(&second, &lower, "unsafe { first.assume_init() }")
            .ok_or_else(|| "operation should be found".to_string())?;
        assert!(before.is_empty(), "fallback lost: {before}");
        Ok(())
    }

    #[test]
    fn source_site_operation_preserves_own_site_whitespace() -> Result<(), String> {
        let lower = [
            "let mut slot = maybeuninit::<u32>::uninit();",
            "unsafe { slot.assume_init() }",
            "let mut slot: maybeuninit<u32> = maybeuninit::<u32>::new(7);",
            "unsafe { slot.assume_init() }",
        ]
        .join("\n");
        let second = site_with_context(
            vec![
                "let mut slot = maybeuninit::<u32>::uninit();",
                "unsafe { slot.assume_init() }",
                "let mut slot: maybeuninit<u32> = maybeuninit::<u32>::new(7);",
            ],
            "unsafe { slot.assume_init() }",
        );

        let before = source_before_site_operation(&second, &lower, "unsafe { slot.assume_init() }")
            .ok_or_else(|| "operation should be found".to_string())?;
        assert!(
            before.contains("let mut slot: maybeuninit<u32>"),
            "own binding missing: {before}"
        );
        assert!(
            !before.contains("letmutslot:"),
            "whitespace not preserved: {before}"
        );
        Ok(())
    }
}
