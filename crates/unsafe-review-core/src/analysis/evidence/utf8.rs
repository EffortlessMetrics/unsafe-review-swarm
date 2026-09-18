use super::{
    compact_code, contains_executable_return, has_assignment_to_identifier,
    has_fresh_guard_pattern, is_receiver_path_char, is_runtime_assert_at,
    matching_call_argument_end, matching_code_block_end, source_value_identifier,
    strip_block_comments_and_literals,
};

pub(super) fn has_from_utf8_unchecked_validation_evidence(
    lower: &str,
    snippet_offset: usize,
) -> bool {
    let compact = compact_code(&strip_block_comments_and_literals(lower));
    let Some((before_call, argument)) =
        from_utf8_unchecked_argument_context(&compact, snippet_offset)
    else {
        return false;
    };
    let Some(argument_identifier) = source_value_identifier(argument) else {
        return false;
    };
    let context = Utf8ValidationContext {
        before_call,
        validation: format!("from_utf8({argument})"),
        argument_identifier,
    };

    has_validation_assert_guard(&context)
        || has_validation_is_ok_branch_guard(&context)
        || has_validation_if_let_ok_branch_guard(&context)
        || has_validation_let_else_ok_guard(&context)
        || has_validation_match_ok_branch_guard(&context)
        || has_validation_if_let_err_return_guard(&context)
        || has_validation_early_return_guard(&context, "is_err")
        || has_validation_question_mark_guard(&context)
        || has_validation_match_return_guard(&context)
}

// UTF-8 validation evidence must target the same source buffer and must stay
// fresh until the unchecked conversion.
struct Utf8ValidationContext<'a> {
    before_call: &'a str,
    validation: String,
    argument_identifier: &'a str,
}

impl Utf8ValidationContext<'_> {
    fn has_stale_argument(&self, text: &str) -> bool {
        self.has_argument_assignment(text) || self.has_argument_mutation(text)
    }

    fn has_argument_assignment(&self, text: &str) -> bool {
        has_assignment_to_identifier(text, self.argument_identifier)
    }

    fn has_argument_mutation(&self, text: &str) -> bool {
        if self.argument_identifier.is_empty() {
            return false;
        }

        let mutating_methods = [
            "append",
            "clear",
            "dedup",
            "dedup_by",
            "dedup_by_key",
            "drain",
            "extend",
            "extend_from_slice",
            "insert",
            "pop",
            "push",
            "remove",
            "resize",
            "resize_with",
            "retain",
            "splice",
            "swap_remove",
            "truncate",
        ];
        mutating_methods
            .iter()
            .any(|method| text.contains(&format!("{}.{method}(", self.argument_identifier)))
    }
}

fn from_utf8_unchecked_argument_context(
    compact: &str,
    snippet_offset: usize,
) -> Option<(&str, &str)> {
    let marker = "from_utf8_unchecked(";
    // Anchor on the site's own call: the first marker at or after the
    // snippet offset. Earlier markers belong to other sites sharing the
    // context window and must not donate their argument or guards.
    // Without a marker there, keep the legacy first-marker anchor.
    let mut search_from = 0usize;
    let mut call_pos = None;
    while let Some(offset) = compact[search_from..].find(marker) {
        let abs_pos = search_from + offset;
        if call_pos.is_none() {
            call_pos = Some(abs_pos);
        }
        if abs_pos >= snippet_offset {
            call_pos = Some(abs_pos);
            break;
        }
        search_from = abs_pos + marker.len();
    }
    let call_pos = call_pos?;
    let before_call = &compact[..call_pos];
    let after_marker = &compact[call_pos + marker.len()..];
    let argument_end = matching_call_argument_end(after_marker)?;
    let argument = &after_marker[..argument_end];
    (!argument.is_empty()).then_some((before_call, argument))
}

/// An `assert!(from_utf8(buffer).is_ok())` before the unchecked conversion
/// panics on invalid input, so the same-buffer conversion after it is
/// guarded. Only plain `assert!` counts: `debug_assert!` is compiled out in
/// release builds (`is_runtime_assert_at`), and an aliased boolean (e.g.
/// `let valid = ...; assert!(valid);`) stays uncredited without dataflow.
fn has_validation_assert_guard(context: &Utf8ValidationContext<'_>) -> bool {
    let predicate = format!("{}.is_ok()", context.validation);
    let mut cursor = context.before_call;
    let mut offset = 0usize;
    while let Some(pos) = cursor.find("assert!(") {
        let abs_pos = offset + pos;
        if is_runtime_assert_at(context.before_call, abs_pos) {
            let after_prefix = &context.before_call[abs_pos + "assert!(".len()..];
            let statement_end = after_prefix.find(';').unwrap_or(after_prefix.len());
            let statement = &after_prefix[..statement_end];
            let after_statement = &after_prefix[statement_end..];
            if statement_contains_validation_predicate(statement, &predicate)
                && !context.has_stale_argument(after_statement)
            {
                return true;
            }
        }
        let next = pos + "assert!(".len();
        offset += next;
        cursor = &cursor[next..];
    }
    false
}

fn statement_contains_validation_predicate(statement: &str, predicate: &str) -> bool {
    let mut search_from = 0usize;
    while let Some(pos) = statement[search_from..].find(predicate) {
        let abs_pos = search_from + pos;
        let before_ok = abs_pos == 0
            || statement[..abs_pos]
                .chars()
                .next_back()
                .is_none_or(|ch| !ch.is_ascii_alphanumeric() && ch != '_');
        let after = &statement[abs_pos + predicate.len()..];
        if before_ok && (after.starts_with(')') || after.starts_with(',')) {
            return true;
        }
        search_from = abs_pos + predicate.len();
    }
    false
}

fn has_validation_is_ok_branch_guard(context: &Utf8ValidationContext<'_>) -> bool {
    let before_call = context.before_call;
    let guard = format!("{}.is_ok(){{", context.validation);
    let mut search_from = 0;
    while let Some(offset) = before_call[search_from..].find(&guard) {
        let guard_start = search_from + offset;
        let after_guard = &before_call[guard_start + guard.len()..];
        let mut depth = 1usize;
        for ch in after_guard.chars() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
        }
        if depth > 0 && !context.has_stale_argument(after_guard) {
            return true;
        }
        search_from = guard_start + guard.len();
    }
    false
}

fn has_validation_if_let_ok_branch_guard(context: &Utf8ValidationContext<'_>) -> bool {
    let before_call = context.before_call;
    let mut search_from = 0;
    while let Some(offset) = before_call[search_from..].find(&context.validation) {
        let validation_start = search_from + offset;
        let before_validation = &before_call[..validation_start];
        let Some(if_let_start) = before_validation.rfind("ifletok(") else {
            search_from = validation_start + context.validation.len();
            continue;
        };
        let pattern = &before_validation[if_let_start + "ifletok(".len()..];
        let Some(pattern_end) = pattern.find(")=") else {
            search_from = validation_start + context.validation.len();
            continue;
        };
        let binding = &pattern[..pattern_end];
        let path_prefix = &pattern[pattern_end + ")=".len()..];
        if !ok_err_pattern_is_plain_binding(binding, path_prefix) {
            search_from = validation_start + context.validation.len();
            continue;
        }
        let after_validation = &before_call[validation_start + context.validation.len()..];
        let Some(after_open) = after_validation.strip_prefix('{') else {
            search_from = validation_start + context.validation.len();
            continue;
        };
        let mut depth = 1usize;
        for ch in after_open.chars() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
        }
        if depth > 0 && !context.has_stale_argument(after_open) {
            return true;
        }
        search_from = validation_start + context.validation.len();
    }
    false
}

fn has_validation_let_else_ok_guard(context: &Utf8ValidationContext<'_>) -> bool {
    let before_call = context.before_call;
    let mut search_from = 0usize;
    while let Some(offset) = before_call[search_from..].find(&context.validation) {
        let validation_start = search_from + offset;
        let before_validation = &before_call[..validation_start];
        let Some(let_start) = before_validation.rfind("letok(") else {
            search_from = validation_start + context.validation.len();
            continue;
        };
        let pattern = &before_validation[let_start + "letok(".len()..];
        let Some(pattern_end) = pattern.find(")=") else {
            search_from = validation_start + context.validation.len();
            continue;
        };
        let binding = &pattern[..pattern_end];
        let path_prefix = &pattern[pattern_end + ")=".len()..];
        if !ok_err_pattern_is_plain_binding(binding, path_prefix) {
            search_from = validation_start + context.validation.len();
            continue;
        }
        let after_validation = &before_call[validation_start + context.validation.len()..];
        let Some(after_else) = after_validation.strip_prefix("else{") else {
            search_from = validation_start + context.validation.len();
            continue;
        };
        let (else_body, after_else_body) = matching_code_block_end(after_else)
            .map_or((after_else, ""), |else_end| {
                (&after_else[..else_end], &after_else[else_end + 1..])
            });
        if contains_executable_return(else_body) && !context.has_stale_argument(after_else_body) {
            return true;
        }
        search_from = validation_start + context.validation.len();
    }
    false
}

fn has_validation_if_let_err_return_guard(context: &Utf8ValidationContext<'_>) -> bool {
    let before_call = context.before_call;
    let mut search_from = 0usize;
    while let Some(offset) = before_call[search_from..].find(&context.validation) {
        let validation_start = search_from + offset;
        let before_validation = &before_call[..validation_start];
        let Some(if_let_start) = before_validation.rfind("ifleterr(") else {
            search_from = validation_start + context.validation.len();
            continue;
        };
        let pattern = &before_validation[if_let_start + "ifleterr(".len()..];
        let Some(pattern_end) = pattern.find(")=") else {
            search_from = validation_start + context.validation.len();
            continue;
        };
        let binding = &pattern[..pattern_end];
        let path_prefix = &pattern[pattern_end + ")=".len()..];
        if !ok_err_pattern_is_plain_binding(binding, path_prefix) {
            search_from = validation_start + context.validation.len();
            continue;
        }
        let after_validation = &before_call[validation_start + context.validation.len()..];
        let Some(after_open) = after_validation.strip_prefix('{') else {
            search_from = validation_start + context.validation.len();
            continue;
        };
        let (guard_body, after_guard_body) = matching_code_block_end(after_open)
            .map_or((after_open, ""), |body_end| {
                (&after_open[..body_end], &after_open[body_end + 1..])
            });
        if contains_executable_return(guard_body) && !context.has_stale_argument(after_guard_body) {
            return true;
        }
        search_from = validation_start + context.validation.len();
    }
    false
}

fn ok_err_pattern_is_plain_binding(binding: &str, path_prefix: &str) -> bool {
    !binding.is_empty()
        && !binding.contains('{')
        && (path_prefix.is_empty() || path_prefix.ends_with("::"))
        && path_prefix
            .chars()
            .all(|ch| is_receiver_path_char(ch) || ch == ':')
}

fn has_validation_match_ok_branch_guard(context: &Utf8ValidationContext<'_>) -> bool {
    let before_call = context.before_call;
    let mut search_from = 0usize;
    while let Some(relative_validation_pos) = before_call[search_from..].find(&context.validation) {
        let validation_pos = search_from + relative_validation_pos;
        let prefix = &before_call[..validation_pos];
        let Some(match_pos) = prefix.rfind("match") else {
            search_from = validation_pos + context.validation.len();
            continue;
        };
        let after_match = &prefix[match_pos + "match".len()..];
        if !(after_match.is_empty() || after_match.ends_with("::")) {
            search_from = validation_pos + context.validation.len();
            continue;
        }

        let after_validation = &before_call[validation_pos + context.validation.len()..];
        let Some(after_open) = after_validation.strip_prefix('{') else {
            search_from = validation_pos + context.validation.len();
            continue;
        };
        if matching_code_block_end(after_open).is_some() {
            search_from = validation_pos + context.validation.len();
            continue;
        }

        let Some(ok_pos) = after_open.rfind("ok(") else {
            search_from = validation_pos + context.validation.len();
            continue;
        };
        if after_open
            .rfind("err(")
            .is_some_and(|err_pos| err_pos > ok_pos)
        {
            search_from = validation_pos + context.validation.len();
            continue;
        }
        let current_arm = &after_open[ok_pos..];
        if current_arm.contains("=>") && !context.has_stale_argument(current_arm) {
            return true;
        }

        search_from = validation_pos + context.validation.len();
    }

    false
}

fn has_validation_early_return_guard(context: &Utf8ValidationContext<'_>, predicate: &str) -> bool {
    let before_call = context.before_call;
    let guard = format!("{}.{predicate}(){{", context.validation);
    let mut search_from = 0;
    while let Some(offset) = before_call[search_from..].find(&guard) {
        let guard_start = search_from + offset;
        let after_guard = &before_call[guard_start + guard.len()..];
        let (guard_body, after_branch) = matching_code_block_end(after_guard)
            .map_or((after_guard, ""), |body_end| {
                (&after_guard[..body_end], &after_guard[body_end + 1..])
            });
        if contains_executable_return(guard_body) && !context.has_stale_argument(after_branch) {
            return true;
        }
        search_from = guard_start + guard.len();
    }
    false
}

fn has_validation_question_mark_guard(context: &Utf8ValidationContext<'_>) -> bool {
    has_fresh_guard_pattern(
        context.before_call,
        &format!("{}?;", context.validation),
        context.argument_identifier,
    )
}

fn has_validation_match_return_guard(context: &Utf8ValidationContext<'_>) -> bool {
    let before_call = context.before_call;
    let mut search_from = 0usize;
    while let Some(relative_validation_pos) = before_call[search_from..].find(&context.validation) {
        let validation_pos = search_from + relative_validation_pos;
        let prefix = &before_call[..validation_pos];
        let Some(match_pos) = prefix.rfind("match") else {
            search_from = validation_pos + context.validation.len();
            continue;
        };
        let after_match = &prefix[match_pos + "match".len()..];
        if !(after_match.is_empty() || after_match.ends_with("::")) {
            search_from = validation_pos + context.validation.len();
            continue;
        }

        let after_validation = &before_call[validation_pos + context.validation.len()..];
        let Some(after_open) = after_validation.strip_prefix('{') else {
            search_from = validation_pos + context.validation.len();
            continue;
        };
        let Some(body_end) = matching_code_block_end(after_open) else {
            return false;
        };
        let body = &after_open[..body_end];
        let after_block = after_open.get(body_end + 1..).unwrap_or("");
        let Some(err_arm) = body.find("err(").map(|err_pos| &body[err_pos..]) else {
            search_from = validation_pos + context.validation.len();
            continue;
        };
        if body.contains("ok(")
            && err_arm_contains_executable_return(err_arm)
            && !context.has_stale_argument(after_block)
        {
            return true;
        }

        search_from = validation_pos + context.validation.len();
    }

    false
}

fn err_arm_contains_executable_return(err_arm: &str) -> bool {
    let Some(arrow_pos) = err_arm.find("=>") else {
        return false;
    };
    let after_arrow = &err_arm[arrow_pos + "=>".len()..];
    if let Some(after_open) = after_arrow.strip_prefix('{') {
        let guard_body = matching_code_block_end(after_open)
            .map_or(after_open, |body_end| &after_open[..body_end]);
        return contains_executable_return(guard_body);
    }

    let guard_body = after_arrow
        .find(',')
        .map_or(after_arrow, |comma_pos| &after_arrow[..comma_pos]);
    contains_executable_return(guard_body)
}
