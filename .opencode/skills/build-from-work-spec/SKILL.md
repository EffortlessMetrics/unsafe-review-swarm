---
name: build-from-work-spec
description: Use immediately before one writer starts or resumes to consume the approved issue-linked work spec, verify live admission and worktree, and create one bounded writer brief with edit cage and proof boundary.
---

# Build From Work Spec

Use immediately before one writer starts or resumes. Delegation is optional for a narrow edit that one writer can complete more cheaply.

## Triggers

- An approved issue-linked work spec exists for the selected issue and the next step is admitting one writer for one PR.
- A writer is resuming after feedback or interruption.

Do not use this skill to repeat discovery already compiled into the work spec, or to create parallel writers on the same branch or overlapping mutation surface.

## Workflow

1. Consume the current work spec defined by `docs/specs/UNSAFE-REVIEW-SPEC-0044-issue-linked-work-specs.md` and `docs/schemas/issue-work-spec.schema.json` (issue #1900) together with live facts: selected issue and disposition, exact base SHA, overlapping PRs and branches, worktree ownership, and source/swarm divergence (`cargo run --locked -p xtask -- source-divergence`).
2. Reuse an existing safe worktree and branch where appropriate rather than creating duplicate work. Inspect `git worktree list`, `git branch`, and open PRs; reject or serialize duplicate or overlapping writers on the same mutation surface. One accountable writer owns one branch at a time.
3. Create exactly one writer brief per `docs/schemas/bounded-subagent-brief.schema.json` with `action: build` and `capability: write`. It must point to one issue, one `work_spec` path under `plans/work-specs/examples/`, the admitted `basis.base_sha`, `admission.worktree`, an explicit `write_scope` edit cage as canonical repo-relative paths, `proof_obligations`, latitude, explicit `non_goals`, and `stop_when` return conditions.
4. Require discriminating tests or an executable oracle before implementation where practical. Keep independent read-only research or proof outside the writer context; do not copy the entire issue history into the writer prompt.
5. Return to the issue when code contradicts a material premise or the requested scope must change; do not silently expand `write_scope` or invariants.
6. Builder self-report and local green remain author claims until independently checked against hosted checks and exact-head review.

## Brief shape

Writer brief fields: `schema: bounded-subagent-brief-v1`, `work_item.issue`, `work_item.work_spec`, `basis.base_sha`, `admission.state: admitted` with `worktree`, `action: build`, `capability: write`, `objective`, `read_scope`, `write_scope` (non-empty), `authorities`, `proof_obligations`, `non_goals`, `stop_when`, `return_schema: bounded-subagent-result-v1`. Validation is offline via `cargo run --locked -p xtask -- check-subagent-briefs`.

## Boundaries

- Narrow edits may stay single-agent without delegation when that is cheaper; this skill does not mandate helpers.
- Read-only work uses separate bounded briefs (`action: investigate` or `verify` with `capability: read_only` and empty `write_scope`) and returns `bounded-subagent-result-v1` per issue #1926.
- No feedback batching, PR publication, merge authorization, cleanup automation, or product behavior is added.
- Manual and non-Codex workflows remain valid: inspect the issue, work spec, and worktree directly.

## Claim boundary

This skill establishes one-writer build guidance. It does not prove the implementation, authorize merge, or prevent a reviewer from becoming a fixer on a later head.

Reference: `AGENTS.md`, `docs/contributing/LIFECYCLE_SURFACE_MAP.md`, `docs/contributing/AGENT-ORCHESTRATION.md`, `docs/specs/UNSAFE-REVIEW-SPEC-0044-issue-linked-work-specs.md`, `docs/schemas/bounded-subagent-brief.schema.json`, `docs/schemas/bounded-subagent-result.schema.json`.
