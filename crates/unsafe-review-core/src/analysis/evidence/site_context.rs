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
