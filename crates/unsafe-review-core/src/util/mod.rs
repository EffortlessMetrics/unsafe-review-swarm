pub(crate) fn path_display(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub(crate) fn slug(value: &str) -> String {
    let mut out = String::new();
    let mut last_was_dash = false;

    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash {
            out.push('-');
            last_was_dash = true;
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
    fn slug_collapses_separator_runs_and_lowercases_ascii() {
        assert_eq!(slug("Hello___WORLD   123"), "hello-world-123");
    }

    #[test]
    fn stable_hash_hex_is_deterministic_and_fixed_width() {
        let hash = stable_hash_hex("review-card");
        assert_eq!(hash, stable_hash_hex("review-card"));
        assert_eq!(hash.len(), 16);
        assert!(hash.chars().all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase()));
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
