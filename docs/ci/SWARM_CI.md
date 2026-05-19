# unsafe-review-swarm CI

`EffortlessMetrics/unsafe-review-swarm` is the day-to-day development landing
zone for unsafe-review. `EffortlessMetrics/unsafe-review` remains the public,
source, release, signing, and publish surface until promotion is deliberately
handled.

The swarm CI result to protect is:

```text
Unsafe Review Rust Result
```

Do not protect the conditional implementation jobs. They are intentionally
skipped depending on the router result:

```text
Route Unsafe Review Rust
Unsafe Review Rust on CX43
Unsafe Review Rust on CX53
Unsafe Review Rust on GitHub Hosted
```

## Routing

The router runs on GitHub-hosted Linux and chooses:

```text
same-repo PR or workflow_dispatch:
  CX43 if an em-ci/cx43/rust-small/trusted-pr runner is idle
  CX53 if an em-ci/cx53/rust-small/trusted-pr runner is idle
  GitHub-hosted otherwise

fork PR:
  GitHub-hosted only

docs/Markdown-only PR:
  no Rust implementation job
  normalized result succeeds with "docs-only/no Rust work"
```

This keeps public fork PRs off self-hosted machines and avoids `paths-ignore`,
which would leave required checks missing after branch protection is enabled.

The runner group is expected to be `em-ci-small` with selected repository access
for `unsafe-review-swarm`. The router token must be scoped to this repository.

## Gate

Every non-docs route runs:

```bash
cargo fmt --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo run --locked -p xtask -- check-pr
```

The same job renders and verifies the advisory artifacts:

```text
target/unsafe-review/cards.json
target/unsafe-review/pr-summary.md
target/unsafe-review/cards.sarif
target/unsafe-review/comment-plan.json
```

These artifacts remain advisory. The workflow does not run Miri, sanitizers,
Loom, Kani, cargo-careful, witness import, automatic PR comments, source edits,
release, signing, crates.io publish, or blocking policy.

## Branch Protection Proof

Defer branch protection until these checks have passed:

```text
1. workflow PR passes
2. manual dispatch on main passes
3. tiny same-repo PR passes
4. CX43 busy -> CX53 selected
5. CX43 and CX53 busy -> GitHub-hosted selected
6. docs-only PR returns normalized success without VPS use
7. 3-5 real PRs pass
```

Only then require `Unsafe Review Rust Result`.

## Current Proof Status

As of 2026-05-19:

```text
source seed:
  EffortlessMetrics/unsafe-review@8668b2f682064ba65f63e693079ed91f01593602

installed workflow:
  https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1

manual dispatch proof:
  https://github.com/EffortlessMetrics/unsafe-review-swarm/actions/runs/26121565219

docs-only quieting proof:
  https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/3
  https://github.com/EffortlessMetrics/unsafe-review-swarm/actions/runs/26121881049
```

The manual dispatch succeeded through GitHub-hosted fallback and uploaded the
`unsafe-review` artifact bundle. The normalized `Unsafe Review Rust Result`
passed.

The docs-only proof selected `router_target=none` with
`router_reason=docs_only_no_rust_work`. All Rust implementation jobs were
skipped and the normalized result passed without consuming a self-hosted runner.

Self-hosted routing is not proven yet. The router saw `EM_RUNNER_READ_TOKEN`,
but the repository runner API returned `HTTP 403`, so it selected
`runner_api_failed` -> `github`. Track the org-side runner-group and token setup
in:

```text
https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/2
```

Branch protection remains deferred until CX43, CX53, GitHub-hosted fallback for
the busy-runner case, and several real PRs have all been proven.
