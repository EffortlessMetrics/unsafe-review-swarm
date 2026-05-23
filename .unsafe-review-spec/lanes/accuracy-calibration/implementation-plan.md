# Accuracy validation and calibration implementation plan

## Scope

Build source-of-truth rails for claim-scoped calibration without promoting any
new calibrated public claims.

## Milestones

1. Spec and lane scaffolding - landed
2. Accuracy policy ledger - landed
3. Labeling protocol and schema - landed
4. Checked calibration report - landed
5. Claim-promotion guardrails - landed

## Claim-promotion guardrails

`cargo run --locked -p xtask -- check-calibration` validates claim entries in
`policy/accuracy-calibration.toml` before they can support public wording.

The guard rejects:

- unknown claim statuses or kinds,
- duplicate claim IDs,
- missing evidence fields,
- support-tier names that do not match `docs/status/SUPPORT_TIERS.md`
  capabilities,
- label samples outside the claim's fixture list, or claim fixtures without
  label samples,
- duplicate fixture/obligation/evidence samples within a claim,
- fixture-pinned claims that carry dogfood targets or labeled reports,
- dogfood-measured claims without known dogfood target IDs,
- labeled-calibrated or policy-eligible claims without checked report files,
- allowed public claim wording that contains global precision/recall,
  policy-ready, memory-safety proof, UB-free, Miri-clean, or witness-execution
  proof language.

## Proof commands

- cargo run --locked -p xtask -- check-calibration
- cargo run --locked -p xtask -- check-dogfood
- cargo run --locked -p xtask -- check-doc-artifacts
- cargo run --locked -p xtask -- check-goals
- cargo run --locked -p xtask -- check-pr
- cargo run --locked -p xtask -- source-divergence
- git diff --check
