---
name: respond-to-feedback
description: Use after a current-head review or hosted check reports findings to verify each claim against the exact PR head, classify, batch accepted repairs through one writer, and rerun proof with focused re-review.
---

# Respond To Feedback

Use after a review or hosted check reports findings on the current head. Feedback handling is bound to one exact PR head and its primary artifacts; any mutation creates a new head that requires fresh review.

## Triggers

- A human, bot, or CI finding exists on the current PR head.
- A prior review returned `revise`, `blocked`, or `not_proven` for the exact head.

Do not use this skill to re-argue settled findings without new evidence, or to create parallel writers on the same branch.

## Workflow

1. Refresh exact head: re-read the current `PR`, `head_sha`, and primary artifacts per `docs/schemas/bounded-subagent-brief.schema.json`. Discard stale-head findings where the reported head no longer matches `origin` or hosted checks.
2. Verify each claim before editing: reproduce the finding against the exact diff, hosted log, or artifact validator. Treat bot or instrument output as a claim, not a verdict. Distinguish provider or instrument failure (timeouts, missing capabilities, misconfigured lanes) from a product defect; do not present instrument failure as product failure.
3. Classify findings: `product`, `test`, `documentation`, `policy`, `instrument`, `flaky`, `stale-head`, `duplicate`, or `out-of-scope`. Preserve contradictions and uncertainty until resolved with evidence per `docs/schemas/bounded-subagent-result.schema.json` (`contradictions`, `uncertainty`); do not collapse them.
4. Batch accepted repairs through one current writer: collect `FIXED` items into a single scoped repair commit owned by the admitted writer on the existing branch and worktree. Enforce one writer per mutation surface; do not fan out to parallel writers on the same branch. Document `REFUTED_WITH_EVIDENCE`, `SUPERSEDED`, or `ACCEPTED_FOLLOW_UP` with evidence or a linked follow-up issue instead of editing.
5. Rerun affected proof and re-review changed seams on the new head: execute only the proof named by the accepted contract that exercises the changed seam, then request focused re-review of that seam. The new head invalidates prior certification; unchanged seams do not need a full re-fan-out but require explicit confirmation they were unaffected.
6. Return a `bounded-subagent-result-v1` for triage where delegation is used (`action: triage_ci` or `verify` with `capability: read_only` and empty `write_scope`), with `findings` dispositions (`FIXED`, `REFUTED_WITH_EVIDENCE`, `SUPERSEDED`, `ACCEPTED_FOLLOW_UP`), evidence references, and overflow refs for large logs.

## Boundaries

- No merge authorization, branch-protection bypass, or auto-merge is added. Hosted checks and repository policy remain the merge authority.
- No persistent workflow database or product behavior is added.
- Manual and non-Codex workflows remain valid: inspect the PR head, hosted checks, and thread directly via Git and GitHub.

## Claim boundary

This skill establishes current-head feedback guidance. It does not prove the repair correct, establish hosted integration, or authorize merge.

Reference: `AGENTS.md`, `docs/contributing/LIFECYCLE_SURFACE_MAP.md`, `docs/contributing/AGENT-ORCHESTRATION.md`, `docs/schemas/bounded-subagent-brief.schema.json`, `docs/schemas/bounded-subagent-result.schema.json`.
