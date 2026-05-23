# Accuracy validation and calibration

This directory holds the source-of-truth documentation for claim-scoped
accuracy validation and calibration.

The initial slice is `raw_pointer_read` alignment evidence. Its first ledger is
fixture-pinned and obligation-level: it checks whether the `alignment`
obligation's discharge state is `present` or `missing` in the linked goldens.

This remains experimental/advisory until human-adjudicated calibration and
report checks are landed.
