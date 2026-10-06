---
name: respond-to-feedback
description: Use after a current-head review or hosted check reports findings to verify each claim against the exact PR head, classify, batch accepted repairs through one writer, and rerun proof with focused re-review.
---

# Respond To Feedback

Use after a review or hosted check reports findings on the current head.
Feedback handling is bound to one exact PR head and its primary artifacts; any
mutation creates a new head that requires fresh review.

## Triggers

- A human, agentic reviewer, bot, or CI finding exists on the current PR head.
- A prior review returned `revise`, `blocked`, or `not_proven` for the exact
  head.

Do not re-argue settled findings without new evidence or create parallel writers
on the same branch.

## Workflow

1. Refresh exact head: re-read the current PR, head SHA, and primary artifacts.
   Discard stale-head findings where the reported head no longer matches the
   live branch or hosted checks.
2. Verify each claim before editing: reproduce the finding against the exact
   diff, hosted log, or artifact validator. Treat bot or instrument output as a
   claim, not a verdict. Distinguish provider or instrument failure from a
   product defect.
3. Classify findings: `product`, `test`, `documentation`, `policy`,
   `instrument`, `flaky`, `stale-head`, `duplicate`, or `out-of-scope`.
   Preserve contradictions and uncertainty until resolved with evidence.
4. Batch accepted repairs through one current writer: collect `FIXED` items into
   a single scoped repair commit owned by the admitted writer. Document
   `REFUTED_WITH_EVIDENCE`, `SUPERSEDED`, or `ACCEPTED_FOLLOW_UP` with evidence
   or a linked follow-up issue instead of editing.
5. If a capability or permission is questioned, attempt the smallest safe
   operation and record the exact result before classifying the item as
   external. A missing pre-existing workflow, runner, reviewer, or ruleset is
   not itself a blocker.
6. Rerun affected proof and re-review changed seams on the new head. The new
   head invalidates prior certification; unchanged seams need explicit
   confirmation, not an automatic full re-fan-out.
7. Keep the lane active while checks or review run. Classify them as
   `in-progress` or `waiting`; continue independent work. Use `blocked` only
   after an attempted failure, exhausted alternatives, and no independent seam.
8. After a clear independent exact-head challenge and required green checks,
   return the current head to the coordinator for ordinary internal merge and
   reconciliation when live policy permits. Do not externalize that work to the
   user merely because the same agent authored an earlier head.
9. Return a `bounded-subagent-result-v1` for delegated triage, with findings
   dispositions, evidence references, contradictions, uncertainty, and bounded
   overflow refs.

## Boundaries

- No branch-protection bypass or automatic policy override is added. Hosted
  checks and repository policy remain authoritative.
- This skill does not itself merge. Its role boundary does not create a
  human-only merge requirement; the coordinator may merge an ordinary internal
  PR after independent exact-head review and policy checks.
- Source-candidate merge, publication, tags, releases, deployments,
  credentials, and moving public refs remain separately authorized objects.
- No persistent workflow database or product behavior is added.
- Manual and non-Codex workflows remain valid through direct Git and GitHub
  inspection.

## Claim boundary

This skill establishes current-head feedback guidance. It does not prove the
repair correct, establish hosted integration, or itself authorize merge. It
also does not terminate or globally block an active repair lane because a check
is running or one operational step has not yet been attempted.

Reference: `AGENTS.md`, `docs/contributing/LIFECYCLE_SURFACE_MAP.md`,
`docs/contributing/AGENT-ORCHESTRATION.md`, and the bounded brief/result schemas.
