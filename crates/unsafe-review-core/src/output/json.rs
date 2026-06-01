use crate::api::{AnalyzeOutput, Scope, Summary};
use crate::domain::{EvidenceState, ObligationEvidence, ReviewCard, WitnessRoute};
use crate::util::path_display;
use serde::Serialize;

const TRUST_BOUNDARY: &str = "Static unsafe contract review only; this is not a proof of memory safety, not UB-free status, and not a Miri result unless a witness receipt is attached.";

pub(crate) fn render(output: &AnalyzeOutput) -> String {
    render_pretty(&JsonAnalyzeOutput::from(output))
}

fn render_pretty(value: &impl Serialize) -> String {
    match serde_json::to_string_pretty(value) {
        Ok(text) => text,
        Err(err) => format!("{{\n  \"error\": \"json serialization failed: {err}\"\n}}"),
    }
}

#[derive(Serialize)]
struct JsonAnalyzeOutput<'a> {
    schema_version: &'a str,
    tool: &'a str,
    scope: &'static str,
    mode: &'static str,
    policy: &'static str,
    trust_boundary: &'static str,
    root: String,
    summary: JsonSummary,
    cards: Vec<JsonCard<'a>>,
}

impl<'a> From<&'a AnalyzeOutput> for JsonAnalyzeOutput<'a> {
    fn from(output: &'a AnalyzeOutput) -> Self {
        Self {
            schema_version: &output.schema_version,
            tool: &output.tool,
            scope: scope_str(output),
            mode: output.mode.as_str(),
            policy: output.policy.as_str(),
            trust_boundary: TRUST_BOUNDARY,
            root: path_display(&output.root),
            summary: JsonSummary::from(&output.summary),
            cards: output.cards.iter().map(JsonCard::from).collect(),
        }
    }
}

#[derive(Serialize)]
struct JsonSummary {
    rust_files: usize,
    changed_rust_files: usize,
    unsafe_sites: usize,
    cards: usize,
    open_actionable_gaps: usize,
    contract_missing: usize,
    guard_missing: usize,
    guarded_unwitnessed: usize,
    unsafe_unreached: usize,
    requires_loom: usize,
    miri_unsupported: usize,
    static_unknown: usize,
}

impl From<&Summary> for JsonSummary {
    fn from(summary: &Summary) -> Self {
        Self {
            rust_files: summary.rust_files,
            changed_rust_files: summary.changed_rust_files,
            unsafe_sites: summary.unsafe_sites,
            cards: summary.cards,
            open_actionable_gaps: summary.open_actionable_gaps,
            contract_missing: summary.contract_missing,
            guard_missing: summary.guard_missing,
            guarded_unwitnessed: summary.guarded_unwitnessed,
            unsafe_unreached: summary.unsafe_unreached,
            requires_loom: summary.requires_loom,
            miri_unsupported: summary.miri_unsupported,
            static_unknown: summary.static_unknown,
        }
    }
}

#[derive(Serialize)]
struct JsonCard<'a> {
    id: &'a str,
    #[serde(rename = "class")]
    class_name: &'static str,
    priority: &'static str,
    confidence: &'static str,
    site: JsonSite<'a>,
    operation: &'a str,
    operation_family: &'static str,
    hazards: Vec<&'static str>,
    obligations: Vec<&'a str>,
    obligation_evidence: Vec<JsonObligationEvidence<'a>>,
    contract: &'a str,
    discharge: &'a str,
    reach: &'a str,
    witness: &'a str,
    witness_routes: Vec<JsonWitnessRoute<'a>>,
    missing: Vec<&'a str>,
    next_action: &'a str,
    verify_commands: &'a [String],
}

impl<'a> From<&'a ReviewCard> for JsonCard<'a> {
    fn from(card: &'a ReviewCard) -> Self {
        Self {
            id: &card.id.0,
            class_name: card.class.as_str(),
            priority: card.priority.as_str(),
            confidence: card.confidence.as_str(),
            site: JsonSite::from(card),
            operation: &card.operation.expression,
            operation_family: card.operation.family.as_str(),
            hazards: card.hazards.iter().map(|hazard| hazard.as_str()).collect(),
            obligations: card
                .obligations
                .iter()
                .map(|obligation| obligation.description.as_str())
                .collect(),
            obligation_evidence: card
                .obligation_evidence
                .iter()
                .map(JsonObligationEvidence::from)
                .collect(),
            contract: &card.contract.summary,
            discharge: &card.discharge.summary,
            reach: &card.reach.summary,
            witness: &card.witness.summary,
            witness_routes: card.routes.iter().map(JsonWitnessRoute::from).collect(),
            missing: card
                .missing
                .iter()
                .map(|missing| missing.message.as_str())
                .collect(),
            next_action: &card.next_action.summary,
            verify_commands: &card.next_action.verify_commands,
        }
    }
}

#[derive(Serialize)]
struct JsonWitnessRoute<'a> {
    kind: &'static str,
    reason: &'a str,
    command: Option<&'a str>,
    required: bool,
}

impl<'a> From<&'a WitnessRoute> for JsonWitnessRoute<'a> {
    fn from(route: &'a WitnessRoute) -> Self {
        Self {
            kind: route.kind.as_str(),
            reason: &route.reason,
            command: route.command.as_deref(),
            required: route.required,
        }
    }
}

#[derive(Serialize)]
struct JsonObligationEvidence<'a> {
    key: &'a str,
    description: &'a str,
    contract: JsonEvidenceState<'a>,
    discharge: JsonEvidenceState<'a>,
    reach: JsonEvidenceState<'a>,
    witness: JsonEvidenceState<'a>,
}

impl<'a> From<&'a ObligationEvidence> for JsonObligationEvidence<'a> {
    fn from(evidence: &'a ObligationEvidence) -> Self {
        Self {
            key: &evidence.obligation.key,
            description: &evidence.obligation.description,
            contract: JsonEvidenceState::from(&evidence.contract),
            discharge: JsonEvidenceState::from(&evidence.discharge),
            reach: JsonEvidenceState::from(&evidence.reach),
            witness: JsonEvidenceState::from(&evidence.witness),
        }
    }
}

#[derive(Serialize)]
struct JsonEvidenceState<'a> {
    present: bool,
    state: &'a str,
    summary: &'a str,
}

impl<'a> From<&'a EvidenceState> for JsonEvidenceState<'a> {
    fn from(state: &'a EvidenceState) -> Self {
        Self {
            present: state.present,
            state: &state.state,
            summary: &state.summary,
        }
    }
}

#[derive(Serialize)]
struct JsonSite<'a> {
    file: String,
    line: usize,
    column: usize,
    kind: &'static str,
    owner: &'a str,
    visibility: &'a str,
    public_api_surface: bool,
    snippet: &'a str,
}

impl<'a> From<&'a ReviewCard> for JsonSite<'a> {
    fn from(card: &'a ReviewCard) -> Self {
        Self {
            file: path_display(&card.site.location.file),
            line: card.site.location.line,
            column: card.site.location.column,
            kind: card.site.kind.as_str(),
            owner: card.site.owner.as_deref().unwrap_or("unknown"),
            visibility: &card.site.visibility,
            public_api_surface: card.site.public_api_surface,
            snippet: &card.site.snippet,
        }
    }
}

fn scope_str(output: &AnalyzeOutput) -> &'static str {
    match output.scope {
        Scope::Diff => "diff",
        Scope::Repo => "repo",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{AnalysisMode, AnalyzeInput, DiffSource, PolicyMode, analyze};
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::PathBuf;

    const FIXTURE_GOLDENS: &[&str] = &[
        "raw_pointer_alignment",
        "raw_pointer_alignment_receipted",
        "raw_pointer_alignment_is_aligned_guard",
        "raw_pointer_alignment_post_check_not_guard",
        "raw_pointer_alignment_observed_not_guard",
        "raw_pointer_alignment_closed_branch_not_guard",
        "raw_pointer_alignment_reassigned_pointer_not_guard",
        "raw_pointer_alignment_modulo_guard",
        "raw_pointer_alignment_modulo_observed_not_guard",
        "raw_pointer_alignment_modulo_closed_branch_not_guard",
        "raw_pointer_alignment_modulo_reassigned_pointer_not_guard",
        "align_of_only_not_guard",
        "alignment_other_pointer_not_guard",
        "comment_alignment_not_guard",
        "safe_code_no_cards",
        "public_unsafe_fn_missing_safety",
        "public_unsafe_fn_with_safety_docs",
        "public_unsafe_fn_safety_colon_docs",
        "public_unsafe_trait_missing_safety",
        "public_unsafe_fn_safety_comment_not_docs",
        "documented_private_unsafe_fn",
        "local_safety_colon_comment",
        "private_unsafe_helper_safety_comment",
        "split_public_unsafe_fn_missing_safety",
        "attributed_unsafe_fn_no_duplicate",
        "inline_unsafe_raw_pointer_deref_no_duplicate",
        "unsafe_fn_call_wrapper",
        "multiline_unsafe_fn_call_wrapper",
        "unsafe_fn_call_encode_utf8_remaining_cap",
        "unchecked_constructor_availability_guard",
        "unchecked_constructor_availability_assert_guard",
        "unchecked_constructor_unavailable_return_guard",
        "unchecked_constructor_unavailable_return_comment_not_guard",
        "unchecked_constructor_other_availability_not_guard",
        "unchecked_constructor_availability_observed_not_guard",
        "unchecked_constructor_availability_closed_branch_not_guard",
        "nonnull_new_guard",
        "nonnull_new_reassigned_ptr_not_guard",
        "nonnull_new_shadowed_ptr_not_guard",
        "nonnull_method_receiver_reassigned_not_guard",
        "nonnull_if_let_new_guard",
        "nonnull_let_else_new_guard",
        "nonnull_match_new_guard",
        "nonnull_other_guard_not_evidence",
        "nonnull_is_null_nonreturning_not_guard",
        "nonnull_is_null_return_comment_not_guard",
        "nonnull_is_null_disjunct_return_guard",
        "nonnull_is_null_conjunct_return_not_guard",
        "nonnull_is_null_reassigned_ptr_not_guard",
        "nonnull_is_null_shadowed_ptr_not_guard",
        "nonnull_is_null_open_branch_guard",
        "nonnull_is_null_conjunct_open_branch_guard",
        "nonnull_is_null_disjunct_open_branch_not_guard",
        "nonnull_is_null_open_branch_reassigned_ptr_not_guard",
        "nonnull_is_null_open_branch_shadowed_ptr_not_guard",
        "nonnull_observed_not_guard",
        "nonnull_post_check_not_guard",
        "nonnull_cast_checked_pointer_not_guard",
        "nonnull_if_let_new_reassigned_ptr_not_guard",
        "nonnull_if_let_new_shadowed_ptr_not_guard",
        "nonnull_let_else_new_reassigned_ptr_not_guard",
        "nonnull_let_else_new_shadowed_ptr_not_guard",
        "nonnull_match_new_reassigned_ptr_not_guard",
        "nonnull_match_new_shadowed_ptr_not_guard",
        "impl_trait_bound_owner_inference",
        "long_unsafe_fn_owner_inference",
        "macro_rules_owner_inference",
        "nested_unsafe_operation_call_dedupe",
        "adjacent_unchanged_unsafe_fn_no_card",
        "split_unsafe_block",
        "raw_pointer_deref",
        "raw_pointer_read_unaligned",
        "raw_pointer_read_volatile",
        "raw_pointer_read_len_capacity_assert",
        "raw_pointer_read_assert_shadowed_origin_not_guard",
        "raw_pointer_read_len_capacity_assert_shadowed_origin_not_guard",
        "raw_pointer_read_bounds_observed_not_guard",
        "raw_pointer_read_len_capacity_observed_not_guard",
        "raw_pointer_read_as_cast_origin_bounds_guard",
        "raw_pointer_read_cast_origin_bounds_guard",
        "raw_pointer_read_open_branch_bounds_guard",
        "raw_pointer_read_open_branch_shadowed_origin_not_guard",
        "raw_pointer_read_typed_shadowed_origin_not_guard",
        "raw_pointer_read_other_len_not_guard",
        "raw_pointer_read_reassigned_origin_not_guard",
        "raw_pointer_write_assignment",
        "raw_pointer_write_unaligned",
        "raw_pointer_write_bytes",
        "raw_pointer_write_bool_bytes_guard",
        "raw_pointer_write_bool_reassigned_byte_not_guard",
        "raw_pointer_write_bool_closed_branch_not_guard",
        "raw_pointer_write_bool_conjunct_branch_guard",
        "raw_pointer_write_bool_disjunct_return_guard",
        "raw_pointer_write_bool_disjunct_branch_not_guard",
        "raw_pointer_write_bool_conjunct_return_not_guard",
        "raw_pointer_write_previous_slice_not_guard",
        "raw_pointer_write_previous_u8_not_guard",
        "raw_pointer_write_previous_bool_not_guard",
        "raw_pointer_write_previous_maybeuninit_not_guard",
        "raw_pointer_write_other_u8_not_guard",
        "raw_pointer_write_maybeuninit",
        "raw_pointer_write_other_maybeuninit_not_guard",
        "raw_pointer_write_volatile",
        "ptr_copy_overlapping",
        "ptr_copy_slice_range_guard",
        "ptr_copy_slice_range_conjunctive_assert_guard",
        "ptr_copy_slice_range_early_return_guard",
        "ptr_copy_slice_range_disjunctive_early_return_guard",
        "ptr_copy_slice_range_open_branch_guard",
        "ptr_copy_slice_range_conjunctive_open_branch_guard",
        "ptr_copy_slice_range_closed_branch_not_guard",
        "ptr_copy_slice_range_or_branch_not_guard",
        "ptr_copy_slice_range_disjunctive_early_return_block_comment_not_guard",
        "ptr_copy_slice_range_disjunctive_early_return_reassigned_count_not_guard",
        "ptr_copy_slice_range_disjunctive_early_return_compound_reassigned_count_not_guard",
        "ptr_copy_slice_range_disjunctive_early_return_shadowed_count_not_guard",
        "ptr_copy_slice_range_disjunctive_early_return_reassigned_src_not_guard",
        "ptr_copy_slice_range_disjunctive_early_return_reassigned_src_path_not_guard",
        "ptr_copy_slice_range_disjunctive_early_return_shadowed_src_path_not_guard",
        "ptr_copy_slice_range_disjunctive_early_return_shadowed_src_not_guard",
        "ptr_copy_slice_range_disjunctive_early_return_reassigned_dst_not_guard",
        "ptr_copy_slice_range_disjunctive_early_return_reassigned_dst_path_not_guard",
        "ptr_copy_slice_range_disjunctive_early_return_shadowed_dst_path_not_guard",
        "ptr_copy_slice_range_disjunctive_early_return_shadowed_dst_not_guard",
        "ptr_copy_slice_range_open_branch_reassigned_count_not_guard",
        "ptr_copy_slice_range_open_branch_compound_reassigned_count_not_guard",
        "ptr_copy_slice_range_open_branch_shadowed_count_not_guard",
        "ptr_copy_slice_range_open_branch_reassigned_src_path_not_guard",
        "ptr_copy_slice_range_open_branch_reassigned_dst_path_not_guard",
        "ptr_copy_slice_range_open_branch_shadowed_src_path_not_guard",
        "ptr_copy_slice_range_open_branch_shadowed_dst_path_not_guard",
        "ptr_copy_slice_range_open_branch_shadowed_src_not_guard",
        "ptr_copy_slice_range_open_branch_shadowed_dst_not_guard",
        "ptr_copy_slice_range_open_branch_reassigned_src_not_guard",
        "ptr_copy_slice_range_open_branch_reassigned_dst_not_guard",
        "ptr_copy_slice_range_src_only_not_guard",
        "ptr_copy_slice_range_dst_only_not_guard",
        "ptr_copy_slice_range_reassigned_count_not_guard",
        "ptr_copy_slice_range_shadowed_count_not_guard",
        "ptr_copy_slice_range_reassigned_src_not_guard",
        "ptr_copy_slice_range_reassigned_src_path_not_guard",
        "ptr_copy_slice_range_shadowed_src_path_not_guard",
        "ptr_copy_slice_range_shadowed_src_not_guard",
        "ptr_copy_slice_range_reassigned_dst_not_guard",
        "ptr_copy_slice_range_reassigned_dst_path_not_guard",
        "ptr_copy_slice_range_shadowed_dst_path_not_guard",
        "ptr_copy_slice_range_shadowed_dst_not_guard",
        "ptr_copy_other_len_not_guard",
        "ptr_replace_value",
        "copy_nonoverlapping",
        "copy_nonoverlapping_slice_range_guard",
        "copy_nonoverlapping_slice_range_conjunctive_assert_guard",
        "copy_nonoverlapping_slice_range_early_return_guard",
        "copy_nonoverlapping_slice_range_disjunctive_early_return_guard",
        "copy_nonoverlapping_slice_range_open_branch_guard",
        "copy_nonoverlapping_slice_range_conjunctive_open_branch_guard",
        "copy_nonoverlapping_slice_range_closed_branch_not_guard",
        "copy_nonoverlapping_slice_range_or_branch_not_guard",
        "copy_nonoverlapping_slice_range_disjunctive_early_return_block_comment_not_guard",
        "copy_nonoverlapping_slice_range_disjunctive_early_return_reassigned_count_not_guard",
        "copy_nonoverlapping_slice_range_disjunctive_early_return_compound_reassigned_count_not_guard",
        "copy_nonoverlapping_slice_range_disjunctive_early_return_shadowed_count_not_guard",
        "copy_nonoverlapping_slice_range_disjunctive_early_return_reassigned_src_not_guard",
        "copy_nonoverlapping_slice_range_disjunctive_early_return_reassigned_src_path_not_guard",
        "copy_nonoverlapping_slice_range_disjunctive_early_return_shadowed_src_path_not_guard",
        "copy_nonoverlapping_slice_range_disjunctive_early_return_shadowed_src_not_guard",
        "copy_nonoverlapping_slice_range_disjunctive_early_return_reassigned_dst_not_guard",
        "copy_nonoverlapping_slice_range_disjunctive_early_return_reassigned_dst_path_not_guard",
        "copy_nonoverlapping_slice_range_disjunctive_early_return_shadowed_dst_path_not_guard",
        "copy_nonoverlapping_slice_range_disjunctive_early_return_shadowed_dst_not_guard",
        "copy_nonoverlapping_slice_range_open_branch_reassigned_count_not_guard",
        "copy_nonoverlapping_slice_range_open_branch_compound_reassigned_count_not_guard",
        "copy_nonoverlapping_slice_range_open_branch_shadowed_count_not_guard",
        "copy_nonoverlapping_slice_range_open_branch_reassigned_src_not_guard",
        "copy_nonoverlapping_slice_range_open_branch_reassigned_src_path_not_guard",
        "copy_nonoverlapping_slice_range_open_branch_shadowed_src_not_guard",
        "copy_nonoverlapping_slice_range_open_branch_shadowed_src_path_not_guard",
        "copy_nonoverlapping_slice_range_open_branch_reassigned_dst_not_guard",
        "copy_nonoverlapping_slice_range_open_branch_reassigned_dst_path_not_guard",
        "copy_nonoverlapping_slice_range_open_branch_shadowed_dst_not_guard",
        "copy_nonoverlapping_slice_range_open_branch_shadowed_dst_path_not_guard",
        "copy_nonoverlapping_slice_range_src_only_not_guard",
        "copy_nonoverlapping_slice_range_dst_only_not_guard",
        "copy_nonoverlapping_slice_range_reassigned_count_not_guard",
        "copy_nonoverlapping_slice_range_shadowed_count_not_guard",
        "copy_nonoverlapping_slice_range_reassigned_src_not_guard",
        "copy_nonoverlapping_slice_range_reassigned_src_path_not_guard",
        "copy_nonoverlapping_slice_range_shadowed_src_path_not_guard",
        "copy_nonoverlapping_slice_range_shadowed_src_not_guard",
        "copy_nonoverlapping_slice_range_reassigned_dst_not_guard",
        "copy_nonoverlapping_slice_range_reassigned_dst_path_not_guard",
        "copy_nonoverlapping_slice_range_shadowed_dst_path_not_guard",
        "copy_nonoverlapping_slice_range_shadowed_dst_not_guard",
        "copy_nonoverlapping_other_len_not_guard",
        "str_from_utf8_unchecked",
        "str_from_utf8_unchecked_comment_not_guard",
        "str_from_utf8_unchecked_is_ok_guard",
        "str_from_utf8_unchecked_is_err_return_guard",
        "str_from_utf8_unchecked_is_err_return_comment_not_guard",
        "str_from_utf8_unchecked_question_mark_guard",
        "str_from_utf8_unchecked_match_return_guard",
        "str_from_utf8_unchecked_post_validation_not_guard",
        "str_from_utf8_unchecked_other_buffer_not_guard",
        "str_from_utf8_unchecked_prefix_validation_not_guard",
        "str_from_utf8_unchecked_suffix_validation_not_guard",
        "str_from_utf8_unchecked_is_ok_observed_not_guard",
        "str_from_utf8_unchecked_guard_then_reassigned_not_guard",
        "str_from_utf8_unchecked_guard_then_mutated_not_guard",
        "str_from_utf8_unchecked_if_let_ok_guard",
        "str_from_utf8_unchecked_if_let_ok_shadowed_not_guard",
        "str_from_utf8_unchecked_if_let_err_return_guard",
        "str_from_utf8_unchecked_if_let_err_reassigned_not_guard",
        "str_from_utf8_unchecked_guard_then_shadowed_not_guard",
        "str_from_utf8_unchecked_let_else_ok_guard",
        "str_from_utf8_unchecked_let_else_ok_reassigned_not_guard",
        "str_from_utf8_unchecked_match_ok_guard",
        "str_from_utf8_unchecked_match_ok_reassigned_not_guard",
        "zeroed_invalid_value",
        "zeroed_valid_u32",
        "inline_asm_human_review",
        "pointer_arithmetic_num_ctrl_bytes_guard",
        "pointer_arithmetic_other_offset_not_guard",
        "pointer_arithmetic_reassigned_offset_not_guard",
        "pointer_arithmetic_shadowed_offset_not_guard",
        "pointer_arithmetic_compound_offset_not_guard",
        "pointer_arithmetic_stale_bound_not_guard",
        "pointer_arithmetic_disjunct_bounds_not_guard",
        "pointer_arithmetic_closed_branch_not_guard",
        "pointer_arithmetic_slice_end",
        "slice_from_raw_parts_mut",
        "slice_from_raw_parts_mut_maybeuninit",
        "slice_from_raw_parts_mut_other_maybeuninit_not_guard",
        "vec_from_raw_parts",
        "vec_from_raw_parts_capacity_guard",
        "vec_from_raw_parts_capacity_assert_guard",
        "vec_from_raw_parts_capacity_observed_not_guard",
        "vec_from_raw_parts_capacity_value_observed_not_guard",
        "vec_from_raw_parts_capacity_closed_branch_not_guard",
        "vec_from_raw_parts_capacity_return_comment_not_guard",
        "vec_from_raw_parts_capacity_reassigned_not_guard",
        "vec_from_raw_parts_manuallydrop_origin",
        "vec_from_raw_parts_stale_pointer_origin_not_guard",
        "box_from_raw",
        "box_from_raw_box_origin",
        "box_from_raw_reassigned_origin_not_guard",
        "static_mut_global_state",
        "safe_reference_deref_no_cards",
        "imports_not_unsafe_operations",
        "cfg_target_feature_not_operation",
        "target_feature_safety_docs",
        "target_feature_missing_safety_docs",
        "split_raw_pointer_read_call",
        "maybeuninit_assume_init",
        "maybeuninit_assume_init_comment_not_guard",
        "maybeuninit_assume_init_write_guard",
        "maybeuninit_assume_init_read_write_guard",
        "maybeuninit_assume_init_ref_write_guard",
        "maybeuninit_assume_init_mut_write_guard",
        "maybeuninit_assume_init_drop_write_guard",
        "maybeuninit_assume_init_open_branch_write_guard",
        "maybeuninit_assume_init_read_open_branch_write_guard",
        "maybeuninit_assume_init_ref_open_branch_write_guard",
        "maybeuninit_assume_init_mut_open_branch_write_guard",
        "maybeuninit_assume_init_drop_open_branch_write_guard",
        "maybeuninit_assume_init_open_branch_new_guard",
        "maybeuninit_assume_init_read_open_branch_new_guard",
        "maybeuninit_assume_init_ref_open_branch_new_guard",
        "maybeuninit_assume_init_mut_open_branch_new_guard",
        "maybeuninit_assume_init_drop_open_branch_new_guard",
        "maybeuninit_assume_init_closed_branch_write_not_guard",
        "maybeuninit_assume_init_read_closed_branch_write_not_guard",
        "maybeuninit_assume_init_ref_closed_branch_write_not_guard",
        "maybeuninit_assume_init_mut_closed_branch_write_not_guard",
        "maybeuninit_assume_init_drop_closed_branch_write_not_guard",
        "maybeuninit_assume_init_new_guard",
        "maybeuninit_assume_init_read_new_guard",
        "maybeuninit_assume_init_ref_new_guard",
        "maybeuninit_assume_init_mut_method_new_guard",
        "maybeuninit_assume_init_mut_new_guard",
        "maybeuninit_assume_init_drop_new_guard",
        "maybeuninit_assume_init_closed_branch_new_not_guard",
        "maybeuninit_assume_init_read_closed_branch_new_not_guard",
        "maybeuninit_assume_init_ref_closed_branch_new_not_guard",
        "maybeuninit_assume_init_mut_closed_branch_new_not_guard",
        "maybeuninit_assume_init_drop_closed_branch_new_not_guard",
        "maybeuninit_assume_init_other_slot_write_not_guard",
        "maybeuninit_assume_init_read_other_slot_write_not_guard",
        "maybeuninit_assume_init_ref_other_slot_write_not_guard",
        "maybeuninit_assume_init_mut_other_slot_write_not_guard",
        "maybeuninit_assume_init_drop_other_slot_write_not_guard",
        "maybeuninit_assume_init_stale_write_not_guard",
        "maybeuninit_assume_init_read_stale_write_not_guard",
        "maybeuninit_assume_init_ref_stale_write_not_guard",
        "maybeuninit_assume_init_mut_stale_write_not_guard",
        "maybeuninit_assume_init_drop_stale_write_not_guard",
        "maybeuninit_assume_init_stale_field_write_not_guard",
        "maybeuninit_assume_init_stale_new_not_guard",
        "maybeuninit_assume_init_read_stale_new_not_guard",
        "maybeuninit_assume_init_ref_stale_new_not_guard",
        "maybeuninit_assume_init_mut_stale_new_not_guard",
        "maybeuninit_assume_init_drop_stale_new_not_guard",
        "maybeuninit_assume_init_shadowed_slot_not_guard",
        "maybeuninit_assume_init_read_shadowed_slot_not_guard",
        "maybeuninit_assume_init_ref_shadowed_slot_not_guard",
        "maybeuninit_assume_init_mut_shadowed_slot_not_guard",
        "maybeuninit_assume_init_drop_shadowed_slot_not_guard",
        "maybeuninit_assume_init_mutslot_new_not_guard",
        "maybeuninit_assume_init_read_mutslot_new_not_guard",
        "maybeuninit_assume_init_ref_mutslot_new_not_guard",
        "maybeuninit_assume_init_mut_mutslot_new_not_guard",
        "maybeuninit_assume_init_drop_mutslot_new_not_guard",
        "maybeuninit_assume_init_partial_field_not_guard",
        "maybeuninit_assume_init_partial_array_not_guard",
        "maybeuninit_assume_init_read",
        "maybeuninit_assume_init_ref",
        "maybeuninit_assume_init_mut",
        "maybeuninit_assume_init_drop",
        "vec_set_len",
        "vec_set_len_comment_not_guard",
        "vec_set_len_initialized_loop",
        "vec_set_len_self_new_const_cap_not_guard",
        "vec_set_len_post_init_not_guard",
        "vec_set_len_slice_binding_initialized_loop",
        "vec_set_len_other_slice_binding_not_guard",
        "vec_set_len_partial_slice_binding_not_guard",
        "vec_set_len_stale_slice_binding_not_guard",
        "vec_set_len_single_index_init_not_guard",
        "vec_set_len_capacity_observed_not_guard",
        "vec_set_len_unrelated_capacity_comparison_not_guard",
        "vec_set_len_unrelated_const_cap_not_guard",
        "vec_set_len_capacity_binding",
        "vec_set_len_stale_capacity_binding_not_guard",
        "vec_set_len_remaining_capacity_guard",
        "vec_set_len_other_remaining_capacity_not_guard",
        "vec_set_len_cap_argument_not_guard",
        "vec_set_len_reassigned_receiver_not_guard",
        "vec_set_len_reassigned_new_len_not_guard",
        "vec_set_len_compound_reassigned_new_len_not_guard",
        "vec_set_len_shadowed_new_len_not_guard",
        "vec_set_len_unrelated_initialization_not_guard",
        "vec_set_len_with_capacity",
        "vec_set_len_stale_with_capacity_not_guard",
        "vec_set_len_stale_with_capacity_len_not_guard",
        "vec_set_len_reserve_capacity",
        "vec_set_len_reserve_reassigned_additional_not_guard",
        "vec_set_len_try_reserve_capacity",
        "vec_set_len_try_reserve_reassigned_additional_not_guard",
        "vec_set_len_call_result_init",
        "vec_set_len_shrink",
        "vec_set_len_last_index_shrink",
        "vec_set_len_stale_last_index_shrink_not_guard",
        "vec_set_len_other_last_index_shrink_not_guard",
        "vec_set_len_start_bound_shrink",
        "vec_set_len_stale_start_bound_shrink_not_guard",
        "vec_set_len_zero_clear",
        "drop_in_place_deallocation",
        "drop_in_place_box_origin",
        "drop_in_place_reassigned_origin_not_guard",
        "atomic_pointer_state_swap",
        "atomic_pointer_state_fetch_ops",
        "unwrap_unchecked_result",
        "unwrap_unchecked_infallible_result",
        "unwrap_unchecked_other_infallible_not_guard",
        "unwrap_unchecked_is_some_guard",
        "unwrap_unchecked_is_some_reassigned_not_guard",
        "unwrap_unchecked_is_ok_guard",
        "unwrap_unchecked_is_ok_reassigned_not_guard",
        "unwrap_unchecked_is_ok_observed_not_guard",
        "unwrap_unchecked_is_some_observed_not_guard",
        "unwrap_unchecked_if_let_some_guard",
        "unwrap_unchecked_if_let_ok_guard",
        "unwrap_unchecked_let_else_some_guard",
        "unwrap_unchecked_let_else_some_reassigned_not_guard",
        "unwrap_unchecked_let_else_ok_guard",
        "unwrap_unchecked_let_else_ok_reassigned_not_guard",
        "unwrap_unchecked_match_some_guard",
        "unwrap_unchecked_match_some_reassigned_not_guard",
        "unwrap_unchecked_match_ok_guard",
        "unwrap_unchecked_match_ok_reassigned_not_guard",
        "unwrap_unchecked_other_if_let_not_guard",
        "unwrap_unchecked_other_if_let_ok_not_guard",
        "unwrap_unchecked_post_check_not_guard",
        "unwrap_unchecked_guard_then_reassigned_not_guard",
        "unwrap_unchecked_is_none_return_guard",
        "unwrap_unchecked_is_none_return_comment_not_guard",
        "unwrap_unchecked_is_err_return_guard",
        "unreachable_unchecked_path",
        "unreachable_unchecked_infallible_path",
        "unreachable_unchecked_other_infallible_not_guard",
        "unreachable_unchecked_post_infallible_not_guard",
        "unreachable_unchecked_closed_infallible_match_not_guard",
        "transmute_invalid_value",
        "transmute_layout_size_guard",
        "transmute_layout_conjunct_branch_guard",
        "transmute_layout_disjunct_branch_not_guard",
        "transmute_layout_mismatch_return_guard",
        "transmute_layout_mismatch_return_comment_not_guard",
        "transmute_layout_conjunct_return_not_guard",
        "transmute_layout_closed_branch_not_guard",
        "transmute_layout_observed_not_guard",
        "transmute_bool_comment_not_guard",
        "transmute_bool_valid_value_guard",
        "transmute_bool_conjunct_branch_guard",
        "transmute_bool_disjunct_branch_not_guard",
        "transmute_bool_invalid_return_guard",
        "transmute_bool_invalid_return_comment_not_guard",
        "transmute_bool_disjunct_return_guard",
        "transmute_bool_conjunct_return_not_guard",
        "transmute_bool_other_value_not_guard",
        "transmute_bool_prior_guarded_call_not_guard",
        "transmute_bool_value_observed_not_guard",
        "transmute_bool_closed_if_observed_not_guard",
        "transmute_bool_guard_then_reassigned_not_guard",
        "transmute_bool_guard_then_compound_reassigned_not_guard",
        "transmute_bool_guard_then_shadowed_not_guard",
        "transmute_copy_invalid_value",
        "transmute_copy_layout_size_guard",
        "transmute_copy_bool_comment_not_guard",
        "transmute_copy_layout_conjunct_branch_guard",
        "transmute_copy_layout_disjunct_branch_not_guard",
        "transmute_copy_layout_mismatch_return_guard",
        "transmute_copy_layout_mismatch_return_comment_not_guard",
        "transmute_copy_layout_conjunct_return_not_guard",
        "transmute_copy_bool_valid_value_guard",
        "transmute_copy_bool_other_value_not_guard",
        "transmute_copy_bool_prior_guarded_call_not_guard",
        "transmute_copy_bool_conjunct_branch_guard",
        "transmute_copy_bool_disjunct_branch_not_guard",
        "transmute_copy_bool_invalid_return_guard",
        "transmute_copy_bool_invalid_return_comment_not_guard",
        "transmute_copy_bool_disjunct_return_guard",
        "transmute_copy_bool_conjunct_return_not_guard",
        "transmute_copy_bool_value_observed_not_guard",
        "transmute_copy_bool_closed_if_observed_not_guard",
        "transmute_copy_bool_guard_then_reassigned_not_guard",
        "transmute_copy_bool_guard_then_compound_reassigned_not_guard",
        "transmute_copy_bool_guard_then_shadowed_not_guard",
        "multiline_transmute_copy_invalid_value",
        "unsafe_impl_send",
        "unsafe_impl_send_generic_owner",
        "unsafe_impl_sync_generic_bound",
        "unsafe_impl_custom_trait_not_send_sync",
        "ffi_sanitizer_route",
        "ffi_missing_boundary_contract",
        "ffi_call_sanitizer_route",
        "ffi_qualified_call_sanitizer_route",
        "ffi_libc_call_sanitizer_route",
        "ffi_non_libc_wrapper_call_not_route",
        "ffi_local_libc_module_call_not_route",
        "get_unchecked_mut_bounds",
        "get_unchecked_mut_len_guard",
        "get_unchecked_mut_conjunct_len_guard",
        "get_unchecked_mut_get_probe_guard",
        "get_unchecked_mut_get_probe_early_return_guard",
        "get_unchecked_mut_get_probe_other_slice_not_guard",
        "get_unchecked_mut_if_let_get_guard",
        "get_unchecked_mut_let_else_get_guard",
        "get_unchecked_mut_match_get_guard",
        "get_unchecked_mut_other_len_not_guard",
        "get_unchecked_mut_disjunct_len_not_guard",
        "get_unchecked_mut_post_check_not_guard",
        "get_unchecked_mut_bounds_observed_not_guard",
        "get_unchecked_mut_closed_bounds_not_guard",
        "get_unchecked_mut_return_comment_not_guard",
        "get_unchecked_mut_reassigned_index_not_guard",
        "get_unchecked_mut_compound_reassigned_index_not_guard",
        "get_unchecked_mut_shadowed_index_not_guard",
        "get_unchecked_mut_reassigned_receiver_not_guard",
        "get_unchecked_mut_reassigned_receiver_path_not_guard",
        "get_unchecked_mut_shadowed_receiver_not_guard",
        "get_unchecked_mut_get_probe_reassigned_index_not_guard",
        "get_unchecked_mut_get_probe_shadowed_index_not_guard",
        "get_unchecked_mut_get_probe_reassigned_receiver_not_guard",
        "get_unchecked_mut_get_probe_reassigned_receiver_path_not_guard",
        "get_unchecked_mut_get_probe_shadowed_receiver_path_not_guard",
        "get_unchecked_mut_get_probe_shadowed_receiver_not_guard",
        "get_unchecked_mut_get_probe_early_return_reassigned_index_not_guard",
        "get_unchecked_mut_get_probe_early_return_shadowed_index_not_guard",
        "get_unchecked_mut_get_probe_early_return_reassigned_receiver_not_guard",
        "get_unchecked_mut_get_probe_early_return_shadowed_receiver_not_guard",
        "get_unchecked_mut_if_let_get_reassigned_index_not_guard",
        "get_unchecked_mut_if_let_get_shadowed_index_not_guard",
        "get_unchecked_mut_if_let_get_reassigned_receiver_not_guard",
        "get_unchecked_mut_if_let_get_reassigned_receiver_path_not_guard",
        "get_unchecked_mut_if_let_get_shadowed_receiver_path_not_guard",
        "get_unchecked_mut_if_let_get_shadowed_receiver_not_guard",
        "get_unchecked_mut_let_else_get_reassigned_index_not_guard",
        "get_unchecked_mut_let_else_get_shadowed_index_not_guard",
        "get_unchecked_mut_let_else_get_reassigned_receiver_not_guard",
        "get_unchecked_mut_let_else_get_shadowed_receiver_not_guard",
        "get_unchecked_mut_match_get_reassigned_index_not_guard",
        "get_unchecked_mut_match_get_shadowed_index_not_guard",
        "get_unchecked_mut_match_get_reassigned_receiver_not_guard",
        "get_unchecked_mut_match_get_shadowed_receiver_not_guard",
        "static_lifetime_mut_ref_not_static_mut",
        "pin_new_unchecked",
    ];

    #[test]
    fn rendered_analysis_json_is_parseable_and_keeps_card_contract() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let value = parse_json(&render(&output))?;

        assert_eq!(value["schema_version"], "0.1");
        assert_eq!(value["tool"], "unsafe-review");
        assert_eq!(value["scope"], "diff");
        assert!(
            value["trust_boundary"]
                .as_str()
                .unwrap_or("")
                .contains("not a Miri result")
        );
        assert_eq!(value["summary"]["cards"], 1);
        assert_eq!(value["cards"][0]["class"], "guard_missing");
        assert_eq!(value["cards"][0]["site"]["file"], "src/lib.rs");
        assert_eq!(value["cards"][0]["site"]["visibility"], "private");
        assert_eq!(value["cards"][0]["site"]["public_api_surface"], false);
        assert_eq!(
            value["cards"][0]["operation"],
            "unsafe { ptr.cast::<Header>().read() }"
        );
        assert_eq!(value["cards"][0]["operation_family"], "raw_pointer_read");
        assert!(value["cards"][0]["obligation_evidence"].is_array());
        assert_eq!(value["cards"][0]["witness_routes"][0]["kind"], "miri");
        assert!(
            value["cards"][0]["next_action"]
                .as_str()
                .unwrap_or("")
                .contains("Add or expose the local guard")
        );
        assert!(value["cards"][0]["verify_commands"].is_array());
        Ok(())
    }

    #[test]
    fn fixture_card_goldens_match_rendered_json() -> Result<(), String> {
        for fixture in FIXTURE_GOLDENS {
            let output = fixture_output(fixture)?;
            let actual = parse_json(&render(&output))?;
            let expected = fixture_expected_cards(fixture)?;
            let Some(actual_cards) = actual.get("cards") else {
                return Err(format!("{fixture} JSON output is missing `cards`"));
            };
            if actual_cards != &expected {
                return Err(format!(
                    "{fixture} card JSON drifted\nexpected:\n{}\nactual:\n{}",
                    pretty_json(&expected),
                    pretty_json(actual_cards)
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn calibration_policy_fixtures_have_json_card_goldens() -> Result<(), String> {
        let registered = FIXTURE_GOLDENS
            .iter()
            .copied()
            .collect::<BTreeSet<&'static str>>();
        let policy_path = workspace_root().join("policy/accuracy-calibration.toml");
        let policy_text = fs::read_to_string(&policy_path)
            .map_err(|err| format!("read {} failed: {err}", policy_path.display()))?;
        let policy: toml::Value = toml::from_str(&policy_text)
            .map_err(|err| format!("parse {} failed: {err}", policy_path.display()))?;
        let claims = policy
            .get("claim")
            .and_then(toml::Value::as_array)
            .ok_or_else(|| "policy/accuracy-calibration.toml is missing [[claim]]".to_string())?;

        let mut missing = BTreeSet::new();
        for claim in claims {
            let Some(fixtures) = claim.get("fixtures").and_then(toml::Value::as_array) else {
                continue;
            };
            for fixture in fixtures {
                let fixture = fixture
                    .as_str()
                    .ok_or_else(|| "policy fixture entry must be a string".to_string())?;
                if fixture_root(fixture).join("expected.cards.json").exists()
                    && !registered.contains(fixture)
                {
                    missing.insert(fixture.to_owned());
                }
            }
        }

        if !missing.is_empty() {
            return Err(format!(
                "calibration fixture(s) with expected.cards.json are missing JSON golden coverage: {}",
                missing.into_iter().collect::<Vec<_>>().join(", ")
            ));
        }

        Ok(())
    }

    fn fixture_output(name: &str) -> Result<AnalyzeOutput, String> {
        let root = fixture_root(name);
        analyze(AnalyzeInput {
            root: root.clone(),
            scope: Scope::Diff,
            diff: DiffSource::File(root.join("change.diff")),
            mode: AnalysisMode::Draft,
            policy: PolicyMode::Advisory,
            include_unchanged_tests: true,
            max_cards: None,
        })
    }

    fn fixture_expected_cards(name: &str) -> Result<serde_json::Value, String> {
        let path = fixture_root(name).join("expected.cards.json");
        let text = fs::read_to_string(&path)
            .map_err(|err| format!("read {} failed: {err}", path.display()))?;
        parse_json(&text)
    }

    fn fixture_root(name: &str) -> PathBuf {
        workspace_root().join("fixtures").join(name)
    }

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn parse_json(text: &str) -> Result<serde_json::Value, String> {
        serde_json::from_str(text).map_err(|err| format!("JSON parse failed: {err}"))
    }

    fn pretty_json(value: &serde_json::Value) -> String {
        match serde_json::to_string_pretty(value) {
            Ok(text) => text,
            Err(err) => format!("<failed to render JSON: {err}>"),
        }
    }
}
