# Accuracy validation and calibration

This directory holds the source-of-truth documentation for claim-scoped
accuracy validation and calibration.

The current fixture-pinned slices are:

- `Box::from_raw` ownership evidence: checks whether the `ownership`
  obligation is discharged by same-pointer `Box::into_raw` origin evidence,
  while rejecting bare and reassigned-origin controls.
- `ptr::drop_in_place` Box-origin evidence: checks whether same-pointer
  `Box::into_raw` origin evidence discharges pointer-liveness, initialized, and
  ownership obligations, while rejecting bare and reassigned-origin controls.
- `ptr::drop_in_place` witness routing: checks that drop/deallocation hazards
  route to Miri/cargo-careful witness suggestions without claiming those
  witnesses ran.
- `copy_nonoverlapping` valid-range evidence: checks whether the `valid-range`
  obligation is discharged by same-call source and destination length checks,
  while preserving missing non-overlap evidence and rejecting stale, partial,
  closed-branch, comment-only, and unrelated-length controls.
- `ptr::copy` valid-range evidence: checks whether the `valid-range`
  obligation is discharged by same-call source and destination length checks,
  while preserving missing initialized-memory evidence and rejecting stale,
  partial, closed-branch, comment-only, and unrelated-length controls.
- `raw_pointer_read` alignment evidence: checks whether the `alignment`
  obligation's discharge state is `present` or `missing` in the linked goldens,
  including a comment-only false-positive control.
- Raw pointer write initialized evidence: checks whether the `initialized`
  obligation is discharged by `u8`, guarded `bool`, or `MaybeUninit` targets,
  while rejecting stale, closed-branch, wrong-target, and previous-operation
  controls.
- `slice::from_raw_parts_mut` initialized-memory evidence: checks whether
  `MaybeUninit` element slices discharge the initialized-memory obligation,
  while rejecting unrelated `MaybeUninit` mentions and keeping pointer,
  alignment, and allocation obligations separate.
- Public unsafe API contract evidence: checks whether the public caller-contract
  obligation has `# Safety` / documented `Safety:` contract evidence, while
  keeping local `SAFETY:` comments from satisfying public API documentation.
- `Vec::set_len` initialized-range evidence: checks whether the `initialized`
  obligation is discharged by visible initialization, shrink, zero-clear, or
  call-result patterns, while rejecting capacity-only, post-init, and unrelated
  initialization controls.
- `Vec::from_raw_parts` capacity evidence: checks whether the `capacity`
  obligation is discharged by same-call len/cap guards, assertions, or
  same-origin ManuallyDrop raw parts, while rejecting observed, closed-branch,
  wrong-capacity, and stale-cap controls.
- `MaybeUninit::assume_init` family initialized-memory evidence: checks that
  `assume_init`, `assume_init_read`, `assume_init_ref`, `assume_init_mut`, and
  `assume_init_drop` report missing initialized evidence and route to
  Miri/cargo-careful witness suggestions.
- `transmute::<u8, bool>` / `transmute_copy::<u8, bool>` valid-value evidence:
  checks whether the `valid-value` obligation is discharged by dominating
  bool-domain or invalid-byte early-return guards, while rejecting layout-only,
  observed, closed-branch, and stale-byte controls.
- `str::from_utf8_unchecked` UTF-8 validation evidence: checks whether the
  `utf8` obligation is discharged by same-buffer validation before conversion,
  while rejecting post-validation, wrong-buffer, observed-only, and stale-buffer
  controls.
- Unsafe impl Send/Sync witness routing: checks that thread-safety invariants
  route to Loom/Shuttle witness suggestions.
- Generic unsafe function call callee-contract evidence: checks whether
  remaining-capacity and availability guards discharge generic unsafe callee
  preconditions, while rejecting wrong-receiver, observed-only, and
  closed-branch controls.
- FFI witness routing: checks that unsafe extern C boundaries route away from
  Miri-first review to sanitizer/cargo-careful witness suggestions.
- FFI boundary obligation evidence: checks that ABI/layout compatibility and
  ownership/lifetime/nullability contracts are tracked as separate obligations
  for unsafe extern C seams.
- Inline assembly human-review routing: checks that `asm!` register, memory,
  clobber, and target invariants route to human deep review without implying an
  executable witness ran.
- Static mutable global-state routing: checks that `static mut` synchronization
  and aliasing invariants route to Loom/Shuttle interleaving witnesses without
  implying those witnesses ran.
- Target-feature human-review routing: checks that documented
  `#[target_feature]` caller-contract sites route to human deep review for
  hardware availability and dispatch correctness without implying witness proof.
- `get_unchecked_mut` bounds evidence: checks whether same-receiver len guards
  discharge the bounds obligation, while rejecting other-receiver, post-check,
  observed-only, closed-branch, and stale-index controls.
- Pointer arithmetic bounds evidence: checks whether `index < num_ctrl_bytes`
  and same-slice end-pointer patterns discharge pointer-arithmetic bounds
  evidence while preserving witness/provenance limits.
- No-card artifact honesty: checks that safe, import-only, cfg-only, and
  unchanged-adjacent fixtures emit zero ReviewCards without turning that into an
  all-clear or safety claim.
- `NonNull::new_unchecked` nullability evidence: checks whether the `non-null`
  obligation is discharged by a same-pointer `NonNull::new` guard, while
  rejecting wrong-pointer, observed-only, non-returning `is_null`, and
  post-check controls.
- `Pin::new_unchecked` human-review routing: checks that pinning move-prevention
  and projection invariants route to human deep review without implying an
  executable witness ran.
- `mem::zeroed` valid-zero evidence: checks that invalid zero bit-patterns keep
  valid-zero evidence missing while known primitive valid-zero targets discharge
  the obligation, with Miri/cargo-careful routes still only suggested.

This remains experimental/advisory until human-adjudicated calibration and
report checks are landed.
