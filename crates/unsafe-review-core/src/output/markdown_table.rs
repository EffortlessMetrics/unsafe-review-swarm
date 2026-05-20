pub(crate) fn cell(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('|', "\\|")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_collapses_whitespace_and_escapes_pipes() {
        assert_eq!(cell("  unsafe | review\ncard  "), "unsafe \\| review card");
    }

    #[test]
    fn cell_returns_empty_for_all_whitespace_input() {
        assert_eq!(cell(" \n\t  "), "");
    }

    #[test]
    fn cell_escapes_multiple_pipes_without_extra_spaces() {
        assert_eq!(cell("a||b\t|\nc"), "a\\|\\|b \\| c");
    }
}
