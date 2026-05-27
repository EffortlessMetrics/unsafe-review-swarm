mod box_raw_origin;
mod call_syntax;
mod control_flow;
mod copy_range;
mod freshness;
mod generic_bounds;
mod get_unchecked;
mod identifier_syntax;
mod maybeuninit;
mod nonnull;
mod option_state;
mod pointer_arithmetic;
mod raw_pointer_alignment;
mod raw_pointer_bounds;
mod receiver_path;
mod set_len;
mod source_value;
mod transmute;
mod u8_bool_value;
mod unreachable_unchecked;
mod unsafe_fn_call;
mod unwrap_unchecked;
mod utf8;
mod vec_from_raw_parts;
mod write_bytes;
mod zeroed;

use self::box_raw_origin::{
    has_box_from_raw_origin_evidence, has_drop_in_place_box_origin_evidence,
};
use self::call_syntax::{
    matching_call_argument_end, matching_generic_argument_end, split_top_level_arguments,
    split_top_level_pair,
};
use self::control_flow::{
    branch_still_open_at_operation, compact_if_guards, matching_code_block_end,
};
use self::copy_range::has_copy_slice_range_evidence;
use self::freshness::{
    has_assignment_to_any_identifier, has_assignment_to_identifier, has_fresh_guard_pattern,
    has_fresh_guard_pattern_for_identifiers, has_open_positive_branch_guard_for_identifiers,
};
use self::generic_bounds::has_length_or_bounds_guard;
use self::get_unchecked::{get_unchecked_receiver_and_index, has_get_unchecked_bounds_guard};
use self::identifier_syntax::{is_simple_identifier, let_binding_name};
use self::maybeuninit::has_maybeuninit_assume_init_initialization_evidence;
use self::nonnull::has_nullability_guard;
use self::option_state::{ends_with_some_pattern, is_some_binding, match_some_branch_after_marker};
use self::pointer_arithmetic::has_slice_end_pointer_arithmetic_evidence;
use self::raw_pointer_alignment::has_alignment_guard;
use self::raw_pointer_bounds::has_raw_pointer_read_bounds_evidence;
use self::receiver_path::{
    contains_receiver_fragment, contains_receiver_path, is_receiver_path_char,
    receiver_before_marker,
};
use self::source_value::source_value_identifier;
use self::transmute::{
    has_transmute_layout_size_evidence, has_transmute_u8_bool_valid_value_evidence,
};
use self::u8_bool_value::{has_u8_bool_value_guard, u8_bool_valid_value_predicates};
use self::unreachable_unchecked::has_unreachable_unchecked_infallible_path_evidence;
use self::unsafe_fn_call::{
    has_encode_utf8_remaining_capacity_evidence, has_unchecked_constructor_availability_evidence,
};
use self::unwrap_unchecked::{
    has_unwrap_unchecked_infallible_result_evidence, has_unwrap_unchecked_receiver_state_evidence,
};
use self::utf8::has_from_utf8_unchecked_validation_evidence;
use self::vec_from_raw_parts::{
    has_vec_from_raw_parts_capacity_evidence, has_vec_from_raw_parts_origin_evidence,
    has_vec_from_raw_parts_origin_initialized_evidence,
    has_vec_from_raw_parts_origin_len_cap_evidence,
    has_vec_from_raw_parts_origin_pointer_live_evidence,
};
use self::write_bytes::{
    has_bool_write_bytes_pointer_context, has_bool_write_bytes_value_evidence,
    has_maybeuninit_raw_write_context, has_maybeuninit_slice_context, has_u8_write_bytes_context,
    has_write_bytes_bounds_evidence,
};
use self::zeroed::has_zeroed_known_valid_zero_type;
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
                && set_len::has_set_len_initialization_evidence(init_scope)
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

fn has_capacity_guard(family: &OperationFamily, lower: &str) -> bool {
    if family == &OperationFamily::VecSetLen {
        return set_len::has_set_len_capacity_evidence(lower);
    }
    if family == &OperationFamily::VecFromRawParts {
        return false;
    }
    lower.contains("capacity") || lower.contains("cap()")
}

fn compact_code(lower: &str) -> String {
    lower
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace())
        .collect()
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
