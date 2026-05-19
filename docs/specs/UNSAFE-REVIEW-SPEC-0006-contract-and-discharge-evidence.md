# UNSAFE-REVIEW-SPEC-0006: Contract and discharge evidence

Status: accepted
Owner: core/spec
Created: 2026-05-17
Updated: 2026-05-19
Linked proposal: ../proposals/UNSAFE-REVIEW-PROP-0001-product-contract.md
Linked plan: ../../plans/0.1.0/implementation-plan.md

## Contract

`unsafe-review` must classify evidence into explicit lanes and apply operand/receiver-sensitive rules before counting discharge evidence.

## Evidence lanes

- `contract`: docs, `# Safety`, `SAFETY:` comments, unsafe API precondition text.
- `discharge`: local executable guards, matching wrappers, matching constructor/privacy boundaries.
- `reach`: static relation from tests/harness inventory only.
- `witness`: imported receipt evidence only.

## Matching / precedence rules

1. Contract evidence never auto-discharges obligations by itself.
2. Discharge evidence must match target operand/receiver/buffer identity when the rule is identity-sensitive.
3. Later checks do not retroactively discharge earlier unsafe operations.
4. Non-returning error branches do not count as guard discharge.

## Counts as evidence

| Evidence rule | Counts when | Does not count when | Fixture proof |
|---|---|---|---|
| Alignment guard | executable check like `is_aligned`, `align_offset`, modulo/equality check over same pointer before use | bare `align_of` mention; comment text; unrelated pointer; observed alignment value; closed positive alignment branch; checked pointer reassigned before use; post-use check | `raw_pointer_alignment`, `raw_pointer_alignment_is_aligned_guard`, `raw_pointer_alignment_modulo_guard`, `raw_pointer_write_alignment_guard`, `raw_pointer_read_volatile_alignment_guard`, `raw_pointer_write_volatile_alignment_guard`, `align_of_only_not_guard`, `alignment_other_pointer_not_guard`, `raw_pointer_alignment_post_check_not_guard`, `raw_pointer_alignment_observed_not_guard`, `raw_pointer_alignment_closed_branch_not_guard`, `raw_pointer_alignment_reassigned_pointer_not_guard`, `raw_pointer_alignment_modulo_observed_not_guard`, `raw_pointer_alignment_modulo_closed_branch_not_guard`, `raw_pointer_alignment_modulo_reassigned_pointer_not_guard`, `raw_pointer_write_alignment_observed_not_guard`, `raw_pointer_write_alignment_closed_branch_not_guard`, `raw_pointer_write_alignment_post_check_not_guard`, `raw_pointer_read_volatile_alignment_observed_not_guard`, `raw_pointer_read_volatile_alignment_other_pointer_not_guard`, `raw_pointer_read_volatile_alignment_post_check_not_guard`, `raw_pointer_write_volatile_alignment_observed_not_guard`, `raw_pointer_write_volatile_alignment_other_pointer_not_guard`, `raw_pointer_write_volatile_alignment_post_check_not_guard`, `comment_alignment_not_guard` |
| Raw pointer nullability guard | same-receiver `is_null()` branch returns before a method-form raw pointer operation | bare nullability observation; nullability guard for a different pointer; post-use check | `raw_pointer_read_null_guard`, `raw_pointer_read_null_observed_not_guard`, `raw_pointer_read_null_other_pointer_not_guard`, `raw_pointer_read_null_post_check_not_guard`, `raw_pointer_read_unaligned_null_guard`, `raw_pointer_read_unaligned_null_observed_not_guard`, `raw_pointer_read_unaligned_null_other_pointer_not_guard`, `raw_pointer_read_unaligned_null_post_check_not_guard`, `raw_pointer_read_volatile_null_guard`, `raw_pointer_read_volatile_null_observed_not_guard`, `raw_pointer_read_volatile_null_other_pointer_not_guard`, `raw_pointer_read_volatile_null_post_check_not_guard`, `raw_pointer_write_null_guard`, `raw_pointer_write_null_observed_not_guard`, `raw_pointer_write_null_other_pointer_not_guard`, `raw_pointer_write_null_post_check_not_guard`, `raw_pointer_write_unaligned_null_guard`, `raw_pointer_write_unaligned_null_observed_not_guard`, `raw_pointer_write_unaligned_null_other_pointer_not_guard`, `raw_pointer_write_unaligned_null_post_check_not_guard`, `raw_pointer_write_volatile_null_guard`, `raw_pointer_write_volatile_null_observed_not_guard`, `raw_pointer_write_volatile_null_other_pointer_not_guard`, `raw_pointer_write_volatile_null_post_check_not_guard` |
| NonNull guard | `NonNull::new(ptr)?` or equivalent returning non-null check for the same pointer before `new_unchecked(ptr)` | guard applies to different pointer, a bare constructor observation, a non-returning error branch, or a post-constructor check | `nonnull_new_guard`, `nonnull_other_guard_not_evidence`, `nonnull_is_null_nonreturning_not_guard`, `nonnull_observed_not_guard`, `nonnull_post_check_not_guard` |
| Unchecked constructor availability | same-receiver `is_available()` assertion, enclosing positive branch, or unavailable-path early return before `new_unchecked` | different receiver; bare availability observation; closed positive branch; post-constructor check | `unchecked_constructor_availability_guard`, `unchecked_constructor_availability_assert_guard`, `unchecked_constructor_unavailable_return_guard`, `unchecked_constructor_other_availability_not_guard`, `unchecked_constructor_availability_observed_not_guard`, `unchecked_constructor_availability_closed_branch_not_guard` |
| UTF-8 validation | `str::from_utf8(buf).is_ok()` branch encloses the unchecked call, a returning error path, `str::from_utf8(buf)?`, or a same-buffer `match` whose error arm returns before `from_utf8_unchecked(buf)`; evidence is rejected if the buffer is reassigned before the call | no validation, bare validation observation, validation after use, validation for a different buffer, stale validation after reassignment, or a match error arm that does not return | `str_from_utf8_unchecked`, `str_from_utf8_unchecked_is_ok_guard`, `str_from_utf8_unchecked_is_err_return_guard`, `str_from_utf8_unchecked_question_mark_guard`, `str_from_utf8_unchecked_match_return_guard`, `str_from_utf8_unchecked_post_validation_not_guard`, `str_from_utf8_unchecked_other_buffer_not_guard`, `str_from_utf8_unchecked_is_ok_observed_not_guard`, `str_from_utf8_unchecked_guard_then_reassigned_not_guard` |
| unwrap_unchecked state | same receiver has an enclosing positive branch (`is_some` / `is_ok`), returning `None` / `Err` path, or narrow `if let ... as_ref()` branch before the unchecked call; evidence is rejected if the receiver is reassigned before the call | other receiver; bare state observation; check after unchecked call; stale state evidence after reassignment; unrelated infallible expression | `unwrap_unchecked_is_some_guard`, `unwrap_unchecked_is_ok_guard`, `unwrap_unchecked_if_let_some_guard`, `unwrap_unchecked_if_let_ok_guard`, `unwrap_unchecked_is_none_return_guard`, `unwrap_unchecked_is_err_return_guard`, `unwrap_unchecked_is_some_observed_not_guard`, `unwrap_unchecked_is_ok_observed_not_guard`, `unwrap_unchecked_guard_then_reassigned_not_guard`, `unwrap_unchecked_other_infallible_not_guard` |
| unreachable_unchecked path | `unreachable_unchecked` appears inside an error arm for a local match whose head uses `Fallibility::Infallible` | infallible evidence belongs to another match, appears after the unchecked operation, or comes from a match already closed before the unchecked operation | `unreachable_unchecked_infallible_path`, `unreachable_unchecked_other_infallible_not_guard`, `unreachable_unchecked_post_infallible_not_guard`, `unreachable_unchecked_closed_infallible_match_not_guard` |
| Bounds guard | `len/capacity` relation executable and relevant to operation family; invalid-length early returns count only when they bound the unsafe operation argument; raw pointer read len/capacity equality must match the source that produced the pointer; narrow same-slice raw-write evidence such as `slice.as_mut_ptr().write_bytes(_, slice.len())`; narrow pointer-arithmetic evidence must match the operation argument before the unsafe operation, and branch evidence must use the correct in-bounds or invalid-path direction | unrelated length variable; comment-only claim; bare predicate observation; generic type angle brackets; post-access check; closed positive branch that no longer encloses the access; checked index or length/capacity argument reassigned before use; checked receiver reassigned before use; capacity binding stale after receiver or binding reassignment; capacity observation without a guard; stale `Vec::with_capacity(new_len)` evidence after the vector binding or checked length is reassigned; unrelated capacity comparison that does not bound the unsafe operation argument; raw pointer read len/capacity equality for another source, a bare equality observation, or a closed equality branch; local argument merely named `cap` without a const-capacity context; pointer-arithmetic checks over a different index, bare observations, closed branches, invalid operation branches, or post-use checks | `vec_set_len`, `vec_set_len_capacity_return_guard`, `raw_pointer_read_len_capacity_assert`, `raw_pointer_read_len_capacity_other_values_not_guard`, `raw_pointer_read_len_capacity_observed_not_guard`, `raw_pointer_read_len_capacity_closed_branch_not_guard`, `raw_pointer_bounds_observed_not_guard`, `raw_pointer_bounds_closed_branch_not_guard`, `raw_pointer_bounds_post_check_not_guard`, `raw_pointer_write_bounds_observed_not_guard`, `raw_pointer_write_bounds_closed_branch_not_guard`, `raw_pointer_write_bounds_post_check_not_guard`, `raw_pointer_write_maybeuninit`, `raw_pointer_write_other_maybeuninit_not_guard`, `get_unchecked_mut_len_guard`, `get_unchecked_mut_other_len_not_guard`, `get_unchecked_mut_post_check_not_guard`, `get_unchecked_mut_bounds_observed_not_guard`, `get_unchecked_mut_closed_bounds_not_guard`, `get_unchecked_mut_reassigned_index_not_guard`, `vec_set_len_capacity_observed_not_guard`, `vec_set_len_capacity_closed_branch_not_guard`, `vec_set_len_capacity_reassigned_not_guard`, `vec_set_len_capacity_receiver_reassigned_not_guard`, `vec_set_len_capacity_binding_receiver_reassigned_not_guard`, `vec_set_len_unrelated_capacity_comparison_not_guard`, `vec_set_len_cap_argument_not_guard`, `vec_set_len_with_capacity_reassigned_not_guard`, `vec_from_raw_parts_capacity_guard`, `vec_from_raw_parts_capacity_assert_guard`, `vec_from_raw_parts_capacity_value_observed_not_guard`, `vec_from_raw_parts_capacity_closed_branch_not_guard`, `vec_from_raw_parts_capacity_reassigned_not_guard`, `pointer_arithmetic_num_ctrl_bytes_guard`, `pointer_arithmetic_num_ctrl_bytes_open_branch_guard`, `pointer_arithmetic_num_ctrl_bytes_return_guard`, `pointer_arithmetic_num_ctrl_bytes_closed_branch_not_guard`, `pointer_arithmetic_num_ctrl_bytes_invalid_branch_not_guard`, `pointer_arithmetic_num_ctrl_bytes_observed_not_guard`, `pointer_arithmetic_num_ctrl_bytes_other_index_not_guard`, `pointer_arithmetic_num_ctrl_bytes_post_check_not_guard` |
| Initialization evidence | writes, constructor results, or `MaybeUninit::new` evidence that initializes the extended range before `Vec::set_len` | mentions or writes that appear only after `set_len` | `vec_set_len_initialized_loop`, `vec_set_len_call_result_init`, `vec_set_len_post_init_not_guard` |
| Box raw ownership evidence | same-pointer `Box::into_raw` origin before `Box::from_raw` or `ptr::drop_in_place` | origin is for another pointer or the raw pointer is reassigned before use | `box_from_raw_box_origin`, `box_from_raw_reassigned_origin_not_guard`, `drop_in_place_box_origin`, `drop_in_place_reassigned_origin_not_guard` |
| Transmute valid-value guard | executable assertion, open positive branch containing the call, or returning invalid-byte branch proving a narrow `u8` to `bool` byte domain before transmute or transmute_copy; evidence is rejected if the checked value is reassigned before the call | bare predicate observation, closed positive branch before the call, stale guard followed by reassignment, different argument, post-call check, or unsupported type pair | `transmute_bool_valid_value_guard`, `transmute_bool_invalid_return_guard`, `transmute_bool_value_observed_not_guard`, `transmute_bool_closed_if_observed_not_guard`, `transmute_bool_guard_then_reassigned_not_guard`, `transmute_copy_bool_value_observed_not_guard`, `transmute_copy_bool_closed_if_observed_not_guard`, `transmute_copy_bool_guard_then_reassigned_not_guard` |

## Does not count

- Comments as discharge evidence.
- Policy receipt metadata as contract/discharge substitution (receipts only populate witness lane).
- Family-incompatible rules (e.g., requiring alignment for `write_unaligned`).

## Fixtures

Every evidence rule in this spec must name at least one positive and one negative fixture (or explicit limitation).

## Output examples

```json
{
  "obligation_evidence": [
    {
      "key": "alignment",
      "description": "pointer is aligned for the accessed type",
      "contract": {"present": true, "state": "present", "summary": "SAFETY comment explains alignment contract"},
      "discharge": {"present": false, "state": "missing", "summary": "No alignment guard code was detected"},
      "reach": {"present": false, "state": "missing", "summary": "No static test relation found"},
      "witness": {"present": false, "state": "missing", "summary": "No imported witness receipt was found"}
    }
  ],
  "missing": ["alignment evidence is missing"],
  "verify_commands": ["cargo +nightly miri test"]
}
```

## CI proof

```bash
cargo xtask check-pr
cargo test --workspace
```

## Promotion rule

Rule changes are promotable only when fixture/golden coverage includes at least one "does not count" case proving false-positive control.
