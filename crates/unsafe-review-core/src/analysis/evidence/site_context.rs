use super::{compact_code, strip_block_comments_and_literals};
use crate::analysis::scanner::ScannedSite;

pub(super) fn code_context(site: &ScannedSite) -> String {
    site.context_before
        .iter()
        .chain(std::iter::once(&site.site.snippet))
        .chain(site.context_after.iter())
        .map(|line| {
            line.split_once("//")
                .map_or(line.as_str(), |(code, _comment)| code)
        })
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

/// Offset where the site snippet starts inside the lowercased,
/// literal-stripped, compacted full context. The site's own call is the
/// first matching marker at or after this offset; markers before it belong
/// to earlier sites sharing the context window.
pub(super) fn site_snippet_offset(site: &ScannedSite) -> usize {
    let before = site
        .context_before
        .iter()
        .map(|line| {
            line.split_once("//")
                .map_or(line.as_str(), |(code, _comment)| code)
        })
        .collect::<Vec<_>>()
        .join("\n");
    compact_code(&strip_block_comments_and_literals(
        &before.to_ascii_lowercase(),
    ))
    .len()
}
