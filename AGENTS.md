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

Before routine swarm implementation, run the source sync guard:

```bash
rtk cargo run --locked -p xtask -- source-divergence
```

If `new_source_commits` is nonzero, repair or acknowledge the source-to-swarm
sync before continuing feature work unless a current handoff already covers the
source-only exception.

Use the repo source-of-truth stack:

1. Read `.unsafe-review-spec/goals/active.toml`.
2. Read the linked plan item.
3. Read the linked spec.
4. Read the linked proposal only for context.
5. Make one PR-sized change.
6. Update support tiers or policy ledgers only if the claim/policy changes.
7. Run the proof commands listed in the plan item.
8. Do not invent missing claims. If proof is missing, keep the claim advisory/experimental.
9. Do not use `.jules`, `.codex`, or product runtime output directories as unsafe-review source-of-truth state.
10. Do not stop at “human merge required” unless the repo has that policy in a current source-of-truth file.

If a specific command, lint, API, feature flag, crate name, or workflow name is mentioned, verify it exists before building a PR around it.

Spec rails are meant to make routine progress easier, not ceremonial. If a PR or
agent task references a not-yet-existing rail and the rail belongs in the repo,
add or align it in `.unsafe-review-spec` or the corresponding `docs/specs/`
contract. Do not put durable repo operating state in `.codex`; keep agent-local
state there only if a local tool requires it.

## PR queue discipline

Review PRs before merging them. Do not merge a batch blindly because checks are
green.

For Codex Web or other generated PR batches:

1. Work one PR at a time from the base of the stack when dependencies are
   obvious.
2. Inspect the stated intent and actual diff.
3. Verify the change is scoped to the PR title and does not create a second
   analyzer truth outside `ReviewCard`.
4. Check whether the change advances an active `.unsafe-review-spec` plan item,
   a documented projection contract, or a narrow maintainability/test goal. If
   it creates a useful new rail, land the rail in the source-of-truth stack with
   the same PR or a clearly linked follow-up.
5. Run the narrow validation first, then broader repo gates when practical.
6. Merge only when the diff is scoped, checks are green, and the repository
   state supports the disposition.
7. If a PR is stale, conflicting, duplicate, or superseded, leave a
   repository-level disposition comment that names the replacement PR, commit,
   or future lane.

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
