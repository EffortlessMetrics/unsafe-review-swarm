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
/// operands, address computations (`lea`), read-only first operands
/// (`cmp`/`test`), and templates whose `options(...)` falls outside the
/// scanner context window stay generic.
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
    let invocation = invocation_prefix(after_macro);
    let (template, after_template) = asm_templates(invocation)?;
    let options = asm_option_words(after_template);
    if options.iter().any(|word| word == "nomem") && template_accesses_memory(&template) {
        return Some(ASM_NOMEM_CONTRADICTION);
    }
    if options.iter().any(|word| word == "readonly") && template_writes_memory(&template) {
        return Some(ASM_READONLY_CONTRADICTION);
    }
    None
}

/// Text up to the closing delimiter of this `asm!(` invocation, so a later
/// `asm!` in the context window cannot lend its options to this site.
/// String literals are skipped; an invocation truncated by the context
/// window simply runs to the end of the text.
fn invocation_prefix(after_macro: &str) -> &str {
    let bytes = after_macro.as_bytes();
    let mut depth = 1usize;
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => index = string_end(bytes, index),
            b'(' | b'[' | b'{' => {
                depth += 1;
                index += 1;
            }
            b')' | b']' | b'}' => {
                depth -= 1;
                if depth == 0 {
                    return &after_macro[..index];
                }
                index += 1;
            }
            _ => index += 1,
        }
    }
    after_macro
}

/// Index just past the string literal that starts at `start` (a `"` byte),
/// honouring backslash escapes.
fn string_end(bytes: &[u8], start: usize) -> usize {
    let mut index = start + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'"' => return index + 1,
            _ => index += 1,
        }
    }
    bytes.len()
}

/// All leading template literals of the invocation, concatenated, plus the
/// text that follows them. `asm!` accepts several consecutive string
/// literals (plain with escapes, or `r#"..."#` raw); any other shape (for
/// example `concat!`) yields `None` and stays generic.
fn asm_templates(invocation: &str) -> Option<(String, &str)> {
    let mut template = String::new();
    let mut rest = invocation;
    loop {
        let (literal, after) = string_literal(rest.trim_start())?;
        if !template.is_empty() {
            template.push('\n');
        }
        template.push_str(&literal);
        rest = after;
        // Another template follows only when the next token past an
        // optional comma is itself a string literal; otherwise `rest`
        // already points at the operand list.
        let candidate = rest.trim_start();
        let candidate = candidate
            .strip_prefix(',')
            .unwrap_or(candidate)
            .trim_start();
        if candidate.starts_with('"') || candidate.starts_with("r#\"") {
            rest = candidate;
        } else {
            return Some((template, rest));
        }
    }
}

fn string_literal(text: &str) -> Option<(String, &str)> {
    if let Some(raw) = text.strip_prefix("r#\"") {
        let end = raw.find("\"#")?;
        return Some((raw[..end].to_string(), &raw[end + 2..]));
    }
    let body = text.strip_prefix('"')?;
    let mut literal = String::new();
    let mut chars = body.char_indices();
    while let Some((index, ch)) = chars.next() {
        match ch {
            '\\' => {
                literal.push(ch);
                if let Some((_, escaped)) = chars.next() {
                    literal.push(escaped);
                }
            }
            '"' => return Some((literal, &body[index + 1..])),
            _ => literal.push(ch),
        }
    }
    None
}

/// Identifier words inside the first `options(...)` after the templates.
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

/// Individual instructions of a template, split on statement separators.
/// Escaped `\n` sequences in the scanned literal are two characters, so
/// they are normalized first; a leading escaped `\t` is indentation.
fn instructions(template: &str) -> impl Iterator<Item = &str> {
    template
        .split([';', '\n'])
        .flat_map(|piece| piece.split("\\n"))
        .map(|piece| {
            let mut instruction = piece.trim();
            while let Some(rest) = instruction.strip_prefix("\\t") {
                instruction = rest.trim_start();
            }
            instruction
        })
}

fn mnemonic(instruction: &str) -> &str {
    instruction
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_end_matches(':')
}

/// Any bracketed operand outside an `lea` is a memory access: `lea` only
/// computes the address and never reads or writes memory.
fn template_accesses_memory(template: &str) -> bool {
    instructions(template)
        .any(|instruction| mnemonic(instruction) != "lea" && instruction.contains('['))
}

/// Intel syntax writes memory when a writing instruction's first operand
/// (the destination, in first position after the mnemonic) is a bracketed
/// memory operand. Read-only first operands (`cmp`, `test`, `push`, jumps)
/// and unclassified mnemonics stay generic.
fn template_writes_memory(template: &str) -> bool {
    instructions(template).any(|instruction| {
        writes_first_operand(mnemonic(instruction)) && first_operand_is_memory(instruction)
    })
}

fn writes_first_operand(mnemonic: &str) -> bool {
    mnemonic.starts_with("mov")
        || mnemonic.starts_with("set")
        || matches!(
            mnemonic,
            "add"
                | "adc"
                | "sub"
                | "sbb"
                | "and"
                | "or"
                | "xor"
                | "inc"
                | "dec"
                | "neg"
                | "not"
                | "shl"
                | "shr"
                | "sar"
                | "sal"
                | "rol"
                | "ror"
                | "xchg"
                | "cmpxchg"
                | "bts"
                | "btr"
                | "btc"
        )
}

fn first_operand_is_memory(instruction: &str) -> bool {
    let mut parts = instruction.splitn(2, char::is_whitespace);
    let _mnemonic = parts.next();
    let Some(operands) = parts.next() else {
        return false;
    };
    operands
        .split(',')
        .next()
        .is_some_and(|first| first.contains('['))
}
