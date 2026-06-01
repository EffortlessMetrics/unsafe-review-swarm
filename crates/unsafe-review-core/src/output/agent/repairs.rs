mod boundary;
mod pointer;
mod value;

use super::AllowedRepairs;
use crate::domain::ReviewCard;

pub(super) fn build(card: &ReviewCard) -> AllowedRepairs {
    let mut repairs = Vec::new();

    pointer::add_for_family(card, &mut repairs);
    value::add_for_family(card, &mut repairs);
    boundary::add_for_family(card, &mut repairs);
    add_cross_cutting_repairs(card, &mut repairs);

    let has_card_scoped_repairs = !repairs.is_empty();
    if !has_card_scoped_repairs {
        repairs.push(card.next_action.summary.clone());
    }
    AllowedRepairs {
        repairs: dedupe_preserve_order(repairs),
        has_card_scoped_repairs,
    }
}

fn add_cross_cutting_repairs(card: &ReviewCard, repairs: &mut Vec<String>) {
    if missing_kind(card, "contract") {
        repairs.push("add or expose the local safety contract for this card".to_string());
    }
    if missing_kind(card, "reach") || missing_kind(card, "test") {
        repairs
            .push("add or point to a focused test that exercises this owner or seam".to_string());
    }
    if missing_kind(card, "witness") {
        repairs.push(
            "attach a scoped witness receipt after running the suggested command outside unsafe-review"
                .to_string(),
        );
    }
}

fn push_if_missing_discharge(
    card: &ReviewCard,
    repairs: &mut Vec<String>,
    key: &str,
    repair: &str,
) {
    if missing_discharge(card, key) {
        repairs.push(repair.to_string());
    }
}

fn missing_discharge(card: &ReviewCard, key: &str) -> bool {
    card.obligation_evidence
        .iter()
        .any(|e| e.obligation.key == key && !e.discharge.present)
}

fn missing_kind(card: &ReviewCard, kind: &str) -> bool {
    card.missing.iter().any(|m| m.kind == kind)
}

fn dedupe_preserve_order(repairs: Vec<String>) -> Vec<String> {
    let mut deduped = Vec::new();
    for repair in repairs {
        if !deduped.contains(&repair) {
            deduped.push(repair);
        }
    }
    deduped
}
