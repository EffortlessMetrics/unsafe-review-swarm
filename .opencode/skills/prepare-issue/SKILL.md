---
name: prepare-issue
description: Use when substantive work lacks current, settled context to locate the controlling GitHub issue, inspect live state, and synthesize scope before mutation.
---

# Prepare Issue

Use when substantive work lacks current, settled context. A mechanical typo, narrow dependency bump, or other change with no meaningful design or proof decision should skip this skill and use a proportional fast path instead.

## Triggers

- A substantive unplanned issue has no current, settled premise, scope, or acceptance.
- Issue assumptions conflict with current `main`, overlapping issues or PRs, source/swarm divergence, or accepted specs/ADRs/plans.
- The request is not yet safe to admit a writer for.

Do not use this skill for a mechanical fast-path fix where the issue or PR already makes the proof unambiguous.

## Workflow

1. Locate or create one controlling GitHub issue. GitHub owns the live portfolio; do not treat a label, goal, assignee, model, or runtime state as authority.
2. Inspect current `main`, exact base, overlapping issues and PRs, source/swarm divergence (`cargo run --locked -p xtask -- source-divergence`), and the source-of-truth stack in `AGENTS.md`.
3. Gather bounded repository or external evidence with read-only helpers when useful. Use the bounded brief/result schemas `docs/schemas/bounded-subagent-brief.schema.json` and `docs/schemas/bounded-subagent-result.schema.json` when delegating; preserve competing explanations, contradictions, and corrected assumptions rather than collapsing them.
4. Synthesize only what is settled: scope, explicit non-goals, decisions, proof obligations, risk, rollback, and return conditions. Keep research in the GitHub issue; do not copy the entire thread into a writer prompt.
5. Stop before mutation when a material premise remains unsettled. Return the contradiction and what would settle it.

## Inputs and outputs

- Inputs: selected GitHub issue or PR, current disposition, linked specs/ADRs/plans, live source references.
- Output: evidence-backed issue context. No branch, worktree, implementation, review, publication, merge, or cleanup is performed here.

## Proportional fast path

When no meaningful design or proof decision exists, prefer a short issue-backed contract and direct execution over research synthesis. Example: a typo in `docs/README.md` with an agreed correction needs no `prepare-issue` or `compile-work-spec` delegation.

## Bounded helper example

For bounded research, create a read-only brief with `action: investigate`, `capability: read_only`, a named `read_scope`, authorities, and `stop_when`. Helpers return a `bounded-subagent-result-v1` that cites evidence, preserves contradictions, and leaves the synthesis decision to the coordinator. Helpers do not select unrelated work or spawn children by default.

## Boundaries

- GitHub issues remain the live research and portfolio surface, not a mirrored packet or database.
- Do not introduce branch or worktree creation, implementation, review, publication, merge, or cleanup behavior.
- Manual and non-Codex workflows remain valid: direct Git, GitHub, and Cargo inspection is sufficient when helpers are unavailable.

## Claim boundary

This skill makes the research-to-contract boundary discoverable. It does not decide priority, authorize mutation, or prove that any later work spec is correct.

Reference: `AGENTS.md`, `docs/contributing/LIFECYCLE_SURFACE_MAP.md`, `docs/contributing/AGENT-ORCHESTRATION.md`.
