use super::{
    compact_code, contains_simple_assignment_to, let_binding_name, matching_call_argument_end,
};

pub(super) fn has_drop_in_place_box_origin_evidence(expression: &str, lower: &str) -> bool {
    let compact = compact_code(lower);
    let compact_expression = compact_code(&expression.to_ascii_lowercase());
    let Some(pointer) = drop_in_place_argument(&compact_expression) else {
        return false;
    };
    let call_pos = compact
        .find(&compact_expression)
        .or_else(|| compact.find(&format!("drop_in_place({pointer})")));
    let Some(call_pos) = call_pos else {
        return false;
    };
    has_same_pointer_box_into_raw_before(&compact[..call_pos], pointer)
}

fn drop_in_place_argument(compact_expression: &str) -> Option<&str> {
    let marker = "drop_in_place(";
    let call_pos = compact_expression.find(marker)? + marker.len();
    let argument_text = &compact_expression[call_pos..];
    let argument_end = matching_call_argument_end(argument_text)?;
    let argument = &argument_text[..argument_end];
    (!argument.is_empty()).then_some(argument)
}

fn box_into_raw_argument(right_side: &str) -> Option<&str> {
    let marker = "box::into_raw(";
    let call_pos = right_side.find(marker)? + marker.len();
    let argument_text = &right_side[call_pos..];
    let argument_end = matching_call_argument_end(argument_text)?;
    let argument = &argument_text[..argument_end];
    (!argument.is_empty()).then_some(argument)
}

pub(super) fn has_box_from_raw_origin_evidence(expression: &str, lower: &str) -> bool {
    let compact = compact_code(lower);
    let compact_expression = compact_code(&expression.to_ascii_lowercase());
    let Some(pointer) = box_from_raw_argument(&compact_expression) else {
        return false;
    };
    let call_pos = compact
        .find(&compact_expression)
        .or_else(|| compact.find(&format!("box::from_raw({pointer})")));
    let Some(call_pos) = call_pos else {
        return false;
    };
    has_same_pointer_box_into_raw_before(&compact[..call_pos], pointer)
}

fn has_same_pointer_box_into_raw_before(before_call: &str, pointer: &str) -> bool {
    let mut offset = 0usize;
    for statement in before_call.split(';') {
        let Some((left, right)) = statement.split_once('=') else {
            offset += statement.len() + 1;
            continue;
        };
        let Some(binding) = let_binding_name(left) else {
            offset += statement.len() + 1;
            continue;
        };
        if binding == pointer && box_into_raw_argument(right).is_some() {
            let after_origin = &before_call[(offset + statement.len()).min(before_call.len())..];
            return !contains_simple_assignment_to(after_origin, pointer);
        }
        offset += statement.len() + 1;
    }
    false
}

fn box_from_raw_argument(compact_expression: &str) -> Option<&str> {
    let marker = "box::from_raw(";
    let call_pos = compact_expression.find(marker)? + marker.len();
    let argument_text = &compact_expression[call_pos..];
    let argument_end = matching_call_argument_end(argument_text)?;
    let argument = &argument_text[..argument_end];
    (!argument.is_empty()).then_some(argument)
}
