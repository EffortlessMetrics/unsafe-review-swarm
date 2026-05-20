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

As of 2026-05-20, swarm routing is proven and branch protection is enabled.
Issue #2 was closed after org-runner discovery, direct self-hosted smoke runs,
the forced busy-runner matrix, and normalized branch protection were all
recorded.

Initial setup proof:

```text
source seed:
  EffortlessMetrics/unsafe-review@8668b2f682064ba65f63e693079ed91f01593602

installed workflow:
  https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1

docs-only quieting proof:
  https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/3
  https://github.com/EffortlessMetrics/unsafe-review-swarm/actions/runs/26121881049
```

The docs-only proof selected `router_target=none` with
`router_reason=docs_only_no_rust_work`. All Rust implementation jobs were
skipped and the normalized result passed without consuming a self-hosted runner.

Org-runner routing proof:

```text
router implementation:
  https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/73

manual occupier workflow:
  https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/74

direct smoke:
  CX43: 26144439267 / em-ci-hel2-cx43-rust-01
  CX53: 26144440130 / em-ci-hel2-cx53-rust-01

routed main:
  CX43 full gate: 26144850143 / router_reason=cx43_idle
  CX53 full gate: 26144683454 / router_reason=cx53_idle
  GitHub fallback: 26144664132 / router_reason=no_idle_runner

forced matrix:
  CX43 unavailable -> CX53 selected: 26145432225 / router_reason=cx53_idle
  CX43 and CX53 unavailable -> GitHub-hosted: 26145526571 / router_reason=no_idle_runner
```

The old repository-runner API failure is no longer the active route path. Current
runs log:

```text
router_endpoint=orgs/EffortlessMetrics/actions/runners?per_page=100
```

and must use `cx43_idle`, `cx53_idle`, or `no_idle_runner` outcomes. A fresh
`runner_api_failed` result is a regression and should block expanding branch
protection or moving additional repos onto this route.

Branch protection on `main` now requires only:

```text
Unsafe Review Rust Result
```

The conditional jobs remain intentionally unprotected:

```text
Route Unsafe Review Rust
Unsafe Review Rust on CX43
Unsafe Review Rust on CX53
Unsafe Review Rust on GitHub Hosted
```

`EM CI Runner Occupier` remains manual-only diagnostic infrastructure. It has
only `contents: read` permission, does not use secrets, is not triggered by pull
requests, and must not become a required branch-protection check.
