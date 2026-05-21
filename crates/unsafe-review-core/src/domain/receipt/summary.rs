use super::WitnessReceipt;

pub(super) fn evidence_summary(receipt: &WitnessReceipt) -> String {
    let mut summary = format!(
        "Imported {} receipt with `{}` strength",
        receipt.tool, receipt.strength
    );
    append_optional(&mut summary, ": ", receipt.summary.as_deref());
    append_optional(&mut summary, "; author: ", receipt.author.as_deref());
    append_optional(
        &mut summary,
        "; recorded_at: ",
        receipt.recorded_at.as_deref(),
    );
    append_optional(
        &mut summary,
        "; expires_at: ",
        receipt.expires_at.as_deref(),
    );
    append_optional(&mut summary, "; command: ", receipt.command.as_deref());
    append_limitations(&mut summary, receipt.limitations.as_ref());
    summary
}

fn append_optional(summary: &mut String, prefix: &str, value: Option<&str>) {
    if let Some(value) = value.filter(|value| !value.is_empty()) {
        summary.push_str(prefix);
        summary.push_str(value);
    }
}

fn append_limitations(summary: &mut String, limitations: Option<&Vec<String>>) {
    if let Some(limitations) = limitations
        && !limitations.is_empty()
    {
        summary.push_str("; limitations: ");
        summary.push_str(&limitations.join("; "));
    }
}
