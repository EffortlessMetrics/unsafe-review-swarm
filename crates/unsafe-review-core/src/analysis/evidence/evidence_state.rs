use super::contract_discharge::{
    DOCUMENTED_PRIVATE_UNSAFE_CONTRACT_DISCHARGE, PUBLIC_UNSAFE_API_CONTRACT_DISCHARGE,
};
use crate::domain::{
    ContractEvidence, DischargeEvidence, EvidenceState, ObligationEvidence, ReachEvidence,
};

pub(crate) fn summarize_discharge(evidence: &[ObligationEvidence]) -> DischargeEvidence {
    if evidence.is_empty() {
        return DischargeEvidence::missing();
    }
    if evidence
        .iter()
        .all(|obligation| obligation.discharge.present)
    {
        if evidence.iter().all(|obligation| {
            obligation.discharge.summary == PUBLIC_UNSAFE_API_CONTRACT_DISCHARGE
                || obligation.discharge.summary == DOCUMENTED_PRIVATE_UNSAFE_CONTRACT_DISCHARGE
        }) {
            return DischargeEvidence::present(&evidence[0].discharge.summary);
        }
        return DischargeEvidence::present(
            "All inferred safety obligations have visible local discharge evidence",
        );
    }
    if evidence
        .iter()
        .any(|obligation| obligation.discharge.present)
    {
        return DischargeEvidence::missing_with(
            "Some inferred safety obligations are missing local guard evidence",
        );
    }
    DischargeEvidence::missing()
}

pub(super) fn contract_state(contract: &ContractEvidence) -> EvidenceState {
    if contract.present {
        EvidenceState::present(&contract.summary)
    } else {
        EvidenceState::missing(&contract.summary)
    }
}

pub(super) fn reach_state(reach: &ReachEvidence) -> EvidenceState {
    if reach.state == "unreached" || reach.state == "unknown" {
        EvidenceState::missing(&reach.summary)
    } else {
        EvidenceState::present(&reach.summary)
    }
}
