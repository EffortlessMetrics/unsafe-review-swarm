use crate::domain::ReviewCard;

pub(super) fn add_if_missing_discharge(
    card: &ReviewCard,
    repairs: &mut Vec<String>,
    key: &str,
    repair: &str,
) {
    if missing_discharge(card, key) {
        repairs.push(repair.to_string());
    }
}

pub(super) fn missing_discharge(card: &ReviewCard, key: &str) -> bool {
    card.obligation_evidence
        .iter()
        .any(|e| e.obligation.key == key && !e.discharge.present)
}

pub(super) fn missing_kind(card: &ReviewCard, kind: &str) -> bool {
    card.missing.iter().any(|m| m.kind == kind)
}

pub(super) fn dedupe_preserve_order(repairs: Vec<String>) -> Vec<String> {
    let mut deduped = Vec::new();
    for repair in repairs {
        if !deduped.contains(&repair) {
            deduped.push(repair);
        }
    }
    deduped
}
