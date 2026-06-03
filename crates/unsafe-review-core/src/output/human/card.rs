use crate::domain::ReviewCard;
use crate::util::path_display;

pub(super) fn render_card(out: &mut String, card: &ReviewCard) {
    out.push_str(&format!(
        "{} {}:{}\n",
        card.class.as_str().to_uppercase(),
        path_display(&card.site.location.file),
        card.site.location.line
    ));
    out.push_str(&format!("  id: {}\n", card.id));
    out.push_str(&format!("  operation: {}\n", card.operation.expression));
    out.push_str(&format!(
        "  operation_family: {}\n",
        card.operation.family.as_str()
    ));
    out.push_str(&format!("  proof_path: {}\n", card.proof_path.as_str()));
    push_list(
        out,
        "  hazards:\n",
        card.hazards.iter().map(|hazard| hazard.as_str()),
    );
    push_list(
        out,
        "  required safety conditions:\n",
        card.obligations
            .iter()
            .map(|obligation| obligation.description.as_str()),
    );
    out.push_str(&format!("  contract: {}\n", card.contract.summary));
    out.push_str(&format!("  discharge: {}\n", card.discharge.summary));
    out.push_str(&format!("  reach: {}\n", card.reach.summary));
    out.push_str("  reach note: static reach evidence only; it does not prove site execution.\n");
    if !card.obligation_evidence.is_empty() {
        out.push_str("  obligation evidence:\n");
        for evidence in &card.obligation_evidence {
            out.push_str(&format!(
                "    - {}: contract {}, guard {}, reach {}, witness {}\n",
                evidence.obligation.key,
                evidence.contract.state,
                evidence.discharge.state,
                evidence.reach.state,
                evidence.witness.state
            ));
        }
    }
    push_list(
        out,
        "  missing:\n",
        card.missing.iter().map(|missing| missing.message.as_str()),
    );
    if !card.routes.is_empty() {
        out.push_str("  witness routes:\n");
        for route in &card.routes {
            out.push_str(&format!(
                "    - {}: {}\n",
                route.kind.as_str(),
                route.reason
            ));
            if let Some(command) = &route.command {
                out.push_str(&format!("      command: {}\n", command));
            }
        }
    }
    out.push_str(&format!("  next: {}\n", card.next_action.summary));
    if !card.next_action.verify_commands.is_empty() {
        out.push_str("  verify:\n");
        for cmd in &card.next_action.verify_commands {
            out.push_str(&format!("    {}\n", cmd));
        }
    }
    out.push('\n');
}

fn push_list<'a>(out: &mut String, title: &str, lines: impl Iterator<Item = &'a str>) {
    out.push_str(title);
    for line in lines {
        out.push_str("    - ");
        out.push_str(line);
        out.push('\n');
    }
}
