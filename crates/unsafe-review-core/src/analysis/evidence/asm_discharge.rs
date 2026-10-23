use crate::analysis::scanner::ScannedSite;
use crate::domain::EvidenceState;

pub(super) const ASM_NOMEM_CONTRADICTION: &str =
    "asm template addresses memory while options declare nomem";
pub(super) const ASM_READONLY_CONTRADICTION: &str =
    "asm template writes memory through its destination operand while options declare readonly";

const GENERIC_MISSING: &str = "No obligation-specific guard code was detected";

/// Discharge slot for the `asm` obligation. Self-contradictory option
/// declarations are definite UB, so they are named as a missing state with a
/// contradiction-specific summary; every other shape keeps the generic
/// missing note. Intel bracket syntax only: AT&T parenthesised memory
/// operands and templates whose `options(...)` falls outside the scanner
/// context window stay generic.
pub(super) fn asm_discharge_state(site: &ScannedSite) -> EvidenceState {
    let text = asm_site_text(site);
    let Some(contradiction) = asm_options_contradiction(&text) else {
        return EvidenceState::missing(GENERIC_MISSING);
    };
    EvidenceState::missing(contradiction)
}

/// The site's own `asm!` text: its snippet line plus the after-context
/// window, comment-stripped and lowercased like `code_context`. Anchoring at
/// the snippet keeps an earlier `asm!` in the before-context from leaking in.
fn asm_site_text(site: &ScannedSite) -> String {
    std::iter::once(&site.site.snippet)
        .chain(site.context_after.iter())
        .map(|line| {
            line.split_once("//")
                .map_or(line.as_str(), |(code, _)| code)
        })
        .collect::<Vec<_>>()
        .join("\n")
        .to_ascii_lowercase()
}

fn asm_options_contradiction(text: &str) -> Option<&'static str> {
    let after_macro = &text[text.find("asm!(")? + "asm!(".len()..];
    let (template, after_template) = asm_template(after_macro)?;
    let options = asm_option_words(after_template);
    if options.iter().any(|word| word == "nomem") && template.contains('[') {
        return Some(ASM_NOMEM_CONTRADICTION);
    }
    if options.iter().any(|word| word == "readonly") && first_operand_is_memory(&template) {
        return Some(ASM_READONLY_CONTRADICTION);
    }
    None
}

/// The first string literal after `asm!(`, plus the text that follows it.
/// Handles plain literals with escapes and `r#"..."#` raw literals; any
/// other shape (for example `concat!`) yields `None` and stays generic.
fn asm_template(after_macro: &str) -> Option<(String, &str)> {
    let rest = after_macro.trim_start();
    if let Some(raw) = rest.strip_prefix("r#\"") {
        let end = raw.find("\"#")?;
        return Some((raw[..end].to_string(), &raw[end + 2..]));
    }
    let body = rest.strip_prefix('"')?;
    let mut template = String::new();
    let mut chars = body.char_indices();
    while let Some((index, ch)) = chars.next() {
        match ch {
            '\\' => {
                template.push(ch);
                if let Some((_, escaped)) = chars.next() {
                    template.push(escaped);
                }
            }
            '"' => return Some((template, &body[index + 1..])),
            _ => template.push(ch),
        }
    }
    None
}

/// Identifier words inside the first `options(...)` after the template.
fn asm_option_words(after_template: &str) -> Vec<String> {
    let Some(open) = after_template.find("options(") else {
        return Vec::new();
    };
    let inner = &after_template[open + "options(".len()..];
    let mut depth = 1usize;
    let mut end = inner.len();
    for (index, ch) in inner.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = index;
                    break;
                }
            }
            _ => {}
        }
    }
    inner[..end]
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .filter(|word| !word.is_empty())
        .map(str::to_string)
        .collect()
}

/// Intel syntax writes memory when the first operand (the destination, in
/// first position after the mnemonic) is a bracketed memory operand. A
/// bracket in a later operand is a memory read, which `readonly` allows.
fn first_operand_is_memory(template: &str) -> bool {
    let mut parts = template.trim().splitn(2, char::is_whitespace);
    let _mnemonic = parts.next();
    let Some(operands) = parts.next() else {
        return false;
    };
    operands
        .split(',')
        .next()
        .is_some_and(|first| first.contains('['))
}
