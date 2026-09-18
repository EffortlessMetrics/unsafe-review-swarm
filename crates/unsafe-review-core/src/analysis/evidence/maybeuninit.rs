use super::operation_scope::source_before_site_operation;
use super::state_bit_predicate::{parse_state_bit_predicate, receiver_root};
use super::{
    any_compact_if_condition, any_marker_occurrence, branch_still_open_at_operation, compact_code,
    contains_simple_assignment_to, is_receiver_path_char, receiver_before_marker,
    strip_block_comments_and_literals,
};
use crate::analysis::scanner::ScannedSite;
use crate::domain::{EvidenceState, OperationFamily};

pub(super) fn maybeuninit_assume_init_discharge_state(
    site: &ScannedSite,
    family: &OperationFamily,
    expression: &str,
    lower: &str,
) -> Option<EvidenceState> {
    if family != &OperationFamily::MaybeUninitAssumeInit {
        return None;
    }
    if is_assume_init_drop_call(expression)
        && let Some(summary) = state_bit_drop_guard_summary(site, expression, lower)
    {
        return Some(EvidenceState::present(summary));
    }
    if has_maybeuninit_assume_init_initialization_evidence(site, expression, lower) {
        Some(EvidenceState::present(
            "MaybeUninit initialization evidence was detected before assume_init",
        ))
    } else {
        None
    }
}

fn is_assume_init_drop_call(expression: &str) -> bool {
    expression
        .to_ascii_lowercase()
        .contains("assume_init_drop(")
}

/// Receiver of an `assume_init_drop` call, crossing one balanced bracket
/// group: `slot` as well as `(*slot.msg.get())`. The shared dot-receiver
/// extractor stops at parens, which would blind the drop guard to the
/// common deref-chain shape, so the drop hook extracts locally without
/// changing method-form behavior elsewhere.
fn drop_call_receiver(compact_expression: &str) -> Option<&str> {
    let marker = ".assume_init_drop(";
    let mut start = compact_expression.find(marker)?;
    let bytes = compact_expression.as_bytes();
    loop {
        while start > 0 && is_receiver_path_char(bytes[start - 1] as char) {
            start -= 1;
        }
        if start == 0 || (bytes[start - 1] != b')' && bytes[start - 1] != b']') {
            break;
        }
        let (open, close) = if bytes[start - 1] == b')' {
            (b'(', b')')
        } else {
            (b'[', b']')
        };
        let mut depth = 0usize;
        let mut idx = start - 1;
        loop {
            if bytes[idx] == close {
                depth += 1;
            } else if bytes[idx] == open {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            if idx == 0 {
                return None;
            }
            idx -= 1;
        }
        start = idx;
    }
    let receiver = compact_expression[start..compact_expression.find(marker)?].trim();
    (!receiver.is_empty()).then_some(receiver)
}

/// State-bit drop guard for `assume_init_drop` only: `if <state> & <MASK> != 0`
/// dominating the drop, testing the same slot root with no intervening
/// reassignment. Returns a summary naming the predicate. This credits the
/// required bit as evidence for initialized/drop eligibility; it does not
/// establish that the bit is semantically correct or always maintained.
fn state_bit_drop_guard_summary(
    site: &ScannedSite,
    expression: &str,
    lower: &str,
) -> Option<String> {
    let compact_expression = compact_code(&expression.to_ascii_lowercase());
    let receiver = drop_call_receiver(&compact_expression)?;
    let drop_root = receiver_root(receiver)?;
    let before_operation = source_before_site_operation(site, lower, expression)?;
    let cleaned = strip_block_comments_and_literals(&before_operation);
    let compact = compact_code(&cleaned);
    let mut found = None;
    any_compact_if_condition(&compact, |condition, after_guard| {
        let Some(predicate) = parse_state_bit_predicate(condition) else {
            return false;
        };
        let Some(state_root) = receiver_root(predicate.state) else {
            return false;
        };
        if state_root != drop_root
            || !branch_still_open_at_operation(after_guard)
            || state_reassigned_after_guard(after_guard, drop_root, predicate.state)
        {
            return false;
        }
        found = Some(format!(
            "State bit required on this branch: {} & {} != 0 supports initialized/drop eligibility",
            predicate.state, predicate.mask,
        ));
        true
    });
    found
}

/// Reassignment of the tested state between guard and operation voids the
/// guard: a rebound root or an assigned state path means the tested value no
/// longer describes the slot.
fn state_reassigned_after_guard(after_guard: &str, root: &str, state: &str) -> bool {
    if contains_simple_assignment_to(after_guard, root) {
        return true;
    }
    let path = state
        .trim()
        .trim_start_matches(['(', '*', '&', ' '])
        .split(['(', '['])
        .next()
        .unwrap_or_default()
        .trim_end();
    !path.is_empty() && contains_assignment_to_receiver_path(after_guard, path)
}

pub(super) fn has_maybeuninit_assume_init_initialization_evidence(
    site: &ScannedSite,
    expression: &str,
    lower: &str,
) -> bool {
    let Some(receiver) = maybeuninit_assume_init_receiver(expression) else {
        return false;
    };
    let Some(before_operation) = source_before_site_operation(site, lower, expression) else {
        return false;
    };
    let cleaned = strip_block_comments_and_literals(&before_operation);
    let compact = compact_code(&cleaned);
    let receiver = compact_code(&receiver);
    if receiver.is_empty() {
        return false;
    }
    let context = MaybeUninitSlotContext::new(&cleaned, &compact, receiver);

    context.has_write_evidence() || context.has_new_binding_evidence()
}

fn maybeuninit_assume_init_receiver(expression: &str) -> Option<String> {
    let compact = compact_code(&expression.to_ascii_lowercase());
    if let Some(receiver) = [
        ".assume_init(",
        ".assume_init_read(",
        ".assume_init_ref(",
        ".assume_init_mut(",
        ".assume_init_drop(",
    ]
    .into_iter()
    .find_map(|marker| receiver_before_marker(&compact, marker))
    {
        return Some(receiver.to_string());
    }
    // Assoc form `MaybeUninit::array_assume_init(slot)`: the slot is the
    // first argument, not a dot receiver. Element-wise `slot[i].write`
    // coverage is deliberately NOT credited here: loop totality cannot be
    // established textually, so only binding evidence (e.g. an array of
    // `MaybeUninit::new`) can discharge through this receiver.
    array_assume_init_first_arg(&compact).map(str::to_string)
}

/// First argument of `array_assume_init(...)` when it is a plain receiver
/// path. Anything else (deref, call result, index) fails closed to `None`.
fn array_assume_init_first_arg(compact: &str) -> Option<&str> {
    let marker = "array_assume_init(";
    let call_start = compact.find(marker)? + marker.len();
    let args = &compact[call_start..];
    let mut depth = 0usize;
    for (idx, ch) in args.char_indices() {
        match ch {
            '(' | '[' => depth += 1,
            ')' | ']' => {
                if depth == 0 {
                    return first_arg_if_receiver_path(&args[..idx]);
                }
                depth -= 1;
            }
            ',' if depth == 0 => return first_arg_if_receiver_path(&args[..idx]),
            _ => {}
        }
    }
    None
}

fn first_arg_if_receiver_path(arg: &str) -> Option<&str> {
    let arg = arg.trim();
    (!arg.is_empty() && arg.chars().all(is_receiver_path_char)).then_some(arg)
}

struct MaybeUninitSlotContext<'a> {
    cleaned: &'a str,
    compact: &'a str,
    same_slot_target: String,
    same_slot_write_marker: String,
}

impl<'a> MaybeUninitSlotContext<'a> {
    fn new(cleaned: &'a str, compact: &'a str, receiver: String) -> Self {
        let same_slot_write_marker = format!("{receiver}.write(");
        Self {
            cleaned,
            compact,
            same_slot_target: receiver,
            same_slot_write_marker,
        }
    }

    fn slot_evidence_reaches_operation(&self, evidence_pos: usize) -> bool {
        maybeuninit_evidence_scope_reaches_operation(self.compact, evidence_pos)
    }

    fn has_stale_slot_assignment(&self, text: &str) -> bool {
        contains_simple_assignment_to(text, &self.same_slot_target)
            || contains_assignment_to_receiver_path(text, &self.same_slot_target)
    }

    fn slot_stays_initialized_after(&self, evidence: &str) -> bool {
        !self.has_stale_slot_assignment(evidence)
    }

    fn slot_evidence_preserves_applicability(
        &self,
        evidence_pos: usize,
        after_evidence: &str,
    ) -> bool {
        self.slot_evidence_reaches_operation(evidence_pos)
            && self.slot_stays_initialized_after(after_evidence)
    }

    fn has_write_evidence(&self) -> bool {
        any_marker_occurrence(
            self.compact,
            &self.same_slot_write_marker,
            |marker_start, after_marker| {
                self.slot_evidence_preserves_applicability(marker_start, after_marker)
            },
        )
    }

    fn has_new_binding_evidence(&self) -> bool {
        let mut search_from = 0usize;
        while let Some(offset) = self.cleaned[search_from..].find("::new(") {
            let call_pos = search_from + offset;
            let statement_start = statement_start_before(self.cleaned, call_pos);
            let before_call = &self.cleaned[statement_start..call_pos];
            let Some((left, right)) = before_call.rsplit_once('=') else {
                search_from = call_pos + "::new(".len();
                continue;
            };
            if right.contains("maybeuninit")
                && maybeuninit_binding_left_declares_receiver(left, &self.same_slot_target)
            {
                let compact_call_pos = compact_code(&self.cleaned[..call_pos]).len();
                let after_call = &self.compact[compact_call_pos..];
                if self.slot_evidence_preserves_applicability(compact_call_pos, after_call) {
                    return true;
                }
            }
            search_from = call_pos + "::new(".len();
        }
        false
    }
}

/// Start of the statement containing `call_pos`: the last `;` outside any
/// bracket pair, or the last opened/closed block. A plain reverse search
/// would split inside array types (`[MaybeUninit<u32>; 4]`) or call
/// arguments, cutting the `let` binding off from its initializer; braces
/// always bound because a binding belongs to the block that opens before it.
fn statement_start_before(cleaned: &str, call_pos: usize) -> usize {
    let mut depth = 0usize;
    let mut boundary = 0usize;
    for (idx, ch) in cleaned[..call_pos].char_indices() {
        match ch {
            '(' | '[' | '{' => {
                depth += 1;
                if ch == '{' {
                    boundary = idx + ch.len_utf8();
                }
            }
            ')' | ']' | '}' => {
                depth = depth.saturating_sub(1);
                if ch == '}' {
                    boundary = idx + ch.len_utf8();
                }
            }
            ';' if depth == 0 => boundary = idx + ch.len_utf8(),
            _ => {}
        }
    }
    boundary
}

fn contains_assignment_to_receiver_path(compact: &str, path: &str) -> bool {
    if path.is_empty() {
        return false;
    }
    let mut cursor = compact;
    let mut offset = 0usize;
    while let Some(pos) = cursor.find(path) {
        let start = offset + pos;
        let after_path_start = start + path.len();
        let before = compact[..start].chars().next_back();
        let after_path = &compact[after_path_start..];
        if before.is_none_or(|ch| !is_receiver_path_char(ch))
            && starts_assignment_operator(after_path)
        {
            return true;
        }
        let next = pos + path.len();
        offset += next;
        cursor = &cursor[next..];
    }
    false
}

fn starts_assignment_operator(value: &str) -> bool {
    value.starts_with("<<=")
        || value.starts_with(">>=")
        || value.starts_with("+=")
        || value.starts_with("-=")
        || value.starts_with("*=")
        || value.starts_with("/=")
        || value.starts_with("%=")
        || value.starts_with("&=")
        || value.starts_with("|=")
        || value.starts_with("^=")
        || (value.starts_with('=') && !value.starts_with("==") && !value.starts_with("=>"))
}

fn maybeuninit_binding_left_declares_receiver(left: &str, receiver: &str) -> bool {
    let Some(after_let) = left.trim().strip_prefix("let ") else {
        return false;
    };
    let after_mut = after_let
        .trim_start()
        .strip_prefix("mut ")
        .unwrap_or(after_let)
        .trim_start();
    if !after_mut.starts_with(receiver) {
        return false;
    }
    let suffix = after_mut[receiver.len()..].trim_start();
    suffix.is_empty() || suffix.starts_with(':')
}

fn maybeuninit_evidence_scope_reaches_operation(compact: &str, evidence_pos: usize) -> bool {
    let mut depth_at_evidence = 0usize;
    for ch in compact[..evidence_pos].chars() {
        match ch {
            '{' => depth_at_evidence += 1,
            '}' => depth_at_evidence = depth_at_evidence.saturating_sub(1),
            _ => {}
        }
    }
    if depth_at_evidence == 0 {
        return true;
    }
    let mut depth = depth_at_evidence;
    for ch in compact[evidence_pos..].chars() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth < depth_at_evidence {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}
