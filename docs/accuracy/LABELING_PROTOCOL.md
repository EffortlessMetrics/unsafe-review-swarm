# Accuracy labeling protocol

Status: experimental

This protocol governs adjudicated sample labels for scoped calibration claims.

## Rules

- Every sample must identify claim, operation family, hazard scope, and expected card behavior.
- Obligation-level samples pin `contract` and `discharge` evidence states only
  when those states come from ReviewCard obligation evidence.
- Route-quality samples may pin `expected_witness_route_kinds` only when those
  route kinds come from ReviewCard `witness_routes`.
- Adjudicated samples require at least two labelers and one adjudicator.
- Holdout samples cannot be used for heuristic tuning or fixture authoring until after the next published calibration report.
- Labels must not claim memory safety proof, UB-free status, Miri-clean status, or global precision/recall.
