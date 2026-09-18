use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceLocation {
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
    /// 1-based end position (exclusive) of the site's source extent.
    /// Syntax-built sites carry the true end of the parsed node, which may
    /// span lines. Sites built without syntax facts default the end to the
    /// start (point range); single-line consumers keep their legacy width.
    pub end_line: usize,
    pub end_column: usize,
}

impl SourceLocation {
    pub fn new(file: impl Into<PathBuf>, line: usize, column: usize) -> Self {
        Self {
            file: file.into(),
            line,
            column,
            end_line: line,
            end_column: column,
        }
    }

    pub fn new_with_end(
        file: impl Into<PathBuf>,
        line: usize,
        column: usize,
        end_line: usize,
        end_column: usize,
    ) -> Self {
        Self {
            file: file.into(),
            line,
            column,
            end_line,
            end_column,
        }
    }
}
