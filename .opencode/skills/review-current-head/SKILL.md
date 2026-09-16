---
name: review-current-head
description: Use on an effectively complete branch and after every substantive mutation to review one exact PR and head SHA across warranted dimensions with evidence-first results.
---

# Review Current Head

Use on an effectively complete branch and after every substantive mutation.
Review is bound to one immutable PR and head SHA; any relevant mutation makes
that review stale.

Independent review does not mean human review. It means a distinct lens, fresh
evidence, and an exact-head boundary. A separate agentic reviewer is valid when
its brief is read-only and its evidence is independent of the branch author's
self-report.

## Triggers

- A scoped commit or PR head exists and is ready for challenge.
- A prior head received feedback and a new head now exists.

Without an exact PR and head SHA the review cannot be performed. That is a
missing input to this review operation, not proof that the whole delivery lane
is blocked.

## Workflow

1. Bind review to one exact PR and head SHA (`basis.pr` and `basis.head_sha` per
   the bounded-subagent brief schema). Report every conclusion against that head
   only.
2. Choose only warranted dimensions for this head: correctness, integration,
   test grip, simplification, claim boundary, safety, performance,
   compatibility, or external truth. Select proportionally to the seam and risk.
3. Use bounded read-only reviewers or verifiers and
   `bounded-subagent-result-v1` results where useful. Read-only briefs carry no
   mutation authority. Use `write_scope` only in a separate writer brief.
4. Distinguish author claims from independent proof. Builder self-reported
   green, local `check-pr` results, and author assertions remain author evidence
   until checked against hosted checks, fresh proof, or the exact diff.
5. Preserve contradictions, scoped `none_found` (naming searched surfaces,
   sources, and limits), and uncertainty. Do not drop an actionable finding or
   turn advisory checks into blocking verdicts.
6. Allow a reviewer to propose or apply a repair, but classify that pass as
   authoring or fix work. The resulting commit is a new head and invalidates
   prior certification; require fresh independent review of the changed head.
7. Classify state precisely. A running hosted check or active reviewer is
   `in-progress`; a known automatic next transition is `waiting`. Use the
   result-schema verdict `blocked` only when the reviewed obligation was
   actually attempted, failed with concrete evidence, has no available
   alternative, and has no independent work remaining. Otherwise use `revise`
   or `not_proven`.
8. Keep GitHub checks and live merge policy authoritative. Once independent
   exact-head challenge and required policy checks are satisfied, the
   coordinator may merge an ordinary internal PR when the selected lane
   authorizes it. Do not invent a human sign-off requirement merely because the
   branch author cannot self-certify independence.

## Result shape

Return a `bounded-subagent-result-v1` with `work_item`, exact `basis`, `verdict`,
bounded `summary`, findings with evidence references, proof, contradictions,
uncertainty, recommended next action, and overflow refs. Large logs stay out of
the durable result.

For `clear`, state the exact head and searched dimensions. For `revise`, give the
smallest actionable repair. For `not_proven`, name the missing evidence without
externalizing executable work to the user. For `blocked`, include the attempted
operation, exact failure, alternatives checked, and why no independent seam can
advance.

## Boundaries

- Independence comes from a distinct lens and fresh exact-head evidence, not a
  fixed agent count or human identity.
- Changed seams receive focused re-review on the new head; unchanged seams do
  not need a full re-fan-out.
- This skill does not mutate, publish, or merge. Its `clear` result is evidence
  consumed by the coordinator and repository policy; it is not a hidden human
  approval gate.
- Source-candidate merge, publication, tags, releases, deployments,
  credentials, and moving public refs retain their separately named authority.

## Claim boundary

This skill establishes exact-head review guidance. It does not prove the
implementation correct or itself authorize merge. It does establish that
independent review may be agentic and that ordinary integration can continue
without a fabricated human-only boundary once the actual authorities are met.

Reference: `AGENTS.md`, `docs/contributing/LIFECYCLE_SURFACE_MAP.md`,
`docs/contributing/AGENT-ORCHESTRATION.md`, and the bounded brief/result schemas.
