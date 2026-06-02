# PR Release Discipline

Release trains may define a cutline. They may not erase work.

This process applies to `unsafe-review-swarm` PR triage, generated PR batches,
source/sync bookkeeping, release preparation, publication, and publication
receipt work.

## Core Rule

An empty open PR queue is not the goal. Truthful disposition is the goal.

Every PR must be in exactly one of these states:

- open and awaiting review, fix, rebase, CI, owner decision, or a later lane;
- merged after review and validation;
- closed as duplicate or superseded by a named merged PR or commit;
- recreated from current `main`, with the original PR linked;
- explicitly abandoned by owner decision.

No other disposition counts.

## Forbidden Closure Reasons

Do not close a useful PR for:

- release timing;
- queue cleanup;
- clean queue optics;
- review quota;
- CI budget;
- local Codex or agent limits;
- stale local checkout state;
- broad "out of lane" wording without preserving the PR as open or recreated;
- "probably stale" without inspecting the diff against current `main`;
- conflicts that have not been evaluated against the useful work in the PR.

Review quota, CI budget, and release timing are scheduling facts, not repository
quality findings.

## Required Triage Sequence

Start from current repository state before every PR disposition pass:

```bash
rtk git fetch origin
rtk git status --short --branch
rtk git pull --ff-only
rtk gh pr list --repo EffortlessMetrics/unsafe-review-swarm --state open --limit 100
rtk gh pr list --repo EffortlessMetrics/unsafe-review-swarm --state closed --limit 100
rtk cargo run --locked -p xtask -- source-divergence
```

If local `main` was behind, refresh the conclusion after pulling. Do not use
stale local state as PR state.

For each PR:

1. Inspect intent, files, commits, review comments, checks, and current-main
   overlap.
2. Decide whether the PR advances a repo goal, source-of-truth rail, test goal,
   refactor goal, release/sync task, or owner-requested feature.
3. If useful but stale, keep it open or recreate it from current `main`.
4. If duplicate, name the exact merged PR or commit that already contains the
   useful work.
5. If abandoned, record the owner decision.

## Generated Refactor PRs

Generated refactors are not junk by default.

Triage them by module family and responsibility:

- merge the cleanest useful shape first;
- compare alternate branches against the merged shape;
- preserve distinct useful extraction, tests, or verifier changes by rebasing
  or recreating from current `main`;
- close only true duplicates, with the merged replacement named.

Do not flatten "many refactors" into "stale backlog".

## Release Trains

Release trains may park work outside the release cutline, but they must not
hide it.

Allowed release-time handling:

- keep the PR open with a comment naming the next step;
- label or document it as post-release;
- recreate it from current `main` if the branch is stale;
- close only if duplicate, superseded by a named merged PR or commit, or
  abandoned by owner decision.

Not allowed:

- close because publication is next;
- close because the release queue should be clear;
- close because a workflow or reviewer budget is unavailable;
- use a release as a reason to discard implementation context.

## Source And Publication Claims

Before saying publication is blocked, read the repository runbook and check the
repo-specific credential path:

```bash
rtk sed -n '1,180p' docs/releases/CRATES_IO_PATCH_RELEASE.md
rtk rg -n "publish|crates.io|credential|token|CARGO|release" docs .cargo .github -S
```

Absence of `CARGO_REGISTRY_TOKEN` or a home cargo credentials file is not by
itself proof that the repository cannot publish.

For synchronized unsafe-review patch releases, publish dependency crates first:

```bash
rtk cargo publish -p unsafe-review-core
rtk cargo publish -p unsafe-review-cli --dry-run
rtk cargo publish -p unsafe-review-cli
rtk cargo publish -p unsafe-review --dry-run
rtk cargo publish -p unsafe-review
```

Downstream dry-run failures before upstream crates are published can be expected
topological failures. Treat them as publish-order evidence, not a code failure.

## Recovery Mode

If PRs may have been closed for queue optics, release timing, stale local state,
or any other forbidden reason:

1. Stop publication and new feature work.
2. Restore visibility first: reopen recoverable PRs before judging them.
3. Create a handoff audit listing every affected PR, current state, branch
   existence, files, close reason, replacement claim, evidence, and final
   disposition.
4. Recreate branches whose head refs are gone if the useful work is not already
   represented by a named merged PR or commit.
5. Resume release work only after every PR has a truthful disposition.

## Boundary

This process does not make unsafe-review findings blocking. It does not execute
witnesses, post comments automatically, edit source, prove memory safety,
prove UB-free status, prove Miri-clean status, prove site execution, claim
calibrated precision/recall, or make policy-readiness claims.
