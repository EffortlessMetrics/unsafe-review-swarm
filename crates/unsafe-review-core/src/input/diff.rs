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

        self.consume_content_line(&path, raw);
    }

    fn consume_file_boundary(&mut self, raw: &str) -> bool {
        if raw.starts_with("diff --git ") {
            self.current_path = None;
            return true;
        }

        if let Some(path) = raw.strip_prefix("+++ b/") {
            let path = PathBuf::from(path);
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
    fn parse_unified_diff_counts_added_lines_that_start_with_plus_markers() {
        let diff = r#"diff --git a/src/lib.rs b/src/lib.rs
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -3,0 +4,2 @@
+++++ not metadata
+--- also content
"#;
        let path = PathBuf::from("src/lib.rs");
        let index = parse_unified_diff(diff);

        assert_eq!(index.changed_lines[&path], BTreeSet::from([4, 5]));
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
    fn parse_unified_diff_preserves_trailing_spaces_in_paths() {
        let diff = concat!(
            "diff --git a/src/name-with-trailing-space.rs  b/src/name-with-trailing-space.rs \n",
            "--- a/src/name-with-trailing-space.rs \n",
            "+++ b/src/name-with-trailing-space.rs \n",
            "@@ -1,0 +1,1 @@\n",
            "+fn added() {}\n",
        );
        let path = PathBuf::from("src/name-with-trailing-space.rs ");
        let index = parse_unified_diff(diff);

        assert!(index.contains_file(&path));
        assert_eq!(index.changed_lines[&path], BTreeSet::from([1]));
    }

    #[test]
    fn parse_unified_diff_keeps_added_line_sets_scoped_per_file() {
        let diff = r#"diff --git a/src/first.rs b/src/first.rs
--- a/src/first.rs
+++ b/src/first.rs
@@ -3,1 +3,2 @@
 keep();
+first_added();
diff --git a/src/second.rs b/src/second.rs
--- a/src/second.rs
+++ b/src/second.rs
@@ -10,0 +10,2 @@
+second_added_one();
+second_added_two();
"#;

        let index = parse_unified_diff(diff);
        let first = PathBuf::from("src/first.rs");
        let second = PathBuf::from("src/second.rs");

        assert_eq!(index.changed_lines[&first], BTreeSet::from([4]));
        assert_eq!(index.changed_lines[&second], BTreeSet::from([10, 11]));
        assert!(index.contains_in_range(&first, 4, 4));
        assert!(!index.contains_in_range(&first, 10, 11));
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
        "[[:alnum:]_ (){};.,/*+-]{0,40}".prop_map(|line: String| line.replace('\n', ""))
    }

    fn append_diff_lines(diff: &mut String, lines: Vec<DiffLine>, start: usize) -> BTreeSet<usize> {
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

        expected
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
            let expected = append_diff_lines(&mut diff, lines, start);

            let parsed = parse_unified_diff(&diff);
            let actual = parsed.changed_lines.get(&path).cloned().unwrap_or_default();
            prop_assert_eq!(actual, expected.clone());
        }

        #[test]
        fn contains_near_matches_six_line_window_for_recorded_changes(
            start in 1usize..500,
            lines in prop::collection::vec(diff_line_strategy(), 1..80),
        ) {
            let path = PathBuf::from("src/lib.rs");
            let mut diff = format!(
                "diff --git a/src/lib.rs b/src/lib.rs
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,1 +{start},1 @@
"
            );
            let expected = append_diff_lines(&mut diff, lines, start);

            let parsed = parse_unified_diff(&diff);
            let actual = parsed.changed_lines.get(&path).cloned().unwrap_or_default();
            prop_assert_eq!(actual, expected.clone());

            for line in 1usize..=600 {
                let expected_near = expected
                    .iter()
                    .any(|changed| changed.abs_diff(line) <= 6);
                prop_assert_eq!(parsed.contains_near(&path, line), expected_near);
            }

            for window_start in 1usize..=590 {
                let window_end = window_start + 10;
                let expected_range = expected
                    .iter()
                    .any(|changed| *changed >= window_start && *changed <= window_end);
                prop_assert_eq!(
                    parsed.contains_in_range(&path, window_start, window_end),
                    expected_range
                );
            }
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

        #[test]
        fn repeated_file_hunks_union_added_lines_after_each_hunk_header(
            first_start in 1usize..250,
            second_start in 251usize..500,
            first_lines in prop::collection::vec(diff_line_strategy(), 0..40),
            second_lines in prop::collection::vec(diff_line_strategy(), 0..40),
        ) {
            let path = PathBuf::from("src/lib.rs");
            let mut diff = format!(
                "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,1 +{first_start},1 @@\n"
            );
            let mut expected = append_diff_lines(&mut diff, first_lines, first_start);

            diff.push_str(&format!("@@ -1,1 +{second_start},1 @@\n"));
            expected.extend(append_diff_lines(&mut diff, second_lines, second_start));

            let parsed = parse_unified_diff(&diff);
            let actual = parsed.changed_lines.get(&path).cloned().unwrap_or_default();

            prop_assert!(parsed.contains_file(&path));
            prop_assert_eq!(actual, expected);
        }

        #[test]
        fn multi_file_hunks_keep_added_lines_scoped_to_their_file(
            first_start in 1usize..250,
            second_start in 251usize..500,
            first_lines in prop::collection::vec(diff_line_strategy(), 0..40),
            second_lines in prop::collection::vec(diff_line_strategy(), 0..40),
        ) {
            let first_path = PathBuf::from("src/first.rs");
            let second_path = PathBuf::from("src/second.rs");
            let mut diff = format!(
                "diff --git a/src/first.rs b/src/first.rs\n--- a/src/first.rs\n+++ b/src/first.rs\n@@ -1,1 +{first_start},1 @@\n"
            );

            let first_expected = append_diff_lines(&mut diff, first_lines, first_start);

            diff.push_str(&format!(
                "diff --git a/src/second.rs b/src/second.rs\n--- a/src/second.rs\n+++ b/src/second.rs\n@@ -1,1 +{second_start},1 @@\n"
            ));
            let second_expected = append_diff_lines(&mut diff, second_lines, second_start);

            let parsed = parse_unified_diff(&diff);
            let first_actual = parsed.changed_lines.get(&first_path).cloned().unwrap_or_default();
            let second_actual = parsed.changed_lines.get(&second_path).cloned().unwrap_or_default();

            prop_assert_eq!(first_actual, first_expected);
            prop_assert_eq!(second_actual, second_expected);
        }
    }
}
