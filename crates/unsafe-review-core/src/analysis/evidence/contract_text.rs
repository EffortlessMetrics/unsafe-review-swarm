use crate::analysis::scanner::ScannedSite;
use crate::domain::ContractEvidence;

pub(crate) fn contract_evidence(site: &ScannedSite) -> ContractEvidence {
    let context = site.context_before.join("\n");
    if let Some(summary) = safety_doc_summary(&context) {
        return ContractEvidence::present(summary);
    }
    if site.site.public_api_surface {
        return ContractEvidence::missing_with(
            "Public unsafe API is missing nearby `# Safety` documentation",
        );
    }
    if site.site.visibility == "restricted" {
        // pub(crate)/pub(super)/pub(in …) — not public API but still callable
        // by in-crate callers; a missing contract is a real gap.
        return ContractEvidence::missing_with(
            "Restricted-visibility unsafe fn is missing nearby `# Safety` documentation for in-crate callers",
        );
    }
    if let Some(summary) = safety_comment_summary(&context, &site.site.snippet) {
        return ContractEvidence::present(summary);
    }
    ContractEvidence::missing()
}

fn safety_doc_summary(context: &str) -> Option<&'static str> {
    for line in context.lines() {
        let trimmed = line.trim_start();
        if !(trimmed.starts_with("///")
            || trimmed.starts_with("//!")
            || trimmed.starts_with("#[doc"))
        {
            continue;
        }
        if trimmed.contains("# Safety") {
            return Some("Nearby `# Safety` documentation was detected");
        }
        if trimmed.contains("Safety:") {
            return Some("Nearby `Safety:` documentation was detected");
        }
    }
    None
}

fn safety_comment_summary(context: &str, snippet: &str) -> Option<&'static str> {
    // The unsafe site's own trailing comment always documents this site.
    if let Some(hit) = safety_marker_in(snippet.lines()) {
        return Some(hit);
    }
    // Scan the before-context upward from the site, but stop at the first scope
    // boundary — a prior `unsafe` statement or a closing brace. A `// SAFETY:`
    // comment that documents an earlier unsafe block, or a sibling item pulled
    // into the flat proximity window, is not owned by this (uncommented) site
    // and must not be credited as its contract evidence (`comment != guard`,
    // and only same-site rationale counts).
    for line in context.lines().rev() {
        let trimmed = line.trim_start();
        if is_attribution_boundary(trimmed) {
            break;
        }
        if let Some(hit) = safety_marker(trimmed) {
            return Some(hit);
        }
    }
    None
}

/// Detect a `SAFETY:` / `Safety:` line comment (not a doc comment) on a single
/// already-trimmed line. Returns the canonical summary string when matched.
fn safety_marker(trimmed: &str) -> Option<&'static str> {
    if trimmed.starts_with("///") || trimmed.starts_with("//!") {
        return None;
    }
    if !(trimmed.starts_with("//")
        || trimmed.contains("// SAFETY:")
        || trimmed.contains("// Safety:"))
    {
        return None;
    }
    if trimmed.contains("SAFETY:") {
        return Some("Nearby `SAFETY:` comment was detected");
    }
    if trimmed.contains("Safety:") {
        return Some("Nearby `Safety:` comment was detected");
    }
    None
}

fn safety_marker_in<'a>(lines: impl Iterator<Item = &'a str>) -> Option<&'static str> {
    for line in lines {
        if let Some(hit) = safety_marker(line.trim_start()) {
            return Some(hit);
        }
    }
    None
}

/// A line in the before-context that ends the current site's comment-attribution
/// scope: a closing brace (a prior block/scope), or a *complete* prior `unsafe`
/// statement that both opens and closes on the same line (e.g.
/// `let a = unsafe { *p };`). The current site's own multi-line `unsafe {`
/// opener has no closing brace on its line and is therefore NOT a boundary, so
/// a `// SAFETY:` comment above a multi-line `unsafe { … }` block still counts.
/// Comment lines are never boundaries, and `unsafe` is matched as a whole token
/// so identifiers like `unsafe_helper` do not trip the boundary.
fn is_attribution_boundary(trimmed: &str) -> bool {
    if trimmed.starts_with("//") {
        return false;
    }
    if trimmed.starts_with('}') {
        return true;
    }
    let has_unsafe_keyword = trimmed
        .split(|ch: char| !ch.is_alphanumeric() && ch != '_')
        .any(|token| token == "unsafe");
    has_unsafe_keyword && trimmed.contains('}')
}

#[cfg(test)]
mod tests {
    use super::{safety_comment_summary, safety_doc_summary};

    #[test]
    fn safety_doc_summary_accepts_doc_safety_headings() {
        for (context, expected) in [
            (
                "/// # Safety\n/// Caller must uphold the pointer contract.",
                "Nearby `# Safety` documentation was detected",
            ),
            (
                "//! Safety: module invariants describe the unsafe boundary.",
                "Nearby `Safety:` documentation was detected",
            ),
            (
                "#[doc = \"# Safety\"]",
                "Nearby `# Safety` documentation was detected",
            ),
        ] {
            assert_eq!(safety_doc_summary(context), Some(expected));
        }
    }

    #[test]
    fn safety_doc_summary_ignores_non_doc_safety_comments() {
        assert_eq!(
            safety_doc_summary("// SAFETY: local unsafe block is guarded here"),
            None
        );
        assert_eq!(safety_doc_summary("let note = \"# Safety\";"), None);
    }

    #[test]
    fn safety_comment_summary_accepts_line_comments_near_site() {
        assert_eq!(
            safety_comment_summary("// SAFETY: len was checked before indexing", ""),
            Some("Nearby `SAFETY:` comment was detected")
        );
        assert_eq!(
            safety_comment_summary("", "unsafe { ptr.read() } // Safety: ptr is live"),
            Some("Nearby `Safety:` comment was detected")
        );
    }

    #[test]
    fn safety_comment_summary_ignores_docs_and_unmarked_comments() {
        assert_eq!(
            safety_comment_summary("/// # Safety\n/// Public contract.", ""),
            None
        );
        assert_eq!(
            safety_comment_summary("// safe because this comment lacks the marker", ""),
            None
        );
    }

    #[test]
    fn safety_comment_not_credited_across_a_prior_unsafe_block() {
        // The `// SAFETY:` documents the earlier `unsafe { *p }` block; a later,
        // uncommented `unsafe { *q }` site in the same window must not inherit it.
        let context = "// SAFETY: p is valid and aligned.\nlet a = unsafe { *p };";
        assert_eq!(
            safety_comment_summary(context, "let b = unsafe { *q };"),
            None
        );
    }

    #[test]
    fn safety_comment_not_credited_across_a_closing_brace() {
        // The `// SAFETY:` belongs to a sibling item above the closing brace.
        let context = "// SAFETY: unrelated sibling rationale.\n    let _ = 1;\n}";
        assert_eq!(safety_comment_summary(context, "unsafe { *p }"), None);
    }

    #[test]
    fn safety_comment_credited_when_attached_to_the_site() {
        // A contiguous `// SAFETY:` immediately above the site (only ordinary
        // setup code before it, no intervening unsafe/brace) is still credited.
        let context = "assert!(new_len <= values.capacity());\n// SAFETY: initialized above.";
        assert_eq!(
            safety_comment_summary(context, "unsafe { values.set_len(new_len) }"),
            Some("Nearby `SAFETY:` comment was detected")
        );
    }

    #[test]
    fn safety_marker_in_string_literal_is_not_a_comment() {
        // `SAFETY:` inside a string literal (no `//`) is not comment evidence.
        assert_eq!(
            safety_comment_summary("let reason = \"SAFETY: bounds are checked\";", ""),
            None
        );
    }

    #[test]
    fn unsafe_substring_identifier_is_not_a_boundary() {
        // `unsafe_helper` is an identifier, not an `unsafe` block boundary, so a
        // `// SAFETY:` above it still attaches to the site below.
        let context = "// SAFETY: checked by unsafe_helper().\nlet v = unsafe_helper();";
        assert_eq!(
            safety_comment_summary(context, "unsafe { v.get() }"),
            Some("Nearby `SAFETY:` comment was detected")
        );
    }
}
