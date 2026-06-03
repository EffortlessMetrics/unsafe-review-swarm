use crate::domain::ReviewCard;
use crate::util::path_display;

use super::{TRUST_BOUNDARY, present_label};

pub(super) fn contents(card: &ReviewCard) -> String {
    let mut text = String::new();
    append_header(&mut text, card);
    append_context(&mut text, card);
    append_obligations(&mut text, card);
    append_evidence(&mut text, card);
    append_resolution_guidance(&mut text, card);
    append_witness_route(&mut text, card);
    append_handoff_commands(&mut text, card);
    text.push_str("\nTrust boundary: ");
    text.push_str(TRUST_BOUNDARY);
    text
}

fn append_header(text: &mut String, card: &ReviewCard) {
    text.push_str(&format!(
        "Card: `{}`; priority `{}`; confidence `{}`\n\n",
        card.id,
        card.priority.as_str(),
        card.confidence.as_str()
    ));
    text.push_str(&format!(
        "Location: {}:{}\n\n",
        path_display(&card.site.location.file),
        card.site.location.line
    ));
}

fn append_context(text: &mut String, card: &ReviewCard) {
    text.push_str("Why this card exists:\n");
    text.push_str(&format!(
        "- The changed code contains a `{}` unsafe operation that unsafe-review classifies as `{}`.\n",
        card.operation.family.as_str(),
        card.class.as_str()
    ));
    text.push_str(&format!("- Operation: `{}`\n\n", card.operation.expression));
    text.push_str(&format!("Proof path: `{}`\n\n", card.proof_path.as_str()));
    if !card.hazards.is_empty() {
        text.push_str("Relevant hazard families:\n");
        for hazard in &card.hazards {
            text.push_str(&format!("- `{}`\n", hazard.as_str()));
        }
        text.push('\n');
    }
}

fn append_obligations(text: &mut String, card: &ReviewCard) {
    text.push_str("Required safety conditions:\n");
    for obligation in &card.obligations {
        text.push_str(&format!("- {}\n", obligation.description));
    }
}

fn append_evidence(text: &mut String, card: &ReviewCard) {
    text.push_str("\nEvidence found:\n");
    text.push_str(&format!(
        "- Contract [{}]: {}\n",
        present_label(card.contract.present),
        card.contract.summary
    ));
    text.push_str(&format!(
        "- Guard/discharge [{}]: {}\n",
        present_label(card.discharge.present),
        card.discharge.summary
    ));
    text.push_str(&format!(
        "- Reach [{}]: {}\n",
        card.reach.state, card.reach.summary
    ));
    text.push_str(&format!(
        "- Witness [{}]: {}\n",
        present_label(card.witness.present),
        card.witness.summary
    ));
    text.push_str("\nEvidence missing:\n");
    if card.missing.is_empty() {
        text.push_str("- none recorded\n");
    } else {
        for missing in &card.missing {
            text.push_str(&format!("- {}\n", missing.message));
        }
    }
}

fn append_resolution_guidance(text: &mut String, card: &ReviewCard) {
    text.push_str("\nWhat would resolve this:\n");
    text.push_str(&format!("- {}\n", card.next_action.summary));
    if !card.next_action.verify_commands.is_empty() {
        text.push_str("\nVerify commands:\n");
        for command in &card.next_action.verify_commands {
            text.push_str(&format!("- `{command}`\n"));
        }
    }
    text.push_str("\nWhat would not resolve this:\n");
    text.push_str("- A `SAFETY:` comment alone does not discharge missing guard evidence.\n");
    text.push_str("- A related test mention is not proof that this unsafe site executed.\n");
    text.push_str("- Do not claim witness proof unless a matching receipt exists.\n");
    text.push_str("- Do not widen unsafe scope, suppress the card, or change unrelated unsafe code to silence this review item.\n");
}

fn append_witness_route(text: &mut String, card: &ReviewCard) {
    if let Some(route) = card.routes.first() {
        text.push_str(&format!(
            "\nWitness route: `{}` because {}.\n",
            route.kind.as_str(),
            route.reason
        ));
    }
    text.push_str(
        "\nReach note: static related-test evidence does not prove the unsafe site executed.\n",
    );
}

fn append_handoff_commands(text: &mut String, card: &ReviewCard) {
    text.push_str("\nHandoff commands:\n");
    text.push_str(&format!("- Explain: `unsafe-review explain {}`\n", card.id));
    text.push_str(&format!(
        "- Agent context: `unsafe-review context {} --json`\n",
        card.id
    ));
}
