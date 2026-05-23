# Accuracy validation and calibration

This directory holds the source-of-truth documentation for claim-scoped
accuracy validation and calibration.

The current fixture-pinned slices are:

- `raw_pointer_read` alignment evidence: checks whether the `alignment`
  obligation's discharge state is `present` or `missing` in the linked goldens,
  including a comment-only false-positive control.
- Public unsafe API contract evidence: checks whether the public caller-contract
  obligation has `# Safety` / documented `Safety:` contract evidence, while
  keeping local `SAFETY:` comments from satisfying public API documentation.
- `Vec::set_len` initialized-range evidence: checks whether the `initialized`
  obligation is discharged by visible initialization, shrink, zero-clear, or
  call-result patterns, while rejecting capacity-only, post-init, and unrelated
  initialization controls.
- `transmute::<u8, bool>` / `transmute_copy::<u8, bool>` valid-value evidence:
  checks whether the `valid-value` obligation is discharged by dominating
  bool-domain or invalid-byte early-return guards, while rejecting layout-only,
  observed, closed-branch, and stale-byte controls.
- Unsafe impl Send/Sync witness routing: checks that thread-safety invariants
  route to Loom/Shuttle witness suggestions.
- FFI witness routing: checks that unsafe extern C boundaries route away from
  Miri-first review to sanitizer/cargo-careful witness suggestions.
- No-card artifact honesty: checks that safe, import-only, cfg-only, and
  unchanged-adjacent fixtures emit zero ReviewCards without turning that into an
  all-clear or safety claim.

This remains experimental/advisory until human-adjudicated calibration and
report checks are landed.
