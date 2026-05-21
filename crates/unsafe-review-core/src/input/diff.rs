use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default)]
pub(crate) struct DiffIndex {
    pub(crate) changed_lines: BTreeMap<PathBuf, BTreeSet<usize>>,
}

impl DiffIndex {
    pub(crate) fn is_empty(&self) -> bool {
        self.changed_lines.is_empty()
    }

    pub(crate) fn contains_file(&self, path: &PathBuf) -> bool {
        self.changed_lines.contains_key(path)
    }

    pub(crate) fn contains_near(&self, path: &PathBuf, line: usize) -> bool {
        self.changed_lines
            .get(path)
            .is_some_and(|lines| lines.iter().any(|changed| changed.abs_diff(line) <= 6))
    }

    pub(crate) fn contains_in_range(&self, path: &PathBuf, start: usize, end: usize) -> bool {
        self.changed_lines.get(path).is_some_and(|lines| {
            lines
                .iter()
                .any(|changed| start <= *changed && *changed <= end)
        })
    }
}

#[derive(Debug, Default)]
struct DiffParserState {
    index: DiffIndex,
    current_path: Option<PathBuf>,
    new_line: usize,
}

impl DiffParserState {
    fn consume_line(&mut self, raw: &str) {
        if self.consume_file_boundary(raw) || self.consume_hunk_header(raw) {
            return;
        }

        let Some(path) = self.current_path.clone() else {
            return;
        };

        if should_skip_metadata(raw) {
            return;
        }

        self.consume_content_line(&path, raw);
    }

    fn consume_file_boundary(&mut self, raw: &str) -> bool {
        if raw.starts_with("diff --git ") {
            self.current_path = None;
            return true;
        }

        if let Some(path) = raw.strip_prefix("+++ b/") {
            let path = PathBuf::from(path.trim());
            self.current_path = Some(path.clone());
            self.index.changed_lines.entry(path).or_default();
            return true;
        }

        false
    }

    fn consume_hunk_header(&mut self, raw: &str) -> bool {
        if !raw.starts_with("@@") {
            return false;
        }

        if let Some(start) = parse_new_start(raw) {
            self.new_line = start;
        }

        true
    }

    fn consume_content_line(&mut self, path: &Path, raw: &str) {
        if raw.starts_with('+') {
            self.index
                .changed_lines
                .entry(path.to_path_buf())
                .or_default()
                .insert(self.new_line);
            self.new_line = self.new_line.saturating_add(1);
        } else if raw.starts_with('-') {
            // Removed lines do not advance the new-file coordinate.
        } else if raw.starts_with(' ') || raw.is_empty() {
            self.new_line = self.new_line.saturating_add(1);
        }
    }
}

fn should_skip_metadata(raw: &str) -> bool {
    raw.starts_with("+++") || raw.starts_with("---")
}

pub(crate) fn parse_unified_diff(input: &str) -> DiffIndex {
    let mut parser = DiffParserState::default();

    for raw in input.lines() {
        parser.consume_line(raw);
    }

    parser.index
}

fn parse_new_start(header: &str) -> Option<usize> {
    // @@ -old,count +new,count @@
    let mut parts = header.split_whitespace();
    let _at = parts.next()?;
    let _old = parts.next()?;
    let new = parts.next()?;
    let new = new.trim_start_matches('+');
    let start = new.split(',').next()?;
    start.parse::<usize>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::collections::BTreeSet;

    #[derive(Clone, Debug)]
    enum DiffLine {
        Context(String),
        Added(String),
        Removed(String),
        EmptyContext,
    }

    #[test]
    fn parse_unified_diff_tracks_new_file_lines_and_skips_deletions() {
        let diff = r#"diff --git a/src/lib.rs b/src/lib.rs
index 1111111..2222222 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -8,7 +8,8 @@ fn demo() {
 context();
-old_call();
+new_call();
 unchanged();
+extra_call();
 tail();
"#;

        let index = parse_unified_diff(diff);
        let path = PathBuf::from("src/lib.rs");

        assert!(index.contains_file(&path));
        assert!(index.changed_lines[&path].contains(&9));
        assert!(index.changed_lines[&path].contains(&11));
        assert!(!index.changed_lines[&path].contains(&10));
        assert!(index.contains_near(&path, 15));
        assert!(!index.contains_near(&path, 18));
        assert!(index.contains_in_range(&path, 8, 12));
        assert!(!index.contains_in_range(&path, 12, 15));
    }

    #[test]
    fn parse_unified_diff_keeps_empty_entries_for_changed_files_without_additions() {
        let diff = r#"diff --git a/src/lib.rs b/src/lib.rs
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,2 +1,1 @@
 fn keep() {}
-fn removed() {}
"#;

        let index = parse_unified_diff(diff);
        let path = PathBuf::from("src/lib.rs");

        assert!(index.contains_file(&path));
        assert!(index.changed_lines[&path].is_empty());
        assert!(!index.contains_near(&path, 1));
    }

    #[test]
    fn parse_unified_diff_supports_single_line_hunk_headers() {
        let diff = r#"diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -42 +42 @@ fn main() {
+println!("new");
"#;

        let index = parse_unified_diff(diff);

        assert!(index.changed_lines[&PathBuf::from("src/main.rs")].contains(&42));
    }

    #[test]
    fn contains_near_uses_six_line_review_window() {
        let diff = r#"diff --git a/src/lib.rs b/src/lib.rs
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -20,0 +21,1 @@
+unsafe { core::ptr::read(ptr) };
"#;

        let index = parse_unified_diff(diff);
        let path = PathBuf::from("src/lib.rs");

        assert!(index.contains_near(&path, 15));
        assert!(index.contains_near(&path, 27));
        assert!(!index.contains_near(&path, 14));
        assert!(!index.contains_near(&path, 28));
        assert!(!index.contains_near(&PathBuf::from("src/other.rs"), 21));
    }

    #[test]
    fn parse_unified_diff_tracks_added_lines_across_multiple_hunks() {
        let diff = r#"diff --git a/src/lib.rs b/src/lib.rs
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,3 +1,4 @@
 fn before() {}
+fn added() {}
 fn after() {}
@@ -20,2 +21,3 @@
 context();
+changed();
"#;
        let path = PathBuf::from("src/lib.rs");
        let index = parse_unified_diff(diff);

        assert!(index.contains_file(&path));
        assert!(index.contains_near(&path, 2));
        assert!(index.contains_near(&path, 22));
        assert!(!index.contains_near(&path, 40));
        assert_eq!(index.changed_lines[&path], BTreeSet::from([2, 22]));
    }

    #[test]
    fn parse_unified_diff_tracks_new_file_added_lines() {
        let diff = r#"diff --git a/src/new.rs b/src/new.rs
--- /dev/null
+++ b/src/new.rs
@@ -0,0 +1,2 @@
+pub fn one() {}
+pub fn two() {}
"#;
        let path = PathBuf::from("src/new.rs");
        let index = parse_unified_diff(diff);

        assert!(index.contains_file(&path));
        assert_eq!(index.changed_lines[&path], BTreeSet::from([1, 2]));
    }

    fn diff_line_strategy() -> impl Strategy<Value = DiffLine> {
        prop_oneof![
            any_line().prop_map(DiffLine::Context),
            any_line().prop_map(DiffLine::Added),
            any_line().prop_map(DiffLine::Removed),
            Just(DiffLine::EmptyContext),
        ]
    }

    fn any_line() -> impl Strategy<Value = String> {
        "[[:alnum:]_ (){};.,/*-]{0,40}".prop_map(|line: String| line.replace('\n', ""))
    }

    proptest! {
        #[test]
        fn added_lines_are_recorded_in_new_file_coordinates(
            start in 1usize..500,
            lines in prop::collection::vec(diff_line_strategy(), 0..80),
        ) {
            let path = PathBuf::from("src/lib.rs");
            let mut diff = format!(
                "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,1 +{start},1 @@\n"
            );
            let mut expected = BTreeSet::new();
            let mut new_line = start;

            for line in lines {
                match line {
                    DiffLine::Context(text) => {
                        diff.push(' ');
                        diff.push_str(&text);
                        diff.push('\n');
                        new_line = new_line.saturating_add(1);
                    }
                    DiffLine::Added(text) => {
                        diff.push('+');
                        diff.push_str(&text);
                        diff.push('\n');
                        expected.insert(new_line);
                        new_line = new_line.saturating_add(1);
                    }
                    DiffLine::Removed(text) => {
                        diff.push('-');
                        diff.push_str(&text);
                        diff.push('\n');
                    }
                    DiffLine::EmptyContext => {
                        diff.push('\n');
                        new_line = new_line.saturating_add(1);
                    }
                }
            }

            let parsed = parse_unified_diff(&diff);
            let actual = parsed.changed_lines.get(&path).cloned().unwrap_or_default();
            prop_assert_eq!(actual, expected);
        }

        #[test]
        fn removed_only_hunks_still_track_the_changed_file(
            start in 1usize..500,
            removed in prop::collection::vec(any_line(), 1..80),
        ) {
            let path = PathBuf::from("src/lib.rs");
            let mut diff = format!(
                "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,1 +{start},0 @@\n"
            );
            for line in removed {
                diff.push('-');
                diff.push_str(&line);
                diff.push('\n');
            }

            let parsed = parse_unified_diff(&diff);
            let actual = parsed.changed_lines.get(&path).cloned().unwrap_or_default();

            prop_assert!(parsed.contains_file(&path));
            prop_assert_eq!(actual, BTreeSet::new());
        }
    }
}
