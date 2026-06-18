use crate::analysis::scanner::ScannedSite;
use crate::domain::{ContractEvidence, OperationFamily, UnsafeSiteKind};

pub(super) const PUBLIC_UNSAFE_API_CONTRACT_DISCHARGE: &str = "Public unsafe API declaration is a caller-contract site; local guard evidence is not expected at the declaration";
pub(super) const DOCUMENTED_PRIVATE_UNSAFE_CONTRACT_DISCHARGE: &str = "Documented private unsafe declaration is a caller-contract site; local guard evidence is not expected at the declaration";
pub(super) const TARGET_FEATURE_CONTRACT_DISCHARGE: &str = "Documented target-feature declaration is a caller-contract site; local guard evidence is not expected at the attribute";

pub(super) fn is_public_unsafe_contract_obligation(site: &ScannedSite, key: &str) -> bool {
    key == "caller-contract"
        && site.site.public_api_surface
        && site.operation.family == OperationFamily::UnsafeDeclaration
        && matches!(
            site.site.kind,
            UnsafeSiteKind::UnsafeFn | UnsafeSiteKind::UnsafeTrait
        )
}

pub(super) fn is_documented_private_unsafe_contract_obligation(
    site: &ScannedSite,
    key: &str,
    contract: &ContractEvidence,
) -> bool {
    key == "caller-contract"
        && !site.site.public_api_surface
        && contract.present
        && contract.summary.contains("documentation")
        && site.operation.family == OperationFamily::UnsafeDeclaration
        && matches!(
            site.site.kind,
            UnsafeSiteKind::UnsafeFn | UnsafeSiteKind::UnsafeTrait
        )
}
