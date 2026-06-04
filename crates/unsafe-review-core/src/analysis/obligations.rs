mod hazards;
mod safety;

pub(crate) use hazards::hazards_for;
pub(crate) use safety::obligations_for;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{HazardKind, OperationFamily};

    fn obligation_keys(family: &OperationFamily) -> Vec<String> {
        obligations_for(family)
            .into_iter()
            .map(|obligation| obligation.key)
            .collect()
    }

    #[test]
    fn raw_pointer_operations_keep_distinct_hazards_and_obligation_keys() {
        for family in [
            OperationFamily::RawPointerDeref,
            OperationFamily::RawPointerRead,
            OperationFamily::RawPointerWrite,
        ] {
            assert_eq!(
                hazards_for(&family),
                vec![
                    HazardKind::PointerValidity,
                    HazardKind::Alignment,
                    HazardKind::InitializedMemory,
                    HazardKind::SameAllocation,
                ],
                "{family:?}"
            );
            assert_eq!(
                obligation_keys(&family),
                vec![
                    "pointer-live".to_string(),
                    "bounds".to_string(),
                    "alignment".to_string(),
                    "initialized".to_string(),
                    "allocation".to_string(),
                ],
                "{family:?}"
            );
        }
    }

    #[test]
    fn unaligned_raw_pointer_operations_do_not_require_alignment_obligation() {
        for family in [
            OperationFamily::RawPointerReadUnaligned,
            OperationFamily::RawPointerWriteUnaligned,
        ] {
            assert_eq!(
                hazards_for(&family),
                vec![
                    HazardKind::PointerValidity,
                    HazardKind::InitializedMemory,
                    HazardKind::SameAllocation,
                ],
                "{family:?}"
            );
            assert_eq!(
                obligation_keys(&family),
                vec![
                    "pointer-live".to_string(),
                    "bounds".to_string(),
                    "initialized".to_string(),
                    "allocation".to_string(),
                ],
                "{family:?}"
            );
        }
    }

    #[test]
    fn specialized_operations_map_to_targeted_obligations() {
        assert_eq!(
            hazards_for(&OperationFamily::VecSetLen),
            vec![HazardKind::InitializedMemory, HazardKind::Bounds]
        );
        assert_eq!(
            obligation_keys(&OperationFamily::VecSetLen),
            vec!["capacity".to_string(), "initialized".to_string()]
        );
        assert_eq!(
            hazards_for(&OperationFamily::UnsafeImplSendSync),
            vec![HazardKind::SendSyncInvariant, HazardKind::AtomicOrdering]
        );
        assert_eq!(
            obligation_keys(&OperationFamily::UnsafeImplSendSync),
            vec!["thread-safety".to_string()]
        );
        assert_eq!(
            hazards_for(&OperationFamily::DropInPlace),
            vec![
                HazardKind::PointerValidity,
                HazardKind::InitializedMemory,
                HazardKind::DropOrDeallocation,
            ]
        );
        assert_eq!(
            obligation_keys(&OperationFamily::DropInPlace),
            vec![
                "pointer-live".to_string(),
                "initialized".to_string(),
                "ownership".to_string(),
            ]
        );
        assert_eq!(
            hazards_for(&OperationFamily::UnsafeFnCall),
            vec![HazardKind::Unknown]
        );
        assert_eq!(
            obligation_keys(&OperationFamily::UnsafeFnCall),
            vec!["callee-contract".to_string()]
        );
        assert_eq!(
            hazards_for(&OperationFamily::StableByteSourceGetterReentry),
            vec![HazardKind::StableByteSource]
        );
        assert_eq!(
            obligation_keys(&OperationFamily::StableByteSourceGetterReentry),
            vec!["byte-stability".to_string()]
        );
        assert_eq!(
            hazards_for(&OperationFamily::StableByteSourceRabAsync),
            vec![HazardKind::StableByteSource]
        );
        assert_eq!(
            obligation_keys(&OperationFamily::StableByteSourceRabAsync),
            vec!["byte-stability".to_string()]
        );
        assert_eq!(
            hazards_for(&OperationFamily::StableByteSourceSabRace),
            vec![HazardKind::StableByteSource]
        );
        assert_eq!(
            obligation_keys(&OperationFamily::StableByteSourceSabRace),
            vec!["byte-stability".to_string()]
        );
        assert_eq!(
            hazards_for(&OperationFamily::StableByteSourceNativeFfiRead),
            vec![HazardKind::StableByteSource]
        );
        assert_eq!(
            obligation_keys(&OperationFamily::StableByteSourceNativeFfiRead),
            vec!["byte-stability".to_string()]
        );
        assert_eq!(
            hazards_for(&OperationFamily::AtomicPointerState),
            vec![HazardKind::AtomicOrdering, HazardKind::DropOrDeallocation]
        );
        assert_eq!(
            obligation_keys(&OperationFamily::AtomicPointerState),
            vec!["state-transition".to_string(), "ordering".to_string()]
        );
        assert_eq!(
            hazards_for(&OperationFamily::UnreachableUnchecked),
            vec![HazardKind::InvalidValue]
        );
        assert_eq!(
            obligation_keys(&OperationFamily::UnreachableUnchecked),
            vec!["unreachable".to_string()]
        );
    }

    #[test]
    fn ffi_and_unknown_families_keep_explicit_review_models() {
        assert_eq!(
            hazards_for(&OperationFamily::Ffi),
            vec![HazardKind::FfiAbi, HazardKind::FfiOwnership]
        );
        assert_eq!(
            obligation_keys(&OperationFamily::Ffi),
            vec!["abi".to_string(), "ownership".to_string()]
        );

        assert_eq!(
            hazards_for(&OperationFamily::Unknown),
            vec![HazardKind::Unknown]
        );
        assert_eq!(
            obligation_keys(&OperationFamily::Unknown),
            vec!["unknown".to_string()]
        );
    }
}
