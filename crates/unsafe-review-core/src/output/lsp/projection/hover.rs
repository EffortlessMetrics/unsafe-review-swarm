use crate::domain::ReviewCard;
use crate::util::path_display;

use super::{TRUST_BOUNDARY, present_label};

pub(super) fn contents(card: &ReviewCard) -> String {
    let mut text = String::new();
    push_header(&mut text, card);
    push_reason(&mut text, card);
    push_required_conditions(&mut text, card);
    push_evidence_found(&mut text, card);
    push_evidence_missing(&mut text, card);
    push_resolution_guidance(&mut text, card);
    push_non_resolution_guidance(&mut text);
    push_witness_route(&mut text, card);
    push_reach_note(&mut text);
    push_handoff_commands(&mut text, card);
    push_trust_boundary(&mut text);
    text
}

fn push_header(text: &mut String, card: &ReviewCard) {
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

fn push_reason(text: &mut String, card: &ReviewCard) {
    text.push_str("Why this card exists:\n");
    text.push_str(&format!(
        "- The changed code contains a `{}` unsafe operation that unsafe-review classifies as `{}`.\n",
        card.operation.family.as_str(),
        card.class.as_str()
    ));
    text.push_str(&format!("- Operation: `{}`\n\n", card.operation.expression));
    if !card.hazards.is_empty() {
        text.push_str("Relevant hazard families:\n");
        for hazard in &card.hazards {
            text.push_str(&format!("- `{}`\n", hazard.as_str()));
        }
        text.push('\n');
    }
}

fn push_required_conditions(text: &mut String, card: &ReviewCard) {
    text.push_str("Required safety conditions:\n");
    for obligation in &card.obligations {
        text.push_str(&format!("- {}\n", obligation.description));
    }
}

fn push_evidence_found(text: &mut String, card: &ReviewCard) {
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
}

fn push_evidence_missing(text: &mut String, card: &ReviewCard) {
    text.push_str("\nEvidence missing:\n");
    if card.missing.is_empty() {
        text.push_str("- none recorded\n");
    } else {
        for missing in &card.missing {
            text.push_str(&format!("- {}\n", missing.message));
        }
    }
}

fn push_resolution_guidance(text: &mut String, card: &ReviewCard) {
    text.push_str("\nWhat would resolve this:\n");
    text.push_str(&format!("- {}\n", card.next_action.summary));
    if !card.next_action.verify_commands.is_empty() {
        text.push_str("\nVerify commands:\n");
        for command in &card.next_action.verify_commands {
            text.push_str(&format!("- `{command}`\n"));
        }
    }
}

fn push_non_resolution_guidance(text: &mut String) {
    text.push_str("\nWhat would not resolve this:\n");
    text.push_str("- A `SAFETY:` comment alone does not discharge missing guard evidence.\n");
    text.push_str("- A related test mention is not proof that this unsafe site executed.\n");
    text.push_str("- Do not claim witness proof unless a matching receipt exists.\n");
    text.push_str("- Do not widen unsafe scope, suppress the card, or change unrelated unsafe code to silence this review item.\n");
}

fn push_witness_route(text: &mut String, card: &ReviewCard) {
    if let Some(route) = card.routes.first() {
        text.push_str(&format!(
            "\nWitness route: `{}` because {}.\n",
            route.kind.as_str(),
            route.reason
        ));
    }
}

fn push_reach_note(text: &mut String) {
    text.push_str(
        "\nReach note: static related-test evidence does not prove the unsafe site executed.\n",
    );
}

fn push_handoff_commands(text: &mut String, card: &ReviewCard) {
    text.push_str("\nHandoff commands:\n");
    text.push_str(&format!("- Explain: `unsafe-review explain {}`\n", card.id));
    text.push_str(&format!(
        "- Agent context: `unsafe-review context {} --json`\n",
        card.id
    ));
}

fn push_trust_boundary(text: &mut String) {
    text.push_str("\nTrust boundary: ");
    text.push_str(TRUST_BOUNDARY);
}
