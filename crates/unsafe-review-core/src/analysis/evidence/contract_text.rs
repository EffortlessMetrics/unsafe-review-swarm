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
    for line in context.lines().chain(snippet.lines()) {
        let trimmed = line.trim_start();
        if trimmed.starts_with("///") || trimmed.starts_with("//!") {
            continue;
        }
        if !(trimmed.starts_with("//")
            || trimmed.contains("// SAFETY:")
            || trimmed.contains("// Safety:"))
        {
            continue;
        }
        if trimmed.contains("SAFETY:") {
            return Some("Nearby `SAFETY:` comment was detected");
        }
        if trimmed.contains("Safety:") {
            return Some("Nearby `Safety:` comment was detected");
        }
    }
    None
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
}
