use crate::analysis::scanner::ScannedSite;
use crate::domain::{EvidenceState, OperationFamily};

pub(super) fn asm_options_discharge_state(
    family: &OperationFamily,
    expression: &str,
    site: &ScannedSite,
    lower: &str,
) -> EvidenceState {
    if family == &OperationFamily::InlineAsm
        && let Some(contradiction) = asm_options_contradiction(expression, site, lower)
    {
        return EvidenceState::missing(contradiction);
    }
    EvidenceState::missing("No obligation-specific guard code was detected")
}

/// Analyzes the `asm!` invocation for an options/template contradiction.
/// The operation expression carries single-line macros whole, but multiline
/// macros truncate to the macro head — so the body is re-extracted from the
/// site line onward in context (balanced parens), keeping attribution to
/// this card's own invocation.
fn asm_options_contradiction(expression: &str, site: &ScannedSite, lower: &str) -> Option<String> {
    let lowered_expression = expression.to_ascii_lowercase();
    if lowered_expression.contains("options(") {
        return contradiction_in(&lowered_expression);
    }
    contradiction_in(&macro_body_from_site(site, lower))
}

/// Joins context lines from the site line onward and returns the first
/// balanced `asm!(` body found there.
fn macro_body_from_site(site: &ScannedSite, lower: &str) -> String {
    let lines: Vec<&str> = lower.split('\n').collect();
    let site_idx = site.context_before.len().min(lines.len().saturating_sub(1));
    let from_site = lines[site_idx..].join("\n");
    let Some(start) = from_site.find("asm!(") else {
        return from_site;
    };
    balanced_body(&from_site[start + "asm!".len()..])
}

/// Returns the balanced paren body starting at the opening `(` (exclusive
/// of it), or the whole remainder when unbalanced.
fn balanced_body(after_macro_name: &str) -> String {
    let Some(body) = after_macro_name.strip_prefix('(') else {
        return after_macro_name.to_string();
    };
    let mut depth = 1usize;
    let mut end = body.len();
    let mut in_string = false;
    let mut escaped = false;
    for (idx, ch) in body.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = idx;
                    break;
                }
            }
            _ => {}
        }
    }
    body[..end].to_string()
}

/// Detects an `asm!` options list that is self-contradictory with its own
/// template: `nomem` forbids every memory operand, and `readonly` forbids a
/// bracketed destination operand. Both are definite UB when they fire, so a
/// hit names the defect; anything else stays silent.
///
/// Scope: Intel bracket syntax only. AT&T paren operands and ARM
/// source-first stores (`str x0, [x1]`) are not flagged — they keep the
/// generic note rather than risk a false contradiction.
fn contradiction_in(lowered: &str) -> Option<String> {
    let options = asm_options_list(lowered)?;
    let templates = asm_template_literals(lowered);
    if templates.is_empty() {
        return None;
    }
    if options.iter().any(|opt| opt == "nomem") && templates.iter().any(|t| t.contains('[')) {
        return Some(
            "asm template addresses memory (`[...]`) while options declare `nomem`".to_string(),
        );
    }
    if options.iter().any(|opt| opt == "readonly")
        && templates.iter().any(|t| intel_destination_writes_memory(t))
    {
        return Some(
            "asm template writes a bracketed memory destination while options declare `readonly`"
                .to_string(),
        );
    }
    None
}

/// Returns true when the literal's first operand (up to the first comma,
/// after the leading mnemonic) contains a bracketed memory operand, i.e.
/// Intel destination position.
fn intel_destination_writes_memory(template: &str) -> bool {
    let first_operand = template.split(',').next().unwrap_or(template).trim_start();
    let mnemonic_end = first_operand
        .find(|ch: char| !is_ident_char(ch))
        .unwrap_or(first_operand.len());
    first_operand[mnemonic_end..].contains('[')
}

fn is_ident_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

/// Extracts the `options(...)` identifier list from lowered macro text.
fn asm_options_list(lowered: &str) -> Option<Vec<String>> {
    let start = lowered.find("options")?;
    let after_keyword = lowered[start + "options".len()..].trim_start();
    let body = after_keyword.strip_prefix('(')?;
    let mut depth = 1usize;
    let mut end = None;
    for (idx, ch) in body.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(idx);
                    break;
                }
            }
            _ => {}
        }
    }
    Some(
        body[..end?]
            .split(',')
            .map(|opt| {
                opt.trim()
                    .trim_matches(|ch: char| !is_ident_char(ch))
                    .to_string()
            })
            .filter(|opt| !opt.is_empty())
            .collect(),
    )
}

/// Collects double-quoted and raw string literal contents from lowered text,
/// honoring backslash escapes in cooked strings.
fn asm_template_literals(lowered: &str) -> Vec<String> {
    let mut literals = Vec::new();
    let bytes = lowered.as_bytes();
    let mut idx = 0usize;
    while idx < bytes.len() {
        if bytes[idx] == b'r' && idx + 1 < bytes.len() && bytes[idx + 1] == b'"' {
            if let Some(end) = lowered[idx + 2..].find('"') {
                literals.push(lowered[idx + 2..idx + 2 + end].to_string());
                idx += 2 + end + 1;
                continue;
            }
            break;
        }
        if bytes[idx] == b'r' && idx + 2 < bytes.len() && bytes[idx + 1] == b'#' {
            let mut hashes = 0usize;
            while idx + 1 + hashes < bytes.len() && bytes[idx + 1 + hashes] == b'#' {
                hashes += 1;
            }
            if idx + 1 + hashes < bytes.len() && bytes[idx + 1 + hashes] == b'"' {
                let closer = format!("\"{}", "#".repeat(hashes));
                let body_start = idx + 2 + hashes;
                if let Some(end) = lowered[body_start..].find(&closer) {
                    literals.push(lowered[body_start..body_start + end].to_string());
                    idx = body_start + end + closer.len();
                    continue;
                }
                break;
            }
        }
        if bytes[idx] == b'"' {
            let mut content = String::new();
            let mut cursor = idx + 1;
            let mut closed = false;
            while cursor < bytes.len() {
                if bytes[cursor] == b'\\' {
                    cursor += 2;
                    continue;
                }
                if bytes[cursor] == b'"' {
                    closed = true;
                    break;
                }
                content.push(bytes[cursor] as char);
                cursor += 1;
            }
            if closed {
                literals.push(content);
                idx = cursor + 1;
                continue;
            }
            break;
        }
        idx += 1;
    }
    literals
}
