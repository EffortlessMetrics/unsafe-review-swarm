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
