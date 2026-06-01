use crate::domain::ReviewCard;

#[derive(Default)]
pub(super) struct RepairList {
    repairs: Vec<String>,
}

impl RepairList {
    pub(super) fn push(&mut self, repair: impl Into<String>) {
        self.repairs.push(repair.into());
    }

    pub(super) fn push_if_missing_discharge(
        &mut self,
        card: &ReviewCard,
        obligation_key: &str,
        repair: impl Into<String>,
    ) {
        if self.missing_discharge(card, obligation_key) {
            self.push(repair);
        }
    }

    pub(super) fn missing_discharge(&self, card: &ReviewCard, obligation_key: &str) -> bool {
        card.obligation_evidence
            .iter()
            .any(|e| e.obligation.key == obligation_key && !e.discharge.present)
    }

    pub(super) fn missing_kind(&self, card: &ReviewCard, kind: &str) -> bool {
        card.missing.iter().any(|m| m.kind == kind)
    }

    pub(super) fn is_empty(&self) -> bool {
        self.repairs.is_empty()
    }

    pub(super) fn into_deduped(self) -> Vec<String> {
        let mut deduped = Vec::new();
        for repair in self.repairs {
            if !deduped.contains(&repair) {
                deduped.push(repair);
            }
        }
        deduped
    }
}
