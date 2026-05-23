pub(crate) fn path_display(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

const FNV_OFFSET_BASIS_64: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME_64: u64 = 0x0000_0100_0000_01b3;

pub(crate) fn slug(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
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
    format!("{:016x}", stable_hash(value.as_bytes()))
}

fn stable_hash(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET_BASIS_64;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME_64);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::path::PathBuf;

    fn stable_hash_extend(mut hash: u64, bytes: &[u8]) -> u64 {
        for &byte in bytes {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(FNV_PRIME_64);
        }
        hash
    }

    #[test]
    fn stable_hash_hex_matches_known_vectors() {
        assert_eq!(stable_hash_hex(""), "cbf29ce484222325");
        assert_eq!(stable_hash_hex("a"), "af63dc4c8601ec8c");
        assert_eq!(stable_hash_hex("hello"), "a430d84680aabd0b");
        assert_eq!(stable_hash_hex("review-card"), "2e81ef99691f2bda");
        assert_eq!(stable_hash_hex("unsafe-review"), "9b8f659941f48b06");
        assert_eq!(stable_hash_hex("ReviewCard"), "06da7eb4d46c02c3");
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

    #[test]
    fn slug_preserves_ascii_alphanumerics_and_lowercases_letters() {
        assert_eq!(slug("AbC123xYz"), "abc123xyz");
        assert_eq!(slug("0Rust99"), "0rust99");
    }

    #[test]
    fn stable_hash_hex_is_fixed_width_lower_hex() {
        let hash = stable_hash_hex("ReviewCard::unsafe { ptr.read() }");
        assert_eq!(hash.len(), 16);
        assert!(
            hash.chars()
                .all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase())
        );
    }

    #[test]
    fn stable_hash_hex_tracks_exact_bytes() {
        assert_ne!(
            stable_hash_hex("unsafe-review"),
            stable_hash_hex("unsafe-review\n")
        );
        assert_ne!(stable_hash_hex("review"), stable_hash_hex("Review"));
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

        #[test]
        fn stable_hash_matches_incremental_streaming(
            prefix in proptest::collection::vec(any::<u8>(), 0..256),
            suffix in proptest::collection::vec(any::<u8>(), 0..256),
        ) {
            let mut combined = prefix.clone();
            combined.extend_from_slice(&suffix);

            let expected = stable_hash(&combined);
            let prefix_hash = stable_hash(&prefix);
            let streaming = stable_hash_extend(prefix_hash, &suffix);

            prop_assert_eq!(streaming, expected);
        }

        #[test]
        fn stable_hash_hex_is_lower_hex_with_fixed_width(input in "\\PC{0,512}") {
            let hex = stable_hash_hex(&input);
            prop_assert_eq!(hex.len(), 16);
            prop_assert!(hex.chars().all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase()));
        }
    }
}
