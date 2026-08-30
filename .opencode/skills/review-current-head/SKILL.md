---
name: review-current-head
description: Use on an effectively complete branch and after every substantive mutation to review one exact PR and head SHA across warranted dimensions with evidence-first results.
---

# Review Current Head

Use on an effectively complete branch and after every substantive mutation. Review is bound to one immutable PR and head SHA; any relevant mutation makes that review stale.

## Triggers

- A scoped commit or PR head exists and is ready for challenge.
- A prior head received feedback and a new head now exists.

Without an exact PR and head SHA the review cannot be performed.

## Workflow

1. Bind review to one exact PR and head SHA (`basis.pr` and `basis.head_sha` per `docs/schemas/bounded-subagent-brief.schema.json`). For `action: review` the schema requires both; a brief without them fails `cargo run --locked -p xtask -- check-subagent-briefs`. Report every conclusion against that head only.
2. Choose only warranted dimensions for this head: correctness, integration, test grip, simplification, claim boundary, safety, performance, compatibility, or external truth. Do not fan out to every dimension by default; select proportionally to the seam and risk.
3. Use bounded read-only reviewers or verifiers and `bounded-subagent-result-v1` results (issue #1926) where useful. Read-only briefs use `capability: read_only` and an empty `write_scope`; they carry no mutation authority. Use `write_scope` only in a separate writer brief.
4. Distinguish author claims from independent proof. Builder self-reported green, local `check-pr` results, and author assertions remain author evidence until checked against hosted checks, fresh proof, or the exact diff. Treat a green subset as insufficient when it exercises the wrong property.
5. Preserve contradictions, scoped `none_found` (naming searched surfaces, sources, and limits), and uncertainty. Do not drop an actionable finding or turn advisory checks into blocking verdicts.
6. Allow a reviewer to propose or apply a repair, but classify that pass as authoring or fix work. The resulting commit is a new head and invalidates prior certification; require fresh independent review of the new head with focused re-review of changed seams.
7. Keep GitHub reviews, hosted checks, and merge policy authoritative. No agent verdict replaces deterministic gates or repository policy.

## Result shape

Return a `bounded-subagent-result-v1` with `work_item`, `basis` including `pr` and `head_sha`, `verdict` (`clear`, `revise`, `blocked`, `not_proven`), bounded `summary`, `findings` with evidence references, `proof`, `contradictions`, `uncertainty`, `recommended_next_action`, and `overflow` refs. Large logs stay out of the durable result.

## Boundaries

- Independent evidence is required where the cost of a wrong judgment is material; independence comes from a distinct lens and fresh evidence, not a fixed agent count.
- Changed seams receive focused re-review on the new head; unchanged seams do not need a full re-fan-out.
- No feedback batching, PR publication, merge authorization, cleanup automation, or product behavior is added.

## Claim boundary

This skill establishes exact-head review guidance. It does not prove the implementation correct, authorize merge, or prevent a reviewer from becoming a fixer.

Reference: `AGENTS.md`, `docs/contributing/LIFECYCLE_SURFACE_MAP.md`, `docs/contributing/AGENT-ORCHESTRATION.md`, `docs/schemas/bounded-subagent-brief.schema.json`, `docs/schemas/bounded-subagent-result.schema.json`.
