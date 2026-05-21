use std::path::Path;
use unsafe_review_core::{
    AnalysisMode, AnalyzeInput, CardId, DiffSource, PolicyMode, Scope, analyze, collect_context,
    explain_card,
};

pub(super) fn analyze_repo_cards(root: &Path) -> Result<unsafe_review_core::AnalyzeOutput, String> {
    analyze(AnalyzeInput {
        root: root.to_path_buf(),
        scope: Scope::Repo,
        diff: DiffSource::NoneRepoScan,
        mode: AnalysisMode::Repo,
        policy: PolicyMode::Advisory,
        include_unchanged_tests: true,
        max_cards: None,
    })
}

pub(super) fn explain_text(
    output: &unsafe_review_core::AnalyzeOutput,
    id: &CardId,
) -> Result<String, String> {
    explain_card(output, id).ok_or_else(|| format!("card `{id}` not found"))
}

pub(super) fn context_packet(
    output: &unsafe_review_core::AnalyzeOutput,
    id: &CardId,
) -> Result<String, String> {
    collect_context(output, id).ok_or_else(|| format!("card `{id}` not found"))
}
