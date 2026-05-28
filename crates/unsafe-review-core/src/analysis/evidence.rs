mod alignment_discharge;
mod assignment_syntax;
mod bounds_discharge;
mod box_raw_origin;
mod call_syntax;
mod callee_contract_discharge;
mod capacity_discharge;
mod code_text;
mod contract_discharge;
mod contract_text;
mod control_flow;
mod copy_range;
mod evidence_state;
mod freshness;
mod generic_bounds;
mod get_unchecked;
mod identifier_syntax;
mod initialized_discharge;
mod layout_discharge;
mod marker_scan;
mod maybeuninit;
mod nonnull;
mod obligation_guard;
mod operation_scope;
mod option_state;
mod ownership_discharge;
mod pointer_arithmetic;
mod pointer_live_discharge;
mod raw_pointer_alignment;
mod raw_pointer_bounds;
mod reach_scan;
mod receiver_path;
mod set_len;
mod site_context;
mod source_value;
mod target_feature_discharge;
mod transmute;
mod u8_bool_value;
mod unreachable_discharge;
mod unreachable_unchecked;
mod unsafe_fn_call;
mod unwrap_unchecked;
mod utf8;
mod utf8_discharge;
mod valid_value_discharge;
mod valid_zero_discharge;
mod vec_from_raw_parts;
mod write_bytes;
mod zeroed;

use self::alignment_discharge::alignment_discharge_state;
use self::assignment_syntax::contains_simple_assignment_to;
use self::bounds_discharge::bounds_discharge_state;
use self::call_syntax::{
    matching_call_argument_end, matching_generic_argument_end, split_top_level_arguments,
    split_top_level_pair,
};
use self::callee_contract_discharge::callee_contract_discharge_state;
use self::capacity_discharge::capacity_discharge_state;
use self::code_text::{
    compact_code, compact_contains_identifier, contains_executable_return,
    strip_block_comments_and_literals,
};
use self::contract_discharge::{
    DOCUMENTED_PRIVATE_UNSAFE_CONTRACT_DISCHARGE, PUBLIC_UNSAFE_API_CONTRACT_DISCHARGE,
    is_documented_private_unsafe_contract_obligation, is_public_unsafe_contract_obligation,
};
pub(crate) use self::contract_text::contract_evidence;
use self::control_flow::{
    branch_still_open_at_operation, compact_if_guards, matching_code_block_end,
};
use self::copy_range::has_copy_slice_range_evidence;
pub(crate) use self::evidence_state::summarize_discharge;
use self::evidence_state::{contract_state, reach_state};
use self::freshness::{
    has_assignment_to_any_identifier, has_assignment_to_identifier, has_fresh_guard_pattern,
    has_fresh_guard_pattern_for_identifiers, has_open_positive_branch_guard_for_identifiers,
};
use self::generic_bounds::has_length_or_bounds_guard;
use self::get_unchecked::{get_unchecked_receiver_and_index, has_get_unchecked_bounds_guard};
use self::identifier_syntax::{is_simple_identifier, let_binding_name};
use self::initialized_discharge::initialized_discharge_state;
use self::layout_discharge::layout_discharge_state;
use self::marker_scan::{any_marker_occurrence, any_marker_tail};
use self::operation_scope::code_before_operation;
use self::option_state::{ends_with_some_pattern, is_some_binding, match_some_branch_after_marker};
use self::ownership_discharge::ownership_discharge_state;
use self::pointer_live_discharge::pointer_live_discharge_state;
use self::raw_pointer_bounds::has_raw_pointer_read_bounds_evidence;
pub(crate) use self::reach_scan::reach_evidence;
use self::receiver_path::{
    contains_receiver_fragment, contains_receiver_path, is_receiver_path_char,
    receiver_before_marker,
};
use self::site_context::{code_context, code_context_through_site};
use self::source_value::source_value_identifier;
use self::target_feature_discharge::target_feature_discharge_state;
use self::u8_bool_value::{has_u8_bool_value_guard, u8_bool_valid_value_predicates};
use self::unreachable_discharge::unreachable_discharge_state;
use self::utf8_discharge::utf8_discharge_state;
use self::valid_value_discharge::valid_value_discharge_state;
use self::valid_zero_discharge::valid_zero_discharge_state;
use self::write_bytes::has_write_bytes_bounds_evidence;
use crate::analysis::scanner::ScannedSite;
use crate::domain::{
    ContractEvidence, EvidenceState, ObligationEvidence, OperationFamily, ReachEvidence,
    SafetyObligation,
};

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
        "alignment" => alignment_discharge_state(site, lower),
        "bounds" | "valid-range" => bounds_discharge_state(site, lower),
        "capacity" => capacity_discharge_state(site, lower),
        "initialized" => initialized_discharge_state(site, lower),
        "non-null" | "pointer-live" => pointer_live_discharge_state(site, lower),
        "ownership" => ownership_discharge_state(family, &site.operation.expression, lower),
        "callee-contract" => {
            callee_contract_discharge_state(family, &site.operation.expression, lower)
        }
        "valid-value" => valid_value_discharge_state(family, lower),
        "layout" => layout_discharge_state(family, lower),
        "unreachable" => unreachable_discharge_state(family, lower),
        "target-feature" => target_feature_discharge_state(family, contract),
        "utf8" => utf8_discharge_state(family, lower),
        "valid-zero" => valid_zero_discharge_state(family, lower),
        _ => EvidenceState::missing("No obligation-specific guard code was detected"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        OperationFamily, SourceLocation, UnsafeOperation, UnsafeSite, UnsafeSiteKind,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
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
        let nested_modulo_guard = site_with_context(
            vec![
                "if (ptr as usize) % core::mem::align_of::<Header>() != 0 {",
                "    if should_count() {",
                "        record_misaligned(ptr);",
                "    }",
                "    return None;",
                "}",
            ],
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
        let comment_return_guard = site_with_context(
            vec!["if !ptr.cast::<Header>().is_aligned() { /* return None; */ }"],
            "ptr.cast::<Header>().read()",
            vec![],
        );
        let string_return_guard = site_with_context(
            vec!["if !ptr.cast::<Header>().is_aligned() { let _note = \"return None\"; }"],
            "ptr.cast::<Header>().read()",
            vec![],
        );
        let line_comment_modulo_guard = site_with_context(
            vec!["// if (ptr as usize) % core::mem::align_of::<Header>() != 0 { return None; }"],
            "ptr.cast::<Header>().read()",
            vec![],
        );
        let line_comment_method_assertion = site_with_context(
            vec!["// assert!(ptr.cast::<Header>().is_aligned());"],
            "ptr.cast::<Header>().read()",
            vec![],
        );
        let line_comment_open_branch_guard = site_with_context(
            vec!["// if ptr.cast::<Header>().is_aligned() {"],
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
            obligation_evidence(&nested_modulo_guard, &obligations, &contract, &reach)[0]
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
            comment_return_guard,
            string_return_guard,
            line_comment_modulo_guard,
            line_comment_method_assertion,
            line_comment_open_branch_guard,
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
        let line_comment_question_mark_guard = site_with_family(
            OperationFamily::NonNullUnchecked,
            vec!["// NonNull::new(ptr)?;"],
            "NonNull::new_unchecked(ptr)",
            vec![],
        );
        let line_comment_if_let_guard = site_with_family(
            OperationFamily::NonNullUnchecked,
            vec!["// if let Some(_) = NonNull::new(ptr) {"],
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
            !obligation_evidence(
                &line_comment_question_mark_guard,
                &obligations,
                &contract,
                &reach,
            )[0]
            .discharge
            .present
        );
        assert!(
            !obligation_evidence(&line_comment_if_let_guard, &obligations, &contract, &reach,)[0]
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
        let nested_returning_guard = site_with_family(
            OperationFamily::NonNullUnchecked,
            vec!["if ptr.is_null() { log_null(); if should_count() { record(); } return None; }"],
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
        let comment_returning_guard = site_with_family(
            OperationFamily::NonNullUnchecked,
            vec!["if ptr.is_null() { /* return None; */ }"],
            "NonNull::new_unchecked(ptr)",
            vec![],
        );
        let line_comment_returning_guard = site_with_family(
            OperationFamily::NonNullUnchecked,
            vec!["if ptr.is_null() { // return None;", "    log_null();", "}"],
            "NonNull::new_unchecked(ptr)",
            vec![],
        );
        let string_returning_guard = site_with_family(
            OperationFamily::NonNullUnchecked,
            vec!["if ptr.is_null() { let _note = \"return None\"; }"],
            "NonNull::new_unchecked(ptr)",
            vec![],
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
            obligation_evidence(&nested_returning_guard, &obligations, &contract, &reach)[0]
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
        assert!(
            !obligation_evidence(&comment_returning_guard, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(
                &line_comment_returning_guard,
                &obligations,
                &contract,
                &reach,
            )[0]
            .discharge
            .present
        );
        assert!(
            !obligation_evidence(&string_returning_guard, &obligations, &contract, &reach)[0]
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
        let line_comment_slice_end = site_with_family(
            OperationFamily::PointerArithmetic,
            vec![
                "let start = haystack.as_ptr();",
                "// let end = start.add(haystack.len());",
            ],
            "let end = start.add(offset);",
            vec![],
        );
        let string_literal_slice_end = site_with_family(
            OperationFamily::PointerArithmetic,
            vec![
                "let start = haystack.as_ptr();",
                "let _note = \"let end = start.add(haystack.len())\";",
            ],
            "let end = start.add(offset);",
            vec![],
        );
        let generic_bounds_guard = site_with_family(
            OperationFamily::PointerArithmetic,
            vec!["assert!(offset < haystack.len());"],
            "let end = start.add(offset);",
            vec![],
        );
        let line_comment_bounds_guard = site_with_family(
            OperationFamily::PointerArithmetic,
            vec!["// assert!(offset < haystack.len());"],
            "let end = start.add(offset);",
            vec![],
        );
        let string_literal_bounds_guard = site_with_family(
            OperationFamily::PointerArithmetic,
            vec!["let _note = \"assert!(offset < haystack.len())\";"],
            "let end = start.add(offset);",
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
        assert!(
            !obligation_evidence(&line_comment_slice_end, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&string_literal_slice_end, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            obligation_evidence(&generic_bounds_guard, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&line_comment_bounds_guard, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(
                &string_literal_bounds_guard,
                &obligations,
                &contract,
                &reach
            )[0]
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
        let line_comment_origin = site_with_family(
            OperationFamily::DropInPlace,
            vec!["// let ptr = Box::into_raw(value);"],
            "core::ptr::drop_in_place(ptr);",
            vec![],
        );
        let string_literal_origin = site_with_family(
            OperationFamily::DropInPlace,
            vec!["let _note = \"let ptr = Box::into_raw(value);\";"],
            "core::ptr::drop_in_place(ptr);",
            vec![],
        );

        let evidence = obligation_evidence(&matching, &obligations, &contract, &reach);
        assert!(evidence.iter().all(|item| item.discharge.present));
        let evidence = obligation_evidence(&other_pointer, &obligations, &contract, &reach);
        assert!(evidence.iter().all(|item| !item.discharge.present));
        let evidence = obligation_evidence(&reassigned_pointer, &obligations, &contract, &reach);
        assert!(evidence.iter().all(|item| !item.discharge.present));
        let evidence = obligation_evidence(&line_comment_origin, &obligations, &contract, &reach);
        assert!(evidence.iter().all(|item| !item.discharge.present));
        let evidence = obligation_evidence(&string_literal_origin, &obligations, &contract, &reach);
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
        let line_comment_origin = site_with_family(
            OperationFamily::BoxFromRaw,
            vec!["// let ptr = Box::into_raw(value);"],
            "unsafe { Box::from_raw(ptr) }",
            vec![],
        );
        let string_literal_origin = site_with_family(
            OperationFamily::BoxFromRaw,
            vec!["let _note = \"let ptr = Box::into_raw(value);\";"],
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
        assert!(
            !obligation_evidence(&line_comment_origin, &obligations, &contract, &reach)[0]
                .discharge
                .present
        );
        assert!(
            !obligation_evidence(&string_literal_origin, &obligations, &contract, &reach)[0]
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
        let line_comment_shrink = site_with_family(
            OperationFamily::VecSetLen,
            vec!["// if new_len <= values.len() {"],
            "values.set_len(new_len);",
            vec![],
        );

        let evidence = obligation_evidence(&set_len, &obligations, &contract, &reach);
        let line_comment_evidence =
            obligation_evidence(&line_comment_shrink, &obligations, &contract, &reach);

        assert!(evidence.iter().all(|item| item.discharge.present));
        assert!(
            line_comment_evidence
                .iter()
                .all(|item| !item.discharge.present)
        );
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
        let nested_guarded = site_with_family(
            OperationFamily::VecSetLen,
            vec![
                "if s.len() > self.capacity() - self.len() {",
                "    if should_count() {",
                "        record_overflow(s.len());",
                "    }",
                "    return;",
                "}",
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
        let comment_return_guard = site_with_family(
            OperationFamily::VecSetLen,
            vec![
                "if s.len() > self.capacity() - self.len() { /* return; */ }",
                "let old_len = self.len();",
                "let new_len = old_len + s.len();",
            ],
            "self.set_len(new_len);",
            vec![],
        );
        let string_return_guard = site_with_family(
            OperationFamily::VecSetLen,
            vec![
                "if s.len() > self.capacity() - self.len() {",
                "    let _note = \"return\";",
                "}",
                "let old_len = self.len();",
                "let new_len = old_len + s.len();",
            ],
            "self.set_len(new_len);",
            vec![],
        );

        let guarded_evidence = obligation_evidence(&guarded, &obligations, &contract, &reach);
        let nested_guarded_evidence =
            obligation_evidence(&nested_guarded, &obligations, &contract, &reach);
        let other_receiver_evidence =
            obligation_evidence(&other_receiver_guard, &obligations, &contract, &reach);
        let stale_evidence =
            obligation_evidence(&stale_after_guard, &obligations, &contract, &reach);
        let comment_return_evidence =
            obligation_evidence(&comment_return_guard, &obligations, &contract, &reach);
        let string_return_evidence =
            obligation_evidence(&string_return_guard, &obligations, &contract, &reach);

        assert!(guarded_evidence[0].discharge.present);
        assert!(nested_guarded_evidence[0].discharge.present);
        assert!(!other_receiver_evidence[0].discharge.present);
        assert!(!stale_evidence[0].discharge.present);
        assert!(!comment_return_evidence[0].discharge.present);
        assert!(!string_return_evidence[0].discharge.present);
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
        let nested_capacity_guard = site_with_family(
            OperationFamily::VecSetLen,
            vec![
                "let old_len = self.len();",
                "let new_len = old_len + s.len();",
                "if new_len > self.capacity() {",
                "    if should_count() {",
                "        record_overflow(new_len);",
                "    }",
                "    return;",
                "}",
                "let dst = &mut self.xs[old_len..new_len];",
                "for (dst, src) in dst.iter_mut().zip(s.as_bytes().iter()) {",
                "    *dst = MaybeUninit::new(*src);",
                "}",
            ],
            "self.set_len(new_len);",
            vec![],
        );
        let line_comment_loop = site_with_family(
            OperationFamily::VecSetLen,
            vec![
                "let old_len = self.len();",
                "let new_len = old_len + s.len();",
                "if new_len > self.capacity() { return; }",
                "// for item in self.xs[old_len..new_len].iter_mut() {",
                "//     *item = MaybeUninit::new(0);",
                "// }",
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
        let comment_return_capacity_guard = site_with_family(
            OperationFamily::VecSetLen,
            vec![
                "let old_len = self.len();",
                "let new_len = old_len + s.len();",
                "if new_len > self.capacity() { /* return; */ }",
                "let dst = &mut self.xs[old_len..new_len];",
                "for (dst, src) in dst.iter_mut().zip(s.as_bytes().iter()) {",
                "    *dst = MaybeUninit::new(*src);",
                "}",
            ],
            "self.set_len(new_len);",
            vec![],
        );
        let string_return_capacity_guard = site_with_family(
            OperationFamily::VecSetLen,
            vec![
                "let old_len = self.len();",
                "let new_len = old_len + s.len();",
                "if new_len > self.capacity() {",
                "    let _note = \"return\";",
                "}",
                "let dst = &mut self.xs[old_len..new_len];",
                "for (dst, src) in dst.iter_mut().zip(s.as_bytes().iter()) {",
                "    *dst = MaybeUninit::new(*src);",
                "}",
            ],
            "self.set_len(new_len);",
            vec![],
        );

        let evidence = obligation_evidence(&set_len, &obligations, &contract, &reach);
        let nested_capacity_guard_evidence =
            obligation_evidence(&nested_capacity_guard, &obligations, &contract, &reach);
        let line_comment_loop_evidence =
            obligation_evidence(&line_comment_loop, &obligations, &contract, &reach);
        let wrong_target_evidence =
            obligation_evidence(&wrong_target, &obligations, &contract, &reach);
        let partial_range_evidence =
            obligation_evidence(&partial_range, &obligations, &contract, &reach);
        let comment_return_capacity_evidence = obligation_evidence(
            &comment_return_capacity_guard,
            &obligations,
            &contract,
            &reach,
        );
        let string_return_capacity_evidence = obligation_evidence(
            &string_return_capacity_guard,
            &obligations,
            &contract,
            &reach,
        );

        assert!(evidence.iter().all(|item| item.discharge.present));
        assert!(
            nested_capacity_guard_evidence
                .iter()
                .all(|item| item.discharge.present)
        );
        assert!(
            line_comment_loop_evidence
                .iter()
                .find(|item| item.obligation.key == "capacity")
                .unwrap()
                .discharge
                .present
        );
        assert!(
            !line_comment_loop_evidence
                .iter()
                .find(|item| item.obligation.key == "initialized")
                .unwrap()
                .discharge
                .present
        );
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
        assert!(
            !comment_return_capacity_evidence
                .iter()
                .find(|item| item.obligation.key == "capacity")
                .unwrap()
                .discharge
                .present
        );
        assert!(
            !string_return_capacity_evidence
                .iter()
                .find(|item| item.obligation.key == "capacity")
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
        let nested_unavailable_return_guard = site_with_family(
            OperationFamily::UnsafeFnCall,
            vec![
                "if !One::is_available() {",
                "    if should_count() {",
                "        record_unavailable();",
                "    }",
                "    return None;",
                "}",
            ],
            "unsafe { Some(One::new_unchecked(needle)) }",
            vec![],
        );
        let comment_unavailable_return_guard = site_with_family(
            OperationFamily::UnsafeFnCall,
            vec!["if !One::is_available() { /* return None; */ }"],
            "unsafe { Some(One::new_unchecked(needle)) }",
            vec![],
        );
        let string_unavailable_return_guard = site_with_family(
            OperationFamily::UnsafeFnCall,
            vec!["if !One::is_available() { let _note = \"return None\"; }"],
            "unsafe { Some(One::new_unchecked(needle)) }",
            vec![],
        );
        let line_comment_assert_guard = site_with_family(
            OperationFamily::UnsafeFnCall,
            vec!["// assert!(One::is_available());"],
            "unsafe { Some(One::new_unchecked(needle)) }",
            vec![],
        );
        let line_comment_open_guard = site_with_family(
            OperationFamily::UnsafeFnCall,
            vec!["// if One::is_available() {"],
            "unsafe { Some(One::new_unchecked(needle)) }",
            vec![],
        );
        let line_comment_unavailable_return_guard = site_with_family(
            OperationFamily::UnsafeFnCall,
            vec!["// if !One::is_available() { return None; }"],
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
        let nested_unavailable_return_evidence = obligation_evidence(
            &nested_unavailable_return_guard,
            &obligations,
            &contract,
            &reach,
        );
        let comment_unavailable_return_evidence = obligation_evidence(
            &comment_unavailable_return_guard,
            &obligations,
            &contract,
            &reach,
        );
        let string_unavailable_return_evidence = obligation_evidence(
            &string_unavailable_return_guard,
            &obligations,
            &contract,
            &reach,
        );
        let line_comment_assert_evidence =
            obligation_evidence(&line_comment_assert_guard, &obligations, &contract, &reach);
        let line_comment_open_evidence =
            obligation_evidence(&line_comment_open_guard, &obligations, &contract, &reach);
        let line_comment_unavailable_return_evidence = obligation_evidence(
            &line_comment_unavailable_return_guard,
            &obligations,
            &contract,
            &reach,
        );
        let unguarded_evidence = obligation_evidence(&unguarded, &obligations, &contract, &reach);

        assert!(guarded_evidence[0].discharge.present);
        assert!(assert_guarded_evidence[0].discharge.present);
        assert!(unavailable_return_evidence[0].discharge.present);
        assert!(nested_unavailable_return_evidence[0].discharge.present);
        assert!(!comment_unavailable_return_evidence[0].discharge.present);
        assert!(!string_unavailable_return_evidence[0].discharge.present);
        assert!(!line_comment_assert_evidence[0].discharge.present);
        assert!(!line_comment_open_evidence[0].discharge.present);
        assert!(
            !line_comment_unavailable_return_evidence[0]
                .discharge
                .present
        );
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
        let nested_short_buffer_guard = site_with_family(
            OperationFamily::RawPointerRead,
            vec![
                "if self.len() < core::mem::size_of::<[T; CAP]>() {",
                "    if should_count() {",
                "        record_short_buffer(self.len());",
                "    }",
                "    return None;",
                "}",
            ],
            "ptr::read(self.as_ptr() as *const [T; CAP])",
            vec![],
        );
        let comment_return_guard = site_with_family(
            OperationFamily::RawPointerRead,
            vec!["if self.len() < core::mem::size_of::<[T; CAP]>() { /* return None; */ }"],
            "ptr::read(self.as_ptr() as *const [T; CAP])",
            vec![],
        );
        let string_return_guard = site_with_family(
            OperationFamily::RawPointerRead,
            vec![
                "if self.len() < core::mem::size_of::<[T; CAP]>() {",
                "    let _note = \"return None\";",
                "}",
            ],
            "ptr::read(self.as_ptr() as *const [T; CAP])",
            vec![],
        );
        let line_comment_capacity_guard = site_with_family(
            OperationFamily::RawPointerRead,
            vec!["// debug_assert_eq!(self.len(), self.capacity());"],
            "ptr::read(self.as_ptr() as *const [T; CAP])",
            vec![],
        );
        let line_comment_size_return_guard = site_with_family(
            OperationFamily::RawPointerRead,
            vec![
                "// if self.len() < core::mem::size_of::<[T; CAP]>() {",
                "//     return None;",
                "// }",
            ],
            "ptr::read(self.as_ptr() as *const [T; CAP])",
            vec![],
        );

        let evidence = obligation_evidence(&raw_read, &obligations, &contract, &reach);
        let nested_guard_evidence =
            obligation_evidence(&nested_short_buffer_guard, &obligations, &contract, &reach);
        let comment_return_evidence =
            obligation_evidence(&comment_return_guard, &obligations, &contract, &reach);
        let string_return_evidence =
            obligation_evidence(&string_return_guard, &obligations, &contract, &reach);
        let line_comment_capacity_evidence = obligation_evidence(
            &line_comment_capacity_guard,
            &obligations,
            &contract,
            &reach,
        );
        let line_comment_size_return_evidence = obligation_evidence(
            &line_comment_size_return_guard,
            &obligations,
            &contract,
            &reach,
        );

        assert!(evidence[0].discharge.present);
        assert!(nested_guard_evidence[0].discharge.present);
        assert!(!comment_return_evidence[0].discharge.present);
        assert!(!string_return_evidence[0].discharge.present);
        assert!(!line_comment_capacity_evidence[0].discharge.present);
        assert!(!line_comment_size_return_evidence[0].discharge.present);
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
        let nested_matching_return_guard = site_with_family(
            OperationFamily::GetUnchecked,
            vec![
                "if index >= values.len() {",
                "    if should_count() { record(index); }",
                "    return None;",
                "}",
            ],
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
            obligation_evidence(
                &nested_matching_return_guard,
                &obligations,
                &contract,
                &reach
            )[0]
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
    fn maybeuninit_slice_evidence_ignores_comments_and_literals() {
        let obligations = vec![SafetyObligation::new(
            "initialized",
            "memory range is initialized",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let line_comment = site_with_family(
            OperationFamily::SliceFromRawParts,
            vec![
                "fn expose(ptr: *mut u8, len: usize) -> &mut [u8] {",
                "    // unsafe { core::slice::from_raw_parts_mut(ptr.cast::<core::mem::MaybeUninit<u8>>(), len) }",
            ],
            "unsafe { core::slice::from_raw_parts_mut(ptr, len) }",
            vec!["}"],
        );
        let string_literal = site_with_family(
            OperationFamily::SliceFromRawParts,
            vec![
                "fn expose(ptr: *mut u8, len: usize) -> &mut [u8] {",
                "    let _note = \"unsafe { core::slice::from_raw_parts_mut(ptr.cast::<core::mem::MaybeUninit<u8>>(), len) }\";",
            ],
            "unsafe { core::slice::from_raw_parts_mut(ptr, len) }",
            vec!["}"],
        );

        let line_comment_evidence =
            obligation_evidence(&line_comment, &obligations, &contract, &reach);
        let string_literal_evidence =
            obligation_evidence(&string_literal, &obligations, &contract, &reach);

        assert!(!line_comment_evidence[0].discharge.present);
        assert!(!string_literal_evidence[0].discharge.present);
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
    fn copy_range_bounds_require_same_source_destination_and_count() {
        let obligations = vec![SafetyObligation::new(
            "bounds",
            "copy source and destination ranges are in bounds",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let matching = site_with_family(
            OperationFamily::CopyNonOverlapping,
            vec![
                "fn copy(src: &[u8], dst: &mut [u8], count: usize) {",
                "    assert!(count <= src.len());",
                "    assert!(count <= dst.len());",
            ],
            "unsafe { core::ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr(), count) }",
            vec!["}"],
        );
        let ptr_copy_matching = site_with_family(
            OperationFamily::PtrCopy,
            vec![
                "fn copy(src: &[u8], dst: &mut [u8], count: usize) {",
                "    assert!(count <= src.len());",
                "    assert!(count <= dst.len());",
            ],
            "unsafe { core::ptr::copy(src.as_ptr(), dst.as_mut_ptr(), count) }",
            vec!["}"],
        );
        let generic_len_guard = site_with_family(
            OperationFamily::CopyNonOverlapping,
            vec![
                "fn copy(src: &[u8], dst: &mut [u8], len: usize, count: usize) {",
                "    assert!(count <= len);",
            ],
            "unsafe { core::ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr(), count) }",
            vec!["}"],
        );
        let wrong_destination = site_with_family(
            OperationFamily::CopyNonOverlapping,
            vec![
                "fn copy(src: &[u8], dst: &mut [u8], other: &[u8], count: usize) {",
                "    assert!(count <= src.len());",
                "    assert!(count <= other.len());",
            ],
            "unsafe { core::ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr(), count) }",
            vec!["}"],
        );
        let stale_count = site_with_family(
            OperationFamily::CopyNonOverlapping,
            vec![
                "fn copy(src: &[u8], dst: &mut [u8], mut count: usize) {",
                "    assert!(count <= src.len());",
                "    assert!(count <= dst.len());",
                "    count = adjusted_count();",
            ],
            "unsafe { core::ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr(), count) }",
            vec!["}"],
        );
        let stale_source = site_with_family(
            OperationFamily::CopyNonOverlapping,
            vec![
                "fn copy(mut src: &[u8], dst: &mut [u8], other: &[u8], count: usize) {",
                "    assert!(count <= src.len());",
                "    assert!(count <= dst.len());",
                "    src = other;",
            ],
            "unsafe { core::ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr(), count) }",
            vec!["}"],
        );
        let stale_destination = site_with_family(
            OperationFamily::CopyNonOverlapping,
            vec![
                "fn copy(src: &[u8], mut dst: &mut [u8], other: &mut [u8], count: usize) {",
                "    assert!(count <= src.len());",
                "    assert!(count <= dst.len());",
                "    dst = other;",
            ],
            "unsafe { core::ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr(), count) }",
            vec!["}"],
        );

        let matching_evidence = obligation_evidence(&matching, &obligations, &contract, &reach);
        let ptr_copy_matching_evidence =
            obligation_evidence(&ptr_copy_matching, &obligations, &contract, &reach);
        let generic_len_guard_evidence =
            obligation_evidence(&generic_len_guard, &obligations, &contract, &reach);
        let wrong_destination_evidence =
            obligation_evidence(&wrong_destination, &obligations, &contract, &reach);
        let stale_count_evidence =
            obligation_evidence(&stale_count, &obligations, &contract, &reach);
        let stale_source_evidence =
            obligation_evidence(&stale_source, &obligations, &contract, &reach);
        let stale_destination_evidence =
            obligation_evidence(&stale_destination, &obligations, &contract, &reach);

        assert!(matching_evidence[0].discharge.present);
        assert!(ptr_copy_matching_evidence[0].discharge.present);
        assert!(!generic_len_guard_evidence[0].discharge.present);
        assert!(!wrong_destination_evidence[0].discharge.present);
        assert!(!stale_count_evidence[0].discharge.present);
        assert!(!stale_source_evidence[0].discharge.present);
        assert!(!stale_destination_evidence[0].discharge.present);
    }

    #[test]
    fn copy_range_early_return_bounds_are_applicable_when_targets_stay_fresh() {
        let obligations = vec![SafetyObligation::new(
            "bounds",
            "copy source and destination ranges are in bounds",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let matching = site_with_family(
            OperationFamily::CopyNonOverlapping,
            vec![
                "fn copy(src: &[u8], dst: &mut [u8], count: usize) {",
                "    if count > src.len() {",
                "        return;",
                "    }",
                "    if count > dst.len() {",
                "        return;",
                "    }",
            ],
            "unsafe { core::ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr(), count) }",
            vec!["}"],
        );
        let stale_after_return_guard = site_with_family(
            OperationFamily::CopyNonOverlapping,
            vec![
                "fn copy(src: &[u8], dst: &mut [u8], mut count: usize) {",
                "    if count > src.len() {",
                "        return;",
                "    }",
                "    if count > dst.len() {",
                "        return;",
                "    }",
                "    count = adjusted_count();",
            ],
            "unsafe { core::ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr(), count) }",
            vec!["}"],
        );

        let matching_evidence = obligation_evidence(&matching, &obligations, &contract, &reach);
        let stale_after_return_guard_evidence =
            obligation_evidence(&stale_after_return_guard, &obligations, &contract, &reach);

        assert!(matching_evidence[0].discharge.present);
        assert!(!stale_after_return_guard_evidence[0].discharge.present);
    }

    #[test]
    fn bool_write_bytes_value_guard_discharges_initialized_obligation() {
        let obligations = vec![SafetyObligation::new(
            "initialized",
            "memory is initialized for the accessed type",
        )];
        let contract = ContractEvidence::present("contract");
        let reach = ReachEvidence {
            state: "owner_reached".to_string(),
            summary: "reached".to_string(),
        };
        let raw_write = site_with_family(
            OperationFamily::RawPointerWrite,
            vec![
                "pub fn fill_bools(ptr: *mut bool, len: usize, byte: u8) {",
                "    if byte > 1 {",
                "        if should_count() {",
                "            record_invalid_bool_byte(byte);",
                "        }",
                "        return;",
                "    }",
            ],
            "unsafe { ptr.write_bytes(byte, len) }",
            vec!["}"],
        );
        let comment_return = site_with_family(
            OperationFamily::RawPointerWrite,
            vec![
                "pub fn fill_bools(ptr: *mut bool, len: usize, byte: u8) {",
                "    if byte > 1 {",
                "        /* return; */",
                "    }",
            ],
            "unsafe { ptr.write_bytes(byte, len) }",
            vec!["}"],
        );
        let string_return = site_with_family(
            OperationFamily::RawPointerWrite,
            vec![
                "pub fn fill_bools(ptr: *mut bool, len: usize, byte: u8) {",
                "    if byte > 1 {",
                "        let _note = \"return\";",
                "    }",
            ],
            "unsafe { ptr.write_bytes(byte, len) }",
            vec!["}"],
        );

        let evidence = obligation_evidence(&raw_write, &obligations, &contract, &reach);
        let comment_evidence =
            obligation_evidence(&comment_return, &obligations, &contract, &reach);
        let string_evidence = obligation_evidence(&string_return, &obligations, &contract, &reach);

        assert!(evidence[0].discharge.present);
        assert!(!comment_evidence[0].discharge.present);
        assert!(!string_evidence[0].discharge.present);
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
        let nested_option = site_with_family(
            OperationFamily::UnwrapUnchecked,
            vec![
                "if option.is_none() {",
                "    if should_count() {",
                "        record_none();",
                "    }",
                "    return 0;",
                "}",
            ],
            "unsafe { option.unwrap_unchecked() }",
            vec![],
        );

        let option_evidence = obligation_evidence(&option, &obligations, &contract, &reach);
        let result_evidence = obligation_evidence(&result, &obligations, &contract, &reach);
        let nested_option_evidence =
            obligation_evidence(&nested_option, &obligations, &contract, &reach);

        assert!(option_evidence[0].discharge.present);
        assert!(result_evidence[0].discharge.present);
        assert!(nested_option_evidence[0].discharge.present);
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
        let nested_early_return = site_with_family(
            OperationFamily::StrFromUtf8Unchecked,
            vec![
                "if core::str::from_utf8(bytes).is_err() {",
                "    if should_count() {",
                "        record_invalid_utf8();",
                "    }",
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
        let match_block_return = site_with_family(
            OperationFamily::StrFromUtf8Unchecked,
            vec![
                "match core::str::from_utf8(bytes) {",
                "    Ok(_) => {}",
                "    Err(err) => {",
                "        record_invalid_utf8();",
                "        return Err(err);",
                "    }",
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
        let nested_return_evidence =
            obligation_evidence(&nested_early_return, &obligations, &contract, &reach);
        let if_let_err_return_evidence =
            obligation_evidence(&if_let_err_return, &obligations, &contract, &reach);
        let question_mark_evidence =
            obligation_evidence(&question_mark, &obligations, &contract, &reach);
        let match_return_evidence =
            obligation_evidence(&match_return, &obligations, &contract, &reach);
        let match_block_return_evidence =
            obligation_evidence(&match_block_return, &obligations, &contract, &reach);
        let if_let_ok_evidence = obligation_evidence(&if_let_ok, &obligations, &contract, &reach);
        let let_else_ok_evidence =
            obligation_evidence(&let_else_ok, &obligations, &contract, &reach);
        let match_ok_evidence = obligation_evidence(&match_ok, &obligations, &contract, &reach);

        assert!(checked_evidence[0].discharge.present);
        assert!(return_evidence[0].discharge.present);
        assert!(nested_return_evidence[0].discharge.present);
        assert!(if_let_err_return_evidence[0].discharge.present);
        assert!(question_mark_evidence[0].discharge.present);
        assert!(match_return_evidence[0].discharge.present);
        assert!(match_block_return_evidence[0].discharge.present);
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
        let line_comment_is_ok_branch = site_with_family(
            OperationFamily::StrFromUtf8Unchecked,
            vec!["// if core::str::from_utf8(bytes).is_ok() {"],
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
        let comment_match_return = site_with_family(
            OperationFamily::StrFromUtf8Unchecked,
            vec![
                "match core::str::from_utf8(bytes) {",
                "    Ok(_) => {}",
                "    Err(_) => {",
                "        /* return Err(err); */",
                "        log_invalid();",
                "    }",
                "}",
            ],
            "unsafe { core::str::from_utf8_unchecked(bytes) }",
            vec![],
        );
        let string_match_return = site_with_family(
            OperationFamily::StrFromUtf8Unchecked,
            vec![
                "match core::str::from_utf8(bytes) {",
                "    Ok(_) => {}",
                "    Err(_) => {",
                "        let _note = \"return Err(err)\";",
                "        log_invalid();",
                "    }",
                "}",
            ],
            "unsafe { core::str::from_utf8_unchecked(bytes) }",
            vec![],
        );
        let comment_early_return = site_with_family(
            OperationFamily::StrFromUtf8Unchecked,
            vec![
                "if core::str::from_utf8(bytes).is_err() {",
                "    /* return \"\"; */",
                "}",
            ],
            "unsafe { core::str::from_utf8_unchecked(bytes) }",
            vec![],
        );
        let string_early_return = site_with_family(
            OperationFamily::StrFromUtf8Unchecked,
            vec![
                "if core::str::from_utf8(bytes).is_err() {",
                "    let _note = \"return\";",
                "}",
            ],
            "unsafe { core::str::from_utf8_unchecked(bytes) }",
            vec![],
        );
        let comment_if_let_err_return = site_with_family(
            OperationFamily::StrFromUtf8Unchecked,
            vec![
                "if let Err(_err) = core::str::from_utf8(bytes) {",
                "    /* return \"\"; */",
                "}",
            ],
            "unsafe { core::str::from_utf8_unchecked(bytes) }",
            vec![],
        );
        let string_if_let_err_return = site_with_family(
            OperationFamily::StrFromUtf8Unchecked,
            vec![
                "if let Err(_err) = core::str::from_utf8(bytes) {",
                "    let _note = \"return\";",
                "}",
            ],
            "unsafe { core::str::from_utf8_unchecked(bytes) }",
            vec![],
        );
        let comment_let_else_return = site_with_family(
            OperationFamily::StrFromUtf8Unchecked,
            vec![
                "let Ok(_) = core::str::from_utf8(bytes) else {",
                "    /* return \"\"; */",
                "};",
            ],
            "unsafe { core::str::from_utf8_unchecked(bytes) }",
            vec![],
        );
        let string_let_else_return = site_with_family(
            OperationFamily::StrFromUtf8Unchecked,
            vec![
                "let Ok(_) = core::str::from_utf8(bytes) else {",
                "    let _note = \"return\";",
                "};",
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
        let comment_match_return_evidence =
            obligation_evidence(&comment_match_return, &obligations, &contract, &reach);
        let string_match_return_evidence =
            obligation_evidence(&string_match_return, &obligations, &contract, &reach);
        let comment_early_return_evidence =
            obligation_evidence(&comment_early_return, &obligations, &contract, &reach);
        let string_early_return_evidence =
            obligation_evidence(&string_early_return, &obligations, &contract, &reach);
        let comment_if_let_err_return_evidence =
            obligation_evidence(&comment_if_let_err_return, &obligations, &contract, &reach);
        let string_if_let_err_return_evidence =
            obligation_evidence(&string_if_let_err_return, &obligations, &contract, &reach);
        let comment_let_else_return_evidence =
            obligation_evidence(&comment_let_else_return, &obligations, &contract, &reach);
        let string_let_else_return_evidence =
            obligation_evidence(&string_let_else_return, &obligations, &contract, &reach);
        let observed_is_ok_evidence =
            obligation_evidence(&observed_is_ok, &obligations, &contract, &reach);
        let line_comment_is_ok_branch_evidence =
            obligation_evidence(&line_comment_is_ok_branch, &obligations, &contract, &reach);
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
        assert!(!comment_match_return_evidence[0].discharge.present);
        assert!(!string_match_return_evidence[0].discharge.present);
        assert!(!comment_early_return_evidence[0].discharge.present);
        assert!(!string_early_return_evidence[0].discharge.present);
        assert!(!comment_if_let_err_return_evidence[0].discharge.present);
        assert!(!string_if_let_err_return_evidence[0].discharge.present);
        assert!(!comment_let_else_return_evidence[0].discharge.present);
        assert!(!string_let_else_return_evidence[0].discharge.present);
        assert!(!observed_is_ok_evidence[0].discharge.present);
        assert!(!line_comment_is_ok_branch_evidence[0].discharge.present);
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
        let line_comment_known_valid_zeroed = site_with_family(
            OperationFamily::Zeroed,
            vec![
                "pub fn invalid_zeroed_nonnull() -> core::ptr::NonNull<u8> {",
                "// unsafe { core::mem::zeroed::<bool>() }",
            ],
            "unsafe { core::mem::zeroed::<core::ptr::NonNull<u8>>() }",
            vec![],
        );
        let string_literal_known_valid_zeroed = site_with_family(
            OperationFamily::Zeroed,
            vec![
                "pub fn invalid_zeroed_nonnull() -> core::ptr::NonNull<u8> {",
                "let _note = \"unsafe { core::mem::zeroed::<bool>() }\";",
            ],
            "unsafe { core::mem::zeroed::<core::ptr::NonNull<u8>>() }",
            vec![],
        );

        let evidence = obligation_evidence(&zeroed, &obligations, &contract, &reach);
        let line_comment_evidence = obligation_evidence(
            &line_comment_known_valid_zeroed,
            &obligations,
            &contract,
            &reach,
        );
        let string_literal_evidence = obligation_evidence(
            &string_literal_known_valid_zeroed,
            &obligations,
            &contract,
            &reach,
        );

        assert!(!evidence[0].discharge.present);
        assert!(!line_comment_evidence[0].discharge.present);
        assert!(!string_literal_evidence[0].discharge.present);
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
        let line_comment_matching_size = site_with_family(
            OperationFamily::Transmute,
            vec!["// assert_eq!(core::mem::size_of::<u8>(), core::mem::size_of::<bool>());"],
            "unsafe { core::mem::transmute::<u8, bool>(value) }",
            vec![],
        );
        let string_literal_matching_size = site_with_family(
            OperationFamily::Transmute,
            vec![
                "let _note = \"assert_eq!(core::mem::size_of::<u8>(), core::mem::size_of::<bool>())\";",
            ],
            "unsafe { core::mem::transmute::<u8, bool>(value) }",
            vec![],
        );

        let evidence = obligation_evidence(&transmute, &obligations, &contract, &reach);
        let line_comment_evidence =
            obligation_evidence(&line_comment_matching_size, &obligations, &contract, &reach);
        let string_literal_evidence = obligation_evidence(
            &string_literal_matching_size,
            &obligations,
            &contract,
            &reach,
        );

        assert!(!evidence[0].discharge.present);
        assert!(!line_comment_evidence[0].discharge.present);
        assert!(!string_literal_evidence[0].discharge.present);
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
        let line_comment_valid_value_assert = site_with_family(
            OperationFamily::Transmute,
            vec!["// assert!(value <= 1);"],
            "unsafe { core::mem::transmute::<u8, bool>(value) }",
            vec![],
        );
        let string_literal_valid_value_assert = site_with_family(
            OperationFamily::Transmute,
            vec!["let _note = \"assert!(value <= 1)\";"],
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
        let line_comment_valid_value_evidence = obligation_evidence(
            &line_comment_valid_value_assert,
            &obligations,
            &contract,
            &reach,
        );
        let string_literal_valid_value_evidence = obligation_evidence(
            &string_literal_valid_value_assert,
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
        assert!(!line_comment_valid_value_evidence[0].discharge.present);
        assert!(!string_literal_valid_value_evidence[0].discharge.present);
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
        let nested_return = site_with_family(
            OperationFamily::Transmute,
            vec![
                "if value > 1 {",
                "    if should_count() {",
                "        record_invalid_bool_byte(value);",
                "    }",
                "    return false;",
                "}",
            ],
            "unsafe { core::mem::transmute::<u8, bool>(value) }",
            vec![],
        );

        let evidence = obligation_evidence(&transmute, &obligations, &contract, &reach);
        let nested_evidence = obligation_evidence(&nested_return, &obligations, &contract, &reach);

        assert!(evidence[0].discharge.present);
        assert!(nested_evidence[0].discharge.present);
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
        let comment_return = site_with_family(
            OperationFamily::Transmute,
            vec!["if value > 1 {", "    /* return false; */", "}"],
            "unsafe { core::mem::transmute::<u8, bool>(value) }",
            vec![],
        );
        let string_return = site_with_family(
            OperationFamily::Transmute,
            vec!["if value > 1 {", "    let _note = \"return false\";", "}"],
            "unsafe { core::mem::transmute::<u8, bool>(value) }",
            vec![],
        );

        let evidence = obligation_evidence(&transmute, &obligations, &contract, &reach);
        let comment_evidence =
            obligation_evidence(&comment_return, &obligations, &contract, &reach);
        let string_evidence = obligation_evidence(&string_return, &obligations, &contract, &reach);

        assert!(!evidence[0].discharge.present);
        assert!(!comment_evidence[0].discharge.present);
        assert!(!string_evidence[0].discharge.present);
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
        let line_comment_infallible_match = site_with_family(
            OperationFamily::UnreachableUnchecked,
            vec!["// match fallible_with_capacity(Fallibility::Infallible) {"],
            "Err(_) => unsafe { hint::unreachable_unchecked() },",
            vec![],
        );
        let string_literal_infallible_match = site_with_family(
            OperationFamily::UnreachableUnchecked,
            vec!["let _note = \"match fallible_with_capacity(Fallibility::Infallible) {\";"],
            "Err(_) => unsafe { hint::unreachable_unchecked() },",
            vec![],
        );

        let evidence = obligation_evidence(&unreachable, &obligations, &contract, &reach);
        let line_comment_evidence = obligation_evidence(
            &line_comment_infallible_match,
            &obligations,
            &contract,
            &reach,
        );
        let string_literal_evidence = obligation_evidence(
            &string_literal_infallible_match,
            &obligations,
            &contract,
            &reach,
        );

        assert!(!evidence[0].discharge.present);
        assert!(!line_comment_evidence[0].discharge.present);
        assert!(!string_literal_evidence[0].discharge.present);
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
