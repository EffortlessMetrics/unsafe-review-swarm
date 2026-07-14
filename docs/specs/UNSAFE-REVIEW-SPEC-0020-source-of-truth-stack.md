# UNSAFE-REVIEW-SPEC-0020: source-of-truth stack contract

Status: accepted
Owner: repo-infra
Created: 2026-05-20
Linked proposal: UNSAFE-REVIEW-PROP-0002-source-of-truth-stack
Linked ADRs:
- none
Linked plan:
- plans/adoption-0.4.0/implementation-plan.md
Linked issues:
- none
Linked PRs:
- TBD
Support-tier impact: `docs/status/SUPPORT_TIERS.md`
Policy impact:
- .allow/artifacts/doc-artifacts.toml
- policy/ci-lane-whitelist.toml
- policy/package-boundary.toml

## Problem

The repository needs a machine-linked contract stack for why/what/how/now/proof boundaries.

## Behavior

The repository MUST maintain linked durable artifacts: proposal, spec, optional
ADR, implementation plan, project charter when required by the configured
cargo-allow profile, support tiers, and policy ledgers. A charter may contain
zero work items and MUST NOT appoint a default issue or controller operation.

The repository-facing governance surface is cargo-allow's `spec-system`
profile and `.allow/` graph. It owns proposal/spec/plan/charter/
support-tier/closeout linkage and emits source-tree graph diagnostics. Its
worklist is a structural report; GitHub issues and project metadata own the
operational portfolio and priority.
`xtask` remains the repository-facing implementation and policy proof surface.
The legacy `.rails/` tree is retained as a read-only parity snapshot until the
migration closeout and must not route new work.

The rule against storing durable state in tool namespaces (`.codex/`, `.spec/`,
`.claude/`, `.jules/`) remains in `AGENTS.md` and the migration guidance in
`docs/contributing/spec-rails.md`.

## Non-goals

This spec does not define runtime unsafe-analysis behavior.

## Required evidence

Policy ledgers parse and referenced artifact files exist.

## Acceptance examples

Proposal `UNSAFE-REVIEW-PROP-0002` links this spec; the project charter links
the durable adoption plan; a selected GitHub issue and its accepted contract
identify current work.

## Test mapping

Policy-contract workflow commands and TOML parse checks.

## Implementation mapping

`docs/`, `plans/`, `.allow/`, `policy/`,
`.github/workflows/`, and `AGENTS.md` for the agent-facing operating entrypoint.

## CI proof

`cargo-allow check --profile spec-system --mode audit`, plus package-boundary,
and CI-lane checks.

## Metrics / promotion rule

Stable once policy contracts run in CI and claims route through support tiers.

## Failure modes

Unlinked artifacts, missing proof commands, and unsupported stable claims must fail validation.
