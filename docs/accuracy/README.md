# Accuracy validation and calibration

This directory holds the source-of-truth documentation for claim-scoped
accuracy validation and calibration.

The current fixture-pinned slices are:

- `Box::from_raw` ownership evidence: checks whether the `ownership`
  obligation is discharged by same-pointer `Box::into_raw` origin evidence,
  while rejecting bare and reassigned-origin controls.
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
- FFI witness routing: checks that unsafe extern C boundaries route away from
  Miri-first review to sanitizer/cargo-careful witness suggestions.
- FFI boundary obligation evidence: checks that ABI/layout compatibility and
  ownership/lifetime/nullability contracts are tracked as separate obligations
  for unsafe extern C seams.
- No-card artifact honesty: checks that safe, import-only, cfg-only, and
  unchanged-adjacent fixtures emit zero ReviewCards without turning that into an
  all-clear or safety claim.
- `NonNull::new_unchecked` nullability evidence: checks whether the `non-null`
  obligation is discharged by a same-pointer `NonNull::new` guard, while
  rejecting wrong-pointer, observed-only, non-returning `is_null`, and
  post-check controls.

This remains experimental/advisory until human-adjudicated calibration and
report checks are landed.
