use crate::domain::ReviewCard;

use super::common;

pub(super) fn add_card_missing_repairs(card: &ReviewCard, repairs: &mut Vec<String>) {
    if common::missing_kind(card, "contract") {
        repairs.push("add or expose the local safety contract for this card".to_string());
    }
    if common::missing_kind(card, "reach") || common::missing_kind(card, "test") {
        repairs
            .push("add or point to a focused test that exercises this owner or seam".to_string());
    }
    if common::missing_kind(card, "witness") {
        repairs.push(
            "attach a scoped witness receipt after running the suggested command outside unsafe-review"
                .to_string(),
        );
    }
}
