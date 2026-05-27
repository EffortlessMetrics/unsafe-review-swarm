use super::{
    branch_still_open_at_operation, compact_code, contains_executable_return,
    is_receiver_path_char, matching_code_block_end,
};

pub(super) fn has_encode_utf8_remaining_capacity_evidence(lower: &str) -> bool {
    let compact = compact_code(lower);
    let context = EncodeUtf8CapacityContext::new(&compact);
    context.has_same_remaining_capacity_argument_evidence()
}

// encode_utf8 contract evidence is scoped to the call shape that passes the
// local remaining capacity value through to the unsafe function.
struct EncodeUtf8CapacityContext<'a> {
    compact: &'a str,
}

impl<'a> EncodeUtf8CapacityContext<'a> {
    fn new(compact: &'a str) -> Self {
        Self { compact }
    }

    fn has_same_remaining_capacity_argument_evidence(&self) -> bool {
        self.has_encode_utf8_remaining_capacity_call()
            && self.has_remaining_capacity_binding()
            && self.has_pointer_argument()
    }

    fn has_encode_utf8_remaining_capacity_call(&self) -> bool {
        self.compact.contains("encode_utf8(c,ptr,remaining_cap)")
    }

    fn has_remaining_capacity_binding(&self) -> bool {
        self.compact.contains("remaining_cap=self.capacity()-len")
    }

    fn has_pointer_argument(&self) -> bool {
        self.compact.contains("ptr")
    }
}

pub(super) fn has_unchecked_constructor_availability_evidence(
    expression: &str,
    lower: &str,
) -> bool {
    let compact_expression = compact_code(&expression.to_ascii_lowercase());
    let Some(receiver) = unchecked_constructor_receiver(&compact_expression) else {
        return false;
    };
    let compact = compact_code(lower);
    let before_call = compact
        .find(&compact_expression)
        .map_or(compact.as_str(), |call_pos| &compact[..call_pos]);
    let context = UncheckedConstructorAvailabilityContext::new(before_call, receiver);
    context.has_availability_guard()
}

// Availability evidence for new_unchecked constructors must target the same
// receiver type and must dominate the constructor call.
struct UncheckedConstructorAvailabilityContext<'a> {
    before_call: &'a str,
    predicate: String,
}

impl<'a> UncheckedConstructorAvailabilityContext<'a> {
    fn new(before_call: &'a str, receiver: &str) -> Self {
        Self {
            before_call,
            predicate: format!("{receiver}::is_available()"),
        }
    }

    fn has_availability_guard(&self) -> bool {
        self.has_availability_assertion()
            || self.has_open_availability_branch()
            || self.has_unavailable_early_return()
    }

    fn has_availability_assertion(&self) -> bool {
        [
            format!("assert!({})", self.predicate),
            format!("assert!({},", self.predicate),
            format!("debug_assert!({})", self.predicate),
            format!("debug_assert!({},", self.predicate),
        ]
        .iter()
        .any(|pattern| self.before_call.contains(pattern))
    }

    fn has_open_availability_branch(&self) -> bool {
        let guard = format!("if{}{{", self.predicate);
        self.any_guard_tail(&guard, branch_still_open_at_operation)
    }

    fn has_unavailable_early_return(&self) -> bool {
        let guard = format!("if!{}{{", self.predicate);
        self.any_guard_tail(&guard, |after_guard| {
            let guard_body = matching_code_block_end(after_guard)
                .map_or(after_guard, |body_end| &after_guard[..body_end]);
            contains_executable_return(guard_body)
        })
    }

    fn any_guard_tail(&self, guard: &str, mut applies: impl FnMut(&str) -> bool) -> bool {
        let mut search_from = 0;
        while let Some(offset) = self.before_call[search_from..].find(guard) {
            let guard_start = search_from + offset;
            let after_guard = &self.before_call[guard_start + guard.len()..];
            if applies(after_guard) {
                return true;
            }
            search_from = guard_start + guard.len();
        }
        false
    }
}

fn unchecked_constructor_receiver(compact_expression: &str) -> Option<&str> {
    let call_pos = compact_expression.find("::new_unchecked")?;
    let before_call = &compact_expression[..call_pos];
    let receiver_start = before_call
        .char_indices()
        .rev()
        .find_map(|(idx, ch)| (!is_receiver_path_char(ch)).then_some(idx + ch.len_utf8()))
        .unwrap_or(0);
    let receiver = &before_call[receiver_start..];
    (!receiver.is_empty()).then_some(receiver)
}
