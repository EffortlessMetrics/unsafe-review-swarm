use std::collections::BTreeSet;

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
        let starts_on_boundary = before.is_none_or(|ch| !is_ident_continue(ch));
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
