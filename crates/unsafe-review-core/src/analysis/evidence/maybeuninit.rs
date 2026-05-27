use super::{
    any_marker_occurrence, code_before_operation, compact_code, contains_simple_assignment_to,
    receiver_before_marker, strip_block_comments_and_literals,
};

pub(super) fn has_maybeuninit_assume_init_initialization_evidence(
    expression: &str,
    lower: &str,
) -> bool {
    let Some(receiver) = maybeuninit_assume_init_receiver(expression) else {
        return false;
    };
    let Some(before_operation) = code_before_operation(lower, expression) else {
        return false;
    };
    let compact = compact_code(&strip_block_comments_and_literals(&before_operation));
    let receiver = compact_code(&receiver);
    if receiver.is_empty() {
        return false;
    }
    let context = MaybeUninitSlotContext::new(&compact, receiver);

    context.has_write_evidence() || context.has_new_binding_evidence()
}

fn maybeuninit_assume_init_receiver(expression: &str) -> Option<String> {
    let compact = compact_code(&expression.to_ascii_lowercase());
    [
        ".assume_init(",
        ".assume_init_read(",
        ".assume_init_ref(",
        ".assume_init_mut(",
        ".assume_init_drop(",
    ]
    .into_iter()
    .find_map(|marker| receiver_before_marker(&compact, marker))
    .map(str::to_string)
}

struct MaybeUninitSlotContext<'a> {
    compact: &'a str,
    same_slot_target: String,
    same_slot_write_marker: String,
}

impl<'a> MaybeUninitSlotContext<'a> {
    fn new(compact: &'a str, receiver: String) -> Self {
        let same_slot_write_marker = format!("{receiver}.write(");
        Self {
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
        any_marker_occurrence(self.compact, "::new(", |call_pos, after_call| {
            let statement_start = self.compact[..call_pos]
                .rfind([';', '{', '}'])
                .map_or(0, |idx| idx + 1);
            let before_call = &self.compact[statement_start..call_pos];
            let Some((left, right)) = before_call.rsplit_once('=') else {
                return false;
            };
            if right.contains("maybeuninit")
                && maybeuninit_binding_left_declares_receiver(left, &self.same_slot_target)
                && self.slot_evidence_preserves_applicability(call_pos, after_call)
            {
                return true;
            }
            false
        })
    }
}

fn maybeuninit_binding_left_declares_receiver(left: &str, receiver: &str) -> bool {
    left == format!("let{receiver}")
        || left == format!("letmut{receiver}")
        || left.starts_with(&format!("let{receiver}:"))
        || left.starts_with(&format!("letmut{receiver}:"))
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
