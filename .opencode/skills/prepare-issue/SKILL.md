---
name: prepare-issue
description: Use when substantive work lacks current, settled context to locate the controlling GitHub issue, inspect live state, and synthesize scope before mutation.
---

# Prepare Issue

Use when substantive work lacks current, settled context. A mechanical typo,
narrow dependency bump, or other change with no meaningful design or proof
decision should skip this skill and use a proportional fast path instead.

A plan, checklist, or quoted workplan supplied or adopted by the user is active
direction unless it was explicitly presented only for review, comparison, or
fact-checking. Verify its load-bearing facts; do not turn it back into a proposal
that needs a second ceremonial "execute" instruction.

## Triggers

- A substantive unplanned issue has no current, settled premise, scope, or acceptance.
- Issue assumptions conflict with current `main`, overlapping issues or PRs,
  source/swarm divergence, or accepted specs/ADRs/plans.
- The request is not yet safe to admit a writer for a specific reason that
  materially changes the proposed mutation.

Do not use this skill for a mechanical fast-path fix where the issue or PR
already makes the proof unambiguous.

## Workflow

1. Locate or create one controlling GitHub issue. GitHub owns the live portfolio;
   do not treat a label, runtime goal, assignee, model, charter, or local status
   flag as authority.
2. Inspect current `main`, exact base, overlapping issues and PRs,
   source/swarm divergence (`cargo run --locked -p xtask -- source-divergence`),
   and the source-of-truth stack in `AGENTS.md`.
3. Gather bounded repository or external evidence with read-only helpers when
   useful. Use the bounded brief/result schemas when delegating; preserve
   competing explanations, contradictions, and corrected assumptions rather
   than collapsing them.
4. Synthesize only what is settled: scope, explicit non-goals, decisions, proof
   obligations, risk, rollback, and return conditions. Keep research in the
   GitHub issue; do not copy the entire thread into a writer prompt.
5. Distinguish an unsettled product/governance stance from an operational
   unknown. Before claiming no runner, missing permission, human-only review, or
   another external blocker, attempt the smallest safe capability probe and
   record the exact result.
6. If a material premise remains unsettled, isolate the affected mutation and
   state what would settle it, then continue every independent reversible seam.
   Do not convert a pending check, unattempted operation, or one blocked substep
   into a global stop.
7. When the contract is settled, hand directly to `compile-work-spec` or the
   proportional build path in the same work cycle. Preparation is not a
   mandatory pause or authorization ceremony.

## Inputs and outputs

- Inputs: selected GitHub issue or PR, current disposition, linked
  specs/ADRs/plans, live source references, and any user-adopted workplan.
- Output: evidence-backed issue context plus the first executable next step.
  This skill itself does not mutate repository state, but it must not imply that
  the user must re-authorize already selected reversible work.

## Proportional fast path

When no meaningful design or proof decision exists, prefer a short issue-backed
contract and direct execution over research synthesis. Example: a typo in
`docs/README.md` with an agreed correction needs no `prepare-issue` or
`compile-work-spec` delegation.

## Bounded helper example

For bounded research, create a read-only brief with `action: investigate`,
`capability: read_only`, a named `read_scope`, authorities, and `stop_when`.
Helpers return a `bounded-subagent-result-v1` that cites evidence, preserves
contradictions, and leaves the synthesis decision to the coordinator. Helpers
do not select unrelated work or spawn children by default.

## Boundaries

- GitHub issues remain the live research and portfolio surface, not a mirrored
  packet or database.
- Do not introduce a repository-global task selector or persist runtime goal
  state.
- Manual and non-Codex workflows remain valid: direct Git, GitHub, and Cargo
  inspection is sufficient when helpers are unavailable.
- Publication, source-candidate merge, tags, releases, deployments,
  credentials, and moving public refs retain their named authority boundaries.

## Claim boundary

This skill makes the research-to-contract boundary discoverable. It does not
decide priority or prove that a later work spec is correct. It also does not
remove authorization already supplied by the selected issue, accepted contract,
or current user instruction.

Reference: `AGENTS.md`, `docs/contributing/LIFECYCLE_SURFACE_MAP.md`,
`docs/contributing/AGENT-ORCHESTRATION.md`.
