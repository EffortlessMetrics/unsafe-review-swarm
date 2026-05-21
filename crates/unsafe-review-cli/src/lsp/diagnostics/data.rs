use serde_json::{Value, json};
use unsafe_review_core::ReviewCard;

use crate::lsp::TRUST_BOUNDARY;

pub(super) fn build_diagnostic_data(card: &ReviewCard) -> Value {
    json!({
        "card_id": &card.id.0,
        "operation": &card.operation.expression,
        "operation_family": card.operation.family.as_str(),
        "hazards": card.hazards.iter().map(|hazard| hazard.as_str()).collect::<Vec<_>>(),
        "required_safety_conditions": card.obligations.iter().map(|obligation| {
            json!({
                "key": &obligation.key,
                "description": &obligation.description
            })
        }).collect::<Vec<_>>(),
        "evidence_summary": {
            "contract": {
                "present": card.contract.present,
                "state": present_label(card.contract.present),
                "summary": &card.contract.summary
            },
            "discharge": {
                "present": card.discharge.present,
                "state": present_label(card.discharge.present),
                "summary": &card.discharge.summary
            },
            "reach": {
                "state": &card.reach.state,
                "summary": &card.reach.summary
            },
            "witness": {
                "present": card.witness.present,
                "state": present_label(card.witness.present),
                "summary": &card.witness.summary
            },
            "reach_limitation": "static reach evidence is not proof that the unsafe site executed"
        },
        "obligation_evidence": card.obligation_evidence.iter().map(|evidence| {
            json!({
                "key": &evidence.obligation.key,
                "description": &evidence.obligation.description,
                "contract": evidence_state(
                    evidence.contract.present,
                    &evidence.contract.state,
                    &evidence.contract.summary
                ),
                "discharge": evidence_state(
                    evidence.discharge.present,
                    &evidence.discharge.state,
                    &evidence.discharge.summary
                ),
                "reach": evidence_state(
                    evidence.reach.present,
                    &evidence.reach.state,
                    &evidence.reach.summary
                ),
                "witness": evidence_state(
                    evidence.witness.present,
                    &evidence.witness.state,
                    &evidence.witness.summary
                )
            })
        }).collect::<Vec<_>>(),
        "missing_evidence": card.missing.iter().map(|missing| missing.message.as_str()).collect::<Vec<_>>(),
        "next_action": &card.next_action.summary,
        "witness_routes": card.routes.iter().map(|route| {
            json!({
                "kind": route.kind.as_str(),
                "reason": &route.reason,
                "command": route.command.as_deref(),
                "required": route.required
            })
        }).collect::<Vec<_>>(),
        "verify_commands": &card.next_action.verify_commands,
        "trust_boundary": TRUST_BOUNDARY
    })
}

fn evidence_state(present: bool, state: &str, summary: &str) -> Value {
    json!({
        "present": present,
        "state": state,
        "summary": summary
    })
}

fn present_label(present: bool) -> &'static str {
    if present { "present" } else { "missing" }
}
