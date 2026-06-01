use crate::domain::ReviewCard;

use super::repair_list::RepairList;

pub(super) fn add_for_card(card: &ReviewCard, repairs: &mut RepairList) {
    if repairs.missing_kind(card, "contract") {
        repairs.push("add or expose the local safety contract for this card");
    }
    if repairs.missing_kind(card, "reach") || repairs.missing_kind(card, "test") {
        repairs.push("add or point to a focused test that exercises this owner or seam");
    }
    if repairs.missing_kind(card, "witness") {
        repairs.push(
            "attach a scoped witness receipt after running the suggested command outside unsafe-review",
        );
    }
}
