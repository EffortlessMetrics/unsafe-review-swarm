/// Typed local state-bit predicate facts (`if <state> & <mask> != 0`).
///
/// A state-bit predicate records only what the analyzer actually knows: a
/// state bit is required on the guarded branch. It must not be read as
/// proof that the bit is semantically correct, that the state is
/// synchronized, or that any property beyond the named bit holds. Callers
/// decide which obligation a required bit supports; this module only
/// recognizes the shape.
///
/// The narrow supported shape is `if <state> & <mask> != 0`, where the mask
/// is an integer literal or a single identifier and the state is a plain
/// value path. Anything else (logical operators, calls or paths as the mask,
/// `== 0` polarity, Yoda order, nested bit combinations) fails closed
/// to `None`.
pub(super) struct StateBitPredicate<'a> {
    /// The tested state expression as written (e.g. `*slot.state.get_mut()`).
    pub(super) state: &'a str,
    /// The required bit mask as written (e.g. `WRITE`).
    pub(super) mask: &'a str,
}

/// Parse a compacted `if` condition as a state-bit predicate.
///
/// Returns `None` for every non-matching shape rather than guessing.
pub(super) fn parse_state_bit_predicate(condition: &str) -> Option<StateBitPredicate<'_>> {
    let (left, right) = split_top_level_not_equal(condition)?;
    if right.trim() != "0" {
        return None;
    }
    let state = strip_balanced_outer_parens(left.trim())?;
    let parts = split_top_level_bitand(state);
    if parts.len() != 2 {
        return None;
    }
    let mask = strip_balanced_outer_parens(parts[1].trim())?;
    let state = strip_balanced_outer_parens(parts[0].trim())?;
    if state.is_empty()
        || mask.is_empty()
        || state
            .bytes()
            .any(|b| matches!(b, b'&' | b'|' | b'!' | b'=' | b'<' | b'>'))
        || !is_bit_mask(mask)
    {
        return None;
    }
    Some(StateBitPredicate { state, mask })
}

/// Root receiver of a value path: `slot` for `slot`, `slot.state`,
/// `*slot.state.get_mut()`, `(*slot.msg.get())`, or `slots[i]`.
/// Returns `None` when no plain identifier root can be established.
pub(super) fn receiver_root(path: &str) -> Option<&str> {
    let path = path.trim().trim_start_matches(['(', '*', '&', ' ']);
    let end = path
        .char_indices()
        .find_map(|(idx, ch)| (!is_root_char(ch)).then_some(idx))
        .unwrap_or(path.len());
    let root = &path[..end];
    (!root.is_empty()
        && root
            .bytes()
            .next()
            .is_some_and(|b| b == b'_' || b.is_ascii_alphabetic()))
    .then_some(root)
}

fn is_root_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

/// Split `condition` at the first top-level `!=`. Returns the sides without
/// consuming them further.
fn split_top_level_not_equal(condition: &str) -> Option<(&str, &str)> {
    let bytes = condition.as_bytes();
    let mut depth = 0usize;
    let mut idx = 0usize;
    while idx < bytes.len() {
        match bytes[idx] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            b'!' if depth == 0 && bytes.get(idx + 1) == Some(&b'=') => {
                return Some((&condition[..idx], &condition[idx + 2..]));
            }
            _ => {}
        }
        idx += 1;
    }
    None
}

/// Split at every top-level single `&`. A `&&` contributes empty parts,
/// which callers reject by arity.
fn split_top_level_bitand(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (idx, ch) in text.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            '&' if depth == 0 => {
                parts.push(&text[start..idx]);
                start = idx + 1;
            }
            _ => {}
        }
    }
    parts.push(&text[start..]);
    parts
}

/// Strip repeated balanced outer parens: `((s))` yields `s`, while
/// `(a) + (b)` and `(s` yield `None`/unchanged respectively.
fn strip_balanced_outer_parens(text: &str) -> Option<&str> {
    let mut current = text.trim();
    loop {
        if !current.starts_with('(') {
            return Some(current);
        }
        let mut depth = 0usize;
        let mut closes_at_end = false;
        for (idx, ch) in current.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        closes_at_end = idx + 1 == current.len();
                        break;
                    }
                }
                _ => {}
            }
        }
        if depth != 0 {
            return None;
        }
        if !closes_at_end {
            return Some(current);
        }
        current = current[1..current.len() - 1].trim();
    }
}

/// A mask token is an integer literal or a single identifier of any case.
/// The pipeline lowercases evidence text, so case cannot distinguish a bit
/// constant here; what matters is the shape (one token, not a call, path,
/// or compound expression). Which bit the token names, and whether that bit
/// means initialized, is explicitly NOT established by this check: callers
/// credit only that a same-root state bit is required on the branch.
fn is_bit_mask(text: &str) -> bool {
    let text = text.trim();
    if text.is_empty() {
        return false;
    }
    let numeric = text.strip_prefix("0x").unwrap_or(text);
    let numeric = numeric.strip_prefix("0b").unwrap_or(numeric);
    let numeric = numeric.strip_prefix("0o").unwrap_or(numeric);
    if !numeric.is_empty()
        && numeric.bytes().next().is_some_and(|b| b.is_ascii_digit())
        && numeric.bytes().all(|b| b.is_ascii_hexdigit() || b == b'_')
    {
        return true;
    }
    let mut bytes = text.bytes();
    bytes
        .next()
        .is_some_and(|b| b == b'_' || b.is_ascii_alphabetic())
        && bytes.all(|b| b == b'_' || b.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::{parse_state_bit_predicate, receiver_root};

    #[test]
    fn parses_supported_predicate_shapes() {
        for (condition, state, mask) in [
            (
                "*slot.state.get_mut()&WRITE!=0",
                "*slot.state.get_mut()",
                "WRITE",
            ),
            ("(flags&MASK)!=0", "flags", "MASK"),
            ("((ready)&0x4)!=0", "ready", "0x4"),
            ("state&4!=0", "state", "4"),
        ] {
            let predicate = parse_state_bit_predicate(condition);
            assert!(predicate.is_some(), "should parse {condition}");
            if let Some(predicate) = predicate {
                assert_eq!(predicate.state, state, "{condition}");
                assert_eq!(predicate.mask, mask, "{condition}");
            }
        }
    }

    #[test]
    fn rejects_unsupported_predicate_shapes() {
        for condition in [
            // Wrong polarity: the unsafe operation sits in the unset arm.
            "*slot.state.get_mut()&WRITE==0",
            // Logical conjunction is not a bit test.
            "ready&&other!=0",
            // No bit test at all.
            "ready!=0",
            // Nonzero comparison target.
            "state&MASK!=1",
            // Yoda order.
            "0!=state&MASK",
            // Calls and paths are not plain mask tokens.
            "state&mask.count()!=0",
            "state&flags::WRITE!=0",
            // Nested combination.
            "(a&b)&MASK!=0",
            // Empty sides.
            "&MASK!=0",
            "state&!=0",
            "",
        ] {
            assert!(
                parse_state_bit_predicate(condition).is_none(),
                "should reject {condition}"
            );
        }
    }

    #[test]
    fn extracts_plain_identifier_roots() {
        for (path, root) in [
            ("slot", "slot"),
            ("slot.state", "slot"),
            ("*slot.state.get_mut()", "slot"),
            ("(*slot.msg.get())", "slot"),
            ("slots[i]", "slots"),
            ("self.buf", "self"),
        ] {
            assert_eq!(receiver_root(path), Some(root), "{path}");
        }
        for path in ["", "*(", "0slot", ".slot"] {
            assert_eq!(receiver_root(path), None, "{path}");
        }
    }
}
