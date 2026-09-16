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
        "cards: {}, open gaps: {}, contract_missing: {}, guard_missing: {}, witness gaps: {}\n",
        output.summary.cards,
        output.summary.open_actionable_gaps,
        output.summary.contract_missing,
        output.summary.guard_missing,
        output.summary.guarded_unwitnessed
    ));
    // Slot-level reconciliation (#2242): the class counters above count each
    // card once under its primary class, while cards list every missing slot
    // under `missing`. Count cards carrying each missing kind so the header
    // no longer reads as though no guard or witness work remains.
    let mut contract = 0usize;
    let mut guard = 0usize;
    let mut reach = 0usize;
    let mut witness = 0usize;
    for card in &output.cards {
        for missing in &card.missing {
            match missing.kind.as_str() {
                "contract" => contract += 1,
                "guard" => guard += 1,
                "reach" => reach += 1,
                "witness" => witness += 1,
                _ => {}
            }
        }
    }
    out.push_str(&format!(
        "slot gaps: contract: {contract}, guard: {guard}, reach: {reach}, witness: {witness}\n\n"
    ));
}
