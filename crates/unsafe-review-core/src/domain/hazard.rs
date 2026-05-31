#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HazardKind {
    PointerValidity,
    Alignment,
    SameAllocation,
    Bounds,
    InitializedMemory,
    InvalidValue,
    AliasingOrProvenance,
    PanicSafety,
    DropOrDeallocation,
    FfiAbi,
    FfiOwnership,
    SendSyncInvariant,
    PinInvariant,
    AtomicOrdering,
    LayoutOrRepr,
    StaticMutGlobalState,
    TargetFeature,
    InlineAsm,
    LeakOrOwnershipTransfer,
    Unknown,
}

impl HazardKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PointerValidity => "pointer_validity",
            Self::Alignment => "alignment",
            Self::SameAllocation => "same_allocation",
            Self::Bounds => "bounds",
            Self::InitializedMemory => "initialized_memory",
            Self::InvalidValue => "invalid_value",
            Self::AliasingOrProvenance => "aliasing_or_provenance",
            Self::PanicSafety => "panic_safety",
            Self::DropOrDeallocation => "drop_or_deallocation",
            Self::FfiAbi => "ffi_abi",
            Self::FfiOwnership => "ffi_ownership",
            Self::SendSyncInvariant => "send_sync_invariant",
            Self::PinInvariant => "pin_invariant",
            Self::AtomicOrdering => "atomic_ordering",
            Self::LayoutOrRepr => "layout_or_repr",
            Self::StaticMutGlobalState => "static_mut_global_state",
            Self::TargetFeature => "target_feature",
            Self::InlineAsm => "inline_asm",
            Self::LeakOrOwnershipTransfer => "leak_or_ownership_transfer",
            Self::Unknown => "unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hazard_strings_cover_every_variant() {
        let cases = [
            (HazardKind::PointerValidity, "pointer_validity"),
            (HazardKind::Alignment, "alignment"),
            (HazardKind::SameAllocation, "same_allocation"),
            (HazardKind::Bounds, "bounds"),
            (HazardKind::InitializedMemory, "initialized_memory"),
            (HazardKind::InvalidValue, "invalid_value"),
            (HazardKind::AliasingOrProvenance, "aliasing_or_provenance"),
            (HazardKind::PanicSafety, "panic_safety"),
            (HazardKind::DropOrDeallocation, "drop_or_deallocation"),
            (HazardKind::FfiAbi, "ffi_abi"),
            (HazardKind::FfiOwnership, "ffi_ownership"),
            (HazardKind::SendSyncInvariant, "send_sync_invariant"),
            (HazardKind::PinInvariant, "pin_invariant"),
            (HazardKind::AtomicOrdering, "atomic_ordering"),
            (HazardKind::LayoutOrRepr, "layout_or_repr"),
            (HazardKind::StaticMutGlobalState, "static_mut_global_state"),
            (HazardKind::TargetFeature, "target_feature"),
            (HazardKind::InlineAsm, "inline_asm"),
            (
                HazardKind::LeakOrOwnershipTransfer,
                "leak_or_ownership_transfer",
            ),
            (HazardKind::Unknown, "unknown"),
        ];

        for (hazard, expected) in cases {
            assert_eq!(hazard.as_str(), expected);
        }
    }
}
