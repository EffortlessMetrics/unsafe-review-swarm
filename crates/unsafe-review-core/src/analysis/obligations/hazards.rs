use crate::domain::{HazardKind, OperationFamily};

pub(crate) fn hazards_for(family: &OperationFamily) -> Vec<HazardKind> {
    match family {
        OperationFamily::RawPointerDeref
        | OperationFamily::RawPointerRead
        | OperationFamily::RawPointerWrite => vec![
            HazardKind::PointerValidity,
            HazardKind::Alignment,
            HazardKind::InitializedMemory,
            HazardKind::SameAllocation,
        ],
        OperationFamily::RawPointerReadUnaligned | OperationFamily::RawPointerWriteUnaligned => {
            vec![
                HazardKind::PointerValidity,
                HazardKind::InitializedMemory,
                HazardKind::SameAllocation,
            ]
        }
        OperationFamily::PointerArithmetic => vec![
            HazardKind::SameAllocation,
            HazardKind::Bounds,
            HazardKind::AliasingOrProvenance,
        ],
        OperationFamily::PtrCopy => vec![
            HazardKind::PointerValidity,
            HazardKind::Bounds,
            HazardKind::InitializedMemory,
        ],
        OperationFamily::PtrReplace => vec![
            HazardKind::PointerValidity,
            HazardKind::Alignment,
            HazardKind::InitializedMemory,
            HazardKind::DropOrDeallocation,
        ],
        OperationFamily::CopyNonOverlapping => vec![
            HazardKind::PointerValidity,
            HazardKind::Bounds,
            HazardKind::AliasingOrProvenance,
        ],
        OperationFamily::SliceFromRawParts => vec![
            HazardKind::PointerValidity,
            HazardKind::Alignment,
            HazardKind::InitializedMemory,
            HazardKind::Bounds,
            HazardKind::SameAllocation,
        ],
        OperationFamily::VecFromRawParts => vec![
            HazardKind::PointerValidity,
            HazardKind::Alignment,
            HazardKind::InitializedMemory,
            HazardKind::Bounds,
            HazardKind::DropOrDeallocation,
            HazardKind::LeakOrOwnershipTransfer,
        ],
        OperationFamily::StrFromUtf8Unchecked => vec![HazardKind::InvalidValue],
        OperationFamily::MaybeUninitAssumeInit => {
            vec![HazardKind::InitializedMemory, HazardKind::InvalidValue]
        }
        OperationFamily::VecSetLen => vec![HazardKind::InitializedMemory, HazardKind::Bounds],
        OperationFamily::Transmute | OperationFamily::Zeroed => vec![
            HazardKind::InvalidValue,
            HazardKind::LayoutOrRepr,
            HazardKind::AliasingOrProvenance,
        ],
        OperationFamily::DropInPlace => vec![
            HazardKind::PointerValidity,
            HazardKind::InitializedMemory,
            HazardKind::DropOrDeallocation,
        ],
        OperationFamily::AtomicPointerState => {
            vec![HazardKind::AtomicOrdering, HazardKind::DropOrDeallocation]
        }
        OperationFamily::UnwrapUnchecked | OperationFamily::UnreachableUnchecked => {
            vec![HazardKind::InvalidValue]
        }
        OperationFamily::UnsafeFnCall => vec![HazardKind::Unknown],
        OperationFamily::BoxFromRaw => vec![
            HazardKind::PointerValidity,
            HazardKind::DropOrDeallocation,
            HazardKind::LeakOrOwnershipTransfer,
        ],
        OperationFamily::NonNullUnchecked => vec![HazardKind::PointerValidity],
        OperationFamily::PinUnchecked => vec![HazardKind::PinInvariant],
        OperationFamily::GetUnchecked => vec![HazardKind::Bounds],
        OperationFamily::UnsafeImplSendSync => {
            vec![HazardKind::SendSyncInvariant, HazardKind::AtomicOrdering]
        }
        OperationFamily::Ffi => vec![HazardKind::FfiAbi, HazardKind::FfiOwnership],
        OperationFamily::StaticMut => vec![HazardKind::StaticMutGlobalState],
        OperationFamily::InlineAsm => vec![HazardKind::InlineAsm, HazardKind::TargetFeature],
        OperationFamily::TargetFeature => vec![HazardKind::TargetFeature],
        OperationFamily::PanicFromSafeJs => vec![HazardKind::PanicSafety],
        OperationFamily::StableByteSourceGetterReentry
        | OperationFamily::StableByteSourceRabAsync
        | OperationFamily::StableByteSourceSabRace
        | OperationFamily::StableByteSourceNativeFfiRead => vec![HazardKind::StableByteSource],
        OperationFamily::UnsafeDeclaration | OperationFamily::Unknown => vec![HazardKind::Unknown],
    }
}
