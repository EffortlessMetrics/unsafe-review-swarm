# UNSAFE-REVIEW-SPEC-0044: issue-linked work specs

Status: proposed
Owner: repo-infra
Created: 2026-07-14
Linked proposal: UNSAFE-REVIEW-PROP-0002-source-of-truth-stack
Linked issues:
- https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1900
Support-tier impact: none
Policy impact:
- .allow/artifacts/doc-artifacts.toml
- docs/schemas/issue-work-spec.schema.json

## Problem

GitHub issues hold the operational portfolio, but a bounded issue/PR contract
needs a stable machine-readable shape that a controller, builder, reviewer,
verifier, and closeout can share without copying the whole issue queue into the
repository.

## Contract

Each work spec describes one issue-linked delivery unit. The version-one TOML
shape is defined by
`docs/schemas/issue-work-spec.schema.json` and exemplified by
`plans/work-specs/examples/UNSAFE-REVIEW-WORK-1900.toml`.

The required fields are the issue URL, work kind, objective, user outcome,
claim boundary, included/excluded scope, invariants, acceptance criteria with
proof commands, integration expectations, risks with mitigations, and rollback
strategy. Optional fields name dependencies/blockers, affected files/symbols,
linked durable specs/ADRs, compatibility posture, and later PR/closeout links.

Invariant and acceptance IDs are stable contract identifiers. Verification may
report evidence against them, but must not rewrite their meaning.

## Authority boundary

The work spec MUST NOT contain repository-wide priority, queue rank, current
task, default goal, or issue-status scheduling fields. GitHub issues and project
metadata own the concurrent work portfolio; the Codex controller's current
issue and phase remain ephemeral.

Validation is offline and structural. It does not query GitHub, execute proof
commands, run tests, establish hosted-CI status, or make unsafe-code safety,
UB-free, Miri-clean, precision, recall, or merge-verdict claims.

Cargo-allow 0.1.10 has a closed artifact-kind vocabulary and no `work_spec`
kind. Until a compatible cargo-allow release adds one, the example is
registered as a draft `plan_item` for source-tree graph visibility while
`xtask check-work-specs` validates the content schema. This compatibility slot
must not be mistaken for a scheduler or for first-class cargo-allow support.

## Required evidence

```text
cargo run --locked -p xtask -- check-work-specs
cargo test --locked -p xtask work_specs
cargo-allow check --profile spec-system --mode audit
cargo run --locked -p xtask -- check-doc-artifacts
cargo run --locked -p xtask -- check-pr
git diff --check
```

## Follow-up boundary

Issue/link validation, surface vocabulary, PR/closeout lifecycle rules, and
controller/agent consumption are follow-up slices of #1900 and must not be
implemented by weakening this structural contract.
