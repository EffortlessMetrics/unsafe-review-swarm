use super::contract_discharge::TARGET_FEATURE_CONTRACT_DISCHARGE;
use crate::domain::{ContractEvidence, EvidenceState, OperationFamily};

pub(super) fn target_feature_discharge_state(
    family: &OperationFamily,
    contract: &ContractEvidence,
) -> EvidenceState {
    if family == &OperationFamily::TargetFeature && contract.present {
        EvidenceState::present(TARGET_FEATURE_CONTRACT_DISCHARGE)
    } else {
        EvidenceState::missing("No obligation-specific guard code was detected")
    }
}
