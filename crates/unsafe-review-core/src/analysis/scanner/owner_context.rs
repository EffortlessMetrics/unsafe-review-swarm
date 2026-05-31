use super::context_slice;
use super::item_names::{parse_fn_name, parse_macro_rules_name, parse_trait_name};
use super::text_detection::{LineCommentState, line_for_text_detection};
use crate::analysis::unsafe_impl::{is_impl_declaration_line, parse_impl_declaration_owner};

const OWNER_SCAN_LIMIT: usize = 160;

pub(super) fn find_owner(lines: &[&str], idx: usize) -> Option<String> {
    for (line_idx, raw) in lines[..=idx]
        .iter()
        .enumerate()
        .rev()
        .take(OWNER_SCAN_LIMIT)
    {
        let line = raw.trim();
        if is_comment_line(line) {
            continue;
        }
        if let Some(name) = parse_fn_name(line)
            && declaration_encloses_line(lines, line_idx, idx)
        {
            return Some(name);
        }
        if let Some(name) = parse_trait_name(line)
            && declaration_encloses_line(lines, line_idx, idx)
        {
            return Some(name);
        }
        if let Some(name) = parse_impl_declaration_owner(line)
            && declaration_encloses_line(lines, line_idx, idx)
        {
            return Some(name);
        }
        if let Some(name) = parse_macro_rules_name(line)
            && declaration_encloses_line(lines, line_idx, idx)
        {
            return Some(name);
        }
        if is_impl_declaration_line(line) && declaration_encloses_line(lines, line_idx, idx) {
            return Some("impl".to_string());
        }
    }
    None
}

pub(super) fn find_following_fn_owner(lines: &[&str], idx: usize) -> Option<String> {
    for line in lines.iter().skip(idx + 1).take(8) {
        let trimmed = line.trim_start();
        if trimmed.is_empty()
            || trimmed.starts_with("#[")
            || trimmed.starts_with("///")
            || trimmed.starts_with("//")
        {
            continue;
        }
        return parse_fn_name(trimmed);
    }
    None
}

pub(super) fn find_extern_block_owner(lines: &[&str], idx: usize) -> Option<String> {
    for line in lines.iter().skip(idx).take(16) {
        let trimmed = line.trim_start();
        if trimmed.is_empty()
            || trimmed.starts_with("#[")
            || trimmed.starts_with("///")
            || trimmed.starts_with("//")
        {
            continue;
        }
        if let Some(name) = parse_fn_name(trimmed) {
            return Some(name);
        }
        if trimmed.contains('}') {
            break;
        }
    }
    None
}

pub(super) fn context_before_site(lines: &[&str], idx: usize) -> Vec<String> {
    let mut start = idx.saturating_sub(8);
    if let Some(owner_idx) = find_owner_declaration_index(lines, idx) {
        start = start.min(owner_doc_start(lines, owner_idx));
    }
    context_slice(lines, start, idx.min(lines.len()))
}

pub(super) fn find_owner_declaration_index(lines: &[&str], idx: usize) -> Option<usize> {
    let limit = idx.min(lines.len().saturating_sub(1));
    for (line_idx, raw) in lines[..=limit]
        .iter()
        .enumerate()
        .rev()
        .take(OWNER_SCAN_LIMIT)
    {
        let line = raw.trim();
        if is_comment_line(line) {
            continue;
        }
        if (parse_fn_name(line).is_some()
            || parse_trait_name(line).is_some()
            || parse_impl_declaration_owner(line).is_some()
            || parse_macro_rules_name(line).is_some())
            && declaration_encloses_line(lines, line_idx, idx)
        {
            return Some(line_idx);
        }
    }
    None
}

fn declaration_encloses_line(lines: &[&str], decl_idx: usize, idx: usize) -> bool {
    if decl_idx == idx {
        return true;
    }

    let mut state = LineCommentState::default();
    let mut depth = 0isize;
    let mut opened = false;
    for (line_idx, raw) in lines
        .iter()
        .enumerate()
        .take(idx.saturating_add(1))
        .skip(decl_idx)
    {
        let code = line_for_text_detection(raw, &mut state);
        for ch in code.chars() {
            match ch {
                '{' => {
                    depth += 1;
                    opened = true;
                }
                '}' => {
                    depth -= 1;
                    if opened && depth <= 0 && line_idx < idx {
                        return false;
                    }
                }
                _ => {}
            }
        }
    }
    opened && depth > 0
}

fn is_comment_line(line: &str) -> bool {
    line.starts_with("//") || line.starts_with("/*") || line.starts_with('*')
}

fn owner_doc_start(lines: &[&str], decl_idx: usize) -> usize {
    let mut start = decl_idx;
    let mut idx = decl_idx;
    while idx > 0 {
        let previous = lines[idx - 1].trim_start();
        if previous.starts_with("///")
            || previous.starts_with("//!")
            || previous.starts_with("#[doc")
            || previous.starts_with("#[")
            || previous.is_empty()
        {
            start = idx - 1;
            idx -= 1;
            continue;
        }
        break;
    }
    start
}
