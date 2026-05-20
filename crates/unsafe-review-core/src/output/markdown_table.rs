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
}
