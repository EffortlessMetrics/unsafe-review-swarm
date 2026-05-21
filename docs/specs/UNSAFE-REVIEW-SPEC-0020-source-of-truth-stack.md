# UNSAFE-REVIEW-SPEC-0020: source-of-truth stack contract

Status: accepted
Owner: repo-infra
Created: 2026-05-20
Linked proposal: UNSAFE-REVIEW-PROP-0002-source-of-truth-stack
Linked ADRs:
- none
Linked plan:
- plans/0.2.0/implementation-plan.md
Linked issues:
- none
Linked PRs:
- TBD
Support-tier impact: `docs/status/SUPPORT_TIERS.md`
Policy impact:
- policy/doc-artifacts.toml
- policy/ci-lane-whitelist.toml
- policy/package-boundary.toml

## Problem

The repository needs a machine-linked contract stack for why/what/how/now/proof boundaries.

## Behavior

The repository MUST maintain linked artifacts: proposal, spec, optional ADR, implementation plan, active goal manifest, support tiers, and policy ledgers.

## Non-goals

This spec does not define runtime unsafe-analysis behavior.

## Required evidence

Policy ledgers parse and referenced artifact files exist.

## Acceptance examples

Proposal `UNSAFE-REVIEW-PROP-0002` links this spec; this spec links `plans/0.2.0/implementation-plan.md`; active goals reference both.

## Test mapping

Policy-contract workflow commands and TOML parse checks.

## Implementation mapping

`docs/`, `plans/`, `.unsafe-review-spec/goals/`, `policy/`,
`.github/workflows/`.

## CI proof

`cargo run --locked -p xtask -- check-doc-artifacts`, plus goal, package-boundary,
and CI-lane checks.

## Metrics / promotion rule

Stable once policy contracts run in CI and claims route through support tiers.

## Failure modes

Unlinked artifacts, missing proof commands, and unsupported stable claims must fail validation.
