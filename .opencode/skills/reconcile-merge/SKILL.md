---
name: reconcile-merge
description: Use after merge or deliberate closure to verify the landed effect on current main, update durable authorities, classify residue, and encode reusable learning only as durable tests, contracts, tools, or policy.
---

# Reconcile Merge

Use after a PR merges or is deliberately closed. Reconciliation is bound to
current `origin/main` ancestry, not a stale branch head.

## Triggers

- A PR merged and `origin/main` has advanced with its commit.
- A PR or issue was deliberately closed without merge and needs disposition
  recorded.
- A preparatory lane reached its named owner-decision stop and needs an honest
  closeout before the separately authorized operation.

Do not close an umbrella issue because one child merged, delete ambiguous or
unique residue automatically, or hold a completed preparatory lane open merely
because its later owner decision has not yet been made.

## Workflow

1. Verify the landed effect: confirm the merge commit is present on current
   `origin/main`, or record the deliberate closure reason with evidence. Re-run
   focused post-merge checks where integration could change the result.
2. Update durable authorities accurately: set issue and work-spec disposition
   to the actual completed scope, correct proof and support claims, record the
   release-note disposition, and create bounded follow-ups as separate issues.
3. When the selected objective says “stop for the owner decision,” verify that
   the preparatory evidence and decision packet are complete, mark that lane
   complete, and stop before the reserved action. The absent decision is not a
   blocker or unfinished criterion inside the preparatory lane.
4. Release branch and worktree ownership only after `origin/main` verification.
   Keep the primary checkout and any ambiguous or user-owned state.
5. Classify residue `KEEP | CACHE_ONLY | REMOVE | SALVAGE | REVIEW` using
   advisory `cargo run --locked -p xtask -- cleanup-audit` where useful. Never
   delete `REVIEW`, ambiguous, or unique work automatically.
6. Encode reusable learning only when it improves a durable test, contract,
   tool, policy, or architecture. Do not encode transient logs, runtime goal
   states, or role narratives.

## Boundaries

- No branch-protection bypass, automatic policy override, fixed agent topology,
  persistent workflow database, or product behavior is added.
- `cleanup-audit` is advisory; it does not prove ownership or authorize
  deletion.
- Source-candidate merge, publication, tags, releases, deployments,
  credentials, and moving public refs retain their separately named authority.
- Manual and non-Codex workflows remain valid through direct Git and GitHub
  inspection.

## Claim boundary

This skill establishes post-merge and decision-boundary reconciliation
guidance. It does not prove every follow-up was captured, authorize deletion of
ambiguous state, or make a release or safety claim. It does prevent a completed
preparatory lane from being mislabeled incomplete while waiting for a later
reserved decision.

Reference: `AGENTS.md`, `docs/contributing/LIFECYCLE_SURFACE_MAP.md`,
`docs/contributing/AGENT-ORCHESTRATION.md`.
