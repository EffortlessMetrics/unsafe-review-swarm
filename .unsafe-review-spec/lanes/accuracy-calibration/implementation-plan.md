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
- allowed public claim wording that omits the claim level,
- forbidden claim lists that omit shared global precision, global recall, and
  memory-safety proof overclaim boundaries,
- fixture-pinned claims that carry dogfood targets or labeled reports,
- dogfood-measured claims without known dogfood target IDs,
- labeled-calibrated or policy-eligible claims without checked report files,
- allowed public claim wording that contains global precision/recall,
  policy-ready, memory-safety proof, UB-free, Miri-clean, or witness-execution
  proof language.
- fixture golden cards whose obligations and obligation_evidence are not
  one-to-one, description-aligned, and coherent across contract/discharge/reach/
  witness present-state fields.
- fixture golden cards whose obligation_evidence keys fall outside the operation
  family registry row.
- fixture golden cards whose top-level missing summaries drift away from the
  per-obligation evidence state.
- fixture golden cards whose top-level reach, per-obligation reach summaries,
  or reach owner drift away from static test-mention evidence or claim site
  execution without a receipt.
- fixture golden cards whose next_action is missing, non-actionable,
  overclaiming, or names a different operation family for safety-obligation
  repair guidance.
- fixture golden cards whose site metadata has unknown kind or visibility,
  invalid source coordinates, invalid file paths, incoherent public API flags,
  or operation/snippet drift.
- fixture golden cards whose class, priority, or confidence are unknown or
  inconsistent with the fixture-pinned classification signal.
- fixture golden cards whose witness routes are missing, required by default,
  outside the operation family registry row, whose commands do not match the
  route kind, or out of sync with verify_commands.
- fixture golden ReviewCard IDs that omit stable fixture/package, file, owner,
  site kind, operation family, operation path/callee, snippet hash, hazard, or
  counted suffix components.
- fixture golden cards whose operation_family or hazards use unknown domain
  vocabulary, whose hazards fall outside the operation family registry row, or
  whose hazard list contains duplicate entries.

## Proof commands

- cargo run --locked -p xtask -- check-calibration
- cargo run --locked -p xtask -- check-dogfood
- cargo run --locked -p xtask -- check-doc-artifacts
- cargo run --locked -p xtask -- check-goals
- cargo run --locked -p xtask -- check-pr
- cargo run --locked -p xtask -- source-divergence
- git diff --check
