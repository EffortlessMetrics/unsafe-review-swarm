---
name: publish-pr
description: Use when the branch is effectively complete to publish a concise ready PR with a review index, keeping raw logs and orchestration telemetry out of the PR body.
---

# Publish PR

Use when the branch is effectively complete and the next step is publication. Ready is the default; draft requires a named exception.

## Triggers

- A scoped commit exists and local proof for the contract has been run.
- A draft PR must be justified for re-evaluation.

Do not use this skill to publish a second PR on the same mutation surface or to treat local green as merge readiness.

## Workflow

1. Verify readiness: confirm the work matches the controlling issue or work spec, scope and non-goals are respected, and focused proof has been run for the head being published. Do not treat local green as hosted integration.
2. Choose publication mode: publish `ready` by default. Use `draft` only for a named exception — remote-only proof, genuine collaboration, experiment, or maintainer meaning decision — and record that exception in the PR body.
3. Render a concise review index in the PR body (template: `.github/PULL_REQUEST_TEMPLATE.md`): controlling `issue` and `work_spec` links, actual change (files and seams), `INV-*` and `AC-*` IDs satisfied, proof summary (commands and results with head SHA), deviations from the work spec, claim boundary (what is and is not established), risk and rollback, and release-note disposition. Keep it scannable.
4. Exclude raw worker logs, internal queues, role narratives, and orchestration telemetry from the PR body. Link overflow artifacts (check logs, result files) by reference instead of pasting them.
5. Keep hosted checks, reviews, and repository policy as the merge authority. Local `check-pr` results, builder self-reports, and bot verdicts remain author evidence until checked against hosted state.

## Boundaries

- No branch-protection bypass, auto-merge authority, fixed agent topology, or product behavior is added.
- A draft PR requires an explicit named exception; incomplete work alone is not an exception.
- Manual and non-Codex workflows remain valid: publish via direct Git and GitHub operations using the template and lifecycle map.

## Claim boundary

This skill establishes PR publication guidance. It does not prove the implementation, establish hosted integration, or authorize merge.

Reference: `AGENTS.md`, `docs/contributing/LIFECYCLE_SURFACE_MAP.md`, `docs/contributing/AGENT-ORCHESTRATION.md`, `.github/PULL_REQUEST_TEMPLATE.md`, `docs/specs/UNSAFE-REVIEW-SPEC-0044-issue-linked-work-specs.md`.
