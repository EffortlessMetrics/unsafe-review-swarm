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
for `unsafe-review-swarm`. The router discovers CX43/CX53 through the
organization runner endpoint:

```text
orgs/EffortlessMetrics/actions/runners?per_page=100
```

`EM_RUNNER_READ_TOKEN` must be exposed to this repository as an Actions secret
and must have organization `Self-hosted runners: read` permission. The route job
logs `router_endpoint` next to `router_target` and `router_reason` so API
authorization failures are distinguishable from clean busy-runner fallback.

The implementation jobs use both the `em-ci-small` runner group and labels:

```yaml
runs-on:
  group: em-ci-small
  labels: [self-hosted, em-ci, cx43, rust-small, trusted-pr]
```

GitHub-hosted fallback remains available when no matching runner is idle.

Self-hosted Rust jobs use repo-specific cache subdirectories:

```text
CARGO_HOME=/mnt/ci-cache/cargo-home/unsafe-review
SCCACHE_DIR=/mnt/ci-cache/sccache/unsafe-review
```

This avoids mutating or depending on stale root-owned entries in broader shared
cache directories while still keeping the cache on the VPS cache volume.

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
1. manual CX43 smoke workflow passes
2. manual CX53 smoke workflow passes
3. routed workflow PR passes
4. manual dispatch on main passes
5. tiny same-repo PR passes
6. CX43 busy -> CX53 selected
7. CX43 and CX53 busy -> GitHub-hosted selected
8. docs-only PR returns normalized success without VPS use
9. 3-5 real PRs pass
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

Self-hosted routing is not proven yet. The previous router saw
`EM_RUNNER_READ_TOKEN`, but the repository runner API returned `HTTP 403`, so it
selected `runner_api_failed` -> `github`. Track the org-side runner-group and
token setup in:

```text
https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/2
```

Branch protection remains deferred until CX43, CX53, GitHub-hosted fallback for
the busy-runner case, and several real PRs have all been proven.

The next proof step is to run the manual `EM CI Self Hosted Smoke` workflow for
both CX43 and CX53. If those jobs schedule and pass, the scheduler can use the
`em-ci-small` group for this repository and the remaining proof is router
discovery through the organization runner endpoint. A clean busy-runner fallback
must report:

```text
router_endpoint=orgs/EffortlessMetrics/actions/runners?per_page=100
router_target=github
router_reason=no_idle_runner
```

`router_reason=runner_api_failed` remains a blocker.

As of 2026-05-20, PR #73 moved discovery to the organization runner endpoint
and proved the old `HTTP 403` path is no longer the active failure mode:

```text
direct smoke:
  CX43: 26144439267 / em-ci-hel2-cx43-rust-01
  CX53: 26144440130 / em-ci-hel2-cx53-rust-01

routed main:
  CX43 full gate: 26144850143 / router_reason=cx43_idle
  CX53 full gate: 26144683454 / router_reason=cx53_idle
  GitHub fallback: 26144664132 / router_reason=no_idle_runner
```

The remaining proof is the forced busy-runner matrix. Use the manual
`EM CI Runner Occupier` workflow only for that diagnostic proof:

```text
1. occupy CX43, then dispatch Unsafe Review Rust
   expected: router_target=cx53, router_reason=cx53_idle

2. occupy CX43 and CX53, then dispatch Unsafe Review Rust
   expected: router_target=github, router_reason=no_idle_runner
```

`EM CI Runner Occupier` has only `contents: read` permission, does not use
secrets, is not triggered by pull requests, and must not become a required
branch-protection check. Branch protection remains deferred until the forced
matrix and enough real PRs have passed.
