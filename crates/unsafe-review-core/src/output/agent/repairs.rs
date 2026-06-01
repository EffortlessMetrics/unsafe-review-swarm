use super::queue::AllowedRepairs;
use crate::domain::ReviewCard;

mod card_missing;
mod common;
mod operation;

pub(super) fn build(card: &ReviewCard) -> AllowedRepairs {
    let mut repairs = Vec::new();

    operation::add_operation_repairs(card, &mut repairs);
    card_missing::add_card_missing_repairs(card, &mut repairs);

    let has_card_scoped_repairs = !repairs.is_empty();
    if !has_card_scoped_repairs {
        repairs.push(card.next_action.summary.clone());
    }

    AllowedRepairs {
        repairs: common::dedupe_preserve_order(repairs),
        has_card_scoped_repairs,
    }
}
