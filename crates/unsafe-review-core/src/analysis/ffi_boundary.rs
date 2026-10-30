use crate::domain::OperationFamily;
use std::collections::BTreeSet;

/// Same-file `extern` static names, split by mutability. Entries carry the
/// same bare and module-qualified spellings as extern fn names.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct ExternStaticNames {
    pub(super) immutable: BTreeSet<String>,
    pub(super) mutable: BTreeSet<String>,
}

/// Family for a bare (non-call) use of a same-file extern static inside an
/// unsafe block. Immutable reads are FFI boundary uses; `static mut` uses
/// carry the global-state hazard. `shadowed` holds same-scope safe binding
/// names (locals/params) that hide the extern static for bare spellings.
pub(super) fn extern_static_use_family(
    line: &str,
    statics: &ExternStaticNames,
    shadowed: &BTreeSet<String>,
) -> Option<OperationFamily> {
    if statics
        .mutable
        .iter()
        .any(|name| contains_bare_static_use(line, name, shadowed))
    {
        return Some(OperationFamily::StaticMut);
    }
    if statics
        .immutable
        .iter()
        .any(|name| contains_bare_static_use(line, name, shadowed))
    {
        return Some(OperationFamily::Ffi);
    }
    None
}

fn contains_bare_static_use(line: &str, path: &str, shadowed: &BTreeSet<String>) -> bool {
    let simple = path.rsplit("::").next().unwrap_or(path);
    if !path.contains("::") && (shadowed.contains(simple) || line_has_let_binding(line, simple)) {
        return false;
    }
    let mut cursor = line;
    let mut offset = 0usize;
    while let Some(pos) = cursor.find(path) {
        let absolute = offset + pos;
        let before = line[..absolute].chars().next_back();
        let starts_on_boundary =
            before.is_none_or(|ch| !is_ident_continue(ch) && ch != ':' && ch != '.');
        let after = &line[absolute + path.len()..];
        let ends_on_boundary = after.chars().next().is_none_or(|ch| !is_ident_continue(ch));
        // A call suffix or a further path segment is not a bare static use.
        let after_trimmed = after.trim_start();
        let not_call_or_path = !after_trimmed.starts_with('(') && !after.starts_with("::");
        // Binding/declaration occurrences (`let NAME`, `static NAME`) name
        // the binding, not a use of the extern static.
        let prefix = line[..absolute].trim_end();
        let is_binding = prefix.ends_with("let")
            || prefix.ends_with("let mut")
            || prefix.ends_with("static")
            || prefix.ends_with("static mut");
        if starts_on_boundary && ends_on_boundary && not_call_or_path && !is_binding {
            return true;
        }
        let next = pos + path.len();
        offset += next;
        cursor = &cursor[next..];
    }
    false
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FfiBoundaryApplicability {
    kind: FfiBoundaryKind,
    call_path: String,
}

impl FfiBoundaryApplicability {
    fn same_file_extern(call_path: impl Into<String>) -> Self {
        Self {
            kind: FfiBoundaryKind::SameFileExtern,
            call_path: call_path.into(),
        }
    }

    fn known_foreign_path(call_path: impl Into<String>) -> Self {
        Self {
            kind: FfiBoundaryKind::KnownForeignPath,
            call_path: call_path.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FfiBoundaryKind {
    SameFileExtern,
    KnownForeignPath,
}

pub(super) fn ffi_boundary_applicability(
    line: &str,
    extern_names: &BTreeSet<String>,
    local_modules: &BTreeSet<String>,
) -> bool {
    ffi_boundary_match(line, extern_names, local_modules).is_some()
}

fn ffi_boundary_match(
    line: &str,
    extern_names: &BTreeSet<String>,
    local_modules: &BTreeSet<String>,
) -> Option<FfiBoundaryApplicability> {
    if !unsafe_block_contains_call(line) {
        return None;
    }
    if !local_modules.contains("libc")
        && let Some(call_path) = matching_call_path_prefix(line, "libc::")
    {
        return Some(FfiBoundaryApplicability::known_foreign_path(call_path));
    }
    if let Some(call_path) = matching_extern_call_path(line, extern_names) {
        return Some(FfiBoundaryApplicability::same_file_extern(call_path));
    }
    None
}

fn line_has_let_binding(line: &str, name: &str) -> bool {
    for marker in ["let mut ", "let "] {
        let mut cursor = line;
        while let Some(pos) = cursor.find(marker) {
            let before = cursor[..pos].chars().next_back();
            let rest = cursor[pos + marker.len()..].trim_start();
            if before.is_none_or(|ch| !is_ident_continue(ch)) && rest.starts_with(name) {
                let after = &rest[name.len()..];
                if after.chars().next().is_none_or(|ch| !is_ident_continue(ch)) {
                    return true;
                }
            }
            cursor = &cursor[pos + marker.len()..];
        }
    }
    false
}

fn matching_extern_call_path<'a>(
    line: &str,
    extern_names: &'a BTreeSet<String>,
) -> Option<&'a str> {
    extern_names
        .iter()
        .find_map(|name| contains_extern_call_path(line, name).then_some(name.as_str()))
}

fn contains_extern_call_path(line: &str, path: &str) -> bool {
    if path.contains("::") {
        contains_call_path(line, path)
    } else {
        contains_unqualified_call_name(line, path)
    }
}

fn contains_call_path(line: &str, path: &str) -> bool {
    let mut cursor = line;
    let mut offset = 0usize;
    while let Some(pos) = cursor.find(path) {
        let absolute = offset + pos;
        let before = line[..absolute].chars().next_back();
        let starts_on_boundary = before.is_none_or(|ch| !is_ident_continue(ch) && ch != ':');
        let after_path = &line[absolute + path.len()..];
        if starts_on_boundary && call_suffix(after_path) {
            return true;
        }
        let next = pos + path.len();
        offset += next;
        cursor = &cursor[next..];
    }
    false
}

fn contains_unqualified_call_name(line: &str, name: &str) -> bool {
    let mut cursor = line;
    let mut offset = 0usize;
    while let Some(pos) = cursor.find(name) {
        let absolute = offset + pos;
        let before = line[..absolute].chars().next_back();
        // Reject matches where the preceding char is a method-call receiver dot (`.`),
        // a path separator (`:`), or any ident-continue char — those indicate the name
        // is part of a qualified path or a method-call chain, not a bare extern call.
        let starts_on_boundary =
            before.is_none_or(|ch| !is_ident_continue(ch) && ch != ':' && ch != '.');
        let after = &line[absolute + name.len()..];
        if starts_on_boundary && call_suffix(after) {
            return true;
        }
        let next = pos + name.len();
        offset += next;
        cursor = &cursor[next..];
    }
    false
}

fn matching_call_path_prefix(line: &str, prefix: &str) -> Option<String> {
    let mut cursor = line;
    let mut offset = 0usize;
    while let Some(pos) = cursor.find(prefix) {
        let absolute = offset + pos;
        let before = line[..absolute].chars().next_back();
        // Reject ident-continue chars, path separators (`:`), and member
        // access (`.`) before the prefix so that an inner path segment such
        // as `something::libc::` does not match as a top-level libc prefix.
        // This mirrors the `contains_call_path` / `contains_unqualified_call_name`
        // boundary checks.
        let starts_on_boundary =
            before.is_none_or(|ch| !is_ident_continue(ch) && ch != ':' && ch != '.');
        let after_prefix = &line[absolute + prefix.len()..];
        if starts_on_boundary && call_path_suffix(after_prefix) {
            let path_end = after_prefix.find('(').unwrap_or(after_prefix.len());
            let call_suffix = after_prefix[..path_end].trim();
            return Some(format!("{prefix}{call_suffix}"));
        }
        let next = pos + prefix.len();
        offset += next;
        cursor = &cursor[next..];
    }
    None
}

fn call_path_suffix(after_prefix: &str) -> bool {
    let Some(paren) = after_prefix.find('(') else {
        return false;
    };
    let path = after_prefix[..paren].trim();
    !path.is_empty()
        && path.chars().next().is_some_and(is_ident_continue)
        && path.chars().all(|ch| is_ident_continue(ch) || ch == ':')
}

fn unsafe_block_contains_call(line: &str) -> bool {
    let Some((_before, after_unsafe)) = line.split_once("unsafe") else {
        return false;
    };
    let Some((_before_block, after_open)) = after_unsafe.split_once('{') else {
        return false;
    };
    after_open.contains('(') && after_open.contains(')')
}

fn call_suffix(after_name: &str) -> bool {
    let rest = after_name.trim_start();
    if rest.starts_with('(') {
        return true;
    }
    rest.strip_prefix("::")
        .is_some_and(|after_colons| after_colons.trim_start().starts_with('<'))
}

fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffi_boundary_applicability_keeps_foreign_boundary_explicit() {
        let extern_names = BTreeSet::from([
            "strlen".to_string(),
            "ffi::strlen".to_string(),
            "crate::ffi::strlen".to_string(),
            "self::ffi::strlen".to_string(),
        ]);
        let local_modules = BTreeSet::new();
        let local_libc = BTreeSet::from(["libc".to_string()]);

        assert_eq!(
            ffi_boundary_match("unsafe { strlen(ptr) }", &extern_names, &local_modules),
            Some(FfiBoundaryApplicability::same_file_extern("strlen"))
        );
        assert_eq!(
            ffi_boundary_match("unsafe { ffi::strlen(ptr) }", &extern_names, &local_modules),
            Some(FfiBoundaryApplicability::same_file_extern("ffi::strlen"))
        );
        assert_eq!(
            ffi_boundary_match(
                "unsafe { other::ffi::strlen(ptr) }",
                &extern_names,
                &local_modules
            ),
            None
        );
        assert_eq!(
            ffi_boundary_match(
                "unsafe { libc::strlen(ptr) }",
                &extern_names,
                &local_modules
            ),
            Some(FfiBoundaryApplicability::known_foreign_path("libc::strlen"))
        );
        assert_eq!(
            ffi_boundary_match("unsafe { libc::strlen(ptr) }", &extern_names, &local_libc),
            None
        );
        assert_eq!(
            ffi_boundary_match(
                "unsafe { mylibc::strlen(ptr) }",
                &extern_names,
                &local_modules
            ),
            None
        );
    }

    #[test]
    fn inner_path_segment_libc_does_not_match_as_prefix() {
        // `something::libc::call()` — `libc` is an inner segment, not the root.
        // The boundary check must reject the `:` preceding `libc::`.
        let extern_names = BTreeSet::new();
        let local_modules = BTreeSet::new();

        // Inner segment: must NOT route to FFI.
        assert_eq!(
            ffi_boundary_match(
                "unsafe { something::libc::strlen(ptr) }",
                &extern_names,
                &local_modules
            ),
            None,
            "inner-segment `something::libc::strlen` must not route to FFI"
        );
        // Top-level path: MUST still route to FFI.
        assert_eq!(
            ffi_boundary_match(
                "unsafe { libc::strlen(ptr) }",
                &extern_names,
                &local_modules
            ),
            Some(FfiBoundaryApplicability::known_foreign_path("libc::strlen")),
            "top-level `libc::strlen` must still route to FFI"
        );
    }

    fn statics_fixture() -> ExternStaticNames {
        ExternStaticNames {
            immutable: BTreeSet::from([
                "ERRNO".to_string(),
                "ffi::ERRNO".to_string(),
                "crate::ffi::ERRNO".to_string(),
            ]),
            mutable: BTreeSet::from(["COUNTER".to_string(), "ffi::COUNTER".to_string()]),
        }
    }

    #[test]
    fn extern_static_bare_uses_route_by_mutability() {
        let statics = statics_fixture();
        let none = BTreeSet::new();
        assert_eq!(
            extern_static_use_family("unsafe { ERRNO }", &statics, &none),
            Some(OperationFamily::Ffi)
        );
        assert_eq!(
            extern_static_use_family("unsafe { ffi::ERRNO + 1 }", &statics, &none),
            Some(OperationFamily::Ffi)
        );
        assert_eq!(
            extern_static_use_family("unsafe { COUNTER += 1 }", &statics, &none),
            Some(OperationFamily::StaticMut)
        );
        assert_eq!(
            extern_static_use_family("unsafe { ffi::COUNTER }", &statics, &none),
            Some(OperationFamily::StaticMut)
        );
    }

    #[test]
    fn extern_static_bare_uses_reject_safe_bindings_and_non_uses() {
        let statics = statics_fixture();
        let none = BTreeSet::new();
        // Field/method receiver and unrelated qualified path are not bare uses.
        assert_eq!(
            extern_static_use_family("unsafe { x.ERRNO }", &statics, &none),
            None
        );
        assert_eq!(
            extern_static_use_family("unsafe { other::ERRNO }", &statics, &none),
            None
        );
        // Call-shaped spelling is not a bare static use.
        assert_eq!(
            extern_static_use_family("unsafe { ERRNO() }", &statics, &none),
            None
        );
        // A `let` binding in the block shadows the extern static.
        assert_eq!(
            extern_static_use_family("unsafe { let ERRNO = 7; ERRNO }", &statics, &none),
            None
        );
        // A shadowed name recorded by the caller does not route.
        let shadowed = BTreeSet::from(["ERRNO".to_string()]);
        assert_eq!(
            extern_static_use_family("unsafe { ERRNO }", &statics, &shadowed),
            None
        );
        // Declaration spelling names the binding, not a use.
        assert_eq!(
            extern_static_use_family("static ERRNO: i32;", &statics, &none),
            None
        );
    }

    #[test]
    fn method_receiver_dot_does_not_match_same_named_extern() {
        // A method call `f.close()` must not match the unqualified extern name `close`
        // even when `close` is declared in an `extern "C"` block in the same file.
        let extern_names = BTreeSet::from(["close".to_string()]);
        let local_modules = BTreeSet::new();

        // Method-call receiver form — must NOT route to FFI.
        assert_eq!(
            ffi_boundary_match("unsafe { f.close() }", &extern_names, &local_modules),
            None,
            "method-call receiver `.close()` must not match unqualified extern `close`"
        );
        // Bare unqualified call — MUST still route to FFI.
        assert_eq!(
            ffi_boundary_match("unsafe { close(fd) }", &extern_names, &local_modules),
            Some(FfiBoundaryApplicability::same_file_extern("close")),
            "bare call `close(fd)` must still route to FFI"
        );
    }
}
