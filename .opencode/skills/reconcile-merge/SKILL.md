---
name: reconcile-merge
description: Use after merge or deliberate closure to verify the landed effect on current main, update durable authorities, classify residue, and encode reusable learning only as durable tests, contracts, tools, or policy.
---

# Reconcile Merge

Use after a PR merges or is deliberately closed. Reconciliation is bound to current `origin/main` ancestry, not a stale branch head.

## Triggers

- A PR merged and `origin/main` has advanced with its commit.
- A PR or issue was deliberately closed without merge and needs disposition recorded.

Do not use this skill to close an umbrella issue because one child merged, or to delete ambiguous or unique residue automatically.

## Workflow

1. Verify the landed effect: confirm the merge commit is present on current `origin/main` (`git fetch origin && git log --oneline origin/main`), or record the deliberate closure reason (duplicate, superseded, rejected, abandoned, unrecoverable) with evidence. Re-run focused post-merge checks where integration could change the result.
2. Update durable authorities accurately: set `issue` and `work_spec` disposition to the actual completed scope, correct `proof` and `support` claims, record `release-note` disposition, and create bounded follow-ups as separate issues with `ACCEPTED_FOLLOW_UP` context. Do not close an umbrella because one child merged.
3. Release branch and worktree ownership: mark the lane's branch and worktree as releasable only after `origin/main` verification. Keep the primary checkout and any ambiguous or user-owned state.
4. Classify residue `KEEP | CACHE_ONLY | REMOVE | SALVAGE | REVIEW` using advisory `cargo run --locked -p xtask -- cleanup-audit` where useful. Never delete `REVIEW` or ambiguous or unique work automatically; explicit controller action is required for removal. Cache-only artifacts may remain for build-cache guidance per `docs/contributing/BUILD_CACHE_SETUP.md`.
5. Encode reusable learning only when it improves a durable `test`, `contract`, `tool`, `policy`, or `architecture`. Do not encode transient logs or role narratives.

## Boundaries

- No branch-protection bypass, auto-merge, fixed agent topology, persistent workflow database, or product behavior is added.
- `cleanup-audit` is advisory; it does not prove ownership or authorize deletion.
- Manual and non-Codex workflows remain valid: verify `origin/main`, update issues, and classify residue via direct Git and GitHub inspection.

## Claim boundary

This skill establishes post-merge reconciliation guidance. It does not prove that every follow-up was captured, authorize deletion of ambiguous state, or make a release or safety claim.

Reference: `AGENTS.md`, `docs/contributing/LIFECYCLE_SURFACE_MAP.md`, `docs/contributing/AGENT-ORCHESTRATION.md`.
