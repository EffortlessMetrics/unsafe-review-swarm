use super::SourceLocation;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnsafeSiteKind {
    UnsafeBlock,
    UnsafeFn,
    UnsafeTrait,
    UnsafeImpl,
    UnsafeImplSend,
    UnsafeImplSync,
    ExternBlock,
    FfiCall,
    StaticMut,
    Operation,
}

impl UnsafeSiteKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UnsafeBlock => "unsafe_block",
            Self::UnsafeFn => "unsafe_fn",
            Self::UnsafeTrait => "unsafe_trait",
            Self::UnsafeImpl => "unsafe_impl",
            Self::UnsafeImplSend => "unsafe_impl_send",
            Self::UnsafeImplSync => "unsafe_impl_sync",
            Self::ExternBlock => "extern_block",
            Self::FfiCall => "ffi_call",
            Self::StaticMut => "static_mut",
            Self::Operation => "operation",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OperationFamily {
    RawPointerDeref,
    RawPointerRead,
    RawPointerReadUnaligned,
    RawPointerWrite,
    RawPointerWriteUnaligned,
    PointerArithmetic,
    PtrCopy,
    PtrReplace,
    CopyNonOverlapping,
    SliceFromRawParts,
    VecFromRawParts,
    StrFromUtf8Unchecked,
    MaybeUninitAssumeInit,
    VecSetLen,
    Transmute,
    Zeroed,
    DropInPlace,
    AtomicPointerState,
    UnwrapUnchecked,
    UnreachableUnchecked,
    UnsafeFnCall,
    BoxFromRaw,
    NonNullUnchecked,
    PinUnchecked,
    GetUnchecked,
    UnsafeImplSendSync,
    Ffi,
    StaticMut,
    InlineAsm,
    TargetFeature,
    PanicFromSafeJs,
    StableByteSourceGetterReentry,
    Unknown,
}

impl OperationFamily {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RawPointerDeref => "raw_pointer_deref",
            Self::RawPointerRead => "raw_pointer_read",
            Self::RawPointerReadUnaligned => "raw_pointer_read_unaligned",
            Self::RawPointerWrite => "raw_pointer_write",
            Self::RawPointerWriteUnaligned => "raw_pointer_write_unaligned",
            Self::PointerArithmetic => "pointer_arithmetic",
            Self::PtrCopy => "ptr_copy",
            Self::PtrReplace => "ptr_replace",
            Self::CopyNonOverlapping => "copy_nonoverlapping",
            Self::SliceFromRawParts => "slice_from_raw_parts",
            Self::VecFromRawParts => "vec_from_raw_parts",
            Self::StrFromUtf8Unchecked => "str_from_utf8_unchecked",
            Self::MaybeUninitAssumeInit => "maybe_uninit_assume_init",
            Self::VecSetLen => "vec_set_len",
            Self::Transmute => "transmute",
            Self::Zeroed => "zeroed",
            Self::DropInPlace => "drop_in_place",
            Self::AtomicPointerState => "atomic_pointer_state",
            Self::UnwrapUnchecked => "unwrap_unchecked",
            Self::UnreachableUnchecked => "unreachable_unchecked",
            Self::UnsafeFnCall => "unsafe_fn_call",
            Self::BoxFromRaw => "box_from_raw",
            Self::NonNullUnchecked => "nonnull_unchecked",
            Self::PinUnchecked => "pin_unchecked",
            Self::GetUnchecked => "get_unchecked",
            Self::UnsafeImplSendSync => "unsafe_impl_send_sync",
            Self::Ffi => "ffi",
            Self::StaticMut => "static_mut",
            Self::InlineAsm => "inline_asm",
            Self::TargetFeature => "target_feature",
            Self::PanicFromSafeJs => "panic_from_safe_js",
            Self::StableByteSourceGetterReentry => "stable_byte_source_getter_reentry",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnsafeOperation {
    pub family: OperationFamily,
    pub expression: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnsafeSite {
    pub location: SourceLocation,
    pub kind: UnsafeSiteKind,
    pub owner: Option<String>,
    pub visibility: String,
    pub public_api_surface: bool,
    pub changed: bool,
    pub snippet: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsafe_site_kind_strings_cover_every_variant() {
        let cases = [
            (UnsafeSiteKind::UnsafeBlock, "unsafe_block"),
            (UnsafeSiteKind::UnsafeFn, "unsafe_fn"),
            (UnsafeSiteKind::UnsafeTrait, "unsafe_trait"),
            (UnsafeSiteKind::UnsafeImpl, "unsafe_impl"),
            (UnsafeSiteKind::UnsafeImplSend, "unsafe_impl_send"),
            (UnsafeSiteKind::UnsafeImplSync, "unsafe_impl_sync"),
            (UnsafeSiteKind::ExternBlock, "extern_block"),
            (UnsafeSiteKind::FfiCall, "ffi_call"),
            (UnsafeSiteKind::StaticMut, "static_mut"),
            (UnsafeSiteKind::Operation, "operation"),
        ];

        for (kind, expected) in cases {
            assert_eq!(kind.as_str(), expected);
        }
    }

    #[test]
    fn operation_family_strings_cover_every_variant() {
        let cases = [
            (OperationFamily::RawPointerDeref, "raw_pointer_deref"),
            (OperationFamily::RawPointerRead, "raw_pointer_read"),
            (
                OperationFamily::RawPointerReadUnaligned,
                "raw_pointer_read_unaligned",
            ),
            (OperationFamily::RawPointerWrite, "raw_pointer_write"),
            (
                OperationFamily::RawPointerWriteUnaligned,
                "raw_pointer_write_unaligned",
            ),
            (OperationFamily::PointerArithmetic, "pointer_arithmetic"),
            (OperationFamily::PtrCopy, "ptr_copy"),
            (OperationFamily::PtrReplace, "ptr_replace"),
            (OperationFamily::CopyNonOverlapping, "copy_nonoverlapping"),
            (OperationFamily::SliceFromRawParts, "slice_from_raw_parts"),
            (OperationFamily::VecFromRawParts, "vec_from_raw_parts"),
            (
                OperationFamily::StrFromUtf8Unchecked,
                "str_from_utf8_unchecked",
            ),
            (
                OperationFamily::MaybeUninitAssumeInit,
                "maybe_uninit_assume_init",
            ),
            (OperationFamily::VecSetLen, "vec_set_len"),
            (OperationFamily::Transmute, "transmute"),
            (OperationFamily::Zeroed, "zeroed"),
            (OperationFamily::DropInPlace, "drop_in_place"),
            (OperationFamily::AtomicPointerState, "atomic_pointer_state"),
            (OperationFamily::UnwrapUnchecked, "unwrap_unchecked"),
            (
                OperationFamily::UnreachableUnchecked,
                "unreachable_unchecked",
            ),
            (OperationFamily::UnsafeFnCall, "unsafe_fn_call"),
            (OperationFamily::BoxFromRaw, "box_from_raw"),
            (OperationFamily::NonNullUnchecked, "nonnull_unchecked"),
            (OperationFamily::PinUnchecked, "pin_unchecked"),
            (OperationFamily::GetUnchecked, "get_unchecked"),
            (OperationFamily::UnsafeImplSendSync, "unsafe_impl_send_sync"),
            (OperationFamily::Ffi, "ffi"),
            (OperationFamily::StaticMut, "static_mut"),
            (OperationFamily::InlineAsm, "inline_asm"),
            (OperationFamily::TargetFeature, "target_feature"),
            (OperationFamily::PanicFromSafeJs, "panic_from_safe_js"),
            (
                OperationFamily::StableByteSourceGetterReentry,
                "stable_byte_source_getter_reentry",
            ),
            (OperationFamily::Unknown, "unknown"),
        ];

        for (family, expected) in cases {
            assert_eq!(family.as_str(), expected);
        }
    }
}
