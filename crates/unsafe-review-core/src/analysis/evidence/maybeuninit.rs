use super::operation_scope::source_before_site_operation;
use super::{
    any_marker_occurrence, compact_code, contains_simple_assignment_to, is_receiver_path_char,
    receiver_before_marker, strip_block_comments_and_literals,
};
use crate::analysis::scanner::ScannedSite;
use crate::domain::{EvidenceState, OperationFamily};

pub(super) fn maybeuninit_assume_init_discharge_state(
    site: &ScannedSite,
    family: &OperationFamily,
    expression: &str,
    lower: &str,
) -> Option<EvidenceState> {
    if family == &OperationFamily::MaybeUninitAssumeInit
        && has_maybeuninit_assume_init_initialization_evidence(site, expression, lower)
    {
        Some(EvidenceState::present(
            "MaybeUninit initialization evidence was detected before assume_init",
        ))
    } else {
        None
    }
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
