# Adoption 0.4.0 implementation plan

Artifact ID: UNSAFE-REVIEW-PLAN-0003

Status: active
Linked proposal: UNSAFE-REVIEW-PROP-0002
Linked spec: UNSAFE-REVIEW-SPEC-0020
Linked support tier: UNSAFE-REVIEW-SUPPORT-0001
Linked goal: UNSAFE-REVIEW-GOAL-0001
Linked closeout: UNSAFE-REVIEW-CLOSEOUT-0001

## Objective

Move governance authority to cargo-allow in advisory posture, make the
documented public product real, promote the accumulated swarm work, and build
measured external usefulness evidence without broadening unsafe-review's
advisory claim boundary.

## Current lane posture

The publication-facing portions of this plan are intentionally parked while
the swarm repository is improved locally. The project charter records the
durable publication boundary; GitHub issue/PR metadata owns queue and
disposition state. Publication work remains parked until the owner explicitly
reopens it. Do not publish
crates, promote source, create or move release tags, move `v1`, or run the
prebuilt-binary lane as part of the current work.

Local implementation, contract coverage, corpus calibration, hostile-input
hardening, read-only pilots, and the diagnostic-to-agent loop remain active.
The delivery artifacts below stay linked as future release work so that
parking publication does not create a second plan or lose the eventual
handoff.

## Work sequence

1. Keep the `.allow` graph and neutral project charter truthful; legacy `.rails`
   remains archive-only. Do not mirror the GitHub issue portfolio in the repo.
2. Reconcile local first-use, tokmd, and consumer-contract work without
   requiring a public release or external Action ref.
3. Reproduce known current-code product gaps, then run rotating holdouts,
   evidence-loss challenges, and read-only external PR pilots.
4. Harden fail-closed behavior and validate the diagnostic-to-agent loop before
   choosing another analyzer or UX expansion.
5. When publication is reopened, resume the parked Action, promotion, release,
   and binary-delivery steps from this plan.

## Invariants

- ReviewCard remains the canonical product unit and all consumers project it.
- The tool remains build-free and advisory by default.
- No automatic comments, witness execution, source edits, or merge blocking.
- Cargo-allow validates source-tree graph structure only; it does not run the
  implementation proof commands.

## Proof commands

```bash
cargo-allow doctor --profile spec-system
cargo-allow check --profile spec-system --mode audit
cargo-allow worklist --profile spec-system --format json
cargo run --locked -p xtask -- check-doc-artifacts
cargo run --locked -p xtask -- check-pr
cargo run --locked -p xtask -- source-divergence
git diff --check
```

## Exit criteria

- `.allow` and cargo-allow are the only active governance route; `.rails` is
  archive-only and no checker or agent instruction routes new work there.
- Publication work is explicitly parked by the durable charter boundary and can
  resume from the linked delivery artifacts without changing governance
  authority.
- The swarm has current producer/consumer contracts, pinned corpus evidence,
  fail-closed regression coverage, and read-only usefulness receipts suitable
  for a later release cutline review.
- Holdouts, evidence-loss challenges, and external pilots provide repeated
  usefulness evidence before any detector or UX broadening decision.
- ReviewCard identity and semantics remain coherent across the CLI, Action,
  SARIF, LSP, agent, tokmd, policy, and ub-review projections.

## Rollback

Remove the `.allow` authority links and restore the previous governance files
only through an explicit owner-approved migration decision, while leaving the
legacy `.rails` snapshot intact as read-only history.

## Claim boundary

This plan proves governance linkage and delivery/usefulness evidence only. It
does not prove UB-freedom, memory safety, soundness, calibrated accuracy,
site execution, witness success, or a merge verdict.
