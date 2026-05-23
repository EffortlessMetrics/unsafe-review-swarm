# Accuracy validation and calibration

This directory holds the source-of-truth documentation for claim-scoped
accuracy validation and calibration.

The current fixture-pinned slices are:

- `raw_pointer_read` alignment evidence: checks whether the `alignment`
  obligation's discharge state is `present` or `missing` in the linked goldens.
- Public unsafe API contract evidence: checks whether the public caller-contract
  obligation has `# Safety` / documented `Safety:` contract evidence, while
  keeping local `SAFETY:` comments from satisfying public API documentation.

This remains experimental/advisory until human-adjudicated calibration and
report checks are landed.
