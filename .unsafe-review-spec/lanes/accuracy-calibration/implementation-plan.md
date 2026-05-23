# Accuracy validation and calibration implementation plan

## Scope

Build source-of-truth rails for claim-scoped calibration without promoting any
new calibrated public claims.

## Milestones

1. Spec and lane scaffolding
2. Accuracy policy ledger
3. Labeling protocol and schema
4. Checked calibration report
5. Claim-promotion guardrails

## Proof commands

- cargo run --locked -p xtask -- check-calibration
- cargo run --locked -p xtask -- check-dogfood
- cargo run --locked -p xtask -- check-doc-artifacts
- cargo run --locked -p xtask -- check-goals
- cargo run --locked -p xtask -- check-pr
- cargo run --locked -p xtask -- source-divergence
- git diff --check
