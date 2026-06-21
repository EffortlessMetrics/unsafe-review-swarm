# Usefulness telemetry lane: low-noise operational diagnostic projection

## End state

Status: landed. SPEC-0038 is accepted and the projection is present in the
first-pr artifact bundle, gate manifest artifact map, review-kit classifier, and
first-pr artifact verifier.

This lane adds a `usefulness-telemetry.json` artifact emitted alongside all
first-pr artifacts. It is a pure read-only projection from existing
`ReviewCard/Summary/CoverageBlock/CommentPlan` data. No new analysis state.
The ReviewCard remains the single truth object.

The artifact surfaces card inventory, coverage slot gaps, agent readiness,
comment selection counts, confidence distribution, and actionability
distribution as operational diagnostic data only — not calibrated, not a
measurement of detection accuracy, not a gate, not a merge verdict.

## Spec

UNSAFE-REVIEW-SPEC-0038: low-noise usefulness telemetry.

## Proof commands

```text
cargo test -p unsafe-review-core usefulness_telemetry
cargo test -p unsafe-review --test e2e first_pr_emits_usefulness_telemetry_artifact
cargo run --locked -p xtask -- check-pr
```

## Work items

- `usefulness-telemetry-projection` - done; SPEC-0038 projection, e2e test,
  artifact wiring, review-kit classifier, and verifier checks landed.
