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
