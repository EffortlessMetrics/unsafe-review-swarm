# UNSAFE-REVIEW-PROP-0002: source-of-truth stack

Status: proposed
Owner: repo-infra
Created: 2026-05-20
Target milestone: 0.2.0
Linked specs:
- UNSAFE-REVIEW-SPEC-0020-source-of-truth-stack
Linked ADRs:
- none
Linked plan:
- plans/0.2.0/implementation-plan.md
Support-tier impact: yes
Policy impact: yes

## Problem

Repository intent, behavior contracts, sequencing, and proof claims are distributed across prose and are not uniformly linked by policy artifacts.

## Users and surfaces

Contributors, release maintainers, and agents consume docs/, plans/, policy/, and CI workflow outputs.

## Success criteria

A linked artifact chain exists from proposal to spec to plan to active goals, with support-tier and policy mappings for enforceability.

## Proposed shape

Adopt explicit templates, active goals, artifact ledgers, and policy workflow checks.

## Alternatives considered

Keep ad hoc docs; rejected because it cannot be validated reliably by automation.

## Specs to create or update

- UNSAFE-REVIEW-SPEC-0020

## Architecture decisions needed

- none

## Implementation campaign shape

Scaffold first, then ledger, then checkers, then CI tightening.

## Evidence plan

`git diff --check`, TOML parse checks, policy contract workflow execution.

## Risks

Initial migration overhead and temporary dual patterns while existing docs converge.

## Non-goals

No runtime behavior change in this proposal.

## Exit criteria

Linked artifacts and policy-ledger baseline land and are reviewable.
