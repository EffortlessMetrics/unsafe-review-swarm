---
name: build-from-work-spec
description: Use immediately before one writer starts or resumes to consume the approved issue-linked work spec, verify live admission and worktree, and create one bounded writer brief with edit cage and proof boundary.
---

# Build From Work Spec

Use immediately before one writer starts or resumes. Delegation is optional for
a narrow edit that one writer can complete more cheaply. Once admission is
settled, this skill moves into execution; it is not another authorization gate.

## Triggers

- An approved issue-linked work spec exists for the selected issue and the next
  step is admitting one writer for one PR.
- A writer is resuming after feedback, a wait, or interruption.

Do not use this skill to repeat discovery already compiled into the work spec,
or to create parallel writers on the same branch or overlapping mutation
surface.

## Workflow

1. Consume the current work spec defined by SPEC-0044 and its schema together
   with live facts: selected issue and disposition, exact base SHA, overlapping
   PRs and branches, worktree ownership, and source/swarm divergence.
2. Reuse an existing safe worktree and branch where appropriate rather than
   creating duplicate work. Inspect worktrees, branches, and open PRs; reject or
   serialize duplicate or overlapping writers on the same mutation surface.
3. Create exactly one writer brief per the bounded-subagent brief schema with
   `action: build` and `capability: write`. It must point to one issue, one work
   spec, exact base, admitted worktree, explicit edit cage, proof obligations,
   latitude, non-goals, and return conditions.
4. Require discriminating tests or an executable oracle before implementation
   where practical. Keep independent read-only research or proof outside the
   writer context; do not copy the entire issue history into the writer prompt.
5. If code contradicts a material premise or the requested mutation surface
   must change, isolate the exact delta and return it to the issue. Continue any
   independent in-scope work; do not silently expand `write_scope`, but do not
   convert one scope question into a global blocked lane.
6. When a named runner, permission, command, review, or merge path appears
   unavailable, attempt the smallest safe operation and capture the exact
   result before externalizing it. Missing pre-existing configuration is not
   evidence that the capability does not exist.
7. Treat running commands/checks/reviews as `in-progress` and known automatic
   transitions as `waiting`. Use `blocked-evidenced` only after an attempted
   failure, exhausted alternatives, and no independent work.
8. Builder self-report and local green remain author evidence until
   independently checked against hosted checks and exact-head review.

## Brief shape

Writer brief fields: `schema: bounded-subagent-brief-v1`, `work_item.issue`,
`work_item.work_spec`, `basis.base_sha`, `admission.state: admitted` with
`worktree`, `action: build`, `capability: write`, `objective`, `read_scope`,
`write_scope` (non-empty), `authorities`, `proof_obligations`, `non_goals`,
`stop_when`, `return_schema: bounded-subagent-result-v1`. Validation is offline
via `cargo run --locked -p xtask -- check-subagent-briefs`.

## Boundaries

- Narrow edits may stay single-agent without delegation when that is cheaper;
  this skill does not mandate helpers.
- Read-only work uses separate bounded briefs and returns a
  `bounded-subagent-result-v1`.
- The writer does not self-certify independent review or authorize publication.
  Those facts do not require a human-only handoff: the coordinator may obtain a
  distinct exact-head review and continue through ordinary merge when the
  selected lane and repository policy permit it.
- Source-candidate merge, publication, tags, releases, deployments,
  credentials, and moving public refs retain their named owner boundaries.
- Manual and non-Codex workflows remain valid: inspect the issue, work spec,
  worktree, proof, and current policy directly.

## Claim boundary

This skill establishes one-writer build guidance. It does not prove the
implementation or authorize a reserved irreversible operation. It also does not
turn a coordinator into a non-executing role or require a second go after the
user has selected the lane.

Reference: `AGENTS.md`, `docs/contributing/LIFECYCLE_SURFACE_MAP.md`,
`docs/contributing/AGENT-ORCHESTRATION.md`, SPEC-0044, and the bounded
brief/result schemas.
