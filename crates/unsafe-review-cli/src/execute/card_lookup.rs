use std::path::Path;
use unsafe_review_core::{
    AnalysisMode, AnalyzeInput, CardId, DiffSource, PolicyMode, Scope, analyze, collect_context,
    explain_card, load_manual_candidate, render_manual_candidate_context,
    render_manual_candidate_explain,
};

use super::first_pr;

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

pub(super) fn manual_candidate_explain(root: &Path, id: &str) -> Result<Option<String>, String> {
    Ok(load_manual_candidate(root, id)?
        .map(|candidate| render_manual_candidate_explain(&candidate)))
}

pub(super) fn manual_candidate_context(root: &Path, id: &str) -> Result<Option<String>, String> {
    let Some(candidate) = load_manual_candidate(root, id)? else {
        return Ok(None);
    };
    let context = render_manual_candidate_context(&candidate)?;
    let mut value = serde_json::from_str::<serde_json::Value>(&context)
        .map_err(|err| format!("parse manual candidate context failed: {err}"))?;
    if let Some(object) = value.as_object_mut() {
        let (seed_source, seed) =
            first_pr::manual_candidate_context_seed_projection(root, &candidate);
        object.insert("stable_byte_seed_source".to_string(), seed_source);
        if let Some(seed) = seed {
            object.insert("stable_byte_seed".to_string(), seed);
        }
    }
    let mut rendered = serde_json::to_string_pretty(&value)
        .map_err(|err| format!("render manual candidate context failed: {err}"))?;
    rendered.push('\n');
    Ok(Some(rendered))
}
