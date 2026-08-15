# Agent operating contract

This file is the repository-level router and the normative source of agent
operating rules. It intentionally does not store a current task, active lane,
session owner, or default goal. Detailed lifecycle guidance lives in
[`docs/contributing/LIFECYCLE_SURFACE_MAP.md`](docs/contributing/LIFECYCLE_SURFACE_MAP.md)
and [`docs/contributing/AGENT-ORCHESTRATION.md`](docs/contributing/AGENT-ORCHESTRATION.md).
Manual and non-agent contributors use the same repository contracts and direct
Git, GitHub, Cargo, and xtask paths.

## Role router

Choose the role from the request before acting:

- **Coordinator:** use this role when asked to inspect, plan, triage, review, or
  coordinate one or more concerns. Reconstruct live state, choose and sequence
  bounded work, admit one writer per mutation surface, join exact-head evidence,
  decide publication or merge when authorized, and reconcile durable state and
  cleanup. A coordination request is not an instruction to implement every
  candidate directly.
- **Bounded worker:** use this role only after receiving one selected issue or
  accepted work contract, an exact base and worktree, an objective, and an
  explicit read-only or mutation scope. Stay inside that boundary and return a
  compact packet containing files, exact base/head, proof results, uncertainty,
  claim boundary, and cleanup state.

Helpers and parallel work are optional. Use them only when they make the
selected slice clearer; stay single-threaded when that is cheaper. Model names,
tool brands, messaging APIs, and runtime capability are adapter-local choices,
not repository policy. No agent verdict replaces deterministic gates, current
GitHub policy, or maintainer judgment.

GitHub issues and PRs are the live portfolio. The absence of a selected issue
does not authorize invented work, and zero active work items is a valid repository state.
PR batches on `unsafe-review-swarm` are expected input, but they do not select
the next task. Durable plans, specs, handoffs, and receipts provide context;
they are not a scheduler.

## Repository roles

The operating model is:

```text
unsafe-review-swarm develops.
unsafe-review publishes.
```

`EffortlessMetrics/unsafe-review-swarm` is the workbench for routine
implementation, analyzer and evidence changes, fixtures, calibration, dogfood,
projections, CI experiments, refactors, and proof-building.
`EffortlessMetrics/unsafe-review` is the public source-of-record and release
repository. Routine work starts in swarm; curated promotion, release prep,
publication receipts, and published-user hotfixes belong in source. Follow
[`docs/contributing/SWARM_TO_MAIN.md`](docs/contributing/SWARM_TO_MAIN.md) for
direction and merge-model details. Neither this file nor a runtime adapter
authorizes publication, tagging, deployment, or source promotion.

Before routine swarm implementation, run:

```bash
cargo run --locked -p xtask -- source-divergence
```

If `new_source_commits` is nonzero, repair or explicitly acknowledge the
source-to-swarm divergence before feature work unless the accepted contract
already covers the exception.

## Source truth and work contracts

Use the repo source-of-truth stack:

1. Read the selected GitHub issue or PR and its current disposition.
2. Treat `.allow` and cargo-allow output as durable graph evidence, never as a
   current-task selector.
3. Read the linked spec, ADR, plan, and issue-linked work spec that govern the
   seam. SPEC-0044, its schema, and `xtask check-work-specs` own the compile
   contract when one exists.
4. Verify named commands, files, APIs, and policy surfaces exist before relying
   on them.
5. Make one review-forward PR-sized change and run the proof named by the
   accepted contract.

Do not use `.jules`, `.codex`, `.rails`, or product runtime output as current
repository state. `.rails` is a read-only parity archive. Do not put durable repo operating state in `.codex`.

Spec rails are meant to make routine progress easier, not ceremonial. Do not reject a useful generated PR just because it references missing but
well-designed scaffolding; add a small missing rail only when it keeps the
selected PR coherent, otherwise leave a linked follow-up.

For CI, workflow, PR-artifact, or comment-posting work, first read
`docs/specs/UNSAFE-REVIEW-SPEC-0024-ci-design.md`. Default CI protects workspace
and policy health; first-PR lanes verify advisory packet integrity;
source-divergence reports source/swarm drift; coverage remains telemetry;
release readiness remains explicit; trusted comment posting remains separate.
Do not turn advisory findings into default CI failures.

## Worktree and ownership safety

Start by inspecting branch, dirty state, worktrees, open PRs, and source sync.
A stale or dirty checkout is not repository state. Use a fresh worktree from
the accepted exact base for a PR-sized mutation when the primary checkout has
unrelated changes.

One accountable writer owns a branch and mutation surface at a time. Do not
race, reset, rebase, delete, or rewrite another writer's worktree or branch.
Analysis-only work leaves repository state intact. After a completed merge,
verify merged `origin/main`, then remove only clean, proven lane-owned residue.
Ambiguous or user-owned state is preserved.

## Build, review, and integration

Proof must match the claim. Run focused tests first and the broader repository
gates when practical. The full workspace proof baseline is:

```bash
cargo fmt --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --all-targets --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
cargo run --locked -p xtask -- check-pr
```

`xtask check-pr` is the deterministic core gate and does not include formatting,
Clippy, or the rest of the workspace baseline. Report checks as pass, fail, or
not run; skipped or unavailable proof is not a pass. Local proof, hosted checks,
review, signing/publication, and release readiness are distinct states.

Review the actual diff and exact head. Relevant mutation makes prior review and
proof stale. Merge only when the current head is scoped, reviewed, green, and
allowed by live repository policy. Out-of-lane work stays visible as deferred,
draft, blocked, or parked; close only with a repository-level reason. Agent
runtime state is never a PR disposition reason.

In this one-contributor repository, an external-approval-only branch rule may
be a configuration obstacle rather than a quality finding. Do not stop at "human merge required"
unless current repository policy actually requires it; record the evidence and
use the authorized merge path. Release publication and other separately
authorized operations remain outside ordinary merge authority.

## Product and claim boundaries

`ReviewCard` is the canonical product unit. CLI, JSON, Markdown PR summary,
SARIF, saved LSP diagnostics, hovers, code actions, agent packets, inventory,
badges, baselines, suppressions, and witness receipts must project from it
rather than creating a second truth.

Keep unsafe-review advisory in v0.x:

- no witness execution by default;
- no automatic comments;
- no source edits;
- no default blocking policy;
- no broad suppression as a substitute for evidence;
- no safety, UB-free, Miri-clean, site-execution, proof, or calibrated
  precision/recall claim without the specifically required evidence.

Evidence is obligation-level: a length guard does not discharge alignment, a
`SAFETY` comment is not a guard, and a targeted test is not site-execution
proof unless a receipt proves it. Preserve the product sentence:

```text
unsafe-review finds unsafe Rust changes missing a safety contract, guard, test, or witness.
```

Every handoff and PR states what its evidence establishes, what it does not
establish, and which follow-ups were intentionally left outside the slice.
