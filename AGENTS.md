# Agent operating contract

## Command style

Prefix local commands with `rtk`. If a command in docs omits `rtk`, keep the
documented command text intact, but run it locally with the `rtk` prefix.

Examples:

```bash
rtk cargo run --locked -p xtask -- check-pr
rtk git diff --check
rtk gh pr view 123 --repo EffortlessMetrics/unsafe-review-swarm
```

## Repository roles

The operating model is:

```text
unsafe-review-swarm develops.
unsafe-review publishes.
```

`EffortlessMetrics/unsafe-review-swarm` is the workbench for routine
implementation, analyzer/evidence changes, fixtures, calibration, dogfood,
agent/LSP projections, CI experiments, refactors, and proof-building.

`EffortlessMetrics/unsafe-review` is the public source-of-record and release
repo. It receives curated promotions from green swarm work, release prep,
publication receipts, public package/docs.rs/crates.io metadata, and urgent
published-user hotfixes.

Do not open routine implementation PRs directly in `unsafe-review`. If a task
starts in source and is not release/public-surface/hotfix work, move the work
to `unsafe-review-swarm` or stop and leave a handoff explaining the routing
problem.

When there is no narrower owner instruction, continue improving this codebase in
`unsafe-review-swarm` along the current rails. Good default work includes
ReviewCard correctness, evidence precision, artifact/schema verification,
first-run UX, saved LSP/agent projections, dogfood calibration, tests,
maintainability refactors, and source-of-truth spec alignment. Do not treat the
absence of a new direction as a reason to pause if the work clearly advances one
of those rails.

Assume the repository has an active multi-day improvement runway. New Codex Web
PR batches on `unsafe-review-swarm` are expected input, not a reason to change
direction. Keep burning down generated PRs, tightening rails, and improving the
codebase inside the current advisory ReviewCard-centered lane unless the owner
explicitly changes the lane.

Before routine swarm implementation, run the source sync guard:

```bash
rtk cargo run --locked -p xtask -- source-divergence
```

If `new_source_commits` is nonzero, repair or acknowledge the source-to-swarm
sync before continuing feature work unless a current handoff already covers the
source-only exception.

## Worktree and branch hygiene

Start every repo operation by inspecting the current branch, dirty state, PR
queue, and source/swarm sync posture. A dirty local checkout, stale branch, or
Codex session branch is not the repository state.

When the primary checkout has unrelated local changes, create a fresh worktree
from current `origin/main` for the PR-sized task instead of editing through the
dirty tree:

```bash
rtk git fetch origin
rtk git worktree add -b <branch> <path> origin/main
```

Do not reset, rebase, delete, or rewrite another worktree to make room for a
new task. Treat existing dirty worktrees as owner or in-flight agent work unless
the owner explicitly asks you to clean them.

After a PR merges, verify the merged `origin/main` state with the relevant proof
commands, then remove only the temporary worktree you created and only after
its status is clean.

Use the repo source-of-truth stack:

1. Read `.rails/goals/active.toml`.
2. Read the linked plan item.
3. Read the linked spec.
4. Read the linked proposal only for context.
5. Make one PR-sized change.
6. Update support tiers or policy ledgers only if the claim/policy changes.
7. Run the proof commands listed in the plan item.
8. Do not invent missing claims. If proof is missing, keep the claim advisory/experimental.
9. Do not use `.jules`, `.codex`, or product runtime output directories as unsafe-review source-of-truth state.
10. Do not stop at "human merge required" unless the repo has that policy in a current source-of-truth file.

If a specific command, lint, API, feature flag, crate name, or workflow name is mentioned, verify it exists before building a PR around it.

Spec rails are meant to make routine progress easier, not ceremonial. If a PR or
agent task references a not-yet-existing rail and the rail belongs in the repo,
add or align it in `.rails` or the corresponding `docs/specs/`
contract. Prefer the smallest useful rail: a plan item, spec clause, template,
or verifier hook that keeps future PRs pointed at the same truth without adding
fake enforcement.

For CI, workflow, PR artifact, or comment-posting work, read
`docs/specs/UNSAFE-REVIEW-SPEC-0024-ci-design.md` before editing. Keep the lane
split intact: default CI protects workspace and policy health, first-pr lanes
verify advisory packet integrity, source-divergence reports source/swarm drift,
coverage remains telemetry, release readiness stays explicit, and trusted
comment posting remains a future split-token lane. Do not turn advisory
unsafe-review findings into default CI failures.

Use the rails as forward drive. A well-designed missing rail is usually a repo
alignment task, not a blocker. Add it when it keeps future work convergent and
does not turn the current PR into a broad process rewrite.

Do not reject a useful generated PR just because it references missing but
well-designed scaffolding. Decide whether the missing rail should exist. If yes,
add or align it in the same PR when the scope stays small, or leave an explicit
follow-up when it would turn the PR into a mixed-scope change.

Do not put durable repo operating state in `.codex`; keep agent-local state
there only if a local tool requires it. Durable unsafe-review repo state belongs
under `.rails`, `docs/specs`, or the documented handoff/status
surfaces.

## Helper roles and workflows

Use available helper agents, skills, or workflows when they make a PR-sized
task cleaner, but keep them bounded to the current slice. Useful default roles
are:

- repo discovery before choosing or scoping work,
- test authoring for fixtures, verifier rails, and regression goldens,
- docs changes for specs, handoffs, ledgers, and user-facing wording,
- config/environment review for CI, workflow, runner, and policy edits,
- code review for incoming PR disposition and risk checks,
- pull-request workflow for scoped staging, PR creation, hosted-check watching,
  merge, post-merge proof, and cleanup,
- release/publication workflow only for source-owned release prep, publish, and
  receipt tasks.

If a named helper is unavailable, perform the same role explicitly. Do not let a
helper create a second source of truth, widen the PR scope, skip validation, or
turn advisory unsafe-review findings into enforcement.

## Model routing and orchestration economics

Use the cheapest model that can produce a checked artifact for the current
phase, and escalate only when synthesis, integration, or risk requires it. The
shape mirrors the product itself: cheap bounded sensors emit evidence, an
orchestrator compiles it, and the deterministic gate — never the model —
decides pass/fail.

```text
cheap parallel discovery -> structured evidence packets -> capable-model
synthesis or implementation -> cheap independent verification ->
deterministic gate -> cleanup / ledger / issue filing
```

Default routing:

- **Discovery, classification, refutation, claim scans, log triage, cleanup
  audits**: cheap fast model (Haiku-class), many bounded passes with distinct
  roles, each returning an evidence packet (facts, paths, commands, status,
  uncertainty, next action) — not essays.
- **Implementation, integration tradeoffs, PR bodies, release sequencing**:
  mid model (Sonnet-class), started only after discovery has made the task
  legible, briefed with objective, scope, non-goals, evidence, acceptance
  criteria, and proof commands.
- **Architecture arbitration, cross-repo conflicts, high-cost-of-wrongness
  decisions**: top model (Opus-class), rarely.

Project subagent roles live in `.claude/agents/` (repo-preflight,
claim-boundary, plan-refuter, artifact-verifier, ci-log-triage,
cleanup-auditor, implementer) with the model pinned per role. Equivalent roles
in other harnesses should follow the same routing.

Rules that keep the economics honest:

- The writer never grades itself: a different, cheaper pass verifies every
  meaningful diff (claim scan, artifact check, or refutation) before the
  deterministic gate runs.
- Ask verifiers checkable questions ("does this diff violate SPEC-0032?"),
  never "is this good?".
- On disagreement between cheap passes, escalate the specific conflict — not
  the whole task — to the next model tier.
- Keep bulk content (logs, large diffs, raw JSON, inventories) in subagent
  contexts; the main context holds objective, plan, decisions, artifact paths,
  validation status, and next action.
- Stable doctrine belongs in cacheable prefixes (this file, specs, schemas);
  per-task content stays in the suffix. Do not bake timestamps or run ids into
  reusable prompts.
- The LLM proposes; the deterministic floor disposes. No model verdict ever
  substitutes for `check-pr`, the test suite, or the required CI check.

## PR queue discipline

Review PRs before merging them. Do not merge a batch blindly because checks are
green.

For Codex Web or other generated PR batches:

1. Work one PR at a time from the base of the stack when dependencies are
   obvious.
2. Inspect the stated intent and actual diff.
3. Verify the change is scoped to the PR title and does not create a second
   analyzer truth outside `ReviewCard`.
4. Check whether the change advances an active `.rails` plan item,
   a documented projection contract, or a narrow maintainability/test goal. If
   it creates a useful new rail, land the rail in the source-of-truth stack with
   the same PR or a clearly linked follow-up.
5. Run the narrow validation first, then broader repo gates when practical.
6. Merge only when the diff is scoped, checks are green, and the repository
   state supports the disposition.
7. If a PR is stale, conflicting, duplicate, or superseded, leave a
   repository-level disposition comment that names the replacement PR, commit,
   or future lane.

Out-of-lane is a scheduling fact, not a close reason. If an aligned PR is not
in the current lane, leave it open as deferred, draft, blocked, or parked and
name the next lane or owner decision needed. Close only duplicate, superseded,
rejected, abandoned, or unrecoverable work, and record the repository-level
evidence for that disposition.

Automation-review PRs such as Droid/MiniMax follow the same rule: aligned but
parked automation stays visible. Capture actionable bot findings and the next
validation path in a PR comment or handoff, but do not reopen automation work
solely to satisfy an unrelated lane unless the owner asks.

Agent runtime state is not PR state. Never close or mark a PR superseded because
the current Codex session is busy, capped, on another branch, or working on a
different PR. Those are handoff facts, not repository facts.

In this one-contributor repository, a merge blocked solely by branch policy that
requires an external approval is a configuration obstacle, not a repository
quality finding. If local validation and hosted checks are green and the owner
has authorized the lane, document the evidence and use the appropriate merge
path rather than parking the PR as externally blocked.

## Product boundaries

`ReviewCard` is the canonical product unit. CLI output, JSON, Markdown PR
summary, SARIF, saved LSP diagnostics, hovers, code actions, agent packets,
repo inventory, badges, baselines, suppressions, and witness receipts must
project from ReviewCard rather than creating separate truths.

Keep unsafe-review advisory in v0.x:

- no witness execution by default,
- no automatic comments,
- no source edits,
- no default blocking policy,
- no broad suppressions as a substitute for evidence,
- no safety, UB-free, Miri-clean, site-execution, proof, or calibrated
  precision/recall claims.

Optimize for card correctness before analyzer breadth. Evidence must be
obligation-level: a length guard does not discharge alignment, a `SAFETY`
comment is not a guard, and a targeted test is not site-execution proof unless
a receipt proves it.

When in doubt, preserve the product sentence:

```text
unsafe-review finds unsafe Rust changes missing a safety contract, guard, test, or witness.
```
