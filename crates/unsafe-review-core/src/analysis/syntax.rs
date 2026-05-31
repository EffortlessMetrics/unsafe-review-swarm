use ra_ap_syntax::{AstNode, Edition, SourceFile};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ParsedSource {
    pub(crate) text: String,
    pub(crate) parse_errors: Vec<String>,
    pub(crate) nodes: Vec<SyntaxNodeFact>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SyntaxNodeFact {
    pub(crate) kind: String,
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) line: usize,
    pub(crate) column: usize,
    pub(crate) snippet: String,
}

pub(crate) fn parse_source(text: impl Into<String>) -> ParsedSource {
    let text = text.into();
    let line_starts = line_starts(&text);
    let parse = SourceFile::parse(&text, Edition::CURRENT);
    let parse_errors = parse
        .errors()
        .iter()
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>();
    let tree = parse.tree();
    let nodes = tree
        .syntax()
        .descendants()
        .map(|node| {
            let range = node.text_range();
            let start = text_size_to_usize(range.start());
            let end = text_size_to_usize(range.end());
            let position = line_column(&text, start, &line_starts);
            SyntaxNodeFact {
                kind: format!("{:?}", node.kind()),
                start,
                end,
                line: position.line,
                column: position.column,
                snippet: snippet(&text, start, end),
            }
        })
        .collect();

    ParsedSource {
        text,
        parse_errors,
        nodes,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LineColumn {
    line: usize,
    column: usize,
}

fn line_column(text: &str, offset: usize, line_starts: &[usize]) -> LineColumn {
    let offset = clamp_to_char_boundary(text, offset.min(text.len()));
    let line_idx = line_starts
        .partition_point(|line_start| *line_start <= offset)
        .saturating_sub(1);
    let line_start = line_starts.get(line_idx).copied().unwrap_or(0);
    let line = line_idx + 1;

    let column = text
        .get(line_start..offset)
        .unwrap_or_default()
        .chars()
        .count()
        + 1;

    LineColumn { line, column }
}

fn clamp_to_char_boundary(text: &str, offset: usize) -> usize {
    if text.is_char_boundary(offset) {
        return offset;
    }

    let mut candidate = offset;
    while candidate > 0 && !text.is_char_boundary(candidate) {
        candidate -= 1;
    }
    candidate
}

fn snippet(text: &str, start: usize, end: usize) -> String {
    text.get(start..end)
        .map_or_else(String::new, str::to_string)
}

fn text_size_to_usize(size: ra_ap_syntax::TextSize) -> usize {
    u32::from(size) as usize
}

fn line_starts(text: &str) -> Vec<usize> {
    let mut starts = vec![0usize];
    starts.extend(
        text.char_indices()
            .filter_map(|(idx, ch)| (ch == '\n').then_some(idx + ch.len_utf8())),
    );
    starts
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn parses_complete_rust_source_without_errors() {
        let parsed = parse_source("pub fn answer() -> usize {\n    42\n}\n");

        assert!(parsed.parse_errors.is_empty());
        assert!(
            parsed
                .nodes
                .iter()
                .any(|node| node.kind == "FN" && node.snippet.contains("answer"))
        );
    }

    #[test]
    fn records_line_column_and_snippet_for_nested_nodes() -> Result<(), String> {
        let parsed =
            parse_source("fn outer() {\n    unsafe { core::ptr::read(0 as *const u8); }\n}\n");

        let Some(call) = parsed
            .nodes
            .iter()
            .find(|node| node.kind == "CALL_EXPR" && node.snippet.contains("core::ptr::read"))
        else {
            return Err("expected a nested call expression node".to_string());
        };

        assert_eq!(call.line, 2);
        assert_eq!(call.column, 14);
        assert_eq!(call.snippet, "core::ptr::read(0 as *const u8)");
        Ok(())
    }

    #[test]
    fn line_column_clamps_non_boundary_offsets() {
        let text = "a\u{e9}z";
        let line_starts = line_starts(text);

        let position = line_column(text, 2, &line_starts);

        assert_eq!(position.line, 1);
        assert_eq!(position.column, 2);
    }

    #[test]
    fn reports_parse_errors_without_discarding_tree() {
        let parsed = parse_source("pub fn broken( {\n");

        assert!(!parsed.parse_errors.is_empty());
        assert!(parsed.nodes.iter().any(|node| node.kind == "SOURCE_FILE"));
    }

    fn unicode_line() -> impl Strategy<Value = String> {
        proptest::collection::vec(
            any::<char>().prop_filter("generated lines must not include newlines", |ch| {
                *ch != '\n'
            }),
            0..24,
        )
        .prop_map(|chars| chars.into_iter().collect())
    }

    proptest! {
        #[test]
        fn line_column_tracks_ascii_newlines(lines in proptest::collection::vec("[ -~]{0,24}", 1..30)) {
            let text = lines.join("\n");
            let line_starts = line_starts(&text);
            let mut line_start = 0usize;

            for (line_index, line) in lines.iter().enumerate() {
                for column_offset in 0..=line.len() {
                    let position = line_column(&text, line_start + column_offset, &line_starts);
                    prop_assert_eq!(position.line, line_index + 1);
                    prop_assert_eq!(position.column, column_offset + 1);
                }
                line_start = line_start.saturating_add(line.len()).saturating_add(1);
            }
        }

        #[test]
        fn line_column_tracks_unicode_columns_and_clamps_inside_scalars(
            lines in proptest::collection::vec(unicode_line(), 1..30),
        ) {
            let text = lines.join("\n");
            let line_starts = line_starts(&text);
            let mut line_start = 0usize;

            for (line_index, line) in lines.iter().enumerate() {
                for (char_index, (byte_offset, ch)) in line.char_indices().enumerate() {
                    let absolute_offset = line_start + byte_offset;
                    let position = line_column(&text, absolute_offset, &line_starts);
                    prop_assert_eq!(position.line, line_index + 1);
                    prop_assert_eq!(position.column, char_index + 1);

                    for interior_byte in 1..ch.len_utf8() {
                        let clamped = line_column(&text, absolute_offset + interior_byte, &line_starts);
                        prop_assert_eq!(clamped.line, line_index + 1);
                        prop_assert_eq!(clamped.column, char_index + 1);
                    }
                }

                let line_end = line_start + line.len();
                let end_position = line_column(&text, line_end, &line_starts);
                prop_assert_eq!(end_position.line, line_index + 1);
                prop_assert_eq!(end_position.column, line.chars().count() + 1);

                line_start = line_start.saturating_add(line.len()).saturating_add(1);
            }
        }

        #[test]
        fn parsed_node_spans_are_valid_text_slices(chars in proptest::collection::vec(any::<char>(), 0..512)) {
            let text = chars.into_iter().collect::<String>();
            let parsed = parse_source(text.clone());

            prop_assert_eq!(parsed.text.as_str(), text.as_str());
            prop_assert!(!parsed.nodes.is_empty());

            for node in &parsed.nodes {
                prop_assert!(node.start <= node.end);
                prop_assert!(node.end <= parsed.text.len());
                prop_assert!(parsed.text.is_char_boundary(node.start));
                prop_assert!(parsed.text.is_char_boundary(node.end));
                prop_assert_eq!(
                    parsed.text.get(node.start..node.end),
                    Some(node.snippet.as_str())
                );
                prop_assert!(node.line >= 1);
                prop_assert!(node.column >= 1);
            }
        }
    }
}
