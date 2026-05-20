# Support tiers

All tiers describe static review evidence. None means memory-safety proof.

For a concise front panel, see [`SUPPORT_SUMMARY.md`](SUPPORT_SUMMARY.md). This
file remains the detailed claim-to-proof ledger.

Recent core smoke proof additions include
`get_unchecked_mut_bounds_observed_not_guard`,
`get_unchecked_mut_closed_bounds_not_guard`, and
`get_unchecked_mut_reassigned_index_not_guard`, which pin that positive bounds
branches must still dominate `get_unchecked`, bare predicate observations are
not guards, and reassigned checked indexes do not discharge bounds evidence.
Recent guard-evidence additions include
`raw_pointer_alignment_is_aligned_guard`,
`raw_pointer_write_alignment_guard`,
`raw_pointer_alignment_observed_not_guard`,
`raw_pointer_write_alignment_observed_not_guard`,
`raw_pointer_write_alignment_closed_branch_not_guard`,
`raw_pointer_write_alignment_post_check_not_guard`,
`raw_pointer_read_null_guard`,
`raw_pointer_read_null_observed_not_guard`,
`raw_pointer_read_null_other_pointer_not_guard`,
`raw_pointer_read_null_post_check_not_guard`,
`raw_pointer_read_unaligned_null_guard`,
`raw_pointer_read_unaligned_null_observed_not_guard`,
`raw_pointer_read_unaligned_null_other_pointer_not_guard`,
`raw_pointer_read_unaligned_null_post_check_not_guard`,
`raw_pointer_read_volatile_null_guard`,
`raw_pointer_read_volatile_null_observed_not_guard`,
`raw_pointer_read_volatile_null_other_pointer_not_guard`,
`raw_pointer_read_volatile_null_post_check_not_guard`,
`raw_pointer_read_volatile_alignment_guard`,
`raw_pointer_read_volatile_alignment_observed_not_guard`,
`raw_pointer_read_volatile_alignment_other_pointer_not_guard`,
`raw_pointer_read_volatile_alignment_post_check_not_guard`,
`raw_pointer_write_null_guard`,
`raw_pointer_write_null_observed_not_guard`,
`raw_pointer_write_null_other_pointer_not_guard`,
`raw_pointer_write_null_post_check_not_guard`,
`raw_pointer_write_unaligned_null_guard`,
`raw_pointer_write_unaligned_null_observed_not_guard`,
`raw_pointer_write_unaligned_null_other_pointer_not_guard`,
`raw_pointer_write_unaligned_null_post_check_not_guard`,
`raw_pointer_write_volatile_null_guard`,
`raw_pointer_write_volatile_null_observed_not_guard`,
`raw_pointer_write_volatile_null_other_pointer_not_guard`,
`raw_pointer_write_volatile_null_post_check_not_guard`,
`raw_pointer_write_volatile_alignment_guard`,
`raw_pointer_write_volatile_alignment_observed_not_guard`,
`raw_pointer_write_volatile_alignment_other_pointer_not_guard`,
`raw_pointer_write_volatile_alignment_post_check_not_guard`,
`raw_pointer_bounds_observed_not_guard`,
`raw_pointer_bounds_closed_branch_not_guard`,
`raw_pointer_bounds_post_check_not_guard`,
`raw_pointer_write_bounds_observed_not_guard`,
`raw_pointer_write_bounds_closed_branch_not_guard`,
`raw_pointer_write_bounds_post_check_not_guard`,
`raw_pointer_alignment_closed_branch_not_guard`, and
`raw_pointer_alignment_reassigned_pointer_not_guard`, which pin same-pointer
`is_aligned` guard evidence and executable raw-pointer bounds evidence without
treating observations, closed branches, post-use checks, generic type angle
brackets, or stale checked pointers as discharge.
The same stale-evidence controls are also pinned for same-pointer modulo
alignment checks through `raw_pointer_alignment_modulo_guard`,
`raw_pointer_alignment_modulo_observed_not_guard`,
`raw_pointer_alignment_modulo_closed_branch_not_guard`, and
`raw_pointer_alignment_modulo_reassigned_pointer_not_guard`.
`Vec::from_raw_parts` capacity evidence is pinned for same-call len/cap
assertions and invalid-path early returns, and rejects bare relation
observations, closed positive branches, and reassigned checked cap arguments.
`Vec::set_len` capacity evidence accepts direct same-vector capacity assertions,
open positive capacity branches, and invalid-length early returns, and rejects
observations, closed branches, stale checked lengths, stale checked receivers,
stale checked lengths or receivers inside open branches, stale capacity
bindings, unrelated comparisons, and unrelated local arguments merely named
`cap` unless a const-capacity context is visible.
Same-vector `Vec::with_capacity(new_len)` evidence also rejects reassigned
vector bindings and reassigned checked lengths.
`Box::from_raw` and `ptr::drop_in_place` reject `Box::into_raw` origin evidence
when the origin is for a different pointer, the raw pointer is reassigned before
use, or the origin appears only after the unsafe operation.
Unchecked-constructor availability evidence is pinned for same-receiver
assertions, enclosing positive branches, and unavailable-path early returns, and
rejects other receivers, bare observations, and closed positive branches.
Pointer-arithmetic `num_ctrl_bytes` evidence is pinned for matching operation
arguments, enclosing in-bounds branches, and invalid-path early returns, and
rejects unrelated checked indexes, bare predicate observations, closed branches,
out-of-bounds operation branches, and post-use checks.
Raw pointer read len/capacity evidence is pinned for same-source assertions and
rejects another container's len/capacity assertion, bare equality observations,
and closed equality branches.

Copy operation range evidence is intentionally conservative:
`copy_nonoverlapping_slice_range_guard`,
`copy_nonoverlapping_slice_range_conjunctive_assert_guard`,
`copy_nonoverlapping_slice_range_early_return_guard`,
`copy_nonoverlapping_slice_range_disjunctive_early_return_guard`,
`copy_nonoverlapping_slice_range_disjunctive_early_return_after_block_guard`,
`copy_nonoverlapping_slice_range_open_branch_guard`,
`copy_nonoverlapping_slice_range_conjunctive_open_branch_guard`,
`ptr_copy_slice_range_guard`,
`ptr_copy_slice_range_conjunctive_assert_guard`,
`ptr_copy_slice_range_early_return_guard`,
`ptr_copy_slice_range_disjunctive_early_return_guard`,
`ptr_copy_slice_range_disjunctive_early_return_after_block_guard`,
`ptr_copy_slice_range_open_branch_guard`, and
`ptr_copy_slice_range_conjunctive_open_branch_guard` pin same-call source and
destination slice length assertions, conjunctive assertions, early returns, or
disjunctive invalid-range early returns, or open branches as valid-range
evidence, while
`copy_nonoverlapping_slice_range_src_only_not_guard`,
`copy_nonoverlapping_slice_range_dst_only_not_guard`,
`ptr_copy_slice_range_src_only_not_guard`,
`ptr_copy_slice_range_dst_only_not_guard`,
`copy_nonoverlapping_slice_range_closed_branch_not_guard`,
`ptr_copy_slice_range_closed_branch_not_guard`,
`copy_nonoverlapping_slice_range_or_branch_not_guard`,
`ptr_copy_slice_range_or_branch_not_guard`,
`copy_nonoverlapping_slice_range_commented_assert_not_guard`,
`ptr_copy_slice_range_commented_assert_not_guard`,
`copy_nonoverlapping_slice_range_disjunctive_early_return_line_comment_not_guard`,
`ptr_copy_slice_range_disjunctive_early_return_line_comment_not_guard`,
`copy_nonoverlapping_slice_range_disjunctive_early_return_block_comment_not_guard`,
`ptr_copy_slice_range_disjunctive_early_return_block_comment_not_guard`,
`copy_nonoverlapping_slice_range_disjunctive_early_return_string_literal_not_guard`,
`ptr_copy_slice_range_disjunctive_early_return_string_literal_not_guard`,
`copy_nonoverlapping_slice_range_disjunctive_nested_return_not_guard`,
`ptr_copy_slice_range_disjunctive_nested_return_not_guard`,
`copy_nonoverlapping_slice_range_disjunctive_early_return_reassigned_count_not_guard`,
`ptr_copy_slice_range_disjunctive_early_return_reassigned_count_not_guard`,
`copy_nonoverlapping_slice_range_open_branch_reassigned_count_not_guard`,
`copy_nonoverlapping_slice_range_open_branch_reassigned_src_not_guard`,
`ptr_copy_slice_range_open_branch_reassigned_count_not_guard`,
`ptr_copy_slice_range_open_branch_reassigned_dst_not_guard`,
`copy_nonoverlapping_slice_range_reassigned_count_not_guard`,
`copy_nonoverlapping_slice_range_reassigned_src_not_guard`,
`ptr_copy_slice_range_reassigned_count_not_guard`,
`ptr_copy_slice_range_reassigned_dst_not_guard`,
`copy_nonoverlapping_other_len_not_guard`, and `ptr_copy_other_len_not_guard`
pin that one-sided, closed-branch, disjunctive positive-branch, comment-only
early-return text, stale count, stale receiver, stale open-branch, or unrelated
slice length assertions do not discharge the source/destination range obligation.

| Capability | Tier | Surface | Proof | Known limits |
|---|---|---|---|---|
| Diff unsafe site inventory | experimental | CLI JSON/human | syntax-backed fixture goldens for unsafe blocks, split unsafe blocks, raw pointer operations, long unsafe-function owner inference, macro owner inference, and exact false-positive controls including `safe_code_no_cards`, `imports_not_unsafe_operations`, `cfg_target_feature_not_operation`, `attributed_unsafe_fn_no_duplicate`, `inline_unsafe_raw_pointer_deref_no_duplicate`, `impl_trait_bound_owner_inference`, `long_unsafe_fn_owner_inference`, `macro_rules_owner_inference`, `nested_unsafe_operation_call_dedupe`, and `adjacent_unchanged_unsafe_fn_no_card` | source-based, not MIR |
| Review-card JSON schema | experimental | CLI JSON | serde-backed DTOs, `schema_version`, top-level trust boundary, site visibility/public API surface fields, and `fixture_card_goldens_match_rendered_json` | fixture corpus is still small; no schema compatibility promise yet |
| Review-card identity | experimental | card `id` | `card_identity` tests cover line drift and duplicate counted identities | exact identity is consumed by baselines, suppressions, and receipts, but broader drift behavior still needs dogfood |
| Advisory baseline/suppression matching | experimental | policy ledgers / card class | xtask ledger tests plus analyzer tests for exact `baseline_known` and `suppressed` card identity matches | exact identity only; no broad suppressions and no blocking policy |
| Explicit no-new-debt mode | experimental | `--policy no-new-debt` | CLI parser tests and e2e cover nonzero exit for unbaselined actionable gaps and success for exact baseline matches | opt-in only; not default, not calibrated blocking, and no broad suppression patterns |
| Advisory no-new-debt policy report | experimental | `policy report` | core report tests, CLI parser tests, and CLI e2e cover JSON/Markdown reporting of new gaps, baseline-known cards, resolved/unmatched baseline entries, expired suppressions, ledger owner/reason/evidence/review metadata, operation expression, operation family, policy reasons, classification explanations, limitations, next action, and trust-boundary wording | advisory report only; does not block, execute witnesses, broaden suppressions, or change card classification |
| Raw pointer card slice | experimental | cards | `raw_pointer_alignment`, `raw_pointer_alignment_post_check_not_guard`, `raw_pointer_bounds_observed_not_guard`, `raw_pointer_bounds_closed_branch_not_guard`, `raw_pointer_bounds_post_check_not_guard`, `raw_pointer_deref`, `raw_pointer_read_unaligned`, `raw_pointer_read_unaligned_null_guard`, `raw_pointer_read_unaligned_null_observed_not_guard`, `raw_pointer_read_unaligned_null_other_pointer_not_guard`, `raw_pointer_read_unaligned_null_post_check_not_guard`, `raw_pointer_read_volatile`, `raw_pointer_read_volatile_alignment_guard`, `raw_pointer_read_volatile_alignment_observed_not_guard`, `raw_pointer_read_volatile_alignment_other_pointer_not_guard`, `raw_pointer_read_volatile_alignment_post_check_not_guard`, `raw_pointer_read_volatile_null_guard`, `raw_pointer_read_volatile_null_observed_not_guard`, `raw_pointer_read_volatile_null_other_pointer_not_guard`, `raw_pointer_read_volatile_null_post_check_not_guard`, `raw_pointer_read_len_capacity_assert`, `raw_pointer_read_len_capacity_other_values_not_guard`, `raw_pointer_read_len_capacity_observed_not_guard`, `raw_pointer_read_len_capacity_closed_branch_not_guard`, `raw_pointer_read_null_guard`, `raw_pointer_read_null_observed_not_guard`, `raw_pointer_read_null_other_pointer_not_guard`, `raw_pointer_read_null_post_check_not_guard`, `raw_pointer_write_assignment`, `raw_pointer_write_unaligned`, `raw_pointer_write_unaligned_null_guard`, `raw_pointer_write_unaligned_null_observed_not_guard`, `raw_pointer_write_unaligned_null_other_pointer_not_guard`, `raw_pointer_write_unaligned_null_post_check_not_guard`, `raw_pointer_write_bytes` including narrow `*mut u8` alignment/value evidence, `raw_pointer_write_alignment_guard`, `raw_pointer_write_alignment_observed_not_guard`, `raw_pointer_write_alignment_closed_branch_not_guard`, `raw_pointer_write_alignment_post_check_not_guard`, `raw_pointer_write_null_guard`, `raw_pointer_write_null_observed_not_guard`, `raw_pointer_write_null_other_pointer_not_guard`, `raw_pointer_write_null_post_check_not_guard`, `raw_pointer_write_bounds_observed_not_guard`, `raw_pointer_write_bounds_closed_branch_not_guard`, `raw_pointer_write_bounds_post_check_not_guard`, `raw_pointer_write_other_u8_not_guard`, `raw_pointer_write_maybeuninit`, `raw_pointer_write_other_maybeuninit_not_guard`, `raw_pointer_write_volatile`, `raw_pointer_write_volatile_alignment_guard`, `raw_pointer_write_volatile_alignment_observed_not_guard`, `raw_pointer_write_volatile_alignment_other_pointer_not_guard`, `raw_pointer_write_volatile_alignment_post_check_not_guard`, `raw_pointer_write_volatile_null_guard`, `raw_pointer_write_volatile_null_observed_not_guard`, `raw_pointer_write_volatile_null_other_pointer_not_guard`, `raw_pointer_write_volatile_null_post_check_not_guard`, `split_raw_pointer_read_call`, `split_unsafe_block`, and `safe_reference_deref_no_cards` fixture | source-level review evidence only; unaligned read/write fixtures prove the card omits alignment evidence, not broader pointer validity; volatile memory/device semantics beyond raw-read/write obligations are not modeled |
| Core operation smoke slice | experimental | cards | `maybeuninit_assume_init`, `maybeuninit_assume_init_read`, `maybeuninit_assume_init_ref`, `maybeuninit_assume_init_mut`, `maybeuninit_assume_init_drop`, `vec_set_len`, `vec_set_len_capacity_return_guard`, `vec_set_len_capacity_open_branch_guard`, `vec_set_len_capacity_open_branch_reassigned_len_not_guard`, `vec_set_len_capacity_open_branch_reassigned_receiver_not_guard`, `vec_set_len_initialized_loop`, `vec_set_len_capacity_observed_not_guard`, `vec_set_len_unrelated_capacity_comparison_not_guard`, `vec_set_len_cap_argument_not_guard`, `vec_set_len_with_capacity`, `vec_set_len_call_result_init`, `vec_set_len_shrink`, `vec_set_len_last_index_shrink`, `vec_set_len_start_bound_shrink`, `vec_set_len_zero_clear`, `vec_set_len_post_init_not_guard`, `vec_from_raw_parts`, `vec_from_raw_parts_capacity_guard`, `vec_from_raw_parts_capacity_assert_guard`, `vec_from_raw_parts_capacity_observed_not_guard`, `vec_from_raw_parts_capacity_value_observed_not_guard`, `vec_from_raw_parts_capacity_closed_branch_not_guard`, `vec_from_raw_parts_capacity_reassigned_not_guard`, `vec_from_raw_parts_manuallydrop_origin`, `box_from_raw`, `box_from_raw_box_origin`, `box_from_raw_reassigned_origin_not_guard`, `box_from_raw_box_origin_after_not_guard`, `box_from_raw_other_origin_not_guard`, `copy_nonoverlapping`, `ptr_copy_overlapping`, `ptr_replace_value`, `str_from_utf8_unchecked`, `str_from_utf8_unchecked_is_ok_guard`, `str_from_utf8_unchecked_is_err_return_guard`, `str_from_utf8_unchecked_question_mark_guard`, `str_from_utf8_unchecked_match_return_guard`, `str_from_utf8_unchecked_post_validation_not_guard`, `str_from_utf8_unchecked_other_buffer_not_guard`, `str_from_utf8_unchecked_is_ok_observed_not_guard`, `str_from_utf8_unchecked_guard_then_reassigned_not_guard`, `zeroed_invalid_value`, `zeroed_valid_u32`, `static_mut_global_state`, `inline_asm_human_review`, `transmute_invalid_value`, `transmute_layout_size_guard`, `transmute_bool_valid_value_guard`, `transmute_bool_invalid_return_guard`, `transmute_bool_value_observed_not_guard`, `transmute_bool_closed_if_observed_not_guard`, `transmute_bool_guard_then_reassigned_not_guard`, `transmute_copy_invalid_value`, `transmute_copy_layout_size_guard`, `transmute_copy_bool_valid_value_guard`, `transmute_copy_bool_invalid_return_guard`, `transmute_copy_bool_value_observed_not_guard`, `transmute_copy_bool_closed_if_observed_not_guard`, `transmute_copy_bool_guard_then_reassigned_not_guard`, `multiline_transmute_copy_invalid_value`, `unwrap_unchecked_result`, `unwrap_unchecked_infallible_result`, `unwrap_unchecked_other_infallible_not_guard`, `unwrap_unchecked_is_some_guard`, `unwrap_unchecked_is_ok_guard`, `unwrap_unchecked_if_let_some_guard`, `unwrap_unchecked_if_let_ok_guard`, `unwrap_unchecked_is_some_observed_not_guard`, `unwrap_unchecked_is_ok_observed_not_guard`, `unwrap_unchecked_other_if_let_not_guard`, `unwrap_unchecked_other_if_let_ok_not_guard`, `unwrap_unchecked_post_check_not_guard`, `unwrap_unchecked_guard_then_reassigned_not_guard`, `unwrap_unchecked_is_none_return_guard`, `unwrap_unchecked_is_err_return_guard`, `unreachable_unchecked_path`, `unreachable_unchecked_infallible_path`, `unreachable_unchecked_other_infallible_not_guard`, `unreachable_unchecked_post_infallible_not_guard`, `unreachable_unchecked_closed_infallible_match_not_guard`, `get_unchecked_mut_bounds`, `get_unchecked_mut_len_guard`, `get_unchecked_mut_other_len_not_guard`, `get_unchecked_mut_post_check_not_guard`, `pin_new_unchecked`, `drop_in_place_deallocation`, `drop_in_place_box_origin`, `drop_in_place_reassigned_origin_not_guard`, `drop_in_place_box_origin_after_not_guard`, `drop_in_place_other_origin_not_guard`, `atomic_pointer_state_swap`, `slice_from_raw_parts_mut`, `slice_from_raw_parts_mut_maybeuninit`, `slice_from_raw_parts_mut_other_maybeuninit_not_guard`, `pointer_arithmetic_num_ctrl_bytes_guard`, `pointer_arithmetic_num_ctrl_bytes_open_branch_guard`, `pointer_arithmetic_num_ctrl_bytes_return_guard`, `pointer_arithmetic_num_ctrl_bytes_closed_branch_not_guard`, `pointer_arithmetic_num_ctrl_bytes_invalid_branch_not_guard`, `pointer_arithmetic_num_ctrl_bytes_observed_not_guard`, `pointer_arithmetic_num_ctrl_bytes_other_index_not_guard`, `pointer_arithmetic_num_ctrl_bytes_post_check_not_guard`, `pointer_arithmetic_slice_end`, `target_feature_safety_docs`, `unsafe_fn_call_wrapper`, `multiline_unsafe_fn_call_wrapper`, `unsafe_fn_call_encode_utf8_remaining_cap`, `unchecked_constructor_availability_guard`, `unchecked_constructor_availability_assert_guard`, `unchecked_constructor_unavailable_return_guard`, `unchecked_constructor_other_availability_not_guard`, `unchecked_constructor_availability_observed_not_guard`, `unchecked_constructor_availability_closed_branch_not_guard`, `nonnull_new_guard`, `nonnull_other_guard_not_evidence`, `nonnull_is_null_nonreturning_not_guard`, `nonnull_observed_not_guard`, `nonnull_post_check_not_guard`, and `nested_unsafe_operation_call_dedupe` fixture goldens | curated fixtures, not broad semantic proof; broader `Vec::set_len`, initialization beyond pre-call local evidence for the extended range, `MaybeUninit` proof beyond `assume_init`, `assume_init_read`, `assume_init_ref`, `assume_init_mut`, and `assume_init_drop` smoke fixtures, `Vec::from_raw_parts` allocator/layout evidence beyond narrow same-pointer `ManuallyDrop` raw-parts origin and capacity evidence beyond same-call len/cap assertions, closed-branch rejection, observed-value rejection, and stale-cap rejection, `Box::from_raw` allocator and unique-ownership evidence beyond same-pointer `Box::into_raw` origin evidence, same-pointer matching, different-pointer rejection, reassignment rejection, and post-use origin rejection, copy range evidence beyond `copy_nonoverlapping` and overlapping `ptr::copy` smoke fixtures, `ptr::replace` drop/ownership evidence beyond operation classification, UTF-8 validation evidence beyond same-buffer `from_utf8` `is_ok` enclosing branches, `is_err` early-return, question-mark propagation, match-return evidence, and stale-buffer rejection after reassignment, `get_unchecked` bounds evidence beyond same-receiver pre-access len guards, valid-zero evidence beyond known primitive scalar target types and the `u32` fixture, static mutable state synchronization evidence beyond route selection, inline-asm register/memory/target evidence, transmute layout evidence beyond explicit matching `size_of` equality checks, transmute valid-value evidence beyond narrow executable `u8` to `bool` byte-domain guards, open positive branches containing the call, invalid-byte early returns, and stale-guard rejection after reassignment, `transmute_copy` ownership/drop-specific evidence beyond referenced-byte value guards, open positive branches containing the call, invalid-byte early returns, and stale-guard rejection after reassignment, drop/deallocation beyond same-pointer `Box::into_raw` origin evidence, same-pointer matching, different-pointer rejection, reassignment rejection, and post-use origin rejection, atomic pointer state transitions beyond narrow null swaps, mutable slice range proof, pointer-arithmetic guard naming beyond narrow operation-argument-matched `num_ctrl_bytes` and same-slice end-pointer evidence, target-feature availability proof beyond documented declaration contracts, option/result state proof beyond local infallible-result, same-receiver enclosing `is_some`/`is_ok` branches, early-return `is_none`/`is_err` guards, narrow `if let ... as_ref()` branches, wrong-receiver and post-call state-check rejection, and stale-receiver rejection after reassignment, control-flow proof beyond open local infallible-path evidence, `NonNull` evidence beyond narrow same-argument `NonNull::new?` and same-pointer pre-constructor returning `is_null` guard evidence, `Vec::set_len` capacity evidence beyond shrink, call-result, const-capacity, same-vector `Vec::with_capacity(new_len)`, direct capacity assertions, open positive capacity branches, invalid-length early returns, argument-specific capacity bound guard patterns, unrelated-comparison rejection, and local cap-name rejection, nested operation attribution, and unsafe-call contract modeling beyond narrow `encode_utf8` and same-receiver unchecked-constructor availability evidence remain limited |
| Fixture calibration manifest | experimental | `fixtures/calibration.toml` / `cargo xtask check-calibration` | manifest covers positive, negative, and false-positive-control core fixture claims, including import/declaration and `cfg(target_feature)` false-positive controls, and validates expected card counts, classes, operation families, and hazards against goldens | proof index only; not real-world calibration or support-tier promotion evidence by itself |
| Real-crate dogfood measurement | experimental | dogfood handoff | `2026-05-18-real-crate-dogfood-v0.6.md` records top-50 capped `rust-smallvec`, `arrayvec`, `memchr`, `hashbrown`, `bytes`, `crossbeam`, and `mio` repo runs plus `memchr#215`, `rust-smallvec#407`, `rust-smallvec#277`, `rust-smallvec#64`, `rust-smallvec#254`, `arrayvec#308`, `arrayvec#137`, `arrayvec#138`, `arrayvec#187`, `arrayvec#174`, `arrayvec#288`, `hashbrown#469`, `hashbrown#501`, `hashbrown#556`, `hashbrown#657`, `hashbrown#667`, `hashbrown#692`, `hashbrown#681`, `hashbrown#693`, `bytes#826`, `crossbeam#1226`, `crossbeam#1187`, and `mio#1388` PR-diff runs; it also records capped `crossbeam` concurrency-heavy Send/Sync, atomic-ordering, raw pointer, ownership-transfer, and transmute_copy card measurement plus capped and PR-diff `mio` unsafe function call, `Vec::set_len`, zeroed-value, pointer-operation, socket-address layout, and unsafe Send/Sync route measurement alongside the import/declaration, adjacent unchanged unsafe declaration, `cfg(target_feature)`, capped-scan, syntax-scan performance hardening for large changed files, owner-contract inheritance, comment-aware owner inference, multi-line `impl Trait` owner-inference hardening, long unsafe-function owner inference, macro owner inference, generic unsafe impl owner inference and Sync classification, `Safety:` doc contract evidence, attributed unsafe-fn dedupe, inline unsafe operation dedupe, `drop_in_place` operation modeling, documented public unsafe API declaration handling, documented private unsafe declaration handling, unsafe function pointer field owner identity, unsafe-call wrapper labeling including multi-line wrappers, narrow remaining-capacity argument evidence, and unchecked-constructor availability evidence, parent-call dedupe for nested unsafe operations, `slice::from_raw_parts_mut` operation modeling and `MaybeUninit` slice evidence, `write_bytes` raw pointer write modeling and `MaybeUninit` raw-write destination evidence, `num_ctrl_bytes` and same-slice end-pointer arithmetic bounds evidence, target-feature declaration contract evidence, len/capacity equality raw-read bounds evidence, `unwrap_unchecked` invalid-value operation modeling and local infallible-result evidence, `unreachable_unchecked` unreachable-path operation modeling and local infallible-path evidence, `Vec::from_raw_parts` operation modeling, `&'static mut` false-positive control, arrayvec#137 raw pointer accessor soundness-fix measurement, hashbrown#681 unsafe-call contract and raw-pointer deref measurement, crossbeam#1226 strict-provenance Miri cfg atomic pointer contract measurement, crossbeam#1187 atomic pointer state transition measurement, and the fixture-backed `Vec::set_len` evidence improvements including call-result initialization evidence | eight capped snapshots plus capped follow-up reruns and twenty-three PR diffs across eight crates only; no calibrated rates, no full audit, no uncapped performance claim, narrow `Vec::set_len`, `Vec::from_raw_parts`, contract evidence, owner inference including generic unsafe impl headers, unsafe function pointer field names, long unsafe function bodies, and macro names, Send/Sync trait classification with generic bounds, drop/deallocation, atomic pointer state transitions, mutable slice, non-`u8` raw pointer write byte-pattern validity, pointer arithmetic, target-feature declaration contracts, raw pointer read, raw pointer accessor soundness-fix measurement, `unwrap_unchecked`, `unreachable_unchecked`, unsafe-call, nested operation attribution, unsafe declaration dogfood improvements, crossbeam Send/Sync route measurement, crossbeam#1226 strict-provenance contract measurement, crossbeam#1187 atomic pointer state measurement, mio unsafe-call, zeroed-value, and socket-address layout measurement, transmute_copy classification and multi-line snippet coverage, hashbrown#681 unsafe-call contract measurement, and syntax-scan performance hardening only, and no support-tier promotion |
| Contract evidence mining | experimental | cards | `public_unsafe_fn_missing_safety`, `public_unsafe_trait_missing_safety`, `split_public_unsafe_fn_missing_safety`, `public_unsafe_fn_with_safety_docs`, `public_unsafe_fn_safety_colon_docs`, `public_unsafe_fn_safety_comment_not_docs`, `documented_private_unsafe_fn`, `unsafe_fn_pointer_field_owner`, `private_unsafe_helper_safety_comment`, and `local_safety_colon_comment` fixtures | comment quality is heuristic; unsafe declarations with `# Safety` or doc-comment `Safety:` docs are treated as contract-only sites, unsafe function pointer fields preserve field-name owner identity without inferring a more specific operation family, local `SAFETY:` / `Safety:` comments remain contract evidence and do not by themselves erase guard prompts, and local `SAFETY:` comments do not satisfy public API docs |
| Guard evidence mining | experimental | cards | fixture groups for raw-read/write alignment, raw-read/write nullability, raw-read/write observed-bounds, closed-bounds, post-use-bounds, bare-`align_of`, other-pointer alignment, and `comment_alignment_not_guard` prove bounds/nullability observations, closed positive branches, post-use checks, standalone alignment constants, mismatched pointer checks, and prose do not discharge guard evidence; `nested_unsafe_operation_call_dedupe` proves the `NonNull::new_unchecked` operation name does not itself discharge non-null evidence | obligation-specific patterns are still sparse |
| Witness routing | experimental | cards | route-table tests plus raw pointer, Pin, invalid-value, and drop/deallocation fixture routes; `ffi_sanitizer_route` for sanitizer/cargo-careful FFI routing; `unsafe_impl_send`, `unsafe_impl_send_generic_owner`, and `unsafe_impl_sync_generic_bound` for Loom/Shuttle Send/Sync routing | route recommendation only; receipt import is a separate surface and does not execute witnesses |
| Witness plan output | experimental | `--format witness-plan` | renderer tests, CLI e2e, and first-pr artifact verifier cover route groups, route limitations, suggested commands, receipt hints, and trust-boundary wording | route artifact only; does not execute witnesses or prove witness success |
| Witness receipt template | experimental | `receipt template` | CLI e2e covers valid JSON rendering from explicit user metadata | authoring aid only; does not run witnesses, validate command output, or prove witness success |
| Miri saved-output receipt adapter | experimental | `receipt import-miri` | core parser tests and CLI e2e cover converting a saved success log into a `miri`/`ran` receipt and rejecting failure-looking output | reads saved output only; does not run Miri, infer site reach, parse native UB diagnostics into cards, or prove safety |
| cargo-careful saved-output receipt adapter | experimental | `receipt import-careful` | core parser tests and CLI e2e cover converting a saved success log into a `cargo-careful`/`ran` receipt and rejecting failure-looking output | reads saved output only; does not run `cargo-careful`, infer site reach, parse diagnostics into cards, or prove safety |
| Sanitizer saved-output receipt adapter | experimental | `receipt import-sanitizer --tool asan|msan|tsan|lsan` | core parser tests and CLI e2e cover converting a saved success log into a sanitizer/`ran` receipt, rejecting unsupported sanitizer tools, and rejecting failure-looking output | reads saved output only; does not run sanitizers, infer site reach, parse diagnostics into cards, or prove safety |
| Loom/Shuttle saved-output receipt adapter | experimental | `receipt import-concurrency --tool loom|shuttle` | core parser tests and CLI e2e cover converting a saved success log into a concurrency/`ran` receipt, rejecting unsupported concurrency tools, and rejecting failure-looking output | reads saved output only; does not run Loom/Shuttle, infer site reach, parse interleaving diagnostics into cards, or prove scheduler coverage |
| Kani/Crux saved-output receipt adapter | experimental | `receipt import-proof --tool kani|crux` | core parser tests and CLI e2e cover converting a saved verification-success log into a proof/`ran` receipt, rejecting unsupported proof tools, and rejecting failure-looking output | reads saved output only; does not run Kani/Crux, infer site reach, parse proof diagnostics into cards, or prove beyond the recorded harness/output |
| Witness receipt validation | experimental | `receipt validate` | CLI e2e covers counting importable receipt files through the importer validation path | validation only; does not analyze cards, execute witnesses, or prove witness success |
| Witness receipt audit | experimental | `receipt audit` | core receipt-audit tests cover matched, unmatched, stale, expired, wrong-identity, wrong-tool, weaker-than-required, duplicate, and invalid metadata; CLI e2e covers JSON and Markdown audit output | advisory metadata audit only; does not execute witnesses, infer site reach, make policy decisions, or prove witness success |
| Repo inventory, posture Markdown, and badge JSON | experimental | `repo --format json`, `repo --format markdown`, `badges` | core Markdown renderer tests and CLI e2e cover repo-scope open-gap counts, posture sections, trust boundary, and badge messages for a fixture card | static open-gap counts only; not calibrated, not a safety badge, and not policy gating |
| Outcome comparison | experimental | `outcome --before before.json --after after.json` | core outcome renderer tests and CLI e2e cover comparing saved JSON snapshots, new-card counts, saved ReviewCard site / operation-family / hazard context, witness receipt strength movement, Markdown output, and trust-boundary wording; repo-policy handoff records capped `memchr` saved-snapshot dogfood with 10 improved cards and 40 unchanged cards | compares existing snapshots only; limited dogfood on one capped repo snapshot pair; does not rerun analysis, reclassify cards, execute witnesses, or make policy decisions |
| PR Markdown summary | experimental | PR artifact Markdown | `pr_summary` renderer tests, CLI `--format pr-summary`, CLI e2e, and advisory workflow upload | advisory artifact only; no comments or blocking policy |
| SARIF projection | experimental | PR artifact SARIF | `sarif` renderer tests, CLI `--format sarif`, CLI e2e, and advisory workflow upload | advisory static review evidence; no default blocking |
| Advisory PR workflow | experimental | GitHub Actions artifacts | workflow renders cards JSON, PR summary, SARIF, and comment plan; runs `cargo xtask check-advisory-artifacts target/unsafe-review` before upload; downloaded artifacts are verified for cards JSON trust boundary and projection card identity consistency | no comments, witnesses, or blocking policy |
| Inline comment plan | experimental | PR artifact JSON | `comment_plan` renderer tests, CLI `--format comment-plan`, CLI e2e, and advisory artifact verifier cover plan-only mode, projected card IDs, concrete operation expression, structured route details, verify commands, artifact-only/no-posting comment-body wording, zero-gap `no_changed_gaps` wording, and advisory workflow upload | artifact-only; no posting by default |
| Saved LSP JSON projection | experimental | `--format lsp` JSON | `lsp_projection` renderer tests and CLI e2e cover read-only status data, diagnostics with concrete operation expression, next action, witness route details, verify commands, hovers with concrete operation expression and missing-evidence context, reach-limitation wording, copy-command action data, and related-test open-command data projected from ReviewCards | saved JSON projection only; does not start the live LSP server, edit source, or claim that static related-test mentions executed the unsafe site |
| LSP server/editor integration | planned | editor | saved-card fixtures | read-only first |
| Agent packets | experimental | `context <card-id> --json` | `agent_packet` renderer tests and CLI e2e cover bounded read-only packets projected from ReviewCards | copy-only; no agent execution, source edits, comments, witness execution, or repair success claim |
| Witness receipt import | experimental | `.unsafe-review/receipts/*.json` / `WitnessReceipt` SDK DTO | receipt parser tests cover exact identity, tool, strength, author, timestamp, and expiry validation; `WitnessReceipt` serde round-trip tests cover the public receipt shape; analyzer tests cover exact-card import; `raw_pointer_alignment_receipted` golden covers rendered card output | imports receipts only; does not execute witnesses, does not prove repository safety, and matches exact card identity only |
| MIR/nightly facts | deferred | optional adapter | ADR needed | not v0.1 product default |
