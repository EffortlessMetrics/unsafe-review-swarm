use super::{
    code_before_operation, compact_code, has_u8_bool_value_guard, is_receiver_path_char,
    matching_call_argument_end, source_value_identifier, split_top_level_pair,
};
use crate::analysis::scanner::ScannedSite;

pub(super) fn has_maybeuninit_slice_context(lower: &str) -> bool {
    let compact = compact_code(lower);
    let Some(call_pos) = compact.find("from_raw_parts_mut(") else {
        return false;
    };
    let before_call = &compact[..call_pos];
    let after_marker = &compact[call_pos + "from_raw_parts_mut(".len()..];
    let argument_end = matching_call_argument_end(after_marker).unwrap_or(after_marker.len());
    let arguments = &after_marker[..argument_end];

    arguments.contains("maybeuninit") || maybeuninit_slice_return_type(before_call)
}

fn maybeuninit_slice_return_type(before_call: &str) -> bool {
    let Some(fn_pos) = before_call.rfind("fn") else {
        return false;
    };
    let fn_context = &before_call[fn_pos..];
    let signature = fn_context
        .split_once('{')
        .map_or(fn_context, |(signature, _body)| signature);

    signature
        .split_once("->")
        .is_some_and(|(_before, return_type)| {
            return_type.contains("maybeuninit") && return_type.contains('[')
        })
}

pub(super) fn has_maybeuninit_raw_write_context(site: &ScannedSite, lower: &str) -> bool {
    let compact_expression = compact_code(&site.operation.expression.to_ascii_lowercase());
    has_maybeuninit_write_bytes_target_context(site, lower, &compact_expression)
        || has_maybeuninit_ptr_write_value_context(&compact_expression)
}

fn has_maybeuninit_write_bytes_target_context(
    site: &ScannedSite,
    lower: &str,
    compact_expression: &str,
) -> bool {
    if !compact_expression.contains("write_bytes(") {
        return false;
    };
    let Some((_before_call, receiver, _byte, _len)) =
        write_bytes_method_context(compact_expression)
    else {
        return false;
    };
    if receiver.contains("maybeuninit") {
        return true;
    }
    let Some(before_operation) = code_before_operation(lower, &site.operation.expression) else {
        return false;
    };

    if receiver == "self" || receiver.starts_with("self.") {
        return maybeuninit_impl_receiver_before_write(&before_operation);
    }
    receiver
        .strip_suffix(".as_mut_ptr()")
        .is_some_and(|slice| maybeuninit_slice_parameter_before_write(&before_operation, slice))
}

fn maybeuninit_slice_parameter_before_write(before_write: &str, slice: &str) -> bool {
    let Some(fn_pos) = before_write.rfind("fn") else {
        return false;
    };
    let fn_context = &before_write[fn_pos..];
    let signature = fn_context
        .split_once('{')
        .map_or(fn_context, |(signature, _body)| signature);

    signature.contains("maybeuninit")
        && (signature.contains(&format!("{slice}:&mut["))
            || signature.contains(&format!("{slice}:&[")))
}

fn maybeuninit_impl_receiver_before_write(before_write: &str) -> bool {
    let Some(impl_pos) = before_write.rfind("impl") else {
        return false;
    };
    let impl_context = &before_write[impl_pos..];
    let header = impl_context
        .split_once('{')
        .map_or(impl_context, |(header, _body)| header);

    header.contains("for[") && header.contains("maybeuninit")
}

fn has_maybeuninit_ptr_write_value_context(compact_expression: &str) -> bool {
    compact_expression.contains("ptr::write(") && compact_expression.contains("maybeuninit::new(")
}

pub(super) fn has_u8_write_bytes_context(site: &ScannedSite, lower: &str) -> bool {
    let compact_expression = compact_code(&site.operation.expression.to_ascii_lowercase());
    let Some((_before_call, receiver, _byte, _len)) =
        write_bytes_method_context(&compact_expression)
    else {
        return false;
    };

    pointer_binding_has_type_before_operation(lower, &site.operation.expression, receiver, "*mutu8")
}

pub(super) fn has_bool_write_bytes_pointer_context(site: &ScannedSite, lower: &str) -> bool {
    let compact_expression = compact_code(&site.operation.expression.to_ascii_lowercase());
    let Some((_before_call, receiver, _byte, _len)) =
        write_bytes_method_context(&compact_expression)
    else {
        return false;
    };

    pointer_binding_has_type_before_operation(
        lower,
        &site.operation.expression,
        receiver,
        "*mutbool",
    )
}

pub(super) fn has_bool_write_bytes_value_evidence(site: &ScannedSite, lower: &str) -> bool {
    let compact_expression = compact_code(&site.operation.expression.to_ascii_lowercase());
    let Some((_before_call, receiver, byte, _len)) =
        write_bytes_method_context(&compact_expression)
    else {
        return false;
    };
    let Some(byte) = source_value_identifier(byte) else {
        return false;
    };
    let Some(before_operation) = code_before_operation(lower, &site.operation.expression) else {
        return false;
    };

    pointer_binding_has_type_before_operation(
        lower,
        &site.operation.expression,
        receiver,
        "*mutbool",
    ) && has_u8_bool_value_guard(&before_operation, byte)
}

pub(super) fn has_write_bytes_bounds_evidence(lower: &str) -> bool {
    let compact = compact_code(lower);
    let Some((_before_call, receiver, _byte, len)) = write_bytes_method_context(&compact) else {
        return false;
    };
    let Some(slice) = receiver.strip_suffix(".as_mut_ptr()") else {
        return false;
    };

    len == format!("{slice}.len()")
}

fn write_bytes_method_context(compact: &str) -> Option<(&str, &str, &str, &str)> {
    let call_marker = ".write_bytes(";
    let call_pos = compact.find(call_marker)?;
    let before_call = &compact[..call_pos];
    let receiver = receiver_expression_before_pos(compact, call_pos)?;
    let after_marker = &compact[call_pos + call_marker.len()..];
    let argument_end = matching_call_argument_end(after_marker)?;
    let arguments = &after_marker[..argument_end];
    let (byte, len) = split_top_level_pair(arguments)?;
    (!byte.is_empty() && !len.is_empty()).then_some((before_call, receiver, byte, len))
}

fn receiver_expression_before_pos(compact: &str, pos: usize) -> Option<&str> {
    let before_marker = compact.get(..pos)?;
    if let Some(receiver) = simple_receiver_from_before_marker(before_marker) {
        return Some(receiver);
    }
    call_receiver_from_before_marker(before_marker)
}

fn simple_receiver_from_before_marker(before_marker: &str) -> Option<&str> {
    let receiver_start = before_marker
        .char_indices()
        .rev()
        .find_map(|(idx, ch)| (!is_receiver_path_char(ch)).then_some(idx + ch.len_utf8()))
        .unwrap_or(0);
    let receiver = &before_marker[receiver_start..];
    (!receiver.is_empty()).then_some(receiver)
}

fn call_receiver_from_before_marker(before_marker: &str) -> Option<&str> {
    if !before_marker.ends_with(')') {
        return None;
    }
    let open = matching_open_for_trailing_call(before_marker)?;
    let before_open = &before_marker[..open];
    let receiver_start = before_open
        .char_indices()
        .rev()
        .find_map(|(idx, ch)| (!is_receiver_path_char(ch)).then_some(idx + ch.len_utf8()))
        .unwrap_or(0);
    let receiver = &before_marker[receiver_start..];
    (!receiver.is_empty()).then_some(receiver)
}

fn matching_open_for_trailing_call(text: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (idx, ch) in text.char_indices().rev() {
        match ch {
            ')' => depth += 1,
            '(' if depth == 1 => return Some(idx),
            '(' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    None
}

fn pointer_binding_has_type_before_operation(
    lower: &str,
    expression: &str,
    receiver: &str,
    pointer_type: &str,
) -> bool {
    let Some(before_operation) = code_before_operation(lower, expression) else {
        return false;
    };
    before_operation.contains(&format!("{receiver}:{pointer_type}"))
}
