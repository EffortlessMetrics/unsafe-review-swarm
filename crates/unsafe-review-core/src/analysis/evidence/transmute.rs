use super::{
    any_compact_if_condition, branch_still_open_at_operation, condition_has_top_level_conjunct,
    has_u8_bool_value_guard, matching_call_argument_end, matching_generic_argument_end,
    source_value_identifier, split_top_level_pair, strip_block_comments_and_literals,
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
            before_call: self.before_call,
            same_source_value_target: source_value_identifier(self.argument)?,
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
    before_call: &'a str,
    same_source_value_target: &'a str,
    domain: TransmuteValueDomain,
}

enum TransmuteValueDomain {
    U8ToBool,
}

impl TransmuteValueDomainContext<'_> {
    fn has_valid_value_evidence(&self) -> bool {
        match self.domain {
            TransmuteValueDomain::U8ToBool => {
                has_u8_bool_value_guard(self.before_call, self.same_source_value_target)
            }
        }
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
    any_compact_if_condition(compact, |condition, after_guard| {
        condition_has_top_level_conjunct(condition, predicate)
            && branch_still_open_at_operation(after_guard)
    })
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
