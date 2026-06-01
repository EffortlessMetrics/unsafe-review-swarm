use super::AllowedRepairs;
use crate::domain::ReviewCard;

mod followup;
mod operation;
mod repair_list;

use repair_list::RepairList;

pub(super) fn build(card: &ReviewCard) -> AllowedRepairs {
    let mut repairs = RepairList::default();

    operation::add_for_card(card, &mut repairs);
    followup::add_for_card(card, &mut repairs);

    let has_card_scoped_repairs = !repairs.is_empty();
    if !has_card_scoped_repairs {
        repairs.push(card.next_action.summary.clone());
    }

    AllowedRepairs {
        repairs: repairs.into_deduped(),
        has_card_scoped_repairs,
    }
}
