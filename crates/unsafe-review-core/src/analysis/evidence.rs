mod get_unchecked;
mod maybeuninit;
mod nonnull;
mod set_len;
mod transmute;

use self::get_unchecked::{get_unchecked_receiver_and_index, has_get_unchecked_bounds_guard};
use self::maybeuninit::has_maybeuninit_assume_init_initialization_evidence;
use self::nonnull::has_nullability_guard;
use self::transmute::{
    has_transmute_layout_size_evidence, has_transmute_u8_bool_valid_value_evidence,
};
use super::set_len_shrink;
use crate::analysis::scanner::ScannedSite;
use crate::domain::{
    ContractEvidence, DischargeEvidence, EvidenceState, ObligationEvidence, OperationFamily,
    ReachEvidence, RelatedTest, SafetyObligation, UnsafeSiteKind,
};
use std::fs;
use std::path::{Path, PathBuf};

const PUBLIC_UNSAFE_API_CONTRACT_DISCHARGE: &str = "Public unsafe API declaration is a caller-contract site; local guard evidence is not expected at the declaration";
const DOCUMENTED_PRIVATE_UNSAFE_CONTRACT_DISCHARGE: &str = "Documented private unsafe declaration is a caller-contract site; local guard evidence is not expected at the declaration";
const TARGET_FEATURE_CONTRACT_DISCHARGE: &str = "Documented target-feature declaration is a caller-contract site; local guard evidence is not expected at the attribute";

pub(crate) fn contract_evidence(site: &ScannedSite) -> ContractEvidence {
    let context = site.context_before.join("\n");
    if let Some(summary) = safety_doc_summary(&context) {
        return ContractEvidence::present(summary);
    }
    if site.site.public_api_surface {
        return ContractEvidence::missing_with(
            "Public unsafe API is missing nearby `# Safety` documentation",
        );
    }
    if let Some(summary) = safety_comment_summary(&context, &site.site.snippet) {
        return ContractEvidence::present(summary);
    }
    ContractEvidence::missing()
}

fn safety_doc_summary(context: &str) -> Option<&'static str> {
    for line in context.lines() {
        let trimmed = line.trim_start();
        if !(trimmed.starts_with("///")
            || trimmed.starts_with("//!")
            || trimmed.starts_with("#[doc"))
        {
            continue;
        }
        if trimmed.contains("# Safety") {
            return Some("Nearby `# Safety` documentation was detected");
        }
        if trimmed.contains("Safety:") {
            return Some("Nearby `Safety:` documentation was detected");
        }
    }
    None
}

fn safety_comment_summary(context: &str, snippet: &str) -> Option<&'static str> {
    for line in context.lines().chain(snippet.lines()) {
        let trimmed = line.trim_start();
        if trimmed.starts_with("///") || trimmed.starts_with("//!") {
            continue;
        }
        if !(trimmed.starts_with("//")
            || trimmed.contains("// SAFETY:")
            || trimmed.contains("// Safety:"))
        {
            continue;
        }
        if trimmed.contains("SAFETY:") {
            return Some("Nearby `SAFETY:` comment was detected");
        }
        if trimmed.contains("Safety:") {
            return Some("Nearby `Safety:` comment was detected");
        }
    }
    None
}

pub(crate) fn obligation_evidence(
    site: &ScannedSite,
    obligations: &[SafetyObligation],
    contract: &ContractEvidence,
    reach: &ReachEvidence,
) -> Vec<ObligationEvidence> {
    let text = code_context(site);
    let lower = text.to_ascii_lowercase();
    obligations
        .iter()
        .map(|obligation| ObligationEvidence {
            obligation: obligation.clone(),
            contract: contract_state(contract),
            discharge: discharge_state_for(site, &obligation.key, &lower, contract),
            reach: reach_state(reach),
            witness: EvidenceState::missing("No imported witness receipt was found"),
        })
        .collect()
}

pub(crate) fn summarize_discharge(evidence: &[ObligationEvidence]) -> DischargeEvidence {
    if evidence.is_empty() {
        return DischargeEvidence::missing();
    }
    if evidence
        .iter()
        .all(|obligation| obligation.discharge.present)
    {
        if evidence.iter().all(|obligation| {
            obligation.discharge.summary == PUBLIC_UNSAFE_API_CONTRACT_DISCHARGE
                || obligation.discharge.summary == DOCUMENTED_PRIVATE_UNSAFE_CONTRACT_DISCHARGE
        }) {
            return DischargeEvidence::present(&evidence[0].discharge.summary);
        }
        return DischargeEvidence::present(
            "All inferred safety obligations have visible local discharge evidence",
        );
    }
    if evidence
        .iter()
        .any(|obligation| obligation.discharge.present)
    {
        return DischargeEvidence::missing_with(
            "Some inferred safety obligations are missing local guard evidence",
        );
    }
    DischargeEvidence::missing()
}

fn code_context(site: &ScannedSite) -> String {
    site.context_before
        .iter()
        .chain(std::iter::once(&site.site.snippet))
        .chain(site.context_after.iter())
        .map(|line| {
            line.split_once("//")
                .map_or(line.as_str(), |(code, _comment)| code)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn code_context_through_site(site: &ScannedSite) -> String {
    site.context_before
        .iter()
        .chain(std::iter::once(&site.site.snippet))
        .map(|line| {
            line.split_once("//")
                .map_or(line.as_str(), |(code, _comment)| code)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn contract_state(contract: &ContractEvidence) -> EvidenceState {
    if contract.present {
        EvidenceState::present(&contract.summary)
    } else {
        EvidenceState::missing(&contract.summary)
    }
}

fn reach_state(reach: &ReachEvidence) -> EvidenceState {
    if reach.state == "unreached" || reach.state == "unknown" {
        EvidenceState::missing(&reach.summary)
    } else {
        EvidenceState::present(&reach.summary)
    }
}

fn discharge_state_for(
    site: &ScannedSite,
    key: &str,
    lower: &str,
    contract: &ContractEvidence,
) -> EvidenceState {
    let family = &site.operation.family;
    if is_public_unsafe_contract_obligation(site, key) {
        return EvidenceState::present(PUBLIC_UNSAFE_API_CONTRACT_DISCHARGE);
    }
    if is_documented_private_unsafe_contract_obligation(site, key, contract) {
        return EvidenceState::present(DOCUMENTED_PRIVATE_UNSAFE_CONTRACT_DISCHARGE);
    }
    match key {
        "alignment" => {
            if family == &OperationFamily::RawPointerWrite
                && has_u8_write_bytes_context(site, lower)
            {
                EvidenceState::present("u8 raw write alignment evidence was detected")
            } else if family == &OperationFamily::RawPointerWrite
                && has_bool_write_bytes_pointer_context(site, lower)
            {
                EvidenceState::present("bool raw write alignment evidence was detected")
            } else if has_alignment_guard(site, lower) {
                EvidenceState::present("Alignment guard code was detected")
            } else {
                EvidenceState::missing("No alignment guard code was detected")
            }
        }
        "bounds" | "valid-range" => {
            if has_bounds_guard(site, lower)
                || (family == &OperationFamily::PointerArithmetic
                    && has_slice_end_pointer_arithmetic_evidence(lower))
            {
                EvidenceState::present("Length or bounds guard code was detected")
            } else {
                EvidenceState::missing("No length or bounds guard code was detected")
            }
        }
        "capacity" => {
            let capacity_scope = (family == &OperationFamily::VecSetLen)
                .then(|| code_context_through_site(site).to_ascii_lowercase());
            let capacity_lower = capacity_scope.as_deref().unwrap_or(lower);
            if family == &OperationFamily::VecFromRawParts
                && has_vec_from_raw_parts_capacity_evidence(&site.operation.expression, lower)
            {
                EvidenceState::present(
                    "Vec::from_raw_parts length/capacity guard code was detected",
                )
            } else if family == &OperationFamily::VecFromRawParts
                && has_vec_from_raw_parts_origin_len_cap_evidence(&site.operation.expression, lower)
            {
                EvidenceState::present(
                    "Vec::from_raw_parts same-origin len/capacity evidence was detected",
                )
            } else if has_capacity_guard(family, capacity_lower) {
                EvidenceState::present("Capacity guard code was detected")
            } else {
                EvidenceState::missing("No capacity guard code was detected")
            }
        }
        "initialized" => {
            let local_lower;
            let init_scope = if family == &OperationFamily::VecSetLen {
                local_lower = code_context_through_site(site).to_ascii_lowercase();
                local_lower.as_str()
            } else {
                lower
            };
            if family == &OperationFamily::VecSetLen
                && has_set_len_initialization_evidence(init_scope)
            {
                EvidenceState::present("Initialization evidence was detected")
            } else if family == &OperationFamily::MaybeUninitAssumeInit
                && has_maybeuninit_assume_init_initialization_evidence(
                    &site.operation.expression,
                    lower,
                )
            {
                EvidenceState::present(
                    "MaybeUninit initialization evidence was detected before assume_init",
                )
            } else if family == &OperationFamily::SliceFromRawParts
                && has_maybeuninit_slice_context(lower)
            {
                EvidenceState::present("MaybeUninit slice element evidence was detected")
            } else if family == &OperationFamily::RawPointerWrite
                && has_maybeuninit_raw_write_context(site, lower)
            {
                EvidenceState::present("MaybeUninit raw write target evidence was detected")
            } else if family == &OperationFamily::RawPointerWrite
                && has_u8_write_bytes_context(site, lower)
            {
                EvidenceState::present("u8 write_bytes target evidence was detected")
            } else if family == &OperationFamily::RawPointerWrite
                && has_bool_write_bytes_value_evidence(site, lower)
            {
                EvidenceState::present("bool write_bytes value evidence was detected")
            } else if family == &OperationFamily::VecFromRawParts
                && has_vec_from_raw_parts_origin_initialized_evidence(
                    &site.operation.expression,
                    lower,
                )
            {
                EvidenceState::present(
                    "Vec::from_raw_parts same-origin initialized range evidence was detected",
                )
            } else if family == &OperationFamily::DropInPlace
                && has_drop_in_place_box_origin_evidence(&site.operation.expression, lower)
            {
                EvidenceState::present("Box::into_raw origin evidence was detected")
            } else if family == &OperationFamily::VecSetLen {
                EvidenceState::missing("No initialization evidence was detected")
            } else {
                EvidenceState::missing("No obligation-specific guard code was detected")
            }
        }
        "non-null" | "pointer-live" => {
            if family == &OperationFamily::VecFromRawParts
                && has_vec_from_raw_parts_origin_pointer_live_evidence(
                    &site.operation.expression,
                    lower,
                )
            {
                EvidenceState::present(
                    "Vec::from_raw_parts same-origin pointer/capacity evidence was detected",
                )
            } else if family == &OperationFamily::DropInPlace
                && has_drop_in_place_box_origin_evidence(&site.operation.expression, lower)
            {
                EvidenceState::present("Box::into_raw origin evidence was detected")
            } else if has_nullability_guard(site, lower) {
                EvidenceState::present("Nullability guard code was detected")
            } else if family == &OperationFamily::VecFromRawParts {
                EvidenceState::missing(
                    "No Vec::from_raw_parts same-origin pointer/capacity evidence was detected",
                )
            } else {
                EvidenceState::missing("No nullability guard code was detected")
            }
        }
        "ownership" => {
            if (family == &OperationFamily::DropInPlace
                && has_drop_in_place_box_origin_evidence(&site.operation.expression, lower))
                || (family == &OperationFamily::BoxFromRaw
                    && has_box_from_raw_origin_evidence(&site.operation.expression, lower))
                || (family == &OperationFamily::VecFromRawParts
                    && has_vec_from_raw_parts_origin_evidence(&site.operation.expression, lower))
            {
                if family == &OperationFamily::VecFromRawParts {
                    EvidenceState::present(
                        "ManuallyDrop Vec raw-parts ownership evidence was detected",
                    )
                } else {
                    EvidenceState::present("Box::into_raw ownership evidence was detected")
                }
            } else {
                EvidenceState::missing("No obligation-specific guard code was detected")
            }
        }
        "callee-contract" => {
            if family == &OperationFamily::UnsafeFnCall
                && has_encode_utf8_remaining_capacity_evidence(lower)
            {
                EvidenceState::present("Unsafe call argument guard code was detected")
            } else if family == &OperationFamily::UnsafeFnCall
                && has_unchecked_constructor_availability_evidence(
                    &site.operation.expression,
                    lower,
                )
            {
                EvidenceState::present("Unchecked constructor availability guard code was detected")
            } else {
                EvidenceState::missing("No obligation-specific guard code was detected")
            }
        }
        "valid-value" => {
            if family == &OperationFamily::UnwrapUnchecked
                && has_unwrap_unchecked_infallible_result_evidence(lower)
            {
                EvidenceState::present(
                    "Infallible Result state evidence was detected before unwrap_unchecked",
                )
            } else if family == &OperationFamily::UnwrapUnchecked
                && has_unwrap_unchecked_receiver_state_evidence(lower)
            {
                EvidenceState::present(
                    "Same-receiver Option/Result state evidence was detected before unwrap_unchecked",
                )
            } else if family == &OperationFamily::Transmute
                && has_transmute_u8_bool_valid_value_evidence(lower)
            {
                EvidenceState::present("Transmute u8-to-bool valid-value evidence was detected")
            } else {
                EvidenceState::missing("No obligation-specific guard code was detected")
            }
        }
        "layout" => {
            if family == &OperationFamily::Transmute && has_transmute_layout_size_evidence(lower) {
                EvidenceState::present("Transmute layout size evidence was detected")
            } else {
                EvidenceState::missing("No obligation-specific guard code was detected")
            }
        }
        "unreachable" => {
            if family == &OperationFamily::UnreachableUnchecked
                && has_unreachable_unchecked_infallible_path_evidence(lower)
            {
                EvidenceState::present(
                    "Infallible error-path evidence was detected before unreachable_unchecked",
                )
            } else {
                EvidenceState::missing("No obligation-specific guard code was detected")
            }
        }
        "target-feature" => {
            if family == &OperationFamily::TargetFeature && contract.present {
                EvidenceState::present(TARGET_FEATURE_CONTRACT_DISCHARGE)
            } else {
                EvidenceState::missing("No obligation-specific guard code was detected")
            }
        }
        "utf8" => {
            if family == &OperationFamily::StrFromUtf8Unchecked
                && has_from_utf8_unchecked_validation_evidence(lower)
            {
                EvidenceState::present(
                    "Same-buffer UTF-8 validation evidence was detected before from_utf8_unchecked",
                )
            } else {
                EvidenceState::missing("No obligation-specific guard code was detected")
            }
        }
        "valid-zero" => {
            if family == &OperationFamily::Zeroed && has_zeroed_known_valid_zero_type(lower) {
                EvidenceState::present(
                    "Known valid-zero target type evidence was detected before zeroed",
                )
            } else {
                EvidenceState::missing("No obligation-specific guard code was detected")
            }
        }
        _ => EvidenceState::missing("No obligation-specific guard code was detected"),
    }
}

fn is_public_unsafe_contract_obligation(site: &ScannedSite, key: &str) -> bool {
    key == "unknown"
        && site.site.public_api_surface
        && site.operation.family == OperationFamily::Unknown
        && matches!(
            site.site.kind,
            UnsafeSiteKind::UnsafeFn | UnsafeSiteKind::UnsafeTrait
        )
}

fn is_documented_private_unsafe_contract_obligation(
    site: &ScannedSite,
    key: &str,
    contract: &ContractEvidence,
) -> bool {
    key == "unknown"
        && !site.site.public_api_surface
        && contract.present
        && contract.summary.contains("documentation")
        && site.operation.family == OperationFamily::Unknown
        && matches!(
            site.site.kind,
            UnsafeSiteKind::UnsafeFn | UnsafeSiteKind::UnsafeTrait
        )
}

fn has_length_or_bounds_guard(lower: &str) -> bool {
    let compact = compact_code(lower);
    has_bounds_assertion_guard(&compact)
        || has_bounds_open_positive_branch_guard(&compact)
        || has_len_capacity_equality_guard(lower)
}

fn has_bounds_guard(site: &ScannedSite, lower: &str) -> bool {
    if site.operation.family == OperationFamily::GetUnchecked
        && let Some((receiver, index)) =
            get_unchecked_receiver_and_index(&site.operation.expression)
    {
        let guard_scope = code_before_operation(lower, &site.operation.expression)
            .unwrap_or_else(|| lower.to_string());
        return has_get_unchecked_bounds_guard(&guard_scope, &receiver, &index);
    }
    if site.operation.family == OperationFamily::RawPointerWrite
        && site
            .operation
            .expression
            .to_ascii_lowercase()
            .contains("write_bytes")
    {
        return has_write_bytes_bounds_evidence(&site.operation.expression);
    }
    let guard_scope = code_before_operation(lower, &site.operation.expression)
        .unwrap_or_else(|| lower.to_string());
    if site.operation.family == OperationFamily::RawPointerRead {
        return has_raw_pointer_read_bounds_evidence(&site.operation.expression, &guard_scope);
    }
    if matches!(
        site.operation.family,
        OperationFamily::CopyNonOverlapping | OperationFamily::PtrCopy
    ) {
        if has_copy_slice_range_evidence(&site.operation.expression, &guard_scope) {
            return true;
        }
        // A generic length comparison does not prove both copy source and destination ranges.
        return false;
    }
    has_length_or_bounds_guard(&guard_scope)
}

fn has_copy_slice_range_evidence(expression: &str, before_call: &str) -> bool {
    let Some((src, dst, count)) = copy_call_arguments(expression) else {
        return false;
    };
    let Some(src_receiver) = copy_source_slice_receiver(&src) else {
        return false;
    };
    let Some(dst_receiver) = copy_destination_slice_receiver(&dst) else {
        return false;
    };

    has_slice_count_bound_guard(before_call, &src_receiver, &count)
        && has_slice_count_bound_guard(before_call, &dst_receiver, &count)
}

fn copy_call_arguments(expression: &str) -> Option<(String, String, String)> {
    let compact = compact_code(&expression.to_ascii_lowercase());
    for marker in ["copy_nonoverlapping(", "ptr::copy("] {
        let Some(call_pos) = compact.find(marker) else {
            continue;
        };
        let after_marker = &compact[call_pos + marker.len()..];
        let Some(end) = matching_call_argument_end(after_marker) else {
            continue;
        };
        let args = split_top_level_arguments(&after_marker[..end]);
        if args.len() == 3 && args.iter().all(|arg| !arg.is_empty()) {
            return Some((
                args[0].to_string(),
                args[1].to_string(),
                args[2].to_string(),
            ));
        }
    }
    None
}

fn copy_source_slice_receiver(argument: &str) -> Option<String> {
    receiver_before_marker(argument, ".as_ptr()").map(str::to_string)
}

fn copy_destination_slice_receiver(argument: &str) -> Option<String> {
    receiver_before_marker(argument, ".as_mut_ptr()").map(str::to_string)
}

fn has_slice_count_bound_guard(before_call: &str, receiver: &str, count: &str) -> bool {
    let receiver = compact_code(receiver);
    let count = compact_code(count);
    if receiver.is_empty() || count.is_empty() {
        return false;
    }
    let len = format!("{receiver}.len()");
    let count_lte_len = format!("{count}<={len}");
    let len_gte_count = format!("{len}>={count}");
    let count_gt_len = format!("{count}>{len}");
    let len_lt_count = format!("{len}<{count}");
    has_slice_count_bound_predicate(before_call, &count_lte_len, &receiver, &count)
        || has_slice_count_bound_predicate(before_call, &len_gte_count, &receiver, &count)
        || has_slice_count_early_return(before_call, &count_gt_len, &receiver, &count)
        || has_slice_count_early_return(before_call, &len_lt_count, &receiver, &count)
}

fn has_slice_count_bound_predicate(
    before_call: &str,
    predicate: &str,
    receiver: &str,
    count: &str,
) -> bool {
    has_slice_count_assertion_guard(before_call, predicate, receiver, count)
        || has_open_slice_count_branch_guard(before_call, predicate, receiver, count)
}

fn has_slice_count_assertion_guard(
    before_call: &str,
    predicate: &str,
    receiver: &str,
    count: &str,
) -> bool {
    ["assert!(", "debug_assert!("].into_iter().any(|prefix| {
        let mut search_from = 0;
        while let Some(offset) = before_call[search_from..].find(prefix) {
            let call_start = search_from + offset + prefix.len();
            let after_prefix = &before_call[call_start..];
            let Some(call_end) = matching_call_argument_end(after_prefix) else {
                search_from = call_start;
                continue;
            };
            let args = split_top_level_arguments(&after_prefix[..call_end]);
            let after_call = &after_prefix[call_end..];
            let statement_end = after_call.find(';').unwrap_or(after_call.len());
            let after_guard = &after_call[statement_end..];
            if args
                .first()
                .is_some_and(|condition| condition_has_top_level_conjunct(condition, predicate))
                && !has_slice_count_assignment(after_guard, receiver, count)
            {
                return true;
            }
            search_from = call_start + call_end;
        }
        false
    })
}

fn has_open_slice_count_branch_guard(
    before_call: &str,
    predicate: &str,
    receiver: &str,
    count: &str,
) -> bool {
    let mut search_from = 0;
    while let Some(offset) = before_call[search_from..].find("if") {
        let guard_start = search_from + offset;
        let before = before_call[..guard_start].chars().next_back();
        if before.is_some_and(is_receiver_path_char) {
            search_from = guard_start + 2;
            continue;
        }
        let after_if = &before_call[guard_start + 2..];
        if let Some(brace_pos) = after_if.find('{') {
            let condition = &after_if[..brace_pos];
            let after_guard = &after_if[brace_pos + 1..];
            if condition_has_top_level_conjunct(condition, predicate)
                && branch_still_open_at_operation(after_guard)
                && !has_slice_count_assignment(after_guard, receiver, count)
            {
                return true;
            }
        }
        search_from = guard_start + 2;
    }
    false
}

fn condition_has_top_level_conjunct(condition: &str, predicate: &str) -> bool {
    let condition = strip_balanced_outer_parens(condition.trim());
    split_top_level_conjuncts(condition)
        .into_iter()
        .any(|conjunct| strip_balanced_outer_parens(conjunct.trim()) == predicate)
}

fn condition_has_top_level_disjunct(condition: &str, predicate: &str) -> bool {
    let condition = strip_balanced_outer_parens(condition.trim());
    split_top_level_disjuncts(condition)
        .into_iter()
        .any(|disjunct| strip_balanced_outer_parens(disjunct.trim()) == predicate)
}

fn split_top_level_conjuncts(condition: &str) -> Vec<&str> {
    split_top_level_condition_operands(condition, b'&')
}

fn split_top_level_disjuncts(condition: &str) -> Vec<&str> {
    split_top_level_condition_operands(condition, b'|')
}

fn split_top_level_condition_operands(condition: &str, operator: u8) -> Vec<&str> {
    let mut conjuncts = Vec::new();
    let mut start = 0usize;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;
    let bytes = condition.as_bytes();
    let mut idx = 0usize;
    while idx < bytes.len() {
        match bytes[idx] {
            b'(' => paren_depth += 1,
            b')' => paren_depth = paren_depth.saturating_sub(1),
            b'[' => bracket_depth += 1,
            b']' => bracket_depth = bracket_depth.saturating_sub(1),
            b'{' => brace_depth += 1,
            b'}' => brace_depth = brace_depth.saturating_sub(1),
            byte if byte == operator
                && idx + 1 < bytes.len()
                && bytes[idx + 1] == operator
                && paren_depth == 0
                && bracket_depth == 0
                && brace_depth == 0 =>
            {
                conjuncts.push(condition[start..idx].trim());
                idx += 2;
                start = idx;
                continue;
            }
            _ => {}
        }
        idx += 1;
    }
    conjuncts.push(condition[start..].trim());
    conjuncts
}

fn strip_balanced_outer_parens(mut text: &str) -> &str {
    loop {
        let Some(inner) = text
            .strip_prefix('(')
            .and_then(|inner| inner.strip_suffix(')'))
        else {
            return text;
        };
        if !outer_parens_enclose_whole_expression(text) {
            return text;
        }
        text = inner.trim();
    }
}

fn outer_parens_enclose_whole_expression(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.first() != Some(&b'(') || bytes.last() != Some(&b')') {
        return false;
    }
    let mut depth = 0usize;
    for (idx, byte) in bytes.iter().enumerate() {
        match byte {
            b'(' => depth += 1,
            b')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 && idx != bytes.len() - 1 {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0
}

fn has_slice_count_early_return(
    before_call: &str,
    predicate: &str,
    receiver: &str,
    count: &str,
) -> bool {
    let mut search_from = 0;
    while let Some(offset) = before_call[search_from..].find("if") {
        let guard_start = search_from + offset;
        let before = before_call[..guard_start].chars().next_back();
        if before.is_some_and(is_receiver_path_char) {
            search_from = guard_start + 2;
            continue;
        }
        let after_if = &before_call[guard_start + 2..];
        if let Some(brace_pos) = after_if.find('{') {
            let condition = &after_if[..brace_pos];
            let after_guard = &after_if[brace_pos + 1..];
            let (guard_body, after_guard_body) = matching_code_block_end(after_guard)
                .map_or((after_guard, ""), |body_end| {
                    (&after_guard[..body_end], &after_guard[body_end + 1..])
                });
            if condition_has_top_level_disjunct(condition, predicate)
                && guard_body_contains_return(guard_body)
                && !has_slice_count_assignment(after_guard_body, receiver, count)
            {
                return true;
            }
        }
        search_from = guard_start + 2;
    }
    false
}

fn guard_body_contains_return(guard_body: &str) -> bool {
    let code = strip_block_comments_and_literals(guard_body);
    compact_contains_identifier(&code, "return")
}

fn strip_block_comments_and_literals(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            let mut prev = '\0';
            for comment_ch in chars.by_ref() {
                if prev == '*' && comment_ch == '/' {
                    break;
                }
                prev = comment_ch;
            }
            continue;
        }
        if ch == '"' {
            output.push('"');
            let mut escaped = false;
            for literal_ch in chars.by_ref() {
                if escaped {
                    escaped = false;
                    continue;
                }
                if literal_ch == '\\' {
                    escaped = true;
                    continue;
                }
                if literal_ch == '"' {
                    output.push('"');
                    break;
                }
            }
            continue;
        }
        output.push(ch);
    }
    output
}

fn has_slice_count_assignment(compact: &str, receiver: &str, count: &str) -> bool {
    contains_simple_assignment_to(compact, receiver)
        || contains_simple_assignment_to(compact, count)
}

fn has_bounds_assertion_guard(compact: &str) -> bool {
    ["assert!(", "debug_assert!("].into_iter().any(|prefix| {
        let mut cursor = compact;
        let mut offset = 0usize;
        while let Some(pos) = cursor.find(prefix) {
            let statement_start = offset + pos + prefix.len();
            let after_prefix = &compact[statement_start..];
            let statement_end = after_prefix.find(';').unwrap_or(after_prefix.len());
            let statement = &after_prefix[..statement_end];
            if has_bounds_condition(statement) {
                return true;
            }
            let next = pos + prefix.len();
            offset += next;
            cursor = &cursor[next..];
        }
        false
    })
}

fn has_bounds_open_positive_branch_guard(compact: &str) -> bool {
    let mut cursor = compact;
    let mut offset = 0usize;
    while let Some(pos) = cursor.find("if") {
        let start = offset + pos;
        let before = compact[..start].chars().next_back();
        if before.is_some_and(is_receiver_path_char) {
            let next = pos + 2;
            offset += next;
            cursor = &cursor[next..];
            continue;
        }
        let after_if = &compact[start + 2..];
        if let Some(brace_pos) = after_if.find('{') {
            let condition = &after_if[..brace_pos];
            let after_body_start = &after_if[brace_pos + 1..];
            if has_bounds_condition(condition) && branch_still_open_at_operation(after_body_start) {
                return true;
            }
        }
        let next = pos + 2;
        offset += next;
        cursor = &cursor[next..];
    }
    false
}

fn has_bounds_condition(compact: &str) -> bool {
    for op in [">=", "<=", "<", ">"] {
        let mut cursor = compact;
        let mut offset = 0usize;
        while let Some(pos) = cursor.find(op) {
            let op_start = offset + pos;
            let op_end = op_start + op.len();
            let left = comparison_left_operand(compact, op_start);
            let right = comparison_right_operand(compact, op_end);
            if operand_mentions_bounds(left) || operand_mentions_bounds(right) {
                return true;
            }
            let next = pos + op.len();
            offset += next;
            cursor = &cursor[next..];
        }
    }
    false
}

fn comparison_left_operand(compact: &str, op_start: usize) -> &str {
    let left = &compact[..op_start];
    let start = left
        .rfind(['{', ';', ',', '=', '!'])
        .map_or(0, |idx| idx + 1);
    &left[start..]
}

fn comparison_right_operand(compact: &str, op_end: usize) -> &str {
    let right = &compact[op_end..];
    let end = right.find(['{', '}', ';', ',', '=']).unwrap_or(right.len());
    &right[..end]
}

fn operand_mentions_bounds(operand: &str) -> bool {
    operand.contains(".len()")
        || operand.contains(".capacity()")
        || operand.contains("num_ctrl_bytes()")
        || compact_contains_identifier(operand, "len")
        || compact_contains_identifier(operand, "capacity")
}

fn compact_contains_identifier(text: &str, ident: &str) -> bool {
    let mut cursor = text;
    while let Some(pos) = cursor.find(ident) {
        let before = cursor[..pos].chars().next_back();
        let after = cursor[pos + ident.len()..].chars().next();
        if before.is_none_or(|ch| !is_receiver_path_char(ch))
            && after.is_none_or(|ch| !is_receiver_path_char(ch))
        {
            return true;
        }
        let next = pos + ident.len();
        cursor = &cursor[next..];
    }
    false
}

fn code_before_operation(lower: &str, expression: &str) -> Option<String> {
    let compact = compact_code(lower);
    let expression = compact_code(&expression.to_ascii_lowercase());
    if expression.is_empty() {
        return None;
    }
    compact
        .find(&expression)
        .map(|operation_pos| compact[..operation_pos].to_string())
}

fn any_marker_occurrence(
    text: &str,
    marker: &str,
    mut applies: impl FnMut(usize, &str) -> bool,
) -> bool {
    let mut cursor = text;
    let mut offset = 0usize;
    while let Some(pos) = cursor.find(marker) {
        let marker_start = offset + pos;
        let after_marker = &text[marker_start + marker.len()..];
        if applies(marker_start, after_marker) {
            return true;
        }
        let next = pos + marker.len();
        offset += next;
        cursor = &cursor[next..];
    }
    false
}

fn any_marker_tail(text: &str, marker: &str, mut applies: impl FnMut(&str) -> bool) -> bool {
    any_marker_occurrence(text, marker, |_marker_start, after_marker| {
        applies(after_marker)
    })
}

fn branch_still_open_at_operation(after_guard: &str) -> bool {
    let mut depth = 1usize;
    for ch in after_guard.chars() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

fn contains_simple_assignment_to(compact: &str, name: &str) -> bool {
    if !is_simple_identifier(name) {
        return false;
    }
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
        if before.is_none_or(|ch| !is_receiver_path_char(ch)) && after_equals != Some('=') {
            return true;
        }
        let next = pos + marker.len();
        offset += next;
        cursor = &cursor[next..];
    }
    false
}

fn has_len_capacity_equality_guard(lower: &str) -> bool {
    let compact = compact_code(lower);
    let has_equality = compact.contains("==")
        || compact.contains("assert_eq!(")
        || compact.contains("debug_assert_eq!(");
    has_equality
        && compact.contains("len")
        && (compact.contains("capacity") || contains_word(&compact, "cap"))
}

fn has_raw_pointer_read_bounds_evidence(expression: &str, before_operation: &str) -> bool {
    let compact_expression = compact_code(&expression.to_ascii_lowercase());
    let Some(pointer) = raw_pointer_read_pointer_receiver(&compact_expression) else {
        return has_length_or_bounds_guard(before_operation);
    };
    let before_operation = compact_code(before_operation);
    let Some(origin) = pointer_origin_receiver_before(&before_operation, pointer) else {
        return false;
    };

    has_origin_len_size_guard(&before_operation, &origin)
        || has_origin_len_capacity_equality_guard(&before_operation, &origin)
}

fn raw_pointer_read_pointer_receiver(compact_expression: &str) -> Option<&str> {
    if let Some(receiver) = receiver_before_marker(compact_expression, ".cast::<") {
        return Some(receiver);
    }
    if let Some(receiver) = receiver_before_marker(compact_expression, ".read(") {
        return Some(receiver);
    }
    if let Some(receiver) = receiver_before_marker(compact_expression, ".read_volatile(") {
        return Some(receiver);
    }
    raw_pointer_read_function_argument(compact_expression)
}

fn raw_pointer_read_function_argument(compact_expression: &str) -> Option<&str> {
    let marker = "ptr::read(";
    let call_pos = compact_expression.find(marker)? + marker.len();
    let after_marker = &compact_expression[call_pos..];
    let argument_end = matching_call_argument_end(after_marker)?;
    let argument = after_marker[..argument_end]
        .split_once("as*")
        .map_or(&after_marker[..argument_end], |(argument, _)| argument)
        .trim();
    (!argument.is_empty()).then_some(argument)
}

fn pointer_origin_receiver_before(before_operation: &str, pointer: &str) -> Option<String> {
    if pointer.contains(".as_ptr()") || pointer.contains(".as_mut_ptr()") {
        return pointer_origin_receiver(pointer).map(str::to_string);
    }
    let mut current_origin = None;
    for statement in before_operation.split(';') {
        let Some((left, right)) = statement.rsplit_once('=') else {
            continue;
        };
        let Some(binding) = assignment_binding_name(left) else {
            continue;
        };
        if binding != pointer {
            continue;
        }
        current_origin = pointer_origin_receiver(right).map(str::to_string);
    }
    current_origin
}

fn pointer_origin_receiver(expression: &str) -> Option<&str> {
    let expression = pointer_expression_before_type_change(expression);
    expression
        .strip_suffix(".as_ptr()")
        .or_else(|| expression.strip_suffix(".as_mut_ptr()"))
        .filter(|receiver| !receiver.is_empty())
}

fn pointer_expression_before_type_change(expression: &str) -> &str {
    expression
        .find(".cast::<")
        .or_else(|| expression.find(".cast()"))
        .or_else(|| expression.find("as*const"))
        .or_else(|| expression.find("as*mut"))
        .map_or(expression, |cast_pos| &expression[..cast_pos])
}

fn assignment_binding_name(left_side: &str) -> Option<&str> {
    if let Some(binding) = let_binding_name(left_side) {
        return Some(binding);
    }
    is_simple_identifier(left_side).then_some(left_side)
}

fn has_origin_len_size_guard(compact: &str, origin: &str) -> bool {
    let len = format!("{origin}.len()");
    has_origin_len_size_assertion_guard(compact, &len, origin)
        || has_origin_len_size_open_positive_branch_guard(compact, &len, origin)
        || has_origin_len_size_early_return_guard(compact, &len, origin)
}

fn has_origin_len_size_assertion_guard(compact: &str, len: &str, origin: &str) -> bool {
    ["assert!(", "debug_assert!("].into_iter().any(|prefix| {
        let mut cursor = compact;
        let mut offset = 0usize;
        while let Some(pos) = cursor.find(prefix) {
            let statement_start = offset + pos + prefix.len();
            let after_prefix = &compact[statement_start..];
            let statement_end = after_prefix.find(';').unwrap_or(after_prefix.len());
            let statement = &after_prefix[..statement_end];
            let after_statement = &after_prefix[statement_end..];
            if origin_len_size_condition_is_positive(statement, len)
                && !contains_simple_assignment_to(after_statement, origin)
            {
                return true;
            }
            let next = pos + prefix.len();
            offset += next;
            cursor = &cursor[next..];
        }
        false
    })
}

fn has_origin_len_size_open_positive_branch_guard(compact: &str, len: &str, origin: &str) -> bool {
    compact_if_guards(compact).any(|guard| {
        origin_len_size_condition_is_positive(guard.condition, len)
            && branch_still_open_at_operation(guard.after_body_start)
            && !contains_simple_assignment_to(guard.after_body_start, origin)
    })
}

fn has_origin_len_size_early_return_guard(compact: &str, len: &str, origin: &str) -> bool {
    compact_if_guards(compact).any(|guard| {
        if !origin_len_size_condition_is_negative(guard.condition, len) {
            return false;
        }
        let (guard_body, after_guard_body) = guard
            .after_body_start
            .split_once('}')
            .map_or((guard.after_body_start, ""), |(guard_body, after)| {
                (guard_body, after)
            });
        guard_body.contains("return") && !contains_simple_assignment_to(after_guard_body, origin)
    })
}

fn origin_len_size_condition_is_positive(condition: &str, len: &str) -> bool {
    condition.contains("size_of")
        && (condition.contains(&format!("{len}>"))
            || condition.contains(&format!("<{len}"))
            || condition.contains(&format!("<={len}")))
}

fn origin_len_size_condition_is_negative(condition: &str, len: &str) -> bool {
    condition.contains("size_of")
        && (condition.contains(&format!("{len}<"))
            || condition.contains(&format!(">{len}"))
            || condition.contains(&format!(">={len}")))
}

fn has_origin_len_capacity_equality_guard(compact: &str, origin: &str) -> bool {
    let len = format!("{origin}.len()");
    let capacity = format!("{origin}.capacity()");
    let cap = format!("{origin}.cap()");
    has_origin_len_capacity_assertion_guard(compact, &len, &capacity, &cap, origin)
        || has_origin_len_capacity_open_positive_branch_guard(
            compact, &len, &capacity, &cap, origin,
        )
}

fn has_origin_len_capacity_assertion_guard(
    compact: &str,
    len: &str,
    capacity: &str,
    cap: &str,
    origin: &str,
) -> bool {
    [
        ("assert_eq!(", false),
        ("debug_assert_eq!(", false),
        ("assert!(", true),
        ("debug_assert!(", true),
    ]
    .into_iter()
    .any(|(prefix, requires_operator)| {
        let mut cursor = compact;
        let mut offset = 0usize;
        while let Some(pos) = cursor.find(prefix) {
            let statement_start = offset + pos + prefix.len();
            let after_prefix = &compact[statement_start..];
            let statement_end = after_prefix.find(';').unwrap_or(after_prefix.len());
            let statement = &after_prefix[..statement_end];
            let after_statement = &after_prefix[statement_end..];
            if origin_len_capacity_condition_matches(statement, len, capacity, cap)
                && (!requires_operator || statement.contains("=="))
                && !contains_simple_assignment_to(after_statement, origin)
            {
                return true;
            }
            let next = pos + prefix.len();
            offset += next;
            cursor = &cursor[next..];
        }
        false
    })
}

fn has_origin_len_capacity_open_positive_branch_guard(
    compact: &str,
    len: &str,
    capacity: &str,
    cap: &str,
    origin: &str,
) -> bool {
    compact_if_guards(compact).any(|guard| {
        origin_len_capacity_condition_matches(guard.condition, len, capacity, cap)
            && guard.condition.contains("==")
            && branch_still_open_at_operation(guard.after_body_start)
            && !contains_simple_assignment_to(guard.after_body_start, origin)
    })
}

fn origin_len_capacity_condition_matches(
    condition: &str,
    len: &str,
    capacity: &str,
    cap: &str,
) -> bool {
    condition.contains(len) && (condition.contains(capacity) || condition.contains(cap))
}

fn has_slice_end_pointer_arithmetic_evidence(lower: &str) -> bool {
    let compact = compact_code(lower);
    for line in lower.lines() {
        let line = compact_code(line);
        let Some(after_let) = line.strip_prefix("let") else {
            continue;
        };
        let Some((binding, expr)) = after_let.split_once('=') else {
            continue;
        };
        let Some(slice_expr) = expr.strip_suffix(".as_ptr();") else {
            continue;
        };
        if !binding.is_empty()
            && !slice_expr.is_empty()
            && compact.contains(&format!("{binding}.add({slice_expr}.len())"))
        {
            return true;
        }
    }
    false
}

fn has_capacity_guard(family: &OperationFamily, lower: &str) -> bool {
    if family == &OperationFamily::VecSetLen {
        return has_set_len_capacity_evidence(lower);
    }
    if family == &OperationFamily::VecFromRawParts {
        return false;
    }
    lower.contains("capacity") || lower.contains("cap()")
}

fn has_vec_from_raw_parts_capacity_evidence(expression: &str, lower: &str) -> bool {
    let compact = compact_code(&strip_block_comments_and_literals(lower));
    let compact_expression = compact_code(&expression.to_ascii_lowercase());
    let Some((_ptr, len, cap)) = vec_from_raw_parts_arguments(&compact_expression) else {
        return false;
    };
    let call_pos = compact
        .find(&compact_expression)
        .or_else(|| compact.find("vec::from_raw_parts("));
    let Some(call_pos) = call_pos else {
        return false;
    };
    let before_call = &compact[..call_pos];
    has_len_cap_bound_guard(before_call, len, cap)
}

fn has_vec_from_raw_parts_origin_len_cap_evidence(expression: &str, lower: &str) -> bool {
    let compact = compact_code(lower);
    let compact_expression = compact_code(&expression.to_ascii_lowercase());
    let Some((_ptr, len, cap)) = vec_from_raw_parts_arguments(&compact_expression) else {
        return false;
    };
    let call_pos = compact
        .find(&compact_expression)
        .or_else(|| compact.find("vec::from_raw_parts("));
    let Some(call_pos) = call_pos else {
        return false;
    };
    has_vec_from_raw_parts_same_origin_len_cap(&compact[..call_pos], len, cap)
}

fn has_vec_from_raw_parts_origin_initialized_evidence(expression: &str, lower: &str) -> bool {
    let compact = compact_code(lower);
    let compact_expression = compact_code(&expression.to_ascii_lowercase());
    let Some((ptr, len, _cap)) = vec_from_raw_parts_arguments(&compact_expression) else {
        return false;
    };
    let call_pos = compact
        .find(&compact_expression)
        .or_else(|| compact.find("vec::from_raw_parts("));
    let Some(call_pos) = call_pos else {
        return false;
    };
    let before_call = &compact[..call_pos];
    let Some(ptr_receiver) = vec_raw_parts_pointer_origin_receiver_before(before_call, ptr) else {
        return false;
    };
    vec_raw_parts_len_origin_receiver(before_call, len)
        .is_some_and(|receiver| receiver == ptr_receiver)
}

fn has_vec_from_raw_parts_origin_pointer_live_evidence(expression: &str, lower: &str) -> bool {
    let compact = compact_code(lower);
    let compact_expression = compact_code(&expression.to_ascii_lowercase());
    let Some((ptr, _len, cap)) = vec_from_raw_parts_arguments(&compact_expression) else {
        return false;
    };
    let call_pos = compact
        .find(&compact_expression)
        .or_else(|| compact.find("vec::from_raw_parts("));
    let Some(call_pos) = call_pos else {
        return false;
    };
    let before_call = &compact[..call_pos];
    let Some(ptr_receiver) = vec_raw_parts_pointer_origin_receiver_before(before_call, ptr) else {
        return false;
    };
    vec_raw_parts_capacity_origin_receiver(before_call, cap)
        .is_some_and(|receiver| receiver == ptr_receiver)
}

fn has_vec_from_raw_parts_origin_evidence(expression: &str, lower: &str) -> bool {
    let compact = compact_code(lower);
    let compact_expression = compact_code(&expression.to_ascii_lowercase());
    let Some((ptr, _len, _cap)) = vec_from_raw_parts_arguments(&compact_expression) else {
        return false;
    };
    let call_pos = compact
        .find(&compact_expression)
        .or_else(|| compact.find("vec::from_raw_parts("));
    let Some(call_pos) = call_pos else {
        return false;
    };
    has_same_pointer_vec_raw_parts_origin_before(&compact[..call_pos], ptr)
}

fn vec_from_raw_parts_arguments(compact_expression: &str) -> Option<(&str, &str, &str)> {
    let marker = "from_raw_parts(";
    let call_pos = compact_expression.find(marker)?;
    let after_marker = &compact_expression[call_pos + marker.len()..];
    let end = matching_call_argument_end(after_marker)?;
    let args = split_top_level_arguments(&after_marker[..end]);
    if args.len() == 3 && args.iter().all(|arg| !arg.is_empty()) {
        Some((args[0], args[1], args[2]))
    } else {
        None
    }
}

fn split_top_level_arguments(text: &str) -> Vec<&str> {
    let mut args = Vec::new();
    let mut start = 0usize;
    let mut angle_depth = 0usize;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;
    for (idx, ch) in text.char_indices() {
        match ch {
            '<' => angle_depth += 1,
            '>' => angle_depth = angle_depth.saturating_sub(1),
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '{' => brace_depth += 1,
            '}' => brace_depth = brace_depth.saturating_sub(1),
            ',' if angle_depth == 0
                && paren_depth == 0
                && bracket_depth == 0
                && brace_depth == 0 =>
            {
                args.push(text[start..idx].trim());
                start = idx + ch.len_utf8();
            }
            _ => {}
        }
    }
    args.push(text[start..].trim());
    args
}

fn has_len_cap_bound_guard(before_call: &str, len: &str, cap: &str) -> bool {
    let len = compact_code(len);
    let cap = compact_code(cap);
    if len.is_empty() || cap.is_empty() {
        return false;
    }
    let len_lte_cap = format!("{len}<={cap}");
    let cap_gte_len = format!("{cap}>={len}");
    let len_gt_cap = format!("{len}>{cap}");
    let cap_lt_len = format!("{cap}<{len}");
    has_len_cap_bound_predicate(before_call, &len_lte_cap, &len, &cap)
        || has_len_cap_bound_predicate(before_call, &cap_gte_len, &len, &cap)
        || has_len_cap_early_return(before_call, &len_gt_cap, &len, &cap)
        || has_len_cap_early_return(before_call, &cap_lt_len, &len, &cap)
}

fn has_len_cap_bound_predicate(before_call: &str, predicate: &str, len: &str, cap: &str) -> bool {
    [
        format!("assert!({predicate})"),
        format!("assert!({predicate},"),
        format!("debug_assert!({predicate})"),
        format!("debug_assert!({predicate},"),
    ]
    .iter()
    .any(|pattern| has_fresh_len_cap_guard_pattern(before_call, pattern, len, cap))
        || has_open_len_cap_branch_guard(before_call, predicate, len, cap)
}

fn has_fresh_len_cap_guard_pattern(before_call: &str, pattern: &str, len: &str, cap: &str) -> bool {
    let mut search_from = 0;
    while let Some(offset) = before_call[search_from..].find(pattern) {
        let pattern_start = search_from + offset;
        let after_pattern = &before_call[pattern_start + pattern.len()..];
        let statement_end = after_pattern.find(';').unwrap_or(after_pattern.len());
        let after_guard = &after_pattern[statement_end..];
        if !has_len_cap_assignment(after_guard, len, cap) {
            return true;
        }
        search_from = pattern_start + pattern.len();
    }
    false
}

fn has_open_len_cap_branch_guard(before_call: &str, predicate: &str, len: &str, cap: &str) -> bool {
    let guard = format!("if{predicate}{{");
    let mut search_from = 0;
    while let Some(offset) = before_call[search_from..].find(&guard) {
        let guard_start = search_from + offset;
        let after_guard = &before_call[guard_start + guard.len()..];
        if branch_still_open_at_operation(after_guard)
            && !has_len_cap_assignment(after_guard, len, cap)
        {
            return true;
        }
        search_from = guard_start + guard.len();
    }
    false
}

fn has_len_cap_early_return(before_call: &str, predicate: &str, len: &str, cap: &str) -> bool {
    let guard = format!("if{predicate}{{");
    let mut search_from = 0;
    while let Some(offset) = before_call[search_from..].find(&guard) {
        let guard_start = search_from + offset;
        let after_guard = &before_call[guard_start + guard.len()..];
        let (guard_body, after_guard_body) = matching_code_block_end(after_guard)
            .map_or((after_guard, ""), |body_end| {
                (&after_guard[..body_end], &after_guard[body_end + 1..])
            });
        if guard_body.contains("return") && !has_len_cap_assignment(after_guard_body, len, cap) {
            return true;
        }
        search_from = guard_start + guard.len();
    }
    false
}

fn has_len_cap_assignment(compact: &str, len: &str, cap: &str) -> bool {
    contains_simple_assignment_to(compact, len) || contains_simple_assignment_to(compact, cap)
}

fn has_vec_from_raw_parts_same_origin_len_cap(before_call: &str, len: &str, cap: &str) -> bool {
    vec_raw_parts_len_origin_receiver(before_call, len).is_some_and(|receiver| {
        vec_raw_parts_capacity_origin_receiver(before_call, cap) == Some(receiver)
    })
}

fn has_set_len_capacity_evidence(lower: &str) -> bool {
    has_set_len_shrink_evidence(lower)
        || has_set_len_call_result_initialization_evidence(lower)
        || has_set_len_const_cap_evidence(lower)
        || has_set_len_with_capacity_evidence(lower)
        || has_set_len_reserve_capacity_evidence(lower)
        || has_capacity_bound_guard(lower)
}

fn has_capacity_bound_guard(lower: &str) -> bool {
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
        set_len::has_capacity_bound_guard(
            self.before_call,
            self.same_vec_target,
            self.set_len_argument,
        )
    }

    fn has_const_capacity_evidence(&self) -> bool {
        set_len::has_const_capacity_evidence(
            self.before_call,
            self.same_vec_target,
            self.set_len_argument,
        )
    }

    fn has_reserve_capacity_evidence(&self) -> bool {
        set_len::has_reserve_capacity_evidence(
            self.before_call,
            self.same_vec_target,
            self.set_len_argument,
        )
    }

    fn has_with_capacity_evidence(&self) -> bool {
        set_len::has_with_capacity_evidence(
            self.before_call,
            self.same_vec_target,
            self.set_len_argument,
        )
    }

    fn has_call_result_initialization_evidence(&self) -> bool {
        set_len::has_call_result_initialization_evidence(self.before_call, self.set_len_argument)
    }

    fn has_initialized_range_evidence(&self) -> bool {
        set_len::has_initialized_range_evidence(
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

pub(super) fn let_binding_name(left_side: &str) -> Option<&str> {
    let let_pos = left_side.rfind("let")?;
    let rest = &left_side[let_pos + "let".len()..];
    let rest = rest.strip_prefix("mut").unwrap_or(rest);
    let end = rest
        .char_indices()
        .find_map(|(idx, ch)| (!(ch == '_' || ch.is_ascii_alphanumeric())).then_some(idx))
        .unwrap_or(rest.len());
    (end > 0).then_some(&rest[..end])
}

fn has_drop_in_place_box_origin_evidence(expression: &str, lower: &str) -> bool {
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

fn has_box_from_raw_origin_evidence(expression: &str, lower: &str) -> bool {
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

fn has_same_pointer_vec_raw_parts_origin_before(before_call: &str, pointer: &str) -> bool {
    vec_raw_parts_pointer_origin_receiver_before(before_call, pointer).is_some()
}

fn vec_raw_parts_pointer_origin_receiver_before(
    before_call: &str,
    pointer: &str,
) -> Option<String> {
    let mut prior_statements = String::new();
    for statement in before_call.split(';') {
        let Some((left, right)) = statement.split_once('=') else {
            prior_statements.push_str(statement);
            prior_statements.push(';');
            continue;
        };
        let Some(binding) = let_binding_name(left) else {
            prior_statements.push_str(statement);
            prior_statements.push(';');
            continue;
        };
        if binding == pointer
            && let Some(receiver) = vec_raw_pointer_receiver(right)
            && vec_raw_pointer_receiver_has_manually_drop_origin(&prior_statements, receiver)
        {
            return Some(receiver.to_string());
        }
        prior_statements.push_str(statement);
        prior_statements.push(';');
    }
    None
}

fn vec_raw_pointer_receiver(right_side: &str) -> Option<&str> {
    receiver_before_marker(right_side, ".as_mut_ptr(")
        .or_else(|| receiver_before_marker(right_side, ".as_ptr("))
}

fn vec_raw_pointer_receiver_has_manually_drop_origin(before_call: &str, receiver: &str) -> bool {
    before_call.split(';').any(|statement| {
        let Some((left, right)) = statement.split_once('=') else {
            return false;
        };
        let Some(binding) = let_binding_name(left) else {
            return false;
        };
        binding == receiver && right.contains("manuallydrop::new(")
    })
}

fn vec_raw_parts_len_origin_receiver(before_call: &str, len: &str) -> Option<String> {
    let len = compact_code(len);
    if len.is_empty() {
        return None;
    }

    let mut origin_receivers = Vec::new();
    for statement in before_call.split(';') {
        let Some((left, right)) = statement.split_once('=') else {
            continue;
        };
        let Some(binding) = let_binding_name(left) else {
            continue;
        };
        if right.contains("manuallydrop::new(") {
            origin_receivers.push(binding.to_string());
        }
        if binding == len
            && let Some(receiver) = receiver_before_marker(right, ".len(")
            && origin_receivers.iter().any(|origin| origin == receiver)
        {
            return Some(receiver.to_string());
        }
    }
    None
}

fn vec_raw_parts_capacity_origin_receiver(before_call: &str, cap: &str) -> Option<String> {
    let cap = compact_code(cap);
    if cap.is_empty() {
        return None;
    }

    let mut origin_receivers = Vec::new();
    for statement in before_call.split(';') {
        let Some((left, right)) = statement.split_once('=') else {
            continue;
        };
        let Some(binding) = let_binding_name(left) else {
            continue;
        };
        if right.contains("manuallydrop::new(") {
            origin_receivers.push(binding.to_string());
        }
        if binding == cap
            && let Some(receiver) = receiver_before_marker(right, ".capacity(")
            && origin_receivers.iter().any(|origin| origin == receiver)
        {
            return Some(receiver.to_string());
        }
    }
    None
}

fn box_from_raw_argument(compact_expression: &str) -> Option<&str> {
    let marker = "box::from_raw(";
    let call_pos = compact_expression.find(marker)? + marker.len();
    let argument_text = &compact_expression[call_pos..];
    let argument_end = matching_call_argument_end(argument_text)?;
    let argument = &argument_text[..argument_end];
    (!argument.is_empty()).then_some(argument)
}

fn has_set_len_initialization_evidence(lower: &str) -> bool {
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

fn has_set_len_call_result_initialization_evidence(lower: &str) -> bool {
    let compact = compact_code(lower);
    let Some(context) = set_len_call_context(&compact) else {
        return false;
    };
    context.has_call_result_initialization_evidence()
}

fn has_encode_utf8_remaining_capacity_evidence(lower: &str) -> bool {
    let compact = compact_code(lower);
    compact.contains("encode_utf8(c,ptr,remaining_cap)")
        && compact.contains("remaining_cap=self.capacity()-len")
        && compact.contains("ptr")
}

fn has_unchecked_constructor_availability_evidence(expression: &str, lower: &str) -> bool {
    let compact_expression = compact_code(&expression.to_ascii_lowercase());
    let Some(receiver) = unchecked_constructor_receiver(&compact_expression) else {
        return false;
    };
    let compact = compact_code(lower);
    let before_call = compact
        .find(&compact_expression)
        .map_or(compact.as_str(), |call_pos| &compact[..call_pos]);
    has_unchecked_constructor_availability_guard(before_call, receiver)
}

fn has_unchecked_constructor_availability_guard(before_call: &str, receiver: &str) -> bool {
    let predicate = format!("{receiver}::is_available()");
    has_unchecked_constructor_availability_assertion(before_call, &predicate)
        || has_open_unchecked_constructor_availability_branch(before_call, &predicate)
        || has_unchecked_constructor_unavailable_early_return(before_call, &predicate)
}

fn has_unchecked_constructor_availability_assertion(before_call: &str, predicate: &str) -> bool {
    [
        format!("assert!({predicate})"),
        format!("assert!({predicate},"),
        format!("debug_assert!({predicate})"),
        format!("debug_assert!({predicate},"),
    ]
    .iter()
    .any(|pattern| before_call.contains(pattern))
}

fn has_open_unchecked_constructor_availability_branch(before_call: &str, predicate: &str) -> bool {
    let guard = format!("if{predicate}{{");
    let mut search_from = 0;
    while let Some(offset) = before_call[search_from..].find(&guard) {
        let guard_start = search_from + offset;
        let after_guard = &before_call[guard_start + guard.len()..];
        if branch_still_open_at_operation(after_guard) {
            return true;
        }
        search_from = guard_start + guard.len();
    }
    false
}

fn has_unchecked_constructor_unavailable_early_return(before_call: &str, predicate: &str) -> bool {
    let guard = format!("if!{predicate}{{");
    let mut search_from = 0;
    while let Some(offset) = before_call[search_from..].find(&guard) {
        let guard_start = search_from + offset;
        let after_guard = &before_call[guard_start + guard.len()..];
        let guard_body = after_guard
            .split_once('}')
            .map_or(after_guard, |(guard_body, _after)| guard_body);
        if guard_body.contains("return") {
            return true;
        }
        search_from = guard_start + guard.len();
    }
    false
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

fn has_unwrap_unchecked_infallible_result_evidence(lower: &str) -> bool {
    let compact = compact_code(lower);
    let Some(context) = unwrap_unchecked_receiver_context(&compact) else {
        return false;
    };
    has_infallible_assignment_to_receiver(context)
}

fn has_infallible_assignment_to_receiver(context: ReceiverEvidenceContext<'_>) -> bool {
    let before_call = context.before_call;
    let receiver = context.receiver;
    let let_assignment = format!("let{receiver}=");
    let assignment = format!("{receiver}=");
    before_call.split(';').any(|statement| {
        statement.contains("fallibility::infallible")
            && (contains_receiver_fragment(statement, &let_assignment)
                || contains_receiver_fragment(statement, &assignment))
    })
}

fn has_unwrap_unchecked_receiver_state_evidence(lower: &str) -> bool {
    let compact = compact_code(&strip_block_comments_and_literals(lower));
    let Some(context) = unwrap_unchecked_receiver_context(&compact) else {
        return false;
    };

    has_receiver_positive_branch_guard(context, "is_some")
        || has_receiver_positive_branch_guard(context, "is_ok")
        || has_receiver_if_let_as_ref_guard(context, "some")
        || has_receiver_let_else_as_ref_guard(context, "some")
        || has_receiver_match_as_ref_guard(context, "some")
        || has_receiver_if_let_as_ref_guard(context, "ok")
        || has_receiver_let_else_as_ref_guard(context, "ok")
        || has_receiver_match_as_ref_guard(context, "ok")
        || has_receiver_early_return_guard(context, "is_none")
        || has_receiver_early_return_guard(context, "is_err")
}

#[derive(Clone, Copy)]
struct ReceiverEvidenceContext<'a> {
    before_call: &'a str,
    receiver: &'a str,
}

impl ReceiverEvidenceContext<'_> {
    fn has_assignment_after_branch(self, after_branch: &str) -> bool {
        is_simple_identifier(self.receiver)
            && has_assignment_to_identifier(after_branch, self.receiver)
    }
}

fn unwrap_unchecked_receiver_context(compact: &str) -> Option<ReceiverEvidenceContext<'_>> {
    let call_pos = compact.find(".unwrap_unchecked(")?;
    let before_call = &compact[..call_pos];
    let receiver_start = before_call
        .char_indices()
        .rev()
        .find_map(|(idx, ch)| (!is_receiver_path_char(ch)).then_some(idx + ch.len_utf8()))
        .unwrap_or(0);
    let receiver = &before_call[receiver_start..];
    (!receiver.is_empty()).then_some(ReceiverEvidenceContext {
        before_call,
        receiver,
    })
}

fn has_receiver_early_return_guard(context: ReceiverEvidenceContext<'_>, predicate: &str) -> bool {
    let before_call = context.before_call;
    let receiver = context.receiver;
    let guard = format!("if{receiver}.{predicate}(){{");
    let Some((_prefix, after_guard)) = before_call.split_once(&guard) else {
        return false;
    };
    let guard_returned = after_guard
        .split_once('}')
        .map_or(after_guard, |(guard_body, _after)| guard_body)
        .contains("return");
    guard_returned && !context.has_assignment_after_branch(after_guard)
}

fn has_receiver_positive_branch_guard(
    context: ReceiverEvidenceContext<'_>,
    predicate: &str,
) -> bool {
    let guard = format!("if{}.{predicate}(){{", context.receiver);
    has_open_receiver_branch_guard(context, &guard)
}

fn has_receiver_if_let_as_ref_guard(
    context: ReceiverEvidenceContext<'_>,
    constructor: &str,
) -> bool {
    let guard = format!("iflet{constructor}(_)={}.as_ref(){{", context.receiver);
    has_open_receiver_branch_guard(context, &guard)
}

fn has_receiver_let_else_as_ref_guard(
    context: ReceiverEvidenceContext<'_>,
    constructor: &str,
) -> bool {
    let before_call = context.before_call;
    let guard = format!("let{constructor}(_)={}.as_ref()else{{", context.receiver);
    let mut search_from = 0usize;
    while let Some(offset) = before_call[search_from..].find(&guard) {
        let guard_start = search_from + offset;
        let after_guard = &before_call[guard_start + guard.len()..];
        let (guard_body, after_guard_body) = matching_code_block_end(after_guard)
            .map_or((after_guard, ""), |body_end| {
                (&after_guard[..body_end], &after_guard[body_end + 1..])
            });
        if guard_body.contains("return") && !context.has_assignment_after_branch(after_guard_body) {
            return true;
        }
        search_from = guard_start + guard.len();
    }
    false
}

fn has_receiver_match_as_ref_guard(
    context: ReceiverEvidenceContext<'_>,
    constructor: &str,
) -> bool {
    let before_call = context.before_call;
    let marker = format!("match{}.as_ref(){{", context.receiver);
    let mut cursor = before_call;
    let mut offset = 0usize;
    while let Some(pos) = cursor.find(&marker) {
        let after_match_start = offset + pos + marker.len();
        let after_match = &before_call[after_match_start..];
        if let Some(branch_after_marker) =
            match_constructor_branch_after_marker(after_match, constructor)
            && branch_still_open_at_operation(branch_after_marker)
            && !context.has_assignment_after_branch(branch_after_marker)
        {
            return true;
        }
        let next = pos + marker.len();
        offset += next;
        cursor = &cursor[next..];
    }
    false
}

fn match_constructor_branch_after_marker<'a>(
    after_match: &'a str,
    constructor: &str,
) -> Option<&'a str> {
    let marker = format!("{constructor}(");
    let constructor_pos = after_match.find(&marker)?;
    let after_constructor = &after_match[constructor_pos + marker.len()..];
    let (binding, after_binding) = after_constructor.split_once(")=>{")?;
    is_some_binding(binding).then_some(after_binding)
}

fn has_open_receiver_branch_guard(context: ReceiverEvidenceContext<'_>, guard: &str) -> bool {
    let before_call = context.before_call;
    let mut search_from = 0;
    while let Some(offset) = before_call[search_from..].find(guard) {
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
        if depth > 0 && !context.has_assignment_after_branch(after_guard) {
            return true;
        }
        search_from = guard_start + guard.len();
    }
    false
}

fn has_unreachable_unchecked_infallible_path_evidence(lower: &str) -> bool {
    let compact = compact_code(lower);
    let Some(call_pos) = compact.find("unreachable_unchecked(") else {
        return false;
    };
    let before_call = &compact[..call_pos];
    let Some(match_pos) = before_call.rfind("match") else {
        return false;
    };
    let match_context = &before_call[match_pos..];
    let Some((match_head, after_open)) = match_context.split_once('{') else {
        return false;
    };
    if !match_head.contains("fallibility::infallible") {
        return false;
    }

    let mut depth = 1usize;
    for ch in after_open.chars() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

fn has_from_utf8_unchecked_validation_evidence(lower: &str) -> bool {
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

fn from_utf8_unchecked_argument_context(compact: &str) -> Option<(&str, &str)> {
    let marker = "from_utf8_unchecked(";
    let call_pos = compact.find(marker)?;
    let before_call = &compact[..call_pos];
    let after_marker = &compact[call_pos + marker.len()..];
    let argument_end = matching_call_argument_end(after_marker)?;
    let argument = &after_marker[..argument_end];
    (!argument.is_empty()).then_some((before_call, argument))
}

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

fn matching_call_argument_end(text: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (idx, ch) in text.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' if depth == 0 => return Some(idx),
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    None
}

fn matching_code_block_end(text_after_open: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (idx, ch) in text_after_open.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' if depth == 0 => return Some(idx),
            '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    None
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
        if binding.is_empty()
            || binding.contains('{')
            || !(path_prefix.is_empty() || path_prefix.ends_with("::"))
            || !path_prefix
                .chars()
                .all(|ch| is_receiver_path_char(ch) || ch == ':')
        {
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
        if binding.is_empty()
            || binding.contains('{')
            || !(path_prefix.is_empty() || path_prefix.ends_with("::"))
            || !path_prefix
                .chars()
                .all(|ch| is_receiver_path_char(ch) || ch == ':')
        {
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
        if else_body.contains("return") && !context.has_argument_assignment(after_else_body) {
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
        if binding.is_empty()
            || binding.contains('{')
            || !(path_prefix.is_empty() || path_prefix.ends_with("::"))
            || !path_prefix
                .chars()
                .all(|ch| is_receiver_path_char(ch) || ch == ':')
        {
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
        if guard_body.contains("return") && !context.has_argument_assignment(after_guard_body) {
            return true;
        }
        search_from = validation_start + context.validation.len();
    }
    false
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
        let guard_end = after_guard.find('}').unwrap_or(after_guard.len());
        let guard_body = &after_guard[..guard_end];
        let after_branch = &after_guard[guard_end..];
        if guard_body.contains("return") && !context.has_argument_assignment(after_branch) {
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
            && err_arm.contains("=>return")
            && !context.has_argument_assignment(after_block)
        {
            return true;
        }

        search_from = validation_pos + context.validation.len();
    }

    false
}

fn has_zeroed_known_valid_zero_type(lower: &str) -> bool {
    let compact = compact_code(lower);
    let Some(target_type) = zeroed_target_type(&compact) else {
        return false;
    };
    matches!(
        target_type,
        "()" | "bool"
            | "char"
            | "f32"
            | "f64"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "isize"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "usize"
    )
}

fn split_top_level_pair(text: &str) -> Option<(&str, &str)> {
    let mut angle_depth = 0usize;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    for (idx, ch) in text.char_indices() {
        match ch {
            '<' => angle_depth += 1,
            '>' => angle_depth = angle_depth.saturating_sub(1),
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            ',' if angle_depth == 0 && paren_depth == 0 && bracket_depth == 0 => {
                let left = &text[..idx];
                let right = &text[idx + 1..];
                return (!left.is_empty() && !right.is_empty()).then_some((left, right));
            }
            _ => {}
        }
    }
    None
}

fn has_u8_bool_value_guard(before_call: &str, argument: &str) -> bool {
    u8_bool_valid_value_predicates(argument)
        .iter()
        .any(|predicate| has_u8_bool_value_predicate_guard(before_call, predicate, argument))
        || has_u8_bool_invalid_early_return_guard(before_call, argument)
}

fn u8_bool_valid_value_predicates(target: &str) -> [String; 8] {
    [
        format!("{target}<=1"),
        format!("1>={target}"),
        format!("{target}<2"),
        format!("2>{target}"),
        format!("matches!({target},0|1)"),
        format!("matches!({target},1|0)"),
        format!("{target}==0||{target}==1"),
        format!("{target}==1||{target}==0"),
    ]
}

fn has_u8_bool_value_predicate_guard(before_call: &str, predicate: &str, argument: &str) -> bool {
    [
        format!("assert!({predicate})"),
        format!("assert!({predicate},"),
        format!("debug_assert!({predicate})"),
        format!("debug_assert!({predicate},"),
    ]
    .iter()
    .any(|pattern| has_fresh_guard_pattern(before_call, pattern, argument))
        || has_open_positive_branch_guard(before_call, predicate, argument)
}

fn has_fresh_guard_pattern(before_call: &str, pattern: &str, argument: &str) -> bool {
    has_fresh_guard_pattern_for_identifiers(before_call, pattern, &[argument])
}

fn has_fresh_guard_pattern_for_identifiers(
    before_call: &str,
    pattern: &str,
    identifiers: &[&str],
) -> bool {
    let mut search_from = 0;
    while let Some(offset) = before_call[search_from..].find(pattern) {
        let pattern_start = search_from + offset;
        let after_pattern = &before_call[pattern_start + pattern.len()..];
        let after_guard = if pattern.ends_with(';') {
            after_pattern
        } else {
            let statement_end = after_pattern.find(';').unwrap_or(after_pattern.len());
            &after_pattern[statement_end..]
        };
        if !has_assignment_to_any_identifier(after_guard, identifiers) {
            return true;
        }
        search_from = pattern_start + pattern.len();
    }
    false
}

fn has_open_positive_branch_guard(before_call: &str, predicate: &str, argument: &str) -> bool {
    has_open_positive_branch_guard_for_identifiers(before_call, predicate, &[argument])
}

fn has_open_positive_branch_guard_for_identifiers(
    before_call: &str,
    predicate: &str,
    identifiers: &[&str],
) -> bool {
    let guard = format!("if{predicate}{{");
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
        if depth > 0 && !has_assignment_to_any_identifier(after_guard, identifiers) {
            return true;
        }
        search_from = guard_start + guard.len();
    }
    false
}

fn has_assignment_to_any_identifier(compact: &str, identifiers: &[&str]) -> bool {
    identifiers
        .iter()
        .any(|identifier| has_assignment_to_identifier(compact, identifier))
}

fn has_u8_bool_invalid_early_return_guard(before_call: &str, argument: &str) -> bool {
    has_invalid_byte_returning_branch(before_call, &format!("{argument}>1"), argument)
        || has_invalid_byte_returning_branch(before_call, &format!("1<{argument}"), argument)
        || has_invalid_byte_returning_branch(before_call, &format!("{argument}>=2"), argument)
        || has_invalid_byte_returning_branch(before_call, &format!("2<={argument}"), argument)
}

fn has_invalid_byte_returning_branch(before_call: &str, predicate: &str, argument: &str) -> bool {
    let guard = format!("if{predicate}{{");
    let mut search_from = 0;
    while let Some(offset) = before_call[search_from..].find(&guard) {
        let guard_start = search_from + offset;
        let after_guard = &before_call[guard_start + guard.len()..];
        let guard_end = after_guard.find('}').unwrap_or(after_guard.len());
        let guard_body = &after_guard[..guard_end];
        let after_branch = &after_guard[guard_end..];
        if guard_body.contains("return") && !has_assignment_to_identifier(after_branch, argument) {
            return true;
        }
        search_from = guard_start + guard.len();
    }
    false
}

fn is_simple_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn source_value_identifier(argument: &str) -> Option<&str> {
    if is_simple_identifier(argument) {
        return Some(argument);
    }
    let referenced = argument.strip_prefix('&')?;
    is_simple_identifier(referenced).then_some(referenced)
}

fn has_assignment_to_identifier(compact: &str, identifier: &str) -> bool {
    let mut cursor = compact;
    let mut offset = 0usize;
    while let Some(pos) = cursor.find(identifier) {
        let start = offset + pos;
        let before = compact[..start].chars().next_back();
        let after_start = start + identifier.len();
        let after = &compact[after_start..];
        let ends_on_boundary = after
            .chars()
            .next()
            .is_none_or(|ch| !is_receiver_path_char(ch));
        if before.is_none_or(|ch| !is_receiver_path_char(ch))
            && ends_on_boundary
            && starts_assignment_operator(after)
        {
            return true;
        }
        let next = pos + identifier.len();
        offset += next;
        cursor = &cursor[next..];
    }
    false
}

fn starts_assignment_operator(after_identifier: &str) -> bool {
    if after_identifier.starts_with("==") || after_identifier.starts_with("=>") {
        return false;
    }
    after_identifier.starts_with('=')
        || ["+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "<<=", ">>="]
            .iter()
            .any(|operator| after_identifier.starts_with(operator))
}

fn zeroed_target_type(compact: &str) -> Option<&str> {
    let marker = "zeroed::<";
    let start = compact.find(marker)? + marker.len();
    let after_marker = &compact[start..];
    let end = matching_generic_argument_end(after_marker)?;
    let target_type = &after_marker[..end];
    (!target_type.is_empty()).then_some(target_type)
}

fn matching_generic_argument_end(text: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (idx, ch) in text.char_indices() {
        match ch {
            '<' => depth += 1,
            '>' if depth == 0 => return Some(idx),
            '>' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    None
}

fn has_maybeuninit_slice_context(lower: &str) -> bool {
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

fn has_maybeuninit_raw_write_context(site: &ScannedSite, lower: &str) -> bool {
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

fn has_u8_write_bytes_context(site: &ScannedSite, lower: &str) -> bool {
    let compact_expression = compact_code(&site.operation.expression.to_ascii_lowercase());
    let Some((_before_call, receiver, _byte, _len)) =
        write_bytes_method_context(&compact_expression)
    else {
        return false;
    };

    pointer_binding_has_type_before_operation(lower, &site.operation.expression, receiver, "*mutu8")
}

fn has_bool_write_bytes_pointer_context(site: &ScannedSite, lower: &str) -> bool {
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

fn has_bool_write_bytes_value_evidence(site: &ScannedSite, lower: &str) -> bool {
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

fn has_write_bytes_bounds_evidence(lower: &str) -> bool {
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

fn has_set_len_shrink_evidence(lower: &str) -> bool {
    set_len_shrink::has_set_len_shrink_evidence(lower)
}

fn compact_code(lower: &str) -> String {
    lower
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace())
        .collect()
}

fn has_alignment_guard(site: &ScannedSite, lower: &str) -> bool {
    let compact = compact_code(lower);
    if let Some(receiver) = raw_pointer_alignment_receiver(&site.operation.expression) {
        let guard_scope = code_before_operation(lower, &site.operation.expression)
            .unwrap_or_else(|| lower.to_string());
        let guard_compact = compact_code(&guard_scope);
        return has_same_receiver_alignment_guard(&guard_compact, &receiver);
    }
    lower.contains("is_aligned")
        || lower.contains("align_offset")
        || lower.contains("addr() %")
        || lower.contains("as usize %")
        || compact.contains("addr()%")
        || compact.contains("asusize)%")
        || compact.contains("asusize%")
}

fn has_same_receiver_alignment_guard(compact: &str, receiver: &str) -> bool {
    let receiver = compact_code(&receiver.to_ascii_lowercase());
    has_same_receiver_alignment_condition_guard(compact, &receiver)
}

fn has_same_receiver_alignment_condition_guard(compact: &str, receiver: &str) -> bool {
    has_alignment_assertion_guard(compact, receiver)
        || has_alignment_open_positive_branch_guard(compact, receiver)
        || has_alignment_early_return_guard(compact, receiver)
}

fn has_alignment_assertion_guard(compact: &str, receiver: &str) -> bool {
    ["assert!(", "debug_assert!("].into_iter().any(|prefix| {
        let mut cursor = compact;
        let mut offset = 0usize;
        while let Some(pos) = cursor.find(prefix) {
            let statement_start = offset + pos + prefix.len();
            let after_prefix = &compact[statement_start..];
            let statement_end = after_prefix.find(';').unwrap_or(after_prefix.len());
            let statement = &after_prefix[..statement_end];
            let after_statement = &after_prefix[statement_end..];
            if alignment_condition_is_positive(statement, receiver)
                && !contains_simple_assignment_to(after_statement, receiver)
            {
                return true;
            }
            let next = pos + prefix.len();
            offset += next;
            cursor = &cursor[next..];
        }
        false
    })
}

fn has_alignment_open_positive_branch_guard(compact: &str, receiver: &str) -> bool {
    compact_if_guards(compact).any(|guard| {
        alignment_condition_is_positive(guard.condition, receiver)
            && branch_still_open_at_operation(guard.after_body_start)
            && !contains_simple_assignment_to(guard.after_body_start, receiver)
    })
}

fn has_alignment_early_return_guard(compact: &str, receiver: &str) -> bool {
    compact_if_guards(compact).any(|guard| {
        if !alignment_condition_is_negative(guard.condition, receiver) {
            return false;
        }
        let (guard_body, after_guard_body) = guard
            .after_body_start
            .split_once('}')
            .map_or((guard.after_body_start, ""), |(guard_body, after)| {
                (guard_body, after)
            });
        guard_body.contains("return") && !contains_simple_assignment_to(after_guard_body, receiver)
    })
}

struct CompactIfGuard<'a> {
    condition: &'a str,
    after_body_start: &'a str,
}

fn compact_if_guards(compact: &str) -> impl Iterator<Item = CompactIfGuard<'_>> {
    let mut guards = Vec::new();
    let mut cursor = compact;
    let mut offset = 0usize;
    while let Some(pos) = cursor.find("if") {
        let start = offset + pos;
        let before = compact[..start].chars().next_back();
        if before.is_some_and(is_receiver_path_char) {
            let next = pos + 2;
            offset += next;
            cursor = &cursor[next..];
            continue;
        }
        let after_if = &compact[start + 2..];
        if let Some(brace_pos) = after_if.find('{') {
            guards.push(CompactIfGuard {
                condition: &after_if[..brace_pos],
                after_body_start: &after_if[brace_pos + 1..],
            });
        }
        let next = pos + 2;
        offset += next;
        cursor = &cursor[next..];
    }
    guards.into_iter()
}

fn alignment_condition_is_positive(condition: &str, receiver: &str) -> bool {
    if same_receiver_method_call(condition, receiver, "is_aligned") {
        return !condition.starts_with('!')
            && !condition.contains(".is_aligned()==false")
            && !condition.contains(".is_aligned()!=true");
    }
    (same_receiver_method_call(condition, receiver, "align_offset")
        || same_receiver_alignment_modulo(condition, receiver))
        && condition.contains("==0")
}

fn alignment_condition_is_negative(condition: &str, receiver: &str) -> bool {
    if same_receiver_method_call(condition, receiver, "is_aligned") {
        return condition.starts_with('!')
            || condition.contains(".is_aligned()==false")
            || condition.contains(".is_aligned()!=true");
    }
    (same_receiver_method_call(condition, receiver, "align_offset")
        || same_receiver_alignment_modulo(condition, receiver))
        && condition.contains("!=0")
}

fn same_receiver_alignment_modulo(compact: &str, receiver: &str) -> bool {
    contains_receiver_fragment(compact, &format!("{receiver}.addr()%"))
        || contains_receiver_fragment(compact, &format!("{receiver}asusize)%"))
        || contains_receiver_fragment(compact, &format!("{receiver}asusize%"))
        || contains_receiver_fragment(compact, &format!("({receiver}asusize)%"))
        || contains_receiver_fragment(compact, &format!("({receiver}asusize%"))
}

fn same_receiver_method_call(compact: &str, receiver: &str, method: &str) -> bool {
    let direct = format!("{receiver}.{method}");
    if contains_receiver_fragment(compact, &direct) {
        return true;
    }
    let cast_prefix = format!("{receiver}.cast");
    let mut cursor = compact;
    let mut offset = 0usize;
    while let Some(pos) = cursor.find(&cast_prefix) {
        let start = offset + pos;
        let before = compact[..start].chars().next_back();
        let starts_on_boundary = before.is_none_or(|ch| !is_receiver_path_char(ch));
        let after_receiver = &compact[start + receiver.len()..];
        let end = after_receiver
            .find([';', '{', '}'])
            .unwrap_or(after_receiver.len());
        if starts_on_boundary && after_receiver[..end].contains(&format!(".{method}")) {
            return true;
        }
        let next = pos + cast_prefix.len();
        offset += next;
        cursor = &cursor[next..];
    }
    false
}

fn contains_receiver_fragment(compact: &str, fragment: &str) -> bool {
    let mut cursor = compact;
    let mut offset = 0usize;
    while let Some(pos) = cursor.find(fragment) {
        let start = offset + pos;
        let before = compact[..start].chars().next_back();
        if before.is_none_or(|ch| !is_receiver_path_char(ch)) {
            return true;
        }
        let next = pos + fragment.len();
        offset += next;
        cursor = &cursor[next..];
    }
    false
}

fn contains_receiver_path(compact: &str, receiver: &str) -> bool {
    let mut cursor = compact;
    let mut offset = 0usize;
    while let Some(pos) = cursor.find(receiver) {
        let start = offset + pos;
        let end = start + receiver.len();
        let before = compact[..start].chars().next_back();
        let after = compact[end..].chars().next();
        if before.is_none_or(|ch| !is_receiver_path_char(ch))
            && after.is_none_or(|ch| ch == '.' || !is_receiver_path_char(ch))
        {
            return true;
        }
        let next = pos + receiver.len();
        offset += next;
        cursor = &cursor[next..];
    }
    false
}

fn raw_pointer_alignment_receiver(expression: &str) -> Option<String> {
    let compact = compact_code(&expression.to_ascii_lowercase());
    if let Some(receiver) = receiver_before_marker(&compact, ".cast::<") {
        return Some(receiver.to_string());
    }
    if let Some(receiver) = receiver_before_marker(&compact, ".read(") {
        return Some(receiver.to_string());
    }
    if let Some(receiver) = receiver_before_marker(&compact, ".read_volatile(") {
        return Some(receiver.to_string());
    }
    if let Some(receiver) = receiver_before_marker(&compact, ".write(") {
        return Some(receiver.to_string());
    }
    if let Some(receiver) = receiver_before_marker(&compact, ".write_volatile(") {
        return Some(receiver.to_string());
    }
    None
}

fn receiver_before_marker<'a>(compact: &'a str, marker: &str) -> Option<&'a str> {
    let pos = compact.find(marker)?;
    let before_marker = &compact[..pos];
    let receiver_start = before_marker
        .char_indices()
        .rev()
        .find_map(|(idx, ch)| (!is_receiver_path_char(ch)).then_some(idx + ch.len_utf8()))
        .unwrap_or(0);
    let receiver = &before_marker[receiver_start..];
    (!receiver.is_empty()).then_some(receiver)
}

fn match_some_branch_after_marker(after_match: &str) -> Option<&str> {
    let some_pos = after_match.find("some(")?;
    let after_some = &after_match[some_pos + "some(".len()..];
    let (binding, after_binding) = after_some.split_once(")=>{")?;
    is_some_binding(binding).then_some(after_binding)
}

fn ends_with_some_pattern(before_marker: &str, keyword: &str) -> bool {
    let prefix = format!("{keyword}some(");
    let Some(pattern_start) = before_marker.rfind(&prefix) else {
        return false;
    };
    let binding_with_close = &before_marker[pattern_start + prefix.len()..];
    let Some(binding) = binding_with_close.strip_suffix(')') else {
        return false;
    };
    is_some_binding(binding)
}

fn is_some_binding(binding: &str) -> bool {
    !binding.is_empty()
        && (binding == "_"
            || binding
                .chars()
                .all(|ch| ch == '_' || ch.is_ascii_alphanumeric()))
}

fn contains_word(text: &str, word: &str) -> bool {
    text.split(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
        .any(|token| token == word)
}

fn is_receiver_path_char(ch: char) -> bool {
    ch == '_' || ch == ':' || ch == '.' || ch.is_ascii_alphanumeric()
}

pub(crate) fn reach_evidence(
    root: &Path,
    owner: Option<&String>,
) -> (ReachEvidence, Vec<RelatedTest>) {
    let Some(owner) = owner else {
        return (
            ReachEvidence {
                state: "unknown".to_string(),
                summary: "No owner function could be inferred".to_string(),
            },
            Vec::new(),
        );
    };
    let mut tests = Vec::new();
    let test_files = collect_test_files(root).unwrap_or_default();
    for rel in test_files {
        let abs = root.join(&rel);
        let Ok(text) = fs::read_to_string(&abs) else {
            continue;
        };
        if !text.contains(owner) {
            continue;
        }
        let mut last_test: Option<(String, usize)> = None;
        for (idx, line) in text.lines().enumerate() {
            if line.contains("#[test]") {
                last_test = Some(("test".to_string(), idx + 1));
            }
            if let Some(name) = parse_test_name(line) {
                last_test = Some((name, idx + 1));
            }
            if line.contains(owner) {
                let (name, line_no) = last_test
                    .clone()
                    .unwrap_or_else(|| (format!("mentions {owner}"), idx + 1));
                tests.push(RelatedTest {
                    name,
                    file: rel.to_string_lossy().replace('\\', "/"),
                    line: line_no,
                });
                break;
            }
        }
    }
    if tests.is_empty() {
        (
            ReachEvidence {
                state: "unreached".to_string(),
                summary: format!("No static test mention of owner `{owner}` was found"),
            },
            tests,
        )
    } else {
        (
            ReachEvidence {
                state: "owner_reached".to_string(),
                summary: format!(
                    "{} related test file(s) mention owner `{owner}`",
                    tests.len()
                ),
            },
            tests,
        )
    }
}

fn parse_test_name(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !(trimmed.starts_with("fn ") || trimmed.starts_with("pub fn ")) {
        return None;
    }
    let pos = trimmed.find("fn ")?;
    let rest = &trimmed[pos + 3..];
    let mut name = String::new();
    for ch in rest.chars() {
        if ch == '_' || ch.is_ascii_alphanumeric() {
            name.push(ch);
        } else {
            break;
        }
    }
    (!name.is_empty()).then_some(name)
}

fn collect_test_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    visit(root, root, &mut out)?;
    out.sort();
    Ok(out)
}

fn visit(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries =
        fs::read_dir(dir).map_err(|err| format!("read {} failed: {err}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|err| format!("read_dir entry failed: {err}"))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if matches!(
                name.as_str(),
                ".git" | "target" | ".unsafe-review" | ".unsafe-review-spec" | "node_modules"
            ) {
                continue;
            }
            visit(root, &path, out)?;
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            let rel_text = rel.to_string_lossy();
            if rel_text.contains("tests")
                || rel_text.contains("test")
                || fs::read_to_string(&path).is_ok_and(|text| text.contains("#[test]"))
            {
                out.push(rel);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        OperationFamily, SourceLocation, UnsafeOperation, UnsafeSite, UnsafeSiteKind,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    fn site_with_context(
        context_before: Vec<&str>,
        snippet: &str,
        context_after: Vec<&str>,
    ) -> ScannedSite {
        site_with_family(
            OperationFamily::RawPointerRead,
            context_before,
            snippet,
            context_after,
        )
    }

    fn site_with_family(
        family: OperationFamily,
        context_before: Vec<&str>,
        snippet: &str,
        context_after: Vec<&str>,
    ) -> ScannedSite {
        ScannedSite {
            site: UnsafeSite {
                location: SourceLocation::new(PathBuf::from("src/lib.rs"), 1, 1),
                kind: UnsafeSiteKind::Operation,
                owner: Some("read_one".to_string()),
                visibility: "private".to_string(),
                public_api_surface: false,
                changed: true,
                snippet: snippet.to_string(),
            },
            operation: UnsafeOperation {
                family,
                expression: snippet.to_string(),
            },
            context_before: context_before.into_iter().map(str::to_string).collect(),
            context_after: context_after.into_iter().map(str::to_string).collect(),
        }
    }

    #[test]
    fn contract_evidence_accepts_safety_docs_and_safety_comments() {
        let doc_site = site_with_context(
            vec!["/// # Safety", "/// pointer must be valid"],
            "ptr.read()",
            vec![],
        );
        let safety_colon_doc_site = site_with_context(
            vec!["/// Safety: pointer must be valid"],
            "ptr.read()",
            vec![],
        );
        let comment_site = site_with_context(
            vec!["// SAFETY: caller checked pointer"],
            "ptr.read()",
            vec![],
        );
        let safety_colon_comment_site = site_with_context(
            vec!["// Safety: caller checked pointer"],
            "ptr.read()",
            vec![],
        );
        let missing_site = site_with_context(vec!["// ordinary comment"], "ptr.read()", vec![]);

        assert!(contract_evidence(&doc_site).present);
        assert!(contract_evidence(&safety_colon_doc_site).present);
        assert!(contract_evidence(&comment_site).present);
        assert!(contract_evidence(&safety_colon_comment_site).present);
        assert!(!contract_evidence(&missing_site).present);
    }

    #[test]
    fn obligation_evidence_ignores_guards_that_only_appear_in_comments() {
        let obligations = vec![SafetyObligation::new(
            "alignment",
            "pointer is aligned for the accessed type",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let misleading_comment = site_with_context(
            vec!["// SAFETY: checked elsewhere"],
            "ptr.read() // align_of::<u32>() proves this",
            vec![],
        );
        let local_guard = site_with_context(
            vec!["if (ptr as usize) % std::mem::align_of::<u32>() != 0 { return None; }"],
            "ptr.read()",
            vec![],
        );

        assert!(
            !obligation_evidence(&misleading_comment, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            obligation_evidence(&local_guard, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
    }

    #[test]
    fn bare_align_of_does_not_discharge_alignment() {
        let obligations = vec![SafetyObligation::new(
            "alignment",
            "pointer is aligned for the accessed type",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let align_of_only = site_with_context(
            vec!["let _required = core::mem::align_of::<Header>();"],
            "ptr.read()",
            vec![],
        );
        let modulo_guard = site_with_context(
            vec!["if (ptr as usize) % core::mem::align_of::<Header>() != 0 { return None; }"],
            "ptr.read()",
            vec![],
        );

        assert!(
            !obligation_evidence(&align_of_only, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            obligation_evidence(&modulo_guard, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
    }

    #[test]
    fn alignment_guard_must_match_raw_pointer_receiver_when_known() {
        let obligations = vec![SafetyObligation::new(
            "alignment",
            "pointer is aligned for the accessed type",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let other_pointer_guard = site_with_context(
            vec!["if (other_ptr as usize) % core::mem::align_of::<Header>() != 0 { return None; }"],
            "ptr.cast::<Header>().read()",
            vec![],
        );
        let matching_modulo_guard = site_with_context(
            vec!["if (ptr as usize) % core::mem::align_of::<Header>() != 0 { return None; }"],
            "ptr.cast::<Header>().read()",
            vec![],
        );
        let matching_modulo_assertion = site_with_context(
            vec!["assert!((ptr as usize) % core::mem::align_of::<Header>() == 0);"],
            "ptr.cast::<Header>().read()",
            vec![],
        );
        let observed_modulo = site_with_context(
            vec![
                "let aligned = (ptr as usize) % core::mem::align_of::<Header>() == 0;",
                "observe(aligned);",
            ],
            "ptr.cast::<Header>().read()",
            vec![],
        );
        let closed_modulo_branch = site_with_context(
            vec![
                "if (ptr as usize) % core::mem::align_of::<Header>() == 0 {",
                "    observe(ptr);",
                "}",
            ],
            "ptr.cast::<Header>().read()",
            vec![],
        );
        let reassigned_modulo_pointer = site_with_context(
            vec![
                "if (ptr as usize) % core::mem::align_of::<Header>() != 0 { return None; }",
                "ptr = other_ptr;",
            ],
            "ptr.cast::<Header>().read()",
            vec![],
        );
        let matching_method_guard = site_with_context(
            vec!["if !ptr.cast::<Header>().is_aligned() { return None; }"],
            "ptr.cast::<Header>().read()",
            vec![],
        );
        let matching_method_assertion = site_with_context(
            vec!["assert!(ptr.cast::<Header>().is_aligned());"],
            "ptr.cast::<Header>().read()",
            vec![],
        );
        let matching_open_branch_guard = site_with_context(
            vec!["if ptr.cast::<Header>().is_aligned() {"],
            "ptr.cast::<Header>().read()",
            vec!["}"],
        );
        let observed_method = site_with_context(
            vec![
                "let aligned = ptr.cast::<Header>().is_aligned();",
                "observe(aligned);",
            ],
            "ptr.cast::<Header>().read()",
            vec![],
        );
        let closed_positive_branch = site_with_context(
            vec![
                "if ptr.cast::<Header>().is_aligned() {",
                "    observe(ptr);",
                "}",
            ],
            "ptr.cast::<Header>().read()",
            vec![],
        );
        let reassigned_pointer = site_with_context(
            vec![
                "if !ptr.cast::<Header>().is_aligned() { return None; }",
                "ptr = other_ptr;",
            ],
            "ptr.cast::<Header>().read()",
            vec![],
        );
        let post_guard = site_with_context(
            vec![],
            "ptr.cast::<Header>().read()",
            vec!["if !ptr.cast::<Header>().is_aligned() { return None; }"],
        );

        assert!(
            !obligation_evidence(&other_pointer_guard, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            obligation_evidence(&matching_modulo_guard, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            obligation_evidence(&matching_modulo_assertion, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            obligation_evidence(&matching_method_guard, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            obligation_evidence(&matching_method_assertion, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            obligation_evidence(&matching_open_branch_guard, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        for stale in [
            observed_modulo,
            closed_modulo_branch,
            reassigned_modulo_pointer,
            observed_method,
            closed_positive_branch,
            reassigned_pointer,
            post_guard,
        ] {
            assert!(
                !obligation_evidence(&stale, &obligations, &contract, &reach)[0]
                    .discharge
                    .present
            );
        }
    }

    #[test]
    fn nonnull_constructor_name_does_not_discharge_nullability() {
        let obligations = vec![SafetyObligation::new(
            "non-null",
            "pointer is non-null before constructing NonNull",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let constructor_only = site_with_family(
            OperationFamily::NonNullUnchecked,
            vec![],
            "NonNull::new_unchecked(ptr)",
            vec![],
        );
        let explicit_guard = site_with_family(
            OperationFamily::NonNullUnchecked,
            vec!["if ptr.is_null() { return None; }"],
            "NonNull::new_unchecked(ptr)",
            vec![],
        );

        assert!(
            !obligation_evidence(&constructor_only, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            obligation_evidence(&explicit_guard, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
    }

    #[test]
    fn nonnull_guard_must_match_new_unchecked_argument() {
        let obligations = vec![SafetyObligation::new(
            "non-null",
            "pointer is non-null before constructing NonNull",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let matching_guard = site_with_family(
            OperationFamily::NonNullUnchecked,
            vec!["NonNull::new(ptr)?;"],
            "NonNull::new_unchecked(ptr)",
            vec![],
        );
        let stale_question_mark_guard = site_with_family(
            OperationFamily::NonNullUnchecked,
            vec!["NonNull::new(ptr)?;", "ptr = other;"],
            "NonNull::new_unchecked(ptr)",
            vec![],
        );
        let if_let_guard = site_with_family(
            OperationFamily::NonNullUnchecked,
            vec!["if let Some(_) = NonNull::new(ptr) {"],
            "NonNull::new_unchecked(ptr)",
            vec!["}"],
        );
        let let_else_guard = site_with_family(
            OperationFamily::NonNullUnchecked,
            vec!["let Some(_) = NonNull::new(ptr) else { return None; };"],
            "NonNull::new_unchecked(ptr)",
            vec![],
        );
        let match_guard = site_with_family(
            OperationFamily::NonNullUnchecked,
            vec!["match NonNull::new(ptr) {", "Some(_) => {"],
            "NonNull::new_unchecked(ptr)",
            vec!["}", "None => None,", "}"],
        );
        let other_guard = site_with_family(
            OperationFamily::NonNullUnchecked,
            vec!["NonNull::new(other)?;"],
            "NonNull::new_unchecked(ptr)",
            vec![],
        );
        let stale_if_let_guard = site_with_family(
            OperationFamily::NonNullUnchecked,
            vec!["if let Some(_) = NonNull::new(ptr) {", "ptr = other;"],
            "NonNull::new_unchecked(ptr)",
            vec!["}"],
        );
        let stale_let_else_guard = site_with_family(
            OperationFamily::NonNullUnchecked,
            vec![
                "let Some(_) = NonNull::new(ptr) else { return None; };",
                "ptr = other;",
            ],
            "NonNull::new_unchecked(ptr)",
            vec![],
        );
        let stale_match_guard = site_with_family(
            OperationFamily::NonNullUnchecked,
            vec!["match NonNull::new(ptr) {", "Some(_) => {", "ptr = other;"],
            "NonNull::new_unchecked(ptr)",
            vec!["}", "None => None,", "}"],
        );
        let method_guard = site_with_family(
            OperationFamily::NonNullUnchecked,
            vec!["if bucket.as_ptr().is_null() { return None; }"],
            "NonNull::new_unchecked(bucket.as_ptr())",
            vec![],
        );
        let post_guard = site_with_family(
            OperationFamily::NonNullUnchecked,
            vec![],
            "NonNull::new_unchecked(ptr)",
            vec!["NonNull::new(ptr)?;"],
        );

        assert!(
            obligation_evidence(&matching_guard, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&stale_question_mark_guard, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            obligation_evidence(&if_let_guard, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            obligation_evidence(&let_else_guard, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            obligation_evidence(&match_guard, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&other_guard, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&stale_if_let_guard, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&stale_let_else_guard, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&stale_match_guard, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            obligation_evidence(&method_guard, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&post_guard, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
    }

    #[test]
    fn nonnull_is_null_guard_must_exit_before_unchecked_constructor() {
        let obligations = vec![SafetyObligation::new(
            "non-null",
            "pointer is non-null before constructing NonNull",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let non_returning_guard = site_with_family(
            OperationFamily::NonNullUnchecked,
            vec!["if ptr.is_null() { log_null(); }"],
            "NonNull::new_unchecked(ptr)",
            vec![],
        );
        let returning_guard = site_with_family(
            OperationFamily::NonNullUnchecked,
            vec!["if ptr.is_null() { return None; }"],
            "NonNull::new_unchecked(ptr)",
            vec![],
        );
        let stale_returning_guard = site_with_family(
            OperationFamily::NonNullUnchecked,
            vec!["if ptr.is_null() { return None; }", "ptr = other;"],
            "NonNull::new_unchecked(ptr)",
            vec![],
        );
        let post_returning_guard = site_with_family(
            OperationFamily::NonNullUnchecked,
            vec![],
            "NonNull::new_unchecked(ptr)",
            vec!["if ptr.is_null() { return None; }"],
        );

        assert!(
            !obligation_evidence(&non_returning_guard, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            obligation_evidence(&returning_guard, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&stale_returning_guard, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&post_returning_guard, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
    }

    #[test]
    fn slice_end_pointer_arithmetic_discharges_bounds() {
        let obligations = vec![SafetyObligation::new(
            "bounds",
            "pointer arithmetic stays in-bounds or one-past inside the same allocation",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let slice_end = site_with_family(
            OperationFamily::PointerArithmetic,
            vec!["let start = haystack.as_ptr();"],
            "let end = start.add(haystack.len());",
            vec![],
        );
        let mismatched_len = site_with_family(
            OperationFamily::PointerArithmetic,
            vec!["let start = needle.as_ptr();"],
            "let end = start.add(haystack.len());",
            vec![],
        );

        assert!(
            obligation_evidence(&slice_end, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&mismatched_len, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
    }

    #[test]
    fn documented_target_feature_declaration_does_not_require_local_guard() {
        let obligations = vec![SafetyObligation::new(
            "target-feature",
            "callers only execute this path on supported hardware",
        )];
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let site = site_with_family(
            OperationFamily::TargetFeature,
            vec!["/// # Safety"],
            "#[target_feature(enable = \"sse2\")]",
            vec!["pub unsafe fn find_raw() {}"],
        );

        let with_contract = obligation_evidence(
            &site,
            &obligations,
            &ContractEvidence::present("contract"),
            &reach,
        );
        let without_contract =
            obligation_evidence(&site, &obligations, &ContractEvidence::missing(), &reach);

        assert!(with_contract[0].discharge.present);
        assert!(!without_contract[0].discharge.present);
    }

    #[test]
    fn maybeuninit_assume_init_accepts_same_slot_initialization_evidence() {
        let obligations = vec![SafetyObligation::new(
            "initialized",
            "all fields/elements are initialized and valid before `assume_init`",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let write_before_assume = site_with_family(
            OperationFamily::MaybeUninitAssumeInit,
            vec![
                "let mut slot = MaybeUninit::<u32>::uninit();",
                "slot.write(7);",
            ],
            "unsafe { slot.assume_init() }",
            vec![],
        );
        let new_before_assume = site_with_family(
            OperationFamily::MaybeUninitAssumeInit,
            vec!["let slot = MaybeUninit::new(7_u32);"],
            "unsafe { slot.assume_init() }",
            vec![],
        );
        let typed_new_before_assume = site_with_family(
            OperationFamily::MaybeUninitAssumeInit,
            vec!["let mut slot: MaybeUninit<u32> = MaybeUninit::<u32>::new(7);"],
            "unsafe { slot.assume_init_read() }",
            vec![],
        );

        let write_evidence =
            obligation_evidence(&write_before_assume, &obligations, &contract, &reach);
        let new_evidence = obligation_evidence(&new_before_assume, &obligations, &contract, &reach);
        let typed_new_evidence =
            obligation_evidence(&typed_new_before_assume, &obligations, &contract, &reach);

        assert!(write_evidence[0].discharge.present);
        assert!(new_evidence[0].discharge.present);
        assert!(typed_new_evidence[0].discharge.present);
    }

    #[test]
    fn maybeuninit_assume_init_rejects_conditional_other_and_stale_initialization() {
        let obligations = vec![SafetyObligation::new(
            "initialized",
            "all fields/elements are initialized and valid before `assume_init`",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let closed_branch_write = site_with_family(
            OperationFamily::MaybeUninitAssumeInit,
            vec![
                "let mut slot = MaybeUninit::<u32>::uninit();",
                "if init {",
                "    slot.write(7);",
                "}",
            ],
            "unsafe { slot.assume_init() }",
            vec![],
        );
        let other_slot_write = site_with_family(
            OperationFamily::MaybeUninitAssumeInit,
            vec![
                "let mut slot = MaybeUninit::<u32>::uninit();",
                "let mut other = MaybeUninit::<u32>::uninit();",
                "other.write(7);",
            ],
            "unsafe { slot.assume_init() }",
            vec![],
        );
        let stale_slot = site_with_family(
            OperationFamily::MaybeUninitAssumeInit,
            vec![
                "let mut slot = MaybeUninit::<u32>::uninit();",
                "slot.write(7);",
                "slot = MaybeUninit::uninit();",
            ],
            "unsafe { slot.assume_init() }",
            vec![],
        );
        let stale_new_slot = site_with_family(
            OperationFamily::MaybeUninitAssumeInit,
            vec![
                "let mut slot = MaybeUninit::new(7_u32);",
                "slot = MaybeUninit::uninit();",
            ],
            "unsafe { slot.assume_init() }",
            vec![],
        );
        let open_branch_write = site_with_family(
            OperationFamily::MaybeUninitAssumeInit,
            vec![
                "let mut slot = MaybeUninit::<u32>::uninit();",
                "if init {",
                "    slot.write(7);",
            ],
            "unsafe { slot.assume_init() }",
            vec!["}"],
        );

        let closed_evidence =
            obligation_evidence(&closed_branch_write, &obligations, &contract, &reach);
        let other_evidence =
            obligation_evidence(&other_slot_write, &obligations, &contract, &reach);
        let stale_evidence = obligation_evidence(&stale_slot, &obligations, &contract, &reach);
        let stale_new_evidence =
            obligation_evidence(&stale_new_slot, &obligations, &contract, &reach);
        let open_evidence =
            obligation_evidence(&open_branch_write, &obligations, &contract, &reach);

        assert!(!closed_evidence[0].discharge.present);
        assert!(!other_evidence[0].discharge.present);
        assert!(!stale_evidence[0].discharge.present);
        assert!(!stale_new_evidence[0].discharge.present);
        assert!(open_evidence[0].discharge.present);
    }

    #[test]
    fn set_len_initialization_evidence_is_operation_specific() {
        let obligations = vec![SafetyObligation::new(
            "initialized",
            "elements in the extended range are initialized",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let context = vec![
            "for (dst, src) in out.xs.iter_mut().zip(bytes.iter()) {",
            "*dst = MaybeUninit::new(*src);",
            "}",
        ];
        let unrelated_context = vec!["let _other = MaybeUninit::new(0_u8);"];
        let prefixed_receiver_context = vec!["let other_out = MaybeUninit::new(0_u8);"];
        let single_index_context = vec![
            "if new_len > out.capacity() { return; }",
            "out.spare_capacity_mut()[0].write(0_u8);",
        ];
        let set_len = site_with_family(
            OperationFamily::VecSetLen,
            context.clone(),
            "out.set_len(CAP);",
            vec![],
        );
        let unrelated_set_len = site_with_family(
            OperationFamily::VecSetLen,
            unrelated_context,
            "out.set_len(CAP);",
            vec![],
        );
        let prefixed_receiver_set_len = site_with_family(
            OperationFamily::VecSetLen,
            prefixed_receiver_context,
            "out.set_len(CAP);",
            vec![],
        );
        let single_index_set_len = site_with_family(
            OperationFamily::VecSetLen,
            single_index_context,
            "out.set_len(new_len);",
            vec![],
        );
        let raw_read = site_with_family(
            OperationFamily::RawPointerRead,
            context,
            "ptr.read()",
            vec![],
        );

        let set_len_evidence = obligation_evidence(&set_len, &obligations, &contract, &reach);
        let unrelated_set_len_evidence =
            obligation_evidence(&unrelated_set_len, &obligations, &contract, &reach);
        let prefixed_receiver_set_len_evidence =
            obligation_evidence(&prefixed_receiver_set_len, &obligations, &contract, &reach);
        let single_index_set_len_evidence =
            obligation_evidence(&single_index_set_len, &obligations, &contract, &reach);
        let raw_read_evidence = obligation_evidence(&raw_read, &obligations, &contract, &reach);

        assert!(set_len_evidence[0].discharge.present);
        assert!(!unrelated_set_len_evidence[0].discharge.present);
        assert!(!prefixed_receiver_set_len_evidence[0].discharge.present);
        assert!(!single_index_set_len_evidence[0].discharge.present);
        assert_eq!(
            raw_read_evidence[0].discharge.summary,
            "No obligation-specific guard code was detected"
        );
    }

    #[test]
    fn set_len_capacity_evidence_accepts_const_cap_token() {
        let obligations = vec![SafetyObligation::new(
            "capacity",
            "new length is at most capacity",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let set_len = site_with_family(
            OperationFamily::VecSetLen,
            vec![
                "pub struct Buffer<const CAP: usize> {",
                "    xs: [MaybeUninit<u8>; CAP],",
            ],
            "out.set_len(CAP);",
            vec![],
        );

        let evidence = obligation_evidence(&set_len, &obligations, &contract, &reach);

        assert!(evidence[0].discharge.present);
    }

    #[test]
    fn set_len_capacity_mention_without_bound_is_not_guard_evidence() {
        let obligations = vec![SafetyObligation::new(
            "capacity",
            "new length is at most capacity",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let observation_only = site_with_family(
            OperationFamily::VecSetLen,
            vec!["let capacity = values.capacity();", "record(capacity);"],
            "values.set_len(new_len);",
            vec![],
        );
        let bounded = site_with_family(
            OperationFamily::VecSetLen,
            vec!["assert!(new_len <= values.capacity());"],
            "values.set_len(new_len);",
            vec![],
        );

        assert!(
            !obligation_evidence(&observation_only, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            obligation_evidence(&bounded, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
    }

    #[test]
    fn set_len_with_capacity_discharges_capacity_only_for_same_receiver_and_len() {
        let obligations = vec![SafetyObligation::new(
            "capacity",
            "new length is at most capacity",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let matching = site_with_family(
            OperationFamily::VecSetLen,
            vec!["let mut values = Vec::with_capacity(new_len);"],
            "unsafe { values.set_len(new_len); }",
            vec![],
        );
        let other_len = site_with_family(
            OperationFamily::VecSetLen,
            vec!["let mut values = Vec::with_capacity(capacity);"],
            "unsafe { values.set_len(new_len); }",
            vec![],
        );
        let other_receiver = site_with_family(
            OperationFamily::VecSetLen,
            vec!["let mut other = Vec::with_capacity(new_len);"],
            "unsafe { values.set_len(new_len); }",
            vec![],
        );

        assert!(
            obligation_evidence(&matching, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&other_len, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&other_receiver, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
    }

    #[test]
    fn set_len_reserve_discharges_capacity_only_for_same_receiver_and_fresh_len() {
        let obligations = vec![SafetyObligation::new(
            "capacity",
            "new length is at most capacity",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let matching = site_with_family(
            OperationFamily::VecSetLen,
            vec![
                "let new_len = values.len() + additional;",
                "values.reserve(additional);",
            ],
            "unsafe { values.set_len(new_len); }",
            vec![],
        );
        let try_reserve = site_with_family(
            OperationFamily::VecSetLen,
            vec![
                "let new_len = values.len() + additional;",
                "values.try_reserve(additional)?;",
            ],
            "unsafe { values.set_len(new_len); }",
            vec![],
        );
        let other_receiver = site_with_family(
            OperationFamily::VecSetLen,
            vec![
                "let new_len = values.len() + additional;",
                "other.reserve(additional);",
            ],
            "unsafe { values.set_len(new_len); }",
            vec![],
        );
        let stale_additional = site_with_family(
            OperationFamily::VecSetLen,
            vec![
                "let new_len = values.len() + additional;",
                "additional = 0;",
                "values.reserve(additional);",
            ],
            "unsafe { values.set_len(new_len); }",
            vec![],
        );
        let stale_new_len = site_with_family(
            OperationFamily::VecSetLen,
            vec![
                "let new_len = values.len() + additional;",
                "values.reserve(additional);",
                "new_len = values.capacity() + 1;",
            ],
            "unsafe { values.set_len(new_len); }",
            vec![],
        );

        assert!(
            obligation_evidence(&matching, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            obligation_evidence(&try_reserve, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&other_receiver, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&stale_additional, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&stale_new_len, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
    }

    #[test]
    fn drop_in_place_box_origin_discharges_drop_obligations_for_same_pointer() {
        let obligations = vec![
            SafetyObligation::new("pointer-live", "pointer is valid for dropping one value"),
            SafetyObligation::new("initialized", "pointed-to value is initialized"),
            SafetyObligation::new(
                "ownership",
                "value will not be dropped again or observed after drop",
            ),
        ];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let matching = site_with_family(
            OperationFamily::DropInPlace,
            vec!["let ptr = Box::into_raw(value);"],
            "core::ptr::drop_in_place(ptr);",
            vec![],
        );
        let other_pointer = site_with_family(
            OperationFamily::DropInPlace,
            vec!["let other = Box::into_raw(value);"],
            "core::ptr::drop_in_place(ptr);",
            vec![],
        );
        let reassigned_pointer = site_with_family(
            OperationFamily::DropInPlace,
            vec!["let mut ptr = Box::into_raw(value);", "ptr = foreign_ptr;"],
            "core::ptr::drop_in_place(ptr);",
            vec![],
        );

        let evidence = obligation_evidence(&matching, &obligations, &contract, &reach);
        assert!(evidence.iter().all(|item| item.discharge.present));
        let evidence = obligation_evidence(&other_pointer, &obligations, &contract, &reach);
        assert!(evidence.iter().all(|item| !item.discharge.present));
        let evidence = obligation_evidence(&reassigned_pointer, &obligations, &contract, &reach);
        assert!(evidence.iter().all(|item| !item.discharge.present));
    }

    #[test]
    fn box_from_raw_origin_discharges_ownership_for_same_pointer() {
        let obligations = vec![SafetyObligation::new(
            "ownership",
            "raw pointer was produced by compatible allocator and is uniquely owned",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let matching = site_with_family(
            OperationFamily::BoxFromRaw,
            vec!["let ptr = Box::into_raw(value);"],
            "unsafe { Box::from_raw(ptr) }",
            vec![],
        );
        let other_pointer = site_with_family(
            OperationFamily::BoxFromRaw,
            vec!["let other = Box::into_raw(value);"],
            "unsafe { Box::from_raw(ptr) }",
            vec![],
        );
        let reassigned_pointer = site_with_family(
            OperationFamily::BoxFromRaw,
            vec!["let mut ptr = Box::into_raw(value);", "ptr = foreign_ptr;"],
            "unsafe { Box::from_raw(ptr) }",
            vec![],
        );

        assert!(
            obligation_evidence(&matching, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&other_pointer, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&reassigned_pointer, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
    }

    #[test]
    fn vec_from_raw_parts_capacity_guard_must_match_len_and_cap_arguments() {
        let obligations = vec![SafetyObligation::new(
            "capacity",
            "`len` is at most `capacity`",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let bounded = site_with_family(
            OperationFamily::VecFromRawParts,
            vec!["if len > cap {", "    return None;", "}"],
            "Some(unsafe { Vec::from_raw_parts(buf, len, cap) })",
            vec![],
        );
        let unrelated_capacity = site_with_family(
            OperationFamily::VecFromRawParts,
            vec!["let capacity = other_cap;", "assert!(len <= capacity);"],
            "unsafe { Vec::from_raw_parts(buf, len, cap) }",
            vec![],
        );
        let after_call = site_with_family(
            OperationFamily::VecFromRawParts,
            vec![],
            "unsafe { Vec::from_raw_parts(buf, len, cap) }",
            vec!["assert!(len <= cap);"],
        );

        assert!(
            obligation_evidence(&bounded, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&unrelated_capacity, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&after_call, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
    }

    #[test]
    fn vec_from_raw_parts_capacity_early_return_ignores_comment_text() {
        let obligations = vec![SafetyObligation::new(
            "capacity",
            "`len` is at most `capacity`",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let actual_return = site_with_family(
            OperationFamily::VecFromRawParts,
            vec!["if len > cap {", "    return None;", "}"],
            "Some(unsafe { Vec::from_raw_parts(buf, len, cap) })",
            vec![],
        );
        let commented_return = site_with_family(
            OperationFamily::VecFromRawParts,
            vec!["if len > cap {", "    /* return None; */", "}"],
            "Some(unsafe { Vec::from_raw_parts(buf, len, cap) })",
            vec![],
        );
        let string_return = site_with_family(
            OperationFamily::VecFromRawParts,
            vec!["if len > cap {", "    let _note = \"return None\";", "}"],
            "Some(unsafe { Vec::from_raw_parts(buf, len, cap) })",
            vec![],
        );

        assert!(
            obligation_evidence(&actual_return, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&commented_return, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&string_return, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
    }

    #[test]
    fn vec_from_raw_parts_capacity_accepts_same_manuallydrop_origin_len_and_cap() {
        let obligations = vec![SafetyObligation::new(
            "capacity",
            "`len` is at most `capacity`",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let same_origin = site_with_family(
            OperationFamily::VecFromRawParts,
            vec![
                "let mut raw = core::mem::ManuallyDrop::new(input);",
                "let len = raw.len();",
                "let cap = raw.capacity();",
            ],
            "unsafe { Vec::from_raw_parts(ptr, len, cap) }",
            vec![],
        );
        let mismatched_origin = site_with_family(
            OperationFamily::VecFromRawParts,
            vec![
                "let raw = core::mem::ManuallyDrop::new(input);",
                "let other = core::mem::ManuallyDrop::new(spare);",
                "let len = raw.len();",
                "let cap = other.capacity();",
            ],
            "unsafe { Vec::from_raw_parts(ptr, len, cap) }",
            vec![],
        );
        let observed_without_origin = site_with_family(
            OperationFamily::VecFromRawParts,
            vec!["let len = input.len();", "let cap = input.capacity();"],
            "unsafe { Vec::from_raw_parts(ptr, len, cap) }",
            vec![],
        );

        assert!(
            obligation_evidence(&same_origin, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&mismatched_origin, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&observed_without_origin, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
    }

    #[test]
    fn vec_from_raw_parts_origin_discharges_pointer_live_for_same_pointer_and_capacity() {
        let obligations = vec![SafetyObligation::new(
            "pointer-live",
            "pointer was allocated by a compatible allocator for `capacity` elements",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let matching = site_with_family(
            OperationFamily::VecFromRawParts,
            vec![
                "let mut raw = core::mem::ManuallyDrop::new(input);",
                "let ptr = raw.as_mut_ptr();",
                "let cap = raw.capacity();",
            ],
            "unsafe { Vec::from_raw_parts(ptr, len, cap) }",
            vec![],
        );
        let mismatched_capacity_origin = site_with_family(
            OperationFamily::VecFromRawParts,
            vec![
                "let mut raw = core::mem::ManuallyDrop::new(input);",
                "let other = core::mem::ManuallyDrop::new(spare);",
                "let ptr = raw.as_mut_ptr();",
                "let cap = other.capacity();",
            ],
            "unsafe { Vec::from_raw_parts(ptr, len, cap) }",
            vec![],
        );
        let observed_without_origin = site_with_family(
            OperationFamily::VecFromRawParts,
            vec![
                "let ptr = input.as_mut_ptr();",
                "let cap = input.capacity();",
            ],
            "unsafe { Vec::from_raw_parts(ptr, len, cap) }",
            vec![],
        );

        assert!(
            obligation_evidence(&matching, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&mismatched_capacity_origin, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&observed_without_origin, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
    }

    #[test]
    fn vec_from_raw_parts_origin_discharges_ownership_for_same_pointer() {
        let obligations = vec![SafetyObligation::new(
            "ownership",
            "the constructed Vec receives unique ownership and will not double-free",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let matching = site_with_family(
            OperationFamily::VecFromRawParts,
            vec![
                "let mut raw = core::mem::ManuallyDrop::new(input);",
                "let ptr = raw.as_mut_ptr();",
            ],
            "unsafe { Vec::from_raw_parts(ptr, len, cap) }",
            vec![],
        );
        let other_pointer = site_with_family(
            OperationFamily::VecFromRawParts,
            vec![
                "let mut raw = core::mem::ManuallyDrop::new(input);",
                "let other = raw.as_mut_ptr();",
            ],
            "unsafe { Vec::from_raw_parts(ptr, len, cap) }",
            vec![],
        );
        let unmanaged_pointer = site_with_family(
            OperationFamily::VecFromRawParts,
            vec!["let ptr = input.as_mut_ptr();"],
            "unsafe { Vec::from_raw_parts(ptr, len, cap) }",
            vec![],
        );
        let out_of_order_origin = site_with_family(
            OperationFamily::VecFromRawParts,
            vec![
                "let ptr = raw.as_mut_ptr();",
                "let mut raw = core::mem::ManuallyDrop::new(input);",
            ],
            "unsafe { Vec::from_raw_parts(ptr, len, cap) }",
            vec![],
        );

        assert!(
            obligation_evidence(&matching, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&other_pointer, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&unmanaged_pointer, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&out_of_order_origin, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
    }

    #[test]
    fn vec_from_raw_parts_origin_discharges_initialized_for_same_pointer_and_len() {
        let obligations = vec![SafetyObligation::new(
            "initialized",
            "first `len` elements are initialized",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let matching = site_with_family(
            OperationFamily::VecFromRawParts,
            vec![
                "let mut raw = core::mem::ManuallyDrop::new(input);",
                "let ptr = raw.as_mut_ptr();",
                "let len = raw.len();",
            ],
            "unsafe { Vec::from_raw_parts(ptr, len, cap) }",
            vec![],
        );
        let mismatched_len_origin = site_with_family(
            OperationFamily::VecFromRawParts,
            vec![
                "let mut raw = core::mem::ManuallyDrop::new(input);",
                "let other = core::mem::ManuallyDrop::new(spare);",
                "let ptr = raw.as_mut_ptr();",
                "let len = other.len();",
            ],
            "unsafe { Vec::from_raw_parts(ptr, len, cap) }",
            vec![],
        );
        let observed_without_origin = site_with_family(
            OperationFamily::VecFromRawParts,
            vec!["let ptr = input.as_mut_ptr();", "let len = input.len();"],
            "unsafe { Vec::from_raw_parts(ptr, len, cap) }",
            vec![],
        );

        assert!(
            obligation_evidence(&matching, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&mismatched_len_origin, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&observed_without_origin, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
    }

    #[test]
    fn set_len_shrink_discharges_capacity_and_initialized_obligations() {
        let obligations = vec![
            SafetyObligation::new("capacity", "new length is at most capacity"),
            SafetyObligation::new(
                "initialized",
                "elements in the extended range are initialized",
            ),
        ];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let set_len = site_with_family(
            OperationFamily::VecSetLen,
            vec!["if new_len <= values.len() {"],
            "values.set_len(new_len);",
            vec![],
        );

        let evidence = obligation_evidence(&set_len, &obligations, &contract, &reach);

        assert!(evidence.iter().all(|item| item.discharge.present));
    }

    #[test]
    fn set_len_zero_discharges_capacity_and_initialized_obligations() {
        let obligations = vec![
            SafetyObligation::new("capacity", "new length is at most capacity"),
            SafetyObligation::new(
                "initialized",
                "elements in the extended range are initialized",
            ),
        ];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let set_len = site_with_family(
            OperationFamily::VecSetLen,
            vec![],
            "values.set_len(0);",
            vec![],
        );

        let evidence = obligation_evidence(&set_len, &obligations, &contract, &reach);

        assert!(evidence.iter().all(|item| item.discharge.present));
    }

    #[test]
    fn set_len_call_result_discharges_initialized_obligation() {
        let obligations = vec![
            SafetyObligation::new("capacity", "new length is at most capacity"),
            SafetyObligation::new(
                "initialized",
                "elements in the extended range are initialized",
            ),
        ];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let set_len = site_with_family(
            OperationFamily::VecSetLen,
            vec![
                "let remaining_cap = self.capacity() - len;",
                "let n = encode_utf8(c, ptr, remaining_cap)?;",
            ],
            "self.set_len(len + n);",
            vec![],
        );

        let evidence = obligation_evidence(&set_len, &obligations, &contract, &reach);

        assert!(evidence.iter().all(|item| item.discharge.present));
    }

    #[test]
    fn set_len_remaining_capacity_guard_discharges_capacity_obligation() {
        let obligations = vec![SafetyObligation::new(
            "capacity",
            "new length is at most capacity",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let guarded = site_with_family(
            OperationFamily::VecSetLen,
            vec![
                "if s.len() > self.capacity() - self.len() { return; }",
                "let old_len = self.len();",
                "let new_len = old_len + s.len();",
            ],
            "self.set_len(new_len);",
            vec![],
        );
        let other_receiver_guard = site_with_family(
            OperationFamily::VecSetLen,
            vec![
                "if s.len() > other.capacity() - other.len() { return; }",
                "let old_len = self.len();",
                "let new_len = old_len + s.len();",
            ],
            "self.set_len(new_len);",
            vec![],
        );
        let stale_after_guard = site_with_family(
            OperationFamily::VecSetLen,
            vec![
                "if s.len() > self.capacity() - self.len() { return; }",
                "let old_len = self.len();",
                "let new_len = old_len + s.len();",
                "s = replacement;",
            ],
            "self.set_len(new_len);",
            vec![],
        );

        let guarded_evidence = obligation_evidence(&guarded, &obligations, &contract, &reach);
        let other_receiver_evidence =
            obligation_evidence(&other_receiver_guard, &obligations, &contract, &reach);
        let stale_evidence =
            obligation_evidence(&stale_after_guard, &obligations, &contract, &reach);

        assert!(guarded_evidence[0].discharge.present);
        assert!(!other_receiver_evidence[0].discharge.present);
        assert!(!stale_evidence[0].discharge.present);
    }

    #[test]
    fn set_len_slice_binding_loop_discharges_initialized_obligation() {
        let obligations = vec![
            SafetyObligation::new("capacity", "new length is at most capacity"),
            SafetyObligation::new(
                "initialized",
                "elements in the extended range are initialized",
            ),
        ];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let set_len = site_with_family(
            OperationFamily::VecSetLen,
            vec![
                "let old_len = self.len();",
                "let new_len = old_len + s.len();",
                "if new_len > self.capacity() { return; }",
                "let dst = &mut self.xs[old_len..new_len];",
                "for (dst, src) in dst.iter_mut().zip(s.as_bytes().iter()) {",
                "    *dst = MaybeUninit::new(*src);",
                "}",
            ],
            "self.set_len(new_len);",
            vec![],
        );
        let wrong_target = site_with_family(
            OperationFamily::VecSetLen,
            vec![
                "let old_len = self.len();",
                "let new_len = old_len + s.len();",
                "if new_len > self.capacity() { return; }",
                "let dst = &mut other[old_len..new_len];",
                "for item in dst.iter_mut() {",
                "    *item = MaybeUninit::new(0);",
                "}",
            ],
            "self.set_len(new_len);",
            vec![],
        );
        let partial_range = site_with_family(
            OperationFamily::VecSetLen,
            vec![
                "let old_len = self.len();",
                "let new_len = old_len + s.len();",
                "if new_len > self.capacity() { return; }",
                "let dst = &mut self.xs[old_len..new_len - 1];",
                "for item in dst.iter_mut() {",
                "    *item = MaybeUninit::new(0);",
                "}",
            ],
            "self.set_len(new_len);",
            vec![],
        );

        let evidence = obligation_evidence(&set_len, &obligations, &contract, &reach);
        let wrong_target_evidence =
            obligation_evidence(&wrong_target, &obligations, &contract, &reach);
        let partial_range_evidence =
            obligation_evidence(&partial_range, &obligations, &contract, &reach);

        assert!(evidence.iter().all(|item| item.discharge.present));
        assert!(
            !wrong_target_evidence
                .iter()
                .find(|item| item.obligation.key == "initialized")
                .unwrap()
                .discharge
                .present
        );
        assert!(
            !partial_range_evidence
                .iter()
                .find(|item| item.obligation.key == "initialized")
                .unwrap()
                .discharge
                .present
        );
    }

    #[test]
    fn set_len_const_cap_evidence_requires_const_capacity_context() {
        let obligations = vec![SafetyObligation::new(
            "capacity",
            "new length is at most capacity",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let const_capacity = site_with_family(
            OperationFamily::VecSetLen,
            vec![
                "impl<const CAP: usize> Buffer<CAP> {",
                "    xs: [MaybeUninit<u8>; CAP],",
                "    let mut out = Self { xs: [MaybeUninit::uninit(); CAP], len: 0 };",
            ],
            "out.set_len(CAP);",
            vec![],
        );
        let unrelated_local_named_cap = site_with_family(
            OperationFamily::VecSetLen,
            vec!["let cap = requested;"],
            "values.set_len(cap);",
            vec![],
        );

        let const_evidence = obligation_evidence(&const_capacity, &obligations, &contract, &reach);
        let unrelated_evidence =
            obligation_evidence(&unrelated_local_named_cap, &obligations, &contract, &reach);

        assert!(const_evidence[0].discharge.present);
        assert!(!unrelated_evidence[0].discharge.present);
    }

    #[test]
    fn unchecked_constructor_availability_guard_discharges_callee_contract() {
        let obligations = vec![SafetyObligation::new(
            "callee-contract",
            "callee safety preconditions are satisfied",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let guarded = site_with_family(
            OperationFamily::UnsafeFnCall,
            vec!["if One::is_available() {"],
            "unsafe { Some(One::new_unchecked(needle)) }",
            vec!["}"],
        );
        let assert_guarded = site_with_family(
            OperationFamily::UnsafeFnCall,
            vec!["assert!(One::is_available());"],
            "unsafe { Some(One::new_unchecked(needle)) }",
            vec![],
        );
        let unavailable_return_guard = site_with_family(
            OperationFamily::UnsafeFnCall,
            vec!["if !One::is_available() { return None; }"],
            "unsafe { Some(One::new_unchecked(needle)) }",
            vec![],
        );
        let unguarded = site_with_family(
            OperationFamily::UnsafeFnCall,
            vec![],
            "unsafe { Some(One::new_unchecked(needle)) }",
            vec![],
        );

        let guarded_evidence = obligation_evidence(&guarded, &obligations, &contract, &reach);
        let assert_guarded_evidence =
            obligation_evidence(&assert_guarded, &obligations, &contract, &reach);
        let unavailable_return_evidence =
            obligation_evidence(&unavailable_return_guard, &obligations, &contract, &reach);
        let unguarded_evidence = obligation_evidence(&unguarded, &obligations, &contract, &reach);

        assert!(guarded_evidence[0].discharge.present);
        assert!(assert_guarded_evidence[0].discharge.present);
        assert!(unavailable_return_evidence[0].discharge.present);
        assert!(!unguarded_evidence[0].discharge.present);
    }

    #[test]
    fn unchecked_constructor_availability_guard_requires_same_receiver_and_precedes_call() {
        let obligations = vec![SafetyObligation::new(
            "callee-contract",
            "callee safety preconditions are satisfied",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let other_receiver = site_with_family(
            OperationFamily::UnsafeFnCall,
            vec!["if Two::is_available() {"],
            "unsafe { Some(One::new_unchecked(needle)) }",
            vec!["}"],
        );
        let post_call = site_with_family(
            OperationFamily::UnsafeFnCall,
            vec![],
            "unsafe { Some(One::new_unchecked(needle)) }",
            vec!["if One::is_available() { record_available(); }"],
        );
        let observed_availability = site_with_family(
            OperationFamily::UnsafeFnCall,
            vec![
                "let available = One::is_available();",
                "record_available(available);",
            ],
            "unsafe { Some(One::new_unchecked(needle)) }",
            vec![],
        );
        let closed_availability_branch = site_with_family(
            OperationFamily::UnsafeFnCall,
            vec![
                "if One::is_available() {",
                "    record_available(true);",
                "}",
            ],
            "unsafe { Some(One::new_unchecked(needle)) }",
            vec![],
        );
        let generic_call = site_with_family(
            OperationFamily::UnsafeFnCall,
            vec!["if One::is_available() {"],
            "unsafe { Some(One::new_unchecked::<Needle>(needle)) }",
            vec!["}"],
        );

        let other_receiver_evidence =
            obligation_evidence(&other_receiver, &obligations, &contract, &reach);
        let post_call_evidence = obligation_evidence(&post_call, &obligations, &contract, &reach);
        let observed_availability_evidence =
            obligation_evidence(&observed_availability, &obligations, &contract, &reach);
        let closed_availability_branch_evidence =
            obligation_evidence(&closed_availability_branch, &obligations, &contract, &reach);
        let generic_call_evidence =
            obligation_evidence(&generic_call, &obligations, &contract, &reach);

        assert!(!other_receiver_evidence[0].discharge.present);
        assert!(!post_call_evidence[0].discharge.present);
        assert!(!observed_availability_evidence[0].discharge.present);
        assert!(!closed_availability_branch_evidence[0].discharge.present);
        assert!(generic_call_evidence[0].discharge.present);
    }

    #[test]
    fn set_len_last_index_shrink_discharges_capacity_and_initialized_obligations() {
        let obligations = vec![
            SafetyObligation::new("capacity", "new length is at most capacity"),
            SafetyObligation::new(
                "initialized",
                "elements in the extended range are initialized",
            ),
        ];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let set_len = site_with_family(
            OperationFamily::VecSetLen,
            vec![
                "if self.len == 0 {",
                "    return None;",
                "}",
                "let last_index = self.len - 1;",
            ],
            "self.set_len(last_index);",
            vec![],
        );

        let evidence = obligation_evidence(&set_len, &obligations, &contract, &reach);

        assert!(evidence.iter().all(|item| item.discharge.present));
    }

    #[test]
    fn set_len_last_index_shrink_accepts_len_method_receiver() {
        let obligations = vec![
            SafetyObligation::new("capacity", "new length is at most capacity"),
            SafetyObligation::new(
                "initialized",
                "elements in the extended range are initialized",
            ),
        ];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let set_len = site_with_family(
            OperationFamily::VecSetLen,
            vec![
                "if values.len() == 0 {",
                "    return None;",
                "}",
                "let last_index = values.len() - 1;",
            ],
            "values.set_len(last_index);",
            vec![],
        );

        let evidence = obligation_evidence(&set_len, &obligations, &contract, &reach);

        assert!(evidence.iter().all(|item| item.discharge.present));
    }

    #[test]
    fn set_len_last_index_shrink_requires_non_empty_guard() {
        let obligations = vec![
            SafetyObligation::new("capacity", "new length is at most capacity"),
            SafetyObligation::new(
                "initialized",
                "elements in the extended range are initialized",
            ),
        ];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let set_len = site_with_family(
            OperationFamily::VecSetLen,
            vec!["let last_index = values.len() - 1;"],
            "values.set_len(last_index);",
            vec![],
        );

        let evidence = obligation_evidence(&set_len, &obligations, &contract, &reach);

        assert!(evidence.iter().all(|item| !item.discharge.present));
    }

    #[test]
    fn set_len_start_bound_shrink_discharges_capacity_and_initialized_obligations() {
        let obligations = vec![
            SafetyObligation::new("capacity", "new length is at most capacity"),
            SafetyObligation::new(
                "initialized",
                "elements in the extended range are initialized",
            ),
        ];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let set_len = site_with_family(
            OperationFamily::VecSetLen,
            vec![
                "let len = values.len();",
                "assert!(start <= end);",
                "assert!(end <= len);",
            ],
            "values.set_len(start);",
            vec![],
        );

        let evidence = obligation_evidence(&set_len, &obligations, &contract, &reach);

        assert!(evidence.iter().all(|item| item.discharge.present));
    }

    #[test]
    fn set_len_start_bound_shrink_requires_upper_bound() {
        let obligations = vec![
            SafetyObligation::new("capacity", "new length is at most capacity"),
            SafetyObligation::new(
                "initialized",
                "elements in the extended range are initialized",
            ),
        ];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let set_len = site_with_family(
            OperationFamily::VecSetLen,
            vec!["let len = values.len();", "assert!(start <= end);"],
            "values.set_len(start);",
            vec![],
        );

        let evidence = obligation_evidence(&set_len, &obligations, &contract, &reach);

        assert!(evidence.iter().all(|item| !item.discharge.present));
    }

    #[test]
    fn len_capacity_equality_discharges_bounds_obligation() {
        let obligations = vec![SafetyObligation::new(
            "bounds",
            "buffer has enough bytes for the accessed type",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let raw_read = site_with_family(
            OperationFamily::RawPointerRead,
            vec!["debug_assert_eq!(self.len(), self.capacity());"],
            "ptr::read(self.as_ptr() as *const [T; CAP])",
            vec![],
        );

        let evidence = obligation_evidence(&raw_read, &obligations, &contract, &reach);

        assert!(evidence[0].discharge.present);
    }

    #[test]
    fn get_unchecked_bounds_guard_must_match_receiver_when_known() {
        let obligations = vec![SafetyObligation::new(
            "bounds",
            "index is in bounds for the collection",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let other_receiver_guard = site_with_family(
            OperationFamily::GetUnchecked,
            vec!["if index < other_values.len() {"],
            "unsafe { values.get_unchecked_mut(index) }",
            vec!["}"],
        );
        let matching_guard = site_with_family(
            OperationFamily::GetUnchecked,
            vec!["if index < values.len() {"],
            "unsafe { values.get_unchecked_mut(index) }",
            vec!["}"],
        );
        let matching_return_guard = site_with_family(
            OperationFamily::GetUnchecked,
            vec!["if index >= values.len() { return None; }"],
            "unsafe { values.get_unchecked_mut(index) }",
            vec![],
        );
        let commented_return_guard = site_with_family(
            OperationFamily::GetUnchecked,
            vec!["if index >= values.len() { /* return None; */ }"],
            "unsafe { values.get_unchecked_mut(index) }",
            vec![],
        );
        let string_return_guard = site_with_family(
            OperationFamily::GetUnchecked,
            vec!["if index >= values.len() { let _note = \"return None\"; }"],
            "unsafe { values.get_unchecked_mut(index) }",
            vec![],
        );
        let matching_assertion = site_with_family(
            OperationFamily::GetUnchecked,
            vec!["assert!(index < values.len());"],
            "unsafe { values.get_unchecked_mut(index) }",
            vec![],
        );
        let post_guard = site_with_family(
            OperationFamily::GetUnchecked,
            vec![],
            "unsafe { values.get_unchecked_mut(index) }",
            vec!["if index < values.len() {", "    return Some(());", "}"],
        );
        let closed_positive_branch = site_with_family(
            OperationFamily::GetUnchecked,
            vec!["if index < values.len() {", "    observe(index);", "}"],
            "unsafe { values.get_unchecked_mut(index) }",
            vec![],
        );
        let reassigned_index = site_with_family(
            OperationFamily::GetUnchecked,
            vec![
                "if index >= values.len() { return None; }",
                "index = values.len();",
            ],
            "unsafe { values.get_unchecked_mut(index) }",
            vec![],
        );
        let assertion_then_reassigned_index = site_with_family(
            OperationFamily::GetUnchecked,
            vec!["assert!(index < values.len());", "index = values.len();"],
            "unsafe { values.get_unchecked_mut(index) }",
            vec![],
        );
        let get_probe_branch = site_with_family(
            OperationFamily::GetUnchecked,
            vec!["if values.get(index).is_some() {"],
            "unsafe { values.get_unchecked_mut(index) }",
            vec!["}"],
        );
        let get_probe_return = site_with_family(
            OperationFamily::GetUnchecked,
            vec!["if values.get(index).is_none() { return None; }"],
            "unsafe { values.get_unchecked_mut(index) }",
            vec![],
        );
        let get_probe_other_receiver = site_with_family(
            OperationFamily::GetUnchecked,
            vec!["if other_values.get(index).is_some() {"],
            "unsafe { values.get_unchecked_mut(index) }",
            vec!["}"],
        );
        let get_probe_reassigned_index = site_with_family(
            OperationFamily::GetUnchecked,
            vec!["if values.get(index).is_some() {", "index = values.len();"],
            "unsafe { values.get_unchecked_mut(index) }",
            vec!["}"],
        );
        let get_probe_if_let = site_with_family(
            OperationFamily::GetUnchecked,
            vec!["if let Some(_) = values.get(index) {"],
            "unsafe { values.get_unchecked_mut(index) }",
            vec!["}"],
        );
        let get_probe_let_else = site_with_family(
            OperationFamily::GetUnchecked,
            vec!["let Some(_) = values.get(index) else { return None; };"],
            "unsafe { values.get_unchecked_mut(index) }",
            vec![],
        );
        let get_probe_match = site_with_family(
            OperationFamily::GetUnchecked,
            vec!["match values.get(index) {", "Some(_) => {"],
            "unsafe { values.get_unchecked_mut(index) }",
            vec!["}", "None => None,", "}"],
        );
        let get_probe_if_let_reassigned_index = site_with_family(
            OperationFamily::GetUnchecked,
            vec![
                "if let Some(_) = values.get(index) {",
                "index = values.len();",
            ],
            "unsafe { values.get_unchecked_mut(index) }",
            vec!["}"],
        );
        let get_probe_let_else_reassigned_index = site_with_family(
            OperationFamily::GetUnchecked,
            vec![
                "let Some(_) = values.get(index) else { return None; };",
                "index = values.len();",
            ],
            "unsafe { values.get_unchecked_mut(index) }",
            vec![],
        );
        let get_probe_match_reassigned_index = site_with_family(
            OperationFamily::GetUnchecked,
            vec![
                "match values.get(index) {",
                "Some(_) => {",
                "index = values.len();",
            ],
            "unsafe { values.get_unchecked_mut(index) }",
            vec!["}", "None => None,", "}"],
        );

        assert!(
            !obligation_evidence(&other_receiver_guard, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            obligation_evidence(&matching_guard, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            obligation_evidence(&matching_return_guard, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&commented_return_guard, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&string_return_guard, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            obligation_evidence(&matching_assertion, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&post_guard, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&closed_positive_branch, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&reassigned_index, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(
                &assertion_then_reassigned_index,
                &obligations,
                &contract,
                &reach
            )[0]
            .discharge
            .present
        );
        assert!(
            obligation_evidence(&get_probe_branch, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            obligation_evidence(&get_probe_return, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&get_probe_other_receiver, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&get_probe_reassigned_index, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            obligation_evidence(&get_probe_if_let, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            obligation_evidence(&get_probe_let_else, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            obligation_evidence(&get_probe_match, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(
                &get_probe_if_let_reassigned_index,
                &obligations,
                &contract,
                &reach
            )[0]
            .discharge
            .present
        );
        assert!(
            !obligation_evidence(
                &get_probe_let_else_reassigned_index,
                &obligations,
                &contract,
                &reach
            )[0]
            .discharge
            .present
        );
        assert!(
            !obligation_evidence(
                &get_probe_match_reassigned_index,
                &obligations,
                &contract,
                &reach
            )[0]
            .discharge
            .present
        );
    }

    #[test]
    fn encode_utf8_remaining_capacity_discharges_unsafe_call_obligation() {
        let obligations = vec![SafetyObligation::new(
            "callee-contract",
            "callee safety preconditions are satisfied",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let unsafe_call = site_with_family(
            OperationFamily::UnsafeFnCall,
            vec![
                "let ptr = self.xs[len..].as_mut_ptr() as *mut u8;",
                "let remaining_cap = self.capacity() - len;",
                "// SAFETY: `ptr` points to `remaining_cap` bytes.",
            ],
            "match unsafe { encode_utf8(c, ptr, remaining_cap) } {",
            vec![],
        );

        let evidence = obligation_evidence(&unsafe_call, &obligations, &contract, &reach);

        assert!(evidence[0].discharge.present);
    }

    #[test]
    fn maybeuninit_slice_discharges_initialized_obligation_only() {
        let obligations = vec![
            SafetyObligation::new("initialized", "memory range is initialized"),
            SafetyObligation::new("alignment", "pointer is aligned for the element type"),
        ];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let slice = site_with_family(
            OperationFamily::SliceFromRawParts,
            vec!["fn ctrl_slice(&mut self) -> &mut [core::mem::MaybeUninit<Tag>] {"],
            "unsafe { core::slice::from_raw_parts_mut(self.ctrl.as_ptr().cast(), self.len) }",
            vec![],
        );

        let evidence = obligation_evidence(&slice, &obligations, &contract, &reach);

        assert!(evidence[0].discharge.present);
        assert!(!evidence[1].discharge.present);
    }

    #[test]
    fn maybeuninit_slice_evidence_must_belong_to_slice_type_or_arguments() {
        let obligations = vec![SafetyObligation::new(
            "initialized",
            "memory range is initialized",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let unrelated = site_with_family(
            OperationFamily::SliceFromRawParts,
            vec![
                "fn expose_mut(ptr: *mut u8, len: usize) -> &mut [u8] {",
                "    let _scratch: core::mem::MaybeUninit<u8> = core::mem::MaybeUninit::uninit();",
            ],
            "unsafe { core::slice::from_raw_parts_mut(ptr, len) }",
            vec!["}"],
        );

        let evidence = obligation_evidence(&unrelated, &obligations, &contract, &reach);

        assert!(!evidence[0].discharge.present);
    }

    #[test]
    fn maybeuninit_raw_write_discharges_initialized_obligation_only() {
        let obligations = vec![
            SafetyObligation::new("initialized", "memory is initialized for the accessed type"),
            SafetyObligation::new("alignment", "pointer is aligned for the accessed type"),
        ];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let raw_write = site_with_family(
            OperationFamily::RawPointerWrite,
            vec!["impl TagSliceExt for [core::mem::MaybeUninit<Tag>] {"],
            "unsafe { self.as_mut_ptr().write_bytes(tag.0, self.len()) }",
            vec![],
        );

        let evidence = obligation_evidence(&raw_write, &obligations, &contract, &reach);

        assert!(evidence[0].discharge.present);
        assert!(!evidence[1].discharge.present);
    }

    #[test]
    fn maybeuninit_raw_write_evidence_must_belong_to_target() {
        let obligations = vec![SafetyObligation::new(
            "initialized",
            "memory is initialized for the accessed type",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let unrelated = site_with_family(
            OperationFamily::RawPointerWrite,
            vec![
                "pub fn fill_tag(ptr: *mut u16, len: usize, byte: u8) {",
                "    let _scratch: core::mem::MaybeUninit<u16> = core::mem::MaybeUninit::uninit();",
            ],
            "unsafe { ptr.write_bytes(byte, len) }",
            vec!["}"],
        );

        let evidence = obligation_evidence(&unrelated, &obligations, &contract, &reach);

        assert!(!evidence[0].discharge.present);
    }

    #[test]
    fn u8_write_bytes_discharges_alignment_and_initialized_obligations_only() {
        let obligations = vec![
            SafetyObligation::new("initialized", "memory is initialized for the accessed type"),
            SafetyObligation::new("alignment", "pointer is aligned for the accessed type"),
            SafetyObligation::new("pointer-live", "pointer is live"),
        ];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let raw_write = site_with_family(
            OperationFamily::RawPointerWrite,
            vec!["pub fn fill_bytes(ptr: *mut u8, len: usize, byte: u8) {"],
            "unsafe { ptr.write_bytes(byte, len) }",
            vec![],
        );

        let evidence = obligation_evidence(&raw_write, &obligations, &contract, &reach);

        assert!(evidence[0].discharge.present);
        assert!(evidence[1].discharge.present);
        assert!(!evidence[2].discharge.present);
    }

    #[test]
    fn u8_write_bytes_evidence_must_match_write_target() {
        let obligations = vec![
            SafetyObligation::new("initialized", "memory is initialized for the accessed type"),
            SafetyObligation::new("alignment", "pointer is aligned for the accessed type"),
        ];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let raw_write = site_with_family(
            OperationFamily::RawPointerWrite,
            vec!["pub fn fill_words(ptr: *mut u16, other: *mut u8, len: usize, byte: u8) {"],
            "unsafe { ptr.write_bytes(byte, len) }",
            vec!["}"],
        );

        let evidence = obligation_evidence(&raw_write, &obligations, &contract, &reach);

        assert!(!evidence[0].discharge.present);
        assert!(!evidence[1].discharge.present);
    }

    #[test]
    fn unwrap_unchecked_infallible_result_discharges_valid_value_obligation() {
        let obligations = vec![SafetyObligation::new(
            "valid-value",
            "value is known to be `Some` or `Ok` before `unwrap_unchecked`",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let unwrap = site_with_family(
            OperationFamily::UnwrapUnchecked,
            vec![
                "let result = reserve_rehash(additional, Fallibility::Infallible);",
                "// SAFETY: infallible mode converts allocation errors before this point.",
            ],
            "unsafe { result.unwrap_unchecked() }",
            vec![],
        );

        let evidence = obligation_evidence(&unwrap, &obligations, &contract, &reach);

        assert!(evidence[0].discharge.present);
    }

    #[test]
    fn unwrap_unchecked_infallible_result_evidence_requires_result_receiver() {
        let obligations = vec![SafetyObligation::new(
            "valid-value",
            "value is known to be `Some` or `Ok` before `unwrap_unchecked`",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let option_unwrap = site_with_family(
            OperationFamily::UnwrapUnchecked,
            vec!["let result = reserve_rehash(additional, Fallibility::Infallible);"],
            "unsafe { option.unwrap_unchecked() }",
            vec![],
        );

        let evidence = obligation_evidence(&option_unwrap, &obligations, &contract, &reach);

        assert!(!evidence[0].discharge.present);
    }

    #[test]
    fn unwrap_unchecked_infallible_result_must_match_receiver_assignment() {
        let obligations = vec![SafetyObligation::new(
            "valid-value",
            "value is known to be `Some` or `Ok` before `unwrap_unchecked`",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let other_result = site_with_family(
            OperationFamily::UnwrapUnchecked,
            vec![
                "let other_result = reserve_rehash(additional, Fallibility::Infallible);",
                "let result = reserve_rehash(additional, Fallibility::Fallible);",
            ],
            "unsafe { result.unwrap_unchecked() }",
            vec![],
        );

        let evidence = obligation_evidence(&other_result, &obligations, &contract, &reach);

        assert!(!evidence[0].discharge.present);
    }

    #[test]
    fn unwrap_unchecked_same_receiver_state_discharges_valid_value_obligation() {
        let obligations = vec![SafetyObligation::new(
            "valid-value",
            "value is known to be `Some` or `Ok` before `unwrap_unchecked`",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let option = site_with_family(
            OperationFamily::UnwrapUnchecked,
            vec!["if option.is_some() {"],
            "unsafe { option.unwrap_unchecked() }",
            vec!["}"],
        );
        let option_if_let = site_with_family(
            OperationFamily::UnwrapUnchecked,
            vec!["if let Some(_) = option.as_ref() {"],
            "unsafe { option.unwrap_unchecked() }",
            vec!["}"],
        );
        let option_let_else = site_with_family(
            OperationFamily::UnwrapUnchecked,
            vec![
                "let Some(_) = option.as_ref() else {",
                "    return 0;",
                "};",
            ],
            "unsafe { option.unwrap_unchecked() }",
            vec![],
        );
        let option_match = site_with_family(
            OperationFamily::UnwrapUnchecked,
            vec!["match option.as_ref() {", "    Some(_) => {"],
            "unsafe { option.unwrap_unchecked() }",
            vec!["}", "None => 0,", "}"],
        );
        let result = site_with_family(
            OperationFamily::UnwrapUnchecked,
            vec!["if result.is_ok() {"],
            "unsafe { result.unwrap_unchecked() }",
            vec!["}"],
        );
        let result_if_let = site_with_family(
            OperationFamily::UnwrapUnchecked,
            vec!["if let Ok(_) = result.as_ref() {"],
            "unsafe { result.unwrap_unchecked() }",
            vec!["}"],
        );
        let result_let_else = site_with_family(
            OperationFamily::UnwrapUnchecked,
            vec!["let Ok(_) = result.as_ref() else {", "    return 0;", "};"],
            "unsafe { result.unwrap_unchecked() }",
            vec![],
        );
        let result_match = site_with_family(
            OperationFamily::UnwrapUnchecked,
            vec!["match result.as_ref() {", "    Ok(_) => {"],
            "unsafe { result.unwrap_unchecked() }",
            vec!["}", "Err(_) => 0,", "}"],
        );

        let option_evidence = obligation_evidence(&option, &obligations, &contract, &reach);
        let option_if_let_evidence =
            obligation_evidence(&option_if_let, &obligations, &contract, &reach);
        let option_let_else_evidence =
            obligation_evidence(&option_let_else, &obligations, &contract, &reach);
        let option_match_evidence =
            obligation_evidence(&option_match, &obligations, &contract, &reach);
        let result_evidence = obligation_evidence(&result, &obligations, &contract, &reach);
        let result_if_let_evidence =
            obligation_evidence(&result_if_let, &obligations, &contract, &reach);
        let result_let_else_evidence =
            obligation_evidence(&result_let_else, &obligations, &contract, &reach);
        let result_match_evidence =
            obligation_evidence(&result_match, &obligations, &contract, &reach);

        assert!(option_evidence[0].discharge.present);
        assert!(option_if_let_evidence[0].discharge.present);
        assert!(option_let_else_evidence[0].discharge.present);
        assert!(option_match_evidence[0].discharge.present);
        assert!(result_evidence[0].discharge.present);
        assert!(result_if_let_evidence[0].discharge.present);
        assert!(result_let_else_evidence[0].discharge.present);
        assert!(result_match_evidence[0].discharge.present);
    }

    #[test]
    fn unwrap_unchecked_state_evidence_requires_same_receiver() {
        let obligations = vec![SafetyObligation::new(
            "valid-value",
            "value is known to be `Some` or `Ok` before `unwrap_unchecked`",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let unchecked = site_with_family(
            OperationFamily::UnwrapUnchecked,
            vec!["if other.is_some() {"],
            "unsafe { option.unwrap_unchecked() }",
            vec!["}"],
        );
        let other_if_let = site_with_family(
            OperationFamily::UnwrapUnchecked,
            vec!["if let Some(_) = other.as_ref() {"],
            "unsafe { option.unwrap_unchecked() }",
            vec!["}"],
        );
        let other_let_else = site_with_family(
            OperationFamily::UnwrapUnchecked,
            vec!["let Some(_) = other.as_ref() else {", "    return 0;", "};"],
            "unsafe { option.unwrap_unchecked() }",
            vec![],
        );
        let other_match = site_with_family(
            OperationFamily::UnwrapUnchecked,
            vec!["match other.as_ref() {", "    Some(_) => {"],
            "unsafe { option.unwrap_unchecked() }",
            vec!["}", "None => 0,", "}"],
        );
        let other_result_if_let = site_with_family(
            OperationFamily::UnwrapUnchecked,
            vec!["if let Ok(_) = other.as_ref() {"],
            "unsafe { result.unwrap_unchecked() }",
            vec!["}"],
        );
        let other_result_let_else = site_with_family(
            OperationFamily::UnwrapUnchecked,
            vec!["let Ok(_) = other.as_ref() else {", "    return 0;", "};"],
            "unsafe { result.unwrap_unchecked() }",
            vec![],
        );
        let other_result_match = site_with_family(
            OperationFamily::UnwrapUnchecked,
            vec!["match other.as_ref() {", "    Ok(_) => {"],
            "unsafe { result.unwrap_unchecked() }",
            vec!["}", "Err(_) => 0,", "}"],
        );
        let let_else_then_reassigned = site_with_family(
            OperationFamily::UnwrapUnchecked,
            vec![
                "let Some(_) = option.as_ref() else {",
                "    return 0;",
                "};",
                "option = None;",
            ],
            "unsafe { option.unwrap_unchecked() }",
            vec![],
        );
        let result_let_else_then_reassigned = site_with_family(
            OperationFamily::UnwrapUnchecked,
            vec![
                "let Ok(_) = result.as_ref() else {",
                "    return 0;",
                "};",
                "result = Err(\"reset\");",
            ],
            "unsafe { result.unwrap_unchecked() }",
            vec![],
        );
        let match_then_reassigned = site_with_family(
            OperationFamily::UnwrapUnchecked,
            vec![
                "match option.as_ref() {",
                "    Some(_) => {",
                "        option = None;",
            ],
            "unsafe { option.unwrap_unchecked() }",
            vec!["}", "None => 0,", "}"],
        );
        let result_match_then_reassigned = site_with_family(
            OperationFamily::UnwrapUnchecked,
            vec![
                "match result.as_ref() {",
                "    Ok(_) => {",
                "        result = Err(\"reset\");",
            ],
            "unsafe { result.unwrap_unchecked() }",
            vec!["}", "Err(_) => 0,", "}"],
        );
        let is_some_then_reassigned = site_with_family(
            OperationFamily::UnwrapUnchecked,
            vec!["if option.is_some() {", "    option = None;"],
            "unsafe { option.unwrap_unchecked() }",
            vec!["}"],
        );
        let is_ok_then_reassigned = site_with_family(
            OperationFamily::UnwrapUnchecked,
            vec!["if result.is_ok() {", "    result = Err(\"reset\");"],
            "unsafe { result.unwrap_unchecked() }",
            vec!["}"],
        );

        let evidence = obligation_evidence(&unchecked, &obligations, &contract, &reach);
        let if_let_evidence = obligation_evidence(&other_if_let, &obligations, &contract, &reach);
        let let_else_evidence =
            obligation_evidence(&other_let_else, &obligations, &contract, &reach);
        let match_evidence = obligation_evidence(&other_match, &obligations, &contract, &reach);
        let result_if_let_evidence =
            obligation_evidence(&other_result_if_let, &obligations, &contract, &reach);
        let result_let_else_evidence =
            obligation_evidence(&other_result_let_else, &obligations, &contract, &reach);
        let result_match_evidence =
            obligation_evidence(&other_result_match, &obligations, &contract, &reach);
        let let_else_then_reassigned_evidence =
            obligation_evidence(&let_else_then_reassigned, &obligations, &contract, &reach);
        let result_let_else_then_reassigned_evidence = obligation_evidence(
            &result_let_else_then_reassigned,
            &obligations,
            &contract,
            &reach,
        );
        let match_then_reassigned_evidence =
            obligation_evidence(&match_then_reassigned, &obligations, &contract, &reach);
        let result_match_then_reassigned_evidence = obligation_evidence(
            &result_match_then_reassigned,
            &obligations,
            &contract,
            &reach,
        );
        let is_some_then_reassigned_evidence =
            obligation_evidence(&is_some_then_reassigned, &obligations, &contract, &reach);
        let is_ok_then_reassigned_evidence =
            obligation_evidence(&is_ok_then_reassigned, &obligations, &contract, &reach);

        assert!(!evidence[0].discharge.present);
        assert!(!if_let_evidence[0].discharge.present);
        assert!(!let_else_evidence[0].discharge.present);
        assert!(!match_evidence[0].discharge.present);
        assert!(!result_if_let_evidence[0].discharge.present);
        assert!(!result_let_else_evidence[0].discharge.present);
        assert!(!result_match_evidence[0].discharge.present);
        assert!(!let_else_then_reassigned_evidence[0].discharge.present);
        assert!(
            !result_let_else_then_reassigned_evidence[0]
                .discharge
                .present
        );
        assert!(!match_then_reassigned_evidence[0].discharge.present);
        assert!(!result_match_then_reassigned_evidence[0].discharge.present);
        assert!(!is_some_then_reassigned_evidence[0].discharge.present);
        assert!(!is_ok_then_reassigned_evidence[0].discharge.present);
    }

    #[test]
    fn unwrap_unchecked_state_evidence_must_precede_call() {
        let obligations = vec![SafetyObligation::new(
            "valid-value",
            "value is known to be `Some` or `Ok` before `unwrap_unchecked`",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let unchecked = site_with_family(
            OperationFamily::UnwrapUnchecked,
            vec![],
            "unsafe { option.unwrap_unchecked() }",
            vec!["if option.is_some() {}"],
        );

        let evidence = obligation_evidence(&unchecked, &obligations, &contract, &reach);

        assert!(!evidence[0].discharge.present);
    }

    #[test]
    fn unwrap_unchecked_early_return_state_guard_discharges_valid_value_obligation() {
        let obligations = vec![SafetyObligation::new(
            "valid-value",
            "value is known to be `Some` or `Ok` before `unwrap_unchecked`",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let option = site_with_family(
            OperationFamily::UnwrapUnchecked,
            vec!["if option.is_none() {", "    return 0;", "}"],
            "unsafe { option.unwrap_unchecked() }",
            vec![],
        );
        let result = site_with_family(
            OperationFamily::UnwrapUnchecked,
            vec!["if result.is_err() {", "    return 0;", "}"],
            "unsafe { result.unwrap_unchecked() }",
            vec![],
        );

        let option_evidence = obligation_evidence(&option, &obligations, &contract, &reach);
        let result_evidence = obligation_evidence(&result, &obligations, &contract, &reach);

        assert!(option_evidence[0].discharge.present);
        assert!(result_evidence[0].discharge.present);
    }

    #[test]
    fn unwrap_unchecked_early_return_guard_requires_returning_branch() {
        let obligations = vec![SafetyObligation::new(
            "valid-value",
            "value is known to be `Some` or `Ok` before `unwrap_unchecked`",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let unchecked = site_with_family(
            OperationFamily::UnwrapUnchecked,
            vec!["if option.is_none() {", "    observe_none();", "}"],
            "unsafe { option.unwrap_unchecked() }",
            vec![],
        );
        let comment_return = site_with_family(
            OperationFamily::UnwrapUnchecked,
            vec!["if option.is_none() {", "    /* return 0; */", "}"],
            "unsafe { option.unwrap_unchecked() }",
            vec![],
        );
        let string_return = site_with_family(
            OperationFamily::UnwrapUnchecked,
            vec![
                "if option.is_none() {",
                "    let _note = \"return 0\";",
                "}",
            ],
            "unsafe { option.unwrap_unchecked() }",
            vec![],
        );

        let evidence = obligation_evidence(&unchecked, &obligations, &contract, &reach);
        let comment_evidence =
            obligation_evidence(&comment_return, &obligations, &contract, &reach);
        let string_evidence = obligation_evidence(&string_return, &obligations, &contract, &reach);

        assert!(!evidence[0].discharge.present);
        assert!(!comment_evidence[0].discharge.present);
        assert!(!string_evidence[0].discharge.present);
    }

    #[test]
    fn from_utf8_validation_discharges_utf8_obligation() {
        let obligations = vec![SafetyObligation::new("utf8", "bytes are valid UTF-8")];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let checked = site_with_family(
            OperationFamily::StrFromUtf8Unchecked,
            vec!["if core::str::from_utf8(bytes).is_ok() {"],
            "unsafe { core::str::from_utf8_unchecked(bytes) }",
            vec!["}"],
        );
        let early_return = site_with_family(
            OperationFamily::StrFromUtf8Unchecked,
            vec![
                "if core::str::from_utf8(bytes).is_err() {",
                "    return \"\";",
                "}",
            ],
            "unsafe { core::str::from_utf8_unchecked(bytes) }",
            vec![],
        );
        let if_let_err_return = site_with_family(
            OperationFamily::StrFromUtf8Unchecked,
            vec![
                "if let Err(_err) = core::str::from_utf8(bytes) {",
                "    return \"\";",
                "}",
            ],
            "unsafe { core::str::from_utf8_unchecked(bytes) }",
            vec![],
        );
        let question_mark = site_with_family(
            OperationFamily::StrFromUtf8Unchecked,
            vec!["core::str::from_utf8(bytes)?;"],
            "unsafe { core::str::from_utf8_unchecked(bytes) }",
            vec![],
        );
        let match_return = site_with_family(
            OperationFamily::StrFromUtf8Unchecked,
            vec![
                "match core::str::from_utf8(bytes) {",
                "    Ok(_) => {}",
                "    Err(err) => return Err(err),",
                "}",
            ],
            "unsafe { core::str::from_utf8_unchecked(bytes) }",
            vec![],
        );
        let if_let_ok = site_with_family(
            OperationFamily::StrFromUtf8Unchecked,
            vec!["if let Ok(_valid) = core::str::from_utf8(bytes) {"],
            "unsafe { core::str::from_utf8_unchecked(bytes) }",
            vec!["}"],
        );
        let let_else_ok = site_with_family(
            OperationFamily::StrFromUtf8Unchecked,
            vec![
                "let Ok(_) = core::str::from_utf8(bytes) else {",
                "    return \"\";",
                "};",
            ],
            "unsafe { core::str::from_utf8_unchecked(bytes) }",
            vec![],
        );
        let match_ok = site_with_family(
            OperationFamily::StrFromUtf8Unchecked,
            vec!["match core::str::from_utf8(bytes) {", "    Ok(_) => {"],
            "unsafe { core::str::from_utf8_unchecked(bytes) }",
            vec!["}", "Err(_) => \"\",", "}"],
        );

        let checked_evidence = obligation_evidence(&checked, &obligations, &contract, &reach);
        let return_evidence = obligation_evidence(&early_return, &obligations, &contract, &reach);
        let if_let_err_return_evidence =
            obligation_evidence(&if_let_err_return, &obligations, &contract, &reach);
        let question_mark_evidence =
            obligation_evidence(&question_mark, &obligations, &contract, &reach);
        let match_return_evidence =
            obligation_evidence(&match_return, &obligations, &contract, &reach);
        let if_let_ok_evidence = obligation_evidence(&if_let_ok, &obligations, &contract, &reach);
        let let_else_ok_evidence =
            obligation_evidence(&let_else_ok, &obligations, &contract, &reach);
        let match_ok_evidence = obligation_evidence(&match_ok, &obligations, &contract, &reach);

        assert!(checked_evidence[0].discharge.present);
        assert!(return_evidence[0].discharge.present);
        assert!(if_let_err_return_evidence[0].discharge.present);
        assert!(question_mark_evidence[0].discharge.present);
        assert!(match_return_evidence[0].discharge.present);
        assert!(if_let_ok_evidence[0].discharge.present);
        assert!(let_else_ok_evidence[0].discharge.present);
        assert!(match_ok_evidence[0].discharge.present);
    }

    #[test]
    fn from_utf8_validation_requires_same_buffer_and_returning_branch() {
        let obligations = vec![SafetyObligation::new("utf8", "bytes are valid UTF-8")];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let wrong_buffer = site_with_family(
            OperationFamily::StrFromUtf8Unchecked,
            vec!["if core::str::from_utf8(other).is_ok() {"],
            "unsafe { core::str::from_utf8_unchecked(bytes) }",
            vec!["}"],
        );
        let wrong_buffer_question_mark = site_with_family(
            OperationFamily::StrFromUtf8Unchecked,
            vec!["core::str::from_utf8(other)?;"],
            "unsafe { core::str::from_utf8_unchecked(bytes) }",
            vec![],
        );
        let wrong_buffer_match = site_with_family(
            OperationFamily::StrFromUtf8Unchecked,
            vec![
                "match core::str::from_utf8(other) {",
                "    Ok(_) => {}",
                "    Err(err) => return Err(err),",
                "}",
            ],
            "unsafe { core::str::from_utf8_unchecked(bytes) }",
            vec![],
        );
        let observed_is_ok = site_with_family(
            OperationFamily::StrFromUtf8Unchecked,
            vec!["let _valid = core::str::from_utf8(bytes).is_ok();"],
            "unsafe { core::str::from_utf8_unchecked(bytes) }",
            vec![],
        );
        let closed_positive_branch = site_with_family(
            OperationFamily::StrFromUtf8Unchecked,
            vec![
                "if core::str::from_utf8(bytes).is_ok() {",
                "    observed_valid();",
                "}",
            ],
            "unsafe { core::str::from_utf8_unchecked(bytes) }",
            vec![],
        );
        let closed_if_let_branch = site_with_family(
            OperationFamily::StrFromUtf8Unchecked,
            vec![
                "if let Ok(_valid) = core::str::from_utf8(bytes) {",
                "    observed_valid();",
                "}",
            ],
            "unsafe { core::str::from_utf8_unchecked(bytes) }",
            vec![],
        );
        let non_returning_branch = site_with_family(
            OperationFamily::StrFromUtf8Unchecked,
            vec![
                "if core::str::from_utf8(bytes).is_err() {",
                "    log_invalid();",
                "}",
            ],
            "unsafe { core::str::from_utf8_unchecked(bytes) }",
            vec![],
        );
        let non_returning_if_let_err = site_with_family(
            OperationFamily::StrFromUtf8Unchecked,
            vec![
                "if let Err(_err) = core::str::from_utf8(bytes) {",
                "    log_invalid();",
                "}",
            ],
            "unsafe { core::str::from_utf8_unchecked(bytes) }",
            vec![],
        );
        let non_returning_match = site_with_family(
            OperationFamily::StrFromUtf8Unchecked,
            vec![
                "match core::str::from_utf8(bytes) {",
                "    Ok(_) => {}",
                "    Err(_) => log_invalid(),",
                "}",
            ],
            "unsafe { core::str::from_utf8_unchecked(bytes) }",
            vec![],
        );
        let reassigned_after_question_mark = site_with_family(
            OperationFamily::StrFromUtf8Unchecked,
            vec!["core::str::from_utf8(bytes)?;", "bytes = b\"\\xff\";"],
            "unsafe { core::str::from_utf8_unchecked(bytes) }",
            vec![],
        );
        let reassigned_after_early_return = site_with_family(
            OperationFamily::StrFromUtf8Unchecked,
            vec![
                "if core::str::from_utf8(bytes).is_err() {",
                "    return \"\";",
                "}",
                "bytes = b\"\\xff\";",
            ],
            "unsafe { core::str::from_utf8_unchecked(bytes) }",
            vec![],
        );
        let reassigned_after_if_let_err = site_with_family(
            OperationFamily::StrFromUtf8Unchecked,
            vec![
                "if let Err(_err) = core::str::from_utf8(bytes) {",
                "    return \"\";",
                "}",
                "bytes = b\"\\xff\";",
            ],
            "unsafe { core::str::from_utf8_unchecked(bytes) }",
            vec![],
        );
        let reassigned_after_match_return = site_with_family(
            OperationFamily::StrFromUtf8Unchecked,
            vec![
                "match core::str::from_utf8(bytes) {",
                "    Ok(_) => {}",
                "    Err(err) => return Err(err),",
                "}",
                "bytes = b\"\\xff\";",
            ],
            "unsafe { core::str::from_utf8_unchecked(bytes) }",
            vec![],
        );
        let reassigned_after_if_let = site_with_family(
            OperationFamily::StrFromUtf8Unchecked,
            vec![
                "if let Ok(_valid) = core::str::from_utf8(bytes) {",
                "    bytes = b\"\\xff\";",
            ],
            "unsafe { core::str::from_utf8_unchecked(bytes) }",
            vec!["}"],
        );
        let reassigned_after_let_else = site_with_family(
            OperationFamily::StrFromUtf8Unchecked,
            vec![
                "let Ok(_) = core::str::from_utf8(bytes) else {",
                "    return \"\";",
                "};",
                "bytes = b\"\\xff\";",
            ],
            "unsafe { core::str::from_utf8_unchecked(bytes) }",
            vec![],
        );
        let reassigned_in_match_ok = site_with_family(
            OperationFamily::StrFromUtf8Unchecked,
            vec![
                "match core::str::from_utf8(bytes) {",
                "    Ok(_) => {",
                "        bytes = b\"\\xff\";",
            ],
            "unsafe { core::str::from_utf8_unchecked(bytes) }",
            vec!["}", "Err(_) => \"\",", "}"],
        );

        let wrong_buffer_evidence =
            obligation_evidence(&wrong_buffer, &obligations, &contract, &reach);
        let wrong_buffer_question_mark_evidence =
            obligation_evidence(&wrong_buffer_question_mark, &obligations, &contract, &reach);
        let wrong_buffer_match_evidence =
            obligation_evidence(&wrong_buffer_match, &obligations, &contract, &reach);
        let non_returning_evidence =
            obligation_evidence(&non_returning_branch, &obligations, &contract, &reach);
        let non_returning_if_let_err_evidence =
            obligation_evidence(&non_returning_if_let_err, &obligations, &contract, &reach);
        let non_returning_match_evidence =
            obligation_evidence(&non_returning_match, &obligations, &contract, &reach);
        let observed_is_ok_evidence =
            obligation_evidence(&observed_is_ok, &obligations, &contract, &reach);
        let closed_positive_branch_evidence =
            obligation_evidence(&closed_positive_branch, &obligations, &contract, &reach);
        let closed_if_let_branch_evidence =
            obligation_evidence(&closed_if_let_branch, &obligations, &contract, &reach);
        let reassigned_after_question_mark_evidence = obligation_evidence(
            &reassigned_after_question_mark,
            &obligations,
            &contract,
            &reach,
        );
        let reassigned_after_early_return_evidence = obligation_evidence(
            &reassigned_after_early_return,
            &obligations,
            &contract,
            &reach,
        );
        let reassigned_after_if_let_err_evidence = obligation_evidence(
            &reassigned_after_if_let_err,
            &obligations,
            &contract,
            &reach,
        );
        let reassigned_after_match_return_evidence = obligation_evidence(
            &reassigned_after_match_return,
            &obligations,
            &contract,
            &reach,
        );
        let reassigned_after_if_let_evidence =
            obligation_evidence(&reassigned_after_if_let, &obligations, &contract, &reach);
        let reassigned_after_let_else_evidence =
            obligation_evidence(&reassigned_after_let_else, &obligations, &contract, &reach);
        let reassigned_in_match_ok_evidence =
            obligation_evidence(&reassigned_in_match_ok, &obligations, &contract, &reach);

        assert!(!wrong_buffer_evidence[0].discharge.present);
        assert!(!wrong_buffer_question_mark_evidence[0].discharge.present);
        assert!(!wrong_buffer_match_evidence[0].discharge.present);
        assert!(!non_returning_evidence[0].discharge.present);
        assert!(!non_returning_if_let_err_evidence[0].discharge.present);
        assert!(!non_returning_match_evidence[0].discharge.present);
        assert!(!observed_is_ok_evidence[0].discharge.present);
        assert!(!closed_positive_branch_evidence[0].discharge.present);
        assert!(!closed_if_let_branch_evidence[0].discharge.present);
        assert!(!reassigned_after_question_mark_evidence[0].discharge.present);
        assert!(!reassigned_after_early_return_evidence[0].discharge.present);
        assert!(!reassigned_after_if_let_err_evidence[0].discharge.present);
        assert!(!reassigned_after_match_return_evidence[0].discharge.present);
        assert!(!reassigned_after_if_let_evidence[0].discharge.present);
        assert!(!reassigned_after_let_else_evidence[0].discharge.present);
        assert!(!reassigned_in_match_ok_evidence[0].discharge.present);
    }

    #[test]
    fn zeroed_known_valid_zero_types_discharge_valid_zero_obligation() {
        let obligations = vec![SafetyObligation::new(
            "valid-zero",
            "all-zero bit pattern is a valid value for the target type",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };

        for target_type in [
            "()", "bool", "char", "f32", "f64", "i8", "i16", "i32", "i64", "i128", "isize", "u8",
            "u16", "u32", "u64", "u128", "usize",
        ] {
            let expression = format!("unsafe {{ core::mem::zeroed::<{target_type}>() }}");
            let zeroed = site_with_family(
                OperationFamily::Zeroed,
                vec!["pub fn zero_value() {"],
                &expression,
                vec![],
            );

            let evidence = obligation_evidence(&zeroed, &obligations, &contract, &reach);

            assert!(
                evidence[0].discharge.present,
                "{target_type} should be recognized as a known valid-zero target"
            );
        }
    }

    #[test]
    fn zeroed_nonnull_target_keeps_valid_zero_obligation_missing() {
        let obligations = vec![SafetyObligation::new(
            "valid-zero",
            "all-zero bit pattern is a valid value for the target type",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let zeroed = site_with_family(
            OperationFamily::Zeroed,
            vec!["pub fn invalid_zeroed_nonnull() -> core::ptr::NonNull<u8> {"],
            "unsafe { core::mem::zeroed::<core::ptr::NonNull<u8>>() }",
            vec![],
        );

        let evidence = obligation_evidence(&zeroed, &obligations, &contract, &reach);

        assert!(!evidence[0].discharge.present);
    }

    #[test]
    fn transmute_size_equality_discharges_layout_obligation_only() {
        let obligations = vec![
            SafetyObligation::new("layout", "source and destination layouts are compatible"),
            SafetyObligation::new(
                "valid-value",
                "destination value satisfies Rust validity rules",
            ),
        ];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let transmute = site_with_family(
            OperationFamily::Transmute,
            vec!["assert_eq!(core::mem::size_of::<u8>(), core::mem::size_of::<bool>());"],
            "unsafe { core::mem::transmute::<u8, bool>(value) }",
            vec![],
        );

        let evidence = obligation_evidence(&transmute, &obligations, &contract, &reach);

        assert!(evidence[0].discharge.present);
        assert!(!evidence[1].discharge.present);
    }

    #[test]
    fn transmute_copy_size_equality_discharges_layout_obligation_only() {
        let obligations = vec![
            SafetyObligation::new("layout", "source and destination layouts are compatible"),
            SafetyObligation::new(
                "valid-value",
                "destination value satisfies Rust validity rules",
            ),
        ];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let transmute = site_with_family(
            OperationFamily::Transmute,
            vec!["debug_assert_eq!(core::mem::size_of::<u8>(), core::mem::size_of::<bool>());"],
            "unsafe { core::mem::transmute_copy::<u8, bool>(&value) }",
            vec![],
        );

        let evidence = obligation_evidence(&transmute, &obligations, &contract, &reach);

        assert!(evidence[0].discharge.present);
        assert!(!evidence[1].discharge.present);
    }

    #[test]
    fn transmute_size_equality_requires_matching_type_pair() {
        let obligations = vec![SafetyObligation::new(
            "layout",
            "source and destination layouts are compatible",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let transmute = site_with_family(
            OperationFamily::Transmute,
            vec!["assert_eq!(core::mem::size_of::<u8>(), core::mem::size_of::<u16>());"],
            "unsafe { core::mem::transmute::<u8, bool>(value) }",
            vec![],
        );

        let evidence = obligation_evidence(&transmute, &obligations, &contract, &reach);

        assert!(!evidence[0].discharge.present);
    }

    #[test]
    fn transmute_size_equality_must_precede_call() {
        let obligations = vec![SafetyObligation::new(
            "layout",
            "source and destination layouts are compatible",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let transmute = site_with_family(
            OperationFamily::Transmute,
            vec![],
            "unsafe { core::mem::transmute::<u8, bool>(value) }",
            vec!["assert_eq!(core::mem::size_of::<u8>(), core::mem::size_of::<bool>());"],
        );

        let evidence = obligation_evidence(&transmute, &obligations, &contract, &reach);

        assert!(!evidence[0].discharge.present);
    }

    #[test]
    fn transmute_u8_bool_guard_discharges_valid_value_obligation_only() {
        let obligations = vec![
            SafetyObligation::new("layout", "source and destination layouts are compatible"),
            SafetyObligation::new(
                "valid-value",
                "destination value satisfies Rust validity rules",
            ),
        ];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let transmute = site_with_family(
            OperationFamily::Transmute,
            vec!["assert!(value <= 1);"],
            "unsafe { core::mem::transmute::<u8, bool>(value) }",
            vec![],
        );
        let branch_scoped_transmute = site_with_family(
            OperationFamily::Transmute,
            vec!["if value <= 1 {"],
            "unsafe { core::mem::transmute::<u8, bool>(value) }",
            vec!["}"],
        );

        let evidence = obligation_evidence(&transmute, &obligations, &contract, &reach);
        let branch_scoped_evidence =
            obligation_evidence(&branch_scoped_transmute, &obligations, &contract, &reach);

        assert!(!evidence[0].discharge.present);
        assert!(evidence[1].discharge.present);
        assert!(branch_scoped_evidence[1].discharge.present);
    }

    #[test]
    fn transmute_copy_u8_bool_guard_discharges_valid_value_obligation() {
        let obligations = vec![SafetyObligation::new(
            "valid-value",
            "destination value satisfies Rust validity rules",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let transmute = site_with_family(
            OperationFamily::Transmute,
            vec!["assert!(value <= 1);"],
            "unsafe { core::mem::transmute_copy::<u8, bool>(&value) }",
            vec![],
        );

        let evidence = obligation_evidence(&transmute, &obligations, &contract, &reach);

        assert!(evidence[0].discharge.present);
    }

    #[test]
    fn transmute_u8_bool_guard_requires_same_argument_and_preceding_guard() {
        let obligations = vec![SafetyObligation::new(
            "valid-value",
            "destination value satisfies Rust validity rules",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let other_arg = site_with_family(
            OperationFamily::Transmute,
            vec!["assert!(other <= 1);"],
            "unsafe { core::mem::transmute::<u8, bool>(value) }",
            vec![],
        );
        let post_call_guard = site_with_family(
            OperationFamily::Transmute,
            vec![],
            "unsafe { core::mem::transmute::<u8, bool>(value) }",
            vec!["assert!(value <= 1);"],
        );
        let unsupported_pair = site_with_family(
            OperationFamily::Transmute,
            vec!["assert!(value <= 1);"],
            "unsafe { core::mem::transmute::<u8, char>(value) }",
            vec![],
        );
        let observed_predicate = site_with_family(
            OperationFamily::Transmute,
            vec!["let _valid_bool_byte = value <= 1;"],
            "unsafe { core::mem::transmute::<u8, bool>(value) }",
            vec![],
        );
        let observed_referenced_predicate = site_with_family(
            OperationFamily::Transmute,
            vec!["let _valid_bool_byte = value <= 1;"],
            "unsafe { core::mem::transmute_copy::<u8, bool>(&value) }",
            vec![],
        );
        let closed_positive_branch = site_with_family(
            OperationFamily::Transmute,
            vec!["if value <= 1 {", "let _observed = value;", "}"],
            "unsafe { core::mem::transmute::<u8, bool>(value) }",
            vec![],
        );
        let closed_referenced_positive_branch = site_with_family(
            OperationFamily::Transmute,
            vec!["if value <= 1 {", "let _observed = value;", "}"],
            "unsafe { core::mem::transmute_copy::<u8, bool>(&value) }",
            vec![],
        );
        let reassigned_after_assert = site_with_family(
            OperationFamily::Transmute,
            vec!["assert!(value <= 1);", "value = 2;"],
            "unsafe { core::mem::transmute::<u8, bool>(value) }",
            vec![],
        );
        let referenced_reassigned_after_assert = site_with_family(
            OperationFamily::Transmute,
            vec!["assert!(value <= 1);", "value = 2;"],
            "unsafe { core::mem::transmute_copy::<u8, bool>(&value) }",
            vec![],
        );
        let reassigned_after_early_return = site_with_family(
            OperationFamily::Transmute,
            vec!["if value > 1 {", "return false;", "}", "value = 2;"],
            "unsafe { core::mem::transmute::<u8, bool>(value) }",
            vec![],
        );

        let other_arg_evidence = obligation_evidence(&other_arg, &obligations, &contract, &reach);
        let post_call_evidence =
            obligation_evidence(&post_call_guard, &obligations, &contract, &reach);
        let unsupported_pair_evidence =
            obligation_evidence(&unsupported_pair, &obligations, &contract, &reach);
        let observed_predicate_evidence =
            obligation_evidence(&observed_predicate, &obligations, &contract, &reach);
        let observed_referenced_predicate_evidence = obligation_evidence(
            &observed_referenced_predicate,
            &obligations,
            &contract,
            &reach,
        );
        let closed_positive_branch_evidence =
            obligation_evidence(&closed_positive_branch, &obligations, &contract, &reach);
        let closed_referenced_positive_branch_evidence = obligation_evidence(
            &closed_referenced_positive_branch,
            &obligations,
            &contract,
            &reach,
        );
        let reassigned_after_assert_evidence =
            obligation_evidence(&reassigned_after_assert, &obligations, &contract, &reach);
        let referenced_reassigned_after_assert_evidence = obligation_evidence(
            &referenced_reassigned_after_assert,
            &obligations,
            &contract,
            &reach,
        );
        let reassigned_after_early_return_evidence = obligation_evidence(
            &reassigned_after_early_return,
            &obligations,
            &contract,
            &reach,
        );

        assert!(!other_arg_evidence[0].discharge.present);
        assert!(!post_call_evidence[0].discharge.present);
        assert!(!unsupported_pair_evidence[0].discharge.present);
        assert!(!observed_predicate_evidence[0].discharge.present);
        assert!(!observed_referenced_predicate_evidence[0].discharge.present);
        assert!(!closed_positive_branch_evidence[0].discharge.present);
        assert!(
            !closed_referenced_positive_branch_evidence[0]
                .discharge
                .present
        );
        assert!(!reassigned_after_assert_evidence[0].discharge.present);
        assert!(
            !referenced_reassigned_after_assert_evidence[0]
                .discharge
                .present
        );
        assert!(!reassigned_after_early_return_evidence[0].discharge.present);
    }

    #[test]
    fn transmute_u8_bool_early_return_guard_discharges_valid_value_obligation() {
        let obligations = vec![SafetyObligation::new(
            "valid-value",
            "destination value satisfies Rust validity rules",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let transmute = site_with_family(
            OperationFamily::Transmute,
            vec!["if value > 1 {", "    return false;", "}"],
            "unsafe { core::mem::transmute::<u8, bool>(value) }",
            vec![],
        );

        let evidence = obligation_evidence(&transmute, &obligations, &contract, &reach);

        assert!(evidence[0].discharge.present);
    }

    #[test]
    fn transmute_copy_u8_bool_early_return_guard_discharges_valid_value_obligation() {
        let obligations = vec![SafetyObligation::new(
            "valid-value",
            "destination value satisfies Rust validity rules",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let transmute = site_with_family(
            OperationFamily::Transmute,
            vec!["if value > 1 {", "    return false;", "}"],
            "unsafe { core::mem::transmute_copy::<u8, bool>(&value) }",
            vec![],
        );

        let evidence = obligation_evidence(&transmute, &obligations, &contract, &reach);

        assert!(evidence[0].discharge.present);
    }

    #[test]
    fn transmute_u8_bool_early_return_guard_requires_returning_branch() {
        let obligations = vec![SafetyObligation::new(
            "valid-value",
            "destination value satisfies Rust validity rules",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let transmute = site_with_family(
            OperationFamily::Transmute,
            vec!["if value > 1 {", "    log_invalid();", "}"],
            "unsafe { core::mem::transmute::<u8, bool>(value) }",
            vec![],
        );

        let evidence = obligation_evidence(&transmute, &obligations, &contract, &reach);

        assert!(!evidence[0].discharge.present);
    }

    #[test]
    fn unreachable_unchecked_infallible_path_discharges_unreachable_obligation() {
        let obligations = vec![SafetyObligation::new(
            "unreachable",
            "control flow cannot reach this path before `unreachable_unchecked`",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let unreachable = site_with_family(
            OperationFamily::UnreachableUnchecked,
            vec![
                "match fallible_with_capacity(Fallibility::Infallible) {",
                "    Ok(value) => value,",
                "    // SAFETY: infallible mode handles allocation errors before this point.",
            ],
            "Err(_) => unsafe { hint::unreachable_unchecked() },",
            vec![],
        );

        let evidence = obligation_evidence(&unreachable, &obligations, &contract, &reach);

        assert!(evidence[0].discharge.present);
    }

    #[test]
    fn unreachable_unchecked_evidence_requires_infallible_context() {
        let obligations = vec![SafetyObligation::new(
            "unreachable",
            "control flow cannot reach this path before `unreachable_unchecked`",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let unreachable = site_with_family(
            OperationFamily::UnreachableUnchecked,
            vec!["match fallible_with_capacity(Fallibility::Fallible) {"],
            "Err(_) => unsafe { hint::unreachable_unchecked() },",
            vec![],
        );

        let evidence = obligation_evidence(&unreachable, &obligations, &contract, &reach);

        assert!(!evidence[0].discharge.present);
    }

    #[test]
    fn unreachable_unchecked_infallible_path_must_match_arm_context() {
        let obligations = vec![SafetyObligation::new(
            "unreachable",
            "control flow cannot reach this path before `unreachable_unchecked`",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let unreachable = site_with_family(
            OperationFamily::UnreachableUnchecked,
            vec![
                "let _other = allocate(Fallibility::Infallible);",
                "match allocate(Fallibility::Fallible) {",
                "    Ok(value) => value,",
                "    // SAFETY: this fixture intentionally makes a different call infallible.",
            ],
            "Err(_) => unsafe { hint::unreachable_unchecked() },",
            vec![],
        );
        let post_infallible = site_with_family(
            OperationFamily::UnreachableUnchecked,
            vec![
                "match allocate(Fallibility::Fallible) {",
                "    Ok(value) => value,",
                "    // SAFETY: this fixture intentionally makes a later call infallible.",
            ],
            "Err(_) => unsafe { hint::unreachable_unchecked() },",
            vec!["};", "let _after = allocate(Fallibility::Infallible);"],
        );
        let closed_infallible = site_with_family(
            OperationFamily::UnreachableUnchecked,
            vec![
                "let _observed = match allocate(Fallibility::Infallible) {",
                "    Ok(value) => value,",
                "    Err(_) => 0,",
                "};",
            ],
            "unsafe { hint::unreachable_unchecked() }",
            vec![],
        );

        let evidence = obligation_evidence(&unreachable, &obligations, &contract, &reach);
        let post_evidence = obligation_evidence(&post_infallible, &obligations, &contract, &reach);
        let closed_evidence =
            obligation_evidence(&closed_infallible, &obligations, &contract, &reach);

        assert!(!evidence[0].discharge.present);
        assert!(!post_evidence[0].discharge.present);
        assert!(!closed_evidence[0].discharge.present);
    }

    #[test]
    fn summarize_discharge_distinguishes_none_some_and_all_present() {
        let missing = EvidenceState::missing("missing");
        let present = EvidenceState::present("present");
        let base = |key: &str, discharge: EvidenceState| ObligationEvidence {
            obligation: SafetyObligation::new(key, "obligation"),
            contract: EvidenceState::present("contract"),
            discharge,
            reach: EvidenceState::present("reach"),
            witness: EvidenceState::missing("witness"),
        };

        assert!(!summarize_discharge(&[]).present);
        assert!(!summarize_discharge(&[base("bounds", missing.clone())]).present);
        assert!(summarize_discharge(&[base("bounds", present.clone())]).present);
        let partial = summarize_discharge(&[base("bounds", present), base("alignment", missing)]);
        assert!(!partial.present);
        assert!(partial.summary.contains("Some inferred"));
    }

    #[test]
    fn reach_evidence_finds_unit_and_integration_tests_by_owner_name() -> Result<(), String> {
        let root = unique_temp_dir()?;
        fs::create_dir_all(root.join("src")).map_err(|err| err.to_string())?;
        fs::create_dir_all(root.join("tests")).map_err(|err| err.to_string())?;
        fs::write(
            root.join("src/reach_test.rs"),
            r#"
#[cfg(test)]
mod tests {
    #[test]
    fn reaches_read_one_in_unit_test() {
        read_one();
    }
}
"#,
        )
        .map_err(|err| err.to_string())?;
        fs::write(
            root.join("tests/reach.rs"),
            r#"
#[test]
fn reaches_read_one_in_integration_test() {
    unsafe_review_fixture::read_one();
}
"#,
        )
        .map_err(|err| err.to_string())?;
        let owner = "read_one".to_string();

        let (reach, related_tests) = reach_evidence(&root, Some(&owner));

        fs::remove_dir_all(&root).map_err(|err| err.to_string())?;
        assert_eq!(reach.state, "owner_reached");
        assert_eq!(related_tests.len(), 2);
        assert!(
            related_tests
                .iter()
                .any(|test| test.name == "reaches_read_one_in_unit_test")
        );
        assert!(
            related_tests
                .iter()
                .any(|test| test.file == "tests/reach.rs"
                    && test.name == "reaches_read_one_in_integration_test")
        );
        Ok(())
    }

    #[test]
    fn reach_evidence_reports_unknown_when_owner_is_missing() {
        let (reach, related_tests) = reach_evidence(Path::new("."), None);

        assert_eq!(reach.state, "unknown");
        assert!(related_tests.is_empty());
    }

    fn unique_temp_dir() -> Result<PathBuf, String> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| err.to_string())?
            .as_nanos();
        let root = std::env::temp_dir().join(format!("unsafe-review-evidence-test-{nanos}"));
        fs::create_dir_all(&root).map_err(|err| err.to_string())?;
        Ok(root)
    }
}
