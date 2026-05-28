use super::{
    any_compact_if_condition, branch_still_open_at_operation, condition_has_top_level_conjunct,
    contains_executable_return, has_assignment_to_identifier, matching_call_argument_end,
    matching_code_block_end, matching_generic_argument_end, source_value_identifier,
    split_top_level_pair, strip_block_comments_and_literals, u8_bool_valid_value_predicates,
};

pub(super) fn has_transmute_layout_size_evidence(lower: &str) -> bool {
    let compact = compact_code(&strip_block_comments_and_literals(lower));
    let Some(context) = TransmuteCallContext::parse(&compact) else {
        return false;
    };
    context.layout_context().has_size_evidence()
}

pub(super) fn has_transmute_u8_bool_valid_value_evidence(lower: &str) -> bool {
    let compact = compact_code(&strip_block_comments_and_literals(lower));
    let Some(context) = TransmuteCallContext::parse(&compact) else {
        return false;
    };
    context
        .value_domain_context()
        .is_some_and(|value_domain| value_domain.has_valid_value_evidence())
}

struct TransmuteCallContext<'a> {
    before_call: &'a str,
    source_type: &'a str,
    destination_type: &'a str,
    argument: &'a str,
}

impl<'a> TransmuteCallContext<'a> {
    fn parse(compact: &'a str) -> Option<Self> {
        for marker in ["transmute::<", "transmute_copy::<"] {
            let Some(marker_start) = compact.find(marker) else {
                continue;
            };
            let before_call = &compact[..marker_start];
            let start = marker_start + marker.len();
            let after_marker = &compact[start..];
            let end = matching_generic_argument_end(after_marker)?;
            let arguments = &after_marker[..end];
            let after_arguments = after_marker.get(end + 1..)?;
            let after_open = after_arguments.strip_prefix('(')?;
            let argument_end = matching_call_argument_end(after_open)?;
            let argument = &after_open[..argument_end];
            if let Some((source_type, destination_type)) = split_top_level_pair(arguments) {
                return Some(Self {
                    before_call,
                    source_type,
                    destination_type,
                    argument,
                });
            }
        }
        None
    }

    fn layout_context(&self) -> TransmuteLayoutContext<'a> {
        TransmuteLayoutContext {
            before_call: self.before_call,
            source_type: self.source_type,
            destination_type: self.destination_type,
        }
    }

    fn value_domain_context(&self) -> Option<TransmuteValueDomainContext<'a>> {
        let domain = self.value_domain()?;
        Some(TransmuteValueDomainContext {
            applicability: TransmuteValueDomainApplicability::new(
                self.before_call,
                source_value_identifier(self.argument)?,
            ),
            domain,
        })
    }

    fn value_domain(&self) -> Option<TransmuteValueDomain> {
        (self.source_type == "u8" && self.destination_type == "bool")
            .then_some(TransmuteValueDomain::U8ToBool)
    }
}

struct TransmuteLayoutContext<'a> {
    before_call: &'a str,
    source_type: &'a str,
    destination_type: &'a str,
}

impl TransmuteLayoutContext<'_> {
    fn has_size_evidence(&self) -> bool {
        let normalized = normalize_size_of_paths(self.before_call);
        has_size_of_equality(&normalized, self.source_type, self.destination_type)
    }
}

struct TransmuteValueDomainContext<'a> {
    applicability: TransmuteValueDomainApplicability<'a>,
    domain: TransmuteValueDomain,
}

enum TransmuteValueDomain {
    U8ToBool,
}

impl TransmuteValueDomainContext<'_> {
    fn has_valid_value_evidence(&self) -> bool {
        match self.domain {
            TransmuteValueDomain::U8ToBool => self.has_u8_to_bool_valid_value_evidence(),
        }
    }

    fn has_u8_to_bool_valid_value_evidence(&self) -> bool {
        self.u8_to_bool_valid_predicates()
            .iter()
            .any(|predicate| self.has_u8_bool_value_predicate_guard(predicate))
            || self.has_u8_bool_invalid_early_return_guard()
    }

    fn u8_to_bool_valid_predicates(&self) -> [String; 8] {
        self.applicability.u8_to_bool_valid_predicates()
    }

    fn has_u8_bool_value_predicate_guard(&self, predicate: &str) -> bool {
        [
            format!("assert!({predicate})"),
            format!("assert!({predicate},"),
            format!("debug_assert!({predicate})"),
            format!("debug_assert!({predicate},"),
        ]
        .iter()
        .any(|pattern| self.applicability.has_fresh_assertion_guard(pattern))
            || self.has_open_positive_branch_guard(predicate)
    }

    fn has_open_positive_branch_guard(&self, predicate: &str) -> bool {
        self.applicability.has_open_positive_branch_guard(predicate)
    }

    fn has_u8_bool_invalid_early_return_guard(&self) -> bool {
        self.has_invalid_byte_returning_branch(&format!(
            "{}>1",
            self.applicability.same_source_value_target
        )) || self.has_invalid_byte_returning_branch(&format!(
            "1<{}",
            self.applicability.same_source_value_target
        )) || self.has_invalid_byte_returning_branch(&format!(
            "{}>=2",
            self.applicability.same_source_value_target
        )) || self.has_invalid_byte_returning_branch(&format!(
            "2<={}",
            self.applicability.same_source_value_target
        ))
    }

    fn has_invalid_byte_returning_branch(&self, predicate: &str) -> bool {
        let guard = format!("if{predicate}{{");
        self.applicability.has_returning_branch_guard(&guard)
    }
}

// Valid-value evidence for transmute applies only to the same source value that
// is converted, and that value must remain fresh until the unsafe operation.
struct TransmuteValueDomainApplicability<'a> {
    before_call: &'a str,
    same_source_value_target: &'a str,
}

impl<'a> TransmuteValueDomainApplicability<'a> {
    fn new(before_call: &'a str, same_source_value_target: &'a str) -> Self {
        Self {
            before_call,
            same_source_value_target,
        }
    }

    fn u8_to_bool_valid_predicates(&self) -> [String; 8] {
        u8_bool_valid_value_predicates(self.same_source_value_target)
    }

    fn has_fresh_assertion_guard(&self, pattern: &str) -> bool {
        let mut search_from = 0;
        while let Some(offset) = self.before_call[search_from..].find(pattern) {
            let pattern_start = search_from + offset;
            let after_pattern = &self.before_call[pattern_start + pattern.len()..];
            let statement_end = after_pattern.find(';').unwrap_or(after_pattern.len());
            let after_guard = &after_pattern[statement_end..];
            if self.source_value_stays_fresh_after(after_guard) {
                return true;
            }
            search_from = pattern_start + pattern.len();
        }
        false
    }

    fn has_open_positive_branch_guard(&self, predicate: &str) -> bool {
        any_compact_if_condition(self.before_call, |condition, after_guard| {
            condition_has_top_level_conjunct(condition, predicate)
                && self.open_branch_preserves_applicability(after_guard)
        })
    }

    fn open_branch_preserves_applicability(&self, after_guard: &str) -> bool {
        branch_still_open_at_operation(after_guard)
            && self.source_value_stays_fresh_after(after_guard)
    }

    fn has_returning_branch_guard(&self, guard: &str) -> bool {
        let mut search_from = 0;
        while let Some(offset) = self.before_call[search_from..].find(guard) {
            let guard_start = search_from + offset;
            let after_guard = &self.before_call[guard_start + guard.len()..];
            let (guard_body, after_branch) = matching_code_block_end(after_guard)
                .map_or((after_guard, ""), |body_end| {
                    (&after_guard[..body_end], &after_guard[body_end + 1..])
                });
            if contains_executable_return(guard_body)
                && self.source_value_stays_fresh_after(after_branch)
            {
                return true;
            }
            search_from = guard_start + guard.len();
        }
        false
    }

    fn source_value_stays_fresh_after(&self, evidence: &str) -> bool {
        !has_assignment_to_identifier(evidence, self.same_source_value_target)
    }
}

fn normalize_size_of_paths(compact: &str) -> String {
    compact
        .replace("core::mem::size_of", "size_of")
        .replace("std::mem::size_of", "size_of")
        .replace("mem::size_of", "size_of")
}

fn has_size_of_equality(compact: &str, left_type: &str, right_type: &str) -> bool {
    let left = format!("size_of::<{left_type}>()");
    let right = format!("size_of::<{right_type}>()");
    let left_eq_right = format!("{left}=={right}");
    let right_eq_left = format!("{right}=={left}");
    has_size_assert_eq(compact, &left, &right)
        || has_size_assert_eq(compact, &right, &left)
        || has_size_assert(compact, &left_eq_right)
        || has_size_assert(compact, &right_eq_left)
        || has_open_size_branch(compact, &left_eq_right)
        || has_open_size_branch(compact, &right_eq_left)
}

fn has_size_assert_eq(compact: &str, left: &str, right: &str) -> bool {
    has_applicable_size_pattern(compact, &format!("assert_eq!({left},{right}"))
        || has_applicable_size_pattern(compact, &format!("debug_assert_eq!({left},{right}"))
}

fn has_size_assert(compact: &str, predicate: &str) -> bool {
    [
        format!("assert!({predicate})"),
        format!("assert!({predicate},"),
        format!("debug_assert!({predicate})"),
        format!("debug_assert!({predicate},"),
    ]
    .iter()
    .any(|pattern| has_applicable_size_pattern(compact, pattern))
}

fn has_open_size_branch(compact: &str, predicate: &str) -> bool {
    let guard = format!("if{predicate}{{");
    let mut search_from = 0usize;
    while let Some(offset) = compact[search_from..].find(&guard) {
        let guard_start = search_from + offset;
        let after_guard = &compact[guard_start + guard.len()..];
        if branch_still_open_at_operation(after_guard) {
            return true;
        }
        search_from = guard_start + guard.len();
    }
    false
}

fn has_applicable_size_pattern(compact: &str, pattern: &str) -> bool {
    let mut search_from = 0usize;
    while let Some(offset) = compact[search_from..].find(pattern) {
        let pattern_start = search_from + offset;
        if evidence_scope_reaches_operation(compact, pattern_start) {
            return true;
        }
        search_from = pattern_start + pattern.len();
    }
    false
}

fn evidence_scope_reaches_operation(compact: &str, evidence_start: usize) -> bool {
    let depth_at_evidence = brace_depth(&compact[..evidence_start]);
    if depth_at_evidence == 0 {
        return true;
    }

    let mut depth = depth_at_evidence;
    for ch in compact[evidence_start..].chars() {
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

fn brace_depth(text: &str) -> usize {
    let mut depth = 0usize;
    for ch in text.chars() {
        match ch {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    depth
}

fn compact_code(lower: &str) -> String {
    lower
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace())
        .collect()
}
