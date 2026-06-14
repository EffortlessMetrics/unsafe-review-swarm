use crate::api::{AnalyzeOutput, Provenance, Scope, Summary};
use crate::domain::coverage::compute_agent_lsp_readiness;
use crate::domain::{
    AgentLspReadiness, BaselineState, CommentPlanStatus, Coverage, CoverageBlock, EvidenceState,
    ManualContext, ObligationEvidence, OperationFamily, OutcomeMovement, ReviewCard,
    WitnessReceiptCoverage, WitnessRoute,
};
use crate::output::REVIEWCARD_TRUST_BOUNDARY as TRUST_BOUNDARY;
use crate::output::agent::card_has_scoped_repairs;
use crate::output::comment_plan;
use crate::output::confirmation::ConfirmationCue;
use crate::util::path_display;
use serde::Serialize;

/// Schema version for the plain (no-provenance) JSON analyze artifact.
const SCHEMA_VERSION_PLAIN: &str = "0.1";

/// Schema version for the JSON analyze artifact with provenance block.
const SCHEMA_VERSION_WITH_PROVENANCE: &str = "0.2";

pub(crate) fn render(output: &AnalyzeOutput) -> String {
    render_pretty(&JsonAnalyzeOutput::from_plain(output))
}

/// Render the JSON analyze artifact with an attached provenance block (schema 0.2).
pub(crate) fn render_with_provenance(output: &AnalyzeOutput, provenance: &Provenance) -> String {
    render_pretty(&JsonAnalyzeOutput::from_with_provenance(output, provenance))
}

fn render_pretty(value: &impl Serialize) -> String {
    match serde_json::to_string_pretty(value) {
        Ok(text) => text,
        Err(err) => format!("{{\n  \"error\": \"json serialization failed: {err}\"\n}}"),
    }
}

#[derive(Serialize)]
struct JsonAnalyzeOutput<'a> {
    schema_version: &'static str,
    tool: &'a str,
    /// Semver tool version. Present in all schema 0.2+ artifacts; also appears nested
    /// in the provenance block for consumers that parse that block exclusively.
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_version: Option<&'static str>,
    scope: &'static str,
    mode: &'static str,
    policy: &'static str,
    trust_boundary: &'static str,
    root: String,
    summary: JsonSummary,
    cards: Vec<JsonCard<'a>>,
    /// Traceable evidence metadata (schema 0.2+). Absent in 0.1 artifacts for
    /// backward compatibility; `schema_version` distinguishes the two shapes.
    #[serde(skip_serializing_if = "Option::is_none")]
    provenance: Option<JsonProvenance>,
}

/// JSON projection of [`Provenance`].
#[derive(Serialize)]
struct JsonProvenance {
    tool_version: &'static str,
    generated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    root_abs: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    base_sha: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    head_sha: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    diff_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    diff_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dirty_worktree: Option<bool>,
}

impl<'a> JsonAnalyzeOutput<'a> {
    /// Build from output without provenance (schema 0.1, backward-compatible).
    fn from_plain(output: &'a AnalyzeOutput) -> Self {
        let statuses = comment_plan::card_statuses(output);
        Self {
            schema_version: SCHEMA_VERSION_PLAIN,
            tool: &output.tool,
            tool_version: None,
            scope: scope_str(output),
            mode: output.mode.as_str(),
            policy: output.policy.as_str(),
            trust_boundary: TRUST_BOUNDARY,
            root: path_display(&output.root),
            summary: JsonSummary::from(&output.summary),
            cards: output
                .cards
                .iter()
                .map(|card| {
                    let status = statuses
                        .get(&card.id)
                        .copied()
                        .unwrap_or(CommentPlanStatus::NotEligible);
                    JsonCard::from_with_status(card, status)
                })
                .collect(),
            provenance: None,
        }
    }

    /// Build from output with provenance block (schema 0.2).
    fn from_with_provenance(output: &'a AnalyzeOutput, provenance: &Provenance) -> Self {
        let json_provenance = JsonProvenance {
            tool_version: env!("CARGO_PKG_VERSION"),
            generated_at: provenance.generated_at.clone(),
            root_abs: provenance.root_abs.clone(),
            base_sha: provenance.base_sha.clone(),
            head_sha: provenance.head_sha.clone(),
            diff_path: provenance.diff_path.clone(),
            diff_sha256: provenance.diff_sha256.clone(),
            dirty_worktree: provenance.dirty_worktree,
        };
        let statuses = comment_plan::card_statuses(output);
        Self {
            schema_version: SCHEMA_VERSION_WITH_PROVENANCE,
            tool: &output.tool,
            tool_version: Some(env!("CARGO_PKG_VERSION")),
            scope: scope_str(output),
            mode: output.mode.as_str(),
            policy: output.policy.as_str(),
            trust_boundary: TRUST_BOUNDARY,
            root: path_display(&output.root),
            summary: JsonSummary::from(&output.summary),
            cards: output
                .cards
                .iter()
                .map(|card| {
                    let status = statuses
                        .get(&card.id)
                        .copied()
                        .unwrap_or(CommentPlanStatus::NotEligible);
                    JsonCard::from_with_status(card, status)
                })
                .collect(),
            provenance: Some(json_provenance),
        }
    }
}

#[derive(Serialize)]
struct JsonSummary {
    rust_files: usize,
    changed_files: usize,
    changed_rust_files: usize,
    changed_non_rust_files: usize,
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
    /// Coverage movement counts (SPEC-0030).
    new_gaps: usize,
    worsened_gaps: usize,
    /// Baseline cards whose evidence coverage improved (pure improvement: at least one slot
    /// advanced, no slot regressed).  Always 0 until a baseline coverage snapshot exists.
    ///
    /// An improved card is still advisory, still open, still present — NOT resolved, NOT safe,
    /// NOT UB-free, NOT Miri-clean, and NOT a site-execution claim.
    improved_gaps: usize,
    resolved_gaps: usize,
    inherited_gaps: usize,
}

impl From<&Summary> for JsonSummary {
    fn from(summary: &Summary) -> Self {
        Self {
            rust_files: summary.rust_files,
            changed_files: summary.changed_files,
            changed_rust_files: summary.changed_rust_files,
            changed_non_rust_files: summary.changed_non_rust_files,
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
            new_gaps: summary.new_gaps,
            worsened_gaps: summary.worsened_gaps,
            improved_gaps: summary.improved_gaps,
            resolved_gaps: summary.resolved_gaps,
            inherited_gaps: summary.inherited_gaps,
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
    proof_path: &'static str,
    site: JsonSite<'a>,
    operation: &'a str,
    operation_family: &'static str,
    /// Advisory static sub-class hint for stable-byte-source operation families.
    /// Absent for all other operation families (backward-compatible via skip).
    /// This is a heuristic aperture label, not a memory-safety proof, UB-free
    /// status, Miri-clean status, or site-execution claim.
    #[serde(skip_serializing_if = "Option::is_none")]
    stable_byte_sub_class: Option<&'static str>,
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
    confirmation_cue: ConfirmationCue,
    coverage: JsonCoverageBlock,
}

impl<'a> JsonCard<'a> {
    /// Build a `JsonCard` for `card`, overriding `comment_plan_status` with
    /// the value computed by the comment-plan selection pass (SPEC-0032).
    ///
    /// This is the only valid constructor.  Callers must supply the status
    /// from [`comment_plan::card_statuses`] so that `cards.json` always
    /// projects the same `comment_plan_status` as `comment-plan.json`.
    fn from_with_status(card: &'a ReviewCard, comment_plan_status: CommentPlanStatus) -> Self {
        let mut coverage_block = card.coverage_block();
        coverage_block.comment_plan_status = comment_plan_status;
        // Guarantee: cards.json coverage.agent_lsp_readiness uses the exact
        // has_card_scoped_repairs value (output audit #1687, findings 3+4).
        // Both cards.json and the agent packet now call compute_agent_lsp_readiness
        // with the same has_card_scoped_repairs so neither surface can diverge.
        coverage_block.agent_lsp_readiness =
            compute_agent_lsp_readiness(card, card_has_scoped_repairs(card)).state;
        Self {
            id: &card.id.0,
            class_name: card.class.as_str(),
            priority: card.priority.as_str(),
            confidence: card.confidence.as_str(),
            proof_path: card.proof_path.as_str(),
            site: JsonSite::from(card),
            operation: &card.operation.expression,
            operation_family: card.operation.family.as_str(),
            stable_byte_sub_class: stable_byte_sub_class(&card.operation.family),
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
            confirmation_cue: ConfirmationCue::from(card),
            coverage: JsonCoverageBlock::from(coverage_block),
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

/// Return the advisory stable-byte sub-class hint for the four `StableByteSource*`
/// operation families, or `None` for all other families.
///
/// This is a heuristic aperture label. It does not prove memory-safety, UB-free
/// status, Miri-clean status, or site execution.
fn stable_byte_sub_class(family: &OperationFamily) -> Option<&'static str> {
    match family {
        OperationFamily::StableByteSourceGetterReentry => Some("getter-reentry"),
        OperationFamily::StableByteSourceRabAsync => Some("rab-async"),
        OperationFamily::StableByteSourceSabRace => Some("sab-race"),
        OperationFamily::StableByteSourceNativeFfiRead => Some("native-ffi-read"),
        _ => None,
    }
}

/// JSON projection of a card's machine-readable coverage block (SPEC-0029).
#[derive(Serialize)]
struct JsonCoverageBlock {
    contract_coverage: &'static str,
    guard_coverage: &'static str,
    test_reach_coverage: &'static str,
    witness_receipt_coverage: &'static str,
    manual_context: &'static str,
    baseline_state: &'static str,
    outcome_movement: &'static str,
    comment_plan_status: &'static str,
    agent_lsp_readiness: &'static str,
}

impl From<CoverageBlock> for JsonCoverageBlock {
    fn from(block: CoverageBlock) -> Self {
        Self {
            contract_coverage: coverage_str(block.contract_coverage),
            guard_coverage: coverage_str(block.guard_coverage),
            test_reach_coverage: coverage_str(block.test_reach_coverage),
            witness_receipt_coverage: witness_receipt_str(block.witness_receipt_coverage),
            manual_context: manual_context_str(block.manual_context),
            baseline_state: baseline_state_str(block.baseline_state),
            outcome_movement: outcome_movement_str(block.outcome_movement),
            comment_plan_status: comment_plan_status_str(block.comment_plan_status),
            agent_lsp_readiness: agent_lsp_readiness_str(block.agent_lsp_readiness),
        }
    }
}

fn coverage_str(coverage: Coverage) -> &'static str {
    coverage.as_str()
}

fn witness_receipt_str(coverage: WitnessReceiptCoverage) -> &'static str {
    coverage.as_str()
}

fn manual_context_str(context: ManualContext) -> &'static str {
    context.as_str()
}

fn baseline_state_str(state: BaselineState) -> &'static str {
    state.as_str()
}

fn outcome_movement_str(movement: OutcomeMovement) -> &'static str {
    movement.as_str()
}

fn comment_plan_status_str(status: CommentPlanStatus) -> &'static str {
    status.as_str()
}

fn agent_lsp_readiness_str(readiness: AgentLspReadiness) -> &'static str {
    readiness.as_str()
}

/// All fixture names that have `expected.cards.json` goldens.
///
/// Shared between the public [`bless_fixture_card_goldens`] helper and the
/// `#[cfg(test)]` drift test.  Lives at module scope so it is accessible from
/// both a non-test `pub fn` and the test module below.
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
    "pub_crate_unsafe_fn_missing_safety",
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
    "nonnull_method_receiver_shadowed_not_guard",
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
    "js_buffer_reentry_sync_compression",
    "js_buffer_reentry_async_helper_capture",
    "js_buffer_reentry_node_fs_rab_scalar_write",
    "js_buffer_reentry_node_fs_rab_encoded_write_file",
    "js_buffer_reentry_raw_parts_materialization",
    "js_buffer_reentry_coerce_after_as_array_buffer",
    "js_buffer_reentry_vector_materialization",
    "js_buffer_reentry_as_ptr_materialization",
    "js_buffer_stale_span_slice_index_after_reentry",
    "js_buffer_stale_span_stale_detached_check",
    "stable_byte_native_ffi_zstd_handoff",
    "stable_byte_native_ffi_zstd_owned_copy_control",
    "stable_byte_sab_borrowed_slice",
    "stable_byte_sab_mysql_blob_rawslice",
    "stable_byte_sab_snapshot_no_card",
    "stable_byte_sab_mysql_blob_owned_copy_no_card",
    "js_buffer_reentry_options_before_capture_no_card",
    "js_buffer_reentry_recapture_after_reentry_no_card",
    "js_buffer_reentry_refetch_after_coercion_no_card",
    "js_buffer_reentry_vector_refetch_after_coercion_no_card",
    "js_buffer_reentry_as_ptr_refetch_after_coercion_no_card",
    "js_buffer_reentry_async_options_before_capture_no_card",
    "js_buffer_reentry_async_recapture_after_reentry_no_card",
    "js_buffer_reentry_node_fs_rab_scalar_write_scheduled_before_capture_no_card",
    "js_buffer_reentry_node_fs_rab_encoded_write_scheduled_before_capture_no_card",
    "js_buffer_reentry_node_fs_rab_encoded_write_recapture_after_dispatch_no_card",
    "js_buffer_stale_span_refetch_before_use_no_card",
    "js_buffer_stale_span_pinned_before_use_no_card",
    "js_buffer_stale_span_use_before_reentry_no_card",
    "js_buffer_stale_span_passed_as_arg_after_reentry",
    "js_buffer_stale_span_snapshot_before_use_no_card",
    "panic_from_safe_js_direct_try_from_expect",
    "panic_from_safe_js_bound_try_from_unwrap",
    "panic_from_safe_js_observed_only_not_guard",
    "panic_from_safe_js_inline_max_no_card",
    "panic_from_safe_js_return_guard_no_card",
    "panic_from_safe_js_non_js_signed_no_card",
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
    "multiline_ptr_copy",
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
    "str_from_utf8_unchecked_is_err_return_string_not_guard",
    "str_from_utf8_unchecked_is_err_return_reassigned_not_guard",
    "str_from_utf8_unchecked_is_err_return_shadowed_not_guard",
    "str_from_utf8_unchecked_question_mark_guard",
    "str_from_utf8_unchecked_question_mark_comment_not_guard",
    "str_from_utf8_unchecked_question_mark_string_not_guard",
    "str_from_utf8_unchecked_match_return_guard",
    "str_from_utf8_unchecked_match_return_comment_not_guard",
    "str_from_utf8_unchecked_match_return_string_not_guard",
    "str_from_utf8_unchecked_match_err_reassigned_not_guard",
    "str_from_utf8_unchecked_match_err_shadowed_not_guard",
    "str_from_utf8_unchecked_post_validation_not_guard",
    "str_from_utf8_unchecked_other_buffer_not_guard",
    "str_from_utf8_unchecked_prefix_validation_not_guard",
    "str_from_utf8_unchecked_suffix_validation_not_guard",
    "str_from_utf8_unchecked_is_ok_observed_not_guard",
    "str_from_utf8_unchecked_is_ok_comment_not_guard",
    "str_from_utf8_unchecked_is_ok_string_not_guard",
    "str_from_utf8_unchecked_guard_then_reassigned_not_guard",
    "str_from_utf8_unchecked_guard_then_mutated_not_guard",
    "str_from_utf8_unchecked_if_let_ok_guard",
    "str_from_utf8_unchecked_if_let_ok_comment_not_guard",
    "str_from_utf8_unchecked_if_let_ok_string_not_guard",
    "str_from_utf8_unchecked_if_let_ok_reassigned_not_guard",
    "str_from_utf8_unchecked_if_let_ok_shadowed_not_guard",
    "str_from_utf8_unchecked_if_let_err_return_guard",
    "str_from_utf8_unchecked_if_let_err_return_comment_not_guard",
    "str_from_utf8_unchecked_if_let_err_return_string_not_guard",
    "str_from_utf8_unchecked_if_let_err_reassigned_not_guard",
    "str_from_utf8_unchecked_if_let_err_shadowed_not_guard",
    "str_from_utf8_unchecked_guard_then_shadowed_not_guard",
    "str_from_utf8_unchecked_let_else_ok_guard",
    "str_from_utf8_unchecked_let_else_ok_comment_not_guard",
    "str_from_utf8_unchecked_let_else_ok_string_not_guard",
    "str_from_utf8_unchecked_let_else_ok_reassigned_not_guard",
    "str_from_utf8_unchecked_let_else_ok_shadowed_not_guard",
    "str_from_utf8_unchecked_match_ok_guard",
    "str_from_utf8_unchecked_match_ok_comment_not_guard",
    "str_from_utf8_unchecked_match_ok_string_not_guard",
    "str_from_utf8_unchecked_match_ok_reassigned_not_guard",
    "str_from_utf8_unchecked_match_ok_shadowed_not_guard",
    "from_utf8_unchecked_safe_wrapper_no_cards",
    "zeroed_invalid_value",
    "zeroed_valid_u32",
    "zeroed_safe_wrapper_no_cards",
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
    "pointer_arithmetic_safe_method_add_no_cards",
    "pointer_arithmetic_unsafe_fn_offset",
    "slice_from_raw_parts_mut",
    "slice_from_raw_parts_mut_maybeuninit",
    "slice_from_raw_parts_mut_other_maybeuninit_not_guard",
    "from_raw_parts_safe_ctor_no_cards",
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
    "assume_init_safe_method_no_cards",
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
    "set_len_safe_method_no_cards",
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
    "ffi_same_named_method_not_route",
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
    "get_unchecked_mut_let_else_get_reassigned_receiver_path_not_guard",
    "get_unchecked_mut_let_else_get_shadowed_receiver_path_not_guard",
    "get_unchecked_mut_let_else_get_shadowed_receiver_not_guard",
    "get_unchecked_mut_match_get_reassigned_index_not_guard",
    "get_unchecked_mut_match_get_shadowed_index_not_guard",
    "get_unchecked_mut_match_get_reassigned_receiver_not_guard",
    "get_unchecked_mut_match_get_reassigned_receiver_path_not_guard",
    "get_unchecked_mut_match_get_shadowed_receiver_path_not_guard",
    "get_unchecked_mut_match_get_shadowed_receiver_not_guard",
    "static_lifetime_mut_ref_not_static_mut",
    "pin_new_unchecked",
    "unsafe_fn_unknown_family_no_card",
    "unsafe_fn_pointer_field_no_cards",
];

/// Regenerate `expected.cards.json` for each named fixture (or all
/// [`FIXTURE_GOLDENS`] fixtures if `names` is empty), always writing LF line
/// endings.
///
/// Called by `cargo run -p xtask -- bless-goldens [fixture ...]`.
/// Does not execute witnesses or assess soundness; reviewing the diff after
/// blessing is the developer's responsibility (same posture as `badges --out`).
pub fn bless_fixture_card_goldens(names: &[&str]) -> Result<Vec<std::path::PathBuf>, String> {
    use crate::api::{AnalysisMode, AnalyzeInput, DiffSource, PolicyMode, Scope, analyze};
    use std::fs;
    use std::path::PathBuf;

    let targets: &[&str] = if names.is_empty() {
        FIXTURE_GOLDENS
    } else {
        names
    };
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut written = Vec::new();
    for &fixture in targets {
        let root = workspace.join("fixtures").join(fixture);
        let output = analyze(AnalyzeInput {
            root: root.clone(),
            scope: Scope::Diff,
            diff: DiffSource::File(root.join("change.diff")),
            mode: AnalysisMode::Draft,
            policy: PolicyMode::Advisory,
            include_unchanged_tests: true,
            max_cards: None,
        })?;
        let statuses = crate::output::comment_plan::card_statuses(&output);
        let cards: Vec<JsonCard<'_>> = output
            .cards
            .iter()
            .map(|card| {
                let status = statuses
                    .get(&card.id)
                    .copied()
                    .unwrap_or(CommentPlanStatus::NotEligible);
                JsonCard::from_with_status(card, status)
            })
            .collect();
        let path = root.join("expected.cards.json");
        let mut text = serde_json::to_string_pretty(&cards)
            .map_err(|err| format!("serialize {fixture} cards failed: {err}"))?;
        text.push('\n');
        // Ensure LF line endings (the repo is LF-only; guard against Windows writers).
        let text = text.replace("\r\n", "\n");
        fs::write(&path, text.as_bytes())
            .map_err(|err| format!("write {} failed: {err}", path.display()))?;
        written.push(path);
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{AnalysisMode, AnalyzeInput, DiffSource, PolicyMode, analyze};
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::PathBuf;

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
                .contains("not a site-execution claim")
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
                .contains("Add or expose local guards")
        );
        assert!(value["cards"][0]["verify_commands"].is_array());
        assert_eq!(
            value["cards"][0]["confirmation_cue"]["build_this_first"]["kind"],
            "verify_command"
        );
        assert_eq!(
            value["cards"][0]["confirmation_cue"]["confirmation_state"],
            "pending"
        );
        assert_eq!(
            value["cards"][0]["confirmation_cue"]["runtime_executed"],
            false
        );
        assert!(
            value["cards"][0]["confirmation_cue"]["hypothesis_to_confirm"]
                .as_str()
                .unwrap_or("")
                .contains("confirm with external evidence")
        );
        assert!(
            value["cards"][0]["confirmation_cue"]["minimal_repro"]["limitation"]
                .as_str()
                .unwrap_or("")
                .contains("unsafe-review did not run this command")
        );
        Ok(())
    }

    #[test]
    fn confirmation_cue_projects_runtime_receipt_verdict_state() -> Result<(), String> {
        use crate::domain::WitnessEvidence;

        let mut output = fixture_output("raw_pointer_alignment")?;
        let card = output
            .cards
            .first_mut()
            .ok_or_else(|| "alignment fixture should emit a card".to_string())?;
        card.witness = WitnessEvidence::present("miri receipt imported")
            .with_runtime_executed(true)
            .with_verdict(Some("not_reproduced".to_string()));

        let value = parse_json(&render(&output))?;

        assert_eq!(
            value["cards"][0]["confirmation_cue"]["confirmation_state"],
            "not_reproduced"
        );
        assert_eq!(
            value["cards"][0]["confirmation_cue"]["runtime_executed"],
            true
        );
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

    /// Regenerate all `expected.cards.json` golden files from the current output.
    ///
    /// Run with `UPDATE_GOLDENS=1 cargo test -p unsafe-review-core bless_fixture_card_goldens`
    /// after intentional changes to the JSON card shape (e.g. adding a new field).
    #[test]
    fn bless_fixture_card_goldens() -> Result<(), String> {
        if std::env::var("UPDATE_GOLDENS").as_deref() != Ok("1") {
            return Ok(());
        }
        for fixture in FIXTURE_GOLDENS {
            let output = fixture_output(fixture)?;
            // Serialize the typed Vec<JsonCard> directly so that serde emits
            // keys in struct-field order (not alphabetically as serde_json::Value
            // would after a round-trip through BTreeMap).
            let statuses = comment_plan::card_statuses(&output);
            let cards: Vec<JsonCard<'_>> = output
                .cards
                .iter()
                .map(|card| {
                    let status = statuses
                        .get(&card.id)
                        .copied()
                        .unwrap_or(CommentPlanStatus::NotEligible);
                    JsonCard::from_with_status(card, status)
                })
                .collect();
            let path = fixture_root(fixture).join("expected.cards.json");
            let mut text = serde_json::to_string_pretty(&cards)
                .map_err(|err| format!("serialize {fixture} cards failed: {err}"))?;
            text.push('\n');
            // Ensure LF line endings (the repo is LF-only).
            let text = text.replace("\r\n", "\n");
            fs::write(&path, text.as_bytes())
                .map_err(|err| format!("write {} failed: {err}", path.display()))?;
        }
        println!(
            "bless_fixture_card_goldens: updated {} fixtures",
            FIXTURE_GOLDENS.len()
        );
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

    /// `render_with_provenance` emits schema 0.2 with `tool_version` top-level
    /// and a `provenance` block; `render` still emits 0.1 (backward compat).
    #[test]
    fn provenance_render_emits_schema_0_2_with_tool_version() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let plain = parse_json(&render(&output))?;
        assert_eq!(plain["schema_version"], "0.1");
        assert!(
            plain["tool_version"].is_null(),
            "plain render must omit tool_version"
        );
        assert!(
            plain["provenance"].is_null(),
            "plain render must omit provenance block"
        );

        let prov = crate::api::Provenance {
            generated_at: "2026-06-07T00:00:00Z".to_string(),
            ..Default::default()
        };
        let with_prov = parse_json(&render_with_provenance(&output, &prov))?;
        assert_eq!(with_prov["schema_version"], "0.2");
        assert!(
            with_prov["tool_version"].is_string(),
            "tool_version must be present"
        );
        assert!(
            with_prov["provenance"].is_object(),
            "provenance block must be present"
        );
        assert_eq!(
            with_prov["provenance"]["generated_at"],
            "2026-06-07T00:00:00Z"
        );
        assert_eq!(
            with_prov["provenance"]["tool_version"], with_prov["tool_version"],
            "top-level tool_version must match provenance.tool_version"
        );
        Ok(())
    }

    /// `diff_path` and `diff_sha256` round-trip into the provenance block.
    #[test]
    fn provenance_diff_mode_carries_path_and_sha256() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let diff_content = b"--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-old\n+new\n";
        let expected_sha = crate::sha256_hex_of(diff_content);

        let prov = crate::api::Provenance {
            generated_at: "2026-06-07T12:00:00Z".to_string(),
            diff_path: Some("fixtures/raw_pointer_alignment/change.diff".to_string()),
            diff_sha256: Some(expected_sha.clone()),
            ..Default::default()
        };
        let value = parse_json(&render_with_provenance(&output, &prov))?;
        assert_eq!(
            value["provenance"]["diff_path"],
            "fixtures/raw_pointer_alignment/change.diff"
        );
        assert_eq!(value["provenance"]["diff_sha256"], expected_sha.as_str());
        assert_eq!(
            expected_sha.len(),
            64,
            "sha256_hex_of must produce 64 hex chars"
        );
        Ok(())
    }

    /// `generated_at` in the provenance block is a syntactically valid RFC3339 UTC timestamp.
    #[test]
    fn provenance_generated_at_is_rfc3339_utc() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let prov = crate::api::Provenance::new_now();
        let value = parse_json(&render_with_provenance(&output, &prov))?;
        let ts = value["provenance"]["generated_at"]
            .as_str()
            .ok_or("generated_at missing")?;
        // RFC3339 UTC: YYYY-MM-DDTHH:MM:SSZ
        assert!(ts.len() >= 20, "generated_at too short: {ts}");
        assert!(ts.ends_with('Z'), "generated_at must end with Z: {ts}");
        assert!(
            ts.contains('T'),
            "generated_at must contain T separator: {ts}"
        );
        Ok(())
    }
}
