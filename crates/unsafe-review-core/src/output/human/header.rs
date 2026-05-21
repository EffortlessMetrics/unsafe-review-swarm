use crate::api::AnalyzeOutput;

pub(super) fn render_header(out: &mut String, output: &AnalyzeOutput) {
    out.push_str("unsafe-review\n");
    out.push_str(&format!(
        "scope: {:?}, mode: {}, policy: {}\n",
        output.scope,
        output.mode.as_str(),
        output.policy.as_str()
    ));
    out.push_str(&format!(
        "cards: {}, open gaps: {}, contract_missing: {}, guard_missing: {}, witness gaps: {}\n\n",
        output.summary.cards,
        output.summary.open_actionable_gaps,
        output.summary.contract_missing,
        output.summary.guard_missing,
        output.summary.guarded_unwitnessed
    ));
}
