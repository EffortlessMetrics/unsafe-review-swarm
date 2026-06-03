# ReviewCard Fix Recipes

These recipes help a reviewer choose a repair shape after `unsafe-review`
emits a `ReviewCard`.

`unsafe-review` does not prove UB. It finds unsafe seams where UB is worth
investigating and tells you what evidence would make the seam reviewable.

Use a recipe after opening the PR summary, `unsafe-review explain <card-id>`,
or `unsafe-review context <card-id> --json`. Match the card's operation family,
pick one narrow repair or one witness route, rerun `unsafe-review`, and compare
the before/after cards. A suggested witness route is not evidence until the
external witness is run outside `unsafe-review` and a current receipt is
recorded.

When this page says "same", it means the same receiver, pointer, buffer, index,
length, owner, boundary, or callee named by the `ReviewCard`, with no
reassignment or shadowing that makes the evidence stale.

Agent rule: these recipes do not make every card agent-ready. An agent may work
only packets whose `agent_readiness.state` is `ready_for_agent`. Other states
are reviewer context.

## `get_unchecked` / `get_unchecked_mut`

What `unsafe-review` is looking for:

- The same slice or receiver as the unsafe operation.
- The same index as the unsafe operation.
- A bounds guard or `get(index)` probe that dominates the unsafe operation.
- Evidence that remains fresh after receiver, receiver-path, or index changes.
- No post-check, comment-only check, observed-only predicate, or guard on
  another receiver.

Good repairs:

- Move or add an executable bounds check immediately before the unsafe use.
- Return, error, or assert before the unsafe operation when `index >= len`.
- Replace the unsafe operation with `get` or `get_mut` when the caller can
  handle `None`.
- Keep the checked receiver and index bindings fresh, or move the unsafe use
  close enough that reassignment and shadowing cannot intervene.

Bad repairs:

- A guard on another slice, copied receiver, or unrelated `len`.
- A guard before index or receiver reassignment.
- A guard in a branch that does not dominate the unsafe operation.
- A post-check after the unsafe operation.
- A `SAFETY:` comment that does not add executable bounds evidence.

Witness route:

- Add a focused unit or property test for the safe caller path and boundary
  indexes.
- Use Miri when the question also involves aliasing, provenance, or
  `get_unchecked_mut` interaction with mutable references.

What this does not prove:

- Pointer provenance, aliasing discipline, caller contract soundness, or that
  all call paths satisfy the invariant.
- UB-free, Miri-clean, site-executed, policy-ready, or calibrated accuracy
  status.

Fixture anchors:

- `docs/accuracy/labels/get-unchecked-mut-bounds.toml`
- `docs/handoffs/2026-06-03-get-unchecked-applicability-closeout.md`
- Examples include `get_unchecked_mut_get_probe_guard`,
  `get_unchecked_mut_get_probe_reassigned_index_not_guard`, and
  `get_unchecked_mut_other_len_not_guard`.

## `MaybeUninit::assume_init*`

What `unsafe-review` is looking for:

- The same `MaybeUninit` slot as the `assume_init`, `assume_init_read`,
  `assume_init_ref`, `assume_init_mut`, or `assume_init_drop` operation.
- Full initialized-memory evidence for the value being exposed, read, mutated,
  or dropped.
- A same-slot `write` or `MaybeUninit::new` path that dominates the unsafe
  operation.
- No stale write, stale `new`, shadowed slot, other slot, partial field, or
  partial array evidence.

Good repairs:

- Initialize the exact slot before the `assume_init*` operation.
- Use `MaybeUninit::new(value)` when the value can be constructed normally.
- For aggregate values, initialize every field or element before the unsafe
  operation.
- Replace the pattern with safe construction when the unsafe form is no longer
  needed.

Bad repairs:

- Writing to a different slot with a similar name.
- Initializing only one field or one array element before assuming the whole
  value is initialized.
- Reassigning or shadowing the slot after the initializing write.
- Treating allocation, capacity, or a comment as initialized-memory evidence.

Witness route:

- Add focused tests for the safe caller paths that decide how many fields or
  elements are initialized.
- Use Miri for uninitialized reads, drops, and invalid reference exposure.

What this does not prove:

- Drop invariants, aliasing, lifetimes, or valid bit patterns beyond the
  initialized-memory obligation visible to the card.
- That every panic path leaves the value in a valid state unless the repair and
  witness target those paths.

Fixture anchors:

- `docs/accuracy/labels/maybeuninit-assume-init-initialized.toml`
- Examples include `maybeuninit_assume_init_write_guard`,
  `maybeuninit_assume_init_new_guard`,
  `maybeuninit_assume_init_other_slot_write_not_guard`,
  `maybeuninit_assume_init_stale_write_not_guard`,
  `maybeuninit_assume_init_partial_field_not_guard`, and
  `maybeuninit_assume_init_partial_array_not_guard`.

## `Vec::set_len`

What `unsafe-review` is looking for:

- The same vector as the `set_len` operation.
- A new length that stays within capacity.
- Initialized elements for the range that becomes visible.
- Fresh receiver, count, capacity, and initialization evidence immediately
  before the unsafe operation.

Good repairs:

- Prefer safe APIs such as `push`, `extend`, or `extend_from_slice` when they
  preserve behavior.
- Reserve or check capacity for the same vector before increasing length.
- Write every element in the newly exposed range before calling `set_len`.
- Move `set_len` after the last initializing write and keep the count binding
  fresh.

Bad repairs:

- Checking capacity on a different vector or a stale receiver.
- Calling `set_len` before initialization finishes.
- Treating one initialized index as proof of a whole range.
- Reusing a stale slice binding, stale count, or stale receiver path.
- Comment-only capacity or initialization claims.

Witness route:

- Add tests for zero, exact-capacity, and boundary-count cases.
- Use Miri for uninitialized element exposure, drop behavior, and panic-path
  questions.

What this does not prove:

- Panic safety, destructor behavior, allocator validity, or aliasing beyond the
  checked capacity and initialized-range evidence.
- That a capacity guard on one vector says anything about another vector.

Fixture anchors:

- `docs/accuracy/labels/vec-set-len-initialized-range.toml`
- Examples include `vec_set_len_initialized_loop`,
  `vec_set_len_reserve_capacity`, `vec_set_len_cap_argument_not_guard`,
  `vec_set_len_reassigned_receiver_not_guard`,
  `vec_set_len_slice_binding_initialized_loop`, and
  `vec_set_len_single_index_init_not_guard`.

## `str::from_utf8_unchecked`

What `unsafe-review` is looking for:

- The same byte buffer as the unchecked conversion.
- UTF-8 validation that dominates the unsafe operation.
- Evidence that the buffer has not been reassigned or mutated after validation.
- No validation on another buffer, post-validation check, or comment-only
  assertion.

Good repairs:

- Replace with `str::from_utf8` and handle the `Result` when possible.
- Validate the same buffer immediately before the unsafe conversion.
- Return or error on invalid UTF-8 before reaching the unsafe operation.
- Bind the validated bytes in a way that cannot be mutated before conversion.

Bad repairs:

- Checking another buffer or a previous version of the buffer.
- Calling `is_ok` without making the invalid path unreachable.
- Validating after `from_utf8_unchecked`.
- Adding a comment that says the bytes are UTF-8 without an executable check or
  contract.

Witness route:

- Add focused invalid-UTF-8 regression tests or property tests for the safe
  caller path.
- Use fuzzing when the byte source is parser- or input-heavy.
- Use Miri only for adjacent pointer, aliasing, or lifetime questions; Miri is
  not a substitute for UTF-8 input coverage.

What this does not prove:

- That every future byte source remains UTF-8.
- That the string's semantic content is valid for higher-level parser or
  protocol invariants.

Fixture anchors:

- `docs/accuracy/labels/str-from-utf8-unchecked-validation.toml`
- Examples include `str_from_utf8_unchecked_is_ok_guard`,
  `str_from_utf8_unchecked_is_err_return_guard`,
  `str_from_utf8_unchecked_post_validation_not_guard`,
  `str_from_utf8_unchecked_other_buffer_not_guard`, and
  `str_from_utf8_unchecked_guard_then_reassigned_not_guard`.

## `copy_nonoverlapping` / `ptr::copy`

What `unsafe-review` is looking for:

- The same source pointer, destination pointer, and count.
- Source and destination ranges valid for the copied element or byte count.
- Fresh evidence after source, destination, count, or path reassignment.
- For `copy_nonoverlapping`, evidence that the ranges do not overlap.
- For `ptr::copy`, valid ranges even though overlap is allowed.

Good repairs:

- Prefer safe slice APIs such as `copy_from_slice` or `copy_within` when they
  match the intended overlap behavior.
- Check both source and destination lengths before the unsafe copy.
- Tie count calculation to the same source and destination used by the copy.
- Keep the unsafe operation immediately after the range and overlap checks.

Bad repairs:

- Checking only the source or only the destination.
- Using an unrelated `len` or a stale count.
- Checking after the copy.
- Using `copy_nonoverlapping` when overlap is possible.
- Treating a comment about buffer size as valid-range evidence.

Witness route:

- Add tests for zero count, exact boundary count, too-large count, and overlap
  behavior.
- Use Miri when pointer validity, provenance, or aliasing is part of the review.
- Use sanitizers when the copy crosses FFI or integration boundaries.

What this does not prove:

- Allocation liveness, provenance, data-race freedom, or foreign memory
  validity.
- That a valid range check for one pointer applies to a different pointer.

Fixture anchors:

- `docs/accuracy/labels/copy-nonoverlapping-valid-range.toml`
- `docs/accuracy/labels/ptr-copy-valid-range.toml`
- Examples include `copy_nonoverlapping_slice_range_guard`,
  `copy_nonoverlapping_slice_range_src_only_not_guard`,
  `copy_nonoverlapping_slice_range_dst_only_not_guard`,
  `copy_nonoverlapping_slice_range_reassigned_count_not_guard`,
  `ptr_copy_slice_range_guard`, and
  `ptr_copy_slice_range_reassigned_count_not_guard`.

## `NonNull::new_unchecked`

What `unsafe-review` is looking for:

- The same pointer passed to `NonNull::new_unchecked`.
- A non-null guard that dominates the unchecked constructor.
- Fresh pointer evidence after casts, reassignment, or shadowing.
- No post-check, observed-only `is_null` predicate, or guard on another pointer.

Good repairs:

- Use `NonNull::new(ptr)` and handle `None` when possible.
- Return, error, or assert before the unchecked constructor when the pointer is
  null.
- Keep the checked pointer binding stable until the unchecked constructor.

Bad repairs:

- Checking a different pointer or an old version of the pointer.
- Observing nullness without making the null path unreachable.
- Checking after `new_unchecked`.
- Claiming dereferenceability from non-nullness alone.

Witness route:

- Add tests for the safe caller path that can pass null.
- Use Miri for pointer validity, alignment, provenance, and later dereference
  questions.

What this does not prove:

- Dereferenceability, alignment, allocation liveness, ownership, aliasing, or
  lifetime validity.
- That non-nullness is sufficient for a later unsafe dereference.

Fixture anchors:

- `docs/accuracy/labels/nonnull-new-unchecked-nullability.toml`
- Examples include `nonnull_new_guard`,
  `nonnull_other_guard_not_evidence`,
  `nonnull_is_null_nonreturning_not_guard`, `nonnull_observed_not_guard`, and
  `nonnull_post_check_not_guard`.

## Raw Pointer Read / Write

What `unsafe-review` is looking for:

- The specific raw pointer operation: deref, read, read_unaligned, read_volatile,
  write, write_unaligned, write_volatile, or write_bytes.
- The hazard-specific evidence for that operation: nullability, alignment,
  bounds, initialized memory, valid value, or allocation-origin evidence.
- The same pointer origin, same length or capacity source, and fresh target
  after casts, reassignment, or shadowing.
- No transfer of evidence from an unrelated pointer, previous operation, or
  different write target.

Good repairs:

- Prefer safe references or slice APIs when they preserve behavior.
- Establish nullability, alignment, bounds, and initialized-memory evidence for
  the exact pointer before reading.
- Establish bounds and valid-value evidence for the exact target before writing.
- Use `MaybeUninit` or typed safe APIs when the operation is really about
  initialization.

Bad repairs:

- Checking a stale origin, typed shadow, unrelated length, or previous pointer.
- Adding an alignment check for a different pointer.
- Treating `read_unaligned` or `write_unaligned` as proof of pointer validity.
- Treating volatile access as proof of memory/device semantics.
- Reusing a previous write to another pointer or type as current evidence.

Witness route:

- Use Miri for Rust-side pointer validity, provenance, alignment, initialized
  memory, and invalid-value questions.
- Use sanitizers for integration and FFI-heavy memory routes.
- Add focused tests for boundary indexes, null paths, and value-domain cases.

What this does not prove:

- Provenance, lifetime, allocation liveness, thread-safety, volatile/device
  semantics, or foreign memory validity unless a separate witness targets them.

Fixture anchors:

- `docs/accuracy/labels/raw-pointer-read-alignment.toml`
- `docs/accuracy/labels/raw-pointer-read-bounds.toml`
- `docs/accuracy/labels/raw-pointer-write-initialized-evidence.toml`
- `docs/accuracy/labels/raw-pointer-operation-family-smoke.toml`
- Examples include `raw_pointer_alignment_is_aligned_guard`,
  `raw_pointer_alignment_post_check_not_guard`,
  `raw_pointer_alignment_reassigned_pointer_not_guard`,
  `raw_pointer_read_len_capacity_assert`,
  `raw_pointer_read_other_len_not_guard`,
  `raw_pointer_read_reassigned_origin_not_guard`,
  `raw_pointer_write_bool_bytes_guard`,
  `raw_pointer_write_bool_reassigned_byte_not_guard`, and
  `raw_pointer_write_previous_slice_not_guard`.

## `transmute` / `transmute_copy`

What `unsafe-review` is looking for:

- Destination value validity for the exact source value.
- Layout and size evidence where the card names a layout obligation.
- Fresh value evidence after reassignment or shadowing.
- For `transmute_copy`, evidence that the copied source is valid to read for
  the destination layout and size.

Good repairs:

- Replace with a safe conversion or explicit `match` when possible.
- Add an executable value-domain check immediately before the unsafe operation.
- Keep layout checks tied to the concrete source and destination types.
- Prefer type-specific constructors that validate invariants.

Bad repairs:

- Treating same-size evidence as valid-value evidence.
- Observing a value without making invalid values unreachable.
- Checking before the value is reassigned or shadowed.
- Adding a comment that names the representation without an executable guard or
  durable type contract.

Witness route:

- Add focused tests or property tests for invalid values and boundary domains.
- Use Miri for invalid bit patterns, invalid references, and
  `transmute_copy` reads.

What this does not prove:

- Future layout stability, repr guarantees, aliasing, lifetimes, or that all
  destination values are semantically valid.
- That a layout check alone satisfies value-level invariants.

Fixture anchors:

- `docs/accuracy/labels/transmute-bool-valid-value.toml`
- Examples include `transmute_bool_valid_value_guard`,
  `transmute_bool_invalid_return_guard`,
  `transmute_bool_value_observed_not_guard`,
  `transmute_bool_closed_if_observed_not_guard`,
  `transmute_bool_guard_then_reassigned_not_guard`,
  `transmute_layout_size_guard`, and
  `transmute_copy_bool_valid_value_guard`.

## FFI / Unsafe Function Calls

What `unsafe-review` is looking for:

- For `unsafe_fn_call`, the callee contract and whether the call arguments
  satisfy that contract locally.
- For FFI, the ABI, ownership, lifetime, buffer, mutation, free, and threading
  responsibilities at the boundary.
- Evidence tied to the exact callee and arguments, not a similarly named module
  or wrapper.
- A human-review or sanitizer route when the invariant depends on foreign code
  or an opaque callee contract.

Good repairs:

- Add a small safe wrapper that checks the callee preconditions before the
  unsafe call.
- Document the exact `# Safety` contract on public unsafe APIs or boundary
  wrappers.
- Check nullability, lengths, ownership, and lifetime assumptions at the Rust
  boundary when those are the missing obligations.
- Route FFI memory questions to sanitizers or `cargo-careful` after external
  execution.

Bad repairs:

- Treating an `extern` declaration as proof of the foreign implementation's
  contract.
- Treating a local module named like `libc` as a known foreign route.
- Adding broad `SAFETY:` text that does not mention the callee arguments or
  boundary responsibilities.
- Claiming Miri coverage for foreign implementation behavior.
- Asking an agent to rewrite an FFI boundary automatically.

Witness route:

- Use human deep review for ABI and callee-contract questions.
- Use sanitizers, `cargo-careful`, or integration tests for FFI memory behavior
  after running them outside `unsafe-review`.
- Use Miri only for Rust-side behavior that does not depend on foreign
  implementation execution.

What this does not prove:

- Foreign implementation correctness, ABI compatibility across platforms,
  ownership discipline on the other side of the boundary, or execution of the
  foreign code.
- That a callee contract is true merely because it is documented.

Fixture anchors:

- `docs/accuracy/labels/unsafe-fn-call-callee-contract.toml`
- `docs/accuracy/labels/ffi-boundary-obligations.toml`
- `docs/accuracy/labels/ffi-sanitizer-witness-routes.toml`
- Examples include `unsafe_fn_call_wrapper`,
  `unsafe_fn_call_encode_utf8_remaining_cap`,
  `ffi_non_libc_wrapper_call_not_route`,
  `ffi_local_libc_module_call_not_route`, `ffi_sanitizer_route`,
  `ffi_call_sanitizer_route`, and `ffi_libc_call_sanitizer_route`.

## `target_feature` / Inline Assembly

What `unsafe-review` is looking for:

- The target-feature, architecture, register, memory, and caller-contract
  assumptions named by the operation.
- A route to human review when the invariant cannot be checked by local syntax.
- Documentation that names the actual preconditions, without treating
  documentation as runtime proof.

Good repairs:

- Gate calls with runtime feature detection where the platform supports it.
- Add or tighten `# Safety` docs for `#[target_feature]` functions and unsafe
  wrappers.
- Keep inline assembly isolated and document register, memory, clobber, and
  architecture assumptions.
- Add platform-specific tests only as witness support, not as proof.

Bad repairs:

- Treating `cfg(target_feature)` predicate text as an unsafe operation.
- Treating documentation as proof that the CPU, OS, or caller state satisfies
  the precondition.
- Letting an agent rewrite inline assembly constraints speculatively.
- Claiming one machine's test run covers all supported targets.

Witness route:

- Use human deep review for assembly constraints and target-feature contracts.
- Use platform-specific CI or hardware witness receipts for the exact target
  when appropriate.
- Use Miri only for adjacent Rust code; it is not an inline-assembly execution
  witness.

What this does not prove:

- Hardware behavior, OS behavior, all CPU feature combinations, register
  constraints, memory clobber correctness, or cross-target support.

Fixture anchors:

- `docs/accuracy/labels/target-feature-human-review-routes.toml`
- `docs/accuracy/labels/inline-asm-human-review-routes.toml`
- Examples include `target_feature_safety_docs`,
  `target_feature_missing_safety_docs`, `inline_asm_human_review`, and
  `cfg_target_feature_not_operation`.
