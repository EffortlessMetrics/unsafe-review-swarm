//! Bounded affected-seam relations for safe-Rust changes (issue #2319 PR1).
//!
//! An unsafe operation often stands still while its safety case moves: a
//! guard, contract, or setup fact changes inside the enclosing owner item.
//! [`relate_same_owner`] connects changed lines to the unsafe subjects they
//! may affect, using parsed same-file owner ranges rather than line
//! proximity. Proximity remains a presentation fallback elsewhere; it never
//! masquerades as a dependency edge here.
//!
//! PR1 covers same-owner relations only:
//! [`ImpactCause::EnclosingOwnerChanged`] for safe-Rust edits inside the
//! enclosing function, and [`ImpactCause::SafetyContractChanged`] for edits
//! to `# Safety` / `SAFETY:` / `Safety:` docs on that owner. Guard, setup,
//! callee, test, and configuration dependencies arrive in later slices.
//! Nothing here changes card movement or classification.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Version of the impact inventory schema. Bump when fields change meaning.
pub const IMPACT_SCHEMA_VERSION: u32 = 1;

/// Why an unchanged subject must be re-reviewed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImpactCause {
    EnclosingOwnerChanged,
    SafetyContractChanged,
}

impl ImpactCause {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::EnclosingOwnerChanged => "enclosing_owner_changed",
            Self::SafetyContractChanged => "safety_contract_changed",
        }
    }
}

/// One unsafe subject in one file, without the full card weight.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImpactSubject {
    pub card_id: String,
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
}

impl From<&crate::domain::ReviewCard> for ImpactSubject {
    fn from(card: &crate::domain::ReviewCard) -> Self {
        Self {
            card_id: card.id.to_string(),
            file: card.site.location.file.clone(),
            line: card.site.location.line,
            column: card.site.location.column.max(1),
        }
    }
}

/// One changed owner item: an `fn` whose range intersects changed lines.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ChangedItem {
    pub file: PathBuf,
    pub owner: String,
    pub start_line: usize,
    pub end_line: usize,
    pub code_changed: bool,
    pub contract_changed: bool,
    pub cfg_touched: bool,
}

/// One unchanged subject with a typed cause to re-review it.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AffectedSeam {
    pub card_id: String,
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
    pub owner: String,
    pub cause: ImpactCause,
    pub changed_lines: Vec<usize>,
}

/// Deterministic same-owner impact inventory over changed lines.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ImpactInventory {
    pub schema_version: u32,
    pub items: Vec<ChangedItem>,
    pub affected: Vec<AffectedSeam>,
    pub limitations: Vec<String>,
    pub digest: String,
}

fn line_starts(text: &str) -> Vec<usize> {
    let mut starts = vec![0usize];
    starts.extend(
        text.char_indices()
            .filter_map(|(idx, ch)| (ch == '\n').then_some(idx + ch.len_utf8())),
    );
    starts
}

fn offset_to_line(offset: usize, line_starts: &[usize]) -> usize {
    line_starts
        .partition_point(|start| *start <= offset)
        .saturating_sub(1)
        + 1
}

fn line_to_offset(text: &str, line_starts: &[usize], line: usize, column: usize) -> Option<usize> {
    let start = *line_starts.get(line.checked_sub(1)?)?;
    let mut current_col = 1usize;
    for (idx, ch) in text[start..].char_indices() {
        if current_col == column {
            return Some(start + idx);
        }
        if ch == '\n' {
            break;
        }
        current_col += 1;
    }
    if current_col == column {
        return Some(text.len());
    }
    text[start..]
        .find('\n')
        .map(|rel| start + rel)
        .or(Some(text.len()))
}

fn text_size_to_usize(size: ra_ap_syntax::TextSize) -> usize {
    u32::from(size) as usize
}

/// True for doc-comment lines (`///`, `//!`, `/**`, `/*!`) and `#[doc]`
/// attributes. Only safety-marker edits inside these become contract edges;
/// unrelated rationale edits affect nothing.
fn is_doc_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("///")
        || trimmed.starts_with("//!")
        || trimmed.starts_with("/**")
        || trimmed.starts_with("/*!")
        || trimmed.starts_with("#[doc")
}

/// True for comment lines: doc comments, line comments, and block-comment
/// openers. String literals that merely mention safety markers stay code.
fn is_comment_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    is_doc_line(line)
        || trimmed.starts_with("//")
        || trimmed.starts_with("/*")
        || trimmed.starts_with("*")
}

/// True for safety-contract markers the analyzer recognizes elsewhere
/// (`# Safety` docs, `SAFETY:` / `Safety:` comments), on comment lines only.
fn is_safety_marker(line: &str) -> bool {
    is_comment_line(line)
        && (line.contains("# Safety") || line.contains("SAFETY:") || line.contains("Safety:"))
}

/// True for configuration attribute lines. Their impact belongs to the
/// configuration slice (PR4), never to same-owner edges.
fn is_cfg_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("#[cfg(")
        || trimmed.starts_with("#[cfg_attr")
        || trimmed.starts_with("#[cfg (")
}

struct OwnerRange {
    name: String,
    /// First line including attached doc and attribute lines.
    start_line: usize,
    /// First line of the function body block. Lines above it are the doc
    /// prefix (contract docs, attributes, signature); lines at and below
    /// it are the body (call-site rationale, code).
    body_line: usize,
    end_line: usize,
}

/// Start lines of `unsafe { ... }` blocks in one file, via parsed syntax.
/// Used to notice operations the diff-proximity scope dropped before cards
/// existed: those lines get a pending-enrichment limitation, never an edge.
fn unsafe_block_lines(text: &str) -> Vec<usize> {
    use ra_ap_syntax::{Edition, SourceFile, ast::AstNode};
    let mut lines = Vec::new();
    let parse = SourceFile::parse(text, Edition::CURRENT);
    let starts = line_starts(text);
    for node in parse.tree().syntax().descendants() {
        let Some(block) = ra_ap_syntax::ast::BlockExpr::cast(node) else {
            continue;
        };
        if block.unsafe_token().is_none() {
            continue;
        }
        lines.push(offset_to_line(
            text_size_to_usize(block.syntax().text_range().start()),
            &starts,
        ));
    }
    lines.sort();
    lines.dedup();
    lines
}

/// A line that can attach to the item below it: doc comments and
/// attributes. Blank lines are not absorbed, so one item's trailing lines
/// cannot leak into the next owner's range. Used to extend an owner range
/// upward over its contract docs however the parser places them.
fn is_attached_line(line: &str) -> bool {
    is_doc_line(line) || line.trim_start().starts_with("#[")
}

/// Enclosing `fn` items in one file with 1-based line ranges, via parsed
/// syntax ranges extended upward over contiguous doc and attribute lines so
/// the owner's contract docs belong to the owner. Methods, test fns, and
/// nested fns are owners alike; modules and macros are not owners here.
fn owner_ranges(text: &str) -> Vec<OwnerRange> {
    use ra_ap_syntax::{
        Edition, SourceFile,
        ast::{AstNode, HasName},
    };
    let mut owners = Vec::new();
    let parse = SourceFile::parse(text, Edition::CURRENT);
    let starts = line_starts(text);
    let source_lines: Vec<&str> = text.lines().collect();
    for node in parse.tree().syntax().descendants() {
        let Some(func) = ra_ap_syntax::ast::Fn::cast(node) else {
            continue;
        };
        let range = func.syntax().text_range();
        let Some(name) = func.name() else { continue };
        let item_line = offset_to_line(text_size_to_usize(range.start()), &starts);
        let mut start_line = item_line;
        while start_line > 1
            && source_lines
                .get(start_line.saturating_sub(2))
                .is_some_and(|line| is_attached_line(line))
        {
            start_line -= 1;
        }
        // The body block start splits the doc prefix from the body. Without
        // a body (declarations), every line counts as prefix.
        let end_line = offset_to_line(text_size_to_usize(range.end()), &starts);
        let body_line = func
            .body()
            .map(|body| {
                offset_to_line(
                    text_size_to_usize(body.syntax().text_range().start()),
                    &starts,
                )
            })
            .unwrap_or(end_line + 1);
        owners.push(OwnerRange {
            name: name.text().to_string(),
            start_line,
            body_line,
            end_line,
        });
    }
    owners.sort_by(|left, right| {
        left.start_line
            .cmp(&right.start_line)
            .then(left.end_line.cmp(&right.end_line))
            .then(left.name.cmp(&right.name))
    });
    owners
}

/// Relate changed new-file lines to unsafe subjects through enclosing `fn`
/// ranges. Deterministic: items and edges sort by file, line, and card.
/// Files that cannot be read or parsed yield limitations, never edges.
pub fn relate_same_owner(
    root: &Path,
    changed: &BTreeMap<PathBuf, BTreeSet<usize>>,
    subjects: &[ImpactSubject],
) -> ImpactInventory {
    const PARENT_LIMITATION: &str =
        "same-file owner ranges only; cross-item, macro, and alias impact is not evaluated";
    let mut items = Vec::new();
    let mut affected = Vec::new();
    let mut limitations: Vec<String> = vec![PARENT_LIMITATION.to_string()];
    if changed.is_empty() {
        limitations
            .push("no changed lines in scope; same-owner reselect not evaluated".to_string());
    }
    // Subjects by file for one parse per file.
    let mut subjects_by_file: BTreeMap<&PathBuf, Vec<&ImpactSubject>> = BTreeMap::new();
    for subject in subjects {
        subjects_by_file
            .entry(&subject.file)
            .or_default()
            .push(subject);
    }
    let mut files: BTreeSet<&PathBuf> = changed.keys().collect();
    files.extend(subjects_by_file.keys().copied());
    // Fail closed on hostile diff paths: the same traversal, absolute, and
    // symlink guard the scanner applies before any filesystem read.
    let canonical_root = std::fs::canonicalize(root).ok();
    for file in files {
        let empty = BTreeSet::new();
        let lines = changed.get(file).unwrap_or(&empty);
        if lines.is_empty() {
            continue;
        }
        let Some(canonical_root) = canonical_root.as_deref() else {
            limitations.push(format!(
                "{} unreadable analysis root; same-owner impact unevaluated",
                file.display()
            ));
            continue;
        };
        if crate::input::diff::diff_path_escapes_root(root, canonical_root, file) {
            limitations.push(format!(
                "{} rejected as escaping the analysis root; same-owner impact unevaluated",
                file.display()
            ));
            continue;
        }
        let Ok(text) = std::fs::read_to_string(root.join(file)) else {
            limitations.push(format!(
                "{} unreadable from analysis root; same-owner impact unevaluated",
                file.display()
            ));
            continue;
        };
        let source_lines: Vec<&str> = text.lines().collect();
        let starts = line_starts(&text);
        let owners = owner_ranges(&text);
        if owners.is_empty() {
            limitations.push(format!(
                "{} has no parsed owner items; same-owner impact unevaluated",
                file.display()
            ));
            continue;
        }
        // Innermost resolution: nested functions are distinct owners, so a
        // changed line and a subject meet only inside the same innermost
        // owner. Cross-item impact stays unevaluated by design.
        let innermost = |line: usize| {
            owners
                .iter()
                .filter(|owner| owner.start_line <= line && line <= owner.end_line)
                .min_by_key(|owner| owner.end_line - owner.start_line)
        };
        let mut owner_order: Vec<usize> = Vec::new();
        let mut owner_changed: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for line in lines {
            if let Some(owner) = innermost(*line) {
                let index = owners.iter().position(|candidate| {
                    candidate.name == owner.name
                        && candidate.start_line == owner.start_line
                        && candidate.end_line == owner.end_line
                });
                if let Some(index) = index {
                    if !owner_order.contains(&index) {
                        owner_order.push(index);
                    }
                    owner_changed.entry(index).or_default().push(*line);
                }
            }
        }
        for index in owner_order {
            let owner = &owners[index];
            let changed_lines = owner_changed.get(&index).cloned().unwrap_or_default();
            let mut code_changed = false;
            let mut contract_changed = false;
            let mut cfg_touched = false;
            let mut call_site_rationale = false;
            for line in &changed_lines {
                let text_line = source_lines
                    .get(line.saturating_sub(1))
                    .copied()
                    .unwrap_or("");
                if is_cfg_line(text_line) {
                    cfg_touched = true;
                } else if *line < owner.body_line {
                    // Doc prefix: owner-level safety documentation changing is
                    // a contract edge; attributes and signature lines affect
                    // the owner item; unrelated rationale edits affect nothing.
                    if is_safety_marker(text_line) {
                        contract_changed = true;
                    } else if !is_doc_line(text_line) && !is_comment_line(text_line) {
                        code_changed = true;
                    }
                } else if is_safety_marker(text_line) {
                    // Call-site `SAFETY:` rationale inside the body is signup
                    // analysis (later slice), never a PR1 contract edge.
                    call_site_rationale = true;
                } else if is_comment_line(text_line) {
                    // Unrelated rationale edits affect nothing.
                } else {
                    code_changed = true;
                }
            }
            items.push(ChangedItem {
                file: (*file).clone(),
                owner: owner.name.clone(),
                start_line: owner.start_line,
                end_line: owner.end_line,
                code_changed,
                contract_changed,
                cfg_touched,
            });
            if cfg_touched {
                limitations.push(format!(
                    "{}:{} cfg attribute changes defer to configuration impact",
                    file.display(),
                    owner.name
                ));
            }
            if call_site_rationale {
                limitations.push(format!(
                    "{}:{} call-site SAFETY rationale changes defer to signup analysis",
                    file.display(),
                    owner.name
                ));
            }
            let file_subjects = subjects_by_file.get(file);
            let mut owner_subject_lines = BTreeSet::new();
            if let Some(file_subjects) = file_subjects {
                for subject in file_subjects.iter().filter(|subject| {
                    let Some(offset) =
                        line_to_offset(&text, &starts, subject.line, subject.column.max(1))
                    else {
                        return false;
                    };
                    let subject_line = offset_to_line(offset, &starts);
                    innermost(subject_line).is_some_and(|inner| {
                        inner.name == owner.name
                            && inner.start_line == owner.start_line
                            && inner.end_line == owner.end_line
                    })
                }) {
                    owner_subject_lines.insert(subject.line);
                    if code_changed {
                        affected.push(AffectedSeam {
                            card_id: subject.card_id.clone(),
                            file: (*file).clone(),
                            line: subject.line,
                            column: subject.column,
                            owner: owner.name.clone(),
                            cause: ImpactCause::EnclosingOwnerChanged,
                            changed_lines: changed_lines.clone(),
                        });
                    }
                    if contract_changed {
                        affected.push(AffectedSeam {
                            card_id: subject.card_id.clone(),
                            file: (*file).clone(),
                            line: subject.line,
                            column: subject.column,
                            owner: owner.name.clone(),
                            cause: ImpactCause::SafetyContractChanged,
                            changed_lines: changed_lines.clone(),
                        });
                    }
                }
            }
            // Blind-spot visibility: unsafe blocks in a changed owner with no
            // emitted subject on their line sit outside diff-proximity scope,
            // so no edge may name them. They are reported as pending
            // enrichment, never silently dropped and never invented.
            for block_line in unsafe_block_lines(&text) {
                if owner.start_line <= block_line
                    && block_line <= owner.end_line
                    && !owner_subject_lines.contains(&block_line)
                {
                    limitations.push(format!(
                        "{}:{} unsafe block at line {} has no emitted subject (outside proximity scope; pending enrichment)",
                        file.display(),
                        owner.name,
                        block_line
                    ));
                }
            }
        }
    }
    limitations.sort();
    limitations.dedup();
    affected.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then(left.line.cmp(&right.line))
            .then(left.column.cmp(&right.column))
            .then(left.card_id.cmp(&right.card_id))
            .then(left.cause.as_str().cmp(right.cause.as_str()))
    });
    let mut inventory = ImpactInventory {
        schema_version: IMPACT_SCHEMA_VERSION,
        items,
        affected,
        limitations,
        digest: String::new(),
    };
    let encoding = serde_json::to_string(&inventory).unwrap_or_default();
    inventory.digest = format!(
        "impact-sha256:{}",
        crate::sha256_hex_of(encoding.as_bytes())
    );
    inventory
}

/// Compact human impact section: one line per affected subject plus the
/// inventory boundary. Rendered only for explicit `--impact` runs so
/// default output stays byte-stable.
pub fn render_impact_human(inventory: &ImpactInventory) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Impact (same-owner relations, {}):\n",
        inventory.digest
    ));
    out.push_str(&format!(
        "- {} changed owner items, {} affected subjects\n",
        inventory.items.len(),
        inventory.affected.len()
    ));
    for seam in &inventory.affected {
        out.push_str(&format!(
            "- {} {}:{}:{} affected by {} in {} [{}]\n",
            seam.card_id,
            seam.file.display(),
            seam.line,
            seam.column,
            seam.cause.as_str(),
            seam.owner,
            seam.changed_lines
                .iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if inventory.limitations.is_empty() {
        out.push_str("- limitations: none\n");
    } else {
        out.push_str("- limitations:\n");
        for limitation in &inventory.limitations {
            out.push_str(&format!("  - {limitation}\n"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    static FIXTURE_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    struct FixtureRoot {
        root: PathBuf,
    }

    impl Drop for FixtureRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn fixture_root(files: &[(&str, &str)]) -> Result<(FixtureRoot, PathBuf), String> {
        let id = FIXTURE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "unsafe-review-impact-{}-{}",
            std::process::id(),
            id
        ));
        for (rel, body) in files {
            let path = root.join(rel);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|err| format!("fixture mkdir failed: {err}"))?;
            }
            std::fs::write(&path, body).map_err(|err| format!("fixture write failed: {err}"))?;
        }
        let dir = FixtureRoot { root: root.clone() };
        Ok((dir, root))
    }

    fn subject(id: &str, file: &str, line: usize, column: usize) -> ImpactSubject {
        ImpactSubject {
            card_id: id.to_string(),
            file: PathBuf::from(file),
            line,
            column,
        }
    }

    fn changed(file: &str, lines: &[usize]) -> BTreeMap<PathBuf, BTreeSet<usize>> {
        BTreeMap::from([(PathBuf::from(file), lines.iter().copied().collect())])
    }

    const OWNER_BODY: &str = "pub unsafe fn read_checked(ptr: *const u8, len: usize) -> u8 {\n    // padding one\n    // padding two\n    // padding three\n    // padding four\n    // padding five\n    // padding six\n    if len == 0 {\n        return 0;\n    }\n    unsafe { *ptr }\n}\n";

    #[test]
    fn guard_edit_lines_away_reselects_the_unchanged_operation() -> Result<(), String> {
        let (_dir, root) = fixture_root(&[("src/lib.rs", OWNER_BODY)])?;
        // Guard `if` sits on line 8, six lines below the fn line: proximity
        // windows stay out of this; the owner range carries the edge.
        let inventory = relate_same_owner(
            &root,
            &changed("src/lib.rs", &[8]),
            &[subject("UR-op-c1", "src/lib.rs", 11, 14)],
        );
        assert_eq!(inventory.affected.len(), 1);
        assert_eq!(
            inventory.affected[0].cause,
            ImpactCause::EnclosingOwnerChanged
        );
        assert_eq!(inventory.affected[0].owner, "read_checked");
        Ok(())
    }

    #[test]
    fn unrelated_rationale_edit_affects_nothing() -> Result<(), String> {
        let body = "/// Plain rationale, no contract.\npub unsafe fn read_checked(ptr: *const u8) -> u8 {\n    unsafe { *ptr }\n}\n";
        let (_dir, root) = fixture_root(&[("src/lib.rs", body)])?;
        let inventory = relate_same_owner(
            &root,
            &changed("src/lib.rs", &[1]),
            &[subject("UR-op-c1", "src/lib.rs", 3, 14)],
        );
        assert!(inventory.affected.is_empty());
        assert_eq!(inventory.items.len(), 1);
        assert!(!inventory.items[0].contract_changed);
        Ok(())
    }

    #[test]
    fn edit_outside_any_owner_affects_nothing() -> Result<(), String> {
        let body = "use core::ptr;\n\npub unsafe fn read_checked(ptr: *const u8) -> u8 {\n    unsafe { *ptr }\n}\n";
        let (_dir, root) = fixture_root(&[("src/lib.rs", body)])?;
        let inventory = relate_same_owner(
            &root,
            &changed("src/lib.rs", &[1]),
            &[subject("UR-op-c1", "src/lib.rs", 4, 14)],
        );
        assert!(inventory.affected.is_empty());
        assert!(inventory.items.is_empty());
        Ok(())
    }

    #[test]
    fn hostile_diff_paths_never_resolve() -> Result<(), String> {
        let (_dir, root) = fixture_root(&[("src/lib.rs", OWNER_BODY)])?;
        let hostile = BTreeMap::from([(
            PathBuf::from("../../tmp/secret.rs"),
            BTreeSet::from([1usize]),
        )]);
        let inventory = relate_same_owner(
            &root,
            &hostile,
            &[subject("UR-op-c1", "src/lib.rs", 11, 14)],
        );
        assert!(inventory.affected.is_empty());
        assert!(inventory.items.is_empty());
        assert!(
            inventory
                .limitations
                .iter()
                .any(|limitation| limitation.contains("escaping the analysis root")),
            "traversal paths must be refused visibly: {:?}",
            inventory.limitations
        );
        Ok(())
    }

    #[test]
    fn removed_guard_attributes_to_its_post_image_owner() -> Result<(), String> {
        // Deletion-only diffs carry no added lines; the deletion anchor
        // still attributes the removed guard to its owner.
        let (_dir, root) = fixture_root(&[("src/lib.rs", OWNER_BODY)])?;
        let removed = BTreeMap::from([(PathBuf::from("src/lib.rs"), BTreeSet::from([8usize]))]);
        let inventory = relate_same_owner(
            &root,
            &removed,
            &[subject("UR-op-c1", "src/lib.rs", 11, 14)],
        );
        assert_eq!(inventory.affected.len(), 1);
        assert_eq!(
            inventory.affected[0].cause,
            ImpactCause::EnclosingOwnerChanged
        );
        Ok(())
    }

    #[test]
    fn doc_prefix_safety_marker_is_a_contract_cause() -> Result<(), String> {
        let body = "/// SAFETY: caller holds the buffer.\npub unsafe fn read_checked(ptr: *const u8) -> u8 {\n    unsafe { *ptr }\n}\n";
        let (_dir, root) = fixture_root(&[("src/lib.rs", body)])?;
        let inventory = relate_same_owner(
            &root,
            &changed("src/lib.rs", &[1]),
            &[subject("UR-op-c1", "src/lib.rs", 3, 14)],
        );
        assert_eq!(inventory.affected.len(), 1);
        assert_eq!(
            inventory.affected[0].cause,
            ImpactCause::SafetyContractChanged
        );
        Ok(())
    }

    #[test]
    fn call_site_safety_rationale_is_neither_edge() -> Result<(), String> {
        // Call-site `SAFETY:` rationale inside the body is signup analysis,
        // never a PR1 edge; it stays visible as a limitation.
        let body = "pub unsafe fn read_checked(ptr: *const u8) -> u8 {\n    // SAFETY: caller holds the buffer.\n    unsafe { *ptr }\n}\n";
        let (_dir, root) = fixture_root(&[("src/lib.rs", body)])?;
        let inventory = relate_same_owner(
            &root,
            &changed("src/lib.rs", &[2]),
            &[subject("UR-op-c1", "src/lib.rs", 3, 14)],
        );
        assert!(inventory.affected.is_empty());
        assert!(
            inventory
                .limitations
                .iter()
                .any(|limitation| limitation.contains("signup analysis")),
            "deferred rationale must stay visible: {:?}",
            inventory.limitations
        );
        Ok(())
    }

    #[test]
    fn nested_functions_are_distinct_owners() -> Result<(), String> {
        let body = "pub fn outer() {\n    fn inner() {}\n    inner();\n}\n\npub unsafe fn read_checked(ptr: *const u8) -> u8 {\n    unsafe { *ptr }\n}\n";
        let (_dir, root) = fixture_root(&[("src/lib.rs", body)])?;
        // Change inside the nested function: the outer subject is untouched
        // and no edge claims the nested edit affects it.
        let inventory = relate_same_owner(
            &root,
            &changed("src/lib.rs", &[2]),
            &[subject("UR-op-c1", "src/lib.rs", 7, 14)],
        );
        assert!(inventory.affected.is_empty());
        assert_eq!(inventory.items.len(), 1);
        assert_eq!(inventory.items[0].owner, "inner");
        Ok(())
    }

    #[test]
    fn proximity_dropped_operations_stay_visible() -> Result<(), String> {
        // No emitted subject for the unsafe block: the owner still changed,
        // so the blind spot is named as pending enrichment, not silence.
        let (_dir, root) = fixture_root(&[("src/lib.rs", OWNER_BODY)])?;
        let inventory = relate_same_owner(&root, &changed("src/lib.rs", &[8]), &[]);
        assert!(inventory.affected.is_empty());
        assert!(
            inventory
                .limitations
                .iter()
                .any(|limitation| limitation.contains("pending enrichment")),
            "dropped operations must stay visible: {:?}",
            inventory.limitations
        );
        Ok(())
    }

    #[test]
    fn inventory_is_deterministic() -> Result<(), String> {
        let (_dir, root) = fixture_root(&[("src/lib.rs", OWNER_BODY)])?;
        let changed_map = changed("src/lib.rs", &[8]);
        let subjects = [subject("UR-op-c1", "src/lib.rs", 11, 14)];
        let first = relate_same_owner(&root, &changed_map, &subjects);
        let second = relate_same_owner(&root, &changed_map, &subjects);
        assert_eq!(first.digest, second.digest);
        assert_eq!(first, second);
        let rendered = render_impact_human(&first);
        assert!(rendered.contains("enclosing_owner_changed"));
        Ok(())
    }
}
