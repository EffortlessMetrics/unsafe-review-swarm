use super::{
    code_context_through_site, compact_code, contains_receiver_path, contains_simple_assignment_to,
    has_assignment_to_any_identifier, has_fresh_guard_pattern_for_identifiers,
    has_open_positive_branch_guard_for_identifiers, is_receiver_path_char, is_simple_identifier,
    let_binding_name, matching_call_argument_end, matching_code_block_end,
    strip_block_comments_and_literals,
};
use crate::analysis::scanner::ScannedSite;
use crate::domain::{EvidenceState, OperationFamily};

pub(super) fn has_set_len_capacity_evidence(lower: &str) -> bool {
    has_set_len_shrink_evidence(lower)
        || has_set_len_call_result_initialization_evidence(lower)
        || has_set_len_const_cap_evidence(lower)
        || has_set_len_with_capacity_evidence(lower)
        || has_set_len_reserve_capacity_evidence(lower)
        || has_capacity_bound_guard_for_call(lower)
}

pub(super) fn has_set_len_initialization_evidence(lower: &str) -> bool {
    if has_set_len_shrink_evidence(lower) || has_set_len_call_result_initialization_evidence(lower)
    {
        return true;
    }
    let compact = compact_code(lower);
    let Some(context) = set_len_call_context(&compact) else {
        return false;
    };
    context.has_initialized_range_evidence()
}

pub(super) fn set_len_initialized_discharge_state(site: &ScannedSite) -> Option<EvidenceState> {
    if site.operation.family != OperationFamily::VecSetLen {
        return None;
    }
    let init_scope = code_context_through_site(site).to_ascii_lowercase();
    if has_set_len_initialization_evidence(&init_scope) {
        Some(EvidenceState::present(
            "Initialization evidence was detected",
        ))
    } else {
        Some(EvidenceState::missing(
            "No initialization evidence was detected",
        ))
    }
}

pub(super) fn has_capacity_bound_guard(
    before_call: &str,
    same_vec_target: &str,
    set_len_argument: &str,
) -> bool {
    SetLenCapacityContext {
        before_call,
        same_vec_target,
        set_len_argument,
    }
    .has_capacity_bound_guard()
}

pub(super) fn has_const_capacity_evidence(
    before_call: &str,
    same_vec_target: &str,
    set_len_argument: &str,
) -> bool {
    SetLenCapacityContext {
        before_call,
        same_vec_target,
        set_len_argument,
    }
    .has_const_capacity_evidence()
}

pub(super) fn has_reserve_capacity_evidence(
    before_call: &str,
    same_vec_target: &str,
    set_len_argument: &str,
) -> bool {
    SetLenCapacityContext {
        before_call,
        same_vec_target,
        set_len_argument,
    }
    .has_reserve_capacity_evidence()
}

pub(super) fn has_with_capacity_evidence(
    before_call: &str,
    same_vec_target: &str,
    set_len_argument: &str,
) -> bool {
    SetLenCapacityContext {
        before_call,
        same_vec_target,
        set_len_argument,
    }
    .has_with_capacity_evidence()
}

pub(super) fn has_initialized_range_evidence(
    before_call: &str,
    same_vec_target: &str,
    set_len_argument: &str,
) -> bool {
    SetLenInitializedRangeContext {
        before_call,
        same_vec_target,
        set_len_argument,
    }
    .has_initialized_range_evidence()
}

pub(super) fn has_call_result_initialization_evidence(
    before_call: &str,
    set_len_argument: &str,
) -> bool {
    SetLenCallResultInitializationContext {
        before_call,
        set_len_argument,
    }
    .has_call_result_initialization_evidence()
}

fn has_capacity_bound_guard_for_call(lower: &str) -> bool {
    let compact = compact_code(lower);
    let Some(context) = set_len_call_context(&compact) else {
        return false;
    };
    context.has_capacity_bound_guard()
}

fn has_set_len_const_cap_evidence(lower: &str) -> bool {
    let compact = compact_code(lower);
    let Some(context) = set_len_call_context(&compact) else {
        return false;
    };
    context.has_const_capacity_evidence()
}

fn has_set_len_with_capacity_evidence(lower: &str) -> bool {
    let compact = compact_code(lower);
    let Some(context) = set_len_call_context(&compact) else {
        return false;
    };
    context.has_with_capacity_evidence()
}

fn has_set_len_reserve_capacity_evidence(lower: &str) -> bool {
    let compact = compact_code(lower);
    let Some(context) = set_len_call_context(&compact) else {
        return false;
    };
    context.has_reserve_capacity_evidence()
}

fn set_len_receiver_and_argument(compact: &str) -> Option<(&str, &str)> {
    let marker = ".set_len(";
    let call_pos = compact.find(marker)?;
    let before_call = &compact[..call_pos];
    let receiver_start = before_call
        .char_indices()
        .rev()
        .find_map(|(idx, ch)| (!is_receiver_path_char(ch)).then_some(idx + ch.len_utf8()))
        .unwrap_or(0);
    let receiver = &before_call[receiver_start..];
    let argument_text = &compact[call_pos + marker.len()..];
    let argument_end = matching_call_argument_end(argument_text)?;
    let argument = &argument_text[..argument_end];
    (!receiver.is_empty() && !argument.is_empty()).then_some((receiver, argument))
}

struct SetLenApplicabilityContext<'a> {
    before_call: &'a str,
    same_vec_target: &'a str,
    set_len_argument: &'a str,
}

impl<'a> SetLenApplicabilityContext<'a> {
    fn has_capacity_bound_guard(&self) -> bool {
        has_capacity_bound_guard(
            self.before_call,
            self.same_vec_target,
            self.set_len_argument,
        )
    }

    fn has_const_capacity_evidence(&self) -> bool {
        has_const_capacity_evidence(
            self.before_call,
            self.same_vec_target,
            self.set_len_argument,
        )
    }

    fn has_reserve_capacity_evidence(&self) -> bool {
        has_reserve_capacity_evidence(
            self.before_call,
            self.same_vec_target,
            self.set_len_argument,
        )
    }

    fn has_with_capacity_evidence(&self) -> bool {
        has_with_capacity_evidence(
            self.before_call,
            self.same_vec_target,
            self.set_len_argument,
        )
    }

    fn has_call_result_initialization_evidence(&self) -> bool {
        has_call_result_initialization_evidence(self.before_call, self.set_len_argument)
    }

    fn has_initialized_range_evidence(&self) -> bool {
        has_initialized_range_evidence(
            self.before_call,
            self.same_vec_target,
            self.set_len_argument,
        )
    }
}

fn set_len_call_context(compact: &str) -> Option<SetLenApplicabilityContext<'_>> {
    let (receiver, new_len) = set_len_receiver_and_argument(compact)?;
    let marker = format!("{receiver}.set_len(");
    let call_pos = compact.find(&marker)?;
    Some(SetLenApplicabilityContext {
        before_call: &compact[..call_pos],
        same_vec_target: receiver,
        set_len_argument: new_len,
    })
}

fn has_set_len_call_result_initialization_evidence(lower: &str) -> bool {
    let compact = compact_code(lower);
    let Some(context) = set_len_call_context(&compact) else {
        return false;
    };
    context.has_call_result_initialization_evidence()
}

fn has_set_len_shrink_evidence(lower: &str) -> bool {
    super::super::set_len_shrink::has_set_len_shrink_evidence(lower)
}

struct SetLenCapacityContext<'a> {
    before_call: &'a str,
    same_vec_target: &'a str,
    set_len_argument: &'a str,
}

impl<'a> SetLenCapacityContext<'a> {
    fn has_capacity_bound_guard(&self) -> bool {
        if self.has_remaining_capacity_guard() {
            return true;
        }
        for capacity in self.capacity_terms() {
            if self.has_capacity_relation(&capacity) {
                return true;
            }
        }
        self.capacity_bindings()
            .into_iter()
            .any(|capacity| self.has_capacity_relation(capacity))
    }

    fn capacity_terms(&self) -> [String; 2] {
        [
            format!("{}.capacity()", self.same_vec_target),
            format!("{}.cap()", self.same_vec_target),
        ]
    }

    fn capacity_bindings(&self) -> Vec<&'a str> {
        self.before_call
            .split(';')
            .filter_map(|statement| {
                let (left, right) = statement.split_once('=')?;
                let binding = let_binding_name(left)?;
                let right = right.trim();
                ((right == format!("{}.capacity()", self.same_vec_target)
                    || right == format!("{}.cap()", self.same_vec_target))
                    && !binding.is_empty())
                .then_some(binding)
            })
            .collect()
    }

    fn has_remaining_capacity_guard(&self) -> bool {
        has_set_len_remaining_capacity_guard(
            self.before_call,
            self.same_vec_target,
            self.set_len_argument,
        )
    }

    fn has_capacity_relation(&self, capacity: &str) -> bool {
        has_set_len_capacity_relation(
            self.before_call,
            self.set_len_argument,
            capacity,
            self.same_vec_target,
        )
    }

    fn has_const_capacity_evidence(&self) -> bool {
        self.set_len_argument == "cap"
            && (self.before_call.contains("maybeuninit::uninit();cap")
                || self.before_call.contains(";cap]"))
    }

    fn has_reserve_capacity_evidence(&self) -> bool {
        if !is_simple_identifier(self.set_len_argument) {
            return false;
        }
        let mut consumed = 0usize;
        for statement in self.before_call.split_inclusive(';') {
            let Some((left, right)) = statement.trim_end_matches(';').split_once('=') else {
                consumed += statement.len();
                continue;
            };
            if let_binding_name(left) != Some(self.set_len_argument) {
                consumed += statement.len();
                continue;
            }
            let Some(additional) = len_plus_additional_argument(right.trim(), self.same_vec_target)
            else {
                consumed += statement.len();
                continue;
            };
            let after_len_binding = &self.before_call[consumed + statement.len()..];
            if has_fresh_set_len_reserve_call(
                after_len_binding,
                self.same_vec_target,
                self.set_len_argument,
                additional,
            ) {
                return true;
            }
            consumed += statement.len();
        }
        false
    }

    fn has_with_capacity_evidence(&self) -> bool {
        self.before_call.split(';').any(|statement| {
            let Some((left, right)) = statement.split_once('=') else {
                return false;
            };
            let Some(binding) = let_binding_name(left) else {
                return false;
            };
            binding == self.same_vec_target
                && with_capacity_argument(right).is_some_and(|arg| arg == self.set_len_argument)
        })
    }
}

struct SetLenCallResultInitializationContext<'a> {
    before_call: &'a str,
    set_len_argument: &'a str,
}

impl SetLenCallResultInitializationContext<'_> {
    fn has_call_result_initialization_evidence(&self) -> bool {
        self.before_call.contains("encode_utf8(")
            && (self.set_len_argument == "len+n" || self.set_len_argument == "old_len+n")
    }
}

struct SetLenInitializedRangeContext<'a> {
    before_call: &'a str,
    same_vec_target: &'a str,
    set_len_argument: &'a str,
}

impl<'a> SetLenInitializedRangeContext<'a> {
    fn has_initialized_range_evidence(&self) -> bool {
        self.has_initialization_loop()
    }

    fn has_initialization_loop(&self) -> bool {
        let slice_bindings = self.slice_bindings();
        self.before_call.split('}').any(|block| {
            let Some((head, body)) = block.rsplit_once('{') else {
                return false;
            };
            self.loop_initializes_same_vec(head, body, &slice_bindings)
        })
    }

    fn loop_initializes_same_vec(&self, head: &str, body: &str, slice_bindings: &[&str]) -> bool {
        self.loop_iterates_receiver(head, slice_bindings)
            && head.contains(".iter_mut(")
            && has_initialization_marker(body)
    }

    fn slice_bindings(&self) -> Vec<&'a str> {
        let mut bindings = Vec::new();
        let mut consumed = 0usize;
        for statement in self.before_call.split_inclusive(';') {
            let statement_without_semicolon = statement.trim_end_matches(';');
            let after_binding =
                &self.before_call[(consumed + statement.len()).min(self.before_call.len())..];
            if let Some(binding) =
                self.fresh_slice_binding(statement_without_semicolon, after_binding)
            {
                bindings.push(binding);
            }
            consumed += statement.len();
        }
        bindings
    }

    fn fresh_slice_binding(&self, statement: &'a str, after_binding: &str) -> Option<&'a str> {
        let (left, right) = statement.split_once('=')?;
        let binding = let_binding_name(left)?;
        let right = right.trim();
        (set_len_slice_binding_references_receiver(right, self.same_vec_target)
            && set_len_slice_binding_covers_argument(right, self.set_len_argument)
            && right.contains('[')
            && right.contains("..")
            && !contains_simple_assignment_to(after_binding, self.same_vec_target)
            && !contains_direct_binding_assignment_to(after_binding, binding))
        .then_some(binding)
    }

    fn loop_iterates_receiver(&self, head: &str, slice_bindings: &[&str]) -> bool {
        contains_receiver_path(head, self.same_vec_target)
            || head.contains(&format!("in{}.", self.same_vec_target))
            || slice_bindings.iter().any(|binding| {
                contains_receiver_path(head, binding) || head.contains(&format!("in{binding}."))
            })
    }
}

fn set_len_slice_binding_references_receiver(right: &str, receiver: &str) -> bool {
    let right = right
        .strip_prefix("&mut")
        .or_else(|| right.strip_prefix('&'))
        .unwrap_or(right);
    contains_receiver_path(right, receiver)
}

fn set_len_slice_binding_covers_argument(right: &str, set_len_argument: &str) -> bool {
    let right = right
        .strip_prefix("&mut")
        .or_else(|| right.strip_prefix('&'))
        .unwrap_or(right);
    let Some(range_start) = right.find('[') else {
        return false;
    };
    let range = &right[range_start + 1..];
    let Some(range_end) = range.find(']') else {
        return false;
    };
    let range = &range[..range_end];
    let Some((_start, end)) = range.split_once("..") else {
        return false;
    };
    end == set_len_argument
}

fn contains_direct_binding_assignment_to(compact: &str, name: &str) -> bool {
    if compact.contains(&format!("let{name}="))
        || compact.contains(&format!("letmut{name}="))
        || compact.contains(&format!("let{name}:"))
        || compact.contains(&format!("letmut{name}:"))
    {
        return true;
    }
    let marker = format!("{name}=");
    let mut cursor = compact;
    let mut offset = 0usize;
    while let Some(pos) = cursor.find(&marker) {
        let start = offset + pos;
        let before = compact[..start].chars().next_back();
        let after_equals = compact[start + marker.len()..].chars().next();
        if before.is_none_or(|ch| ch != '*' && !is_receiver_path_char(ch))
            && after_equals != Some('=')
        {
            return true;
        }
        let next = pos + marker.len();
        offset += next;
        cursor = &cursor[next..];
    }
    false
}

fn has_initialization_marker(statement: &str) -> bool {
    statement.contains("maybeuninit::new")
        || statement.contains(".write(")
        || statement.contains("ptr::write")
        || statement.contains("copy_nonoverlapping")
        || statement.contains("copy_to_nonoverlapping")
}

fn has_set_len_remaining_capacity_guard(before_call: &str, receiver: &str, new_len: &str) -> bool {
    if !is_simple_identifier(new_len) {
        return false;
    }
    let receiver_len = format!("{receiver}.len()");
    let mut receiver_len_bindings = Vec::new();
    let mut consumed = 0usize;
    for statement in before_call.split_inclusive(';') {
        let statement_without_semicolon = statement.trim_end_matches(';');
        let Some((left, right)) = statement_without_semicolon.split_once('=') else {
            consumed += statement.len();
            continue;
        };
        let Some(binding) = let_binding_name(left) else {
            consumed += statement.len();
            continue;
        };
        let right = right.trim();
        if right == receiver_len {
            receiver_len_bindings.push(binding);
        }
        if binding == new_len
            && let Some((len_term, additional)) =
                set_len_growth_terms(right, &receiver_len, &receiver_len_bindings)
        {
            let before_new_len_binding = &before_call[..consumed];
            let after_new_len_binding =
                &before_call[(consumed + statement.len()).min(before_call.len())..];
            if has_fresh_remaining_capacity_early_return(
                before_new_len_binding,
                receiver,
                &receiver_len,
                additional,
            ) && set_len_growth_terms_stay_fresh(
                after_new_len_binding,
                receiver,
                new_len,
                len_term,
                additional,
            ) {
                return true;
            }
        }
        consumed += statement.len();
    }
    false
}

fn set_len_growth_terms<'a>(
    expression: &'a str,
    receiver_len: &str,
    receiver_len_bindings: &[&str],
) -> Option<(&'a str, &'a str)> {
    let (left, right) = expression.split_once('+')?;
    let left = left.trim();
    let right = right.trim();
    if set_len_growth_len_term_matches(left, receiver_len, receiver_len_bindings) {
        return Some((left, right));
    }
    if set_len_growth_len_term_matches(right, receiver_len, receiver_len_bindings) {
        return Some((right, left));
    }
    None
}

fn set_len_growth_len_term_matches(
    term: &str,
    receiver_len: &str,
    receiver_len_bindings: &[&str],
) -> bool {
    term == receiver_len || receiver_len_bindings.iter().any(|binding| term == *binding)
}

fn has_fresh_remaining_capacity_early_return(
    before_new_len_binding: &str,
    receiver: &str,
    receiver_len: &str,
    additional: &str,
) -> bool {
    let Some(additional_stale_identifier) = expression_base_identifier(additional) else {
        return false;
    };
    for capacity in [
        format!("{receiver}.capacity()"),
        format!("{receiver}.cap()"),
    ] {
        let remaining = format!("{capacity}-{receiver_len}");
        for predicate in [
            format!("{additional}>{remaining}"),
            format!("{remaining}<{additional}"),
        ] {
            if remaining_capacity_early_return_matches(
                before_new_len_binding,
                &predicate,
                receiver,
                additional_stale_identifier,
            ) {
                return true;
            }
        }
    }
    false
}

fn remaining_capacity_early_return_matches(
    before_new_len_binding: &str,
    predicate: &str,
    receiver: &str,
    additional_stale_identifier: &str,
) -> bool {
    let guard = format!("if{predicate}{{");
    let mut search_from = 0usize;
    while let Some(offset) = before_new_len_binding[search_from..].find(&guard) {
        let guard_start = search_from + offset;
        let after_guard = &before_new_len_binding[guard_start + guard.len()..];
        let (guard_body, after_branch) = matching_code_block_end(after_guard)
            .map_or((after_guard, ""), |body_end| {
                (&after_guard[..body_end], &after_guard[body_end + 1..])
            });
        if guard_body_contains_return(guard_body)
            && !has_assignment_to_any_identifier(
                after_branch,
                &[receiver, additional_stale_identifier],
            )
        {
            return true;
        }
        search_from = guard_start + guard.len();
    }
    false
}

fn set_len_growth_terms_stay_fresh(
    after_new_len_binding: &str,
    receiver: &str,
    new_len: &str,
    len_term: &str,
    additional: &str,
) -> bool {
    let mut identifiers = vec![receiver, new_len];
    if is_simple_identifier(len_term) {
        identifiers.push(len_term);
    }
    if let Some(identifier) = expression_base_identifier(additional) {
        identifiers.push(identifier);
    }
    !has_assignment_to_any_identifier(after_new_len_binding, &identifiers)
}

fn expression_base_identifier(expression: &str) -> Option<&str> {
    if is_simple_identifier(expression) {
        return Some(expression);
    }
    let base_end = expression.find('.')?;
    let base = &expression[..base_end];
    is_simple_identifier(base).then_some(base)
}

fn has_set_len_capacity_relation(
    before_call: &str,
    new_len: &str,
    capacity: &str,
    receiver: &str,
) -> bool {
    let len_lte_cap = format!("{new_len}<={capacity}");
    let cap_gte_len = format!("{capacity}>={new_len}");
    let len_gt_cap = format!("{new_len}>{capacity}");
    let cap_lt_len = format!("{capacity}<{new_len}");
    has_set_len_capacity_predicate_guard(before_call, &len_lte_cap, new_len, capacity, receiver)
        || has_set_len_capacity_predicate_guard(
            before_call,
            &cap_gte_len,
            new_len,
            capacity,
            receiver,
        )
        || has_set_len_capacity_early_return(before_call, &len_gt_cap, new_len, capacity, receiver)
        || has_set_len_capacity_early_return(before_call, &cap_lt_len, new_len, capacity, receiver)
}

fn has_set_len_capacity_predicate_guard(
    before_call: &str,
    predicate: &str,
    new_len: &str,
    capacity: &str,
    receiver: &str,
) -> bool {
    let stale_identifiers = set_len_capacity_stale_identifiers(new_len, capacity, receiver);
    [
        format!("assert!({predicate})"),
        format!("assert!({predicate},"),
        format!("debug_assert!({predicate})"),
        format!("debug_assert!({predicate},"),
    ]
    .iter()
    .any(|pattern| {
        has_fresh_guard_pattern_for_identifiers(before_call, pattern, &stale_identifiers)
    }) || has_open_positive_branch_guard_for_identifiers(before_call, predicate, &stale_identifiers)
}

fn has_set_len_capacity_early_return(
    before_call: &str,
    predicate: &str,
    new_len: &str,
    capacity: &str,
    receiver: &str,
) -> bool {
    let stale_identifiers = set_len_capacity_stale_identifiers(new_len, capacity, receiver);
    let guard = format!("if{predicate}{{");
    let mut search_from = 0;
    while let Some(offset) = before_call[search_from..].find(&guard) {
        let guard_start = search_from + offset;
        let after_guard = &before_call[guard_start + guard.len()..];
        let (guard_body, after_branch) = matching_code_block_end(after_guard)
            .map_or((after_guard, ""), |body_end| {
                (&after_guard[..body_end], &after_guard[body_end + 1..])
            });
        if guard_body_contains_return(guard_body)
            && !has_assignment_to_any_identifier(after_branch, &stale_identifiers)
        {
            return true;
        }
        search_from = guard_start + guard.len();
    }
    false
}

fn guard_body_contains_return(guard_body: &str) -> bool {
    let code = strip_block_comments_and_literals(guard_body);
    code.starts_with("return")
        || code.contains(";return")
        || code.contains("{return")
        || code.contains("}return")
        || code.contains("=>return")
}

fn set_len_capacity_stale_identifiers<'a>(
    new_len: &'a str,
    capacity: &'a str,
    receiver: &'a str,
) -> Vec<&'a str> {
    let mut identifiers = vec![new_len, receiver];
    if is_simple_identifier(capacity) {
        identifiers.push(capacity);
    }
    identifiers
}

fn len_plus_additional_argument<'a>(expression: &'a str, receiver: &str) -> Option<&'a str> {
    let len_expr = format!("{receiver}.len()");
    let after_len = expression.strip_prefix(&format!("{len_expr}+"));
    let before_len = expression.strip_suffix(&format!("+{len_expr}"));
    after_len
        .or(before_len)
        .filter(|additional| is_simple_identifier(additional))
}

fn has_fresh_set_len_reserve_call(
    after_len_binding: &str,
    receiver: &str,
    new_len: &str,
    additional: &str,
) -> bool {
    let reserve_patterns = [
        format!("{receiver}.reserve({additional});"),
        format!("{receiver}.try_reserve({additional})?;"),
    ];
    let identifiers = [receiver, new_len, additional];
    for reserve in reserve_patterns {
        let mut search_from = 0usize;
        while let Some(offset) = after_len_binding[search_from..].find(&reserve) {
            let reserve_start = search_from + offset;
            let before_reserve = &after_len_binding[..reserve_start];
            let after_reserve = &after_len_binding[reserve_start + reserve.len()..];
            if !has_assignment_to_any_identifier(before_reserve, &identifiers)
                && !has_assignment_to_any_identifier(after_reserve, &identifiers)
            {
                return true;
            }
            search_from = reserve_start + reserve.len();
        }
    }
    false
}

fn with_capacity_argument(right_side: &str) -> Option<&str> {
    let marker = "with_capacity(";
    let call_pos = right_side.find(marker)? + marker.len();
    let argument_text = &right_side[call_pos..];
    let argument_end = matching_call_argument_end(argument_text)?;
    let argument = &argument_text[..argument_end];
    (!argument.is_empty()).then_some(argument)
}
