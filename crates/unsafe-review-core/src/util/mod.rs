pub(crate) fn path_display(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub(crate) fn slug(value: &str) -> String {
    let mut out = String::new();
    let mut trailing_dash = false;

    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            trailing_dash = false;
        } else if !trailing_dash {
            out.push('-');
            trailing_dash = true;
        }
    }

    out.trim_matches('-').to_string()
}

pub(crate) fn stable_hash_hex(value: &str) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::path::PathBuf;

    #[test]
    fn stable_hash_hex_matches_known_vectors() {
        assert_eq!(stable_hash_hex(""), "cbf29ce484222325");
        assert_eq!(stable_hash_hex("a"), "af63dc4c8601ec8c");
        assert_eq!(stable_hash_hex("hello"), "a430d84680aabd0b");
    }

    #[test]
    fn slug_collapses_symbol_runs_and_trims_boundaries() {
        assert_eq!(slug("  Hello,   WORLD!!!  "), "hello-world");
        assert_eq!(slug("___Rust__Unsafe__Review___"), "rust-unsafe-review");
        assert_eq!(slug("***"), "");
    }

    #[test]
    fn slug_collapses_non_ascii_and_separators() {
        assert_eq!(
            slug("  \u{041f}\u{0440}\u{0438}\u{0432}\u{0435}\u{0442}, Rust---World!  "),
            "rust-world"
        );
    }

    proptest! {
        #[test]
        fn slug_outputs_are_stable_ascii_tokens(input in "\\PC{0,256}") {
            let slugged = slug(&input);

            prop_assert_eq!(slug(&slugged), slugged.clone());
            prop_assert!(!slugged.starts_with('-'));
            prop_assert!(!slugged.ends_with('-'));
            prop_assert!(!slugged.contains("--"));
            prop_assert!(slugged
                .chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-'));
        }

        #[test]
        fn path_display_normalizes_backslashes(input in "[[:alnum:]_./\\\\ -]{0,256}") {
            let path = PathBuf::from(input);
            let displayed = path_display(&path);

            prop_assert!(!displayed.contains('\\'));
        }
    }
}
