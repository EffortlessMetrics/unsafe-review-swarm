# Source history catch-up

`unsafe-review-swarm` is the workbench. It squash-merges each reviewed PR into
one `main` commit. Those commits are the durable review units for source-relevant
work.

`unsafe-review` is the public source-of-record for releases. It must not lose the
review history behind source-relevant swarm work.

## When to use this runbook

Use this runbook when:

- `unsafe-review` is hundreds of commits behind `unsafe-review-swarm`,
- source is missing product-relevant swarm commits,
- a published crate surface depends on a swarm commit not present in source,
- curated source promotions have already flattened too much history,
- source must become history-aware before another patch release.

Do not use this for routine narrow source promotion.

## Forbidden fixes

Do not:

- copy the final swarm tree into source as one sync commit,
- squash already-squashed swarm PR commits again,
- use an `ours` merge when source is missing tree changes,
- force-push source `main`,
- rewrite existing release tags,
- publish a patch release before source history is repaired.

An `ours` merge is allowed only for pure ancestry bookkeeping when the source
tree is already known-equivalent and no reviewed swarm tree change needs to
land. It is forbidden when source is missing product-relevant tree state, such
as a crate README packaging fix.

## Required merge model

Create a source branch from current `unsafe-review/main`.

Add or fetch the swarm remote:

```bash
rtk git remote add swarm <swarm-remote-if-missing>
rtk git fetch origin main
rtk git fetch swarm main
```

Create the catch-up branch:

```bash
rtk git switch -c sync/swarm-history-catchup origin/main
```

Merge swarm with real tree merge semantics:

```bash
rtk git merge --allow-unrelated-histories --no-ff swarm/main
```

Resolve conflicts deliberately. Prefer swarm for files where source is stale
and swarm contains reviewed product-relevant state. Inspect release-critical
files manually.

## Required checks

Check for unresolved merge state:

```bash
rtk proxy git diff --check
rtk rg "^<<<<<<<|^=======|^>>>>>>>" -n
```

Run the source validation suite:

```bash
rtk cargo fmt --check
rtk cargo check --workspace --all-targets --locked
rtk cargo clippy --workspace --all-targets --locked -- -D warnings
rtk cargo test --workspace --locked
rtk cargo run --locked -p xtask -- check-pr
rtk cargo run --locked -p xtask -- check-calibration
rtk cargo run --locked -p xtask -- check-dogfood
```

Run a release-surface smoke:

```bash
rtk cargo run --locked -p unsafe-review -- first-pr --root fixtures/raw_pointer_alignment --diff fixtures/raw_pointer_alignment/change.diff --out-dir target/unsafe-review-history-catchup-smoke
rtk cargo run --locked -p xtask -- check-first-pr-artifacts target/unsafe-review-history-catchup-smoke
rtk cargo run --locked -p unsafe-review -- support
```

Verify history and key tree state:

```bash
rtk proxy git merge-base --is-ancestor <key-swarm-commit> HEAD
rtk proxy git merge-base --is-ancestor swarm/main HEAD
rtk proxy git log --oneline --parents -1
rtk rg "raw.githubusercontent.com/EffortlessMetrics/unsafe-review/main/unsafe-review-logo.svg" crates/unsafe-review/README.md
```

Also inspect the remaining tree difference from swarm:

```bash
rtk proxy git diff --stat swarm/main..HEAD
rtk proxy git diff --name-status swarm/main..HEAD
```

That diff must be explainable. It should not show accidental source-only stale
files such as duplicate Rust modules, old workflows, stale crate README content,
or release metadata drift.

## PR requirements

The source PR must state:

```text
This PR preserves unsafe-review-swarm PR-squashed history in unsafe-review.

It must be merged with a merge commit.
Do not squash or rebase.
```

The PR body should identify:

- the previous source `main` parent,
- the swarm `main` parent used for catch-up,
- key swarm commits that must be reachable,
- release-critical files inspected manually,
- any remaining source/swarm tree differences,
- validation results.

Merge with:

```bash
gh pr merge <PR> --repo EffortlessMetrics/unsafe-review --merge --delete-branch
```

If merge commits are blocked, stop and fix repository settings or branch policy.

## Boundary

This runbook repairs repository history and source tree state. It does not
publish crates, change versions, run Bun scans, add analyzer behavior, start
policy gating, execute witnesses, post comments, edit downstream source, or make
proof, UB-free, Miri-clean, site-execution, calibrated precision/recall, or
policy-readiness claims.
