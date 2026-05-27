use super::{
    compact_code, has_assignment_to_identifier, has_fresh_guard_pattern, is_receiver_path_char,
    matching_call_argument_end, matching_code_block_end, source_value_identifier,
    strip_block_comments_and_literals,
};

pub(super) fn has_from_utf8_unchecked_validation_evidence(lower: &str) -> bool {
    let compact = compact_code(lower);
    let Some((before_call, argument)) = from_utf8_unchecked_argument_context(&compact) else {
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

    has_validation_is_ok_branch_guard(&context)
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
    fn has_argument_assignment(&self, text: &str) -> bool {
        has_assignment_to_identifier(text, self.argument_identifier)
    }
}

fn from_utf8_unchecked_argument_context(compact: &str) -> Option<(&str, &str)> {
    let marker = "from_utf8_unchecked(";
    let call_pos = compact.find(marker)?;
    let before_call = &compact[..call_pos];
    let after_marker = &compact[call_pos + marker.len()..];
    let argument_end = matching_call_argument_end(after_marker)?;
    let argument = &after_marker[..argument_end];
    (!argument.is_empty()).then_some((before_call, argument))
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
        if depth > 0 && !context.has_argument_assignment(after_guard) {
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
        if depth > 0 && !context.has_argument_assignment(after_open) {
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
        if guard_body_contains_return(else_body)
            && !context.has_argument_assignment(after_else_body)
        {
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
        if guard_body_contains_return(guard_body)
            && !context.has_argument_assignment(after_guard_body)
        {
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
        if current_arm.contains("=>") && !context.has_argument_assignment(current_arm) {
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
        if guard_body_contains_return(guard_body) && !context.has_argument_assignment(after_branch)
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
            && err_arm.contains("=>return")
            && !context.has_argument_assignment(after_block)
        {
            return true;
        }

        search_from = validation_pos + context.validation.len();
    }

    false
}
