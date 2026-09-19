use crate::analysis::scanner::ScannedSite;

fn strip_line_comment(line: &str) -> &str {
    line.split_once("//").map_or(line, |(code, _comment)| code)
}

pub(crate) fn code_context(site: &ScannedSite) -> String {
    site.context_before
        .iter()
        .chain(std::iter::once(&site.site.snippet))
        .chain(site.context_after.iter())
        .map(|line| strip_line_comment(line))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The `code_context` prefix ending where the site's own snippet starts.
/// Evidence anchoring uses this to resolve the site's own operation
/// occurrence instead of the first textually identical one.
pub(super) fn code_context_before(site: &ScannedSite) -> String {
    site.context_before
        .iter()
        .map(|line| strip_line_comment(line))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn code_context_through_site(site: &ScannedSite) -> String {
    site.context_before
        .iter()
        .chain(std::iter::once(&site.site.snippet))
        .map(|line| {
            line.split_once("//")
                .map_or(line.as_str(), |(code, _comment)| code)
        })
        .collect::<Vec<_>>()
        .join("\n")
}
