# Explain Examples

`unsafe-review explain <card-id>` turns one `ReviewCard` into a reviewer note.
It should answer what changed, which obligation matters, what evidence exists,
what evidence is missing, what would resolve the card, what would not resolve
it, which witness route fits, and what unsafe-review is not claiming.

These examples are intentionally fixture-backed. Each row names the fixture that
proves the current card shape. The examples are not a support-tier promotion and
not a safety claim.

## How To Read These Examples

Each example has the same review shape:

- **Bad pattern:** the unsafe-adjacent shape that should produce a card.
- **Card summary:** the operation family, review class, and missing evidence.
- **Good fix:** the kind of evidence a reviewer should ask for.
- **Bad fix:** changes that should not discharge the card.
- **Witness route:** the cheapest credible follow-up, if static evidence remains
  incomplete.

`unsafe-review` does not run the witness route. A route is a suggestion for a
reviewer or CI lane, not a recorded receipt.

## Raw Pointer Alignment

Fixture proof:

- `raw_pointer_alignment`
- `raw_pointer_alignment_is_aligned_guard`
- false-positive controls such as `raw_pointer_alignment_post_check_not_guard`
  and `raw_pointer_alignment_reassigned_pointer_not_guard`

Bad pattern:

```rust
unsafe { ptr.cast::<Header>().read() }
```

Card summary:

- Operation family: `raw_pointer_read`
- Class: `guard_missing`
- Missing evidence: local guard evidence for pointer validity, alignment,
  initialization, and allocation obligations

Good fix:

- Add a local pre-use guard proving the pointer is aligned for `Header`, while
  preserving the existing bounds and lifetime evidence.
- Use `read_unaligned` only when unaligned input is intentional and the contract
  says so.

Bad fix:

- Add only a `SAFETY:` comment.
- Check alignment after the read.
- Check a different pointer, or reassign the pointer after the check.

Witness route:

- Miri is the strongest focused route for a pure-Rust pointer-read fixture when
  a targeted test exists.
- `cargo-careful` is a cheaper compatibility-oriented runtime route.

## `copy_nonoverlapping` Range

Fixture proof:

- `copy_nonoverlapping`
- `copy_nonoverlapping_slice_range_guard`
- false-positive controls such as
  `copy_nonoverlapping_slice_range_src_only_not_guard`,
  `copy_nonoverlapping_slice_range_dst_only_not_guard`, and
  `copy_nonoverlapping_slice_range_disjunctive_early_return_block_comment_not_guard`

Bad pattern:

```rust
unsafe { core::ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr(), count) }
```

Card summary:

- Operation family: `copy_nonoverlapping`
- Class: usually `guard_missing` until the same-call source and destination
  ranges are proven
- Missing evidence: valid source range, valid destination range, non-overlap, or
  witness receipt, depending on the fixture

Good fix:

- Add an assertion or early return that proves `count <= src.len()` and
  `count <= dst.len()` for the exact source, destination, and count used by the
  call.
- Keep non-overlap evidence separate from range evidence.

Bad fix:

- Check only `src.len()` or only `dst.len()`.
- Leave the early return only in a comment.
- Reassign `count`, `src`, or `dst` after the guard and before the call.

Witness route:

- Miri or `cargo-careful` is useful for targeted pure-Rust copy fixtures.
- Human review remains necessary for non-overlap reasoning that is not visible
  in local static evidence.

## `ptr::copy` Range

Fixture proof:

- `ptr_copy_overlapping`
- `ptr_copy_slice_range_guard`
- false-positive controls such as `ptr_copy_slice_range_src_only_not_guard`,
  `ptr_copy_slice_range_dst_only_not_guard`, and
  `ptr_copy_slice_range_reassigned_count_not_guard`

Bad pattern:

```rust
unsafe { core::ptr::copy(src.as_ptr(), dst.as_mut_ptr(), count) }
```

Card summary:

- Operation family: `ptr_copy`
- Class: usually `guard_missing` until the source and destination ranges are
  proven
- Missing evidence: source range, destination range, initialized memory, or
  witness receipt, depending on the fixture

Good fix:

- Add same-call range evidence for the exact source, destination, and count.
- Preserve the distinction between overlapping-copy semantics and
  `copy_nonoverlapping` non-overlap obligations.

Bad fix:

- Treat `ptr::copy` as if it had the same non-overlap obligation as
  `copy_nonoverlapping`.
- Accept a source-only or destination-only length check as full range evidence.

Witness route:

- Miri or `cargo-careful` is credible for focused pure-Rust runtime checks.
- Human review is still needed when aliasing or provenance is not locally
  visible.

## `Vec::set_len` Initialized Range

Fixture proof:

- `vec_set_len`
- `vec_set_len_initialized_loop`
- false-positive controls such as `vec_set_len_post_init_not_guard` and
  `vec_set_len_reassigned_receiver_not_guard`

Bad pattern:

```rust
unsafe { values.set_len(new_len) }
```

Card summary:

- Operation family: `vec_set_len`
- Class: `guard_missing`
- Missing evidence: capacity evidence, initialized-range evidence, or witness
  receipt

Good fix:

- Prove capacity for the same vector and `new_len`.
- Prove every newly exposed element is initialized before `set_len`.
- Shrink-only or clear-to-zero cases may satisfy different obligations than
  extend cases.

Bad fix:

- Initialize elements after `set_len`.
- Check the capacity of a different vector.
- Reassign the vector after the guard.

Witness route:

- Miri can be a useful focused witness when the test actually exercises the
  `set_len` path.
- Without a receipt, a related test mention is not proof of execution.

## `str::from_utf8_unchecked` Validation

Fixture proof:

- `str_from_utf8_unchecked`
- `str_from_utf8_unchecked_is_ok_guard`
- `str_from_utf8_unchecked_is_err_return_guard`
- false-positive controls such as
  `str_from_utf8_unchecked_post_validation_not_guard`,
  `str_from_utf8_unchecked_other_buffer_not_guard`, and
  `str_from_utf8_unchecked_guard_then_reassigned_not_guard`

Bad pattern:

```rust
unsafe { core::str::from_utf8_unchecked(bytes) }
```

Card summary:

- Operation family: `str_from_utf8_unchecked`
- Class: `guard_missing`
- Missing evidence: same-buffer UTF-8 validation or witness receipt

Good fix:

- Validate the same buffer before the unchecked conversion.
- Use an early-return, question-mark, or match-return shape that keeps invalid
  bytes out of the unchecked branch.

Bad fix:

- Validate after the unchecked conversion.
- Validate a different buffer.
- Observe `from_utf8(bytes).is_ok()` in a closed branch that does not dominate
  the unsafe call.
- Reassign the buffer after validation.

Witness route:

- Miri can catch some concrete invalid-value executions, but the static review
  still needs same-buffer validation or a receipt.

## `MaybeUninit::assume_init`

Fixture proof:

- `maybeuninit_assume_init`
- `maybeuninit_assume_init_read`
- `maybeuninit_assume_init_ref`
- `maybeuninit_assume_init_mut`
- `maybeuninit_assume_init_drop`

Bad pattern:

```rust
unsafe { value.assume_init() }
```

Card summary:

- Operation family: `maybe_uninit_assume_init`
- Class: `guard_missing`
- Missing evidence: initialized memory evidence or witness receipt

Good fix:

- Show the exact value was fully initialized before `assume_init`.
- Keep initialization evidence before the unsafe call and tied to the same
  object.

Bad fix:

- Rely on variable names or comments that imply initialization.
- Move initialization after `assume_init`.
- Prove initialization for another value.

Witness route:

- Miri is the best focused witness for concrete uninitialized-memory execution,
  but a receipt does not remove the need for static initialization evidence.

## `transmute` Invalid Value

Fixture proof:

- `transmute_invalid_value`
- `transmute_bool_valid_value_guard`
- `transmute_bool_invalid_return_guard`
- false-positive controls such as `transmute_bool_value_observed_not_guard`,
  `transmute_bool_closed_if_observed_not_guard`, and
  `transmute_bool_guard_then_reassigned_not_guard`

Bad pattern:

```rust
unsafe { core::mem::transmute::<u8, bool>(byte) }
```

Card summary:

- Operation family: `transmute`
- Class: `guard_missing`
- Missing evidence: valid-value evidence, layout evidence, or witness receipt,
  depending on the target type

Good fix:

- Prove the exact value is valid for the target type before the transmute.
- Prefer a safe conversion when one exists.

Bad fix:

- Only observe that the value is valid in a closed branch.
- Check the value, then reassign it before the transmute.
- Treat equal layout size as proof of valid values.

Witness route:

- Miri can be a useful focused witness for concrete invalid-value cases.
- Human review is needed for representation and invariants outside local static
  evidence.

## `NonNull::new_unchecked` Nullability

Fixture proof:

- `nonnull_new_guard`
- false-positive controls such as `nonnull_other_guard_not_evidence`,
  `nonnull_is_null_nonreturning_not_guard`, `nonnull_observed_not_guard`, and
  `nonnull_post_check_not_guard`

Bad pattern:

```rust
unsafe { core::ptr::NonNull::new_unchecked(ptr) }
```

Card summary:

- Operation family: `nonnull_unchecked`
- Class: `guard_missing` when the nullability obligation is not discharged
- Missing evidence: same-pointer non-null evidence or witness receipt

Good fix:

- Use `NonNull::new(ptr)` when possible.
- Otherwise, add a same-pointer non-null guard before `new_unchecked`.

Bad fix:

- Check a different pointer.
- Check nullability after `new_unchecked`.
- Treat a non-returning branch or observation that does not dominate the call as
  proof.

Witness route:

- Miri or `cargo-careful` can be useful for targeted concrete paths, but the
  review card still asks for visible same-pointer evidence.

## `unsafe impl Send` / `Sync`

Fixture proof:

- `unsafe_impl_send`
- `unsafe_impl_send_generic_owner`
- `unsafe_impl_sync_generic_bound`

Bad pattern:

```rust
unsafe impl Send for SharedCell {}
```

Card summary:

- Operation family: `unsafe_impl_send` or `unsafe_impl_sync`
- Class: often `requires_loom` or human-review-oriented depending on the card
- Missing evidence: concurrency invariant explanation, related test/witness
  evidence, or receipt

Good fix:

- Document the invariant that makes the type safe to send or share.
- Add a focused concurrency witness when interleavings matter.
- Keep generic bounds explicit and tied to the owner type.

Bad fix:

- Treat a passing unit test as proof of all scheduler interleavings.
- Add a broad `SAFETY:` comment without explaining the concurrency invariant.
- Widen the unsafe impl to more generic parameters than the invariant supports.

Witness route:

- Loom or Shuttle is a better route than Miri for scheduler/interleaving
  questions.
- Human deep review remains appropriate for complex ownership invariants.

## FFI Boundary

Fixture proof:

- `ffi_sanitizer_route`

Bad pattern:

```rust
unsafe extern "C" {
    fn foreign_read(ptr: *const u8, len: usize) -> i32;
}
```

Card summary:

- Operation family: `ffi`
- Class: route-oriented rather than a local pure-Rust guard proof
- Missing evidence: ABI, ownership, lifetime, buffer, or witness evidence,
  depending on the boundary

Good fix:

- Document who owns each pointer, how long it is valid, and which side may
  mutate or free it.
- Route runtime checks to the sanitizer that matches the boundary risk.

Bad fix:

- Treat the Rust-side declaration as proof of the foreign-side contract.
- Claim Miri coverage for code that depends on foreign implementation behavior.
- Hide the boundary behind a broad suppression.

Witness route:

- Sanitizers are often a better first runtime route than Miri for FFI memory
  boundaries.
- Human deep review is still needed for ABI and foreign ownership contracts.

## Safe Code Emits No Card

Fixture proof:

- `safe_code_no_cards`

Expected result:

```text
No changed unsafe-review gaps were found.
This does not prove the repo safe, UB-free, Miri-clean, or that any unsafe site executed.
```

Review meaning:

- No card means the selected static review scope did not emit open
  unsafe-review gaps.
- It is not an all-clear, a safety proof, or evidence that any unsafe site ran.

