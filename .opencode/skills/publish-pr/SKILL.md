---
name: publish-pr
description: Use when the branch is effectively complete to publish a concise ready PR with a review index, keeping raw logs and orchestration telemetry out of the PR body.
---

# Publish PR

Use when the branch is effectively complete and the next step is PR
publication. Ready is the default; draft requires a named exception. This skill
is one transition in an active delivery lane, not a handoff that makes the user
responsible for every later review or merge operation.

## Triggers

- A scoped commit exists and local proof for the contract has been run.
- A draft PR must be justified for re-evaluation.

Do not publish a second PR on the same mutation surface or treat local green as
merge readiness.

## Workflow

1. Verify readiness: confirm the work matches the controlling issue or work
   spec, scope and non-goals are respected, and focused proof has been run for
   the exact head being published. Do not treat local green as hosted
   integration.
2. Choose publication mode: publish `ready` by default. Use `draft` only for a
   named exception—remote-only proof, genuine collaboration, experiment, or a
   real maintainer meaning decision—and record that exception in the PR body.
3. Render a concise review index in the PR body using the repository template:
   controlling issue/work-spec links, actual files and seams, `INV-*` and
   `AC-*` results, proof summary with head SHA, deviations, claim boundary, risk,
   rollback, and release-note disposition.
4. Exclude raw worker logs, internal queues, role narratives, runtime goal
   percentages, and orchestration telemetry. Link bounded overflow evidence by
   reference rather than pasting it.
5. Keep the lane active while hosted checks and independent exact-head review
   run; classify them as `in-progress` or `waiting`, not blocked. Continue any
   independent work and resume the known transition when the result arrives.
6. Distinguish author evidence from independent challenge. A separate agentic
   or human reviewer may supply the exact-head review. Independent does not mean
   human-only.
7. Once required hosted checks, exact-head challenge, live mergeability, and
   repository policy are satisfied, return control to the coordinator for
   ordinary internal merge and reconciliation. Do not manufacture a new owner
   approval requirement unless live policy or the selected contract actually
   requires one.

## Boundaries

- No branch-protection bypass, automatic policy override, fixed agent topology,
  or product behavior is added.
- A draft PR requires an explicit named exception; incomplete work alone is not
  an exception.
- This skill does not itself merge. That is a role boundary, not a human-only
  authority claim: the coordinator may perform the ordinary internal merge
  when the selected lane and repository policy authorize it.
- Source-candidate merge, publication, tags, releases, deployments,
  credentials, and moving public refs remain separately authorized objects.
- Manual and non-Codex workflows remain valid through direct Git and GitHub
  operations using the template and lifecycle map.

## Claim boundary

This skill establishes PR publication guidance. It does not prove the
implementation, establish hosted integration, or itself authorize merge. It
also does not terminate the delivery lane at PR creation or externalize the
remaining review-and-integration work to the user.

Reference: `AGENTS.md`, `docs/contributing/LIFECYCLE_SURFACE_MAP.md`,
`docs/contributing/AGENT-ORCHESTRATION.md`,
`.github/PULL_REQUEST_TEMPLATE.md`, and SPEC-0044.
